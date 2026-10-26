// Cible de focus clavier du compositor YAOR (jalon v0.4, étape 4).
//
// Modèle anvil/src/focus.rs (licence MIT © Smithay contributors),
// restreint au périmètre v0.4 : disjonction fenêtre Wayland (wl_surface)
// ↔ fenêtre X11 (X11Surface). Le pointeur garde le type WlSurface —
// une fenêtre X11 expose une wl_surface de son propre cru (client
// Xwayland), tout déplacement/clic s'y délègue déjà.
//
// La branche X11 délègue à l'impl smithay KeyboardTarget for
// X11Surface, qui règle l'état X côté serveur (SetInputFocus,
// WM_TAKE_FOCUS selon le mode d'entrée du client) — c'est le chemin
// de la livraison des touches aux applications X11.

use std::borrow::Cow;

use smithay::{
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
            GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
            MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        Seat,
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
    xwayland::X11Surface,
};

use crate::state::Yaor;

/// Cible de focus clavier : fenêtre Wayland ou fenêtre X11.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum FocusTarget {
    /// Fenêtre Wayland : focus porté par sa wl_surface (souvent le
    /// toplevel, comme smallvil).
    Wl(WlSurface),
    /// Fenêtre X11 : focus délégué à Xwayland (traduit en focus X).
    X11(X11Surface),
}

impl IsAlive for FocusTarget {
    fn alive(&self) -> bool {
        match self {
            FocusTarget::Wl(w) => w.alive(),
            FocusTarget::X11(w) => w.alive(),
        }
    }
}

impl WaylandFocus for FocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            FocusTarget::Wl(w) => w.wl_surface(),
            FocusTarget::X11(w) => w.wl_surface().map(Cow::Owned),
        }
    }

    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            FocusTarget::Wl(w) => w.same_client_as(object_id),
            FocusTarget::X11(w) => w.same_client_as(object_id),
        }
    }
}

impl KeyboardTarget<Yaor> for FocusTarget {
    fn enter(
        &self,
        seat: &Seat<Yaor>,
        data: &mut Yaor,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            FocusTarget::Wl(w) => KeyboardTarget::enter(w, seat, data, keys, serial),
            FocusTarget::X11(w) => KeyboardTarget::enter(w, seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<Yaor>, data: &mut Yaor, serial: Serial) {
        match self {
            FocusTarget::Wl(w) => KeyboardTarget::leave(w, seat, data, serial),
            FocusTarget::X11(w) => KeyboardTarget::leave(w, seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<Yaor>,
        data: &mut Yaor,
        key: KeysymHandle<'_>,
        state: smithay::backend::input::KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            FocusTarget::Wl(w) => KeyboardTarget::key(w, seat, data, key, state, serial, time),
            FocusTarget::X11(w) => KeyboardTarget::key(w, seat, data, key, state, serial, time),
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<Yaor>,
        data: &mut Yaor,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            FocusTarget::Wl(w) => KeyboardTarget::modifiers(w, seat, data, modifiers, serial),
            FocusTarget::X11(w) => KeyboardTarget::modifiers(w, seat, data, modifiers, serial),
        }
    }
}

impl PointerTarget<Yaor> for FocusTarget {
    fn enter(&self, seat: &Seat<Yaor>, data: &mut Yaor, event: &MotionEvent) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::enter(w, seat, data, event),
            FocusTarget::X11(w) => PointerTarget::enter(w, seat, data, event),
        }
    }

    fn motion(&self, seat: &Seat<Yaor>, data: &mut Yaor, event: &MotionEvent) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::motion(w, seat, data, event),
            FocusTarget::X11(w) => PointerTarget::motion(w, seat, data, event),
        }
    }

    fn relative_motion(
        &self,
        seat: &Seat<Yaor>,
        data: &mut Yaor,
        event: &RelativeMotionEvent,
    ) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::relative_motion(w, seat, data, event),
            FocusTarget::X11(w) => PointerTarget::relative_motion(w, seat, data, event),
        }
    }

    fn button(&self, seat: &Seat<Yaor>, data: &mut Yaor, event: &ButtonEvent) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::button(w, seat, data, event),
            FocusTarget::X11(w) => PointerTarget::button(w, seat, data, event),
        }
    }

    fn axis(&self, seat: &Seat<Yaor>, data: &mut Yaor, frame: AxisFrame) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::axis(w, seat, data, frame),
            FocusTarget::X11(w) => PointerTarget::axis(w, seat, data, frame),
        }
    }

    fn frame(&self, seat: &Seat<Yaor>, data: &mut Yaor) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::frame(w, seat, data),
            FocusTarget::X11(w) => PointerTarget::frame(w, seat, data),
        }
    }

    fn leave(&self, seat: &Seat<Yaor>, data: &mut Yaor, serial: Serial, time: u32) {
        match self {
            FocusTarget::Wl(w) => PointerTarget::leave(w, seat, data, serial, time),
            FocusTarget::X11(w) => PointerTarget::leave(w, seat, data, serial, time),
        }
    }

    // Gestes : pas de pavé tactile multi-toucher à la cible QA QEMU
    // (PS/2 + tablette) — no-op explicites, délégation inutile.
    fn gesture_swipe_begin(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GestureSwipeBeginEvent,
    ) {
    }
    fn gesture_swipe_update(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GestureSwipeUpdateEvent,
    ) {
    }
    fn gesture_swipe_end(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GestureSwipeEndEvent,
    ) {
    }
    fn gesture_pinch_begin(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GesturePinchBeginEvent,
    ) {
    }
    fn gesture_pinch_update(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GesturePinchUpdateEvent,
    ) {
    }
    fn gesture_pinch_end(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GesturePinchEndEvent,
    ) {
    }
    fn gesture_hold_begin(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GestureHoldBeginEvent,
    ) {
    }
    fn gesture_hold_end(
        &self,
        _seat: &Seat<Yaor>,
        _data: &mut Yaor,
        _event: &GestureHoldEndEvent,
    ) {
    }
}

// ---------------------------------------------------------------- conversions

impl From<WlSurface> for FocusTarget {
    fn from(w: WlSurface) -> Self {
        FocusTarget::Wl(w)
    }
}

impl From<X11Surface> for FocusTarget {
    fn from(w: X11Surface) -> Self {
        FocusTarget::X11(w)
    }
}
