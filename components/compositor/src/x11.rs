// Bus XWayland du compositor YAOR (jalon v0.4, étape 4).
//
// Portage d'anvil/src/shell/x11.rs (licence MIT © Smithay contributors) :
// le rattachement des fenêtres X11 à l'espace (Space<Window>), la
// configuration, et les sélections (presse-papiers / primaire). Le
// démarrage lui-même est dans state::start_xwayland ; les demandes du
// protocole xwayland_shell_v1 sont déléguées depuis ici.
//
// Types de focus inchangés (WlSurface) : une fenêtre X11 mappe ses
// interactions sur une wl_surface du client Xwayland ; le focus clavier
// lui est remis comme à tout client Wayland (cf. input.rs) — Xwayland
// redistribue ensuite vers la fenêtre X concernée.

use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle, SERIAL_COUNTER},
    wayland::{
        selection::{
            data_device::{
                clear_data_device_selection, current_data_device_selection_userdata,
                request_data_device_client_selection, set_data_device_selection,
            },
            primary_selection::{
                clear_primary_selection, current_primary_selection_userdata,
                request_primary_client_selection, set_primary_selection,
            },
            SelectionTarget,
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XwmHandler,
    },
};

use crate::{grabs::MoveSurfaceGrab, state::Yaor};

impl XWaylandShellHandler for Yaor {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

smithay::delegate_xwayland_shell!(Yaor);

/// Fenêtre de l'espace associée à une surface X11 donnée.
fn element_for_x11_window(
    space: &smithay::desktop::Space<Window>,
    window: &X11Surface,
) -> Option<Window> {
    space
        .elements()
        .find(|e| matches!(e.x11_surface(), Some(w) if w == window))
        .cloned()
}

impl XwmHandler for Yaor {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).unwrap();
        let window = Window::new_x11_window(window);
        let position = crate::policy::next_window_location(self);
        self.space.map_element(window.clone(), position, false);

        // Configure avec la taille du cadre de l'espace (bbox), comme
        // anvil : le client s'aligne sur la géométrie du compositor.
        let bbox = self.space.element_bbox(&window).expect("bbox fenêtre X11");
        let Some(xsurface) = window.x11_surface() else {
            unreachable!("fenêtre X11 fraîchement créée");
        };
        xsurface.configure(Some(bbox)).unwrap();
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let location = window.geometry().loc;
        let window = Window::new_x11_window(window);
        self.space.map_element(window, location, true);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(elem) = element_for_x11_window(&self.space, &window) {
            self.space.unmap_elem(&elem);
        }
        if !window.is_override_redirect() {
            window.set_mapped(false).unwrap();
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // On accepte le redimensionnement demandé, pas le déplacement
        // libre (modèle anvil) : la politique de placement reste au
        // compositor.
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<smithay::xwayland::xwm::X11Window>,
    ) {
        // Fenêtres override-redirect : repositionnement fidèle, ordre non
        // géré (modèle anvil TODO).
        let Some(elem) = element_for_x11_window(&self.space, &window) else {
            return;
        };
        self.space.map_element(elem, geometry.loc, false);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        // Déplacement par le pointeur : réutilise le grab xdg (le grab ne
        // connaît pas la nature xdg/x11 de la fenêtre ; handle_commit ne
        // repositionne que les fenêtres en resize).
        let Some(start_data) = self.seat.get_pointer().unwrap().grab_start_data() else {
            return;
        };

        let Some(element) = element_for_x11_window(&self.space, &window) else {
            return;
        };

        let Some(initial_window_location) = self.space.element_location(&element) else {
            return;
        };

        let grab = MoveSurfaceGrab {
            start_data,
            window: element,
            initial_window_location,
        };

        let pointer = self.seat.get_pointer().unwrap();
        pointer.set_grab(
            self,
            grab,
            SERIAL_COUNTER.next_serial(),
            smithay::input::pointer::Focus::Clear,
        );
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _button: u32,
        _edges: X11ResizeEdge,
    ) {
        // Redimensionnement à la souris côté serveur X : hors périmètre
        // v0.4 (le grab de resize du compositor parle l'état xdg). On
        // ré-conigure la taille courante pour ne pas laisser la fenêtre
        // en attente.
        let geo = window.geometry();
        let _ = window.configure(Rectangle::new(geo.loc, geo.size));
    }

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        // Accès servi uniquement si une fenêtre de ce Xwayland a le focus
        // clavier (modèle anvil).
        let Some(keyboard) = self.seat.get_keyboard() else {
            return false;
        };
        match keyboard.current_focus() {
            Some(crate::focus::FocusTarget::X11(surface)) => {
                surface.xwm_id().map(|id| id == xwm).unwrap_or(false)
            }
            _ => false,
        }
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: std::os::unix::io::OwnedFd,
    ) {
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(err) =
                    request_data_device_client_selection(&self.seat, mime_type, fd)
                {
                    tracing::error!(?err, "échec de la demande de presse-papiers wayland");
                }
            }
            SelectionTarget::Primary => {
                if let Err(err) = request_primary_client_selection(&self.seat, mime_type, fd) {
                    tracing::error!(?err, "échec de la demande de sélection primaire");
                }
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => set_data_device_selection(
                &self.display_handle,
                &self.seat,
                mime_types,
                (),
            ),
            SelectionTarget::Primary => {
                set_primary_selection(&self.display_handle, &self.seat, mime_types, ())
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.seat).is_some() {
                    clear_data_device_selection(&self.display_handle, &self.seat)
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.seat).is_some() {
                    clear_primary_selection(&self.display_handle, &self.seat)
                }
            }
        }
    }
}
