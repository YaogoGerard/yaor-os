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
    backend::renderer::{
        element::{
            solid::{SolidColorBuffer, SolidColorRenderElement},
            AsRenderElements, Kind,
        },
        Renderer,
    },
    input::Seat,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::Resource,
    utils::{Logical, Point, Serial},
    wayland::seat::WaylandFocus,
};
use std::cell::{RefCell, RefMut};

use crate::{element::WindowElement, state::Yaor};

// ---------------------------------------------------------------------------
// Palette « ambre sur noir » (docs/05-ui-ux/design-system.md)
// ---------------------------------------------------------------------------

/// Fond de la barre de titre (`#101014`).
const BG_COLOR: [f32; 4] = [0.0627, 0.0627, 0.0784, 1.0];
/// Boutons au repos (`#26262E`).
const BUTTON_COLOR: [f32; 4] = [0.149, 0.149, 0.180, 1.0];
/// Boutons survolés (`#3A3A44`).
const BUTTON_HOVER_COLOR: [f32; 4] = [0.227, 0.227, 0.267, 1.0];
/// Bordure fenêtre focusée — accent (`#E8862E`).
const BORDER_FOCUSED_COLOR: [f32; 4] = [0.9098, 0.5255, 0.1804, 1.0];
/// Bordure fenêtre au repos — accent sombre (`#B96A1F`).
const BORDER_UNFOCUSED_COLOR: [f32; 4] = [0.7255, 0.4157, 0.1216, 1.0];

/// Hauteur de la barre de titre (px logiques).
pub const HEADER_H: i32 = 28;
/// Largeur des boutons (zones de clic QA).
pub const BUTTON_W: u32 = 32;
/// Épaisseur de la bordure de focus (px logiques).
pub const BORDER_W: i32 = 2;
const BUTTON_H: i32 = HEADER_H;

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
}

/// Barre de titre : fond + trois boutons à droite (minimiser,
/// agrandir, fermer — façon Windows). Hit-test sur la position du
/// pointeur en coordonnées LOCALES à l'élément (Smithay passe
/// `MotionEvent.location - loc` aux cibles pointeur).
#[derive(Debug, Clone)]
pub struct HeaderBar {
    pub pointer_loc: Option<Point<f64, Logical>>,
    pub width: u32,
    pub minimize_hover: bool,
    pub maximize_hover: bool,
    pub close_hover: bool,
    pub background: SolidColorBuffer,
    pub minimize_button: SolidColorBuffer,
    pub maximize_button: SolidColorBuffer,
    pub close_button: SolidColorBuffer,
}

/// Buffers de la bordure de focus : soulignement de la barre de titre
/// (bande haute) + gauche/droite/bas autour de la surface cliente.
#[derive(Debug, Clone)]
pub struct Border {
    pub top: SolidColorBuffer,
    pub left: SolidColorBuffer,
    pub right: SolidColorBuffer,
    pub bottom: SolidColorBuffer,
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

    pub fn redraw(&mut self, width: u32) {
        if width == 0 {
            self.width = 0;
            return;
        }

        self.background.update((width as i32, HEADER_H), BG_COLOR);

        let needs_redraw_buttons = width != self.width;
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

        if needs_redraw_buttons || min_hover != self.minimize_hover {
            self.minimize_button.update(
                (BUTTON_W as i32, BUTTON_H),
                if min_hover {
                    BUTTON_HOVER_COLOR
                } else {
                    BUTTON_COLOR
                },
            );
            self.minimize_hover = min_hover;
        }
        if needs_redraw_buttons || max_hover != self.maximize_hover {
            self.maximize_button.update(
                (BUTTON_W as i32, BUTTON_H),
                if max_hover {
                    BUTTON_HOVER_COLOR
                } else {
                    BUTTON_COLOR
                },
            );
            self.maximize_hover = max_hover;
        }
        if needs_redraw_buttons || close_hover != self.close_hover {
            self.close_button.update(
                (BUTTON_W as i32, BUTTON_H),
                if close_hover {
                    BUTTON_HOVER_COLOR
                } else {
                    BUTTON_COLOR
                },
            );
            self.close_hover = close_hover;
        }
    }
}

impl<R: Renderer> AsRenderElements<R> for HeaderBar {
    type RenderElement = SolidColorRenderElement;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        _renderer: &mut R,
        location: Point<i32, smithay::utils::Physical>,
        scale: smithay::utils::Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        let offset = |x: i32| {
            location
                + Point::<i32, Logical>::from((x, 0)).to_physical_precise_round(scale)
        };
        let w = self.width as i32;
        vec![
            // fond d'abord (dessous), boutons par-dessus à droite
            SolidColorRenderElement::from_buffer(
                &self.background,
                location,
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
            SolidColorRenderElement::from_buffer(
                &self.minimize_button,
                offset(w - 3 * BUTTON_W as i32),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
            SolidColorRenderElement::from_buffer(
                &self.maximize_button,
                offset(w - 2 * BUTTON_W as i32),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
            SolidColorRenderElement::from_buffer(
                &self.close_button,
                offset(w - BUTTON_W as i32),
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        ]
    }
}

impl Border {
    /// Met à jour les buffers de bordure selon le focus (couleur) et
    /// la géométrie courante de la surface (taille). Appelé à chaque
    /// rendu de l'élément (SSD actif).
    pub fn redraw(&mut self, width: i32, surface_h: i32, focused: bool) {
        let color = if focused {
            BORDER_FOCUSED_COLOR
        } else {
            BORDER_UNFOCUSED_COLOR
        };
        // soulignement de la barre de titre (2 px sous le fond)
        self.top.update((width, BORDER_W), color);
        // gauche/droite descendent jusqu'au bas de la bordure basse
        // (recouvrent les angles) ; la basse déborde d'un trait de
        // chaque côté pour fermer le cadre.
        self.left.update((BORDER_W, surface_h + BORDER_W), color);
        self.right.update((BORDER_W, surface_h + BORDER_W), color);
        self.bottom.update((width + 2 * BORDER_W, BORDER_W), color);
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
                    background: SolidColorBuffer::default(),
                    minimize_button: SolidColorBuffer::default(),
                    maximize_button: SolidColorBuffer::default(),
                    close_button: SolidColorBuffer::default(),
                },
                border: Border {
                    top: SolidColorBuffer::default(),
                    left: SolidColorBuffer::default(),
                    right: SolidColorBuffer::default(),
                    bottom: SolidColorBuffer::default(),
                },
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
