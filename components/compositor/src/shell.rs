// Shell xdg (fenêtres, popups, seat, sortie, data device) du
// compositor YAOR (jalon v0.4).
//
// Adapté de smallvil/src/handlers (MIT © Smithay contributors) : la
// grande différence est le seat_name de libseat (session udev) et le
// curseur laissé au statut par défaut (pas d'xcursor en v0.4).

use smithay::{
    delegate_data_device, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell,
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{
        find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager, Window,
    },
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat, SeatHandler, SeatState,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            Client, Resource,
        },
    },
    utils::{Rectangle, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, with_states},
        output::OutputHandler,
        selection::data_device::{
            set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
        },
        selection::SelectionHandler,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
        shm::ShmHandler,
    },
};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    state::{ClientState, Yaor},
};

impl BufferHandler for Yaor {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {
    }
}

impl ShmHandler for Yaor {
    fn shm_state(self: &Self) -> &smithay::wayland::shm::ShmState {
        &self.shm_state
    }
}

impl smithay::wayland::compositor::CompositorHandler for Yaor {
    fn compositor_state(&mut self) -> &mut smithay::wayland::compositor::CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a smithay::wayland::compositor::CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Yaor>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| w.toplevel().map(|t| t.wl_surface().id() == root.id()).unwrap_or(false))
                .cloned()
            {
                window.on_commit();
            }
        }
        shell_commit(&mut self.popups, &mut self.space, surface);
    }
}
smithay::delegate_compositor!(Yaor);
delegate_shm!(Yaor);

impl XdgShellHandler for Yaor {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        // Taille initiale : le compositor guide les clients (à la configure
        // initiale, avant le premier commit — le client suit via xdg).
        // Démontre aussi la gestion de la taille à chaque nouvelle fenêtre
        // (QA : deux fenêtres 640x480 simultanées).
        surface.with_pending_state(|state| state.size = Some((640, 480).into()));
        let position = crate::policy::next_window_location(self);
        self.space.map_element(window, position, false);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();
        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();
        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // v0.4 : pas de grab popups ; viendra au moment des menus.
    }
}

delegate_xdg_shell!(Yaor);

fn check_grab(
    seat: &Seat<Yaor>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<Yaor>> {
    let pointer = seat.get_pointer()?;

    // Cette surface peut-elle démarrer un grab ? ( clic en cours, etc.)
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Commit d'une surface shell : configure initiale, popups, resize.
pub fn shell_commit(
    popups: &mut PopupManager,
    space: &mut smithay::desktop::Space<Window>,
    surface: &WlSurface,
) {
    // commits de toplevel
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == surface)
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        if !initial_configure_sent {
            window.toplevel().unwrap().send_configure();
        }
    }

    // commits de popups
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    xdg.send_configure().expect("configure initiale du popup");
                }
            }
            // gros cas : les autres types de popups n'existent pas en v0.4
            _ => {}
        }
    }

    // commit de resize (bord top/left : déplacement)
    grabs::handle_commit(space, surface);
}

use crate::grabs;

impl Yaor {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();
        let window_geo = self.space.element_geometry(window).unwrap();

        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

//
// Wl Seat
//

impl SeatHandler for Yaor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Yaor> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        // le statut du curseur est le statut global de Yaor —
        // rendu du curseur dans session.rs.
        self.cursor_status = image;
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

delegate_seat!(Yaor);

//
// Wl Data Device (sélections presse-papiers)
//

impl SelectionHandler for Yaor {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Yaor {
    fn data_device_state(&self) -> &smithay::wayland::selection::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Yaor {}
impl ServerDndGrabHandler for Yaor {}

delegate_data_device!(Yaor);

//
// Wl Output & Xdg Output
//

impl OutputHandler for Yaor {}
delegate_output!(Yaor);
