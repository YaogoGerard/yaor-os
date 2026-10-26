// Traitement des entrées du compositor YAOR (jalon v0.4).
//
// Adapté de smallvil/src/input.rs et anvil/src/input_handler.rs (MIT) :
// clavier (filtre « forward », le policy des raccourcis viendra d'une
// étape ultérieure), pointeur en mouvement RELATIF (contrairement à
// smallvil dont la session winit produit de l'absolu — la session
// libinput produit du relatif), boutons (focus clavier à la presse),
// molette. Les demandes de grab passent par crate::grabs.

use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::{Point, Rectangle, SERIAL_COUNTER},
    wayland::seat::WaylandFocus,
};

use crate::state::Yaor;

/// Entrées libinput → seat wayland (clavier + pointeur).
pub fn process_input_event<I: InputBackend>(
    data: &mut Yaor,
    event: InputEvent<I>,
) {
    match event {
        InputEvent::Keyboard { event, .. } => {
            let serial = SERIAL_COUNTER.next_serial();
            let time = event.time_msec();

            data.seat.get_keyboard().unwrap().input::<(), _>(
                data,
                event.key_code(),
                event.state(),
                serial,
                time,
                |_, _, _| FilterResult::Forward,
            );
        }
        InputEvent::PointerMotion { event, .. } => {
            // Mouvement relatif (libinput) : accumulation dans la
            // position du pointeur, bornée au rectangle des sorties.
            // Delta BRUT (delta_unaccel) : le déplacement du pointeur
            // vaut le delta du dispositif, sans accélération
            // matérielle — mouvement reproductible (QA positionnelle)
            // et 1:1 sur PS/2 ; la politique d'accélération viendra
            // avec la configuration v0.5.
            let serial = SERIAL_COUNTER.next_serial();
            let mut location = data.pointer_location + event.delta_unaccel();

            if let Some(rect) = combined_output_geometry(data) {
                location.x = location.x.clamp(rect.loc.x as f64, (rect.loc.x + rect.size.w) as f64);
                location.y = location.y.clamp(rect.loc.y as f64, (rect.loc.y + rect.size.h) as f64);
            }
            data.pointer_location = location;

            let under = data.surface_under(location);
            let pointer = data.seat.get_pointer().unwrap();
            pointer.motion(
                data,
                under,
                &MotionEvent {
                    location,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(data);
        }
        InputEvent::PointerMotionAbsolute { event, .. } => {
            let output = data.space.outputs().next().unwrap();

            let output_geo = data.space.output_geometry(output).unwrap();

            let pos: Point<f64, smithay::utils::Logical> =
                event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

            let serial = SERIAL_COUNTER.next_serial();

            let pointer = data.seat.get_pointer().unwrap();

            let under = data.surface_under(pos);

            pointer.motion(
                data,
                under,
                &MotionEvent {
                    location: pos,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(data);
        }
        InputEvent::PointerButton { event, .. } => {
            let pointer = data.seat.get_pointer().unwrap();
            let keyboard = data.seat.get_keyboard().unwrap();

            let serial = SERIAL_COUNTER.next_serial();

            let button = event.button_code();

            let button_state = event.state();

            if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                if let Some((window, _loc)) = data
                    .space
                    .element_under(pointer.current_location())
                    .map(|(w, l)| (w.clone(), l))
                {
                    data.space.raise_element(&window, true);
                    // Fenêtre X11 : on demande à Xwayland de la remonter
                    // côté X aussi (sinon stacking du serveur diverge).
                    let focus = if let Some(surface) = window.x11_surface().cloned() {
                        if let Some(xwm) = data.xwm.as_mut() {
                            let _ = xwm.raise_window(&surface);
                        }
                        crate::focus::FocusTarget::X11(surface)
                    } else {
                        crate::focus::FocusTarget::Wl(
                            window.wl_surface().map(|w| w.into_owned()).expect(
                                "adversaire : fenêtre wayland sans surface",
                            ),
                        )
                    };
                    keyboard.set_focus(data, Some(focus), serial);
                    data.space.elements().for_each(|window| {
                        if let Some(toplevel) = window.toplevel() {
                            toplevel.send_pending_configure();
                        }
                    });
                } else {
                    data.space.elements().for_each(|window| {
                        window.set_activated(false);
                        if let Some(toplevel) = window.toplevel() {
                            toplevel.send_pending_configure();
                        }
                    });
                    keyboard.set_focus(
                        data,
                        Option::<crate::focus::FocusTarget>::None,
                        serial,
                    );
                }
            }

            pointer.button(
                data,
                &ButtonEvent {
                    button,
                    state: button_state,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(data);
        }
        InputEvent::PointerAxis { event, .. } => {
            let source = event.source();

            let horizontal_amount = event
                .amount(Axis::Horizontal)
                .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
            let vertical_amount = event
                .amount(Axis::Vertical)
                .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
            let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
            let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

            let mut frame = AxisFrame::new(event.time_msec()).source(source);
            if horizontal_amount != 0.0 {
                frame = frame.value(Axis::Horizontal, horizontal_amount);
                if let Some(discrete) = horizontal_amount_discrete {
                    frame = frame.v120(Axis::Horizontal, discrete as i32);
                }
            }
            if vertical_amount != 0.0 {
                frame = frame.value(Axis::Vertical, vertical_amount);
                if let Some(discrete) = vertical_amount_discrete {
                    frame = frame.v120(Axis::Vertical, discrete as i32);
                }
            }

            if source == AxisSource::Finger {
                if event.amount(Axis::Horizontal) == Some(0.0) {
                    frame = frame.stop(Axis::Horizontal);
                }
                if event.amount(Axis::Vertical) == Some(0.0) {
                    frame = frame.stop(Axis::Vertical);
                }
            }

            let pointer = data.seat.get_pointer().unwrap();
            pointer.axis(data, frame);
            pointer.frame(data);
        }
        _ => {}
    }
}

/// Rectangle couvrant toutes les sorties (pour borner le pointeur).
fn combined_output_geometry(data: &mut Yaor) -> Option<Rectangle<i32, smithay::utils::Logical>> {
    let mut rect: Option<Rectangle<i32, smithay::utils::Logical>> = None;
    for output in data.space.outputs().cloned().collect::<Vec<_>>() {
        let geo = data.space.output_geometry(&output);
        rect = match (rect, geo) {
            (None, geo) => geo,
            (Some(r), Some(g)) => Some(r.merge(g)),
            (Some(r), None) => Some(r),
        };
    }
    rect
}
