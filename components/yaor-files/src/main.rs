// yaor-files — gestionnaire de fichiers du bureau YAOR (v0.6 étape 7).
//
// Client xdg-shell régulier : fenêtre flottante décorée par le SSD du
// compositor (feux tricolores + titre), rendu wl_shm au rastérisateur
// yaor-ui (police réelle). v0.6 minimal assumé : listing du répertoire
// courant (std::fs, dossiers d'abord, tri alphabétique), clic sur un
// dossier = navigation dans, « .. » = remontée, clic sur un fichier =
// lecture texte en lecture seule (premières lignes) ; tout clic en vue
// texte revient à la liste. Pointeur sctk (position absolue lignes).
//
// Modèle : yaor-panel (sctk) et examples/simple_window.rs de sctk
// 0.19.2. Rendu logiciel Argb8888. Ack des configure géré par sctk.
//
// Journal : stderr préfixé « yaor-files: » (jetons QA courts).
use std::fs;
use std::path::PathBuf;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};

use yaor_ui::Theme;

const PROG: &str = "yaor-files";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const WIN_MIN: (u32, u32) = (320, 240);
const MARGIN: i32 = 10;
const ROW: i32 = 22;
const TEXT_SIZE: u32 = 13;

/// Vue courante.
enum Vue {
    /// Liste du répertoire : (chemin, (nom, est_dossier)) dans l'ordre
    /// d'affichage.
    Liste(PathBuf, Vec<(String, bool)>),
    /// Lecture texte d'un fichier : (chemin, nom, lignes, tronquee).
    Texte(PathBuf, String, Vec<String>, bool),
}

// ---------------------------------------------------------------------
// État du client
// ---------------------------------------------------------------------

struct Files {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    // compositor/xdg détenus (cf. compositor.create_surface,
    // xdg.create_window) mais les handler-ids vivent dans surface et
    // window : aucun accès direct après le branchement.
    #[allow(dead_code)]
    compositor: CompositorState,
    #[allow(dead_code)]
    xdg: XdgShell,
    exit: bool,
    first_configure: bool,
    pool: SlotPool,
    width: u32,
    height: u32,
    window: Window,
    theme: Theme,
    pointer: Option<wl_pointer::WlPointer>,
    vue: Vue,
}

impl Files {
    /// Listing du répertoire : dossiers d'abord, tri alphabétique, « .. »
    /// en tête (sauf à la racine).
    fn reload(path: &PathBuf) -> Vue {
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();
        if let Ok(rd) = fs::read_dir(path) {
            for entry in rd.flatten() {
                match entry.path() {
                    p if p.is_dir() => dirs.push(entry.file_name().to_string_lossy().into()),
                    _ => files.push(entry.file_name().to_string_lossy().into()),
                }
            }
        }
        dirs.sort();
        files.sort();
        let mut rows = Vec::with_capacity(dirs.len() + files.len() + 1);
        if path.parent().is_some() {
            rows.push(("..".to_string(), true));
        }
        for d in dirs {
            rows.push((d, true));
        }
        for f in files {
            rows.push((f, false));
        }
        Vue::Liste(path.clone(), rows)
    }

    fn title(&self) -> String {
        match &self.vue {
            Vue::Liste(p, _) => format!("Fichiers — {}", p.display()),
            Vue::Texte(p, name, ..) => {
                let _ = p;
                format!("Fichiers — {name}")
            }
        }
    }

    /// Rendu : fond, en-tête, lignes (dossier en accent + «/», fichier
    /// en texte), vue texte = contenu en gris lecture seule.
    fn draw(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>) {
        let (w, h) = (self.width as i32, self.height as i32);
        if w <= 0 || h <= 0 {
            return;
        }
        let titre = self.title();
        let stride = w * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w, h, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de fichiers");

        canvas.fill(0);
        yaor_ui::fill_rect(&mut canvas, w, h, 0, 0, w, h, self.theme.panel_bg);
        yaor_ui::text::draw_text(
            &mut canvas,
            w,
            h,
            &titre,
            MARGIN,
            MARGIN,
            TEXT_SIZE,
            yaor_ui::text::Weight::Bold,
            self.theme.accent,
        );

        let mut y = MARGIN + 26;
        match &self.vue {
            Vue::Liste(_, rows) => {
                let visibles = ((h - y - MARGIN) / ROW).max(1) as usize;
                for (i, (name, is_dir)) in rows.iter().enumerate().take(visibles) {
                    let texte = if *is_dir { format!("{name}/") } else { name.clone() };
                    let couleur = match (i, *is_dir) {
                        (0, _) => self.theme.text_gray, // « .. » discret
                        (_, true) => self.theme.accent, // dossier : accent
                        (_, false) => self.theme.text_fg,
                    };
                    yaor_ui::text::draw_text(
                        &mut canvas,
                        w,
                        h,
                        &texte,
                        MARGIN + 4,
                        y,
                        TEXT_SIZE,
                        yaor_ui::text::Weight::Regular,
                        couleur,
                    );
                    y += ROW;
                }
            }
            Vue::Texte(_, _, lines, tronquee) => {
                let visibles = ((h - y - MARGIN) / ROW).max(1) as usize;
                for line in lines.iter().take(visibles) {
                    yaor_ui::text::draw_text(
                        &mut canvas,
                        w,
                        h,
                        line,
                        MARGIN + 4,
                        y,
                        TEXT_SIZE,
                        yaor_ui::text::Weight::Regular,
                        self.theme.text_fg,
                    );
                    y += ROW;
                }
                if *tronquee {
                    yaor_ui::text::draw_text(
                        &mut canvas,
                        w,
                        h,
                        "… (tronque)",
                        MARGIN + 4,
                        (h - MARGIN - 16).max(y),
                        11,
                        yaor_ui::text::Weight::Regular,
                        self.theme.text_gray,
                    );
                }
            }
        }

        self.window
            .wl_surface()
            .damage_buffer(0, 0, w, h);
        buffer
            .attach_to(self.window.wl_surface())
            .expect("attach du buffer de fichiers");
        self.window.commit();
    }
}

// ---------------------------------------------------------------------
// Handlers sctk
// ---------------------------------------------------------------------

impl CompositorHandler for Files {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Files {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for Files {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl PointerHandler for Files {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            // Ignorer les événements des autres surfaces (SSD du
            // compositor = autre surface).
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            if let PointerEventKind::Press { button, .. } = event.kind {
                if button != 0x110 {
                    continue; // bouton gauche
                }
                let (px, py) = event.position;
                let (x, y) = (px as i32, py as i32);
                let old = std::mem::replace(&mut self.vue, Vue::Liste(PathBuf::from("."), vec![]));
                self.vue = match old {
                    Vue::Liste(path, rows) => {
                        // Clic dans une ligne (en-tête exclu).
                        let idx = ((y - (MARGIN + 26)) / ROW).max(0) as usize;
                        let nv = if x < MARGIN || y < MARGIN + 26 {
                            Vue::Liste(path, rows)                        } else if let Some((name, is_dir)) = rows.get(idx).cloned() {
                            if is_dir && name == ".." {
                                let parent = path.parent().unwrap_or(&path).to_path_buf();
                                Self::reload(&parent)
                            } else if is_dir {
                                Self::reload(&path.join(&name))
                            } else {
                                // Lecture texte read-only (16 Ko, 200 lignes).
                                let p = path.join(&name);
                                let mut lines = Vec::new();
                                let mut tronquee = false;
                                if p.is_file() {
                                    if let Ok(bytes) = fs::read(&p) {
                                        if let Ok(texte) = String::from_utf8(bytes[..].to_vec())
                                        {
                                            let mut count = 0;
                                            for l in texte.lines() {
                                                if count >= 200 {
                                                    tronquee = true;
                                                    break;
                                                }
                                                lines.push(l.to_string());
                                                count += 1;
                                            }
                                        }
                                    }
                                }
                                Vue::Texte(path, name, lines, tronquee)
                            }
                        } else {
                            Vue::Liste(path, rows)
                        };
                        if let Vue::Liste(ref np, _) = nv {
                            if np != &PathBuf::from(".") {
                                eprintln!(
                                    "{PROG}: [{:.3}] navigation {}",
                                    uptime(),
                                    np.display()
                                );
                            }
                        }
                        nv
                    }
                    Vue::Texte(path, ..) => {
                        // tout clic en vue texte : retour à la liste du
                        // dossier d'origine (path conservé dans la vue).
                        Self::reload(&path)
                    }
                };
                self.window.set_title(self.title());
                self.draw(conn, _qh);
            }
        }
    }
}

impl SeatHandler for Files {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer =
                Some(self.seat_state.get_pointer(qh, &seat).expect("pointeur indisponible"));
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl WindowHandler for Files {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        if let Some(w) = configure.new_size.0 {
            self.width = w.get();
        }
        if let Some(h) = configure.new_size.1 {
            self.height = h.get();
        }
        if self.first_configure {
            self.first_configure = false;
            eprintln!(
                "{PROG}: [{:.3}] configure {w}x{h} (app_id=yaor-files)",
                uptime(),
                w = self.width,
                h = self.height
            );
            self.draw(conn, qh);
        }
    }
}

delegate_compositor!(Files);
delegate_output!(Files);
delegate_shm!(Files);
delegate_seat!(Files);
delegate_pointer!(Files);
delegate_xdg_shell!(Files);
delegate_xdg_window!(Files);
delegate_registry!(Files);

impl ProvidesRegistryState for Files {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn uptime() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

fn main() -> std::process::ExitCode {
    eprintln!("{PROG}: [{:.3}] demarre {} (listing reelle)", uptime(), VERSION);

    // Options sans connexion Wayland (vérification de distribution).
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" => eprintln!("{PROG} {VERSION}"),
            "--help" => eprintln!(
                "{PROG}: gestionnaire de fichiers du bureau YAOR (navigation + lecture)"
            ),
            _ => eprintln!("{PROG}: argument ignoré : {arg}"),
        }
        if arg == "--version" || arg == "--help" {
            return std::process::ExitCode::SUCCESS;
        }
    }

    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{PROG}: connexion wayland impossible : {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let (globals, mut event_queue) = registry_queue_init(&conn).expect("registry");
    let qh = event_queue.handle();
    let registry_state = RegistryState::new(&globals);
    let seat_state = SeatState::new(&globals, &qh);
    let output_state = OutputState::new(&globals, &qh);
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg_shell");

    let (cfg, _src) = yaor_config::DesktopConfig::load();
    let theme = Theme::from_config(&cfg.theme);
    let pool = SlotPool::new(2 * WIN_MIN.0 as usize * WIN_MIN.1 as usize * 4, &shm)
        .expect("pool");

    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_app_id(PROG);
    window.set_min_size(Some(WIN_MIN));
    window.commit();

    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut files = Files {
        registry_state,
        seat_state,
        output_state,
        shm,
        compositor,
        xdg,
        exit: false,
        first_configure: true,
        pool,
        width: WIN_MIN.0,
        height: WIN_MIN.1,
        window,
        theme,
        pointer: None,
        vue: Files::reload(&start),
    };
    files.window.set_title(files.title());
    files.window.commit();
    eprintln!("{PROG}: [{:.3}] listing {}", uptime(), start.display());

    // Boucle bloquante tenue : le compositor réveille via les configure,
    // pointeur et rendus — aucune requête périodique (comme le dock).
    while !files.exit {
        event_queue
            .blocking_dispatch(&mut files)
            .expect("boucle de files");
        let _ = conn.flush();
    }

    eprintln!("{PROG}: [{:.3}] arret", uptime());
    std::process::ExitCode::SUCCESS
}
