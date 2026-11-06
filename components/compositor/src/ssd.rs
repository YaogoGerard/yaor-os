// Décorations serveur (SSD) du compositor YAOR — jalon v0.6 étape 1b.b.
//
// Modèle anvil/src/shell/ssd.rs (licence MIT © Smithay contributors),
// adapté à l'identité « ambre sur noir » (docs/05-ui-ux/branding/) :
// barre de titre avec trois boutons façon Windows à DROITE —
// minimiser (–), agrandir (□), fermer (×) — et bordure de focus.
//
// Les couleurs sont des constantes locales (convention « un besoin =
// un fournisseur » : pas de lecture de thème à l'exécution en 1b.b).
// Palette de référence : docs/05-ui-ux/branding/README.md.

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                solid::SolidColorBuffer,
                AsRenderElements, Kind,
            },
            Renderer,
        },
    },
    input::Seat,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::Resource,
    utils::{Logical, Point, Serial, Transform},
    wayland::seat::WaylandFocus,
};
use smithay::backend::renderer::ImportMem;
use std::cell::{RefCell, RefMut};

use crate::{element::WindowElement, state::Yaor};

// ---------------------------------------------------------------------------
// Palette « ambre sur noir » (1e.theme) : les couleurs viennent du
// thème de `desktop.toml` (crate `yaor-config`, repli canonique couleur
// par couleur). La palette est relue à chaque RECONSTRUCTION de buffer,
// et les caches de décoration portent `theme_epoch()` dans leur clé :
// un SIGHUP change donc couleurs, rayon et ombre sans quitter la
// session (TEST-CONFIG-001).
// ---------------------------------------------------------------------------

/// Éclaircit les composantes RVB (alpha conservé) — dérivation du
/// survol des pastilles, que le thème ne porte pas explicitement.
fn lighten(color: u32, amount: f64) -> u32 {
    let a = amount.clamp(0.0, 1.0);
    let ch = |shift: u32| {
        let v = ((color >> shift) & 0xFF) as f64;
        (((v + (255.0 - v) * a).round() as u32) & 0xFF) << shift
    };
    (color & 0xFF00_0000) | ch(16) | ch(8) | ch(0)
}

/// Hauteur de la barre de titre (px logiques).
pub const HEADER_H: i32 = 28;
/// Largeur des boutons (zones de clic QA).
pub const BUTTON_W: u32 = 32;
/// Épaisseur de la bordure de focus (px logiques).
pub const BORDER_W: i32 = 2;

// --- 1d.ssd : coins arrondis, pastilles, ombre ------------------------------

/// Rayon des coins arrondis (px logiques), du thème (1e.theme) — borné
/// dans `config::palette()`. Lu au moment du rendu : géométrie des
/// anneaux de coins et dessin du header restent cohérents.
pub(crate) fn corner_r() -> i32 {
    crate::config::palette().corner_radius
}
/// Marge de l'ombre douce autour de la fenêtre.
pub(crate) const SHADOW_M: i32 = 16;
/// Diamètre des pastilles de bouton.
const PASTILLE_D: i32 = 14;

// ---------------------------------------------------------------------------
// Rendu par pixel (buffers mémoire Argb8888 prémultiplié, 1d.ssd)
// ---------------------------------------------------------------------------

/// Composite « source over » prémultiplié d'un pixel de couleur
/// `color` (0xAARRGGBB) avec la couverture `a` (0..1).
fn over_px(px: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32, a: f64) {
    if a <= 0.0 || x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let a = a.clamp(0.0, 1.0);
    let i = ((y * w + x) * 4) as usize;
    let da = px[i + 3] as f64 / 255.0;
    for k in 0..3 {
        // octets B,G,R (petit boutisme) — k=0 -> poids bas
        let src = ((color >> (8 * k)) & 0xFF) as f64 / 255.0 * a;
        let dst = px[i + k] as f64 / 255.0; // déjà prémultiplié
        px[i + k] = ((src + dst * (1.0 - a)) * 255.0).round() as u8;
    }
    px[i + 3] = ((a + da * (1.0 - a)) * 255.0).round() as u8;
}

/// Couverture d'un rectangle à coins supérieurs arrondis (rayon `r`,
/// coins inférieurs droits) — AA 1 px sur l'arc et le bord droit.
fn rounded_top_alpha(x: i32, y: i32, w: i32, _h: i32, r: i32) -> f64 {
    let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
    let mut a = 1.0;
    if fx > w as f64 - 0.5 {
        a *= (w as f64 + 0.5 - fx).clamp(0.0, 1.0);
    }
    if fy < r as f64 {
        if fx < r as f64 {
            let d = (r as f64 - fx).hypot(r as f64 - fy);
            a *= (r as f64 + 0.5 - d).clamp(0.0, 1.0);
        } else if fx > (w - r) as f64 {
            let d = (fx - (w - r) as f64).hypot(r as f64 - fy);
            a *= (r as f64 + 0.5 - d).clamp(0.0, 1.0);
        }
    }
    a
}

/// État de décoration d'une fenêtre (porté par la user_data de
/// l'élément — modèle anvil WindowState).
pub struct DecorationState {
    pub is_ssd: bool,
    /// Fenêtre décorée focusée (bordure accent) — mis à jour aux
    /// changements de focus (clic, restauration, minimisation).
    pub is_focused: bool,
    /// Taille avant maximisation (pour rendre au bouton agrandir).
    pub maximized_pre_size: Option<smithay::utils::Size<i32, Logical>>,
    /// Position avant maximisation (le repli la restitue).
    pub maximized_pre_loc: Option<smithay::utils::Point<i32, Logical>>,
    pub header_bar: HeaderBar,
    pub border: Border,
    /// Ombre douce (1d.ssd) : cache (largeur, hauteur totale, focus).
    pub shadow: Option<((u64, i32, i32, bool), MemoryRenderBuffer)>,
}

/// Ombre douce (1d.ssd) : halo noir semi-transparent autour de la
/// fenêtre — gaussienne de distance au rectangle (σ 9 px), plus
/// marquée si focusée. Buffer (w+2M) x (h+2M), cache par clé.
pub(crate) fn shadow_buffer(w: i32, h: i32, focused: bool) -> MemoryRenderBuffer {
    let bw = w + 2 * SHADOW_M;
    let bh = h + 2 * SHADOW_M;
    let mut px = vec![0u8; (4 * (bw * bh)) as usize];
    // Intensité du thème (1e.theme) : 0.0 = pas d'ombre (dessinée
    // transparente plutôt que retirée — la géométrie de l'élément
    // garde sa marge constante).
    let amp = if focused { 0.55 } else { 0.40 } * crate::config::palette().shadow;
    let sigma2 = 2.0 * 9.0 * 9.0;
    for y in 0..bh {
        for x in 0..bw {
            let dx = if x < SHADOW_M {
                (SHADOW_M - x) as f64
            } else if x >= SHADOW_M + w {
                (x - (SHADOW_M + w) + 1) as f64
            } else {
                0.0
            };
            let dy = if y < SHADOW_M {
                (SHADOW_M - y) as f64
            } else if y >= SHADOW_M + h {
                (y - (SHADOW_M + h) + 1) as f64
            } else {
                0.0
            };
            let d2 = dx * dx + dy * dy;
            if d2 > 400.0 {
                // au-delà de ~2σ : rien (économie de blend)
                continue;
            }
            let a = amp * (-d2 / sigma2).exp();
            if a > 0.004 {
                over_px(&mut px, bw, bh, x, y, 0xFF_00_00_00, a);
            }
        }
    }
    MemoryRenderBuffer::from_slice(&px, Fourcc::Argb8888, (bw, bh), 1, Transform::Normal, None)
}

/// Barre de titre : fond à coins supérieurs arrondis + trois pastilles
/// à droite (minimiser, agrandir, fermer — 1d.ssd), rendues dans UN
/// buffer mémoire reconstruit seulement si géométrie/survol/focus
/// changent. Hit-test sur la position du pointeur en coordonnées
/// LOCALES à l'élément (Smithaye passe `MotionEvent.location - loc`
/// aux cibles pointeur) — les zones de clic 32 px sont inchangées.
#[derive(Debug, Clone)]
pub struct HeaderBar {
    pub pointer_loc: Option<Point<f64, Logical>>,
    pub width: u32,
    pub minimize_hover: bool,
    pub maximize_hover: bool,
    pub close_hover: bool,
    /// Buffer mémoire unique (fond arrondi + pastilles + glyphes).
    pub buffer: MemoryRenderBuffer,
    /// Clé du dernier rendu : (version du thème, largeur, focus,
    /// 3 survols).
    cache: Option<(u64, u32, bool, bool, bool, bool)>,
}

/// Barre de titre : construction du buffer (silhouette arrondie en
/// haut — anneau bordure + fond — puis pastilles et glyphes).
fn header_buffer(width: u32, focused: bool, min_hover: bool, max_hover: bool, close_hover: bool) -> MemoryRenderBuffer {
    let w = width as i32;
    let h = HEADER_H;
    let mut px = vec![0u8; (4 * (w * h)) as usize];

    let p = crate::config::palette();
    let rayon = p.corner_radius;

    // 1) anneau extérieur : silhouette arrondie en haut, couleur bordure.
    let border = if focused { p.accent } else { p.accent_dark };
    for y in 0..h {
        for x in 0..w {
            let a = rounded_top_alpha(x, y, w, h, rayon);
            over_px(&mut px, w, h, x, y, border, a);
        }
    }
    // 2) fond intérieur (encart 2 px, rayon réduit) — les 2 px du bas
    //    restent bordure pleine : soulignement ambre de la barre
    //    (anneau continu autour de la fenêtre, design Classique).
    for y in BORDER_W..h - BORDER_W {
        for x in BORDER_W..w - BORDER_W {
            let a = rounded_top_alpha(
                x - BORDER_W,
                y - BORDER_W,
                w - 2 * BORDER_W,
                h - 2 * BORDER_W,
                (rayon - BORDER_W).max(0),
            );
            over_px(&mut px, w, h, x, y, p.panel_bg, a);
        }
    }

    // 3) pastilles : cercles 14 px centrés dans les zones 32 px
    //    (minimiser, agrandir, fermer — de gauche à droite).
    let cy = h / 2;
    let centers = [
        (w - (3 * BUTTON_W as i32 - BUTTON_W as i32 / 2), cy, min_hover),
        (w - (2 * BUTTON_W as i32 - BUTTON_W as i32 / 2), cy, max_hover),
        (w - BUTTON_W as i32 / 2, cy, close_hover),
    ];
    for (cx, cy, hover) in centers {
        let color = if hover {
            lighten(p.button_bg, 0.28)
        } else {
            p.button_bg
        };
        let r = PASTILLE_D as f64 / 2.0;
        for dy in -r as i32..=r as i32 {
            for dx in -r as i32..=r as i32 {
                let d = (dx as f64 + 0.5).hypot(dy as f64 + 0.5);
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                over_px(&mut px, w, h, cx + dx, cy + dy, color, cov);
            }
        }
        // 4) glyphe (7x7) : – ▢ × — couleur du texte du thème.
        let glyph = p.text_fg;
        match (cx, hover) {
            (cx, _) if cx == centers[0].0 => {
                // minimiser : barre horizontale
                for gx in -3..=3 {
                    over_px(&mut px, w, h, cx + gx, cy, glyph, 1.0);
                }
            }
            (cx, _) if cx == centers[1].0 => {
                // agrandir : carré 7x7 en trait 1 px
                for g in -3..=3 {
                    over_px(&mut px, w, h, cx + g, cy - 3, glyph, 1.0);
                    over_px(&mut px, w, h, cx + g, cy + 3, glyph, 1.0);
                    over_px(&mut px, w, h, cx - 3, cy + g, glyph, 1.0);
                    over_px(&mut px, w, h, cx + 3, cy + g, glyph, 1.0);
                }
            }
            (cx, _) if cx == centers[2].0 => {
                // fermer : croix en diagonales
                for g in -3..=3 {
                    over_px(&mut px, w, h, cx + g, cy + g, glyph, 1.0);
                    over_px(&mut px, w, h, cx + g, cy - g, glyph, 1.0);
                }
            }
            _ => {}
        }
    }

    MemoryRenderBuffer::from_slice(&px, Fourcc::Argb8888, (w, h), 1, Transform::Normal, None)
}

/// Buffers de la bordure de focus : soulignement de la barre de titre
/// (bande haute) + gauche/droite/bas autour de la surface cliente.
/// 1d.ssd : anneaux de coins bas (quart d'anneau ambre + « morsure
/// d'ombre » par-dessus le coin carré du client — Smithay 0.7 ne sait
/// pas découper une surface cliente en arrondi ; documenté comme
/// compromis : coins hauts réellement arrondis via le header).
#[derive(Debug, Clone)]
pub struct Border {
    pub top: SolidColorBuffer,
    pub left: SolidColorBuffer,
    pub right: SolidColorBuffer,
    pub bottom: SolidColorBuffer,
    /// Quart d'anneau bas-gauche (R x R, rayon du thème).
    pub corner_bl: MemoryRenderBuffer,
    /// Quart d'anneau bas-droit (R x R, rayon du thème).
    pub corner_br: MemoryRenderBuffer,
    /// Clé du dernier rendu des anneaux : (version du thème,
    /// surface_h, focus, rayon).
    pub corners_cache: Option<(u64, i32, bool, i32)>,
}

/// Quart d'anneau de coin bas (buffer R x R, rayon du thème) :
/// arc bordure + remplissage « ombre » au-delà de l'arc (la zone du
/// coin carré du client est assombrie au lieu d'être découpée).
/// `bottom_right` : centre du cercle en (0,0) local sinon (R,0).
fn corner_buffer(focused: bool, bottom_right: bool) -> MemoryRenderBuffer {
    let p = crate::config::palette();
    let r = p.corner_radius;
    let mut px = vec![0u8; (4 * (r * r)) as usize];
    let border = if focused { p.accent } else { p.accent_dark };
    let (ccx, ccy) = if bottom_right { (0.0, 0.0) } else { (r as f64, 0.0) };
    for ly in 0..r {
        for lx in 0..r {
            let d = (lx as f64 + 0.5 - ccx).hypot(ly as f64 + 0.5 - ccy);
            // zone extérieure à l'arc : « morsure d'ombre » (noir
            // semi-transparent, cohérent avec l'ombre de fenêtre).
            if d > r as f64 {
                let a = (d - r as f64 - 0.5).clamp(0.0, 1.0) * 0.5;
                over_px(&mut px, r, r, lx, ly, 0xFF_00_00_00, a);
            }
            // quart d'anneau bordure
            let ring = (r as f64 + 0.5 - d).clamp(0.0, 1.0)
                * (d - (r - BORDER_W) as f64 + 0.5).clamp(0.0, 1.0);
            over_px(&mut px, r, r, lx, ly, border, ring);
        }
    }
    MemoryRenderBuffer::from_slice(&px, Fourcc::Argb8888, (r, r), 1, Transform::Normal, None)
}

/// Action demandée par un clic dans la barre de titre (résolue par
/// hit-test, exécutée hors de l'emprunt de la DecorationState).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsdClick {
    Close,
    ToggleMaximize,
    Minimize,
    Move,
}

impl HeaderBar {
    pub fn pointer_enter(&mut self, loc: Point<f64, Logical>) {
        self.pointer_loc = Some(loc);
    }

    pub fn pointer_leave(&mut self) {
        self.pointer_loc = None;
    }

    /// Clic dans la barre de titre : hit-test PUR (boutons à droite,
    /// sinon grab de déplacement). L'action retournée est exécutée
    /// HORS de l'emprunt de la DecorationState — les handlers
    /// (minimisation, maximisation, géométrie de l'espace) relisent
    /// `decoration_state()` et paniqueraient « RefCell already
    /// borrowed » sinon.
    pub fn clicked_action(&self) -> Option<SsdClick> {
        let width = self.width;
        match self.pointer_loc.as_ref() {
            // fermer (la plus à droite)
            Some(loc) if loc.x >= (width - BUTTON_W) as f64 => Some(SsdClick::Close),
            // agrandir
            Some(loc) if loc.x >= (width - 2 * BUTTON_W) as f64 => {
                Some(SsdClick::ToggleMaximize)
            }
            // minimiser (la plus à gauche des trois)
            Some(loc) if loc.x >= (width - 3 * BUTTON_W) as f64 => Some(SsdClick::Minimize),
            // ailleurs dans la barre : déplacer la fenêtre
            Some(_) => Some(SsdClick::Move),
            None => None,
        }
    }

    /// Met à jour le buffer mémoire seulement si géométrie, focus ou
    /// survols ont changé (sinon le buffer est réutilisé tel quel).
    pub fn redraw(&mut self, width: u32, focused: bool) {
        if width == 0 {
            self.width = 0;
            return;
        }
        self.width = width;

        let hover = |loc: &Option<Point<f64, Logical>>, from: u32, to: u32| {
            loc.as_ref()
                .map(|l| l.x >= from as f64 && l.x < to as f64)
                .unwrap_or(false)
        };

        // zones (x logiques, depuis la droite) : close [w-32,w),
        // maximize [w-64,w-32), minimize [w-96,w-64).
        let min_hover = hover(&self.pointer_loc, width - 3 * BUTTON_W, width - 2 * BUTTON_W);
        let max_hover = hover(&self.pointer_loc, width - 2 * BUTTON_W, width - BUTTON_W);
        let close_hover = hover(&self.pointer_loc, width - BUTTON_W, width);

        // recalcul des survols AVANT la comparaison de cache : le
        // pointeur a pu bouger depuis le dernier rendu.
        // La version du thème fait partie de la clé : un SIGHUP qui
        // change `[theme]` force la reconstruction du buffer.
        let key = (
            crate::config::theme_epoch(),
            width,
            focused,
            min_hover,
            max_hover,
            close_hover,
        );
        if self.cache == Some(key) {
            return;
        }

        self.buffer = header_buffer(width, focused, min_hover, max_hover, close_hover);
        self.cache = Some(key);
        self.minimize_hover = min_hover;
        self.maximize_hover = max_hover;
        self.close_hover = close_hover;
    }
}

impl<R> AsRenderElements<R> for HeaderBar
where
    R: Renderer + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    type RenderElement = MemoryRenderBufferRenderElement<R>;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        renderer: &mut R,
        location: Point<i32, smithay::utils::Physical>,
        _scale: smithay::utils::Scale<f64>,
        _alpha: f32,
    ) -> Vec<C> {
        let loc_f = Point::<f64, smithay::utils::Physical>::from((
            location.x as f64,
            location.y as f64,
        ));
        let elem = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            loc_f,
            &self.buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        )
        .expect("buffer de barre de titre");
        vec![C::from(elem)]
    }
}

impl Border {
    /// Met à jour les buffers de bordure selon le focus (couleur) et
    /// la géométrie courante de la surface (taille). Appelé à chaque
    /// rendu de l'élément (SSD actif). Les anneaux de coins ne sont
    /// reconstruits qu'au changement de focus.
    pub fn redraw(&mut self, width: i32, surface_h: i32, focused: bool) {
        let p = crate::config::palette();
        let color = crate::config::Palette::rgba(if focused {
            p.accent
        } else {
            p.accent_dark
        });
        // soulignement de la barre de titre (2 px sous le fond)
        self.top.update((width, BORDER_W), color);
        // gauche/droite descendent jusqu'au bas de la bordure basse
        // (recouvrent les angles) ; la basse déborde d'un trait de
        // chaque côté pour fermer le cadre.
        self.left.update((BORDER_W, surface_h + BORDER_W), color);
        self.right.update((BORDER_W, surface_h + BORDER_W), color);
        self.bottom.update((width + 2 * BORDER_W, BORDER_W), color);

        let corners_key = (crate::config::theme_epoch(), surface_h, focused, p.corner_radius);
        if self.corners_cache != Some(corners_key) {
            self.corner_bl = corner_buffer(focused, false);
            self.corner_br = corner_buffer(focused, true);
            self.corners_cache = Some(corners_key);
        }
    }
}

/// Bascule maximisée ⇔ normale pour une fenêtre (bouton agrandir).
/// L'état passe par xdg_toplevel::State::Maximized (le client suit la
/// configure) ; la taille de repos est mémorisée dans la décoration.
pub(crate) fn toggle_maximized(state: &mut Yaor, window: &WindowElement) {
    let Some(toplevel) = window.toplevel() else {
        // fenêtre X11 : hors périmètre (les fenêtres Xwayland sont
        // géométrées côté X/XWM en v0.6).
        tracing::info!("decoration : maximisation x11 non supportee");
        return;
    };
    let maximized = toplevel
        .current_state()
        .states
        .contains(xdg_toplevel::State::Maximized);
    if maximized {
        let (pre, pre_loc) = {
            let d = window.decoration_state();
            (d.maximized_pre_size, d.maximized_pre_loc)
        };
        toplevel.with_pending_state(|s| {
            s.states.unset(xdg_toplevel::State::Maximized);
            s.size = pre;
        });
        toplevel.send_configure();
        if let Some(loc) = pre_loc {
            state.space.map_element(window.clone(), loc, false);
        }
        tracing::info!("decoration : restauration");
    } else {
        let pre = toplevel.current_state().size;
        window.decoration_state().maximized_pre_size = pre;
        window.decoration_state().maximized_pre_loc =
            state.space.element_location(window);
        // taille cible : zone utile de la sortie de la fenêtre
        let target = maximized_target(state, window);
        toplevel.with_pending_state(|s| {
            s.states.set(xdg_toplevel::State::Maximized);
            s.size = target.map(|t| t.1);
        });
        toplevel.send_configure();
        // la fenêtre remplissant la zone utile, l'élément est remappé
        // à son origine (sinon il déborde de sa position courante et
        // les boutons du header passent hors écran).
        if let Some((loc, _)) = target {
            state.space.map_element(window.clone(), loc, false);
        }
        tracing::info!("decoration : maximisation");
    }
}

/// Cible de maximisation : origine + taille de la zone utile
/// (non_exclusive_zone) de la sortie qui porte le centre de la
/// fenêtre ; repli : None (le client choisira sa taille maximisée).
fn maximized_target(
    state: &mut Yaor,
    window: &WindowElement,
) -> Option<(smithay::utils::Point<i32, Logical>, smithay::utils::Size<i32, Logical>)> {
    let geo = state.space.element_geometry(window)?;
    let center = (geo.loc.x + geo.size.w / 2, geo.loc.y + geo.size.h / 2);
    let outputs: Vec<_> = state.space.outputs().cloned().collect();
    for output in outputs {
        let Some(out_geo) = state.space.output_geometry(&output) else {
            continue;
        };
        if out_geo.contains(center) {
            let map = smithay::desktop::layer_map_for_output(&output);
            let zone = map.non_exclusive_zone();
            return Some((zone.loc, zone.size));
        }
    }
    None
}

/// Grab de déplacement lancé par la barre de titre (chemin partagé
/// avec move_request xdg — le serial est celui du clic sur la barre,
/// le start_data du pointeur est déjà celui du bon client).
pub fn ssd_move_request(
    state: &mut Yaor,
    toplevel: &smithay::wayland::shell::xdg::ToplevelSurface,
    seat: &Seat<Yaor>,
    serial: Serial,
) {
    let wl_surface = toplevel.wl_surface();
    let Some(start_data) = seat.get_pointer().unwrap().grab_start_data() else {
        return;
    };
    let ok = start_data
        .focus
        .as_ref()
        .map(|(focus, _)| focus.same_client_as(&wl_surface.id()))
        .unwrap_or(false);
    if !ok {
        return;
    }

    let Some(window) = state
        .space
        .elements()
        .find(|w| {
            w.toplevel()
                .as_ref()
                .map(|t| t.wl_surface() == wl_surface)
                .unwrap_or(false)
        })
        .cloned()
    else {
        return;
    };
    crate::policy::mark_floating(state, &window);
    let Some(initial_window_location) = state.space.element_location(&window) else {
        return;
    };

    let grab = crate::grabs::MoveSurfaceGrab {
        start_data,
        window,
        initial_window_location,
    };
    seat.get_pointer()
        .unwrap()
        .set_grab(state, grab, serial, smithay::input::pointer::Focus::Clear);
}

/// DecorationState : créé à la volée dans la user_data de l'élément
/// (modèle anvil decoration_state/set_ssd).
impl WindowElement {
    /// Deux mêmes fenêtres (surface X11 pour X ; wl_surface sinon —
    /// comparaison d'identité d'objet wayland).
    pub fn same_as(&self, other: &WindowElement) -> bool {
        if let (Some(x1), Some(x2)) = (self.x11_surface(), other.x11_surface()) {
            return x1 == x2;
        }
        self.wl_surface() == other.wl_surface()
    }

    pub fn decoration_state(&self) -> RefMut<'_, DecorationState> {
        self.user_data().insert_if_missing(|| {
            RefCell::new(DecorationState {
                is_ssd: false,
                is_focused: false,
                maximized_pre_size: None,
                maximized_pre_loc: None,
                header_bar: HeaderBar {
                    pointer_loc: None,
                    width: 0,
                    minimize_hover: false,
                    maximize_hover: false,
                    close_hover: false,
                    // buffer trompe d'attente : remplacé au premier
                    // redraw (vraie taille connue au rendu).
                    buffer: MemoryRenderBuffer::new(
                        Fourcc::Argb8888,
                        (1, 1),
                        1,
                        Transform::Normal,
                        None,
                    ),
                    cache: None,
                },
                border: Border {
                    top: SolidColorBuffer::default(),
                    left: SolidColorBuffer::default(),
                    right: SolidColorBuffer::default(),
                    bottom: SolidColorBuffer::default(),
                    corner_bl: MemoryRenderBuffer::new(
                        Fourcc::Argb8888,
                        (1, 1),
                        1,
                        Transform::Normal,
                        None,
                    ),
                    corner_br: MemoryRenderBuffer::new(
                        Fourcc::Argb8888,
                        (1, 1),
                        1,
                        Transform::Normal,
                        None,
                    ),
                    corners_cache: None,
                },
                shadow: None,
            })
        });

        self.user_data()
            .get::<RefCell<DecorationState>>()
            .unwrap()
            .borrow_mut()
    }

    pub fn set_ssd(&self, ssd: bool) {
        self.decoration_state().is_ssd = ssd;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighten_eclaircit_sans_toucher_l_alpha() {
        assert_eq!(lighten(0xFF00_0000, 1.0), 0xFFFF_FFFF);
        assert_eq!(lighten(0xFF00_0000, 0.0), 0xFF00_0000);
        // #26262E éclairci de 28 % : composantes relevées, alpha intact.
        let c = lighten(0xFF26_262E, 0.28);
        assert_eq!(c & 0xFF00_0000, 0xFF00_0000);
        assert!(c & 0xFF > 0x2E, "composante bleue relevée");
    }
}
