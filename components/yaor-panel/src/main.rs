// yaor-panel — panneau du bureau YAOR (jalon v0.6 « Caméléon », étape 1).
//
// Premier client layer-shell du système : barre supérieure ancrée en haut
// (couche Top, zone exclusive de 24 px — le compositor soustrait la bande
// du tiling, cf. policy.rs). Bouton menu ouvrant un volet (couche Overlay,
// clavier exclusif pendant l'ouverture) peuplé des apps YAOR de démo
// (waytest, xwtest) — décision produit v0.6 : pas de scan .desktop.
//
// Modèle : examples/simple_layer.rs de smithay-client-toolkit 0.19.2
// (licence MIT © Smithay contributors). Rendu logiciel wl_shm (Argb8888),
// police bitmap 5x7 intégrée (capitales + chiffres — mêmes conventions
// que waytest : « un besoin = un fournisseur », pas de bibliothèque de
// texte). Journal : stderr préfixé « yaor-panel: » (jetons QA courts).
//
// Connexion : WAYLAND_DISPLAY sous XDG_RUNTIME_DIR (socket du compositor).

use std::process::Command;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

mod logo;

const PROG: &str = "yaor-panel";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Uptime invité (s) — horodatage des journaux, corrélable avec le
/// compositor et les sondages du harnais QA (diagnostic v0.6-étape 1).
fn uptime() -> f64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(0.0)
}

/// Hauteur de la barre (zone exclusive posée au compositor).
const BAR_H: u32 = 24;
/// Largeur du bouton menu (px logiques).
const BTN_W: u32 = 60;
/// Dimensions du volet menu : une entrée de 24 px par app + bordures.
const MENU_W: u32 = 140;
const MENU_ENTRY_H: u32 = 24;

/// Apps de démo du menu (décision produit v0.6 : apps YAOR internes,
/// pas de scan .desktop). (libellé affiché, binaire à lancer.)
const APPS: &[(&str, &str)] = &[("WAYTEST", "waytest"), ("XWTEST", "xwtest")];

// Couleurs Argb8888 (0xAARRGGBB — converties en octets BGR/A au
// rendu). Identité « ambre sur noir » d'après l'emblème du produit
// (docs/05-ui-ux/branding/yaor-os-amber-emblem.png).
const BAR_BG: u32 = 0xFF_10_10_14; // noir du panneau
const BTN_BG: u32 = 0xFF_E8_86_2E; // bouton menu (ambre)
const BTN_FG: u32 = 0xFF_10_10_14; // texte du bouton (noir)
const MENU_BG: u32 = 0xFF_10_10_14;
const BORDER: u32 = 0xFF_B9_6A_1F; // bordure du volet (ambre sombre)
const TEXT_FG: u32 = 0xFF_F2_E8_DC; // texte clair chaud

/// Position du bouton menu : le logo 24x24 occupe x=2..26, le bouton
/// le suit (QA : clic au centre (60,12)).
const BTN_X: u32 = 30;

// ---------------------------------------------------------------------------
// Police bitmap 5x7 (capitales, chiffres, « . ») — dessin littéral.
// ---------------------------------------------------------------------------

/// Glyphes 5x7 : 7 lignes de 5 colonnes, « X » = pixel allumé.
const GLYPHS: &[(char, [&str; 7])] = &[
    (
        '0',
        ["0XXX0", "0..X0", "X..X0", "X..X0", "X..X0", "X..X0", "0XXX0"],
    ),
    (
        '1',
        ["..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", "0XXXX"],
    ),
    (
        '2',
        ["0XXX0", "X...X", "....X", "...X.", "..X..", ".X...", "XXXXX"],
    ),
    (
        '3',
        ["XXXXX", "....X", "...X.", "..XX.", "....X", "....X", "XXXXX"],
    ),
    (
        '4',
        ["X...X", "X...X", "X...X", "XXXXX", "....X", "....X", "....X"],
    ),
    (
        '5',
        ["XXXXX", "X....", "XXXX.", "....X", "....X", "X...X", "0XXX0"],
    ),
    (
        '6',
        [".XXX.", "X....", "X....", "XXXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '7',
        ["XXXXX", "....X", "...X.", "..X..", ".X...", ".X...", ".X..."],
    ),
    (
        '8',
        [".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '9',
        [".XXX.", "X...X", "X...X", ".XXXX", "....X", "....X", ".XXX."],
    ),
    (
        'A',
        [".XXX.", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    ),
    (
        'E',
        ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX"],
    ),
    (
        'M',
        ["X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X"],
    ),
    (
        'N',
        ["X...X", "XX..X", "XX..X", "X.X.X", "X..XX", "X..XX", "X...X"],
    ),
    (
        'O',
        [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'R',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X"],
    ),
    (
        'S',
        [".XXXX", "X....", "X....", ".XXX.", "....X", "....X", "XXXX."],
    ),
    (
        'T',
        ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        'U',
        ["X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'W',
        ["X...X", "X...X", "X...X", "X.X.X", "X.X.X", "X.X.X", ".X.X."],
    ),
    (
        'X',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", ".X.X.", "X...X"],
    ),
    (
        'Y',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        '.',
        [".....", ".....", ".....", ".....", ".....", ".XX..", ".XX.."],
    ),
];

fn glyph(ch: char) -> Option<&'static [&'static str; 7]> {
    let up = ch.to_ascii_uppercase();
    GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, g)| g)
}
/// Largeur d'un texte rendu (6 px par glyphe : 5 + 1 d'espacement).
fn text_width(s: &str) -> i32 {
    (s.chars().count() as i32) * 6
}

// ---------------------------------------------------------------------------
// Rendu (canvas wl_shm, octets B,G,R,A — petit boutisme Argb8888).
// ---------------------------------------------------------------------------

fn put(canvas: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let i = ((y * w + x) * 4) as usize;
    if i + 3 < canvas.len() {
        canvas[i] = (color & 0xFF) as u8;
        canvas[i + 1] = ((color >> 8) & 0xFF) as u8;
        canvas[i + 2] = ((color >> 16) & 0xFF) as u8;
        canvas[i + 3] = ((color >> 24) & 0xFF) as u8;
    }
}

fn fill_rect(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    rw: i32,
    rh: i32,
    color: u32,
) {
    for y in y0..y0 + rh {
        for x in x0..x0 + rw {
            put(canvas, w, h, x, y, color);
        }
    }
}

/// Logo 24x24 (module généré `logo.rs`) : blit sans mélange — pixel
/// opaque (alpha > 0) écrit, pixel transparent sauté (le noir du
/// panneau transparaît).
fn draw_logo(canvas: &mut [u8], w: i32, h: i32, x0: i32, y0: i32) {
    for (row, slice) in logo::LOGO_24.chunks_exact(24 * 4).enumerate() {
        let y = y0 + row as i32;
        if y < 0 || y >= h {
            continue;
        }
        for col in 0..24usize {
            if slice[col * 4 + 3] == 0 {
                continue;
            }
            let x = x0 + col as i32;
            if x < 0 || x >= w {
                continue;
            }
            let off = (y as usize * w as usize + x as usize) * 4;
            canvas[off] = slice[col * 4];
            canvas[off + 1] = slice[col * 4 + 1];
            canvas[off + 2] = slice[col * 4 + 2];
            canvas[off + 3] = 0xFF;
        }
    }
}

fn draw_text(canvas: &mut [u8], w: i32, h: i32, text: &str, x0: i32, y0: i32, color: u32) {
    let mut x = x0;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c == 'X' {
                        put(canvas, w, h, x + rx as i32, y0 + ry as i32, color);
                    }
                }
            }
        }
        x += 6;
    }
}

// ---------------------------------------------------------------------------
// État du panneau
// ---------------------------------------------------------------------------

struct Panel {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,

    /// Barre supérieure (couche Top, zone exclusive BAR_H).
    bar: LayerSurface,
    /// Volet menu ouvert (couche Overlay) — créé à l'ouverture, détruit
    /// à la fermeture (la destruction des objets wayland démappe la
    /// couche côté compositor, cf. shell.rs layer_destroyed).
    menu: Option<LayerSurface>,

    pool: SlotPool,
    pool_capacity: usize,
    width: u32,
    height: u32,
    configured: bool,
    exit: bool,

    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
}

impl Panel {
    fn ensure_pool(&mut self, w: i32, h: i32) {
        let need = (w * h * 4) as usize;
        if self.pool_capacity < need {
            self.pool = SlotPool::new(need, &self.shm).expect("création du slot pool");
            self.pool_capacity = need;
        }
    }

    /// Rendu de la barre : logo + bouton menu (ambre) + version.
    fn draw_bar(&mut self, _qh: &QueueHandle<Self>) {
        let w = self.width;
        let h = self.height;
        if w == 0 || h == 0 {
            return;
        }
        self.ensure_pool(w as i32, h as i32);
        let stride = w as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de barre");
        {
            let (cw, ch) = (w as i32, h as i32);
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, BAR_BG);
            // Logo du produit (emblème ambre 24x24, fond transparent).
            draw_logo(&mut canvas, cw, ch, 2, 0);
            fill_rect(&mut canvas, cw, ch, BTN_X as i32, 4, BTN_W as i32, 16, BTN_BG);
            let label = "YAOR";
            let tx = BTN_X as i32 + (BTN_W as i32 - text_width(label)) / 2;
            draw_text(&mut canvas, cw, ch, label, tx, 8, BTN_FG);
            // Version du produit à droite (ADR-022).
            let vers = format!("YAOR {VERSION}");
            draw_text(&mut canvas, cw, ch, &vers, cw - text_width(&vers) - 6, 8, TEXT_FG);
        }
        self.bar
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(self.bar.wl_surface()).expect("attach barre");
        self.bar.commit();
    }

    /// Rendu du volet menu (fond, bordure, entrées d'apps).
    fn draw_menu(&mut self, layer: &LayerSurface) {
        let w = MENU_W;
        let h = MENU_ENTRY_H * APPS.len() as u32 + 8;
        self.ensure_pool(w as i32, h as i32);
        let stride = w as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de menu");
        {
            let (cw, ch) = (w as i32, h as i32);
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, MENU_BG);
            // bordure : 1 px
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, 1, BORDER);
            fill_rect(&mut canvas, cw, ch, 0, ch - 1, cw, 1, BORDER);
            fill_rect(&mut canvas, cw, ch, 0, 0, 1, ch, BORDER);
            fill_rect(&mut canvas, cw, ch, cw - 1, 0, 1, ch, BORDER);
            for (idx, (label, _)) in APPS.iter().enumerate() {
                let top = 4 + (idx as u32 * MENU_ENTRY_H) as i32;
                let ty = top + (MENU_ENTRY_H as i32 - 7) / 2;
                draw_text(&mut canvas, cw, ch, label, 8, ty, TEXT_FG);
            }
        }
        layer
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(layer.wl_surface()).expect("attach menu");
        layer.commit();
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            // Destruction des objets wayland : le compositor reçoit
            // layer_destroyed et démappe la couche (retour du clavier
            // aux fenêtres à la prochaine touche).
            drop(menu);
            eprintln!("{PROG}: [{:.3}] menu fermé", uptime());
        }
    }

    fn open_menu(&mut self, qh: &QueueHandle<Self>) {
        if self.menu.is_some() {
            return;
        }
        let surface = self.compositor.create_surface(qh);
        let layer = self
            .layer_shell
            .create_layer_surface(qh, surface, Layer::Overlay, Some("yaor-panel-menu"), None);
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        // Marge haute 0 : la zone exclusive de la barre (24 px) est
        // DÉJÀ soustraite par le layer map — Smithay empile marge ET
        // zone exclusive, un set_margin(BAR_H, …) décalait le volet
        // de 24 px (volet à y=48 au lieu de 24) et faussait le
        // hit-test des entrées (QA v0.6-étape 1).
        layer.set_margin(0, 0, 0, 4);
        layer.set_size(MENU_W, MENU_ENTRY_H * APPS.len() as u32 + 8);
        // Clavier exclusif pendant l'ouverture : les touches sont
        // routées au volet par le compositor (Escape referme — preuve
        // du routage clavier layer-shell de l'étape 1).
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        eprintln!("{PROG}: [{:.3}] menu ouvert", uptime());
        self.menu = Some(layer);
    }

    /// Lance une app de démo du menu (env hérité : WAYLAND_DISPLAY,
    /// DISPLAY — le compositor et Xwayland servent les clients).
    fn launch(&self, bin: &str) {
        eprintln!("{PROG}: [{:.3}] lancement {bin} demandé (fork…)", uptime());
        match Command::new(bin).spawn() {
            Ok(child) => eprintln!(
                "{PROG}: [{:.3}] lancement {bin} (pid {})",
                uptime(),
                child.id()
            ),
            Err(e) => eprintln!("{PROG}: [{:.3}] échec lancement {bin} : {e}", uptime()),
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers sctk
// ---------------------------------------------------------------------------

impl CompositorHandler for Panel {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Panel {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Panel {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        eprintln!("{PROG}: couche fermée par le compositor");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if layer.wl_surface() == self.bar.wl_surface() {
            // Barre : largeur attribuée par le compositor (ancrée
            // gauche+droite, largeur demandée 0 = pleine largeur).
            self.width = if configure.new_size.0 == 0 { 1280 } else { configure.new_size.0 };
            self.height = if configure.new_size.1 == 0 { BAR_H } else { configure.new_size.1 };
            eprintln!("{PROG}: [{:.3}] barre configurée {}x{}", uptime(), self.width, self.height);
            if !self.configured {
                self.configured = true;
            }
            self.draw_bar(qh);
        } else if self.menu.is_some() {
            // Volet menu : taille demandée honorée → premier rendu.
            let (mw, mh) = (configure.new_size.0, configure.new_size.1);
            eprintln!("{PROG}: [{:.3}] menu configuré {}x{}", uptime(), mw, mh);
            if let Some(menu) = self.menu.clone() {
                self.draw_menu(&menu);
            }
        }
    }
}

impl SeatHandler for Panel {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("clavier indisponible");
            self.keyboard = Some(keyboard);
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self.seat_state.get_pointer(qh, &seat).expect("pointeur indisponible");
            self.pointer = Some(pointer);
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
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Panel {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // Menu ouvert : Escape referme (le clavier nous est exclusif).
        if self.menu.is_some() && event.keysym == Keysym::Escape {
            self.close_menu();
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for Panel {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            let (x, y) = event.position;
            if event.surface == *self.bar.wl_surface() {
                // Bouton menu : (4,4)-(4+BTN_W,20).
                if let Press { button, .. } = event.kind {
                    if button == 0x110
                        && x >= BTN_X as f64
                        && x < (BTN_X + BTN_W) as f64
                        && y >= 4.0
                        && y < 20.0
                    {
                        if self.menu.is_some() {
                            self.close_menu();
                        } else {
                            self.open_menu(qh);
                        }
                    }
                }
            } else if let Some(menu) = self.menu.clone() {
                if event.surface == *menu.wl_surface() {
                    if let Press { button, .. } = event.kind {
                        if button == 0x110 {
                            // Entrée sous le clic (bandes de MENU_ENTRY_H).
                            let idx = ((y as u32).saturating_sub(4)) / MENU_ENTRY_H;
                            if let Some((_, bin)) = APPS.get(idx as usize) {
                                self.launch(bin);
                            }
                            self.close_menu();
                        }
                    }
                }
            }
        }
    }
}

impl ShmHandler for Panel {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(Panel);
delegate_output!(Panel);
delegate_shm!(Panel);
delegate_seat!(Panel);
delegate_keyboard!(Panel);
delegate_pointer!(Panel);
delegate_layer!(Panel);
delegate_registry!(Panel);

impl ProvidesRegistryState for Panel {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

// ---------------------------------------------------------------------------
// Démarrage
// ---------------------------------------------------------------------------

fn usage(out: &mut dyn std::io::Write) {
    let _ = writeln!(out, "Usage: {PROG} [--help] [--version]");
    let _ = writeln!(
        out,
        "Panneau du bureau YAOR (barre layer-shell + menu d'apps de démo)"
    );
    let _ = writeln!(out, "  --help      affiche cette aide");
    let _ = writeln!(out, "  --version   affiche la version du produit");
}

fn main() -> std::process::ExitCode {
    // Arguments : pas de connexion Wayland pour --help/--version (les
    // contrôles de build interrogent le binaire sans session).
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {}
        Some("--help" | "-h") => {
            usage(&mut std::io::stdout());
            return std::process::ExitCode::SUCCESS;
        }
        Some("--version" | "-V") => {
            println!("{PROG} {VERSION}");
            return std::process::ExitCode::SUCCESS;
        }
        Some(other) => {
            usage(&mut std::io::stderr());
            eprintln!("{PROG}: argument inconnu : {other}");
            return std::process::ExitCode::FAILURE;
        }
    }

    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("{PROG}: connexion au compositor impossible : {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let (globals, mut event_queue) = registry_queue_init(&conn).expect("registre wayland");
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor indisponible");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer-shell indisponible");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm indisponible");

    // Barre : couche Top, ancrée haut + pleine largeur, zone
    // exclusive BAR_H (le tiling du compositor se retire de la bande).
    let bar_surface = compositor.create_surface(&qh);
    let bar = layer_shell.create_layer_surface(&qh, bar_surface, Layer::Top, Some("yaor-panel"), None);
    bar.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    bar.set_size(0, BAR_H);
    bar.set_exclusive_zone(BAR_H as i32);
    bar.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    // Commit initial sans buffer : déclenche la configure du compositor,
    // qui pilote le premier rendu (modèle simple_layer).
    bar.commit();

    // Pool pré-dimensionné pour 2 slots de barre + 1 slot de menu :
    // la croissance d'un SlotPool (set_len + remap mmap + requête
    // wl_shm_pool.resize) a provoqué un décrochage du panel au premier
    // rendu du menu (QA v0.6-étape 1) — on la rend impossible.
    let bar_need = 1280 * BAR_H as usize * 4;
    let menu_need = MENU_W as usize * (MENU_ENTRY_H * APPS.len() as u32 + 8) as usize * 4;
    let pool_capacity = 2 * bar_need + menu_need;
    let pool = SlotPool::new(pool_capacity, &shm).expect("création du slot pool");

    let mut panel = Panel {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        layer_shell,
        bar,
        menu: None,
        pool,
        pool_capacity,
        width: 0,
        height: BAR_H,
        configured: false,
        exit: false,
        keyboard: None,
        pointer: None,
    };
    eprintln!("{PROG}: [{:.3}] démarré (version {VERSION})", uptime());

    while !panel.exit {
        if event_queue.blocking_dispatch(&mut panel).is_err() {
            break;
        }
    }
    eprintln!("{PROG}: [{:.3}] arrêt", uptime());
    std::process::ExitCode::SUCCESS
}
