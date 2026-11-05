// Élément d'espace enveloppé + décorations serveur du compositor YAOR
// — jalon v0.6 étape 1b.b.
//
// Modèle anvil/src/shell/element.rs (licence MIT © Smithay
// contributors) : `WindowElement` enveloppe une fenêtre (Wayland ou
// X11) pour que la décoration (barre de titre + bordure de focus)
// fasse partie de l'élément de l'espace. C'est la raison d'être du
// wrapper : l'empilement est ENTRELACÉ — la décoration d'une fenêtre
// du fond ne peint jamais par-dessus une fenêtre de premier plan
// (impossible avec des éléments de décoration globaux dessinés après
// l'espace).
//
// La surface cliente est rendue décalée de HEADER_H vers le bas ; la
// géométrie d'espace (SpaceElement) inclut la barre ; le pointeur est
// routé vers la barre (crate::ssd::HeaderBar) quand il survole le
// haut de l'élément.

use std::{borrow::Cow, time::Duration};

use smithay::{
    backend::input::ButtonState,
    backend::renderer::{
        element::{
            solid::SolidColorRenderElement,
            surface::WaylandSurfaceRenderElement,
            AsRenderElements, Kind,
        },
        ImportAll, ImportMem, Renderer, Texture,
    },
    desktop::{
        space::SpaceElement,
        utils::OutputPresentationFeedback,
        Window, WindowSurface, WindowSurfaceType,
    },
    input::{
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
            GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
            MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        Seat,
    },
    output::Output,
    reexports::{
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    render_elements,
    utils::{user_data::UserDataMap, IsAlive, Logical, Point, Rectangle, Scale},
    wayland::{compositor::SurfaceData as WlSurfaceData, dmabuf::DmabufFeedback, seat::WaylandFocus},
};

use crate::{
    focus::FocusTarget,
    ssd,
    ssd::HEADER_H,
    state::Yaor,
};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowElement(pub Window);

impl WindowElement {
    /// Surface (ou décoration) sous un point LOCAL à l'élément :
    /// la barre de titre en haut, la surface cliente décalée en dessous.
    pub fn surface_under(
        &self,
        location: Point<f64, Logical>,
        window_type: WindowSurfaceType,
    ) -> Option<(FocusTarget, Point<i32, Logical>)> {
        let state = self.decoration_state();
        if state.is_ssd && location.y < HEADER_H as f64 {
            return Some((
                FocusTarget::Ssd(crate::element::SSD(self.clone())),
                Point::default(),
            ));
        }
        let offset = if state.is_ssd {
            Point::from((0, HEADER_H))
        } else {
            Point::default()
        };
        drop(state);

        let surface_under = self.0.surface_under(location - offset.to_f64(), window_type);
        let (under, loc) = match self.0.underlying_surface() {
            WindowSurface::Wayland(_) => surface_under
                .map(|(surface, loc)| (FocusTarget::Wl(surface), loc)),
            WindowSurface::X11(s) => surface_under.map(|(_, loc)| (FocusTarget::X11(s.clone()), loc)),
        }?;
        Some((under, loc + offset))
    }

    pub fn with_surfaces<F>(&self, processor: F)
    where
        F: FnMut(&WlSurface, &WlSurfaceData),
    {
        self.0.with_surfaces(processor);
    }

    pub fn send_frame<T, F>(&self, output: &Output, time: T, throttle: Option<Duration>, primary_scan_out_output: F)
    where
        T: Into<Duration>,
        F: FnMut(&WlSurface, &WlSurfaceData) -> Option<Output> + Copy,
    {
        self.0.send_frame(output, time, throttle, primary_scan_out_output)
    }

    pub fn send_dmabuf_feedback<'a, P, F>(&self, output: &Output, primary_scan_out_output: P, select_dmabuf_feedback: F)
    where
        P: FnMut(&WlSurface, &WlSurfaceData) -> Option<Output> + Copy,
        F: Fn(&WlSurface, &WlSurfaceData) -> &'a DmabufFeedback + Copy,
    {
        self.0
            .send_dmabuf_feedback(output, primary_scan_out_output, select_dmabuf_feedback)
    }

    pub fn take_presentation_feedback<F1, F2>(
        &self,
        output_feedback: &mut OutputPresentationFeedback,
        primary_scan_out_output: F1,
        presentation_feedback_flags: F2,
    ) where
        F1: FnMut(&WlSurface, &WlSurfaceData) -> Option<Output> + Copy,
        F2: FnMut(&WlSurface, &WlSurfaceData) -> wp_presentation_feedback::Kind + Copy,
    {
        self.0.take_presentation_feedback(
            output_feedback,
            primary_scan_out_output,
            presentation_feedback_flags,
        )
    }

    #[inline]
    pub fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        self.0.wl_surface()
    }

    #[inline]
    pub fn user_data(&self) -> &UserDataMap {
        self.0.user_data()
    }
}

impl std::ops::Deref for WindowElement {
    type Target = Window;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Window> for WindowElement {
    #[inline]
    fn from(window: Window) -> Self {
        WindowElement(window)
    }
}

impl IsAlive for WindowElement {
    #[inline]
    fn alive(&self) -> bool {
        self.0.alive()
    }
}

/// Cible pointeur de la décoration : la barre de titre. Jamais
/// focusée au clavier (le clavier reste à la fenêtre) — uniquement
/// survol/clic pour les boutons et le grab de déplacement.
#[derive(Debug, Clone, PartialEq)]
pub struct SSD(pub WindowElement);

impl IsAlive for SSD {
    #[inline]
    fn alive(&self) -> bool {
        self.0.alive()
    }
}

impl WaylandFocus for SSD {
    #[inline]
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        self.0.wl_surface()
    }
}

impl PointerTarget<Yaor> for SSD {
    fn enter(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, event: &MotionEvent) {
        let mut state = self.0.decoration_state();
        if state.is_ssd {
            state.header_bar.pointer_enter(event.location);
        }
    }
    fn motion(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, event: &MotionEvent) {
        let mut state = self.0.decoration_state();
        if state.is_ssd {
            state.header_bar.pointer_enter(event.location);
        }
    }
    fn relative_motion(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &RelativeMotionEvent) {}
    fn button(&self, seat: &Seat<Yaor>, data: &mut Yaor, event: &ButtonEvent) {
        // Une seule action par clic : au PRESS (le release rappellerait
        // sinon clicked_action — jetons comptés en double).
        if event.state != ButtonState::Pressed {
            return;
        }
        // L'action est résolue PUIS exécutée hors de l'emprunt de la
        // DecorationState : minimisation/maximisation relisent
        // `decoration_state()` via la géométrie de l'espace
        // (« RefCell already borrowed » sinon).
        let (action, loc_dbg, width) = {
            let state = self.0.decoration_state();
            if !state.is_ssd {
                return;
            }
            (
                state.header_bar.clicked_action(),
                state.header_bar.pointer_loc,
                state.header_bar.width,
            )
        };
        tracing::info!(
            "decoration : clic header loc {:?} width {} -> {:?}",
            loc_dbg, width, action
        );
        match action {
            Some(ssd::SsdClick::Close) => {
                tracing::info!("decoration : fermeture demandee");
                match self.0.underlying_surface() {
                    WindowSurface::Wayland(w) => w.send_close(),
                    WindowSurface::X11(w) => {
                        let _ = w.close();
                    }
                }
            }
            Some(ssd::SsdClick::ToggleMaximize) => ssd::toggle_maximized(data, &self.0),
            Some(ssd::SsdClick::Minimize) => {
                tracing::info!("decoration : minimisation demandee");
                crate::policy::minimize_window(data, &self.0);
            }
            Some(ssd::SsdClick::Move) => {
                let serial = event.serial;
                match self.0.underlying_surface() {
                    WindowSurface::Wayland(w) => {
                        let seat = seat.clone();
                        let toplevel = w.clone();
                        data.handle.insert_idle(move |data| {
                            ssd::ssd_move_request(data, &toplevel, &seat, serial);
                        });
                    }
                    WindowSurface::X11(w) => {
                        let window_x = w.clone();
                        data.handle.insert_idle(move |data| {
                            crate::x11::ssd_move_request_x11(data, &window_x);
                        });
                    }
                }
            }
            None => {}
        }
    }
    fn axis(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _frame: AxisFrame) {}
    fn frame(&self, _seat: &Seat<Yaor>, _data: &mut Yaor) {}
    fn leave(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _serial: smithay::utils::Serial, _time: u32) {
        let mut state = self.0.decoration_state();
        if state.is_ssd {
            state.header_bar.pointer_leave();
        }
    }
    fn gesture_swipe_begin(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GestureSwipeBeginEvent) {}
    fn gesture_swipe_update(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GestureSwipeUpdateEvent) {}
    fn gesture_swipe_end(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GestureSwipeEndEvent) {}
    fn gesture_pinch_begin(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GesturePinchBeginEvent) {}
    fn gesture_pinch_update(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GesturePinchUpdateEvent) {}
    fn gesture_pinch_end(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GesturePinchEndEvent) {}
    fn gesture_hold_begin(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GestureHoldBeginEvent) {}
    fn gesture_hold_end(&self, _seat: &Seat<Yaor>, _data: &mut Yaor, _event: &GestureHoldEndEvent) {}
}

impl SpaceElement for WindowElement {
    fn geometry(&self) -> Rectangle<i32, Logical> {
        let mut geo = SpaceElement::geometry(&self.0);
        if self.decoration_state().is_ssd {
            geo.size.h += HEADER_H;
        }
        geo
    }
    fn bbox(&self) -> Rectangle<i32, Logical> {
        let mut bbox = SpaceElement::bbox(&self.0);
        if self.decoration_state().is_ssd {
            bbox.size.h += HEADER_H;
        }
        bbox
    }
    fn is_in_input_region(&self, point: &Point<f64, Logical>) -> bool {
        if self.decoration_state().is_ssd {
            point.y < HEADER_H as f64
                || SpaceElement::is_in_input_region(
                    &self.0,
                    &(*point - Point::from((0.0, HEADER_H as f64))),
                )
        } else {
            SpaceElement::is_in_input_region(&self.0, point)
        }
    }
    fn z_index(&self) -> u8 {
        SpaceElement::z_index(&self.0)
    }
    fn set_activate(&self, activated: bool) {
        SpaceElement::set_activate(&self.0, activated);
    }
    fn output_enter(&self, output: &Output, overlap: Rectangle<i32, Logical>) {
        SpaceElement::output_enter(&self.0, output, overlap);
    }
    fn output_leave(&self, output: &Output) {
        SpaceElement::output_leave(&self.0, output);
    }
    fn refresh(&self) {
        SpaceElement::refresh(&self.0);
    }
}

render_elements!(
    pub WindowRenderElement<R> where R: ImportAll + ImportMem;
    Window=WaylandSurfaceRenderElement<R>,
    Decoration=SolidColorRenderElement,
);

impl<R: Renderer> std::fmt::Debug for WindowRenderElement<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Window(arg0) => f.debug_tuple("Window").field(arg0).finish(),
            Self::Decoration(arg0) => f.debug_tuple("Decoration").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

impl<R> AsRenderElements<R> for WindowElement
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Texture + 'static,
{
    type RenderElement = WindowRenderElement<R>;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        renderer: &mut R,
        mut location: Point<i32, smithay::utils::Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        let window_bbox = SpaceElement::bbox(&self.0);

        if self.decoration_state().is_ssd && !window_bbox.is_empty() {
            let window_geo = SpaceElement::geometry(&self.0);
            let width = window_geo.size.w;
            let surface_h = window_bbox.size.h;
            if width == 0 {
                return AsRenderElements::render_elements(&self.0, renderer, location, scale, alpha)
                    .into_iter()
                    .map(C::from)
                    .collect();
            }

            // 1) barre de titre (fond + boutons) à l'origine de l'élément
            let mut state = self.decoration_state();
            state.header_bar.redraw(width as u32);
            let focused = state.is_focused;
            state.border.redraw(width, surface_h, focused);

            let mut vec = AsRenderElements::<R>::render_elements::<WindowRenderElement<R>>(
                &state.header_bar,
                renderer,
                location,
                scale,
                alpha,
            );

            // 2) bordure de focus : soulignement de barre (top), puis
            //    gauche/droite/bas autour de la surface
            let border_loc = |x: i32, y: i32| {
                location
                    + Point::<i32, Logical>::from((x, y)).to_physical_precise_round(scale)
            };
            for (buffer, x, y) in [
                (&state.border.top, 0, HEADER_H - crate::ssd::BORDER_W),
                (&state.border.left, -crate::ssd::BORDER_W, HEADER_H),
                (&state.border.right, width, HEADER_H),
                (&state.border.bottom, -crate::ssd::BORDER_W, HEADER_H + surface_h),
            ] {
                vec.push(WindowRenderElement::Decoration(
                    SolidColorRenderElement::from_buffer(
                        buffer,
                        border_loc(x, y),
                        scale,
                        alpha,
                        Kind::Unspecified,
                    ),
                ));
            }
            drop(state);

            // 3) surface cliente, décalée sous la barre de titre
            location.y += (scale.y * HEADER_H as f64) as i32;
            let window_elements =
                AsRenderElements::render_elements(&self.0, renderer, location, scale, alpha);
            vec.extend(window_elements);
            vec.into_iter().map(C::from).collect()
        } else {
            AsRenderElements::render_elements(&self.0, renderer, location, scale, alpha)
                .into_iter()
                .map(C::from)
                .collect()
        }
    }
}

/// Met à jour l'état de focus décoré de chaque élément de l'espace :
/// seul `focused` porte la bordure accent (None : toutes au repos).
pub fn update_focused_decoration(state: &mut crate::state::Yaor, focused: Option<&WindowElement>) {
    state.space.elements().for_each(|w| {
        let is_focused = focused.map(|f| f.same_as(w)).unwrap_or(false);
        w.decoration_state().is_focused = is_focused;
    });
}
