// Shell xdg (fenêtres, popups, seat, sortie, data device) du
// compositor YAOR (jalon v0.4).
//
// Adapté de smallvil/src/handlers (MIT © Smithay contributors) : la
// grande différence est le seat_name de libseat (session udev) et le
// curseur laissé au statut par défaut (pas d'xcursor en v0.4).

use smithay::{
    delegate_data_device, delegate_layer_shell, delegate_output, delegate_primary_selection,
    delegate_seat, delegate_shm, delegate_xdg_shell,
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{
        find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output, PopupKind,
        PopupManager, Window, WindowSurfaceType,
    },
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat, SeatHandler, SeatState,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_output, wl_seat, wl_surface::WlSurface},
            Client, Resource,
        },
    },
    utils::{Rectangle, Serial},
    output::Output,
    wayland::{
        seat::WaylandFocus,
        buffer::BufferHandler,
        compositor::{get_parent, is_sync_subsurface, with_states},
        output::OutputHandler,
        selection::data_device::{
            set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
        },
        selection::SelectionHandler,
        selection::SelectionTarget,
        shell::wlr_layer::{Layer, LayerSurface as WlrLayerSurfaceResource, WlrLayerShellHandler},
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
    fn shm_state(&self) -> &smithay::wayland::shm::ShmState {
        &self.shm_state
    }
}

impl smithay::wayland::compositor::CompositorHandler for Yaor {
    fn compositor_state(&mut self) -> &mut smithay::wayland::compositor::CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a smithay::wayland::compositor::CompositorClientState {
        // Client Xwayland : données propres à XWayland (modèle anvil) —
        // pas de ClientState de dispatcher de clients (le client est
        // inséré par smithay::xwayland::XWayland::spawn).
        if let Some(state) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &state.compositor_state;
        }
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
        // layer-shell : le configure initial est envoyé EN RÉPONSE au
        // premier commit du client (la spec interdit de l'envoyer avant
        // — gabarit anvil ensure_initial_configure, branche layer :
        // arrange d'abord pour respecter la taille demandée, puis
        // send_configure) ; les commits suivants re-déclenchent
        // l'arrange (ancre/taille changées).
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let found = layer_map_for_output(&output)
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .cloned();
            if let Some(layer) = found {
                let mut map = layer_map_for_output(&output);
                map.arrange();
                let initial_configure_sent = with_states(surface, |states| {
                    states
                        .data_map
                        .get::<smithay::wayland::shell::wlr_layer::LayerSurfaceData>()
                        .expect("layer sans données")
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                });
                if !initial_configure_sent {
                    layer.layer_surface().send_configure();
                }
                break;
            }
        }
        shell_commit(&mut self.popups, &mut self.space, surface);
    }
}
smithay::delegate_compositor!(Yaor);
delegate_shm!(Yaor);

// Panneaux layer-shell (v0.6 Caméléon — UI Engine, gabarit anvil).
impl WlrLayerShellHandler for Yaor {
    fn shell_state(&mut self) -> &mut smithay::wayland::shell::wlr_layer::WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurfaceResource,
        wl_output: Option<wl_output::WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = match wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| output_under_first(self))
        {
            Some(output) => output,
            None => {
                // Aucune sortie encore (démarrage) : la surface sera
                // ignorée — le client layer-shell reconnectera.
                tracing::warn!("layer-shell : aucune sortie pour « {} »", namespace);
                return;
            }
        };
        tracing::info!(
            "layer-shell : panneau « {} » sur {}",
            namespace,
            output.name()
        );
        let mut map = layer_map_for_output(&output);
        if let Err(err) = map.map_layer(&smithay::desktop::LayerSurface::new(surface, namespace)) {
            tracing::warn!("layer-shell : map_layer refusé : {}", err);
        }
    }

    fn layer_destroyed(&mut self, surface: smithay::wayland::shell::wlr_layer::LayerSurface) {
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let mut map = layer_map_for_output(&output);
            let layer = map
                .layers()
                .find(|&layer| layer.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = layer {
                map.unmap_layer(&layer);
            }
        }
    }
}
delegate_layer_shell!(Yaor);

/// Première sortie connue (repli d'un layer-shell sans wl_output).
fn output_under_first(state: &Yaor) -> Option<Output> {
    state.space.outputs().next().cloned()
}

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
        // Recensement tiling (v0.5) avant la pose dans l'espace : la
        // boucle re-tile à la frame suivante selon la configuration.
        crate::policy::register(self, &window);
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
                .find(|w| {
                    w.toplevel()
                        .as_ref()
                        .map(|t| t.wl_surface() == wl_surface)
                        .unwrap_or(false)
                })
                .unwrap()
                .clone();
            // Grab interactif de déplacement : la fenêtre quitte le
            // tiling et reste flottante à sa géométrie après relâchement.
            crate::policy::mark_floating(self, &window);

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
                .find(|w| {
                    w.toplevel()
                        .as_ref()
                        .map(|t| t.wl_surface() == wl_surface)
                        .unwrap_or(false)
                })
                .unwrap()
                .clone();
            // Grab interactif de redimensionnement : la fenêtre quitte
            // le tiling (flottante).
            crate::policy::mark_floating(self, &window);

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
    // commits de toplevel — les fenêtres X11 n'ont pas de toplevel xdg
    // (elles ne concernent que le bus XWM) : elles sont ignorées ici.
    if let Some(window) = space
        .elements()
        .find(|w| {
            w.toplevel()
                .map(|t| t.wl_surface() == surface)
                .unwrap_or(false)
        })
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
    if let Some(PopupKind::Xdg(ref xdg)) = popups.find_popup(surface) {
        // gros cas : les autres types de popups n'existent pas en v0.4
        if !xdg.is_initial_configure_sent() {
            xdg.send_configure().expect("configure initiale du popup");
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
    type KeyboardFocus = crate::focus::FocusTarget;
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

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&crate::focus::FocusTarget>) {
        let dh = &self.display_handle;
        // data device : le client qui porte la surface (wayland OU x11)
        let client = focused
            .and_then(|f| f.wl_surface())
            .and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

delegate_seat!(Yaor);

//
// Wl Data Device (sélections presse-papiers)
//

impl SelectionHandler for Yaor {
    type SelectionUserData = ();

    // Relais vers Xwayland (v0.4 étape 4) : toute sélection wayland mise
    // à jour/demandée est notifiée au WM X11 (portage anvil). Sans xwm
    // actif, les branches xwayland tombent en no-op.
    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<smithay::wayland::selection::SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.new_selection(ty, source.map(|source| source.mime_types())) {
                tracing::warn!(?err, ?ty, "échec de la sélection Xwayland (côté X)");
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: std::os::unix::io::OwnedFd,
        seat: Seat<Self>,
        _user_data: &(),
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.send_selection(ty, mime_type, fd, self.handle.clone()) {
                tracing::warn!(?err, "échec de l'envoi de sélection vers X");
            }
        }
        let _ = seat;
    }
}

impl smithay::wayland::selection::primary_selection::PrimarySelectionHandler for Yaor {
    fn primary_selection_state(
        &self,
    ) -> &smithay::wayland::selection::primary_selection::PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(Yaor);

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
