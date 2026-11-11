// yaor-term — terminal du bureau YAOR (v0.6 étape 7).
//
// Client xdg-shell régulier : fenêtre flottante décorée par le SSD du
// compositor (feux tricolores + titre, étape 4), rendu wl_shm au
// rastérisateur yaor-ui (police réelle, étape 1). Un PTY vers /bin/bash
// (forkpty, libc) alimente un tampon de lignes ; les séquences CSI/OSC
// sont CONSOMMÉES (rendu ligne à ligne — un émulateur VT complet reste
// hors périmètre v0.6). Le clavier est décodé par sctk (xkbcommon, keymap
// du compositor) et écrit dans le PTY : lettres/frappe utf-8, Entrée,
// Retour arrière, Tab, flèches (séquences ESC), maison/fin, Suppr,
// Ctrl+lettre.
//
// Modèle : yaor-panel (sctk) et examples/simple_window.rs de sctk
// 0.19.2. Rendu logiciel Argb8888. Ack des configure géré par sctk
// (inner.rs : xdg_surface.ack_configure à la réception).
//
// Journal : stderr préfixé « yaor-term: » (jetons QA courts, calculés
// par le harnais côté invité — leçon t-yaor-dialog).
use std::collections::VecDeque;
use std::os::fd::{AsRawFd, RawFd};
use std::process::Command;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
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
    protocol::{wl_keyboard, wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use yaor_ui::Theme;

const PROG: &str = "yaor-term";
const VERSION: &str = env!("CARGO_PKG_VERSION");

const WIN_MIN: (u32, u32) = (320, 240);
const MARGIN: i32 = 10;
const TEXT_SIZE: u32 = 13;
/// Pitch de ligne (px) : hauteur de police + interligne.
const LINE_PITCH: i32 = 18;

// ---------------------------------------------------------------------
// Tampon terminal (Vt)
// ---------------------------------------------------------------------

/// Lignes du PTY, bornées (ring). CR/redémarre la ligne, LF publie,
/// Backspace efface, Tab aligne, CSI/OSC consommés. UTF-8 incrémental :
/// les lignes sont tenues en octets et décodées `lossy` au rendu —
/// aucune perte aux frontières multi-octets (contrairement à un simple
/// `b as char`).
struct Vt {
    lines: VecDeque<Vec<u8>>,
    cur: Vec<u8>,
    /// État d'échappement : 0 = normal, 1 = ESC vu, 2 = CSI (params,
    /// fin au final 0x40..=0x7E), 3 = OSC (fin au BEL 0x07).
    esc: u8,
    /// CR vu (retour chariot) : réglé au LF suivant (le contenu reste),
    /// effacé si un octet de contenu arrive avant (barre de progression
    /// « \r » qui réécrit la ligne).
    cr: bool,
    max: usize,
    #[allow(dead_code)]
    bytes: usize,
}

impl Vt {
    fn new(max: usize) -> Self {
        Vt { lines: VecDeque::new(), cur: Vec::new(), esc: 0, cr: false, max, bytes: 0 }
    }

    fn feed(&mut self, data: &[u8]) {
        self.bytes += data.len();
        for &b in data {
            match self.esc {
                0 => self.plain(b),
                1 => self.esc = if b == b'[' { 2 } else if b == b']' { 3 } else { 0 },
                2 => {
                    if (0x40..=0x7E).contains(&b) {
                        // final CSI (m, H, J, K…) — consommé, pas rendu
                        self.esc = 0;
                    }
                }
                _ => {
                    if b == 0x07 {
                        self.esc = 0;
                    } else if b == 0x1B {
                        self.esc = 1;
                    }
                }
            }
        }
    }

    fn plain(&mut self, b: u8) {
        // CR différé : un CR suivi d'un LF publie la ligne sans la
        // perdre ; un CR suivi d'un contenu efface d'abord la ligne
        // (barre de progression « \r » qui réécrit).
        self.plain_byte(b);
    }

    fn plain_byte(&mut self, b: u8) {
        if self.cr && b != b'\n' {
            self.cr = false;
            self.cur.clear();
        }
        match b {
            0x1B => self.esc = 1,
            0x0D => {
                if self.cr {
                    // CR CR : le premier a déjà purgé, repart de zéro.
                    self.cur.clear();
                }
                self.cr = true;
            }
            0x07 | 0x08 | 0x7F => {
                self.cur.pop();
            }
            b'\n' => {
                self.push_line();
            }
            b'\t' => {
                const TAB: usize = 4;
                let cols = String::from_utf8_lossy(&self.cur);
                let pad = TAB - cols.chars().count() % TAB;
                self.cur.extend(std::iter::repeat(b' ').take(pad));
            }
            0x00..=0x1F => {} // autres contrôles : ignorés
            _ => self.cur.push(b),
        }
    }

    fn push_line(&mut self) {
        self.lines.push_back(std::mem::take(&mut self.cur));
        while self.lines.len() >= self.max {
            self.lines.pop_front();
        }
    }

    /// Lignes affichables (octets), ligne courante incluse (si non
    /// vide : un cur vide après un LF final ne doit pas tracer).
    fn snapshot(&self) -> (Vec<Vec<u8>>, usize) {
        let mut v: Vec<Vec<u8>> = self.lines.iter().cloned().collect();
        if !self.cur.is_empty() {
            v.push(self.cur.clone());
        }
        (v, self.cur.len())
    }
}

// ---------------------------------------------------------------------
// État du client
// ---------------------------------------------------------------------

struct Term {
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
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: bool,
    modi: Modifiers,
    pty: Option<RawFd>,
    vt: Vt,
    logged_output: bool,
}

impl Term {
    /// Rendu : fond plein + dernières lignes du tampon + curseur bloc.
    fn draw(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>) {
        let (w, h) = (self.width as i32, self.height as i32);
        if w <= 0 || h <= 0 {
            return;
        }
        let stride = w * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w, h, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer du terminal");

        canvas.fill(0);
        yaor_ui::fill_rect(&mut canvas, w, h, 0, 0, w, h, self.theme.panel_bg);
        // Liseré d'accent fin sur le bord gauche (patte visuelle YAOR).
        for x in 0..2 {
            for y in 0..h {
                yaor_ui::put(&mut canvas, w, h, x, y, self.theme.accent_dark);
            }
        }

        // Lignes visibles : les DERNIÈRES, collées en bas (comme une
        // console qui défile).
        let pitch = LINE_PITCH;
        let shown = (h - MARGIN) / pitch;
        let (lines, cur_len) = self.vt.snapshot();
        let skip = lines.len() as i32 - shown;
        let mut y = MARGIN + shown * pitch - (lines.len() as i32) * pitch;
        for (i, line) in lines.iter().enumerate() {
            if (i as i32) < skip.max(0) {
                continue;
            }
            let text = String::from_utf8_lossy(line);
            yaor_ui::text::draw_text(
                &mut canvas,
                w,
                h,
                &text,
                MARGIN,
                y,
                TEXT_SIZE,
                yaor_ui::text::Weight::Regular,
                self.theme.text_fg,
            );
            // Curseur bloc sous la fin de la dernière ligne rendue
            // (y compris ligne courante vide).
            let is_last = i == lines.len() - 1;
            if is_last {
                let cwid = yaor_ui::text::text_width(
                    &text,
                    TEXT_SIZE,
                    yaor_ui::text::Weight::Regular,
                );
                let cx = MARGIN + cwid + 2 + cur_len.min(1) as i32 * 0;
                let cy = y.max(MARGIN);
                for dy in 0..(TEXT_SIZE as i32 + 2) {
                    for dx in 0..7 {
                        yaor_ui::blend(
                            &mut canvas,
                            w,
                            h,
                            cx.min(w - 10) + dx,
                            cy + dy,
                            self.theme.accent,
                            0.65,
                        );
                    }
                }
            }
            y += pitch;
        }

        self.window
            .wl_surface()
            .damage_buffer(0, 0, w, h);
        buffer
            .attach_to(self.window.wl_surface())
            .expect("attach du buffer du terminal");
        self.window.commit();
        let _ = (_conn, _qh);
    }

    /// Lecture PTY : Some(true)=`n` octets, None=EAGAIN, EOF → arrêt.
    fn read_pty(&mut self) -> Option<bool> {
        let fd = self.pty?;
        let mut buf = [0u8; 4096];
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len()) };
        match n {
            0 => None,
            n if n < 0 => {
                let err = std::io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::EAGAIN) | Some(libc::EINTR) => Some(false),
                    _ => None,
                }
            }
            n => {
                self.vt.feed(&buf[..n as usize]);
                if !self.logged_output {
                    self.logged_output = true;
                    eprintln!("{PROG}: [{:.3}] entree : {} octets", uptime(), self.vt.bytes);
                }
                Some(true)
            }
        }
    }

    fn write_pty(&mut self, data: &[u8]) {
        let Some(fd) = self.pty else { return };
        let mut off = 0;
        while off < data.len() {
            let n = unsafe {
                libc::write(fd, data[off..].as_ptr().cast::<libc::c_void>(), data.len() - off)
            };
            if n <= 0 {
                break;
            }
            off += n as usize;
        }
    }
}

fn uptime() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

impl OutputHandler for Term {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

// ---------------------------------------------------------------------
// Handlers sctk
// ---------------------------------------------------------------------

impl CompositorHandler for Term {
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
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if surface == self.window.wl_surface() {
            self.draw(conn, qh);
        }
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

impl ShmHandler for Term {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl KeyboardHandler for Term {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        if surface == self.window.wl_surface() {
            self.keyboard_focus = true;
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if surface == self.window.wl_surface() {
            self.keyboard_focus = false;
        }
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _layout: u32,
    ) {
        self.modi = modifiers;
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // Contrôles d'abord (Entrée/Retour arrière/Tab/flèches…).
        let bytes: Option<Vec<u8>> = match event.keysym {
            Keysym::Return => Some(b"\r".to_vec()),
            Keysym::BackSpace => Some(b"\x7f".to_vec()),
            Keysym::Tab => Some(b"\t".to_vec()),
            Keysym::Up => Some(b"\x1b[A".to_vec()),
            Keysym::Down => Some(b"\x1b[B".to_vec()),
            Keysym::Right => Some(b"\x1b[C".to_vec()),
            Keysym::Left => Some(b"\x1b[D".to_vec()),
            Keysym::Home => Some(b"\x1b[H".to_vec()),
            Keysym::End => Some(b"\x1b[F".to_vec()),
            Keysym::Delete => Some(b"\x1b[3~".to_vec()),
            _ => None,
        };
        if let Some(b) = bytes {
            self.write_pty(&b);
            return;
        }
        if self.modi.ctrl {
            // Ctrl+lettre : octet de contrôle (Ctrl+C = 0x03, Ctrl+D…).
            if let Some(ch) = event.utf8.as_ref().and_then(|s| s.chars().next()) {
                let c = ch.to_ascii_lowercase();
                if c.is_ascii_alphabetic() {
                    self.write_pty(&[(c as u8) - b'a' + 1]);
                    return;
                }
            }
            return;
        }
        if let Some(text) = event.utf8.as_ref() {
            if !text.is_empty() {
                self.write_pty(text.as_bytes());
            }
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
}

impl SeatHandler for Term {
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
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(
                self.seat_state
                    .get_keyboard(qh, &seat, None)
                    .expect("clavier indisponible"),
            );
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            self.keyboard.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl WindowHandler for Term {
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
        // Ack automatiquement posé par sctk (inner.rs). Taille CLIENTE
        // suggérée : corps de la fenêtre (SSD décompté par le
        // compositor). Zero = libre → on garde la taille précédente.
        if let Some(w) = configure.new_size.0 {
            self.width = w.get();
        }
        if let Some(h) = configure.new_size.1 {
            self.height = h.get();
        }
        if self.first_configure {
            self.first_configure = false;
            eprintln!(
                "{PROG}: [{:.3}] configure {w}x{h} (app_id=yaor-term)",
                uptime(),
                w = self.width,
                h = self.height
            );
            self.draw(conn, qh);
        }
    }
}

delegate_compositor!(Term);
delegate_output!(Term);
delegate_shm!(Term);
delegate_seat!(Term);
delegate_keyboard!(Term);
delegate_xdg_shell!(Term);
delegate_xdg_window!(Term);
delegate_registry!(Term);

impl ProvidesRegistryState for Term {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

// ---------------------------------------------------------------------
// PTY + boucle principale
// ---------------------------------------------------------------------

/// forkpty + exec du shell interactif. Retourne (pid enfant, fd PTY).
fn spawn_shell() -> (libc::pid_t, RawFd) {
    let mut master: libc::c_int = -1;
    let pid = unsafe {
        libc::forkpty(&mut master, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
    };
    if pid < 0 {
        panic!("forkpty : std::io::Error::last_os_error()");
    }
    if pid == 0 {
        // Enfant : la session a déjà les tty/sid de forkpty ; le shell
        // interactif hérite fauteuil + stdin/stdout/stderr du PTY. Les
        // fds Wayland sont CLOEXEC (fermés par exec).
        let term = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        // TERM nécessaire pour bash et les outils dans le PTY.
        std::env::set_var("TERM", "xterm-256color");
        let status = Command::new(term)
            .arg("-i")
            .status()
            .or_else(|_| Command::new("/bin/sh").arg("-i").status());
        if status.is_err() {
            eprintln!("{PROG}: exec shell impossible");
        }
        std::process::exit(1);
    }
    // Parent : fd maître, non bloquant (poll le réveille, EAGAIN lueur
    // en lecture si rien).
    unsafe {
        let flags = libc::fcntl(master, libc::F_GETFL, 0);
        libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    (pid, master as RawFd)
}

/// poll(2) avec réessai EINTR (comme yaor-session, appels directs).
fn poll_all(fds: &mut [libc::pollfd], timeout: libc::c_int) -> libc::c_int {
    loop {
        let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
        if n < 0 {
            let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            if err == libc::EINTR {
                continue;
            }
            return -1;
        }
        return n;
    }
}

fn main() -> std::process::ExitCode {
    eprintln!("{PROG}: [{:.3}] demarre {} (police reelle, PTY)", uptime(), VERSION);

    // Options sans connexion Wayland (vérification de distribution).
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" => eprintln!("{PROG} {VERSION}"),
            "--help" => eprintln!("{PROG}: terminal du bureau YAOR (PTY bash + police reelle)"),
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
    window.set_title("Terminal");
    window.set_min_size(Some(WIN_MIN));
    window.commit();

    let term = Term {
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
        keyboard: None,
        keyboard_focus: false,
        modi: Modifiers::default(),
        pty: None,
        vt: Vt::new(400),
        logged_output: false,
    };
    let mut term = term;

    // PTY : forkpty + bash interactif. Le parent garde le fd seul (l'enfant
    // exec bash : le fd wayland CLOEXEC disparaît, aucun héritage).
    let (shell_pid, pty_fd) = spawn_shell();
    term.pty = Some(pty_fd);
    eprintln!("{PROG}: [{:.3}] shell pid {shell_pid}", uptime());

    // Boucle : poll(2) sur les deux fd (wayland + PTY) puis pend. Pas de
    // calloop (dépendance de moins) : pattern manuel de wayland-client —
    // prepare_read avant poll, read_events si réveil wayland,
    // dispatch_pending toujours, flush en fin d'itération (leçon du
    // panel) et de wayland-client (read_events sans dispatch = perte).
    let wl_fd = conn.backend().poll_fd().as_raw_fd();
    let _ = conn.flush();
    while !term.exit {
        let read = event_queue.prepare_read();
        let mut fds = [
            libc::pollfd { fd: wl_fd, events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: pty_fd, events: libc::POLLIN, revents: 0 },
        ];
        poll_all(&mut fds, -1);

        if (fds[1].revents & (libc::POLLIN | libc::POLLHUP)) != 0 {
            let n = match term.read_pty() {
                Some(true) => 1,
                Some(false) => 0,
                None => {
                    // EOF : shell parti → fermer (planque, pas console).
                    eprintln!("{PROG}: [{:.3}] shell sortie", uptime());
                    unsafe { libc::waitpid(shell_pid, std::ptr::null_mut(), 0) };
                    break;
                }
            };
            if n > 0 {
                term.draw(&conn, &qh);
            }
        }
        if (fds[0].revents & libc::POLLIN) != 0 {
            if let Some(read) = read {
                match read.read() {
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        }
        if event_queue.dispatch_pending(&mut term).is_err() {
            break;
        }
        let _ = conn.flush();
    }

    eprintln!("{PROG}: [{:.3}] arret", uptime());
    if term.pty.is_some() {
        unsafe { libc::close(pty_fd) };
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::Vt;

    #[test]
    fn crlf_deux_sorties_une_ligne_chaque() {
        // "a\r\nb\r\n" → 2 lignes terminées.
        let mut vt = Vt::new(100);
        vt.feed(b"a\r\nb\r\n");
        let (lines, _) = vt.snapshot();
        let texts: Vec<String> = lines.iter().map(|l| String::from_utf8_lossy(l).into()).collect();
        assert_eq!(texts, vec!["a".to_string(), "b".to_string() + ""]);
    }

    #[test]
    fn csi_osc_consommes() {
        let mut vt = Vt::new(100);
        vt.feed(b"\x1b[1;32mOK\x1b[0m \x1b]0;titre\x07x");
        let (lines, _) = vt.snapshot();
        let text = String::from_utf8_lossy(&lines[0]);
        assert_eq!(text, "OK x");
    }

    #[test]
    fn utf8_frontiere_compteur_aucun_octet_perdu() {
        // Caractère multi-octets coupé par un push individuel : le Vt ne
        // perd RIEN (octets tenus bruts, décodés au rendu).
        let mut vt = Vt::new(100);
        for b in "é".as_bytes() {
            vt.plain(*b);
        }
        let (lines, _) = vt.snapshot();
        let text = String::from_utf8_lossy(&lines[0]);
        assert_eq!(text, "é");
    }
}
