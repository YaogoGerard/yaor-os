// yaor-settings — app « Réglages » du bureau YAOR (v0.7 « Verre »).
//
// Client xdg-shell régulier (fenêtre décorée par le SSD du compositor),
// rendu wl_shm au rastérisateur yaor-ui. Modèle : yaor-files (sctk 0.19).
//
// AUCUNE logique de thème propre : chaque geste (carte de préset, pastille
// d'accent, curseur, bascule Plat/Verre) appelle les fonctions partagées de
// yaor-config — `presets::apply_to_text` / `presets::tune_text` — qui sont
// aussi celles de `yaor-ctl theme`, écrit le desktop.toml cible
// (`edit::update_config`, même fichier que `profile set`), puis envoie
// SIGHUP au compositor (`hup::notify_compositor`). Le compositor relaie aux
// clients du bureau : panel, dock et cette fenêtre se restylent.
//
// Curseurs : le tirage met à jour l'aperçu et le fichier n'est écrit qu'au
// RELÂCHEMENT (un SIGHUP par geste, pas par pixel).
//
// Journal : stderr préfixé « yaor-settings: » (jetons QA courts).

mod ui;

use std::os::fd::AsRawFd;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_output, delegate_pointer, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
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

use ui::{Hit, Model, MIN_H, MIN_W, PREF_H, PREF_W};
use yaor_config::{edit, hup, presets};

const PROG: &str = "yaor-settings";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const BTN_LEFT: u32 = 0x110;

struct Settings {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
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
    pointer: Option<wl_pointer::WlPointer>,
    model: Model,
}

impl Settings {
    fn draw(&mut self) {
        let (w, h) = (self.width as i32, self.height as i32);
        if w <= 0 || h <= 0 {
            return;
        }
        let need = (w * h * 4) as usize;
        if self.pool.len() < need {
            let _ = self.pool.resize(need);
        }
        let (buffer, canvas) = self
            .pool
            .create_buffer(w, h, w * 4, wl_shm::Format::Argb8888)
            .expect("création du buffer de réglages");
        self.model.draw(canvas, w, h);
        self.window.wl_surface().damage_buffer(0, 0, w, h);
        buffer
            .attach_to(self.window.wl_surface())
            .expect("attach du buffer de réglages");
        self.window.commit();
    }

    /// Relit le thème depuis le fichier (après écriture ou SIGHUP).
    fn reload(&mut self) {
        let (cfg, _) = yaor_config::DesktopConfig::load();
        self.model = Model::new(cfg.theme);
    }

    /// Écrit via la même chaîne que la CLI, valide, réveille le bureau.
    fn commit(&mut self, what: &str, f: impl FnOnce(&str) -> Result<String, String>) {
        let mut erreur: Option<String> = None;
        let res = edit::update_config(|ancien| match f(ancien) {
            Ok(nouveau) if yaor_config::DesktopConfig::parse_str(&nouveau).is_ok() => nouveau,
            Ok(_) => {
                erreur = Some("résultat illisible".to_string());
                ancien.to_string()
            }
            Err(e) => {
                erreur = Some(e);
                ancien.to_string()
            }
        });
        match (res, erreur) {
            (Ok((path, _)), None) => {
                let hups = hup::notify_compositor();
                eprintln!("{PROG}: [{:.3}] {what} (hup {hups}, {})", uptime(), path.display());
            }
            (Ok(_), Some(e)) => eprintln!("{PROG}: [{:.3}] {what} refusé : {e}", uptime()),
            (Err(e), _) => eprintln!("{PROG}: [{:.3}] écriture impossible : {e}", uptime()),
        }
        self.reload();
        self.draw();
    }

    fn on_press(&mut self, x: i32, y: i32) {
        let w = self.width as i32;
        match self.model.hit(w, x, y) {
            Hit::Preset(i) => {
                let name = presets::NAMES[i];
                self.commit(&format!("préset : {name}"), |t| {
                    presets::apply_to_text(t, name).ok_or_else(|| format!("préset inconnu : {name}"))
                });
            }
            Hit::Style(glass) => {
                let v = if glass { "glass" } else { "flat" };
                self.commit(&format!("style : {v}"), |t| presets::tune_text(t, &[("style", v)]));
            }
            Hit::Swatch(i) => {
                let c = ui::SWATCHES[i];
                // accent sombre dérivé : même teinte, 78 % de luminosité
                let dark = format!("#{:06X}", yaor_ui::dim(yaor_config::parse_color(c).unwrap_or(0xFFE8862E), 0.78) & 0x00FF_FFFF);
                self.commit(&format!("accent : {c}"), |t| {
                    presets::tune_text(t, &[("accent", c), ("accent_dark", &dark)])
                });
            }
            Hit::Slider(s) => {
                self.model.drag = Some(s);
                let f = self.model.fraction_at(w, x);
                self.model.set_fraction(s, f);
                self.draw();
            }
            Hit::None => {}
        }
    }

    fn on_motion(&mut self, x: i32) {
        if let Some(s) = self.model.drag {
            let f = self.model.fraction_at(self.width as i32, x);
            self.model.set_fraction(s, f);
            self.draw();
        }
    }

    fn on_release(&mut self) {
        if let Some(s) = self.model.drag.take() {
            let (key, val) = (s.key(), self.model.value_text(s));
            self.commit(&format!("réglage : {key}={val}"), |t| presets::tune_text(t, &[(key, &val)]));
        }
    }
}

// --------------------------------------------------------------------- sctk

impl CompositorHandler for Settings {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for Settings {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for Settings {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl PointerHandler for Settings {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            // Les événements des autres surfaces (SSD) ne nous concernent pas.
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            let (x, y) = (event.position.0 as i32, event.position.1 as i32);
            match event.kind {
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => self.on_press(x, y),
                PointerEventKind::Motion { .. } => self.on_motion(x),
                PointerEventKind::Release { button, .. } if button == BTN_LEFT => self.on_release(),
                _ => {}
            }
        }
    }
}

impl SeatHandler for Settings {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(self.seat_state.get_pointer(qh, &seat).expect("pointeur indisponible"));
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl WindowHandler for Settings {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.exit = true;
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window, configure: WindowConfigure, _: u32) {
        if let Some(w) = configure.new_size.0 {
            self.width = w.get().max(MIN_W as u32);
        }
        if let Some(h) = configure.new_size.1 {
            self.height = h.get().max(MIN_H as u32);
        }
        if self.first_configure {
            self.first_configure = false;
            eprintln!(
                "{PROG}: [{:.3}] configure {}x{} (app_id=yaor-settings)",
                uptime(),
                self.width,
                self.height
            );
        }
        self.draw();
    }
}

delegate_compositor!(Settings);
delegate_output!(Settings);
delegate_shm!(Settings);
delegate_seat!(Settings);
delegate_pointer!(Settings);
delegate_xdg_shell!(Settings);
delegate_xdg_window!(Settings);
delegate_registry!(Settings);

impl ProvidesRegistryState for Settings {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn uptime() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY : clock_gettime écrit dans `ts`, valide.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

fn main() -> std::process::ExitCode {
    eprintln!("{PROG}: [{:.3}] démarre {VERSION}", uptime());
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" => {
                eprintln!("{PROG} {VERSION}");
                return std::process::ExitCode::SUCCESS;
            }
            "--help" => {
                eprintln!("{PROG}: réglages du bureau YAOR (thème, présets, matière verre)");
                return std::process::ExitCode::SUCCESS;
            }
            _ => eprintln!("{PROG}: argument ignoré : {arg}"),
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
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let xdg = XdgShell::bind(&globals, &qh).expect("xdg_shell");
    let pool = SlotPool::new((PREF_W * PREF_H * 4) as usize, &shm).expect("pool");

    let surface = compositor.create_surface(&qh);
    let window = xdg.create_window(surface, WindowDecorations::RequestServer, &qh);
    window.set_app_id(PROG);
    window.set_title("Réglages");
    window.set_min_size(Some((MIN_W as u32, MIN_H as u32)));
    window.commit();

    let (cfg, src) = yaor_config::DesktopConfig::load();
    eprintln!(
        "{PROG}: [{:.3}] thème {} (style {}, source {})",
        uptime(),
        cfg.theme.name,
        cfg.theme.style,
        src.map(|p| p.display().to_string()).unwrap_or_else(|| "défauts".into())
    );

    let mut app = Settings {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        xdg,
        exit: false,
        first_configure: true,
        pool,
        width: PREF_W as u32,
        height: PREF_H as u32,
        window,
        pointer: None,
        model: Model::new(cfg.theme),
    };

    // SIGHUP (relayé par le compositor, ou envoyé par `yaor-ctl theme`) :
    // la fenêtre se restyle comme panel et dock.
    let hup_tube = hup::install();
    let _ = conn.flush();
    while !app.exit {
        if let Some(tube) = hup_tube {
            let wl_fd = conn.backend().poll_fd().as_raw_fd();
            if hup::wait(wl_fd, tube) {
                app.reload();
                app.draw();
                let _ = conn.flush();
            }
        }
        if event_queue.blocking_dispatch(&mut app).is_err() {
            break;
        }
        let _ = conn.flush();
    }
    eprintln!("{PROG}: [{:.3}] arrêt", uptime());
    std::process::ExitCode::SUCCESS
}
