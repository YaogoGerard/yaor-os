// yaor-dock — dock du profil macOS-like (jalon v0.6 « Caméléon », étape 2).
//
// Second client layer-shell du système, après le panel : barre flottante
// ancrée en bas et **centrée** (ancre BOTTOM seule, largeur calculée), sans
// zone exclusive — les fenêtres gardent tout l'écran, comme le dock de
// macOS (le profil classique, lui, garde sa taskbar pleine largeur ancrée
// bas avec zone exclusive).
//
// Contenu : une tuile d'emblème (lanceur, image `logo.path`) puis une tuile
// par fenêtre publiée par le compositor (protocole wlr-foreign-toplevel-
// management, même canal que la taskbar : titre, app_id, focus, minimisée).
// Clic sur une tuile = restauration si minimisée puis activation ; un point
// ambre sous la tuile marque la fenêtre focusée. Rendu logiciel wl_shm
// Argb8888 **prémultiplié** : la barre est arrondie sur fond transparent
// (yaor-ui, étape 2).
//
// Modèle sctk : examples/simple_layer.rs (licence MIT © Smithay
// contributors). Journal : stderr préfixé « yaor-dock: » (jetons QA courts).
// Connexion : WAYLAND_DISPLAY sous XDG_RUNTIME_DIR (socket du compositor).

use std::os::fd::AsRawFd;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
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
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{Event as ToplevelEvent, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{Event as ManagerEvent, ZwlrForeignToplevelManagerV1},
};
use yaor_ui::{
    dim, draw_text_scaled, fill_circle, fill_rrect, fill_rrect_premul, glyph, Theme,
};

const PROG: &str = "yaor-dock";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Uptime invité (s) — horodatage corrélable avec le compositor et les
/// sondes du harnais QA (même convention que le panel).
fn uptime() -> f64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(0.0)
}

/// Namespace layer-shell du dock (journaux du compositor).
const NAMESPACE: &str = "yaor-dock";

// ---------------------------------------------------------------------------
// Géométrie (px logiques) — calculs purs, testés sans session Wayland
// ---------------------------------------------------------------------------

/// Géométrie du dock : hauteur de barre, côté des tuiles, pas et marges.
/// Les valeurs de `desktop.toml` sont **bornées** : une configuration
/// aberrante (hauteur 0, tuile plus grande que la barre) ne casse pas le
/// bureau, elle retombe dans un intervalle utilisable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Geometry {
    height: u32,
    icon: u32,
    gap: u32,
    pad: u32,
    margin_bottom: u32,
}

impl Geometry {
    /// Bornes de la hauteur de barre (px).
    const MIN_H: u32 = 40;
    const MAX_H: u32 = 200;
    /// Côté minimal d'une tuile (px) — en dessous, le glyphe est illisible.
    const MIN_ICON: u32 = 24;
    /// Espace entre deux tuiles (px).
    const GAP: u32 = 10;
    /// Marge intérieure de la barre, à gauche et à droite (px).
    const PAD: u32 = 10;
    /// Distance entre la barre et le bas de l'écran (px).
    const MARGIN_BOTTOM: u32 = 10;
    /// Rayon des coins de la barre (px).
    const BAR_RADIUS: i32 = 20;
    /// Rayon des coins d'une tuile (px).
    const TILE_RADIUS: i32 = 13;

    fn from_config(cfg: &yaor_config::DockConfig) -> Geometry {
        let height = cfg.height.clamp(Self::MIN_H, Self::MAX_H);
        // 16 px de marge verticale dans la barre (8 en haut, 8 en bas).
        let icon = cfg.icon.clamp(Self::MIN_ICON, height - 16);
        Geometry {
            height,
            icon,
            gap: Self::GAP,
            pad: Self::PAD,
            margin_bottom: Self::MARGIN_BOTTOM,
        }
    }

    /// Nombre de tuiles : l'emblème (lanceur) + une par fenêtre.
    fn tiles(windows: usize) -> usize {
        windows + 1
    }

    /// Largeur de la barre pour `windows` fenêtres publiées.
    fn width(&self, windows: usize) -> u32 {
        let n = Self::tiles(windows) as u32;
        self.pad * 2 + n * self.icon + n.saturating_sub(1) * self.gap
    }

    /// Abscisse de la tuile d'indice `i` (0 = emblème).
    fn tile_x(&self, i: usize) -> i32 {
        (self.pad + i as u32 * (self.icon + self.gap)) as i32
    }

    /// Ordonnée d'une tuile (centrée verticalement dans la barre).
    fn tile_y(&self) -> i32 {
        ((self.height - self.icon) / 2) as i32
    }

    /// Tuile sous le pointeur (`x` en px logiques), hors des tuiles : `None`.
    fn hit(&self, windows: usize, x: f64) -> Option<usize> {
        let x = x as i32;
        if x < 0 {
            return None;
        }
        (0..Self::tiles(windows)).find(|i| {
            let x0 = self.tile_x(*i);
            x >= x0 && x < x0 + self.icon as i32
        })
    }

    /// Échelle du glyphe dans une tuile (police 5x7) : 1 à 3.
    fn glyph_scale(&self) -> i32 {
        (((self.icon as i32) - 14) / 7).clamp(1, 3)
    }
}

/// Initiale affichée dans une tuile de fenêtre : premier caractère
/// alphanumérique de l'app_id, sinon du titre, sinon « . » (la police
/// bitmap n'a pas de glyphe vide).
fn initial(app_id: &str, title: &str) -> char {
    let pick = |s: &str| {
        s.chars()
            .find(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
    };
    let ch = pick(app_id).or_else(|| pick(title)).unwrap_or('.');
    if glyph(ch).is_some() {
        ch
    } else {
        '.'
    }
}

/// Dock visible pour un profil donné : il n'appartient qu'au profil
/// macOS-like (étape 3). Profil absent = comportement de l'étape 2 : le
/// dock lancé s'affiche, quel que soit `[layout] mode`.
fn dock_visible(profile: Option<yaor_config::Profile>) -> bool {
    profile.map_or(true, |p| p.wants_dock())
}

// ---------------------------------------------------------------------------
// État du dock
// ---------------------------------------------------------------------------

struct Dock {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,

    /// Barre du dock (couche Top, ancrée en bas, centrée).
    layer: LayerSurface,
    geom: Geometry,
    /// Taille actuelle de la surface (fixée par le configure du compositor).
    width: u32,
    height: u32,
    configured: bool,

    /// Palette `[theme]` et emblème `logo.path` (assets chargés une fois).
    theme: Theme,
    emblem: Option<Vec<u8>>,
    emblem_px: u32,

    /// Fenêtres publiées par le compositor (ordre d'arrivée).
    entries: Vec<Entry>,
    foreign_manager: Option<ZwlrForeignToplevelManagerV1>,
    /// Ressource wl_seat (l'activation foreign-toplevel l'exige).
    seat_object: Option<wl_seat::WlSeat>,

    /// Tuile sous le pointeur (mise en évidence, redessin sur changement).
    hovered: Option<usize>,

    /// Dock affiché ? (étape 3 : présent dans le seul profil macOS-like —
    /// masqué il est réduit à une surface 1x1 transparente).
    visible: bool,

    pool: SlotPool,
    pool_capacity: usize,
    pointer: Option<wl_pointer::WlPointer>,
    exit: bool,
}

/// Une fenêtre du dock : handle foreign-toplevel + état affiché.
struct Entry {
    handle: ZwlrForeignToplevelHandleV1,
    title: String,
    app_id: String,
    activated: bool,
    minimized: bool,
}

impl Dock {
    fn ensure_pool(&mut self, w: i32, h: i32) {
        let need = (w * h * 4) as usize;
        if self.pool_capacity < need {
            self.pool = SlotPool::new(need, &self.shm).expect("création du slot pool");
            self.pool_capacity = need;
        }
    }

    /// Rendu de la barre : fond arrondi prémultiplié (surface transparente
    /// hors de la barre), tuiles emblème + fenêtres, point de focus.
    /// Journal : jeton « dock : rendu ».
    fn draw(&mut self) {
        let w = self.width as i32;
        let h = self.height as i32;
        if w <= 0 || h <= 0 {
            return;
        }
        self.ensure_pool(w, h);
        let stride = w * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w, h, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer du dock");
        if !self.visible {
            // Profil non macOS-like (étape 3) : surface réduite à 1x1 et
            // buffer transparent — rien à l'écran, aucune zone réservée.
            canvas.fill(0);
            self.layer
                .wl_surface()
                .damage_buffer(0, 0, w, h);
            buffer
                .attach_to(self.layer.wl_surface())
                .expect("attach dock");
            self.layer.commit();
            // Jeton QA : le masquage n'a PAS de ligne « rendu » ci-dessous
            // (return avant le log) — sans ce jeton, un harnais qui greffe
            // dessus le configure(1,1) croit à un configure perdu
            // (leçon sondages 1/2/2b : tout se passe nominalement).
            eprintln!(
                "{PROG}: [{:.3}] masqué : tampon {w}x{} transparent attaché",
                uptime(),
                h
            );
            return;
        }
        {
            // Fond transparent : wl_shm ne garantit pas un buffer vierge
            // (slots réutilisés) — on efface avant de peindre la barre.
            canvas.fill(0);
            fill_rrect_premul(
                &mut canvas,
                w,
                h,
                0,
                0,
                w,
                h,
                Geometry::BAR_RADIUS,
                self.theme.panel_bg,
            );

            let tile = self.geom.icon as i32;
            let ty = self.geom.tile_y();
            for i in 0..Geometry::tiles(self.entries.len()) {
                let tx = self.geom.tile_x(i);
                let active = i > 0
                    && self
                        .entries
                        .get(i - 1)
                        .map(|e| e.activated)
                        .unwrap_or(false);
                let hovered = self.hovered == Some(i);
                let border = if active {
                    self.theme.accent
                } else if hovered {
                    dim(self.theme.accent, 0.7)
                } else {
                    self.theme.rest_border()
                };
                // Tuile : liseré puis fond intérieur (même construction que
                // les pilules de la taskbar du profil classique).
                fill_rrect(
                    &mut canvas,
                    w,
                    h,
                    tx,
                    ty,
                    tile,
                    tile,
                    Geometry::TILE_RADIUS,
                    border,
                );
                fill_rrect(
                    &mut canvas,
                    w,
                    h,
                    tx + 2,
                    ty + 2,
                    tile - 4,
                    tile - 4,
                    Geometry::TILE_RADIUS - 2,
                    if hovered {
                        self.theme.accent_dark
                    } else {
                        self.theme.button_bg
                    },
                );

                if i == 0 {
                    // Lanceur : emblème du produit (sinon disque ambre).
                    match self.emblem.as_ref() {
                        Some(px) => {
                            let side = self.emblem_px as i32;
                            yaor_ui::blit(
                                &mut canvas,
                                w,
                                h,
                                tx + (tile - side) / 2,
                                ty + (tile - side) / 2,
                                px,
                                side,
                                side,
                            );
                        }
                        None => {
                            let cx = tx + tile / 2;
                            let cy = ty + tile / 2;
                            fill_circle(&mut canvas, w, h, cx, cy, tile / 4, self.theme.accent);
                            let sc = self.geom.glyph_scale();
                            draw_text_scaled(
                                &mut canvas,
                                w,
                                h,
                                "Y",
                                cx - 5 * sc / 2,
                                cy - 7 * sc / 2,
                                self.theme.panel_bg,
                                sc,
                            );
                        }
                    }
                    continue;
                }

                let Some(entry) = self.entries.get(i - 1) else {
                    continue;
                };
                // Glyphe de la fenêtre : initiale de l'app_id, colorée
                // ambre si la fenêtre a le focus.
                let sc = self.geom.glyph_scale();
                let ch = initial(&entry.app_id, &entry.title);
                draw_text_scaled(
                    &mut canvas,
                    w,
                    h,
                    &ch.to_string(),
                    tx + (tile - 5 * sc) / 2,
                    ty + (tile - 7 * sc) / 2,
                    if active {
                        self.theme.accent
                    } else {
                        self.theme.text_fg
                    },
                    sc,
                );
                // Point d'état : ambre = focus, gris = ouverte, absent si
                // minimisée (la tuile est alors plus sombre).
                if !entry.minimized {
                    let color = if active {
                        self.theme.accent
                    } else {
                        self.theme.rest_border()
                    };
                    fill_circle(&mut canvas, w, h, tx + tile / 2, h - 6, 2, color);
                }
            }
        }
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, w, h);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach dock");
        self.layer.commit();
        let focus = self
            .entries
            .iter()
            .position(|e| e.activated)
            .map(|i| (i + 1).to_string())
            .unwrap_or_else(|| "-".into());
        eprintln!(
            "{PROG}: [{:.3}] dock : rendu {} tuile(s) {}x{} focus={}",
            uptime(),
            Geometry::tiles(self.entries.len()),
            w,
            h,
            focus
        );
    }

    /// Recalcule la largeur de la barre après un changement de la liste des
    /// fenêtres : `set_size` + commit déclenchent l'arrange du compositor,
    /// qui renvoie un configure — le redessin part de là (jamais de buffer
    /// attaché à une taille que le compositor n'a pas encore validée).
    fn sync_size(&mut self) {
        // Masqué (profil non macOS-like) : la surface reste 1x1, la liste
        // des fenêtres ne la redimensionne pas.
        if !self.configured || !self.visible {
            return;
        }
        let target = self.geom.width(self.entries.len());
        if target != self.width {
            self.layer.set_size(target, self.geom.height);
            self.layer.commit();
        } else {
            self.draw();
        }
    }

    /// Affiche ou masque la barre (étape 3). Masquée : surface 1x1
    /// transparente (aucune zone exclusive, rien à l'écran) ; affichée :
    /// largeur des tuiles redemandée, le redessin part du configure.
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.layer.set_size(1, 1);
            self.layer.commit();
        } else if self.configured {
            // Largeur inconnue : on redemande celle du contenu actuel.
            self.width = 0;
            self.layer
                .set_size(self.geom.width(self.entries.len()), self.geom.height);
            self.layer.commit();
        }
    }

    /// Rechargement au SIGHUP (étape 3) : le compositor applique le nouveau
    /// profil, le dock suit — thème, géométrie `[dock]` et visibilité
    /// (profil macOS-like seul). Journal : jeton « dock : masqué ».
    fn reload_config(&mut self) {
        let (cfg, src) = yaor_config::DesktopConfig::load();
        self.theme = Theme::from_config(&cfg.theme);
        self.geom = Geometry::from_config(&cfg.dock);
        let visible = dock_visible(cfg.profile.name);
        eprintln!(
            "{PROG}: [{:.3}] rechargement : profil {} - dock {} (accent #{:06X}, source {})",
            uptime(),
            match cfg.profile.name {
                Some(p) => p.as_str(),
                None => "aucun",
            },
            if visible { "affiché" } else { "masqué" },
            self.theme.accent & 0x00FF_FFFF,
            match src {
                Some(p) => p.display().to_string(),
                None => "défauts".to_string(),
            }
        );
        if visible != self.visible {
            self.set_visible(visible);
        } else if visible {
            // Thème éventuellement neuf et `[dock]` éventuellement modifié :
            // taille redemandée, le configure déclenche le redessin.
            self.width = 0;
            self.sync_size();
        }
    }

    /// Clic sur une tuile : emblème = lanceur (visuel pour l'instant),
    /// fenêtre = restauration si minimisée puis activation.
    fn click(&mut self, x: f64) {
        let Some(idx) = self.geom.hit(self.entries.len(), x) else {
            eprintln!("{PROG}: [{:.3}] dock : clic hors tuile x={x}", uptime());
            return;
        };
        if idx == 0 {
            eprintln!("{PROG}: [{:.3}] dock : emblème (visuel)", uptime());
            return;
        }
        let Some(seat) = self.seat_object.clone() else {
            eprintln!("{PROG}: [{:.3}] dock : seat absent", uptime());
            return;
        };
        let Some(entry) = self.entries.get(idx - 1) else {
            return;
        };
        if entry.minimized {
            entry.handle.unset_minimized();
            eprintln!("{PROG}: [{:.3}] dock : restauration envoyée idx={idx}", uptime());
        }
        entry.handle.activate(&seat);
        eprintln!("{PROG}: [{:.3}] dock : activation envoyée idx={idx}", uptime());
    }
}

// ---------------------------------------------------------------------------
// Handlers sctk
// ---------------------------------------------------------------------------

impl CompositorHandler for Dock {
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

impl OutputHandler for Dock {
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

impl LayerShellHandler for Dock {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        eprintln!("{PROG}: couche fermée par le compositor");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Largeur calculée par le dock, validée par le compositor (borner à
        // la sortie) : on dessine EXACTEMENT la taille annoncée.
        self.width = if configure.new_size.0 == 0 {
            self.geom.width(self.entries.len())
        } else {
            configure.new_size.0
        };
        self.height = if configure.new_size.1 == 0 {
            self.geom.height
        } else {
            configure.new_size.1
        };
        if !self.configured {
            self.configured = true;
            eprintln!(
                "{PROG}: [{:.3}] dock configuré {}x{}",
                uptime(),
                self.width,
                self.height
            );
        }
        self.draw();
    }
}

impl SeatHandler for Dock {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.seat_object = Some(seat);
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        // ATTENTION : new_seat n'est pas invoqué pour les seats liés par
        // SeatState::new (burst initial) — seule la capability arrive.
        self.seat_object = Some(seat.clone());
        // Pas de clavier : le dock ne prend jamais le focus (Keyboard
        // interactivity None) — seulement le pointeur.
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("pointeur indisponible");
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
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Dock {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            if event.surface != *self.layer.wl_surface() {
                continue;
            }
            let x = event.position.0;
            match event.kind {
                Enter { .. } | Motion { .. } => {
                    let hovered = self.geom.hit(self.entries.len(), x);
                    if hovered != self.hovered {
                        self.hovered = hovered;
                        self.draw();
                    }
                }
                Leave { .. } => {
                    if self.hovered.take().is_some() {
                        self.draw();
                    }
                }
                Press { button, .. } => {
                    if button == 0x110 {
                        eprintln!(
                            "{PROG}: [{:.3}] dock : presse à ({}, {})",
                            uptime(),
                            x as i32,
                            event.position.1 as i32
                        );
                        self.click(x);
                    }
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for Dock {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// ---------------------------------------------------------------------------
// Foreign-toplevel : liste des fenêtres + activation
// ---------------------------------------------------------------------------

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Dock {
    // L'événement toplevel crée un objet (new_id) : la spécialisation
    // `event_created_child` est obligatoire (sinon panic au premier
    // toplevel) — même branchement que la taskbar du panel.
    wayland_client::event_created_child!(Dock, ZwlrForeignToplevelManagerV1, [
        wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (
            ZwlrForeignToplevelHandleV1,
            ()
        ),
    ]);

    fn event(
        state: &mut Self,
        _manager: &ZwlrForeignToplevelManagerV1,
        event: ManagerEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ManagerEvent::Toplevel { toplevel } => {
                state.entries.push(Entry {
                    handle: toplevel,
                    title: String::new(),
                    app_id: String::new(),
                    activated: false,
                    minimized: false,
                });
                eprintln!(
                    "{PROG}: [{:.3}] dock : fenêtre ajoutée (total={})",
                    uptime(),
                    state.entries.len()
                );
                state.sync_size();
            }
            ManagerEvent::Finished => {
                state.entries.clear();
                state.sync_size();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Dock {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: ToplevelEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Tableau d'états : couples (état, valeur) u32 — maximized=0,
        // minimized=1, activated=2 (contrat du protocole wlr).
        const MINIMIZED: u32 = 1;
        const ACTIVATED: u32 = 2;
        let decode = |bytes: &Vec<u8>| -> (bool, bool) {
            let mut min = false;
            let mut act = false;
            for chunk in bytes.chunks_exact(8) {
                let s = u32::from_ne_bytes(chunk[0..4].try_into().unwrap());
                match s {
                    MINIMIZED => min = true,
                    ACTIVATED => act = true,
                    _ => {}
                }
            }
            (min, act)
        };
        match event {
            ToplevelEvent::Title { title } => {
                if let Some(e) = state.entries.iter_mut().find(|e| e.handle == *handle) {
                    e.title = title;
                }
            }
            ToplevelEvent::AppId { app_id } => {
                if let Some(e) = state.entries.iter_mut().find(|e| e.handle == *handle) {
                    e.app_id = app_id;
                }
            }
            ToplevelEvent::State { state: bytes } => {
                let (minimized, activated) = decode(&bytes);
                if let Some(e) = state.entries.iter_mut().find(|e| e.handle == *handle) {
                    e.minimized = minimized;
                    e.activated = activated;
                }
            }
            ToplevelEvent::Done => {
                state.draw();
            }
            ToplevelEvent::Closed => {
                if let Some(idx) = state.entries.iter().position(|e| e.handle == *handle) {
                    state.entries.remove(idx);
                }
                eprintln!(
                    "{PROG}: [{:.3}] dock : fenêtre fermée (total={})",
                    uptime(),
                    state.entries.len()
                );
                state.sync_size();
            }
            _ => {}
        }
    }
}

delegate_compositor!(Dock);
delegate_output!(Dock);
delegate_shm!(Dock);
delegate_seat!(Dock);
delegate_pointer!(Dock);
delegate_layer!(Dock);
delegate_registry!(Dock);

impl ProvidesRegistryState for Dock {
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
        "Dock du profil macOS-like (barre layer-shell flottante, tuiles des fenêtres)"
    );
    let _ = writeln!(out, "  --help      affiche cette aide");
    let _ = writeln!(out, "  --version   affiche la version du produit");
}

/// Charge thème et emblème (`desktop.toml`) — une lecture et un décodage
/// uniques au démarrage, comme le panel.
fn load_assets(
    cfg: &yaor_config::DesktopConfig,
    src: Option<&std::path::Path>,
    emblem_px: u32,
) -> (Theme, Option<Vec<u8>>) {
    let theme = Theme::from_config(&cfg.theme);
    eprintln!(
        "{PROG}: [{:.3}] thème : accent #{:06X} panel #{:06X} source {}",
        uptime(),
        theme.accent & 0x00FF_FFFF,
        theme.panel_bg & 0x00FF_FFFF,
        match src {
            Some(p) => p.display().to_string(),
            None => "défauts".to_string(),
        }
    );
    let emblem = if cfg.logo.path.is_empty() {
        eprintln!("{PROG}: emblème : absent (logo.path vide)");
        None
    } else {
        match yaor_assets::load_image(std::path::Path::new(&cfg.logo.path)) {
            Some(img) => {
                let px = yaor_assets::resize(&img, emblem_px, emblem_px).to_argb8888();
                eprintln!(
                    "{PROG}: [{:.3}] emblème : {} {}x{} -> {}x{}",
                    uptime(),
                    cfg.logo.path,
                    img.w,
                    img.h,
                    emblem_px,
                    emblem_px
                );
                Some(px)
            }
            None => {
                eprintln!(
                    "{PROG}: [{:.3}] emblème : {} illisible - repli ambre",
                    uptime(),
                    cfg.logo.path
                );
                None
            }
        }
    };
    (theme, emblem)
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

    let (cfg, src) = yaor_config::DesktopConfig::load();
    let geom = Geometry::from_config(&cfg.dock);
    let emblem_px = geom.icon - 16;
    let (theme, emblem) = load_assets(&cfg, src.as_deref(), emblem_px);
    eprintln!(
        "{PROG}: [{:.3}] géométrie : {}x{} tuile {} pas {} marge {}",
        uptime(),
        geom.width(0),
        geom.height,
        geom.icon,
        geom.gap,
        geom.margin_bottom
    );
    // Profil élu (étape 3) : hors profil macOS-like le dock démarre masqué.
    // Journalisé avant la connexion (contrôle de configuration hors session).
    let visible = dock_visible(cfg.profile.name);
    eprintln!(
        "{PROG}: [{:.3}] profil : {} - dock {}",
        uptime(),
        match cfg.profile.name {
            Some(p) => p.as_str(),
            None => "aucun",
        },
        if visible { "affiché" } else { "masqué" }
    );

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

    // Barre : couche Top (au-dessus des fenêtres), ancrée en bas seulement —
    // le compositor la CENTRE horizontalement (arrange du layer-map) et,
    // faute de zone exclusive, les fenêtres gardent tout l'écran.
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some(NAMESPACE), None);
    // Masqué (profil non macOS-like) : surface 1x1, il ne revient qu'au
    // SIGHUP qui élit le profil macOS-like.
    layer.set_anchor(Anchor::BOTTOM);
    layer.set_margin(0, 0, geom.margin_bottom as i32, 0);
    layer.set_size(
        if visible { geom.width(0) } else { 1 },
        if visible { geom.height } else { 1 },
    );
    layer.set_exclusive_zone(0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    // Commit initial sans buffer : déclenche le configure du compositor,
    // qui pilote le premier rendu (modèle simple_layer).
    layer.commit();

    let initial_need = geom.width(8) as usize * geom.height as usize * 4;
    let pool = SlotPool::new(initial_need, &shm).expect("création du slot pool");

    let mut dock = Dock {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        layer,
        geom,
        width: 0,
        height: geom.height,
        configured: false,
        theme,
        emblem,
        emblem_px,
        entries: Vec::new(),
        foreign_manager: None,
        seat_object: None,
        hovered: None,
        visible,
        pool,
        pool_capacity: initial_need,
        pointer: None,
        exit: false,
    };
    eprintln!("{PROG}: [{:.3}] démarré (version {VERSION})", uptime());

    // Foreign-toplevel : liste des fenêtres + activation. Global absent
    // (compositor ancien) : le dock reste à la tuile emblème, non bloquant.
    match dock
        .registry_state
        .bind_one::<ZwlrForeignToplevelManagerV1, Dock, ()>(&qh, 1..=1, ())
    {
        Ok(manager) => dock.foreign_manager = Some(manager),
        Err(e) => eprintln!("{PROG}: foreign-toplevel indisponible : {e:?}"),
    }

    // SIGHUP (étape 3) : même rechargement que le panel — l'attente
    // surveille le fd Wayland et le tube du signal.
    let hup_tube = yaor_config::hup::install();
    // Flush AVANT la première attente : commit initial + bind
    // foreign-toplevel dorment dans le tampon local du socket —
    // `hup::wait` polle un socket silencieux sinon (le configure du
    // compositor n'arrive jamais, le dock reste noir). Le flush du
    // `blocking_dispatch` n'est atteint qu'APRÈS cette attente.
    let _ = conn.flush();
    while !dock.exit {
        if let Some(tube) = hup_tube {
            let wl_fd = conn.backend().poll_fd().as_raw_fd();
            if yaor_config::hup::wait(wl_fd, tube) {
                dock.reload_config();
                let _ = conn.flush();
            }
        }
        if event_queue.blocking_dispatch(&mut dock).is_err() {
            break;
        }
        // Flush de fin d'itération : les commits des handlers
        // (redessins, set_size après ajout/retrait de fenêtre) partent
        // aussitôt — sinon ils dorment dans le tampon local jusqu'à un
        // événement fortuit (leçon t-yaor-dock : la barre ne
        // grandissait qu'au passage du pointeur, qui réveillait le
        // socket).
        let _ = conn.flush();
    }
    eprintln!("{PROG}: [{:.3}] arrêt", uptime());
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_masque_hors_profil_macos_like() {
        use yaor_config::Profile;
        // Profil absent : le dock lancé s'affiche (comportement étape 2).
        assert!(dock_visible(None));
        assert!(dock_visible(Some(Profile::Macos)));
        assert!(!dock_visible(Some(Profile::Classic)));
        assert!(!dock_visible(Some(Profile::Tiling)));
    }

    fn geometrie() -> Geometry {
        Geometry::from_config(&yaor_config::DockConfig::default())
    }

    #[test]
    fn geometrie_par_defaut_centree_et_bornee() {
        let g = geometrie();
        assert_eq!((g.height, g.icon), (72, 56));
        // Emblème seul : 2 marges + 1 tuile.
        assert_eq!(g.width(0), 2 * 10 + 56);
        // Deux fenêtres : emblème + 2 tuiles + 2 espaces.
        assert_eq!(g.width(2), 2 * 10 + 3 * 56 + 2 * 10);
        assert_eq!(g.tile_y(), 8, "tuile centrée verticalement");
    }

    #[test]
    fn configuration_aberrante_reste_utilisable() {
        let bas = Geometry::from_config(&yaor_config::DockConfig {
            height: 0,
            icon: 0,
        });
        assert_eq!(bas.height, Geometry::MIN_H);
        assert_eq!(bas.icon, Geometry::MIN_ICON);
        let haut = Geometry::from_config(&yaor_config::DockConfig {
            height: 9_000,
            icon: 9_000,
        });
        assert_eq!(haut.height, Geometry::MAX_H);
        assert_eq!(haut.icon, Geometry::MAX_H - 16, "tuile laissée dans la barre");
    }

    #[test]
    fn clic_sur_la_tuile_correspond_au_bon_indice() {
        let g = geometrie();
        // Tuile 0 (emblème) : de 10 à 66 px.
        assert_eq!(g.hit(1, 10.0), Some(0));
        assert_eq!(g.hit(1, 65.9), Some(0));
        // Espace entre deux tuiles : aucun clic.
        assert_eq!(g.hit(2, 68.0), None);
        // Tuile 1 : 76 px (première fenêtre).
        assert_eq!(g.hit(2, 76.0), Some(1));
        // Hors de la barre, à droite.
        assert_eq!(g.hit(2, (g.width(2) + 5) as f64), None);
        // Une seule fenêtre : la tuile 2 n'existe pas.
        assert_eq!(g.hit(1, g.tile_x(2) as f64), None);
    }

    #[test]
    fn initiale_prend_l_app_id_puis_le_titre() {
        assert_eq!(initial("waytest", "Terminal"), 'W');
        assert_eq!(initial("", "Terminal"), 'T');
        assert_eq!(initial("xwtest", ""), 'X');
        // Rien d'exploitable : point (la police n'a pas de glyphe vide).
        assert_eq!(initial("", ""), '.');
        assert_eq!(initial("---", ""), '.');
        // Caractère absent de la police : point plutôt qu'un trou.
        assert_eq!(initial("é", ""), '.');
    }
}
