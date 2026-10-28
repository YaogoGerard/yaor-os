// État commun du compositor YAOR (jalon v0.4).
//
// Modèle anvil/src/state.rs (licence MIT © Smithay contributors) :
// globals Wayland, socket de session, seat (clavier + pointeur), espace
// de fenêtres. Les helpers de presentation / scanout sont des
// transpositions simplifiées d'anvil (pas de layer-shell en v0.4).

use std::{
    ffi::OsString,
    os::unix::fs::PermissionsExt,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use smithay::{
    backend::{
        renderer::element::{default_primary_scanout_output_compare, RenderElementStates},
        session::Session,
    },
    desktop::utils::{
        surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
        update_surface_primary_scanout_output, with_surfaces_surface_tree,
        OutputPresentationFeedback,
    },
    desktop::{
        layer_map_for_output, PopupManager, Space, Window, WindowSurfaceType,
    },
    input::{Seat, SeatState},
    output::Output,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::wlr_layer::{KeyboardInteractivity, Layer as WlrLayer, WlrLayerShellState},
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
        xwayland_shell::XWaylandShellState,
        selection::primary_selection::PrimarySelectionState,
    },
    xwayland::{X11Wm, XWayland, XWaylandEvent},
};

use crate::session::UdevData;

/// Type du contenu de l'espace : fenêtres xdg (v0.4).
pub type SpaceWindow = Space<Window>;

/// Client data : l'état par client du compositor.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

/// L'état du compositor : globals Wayland, espace de fenêtres, seat.
pub struct Yaor {
    /// Horodatage de démarrage (diagnostic).
    #[allow(dead_code)]
    pub start_time: std::time::Instant,
    pub socket_name: Option<OsString>,
    pub display_handle: DisplayHandle,
    pub handle: LoopHandle<'static, Yaor>,
    /// Signal de boucle, pour les arrêts demandés par un raccourci.
    #[allow(dead_code)]
    pub loop_signal: LoopSignal,
    /// Fin de session propre (Ctrl+Alt+Suppr etc. l'arrêteront).
    pub running: AtomicBool,

    pub space: SpaceWindow,
    /// Disposition automatique (v0.5 « Desktop Core ») : liste tiling
    /// (maître + pile) et fenêtres flottantes — moteur crate::policy.
    pub layout: crate::policy::LayoutState,
    pub popups: PopupManager,

    pub pointer_location: Point<f64, Logical>,
    pub cursor_status: crate::render::CursorImageStatus,

    // Smithay states
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    /// Protocole wlr_layer_shell_v1 : panneaux/docks/barres (clients
    /// layer-shell — v0.6 Caméléon, UI Engine de l'HLD).
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    /// Global wl_output manager (conservé vivant pendant la session).
    #[allow(dead_code)]
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Yaor>,
    pub data_device_state: DataDeviceState,
    /// Protocole xwayland_shell_v1 : association surfaces wayland ↔ fenêtres
    /// X11 d'un client (gestion des demandes d'override-redirect).
    pub xwayland_shell_state: XWaylandShellState,
    /// Sélection primaire (bouton du milieu — x11 copie dans les deux).
    pub primary_selection_state: PrimarySelectionState,

    pub seat: Seat<Self>,

    pub backend_data: Option<UdevData>,

    /// WM X11 d'une instance Xwayland vivante (Option : pas encore prête —
    /// XWayland::spawn, puis XWaylandEvent::Ready quand le socket X11
    /// est connecté).
    pub xwm: Option<X11Wm>,
    /// Numéro du display X11 (::N) choisi par Xwayland (QA : `DISP-0`).
    pub xdisplay: Option<u32>,
}

impl Yaor {
    pub fn init(
        display: Display<Yaor>,
        dh: DisplayHandle,
        handle: LoopHandle<'static, Yaor>,
        loop_signal: LoopSignal,
        backend_data: UdevData,
        listen_on_socket: bool,
    ) -> Self {
        // Socket des clients compositor (comme anvil avec listen_on_socket).
        let socket_name = if listen_on_socket {
            let source = ListeningSocketSource::new_auto().expect("source de socket wayland");
            let socket_name = source.socket_name().to_os_string();
            handle
                .insert_source(source, |client_stream, _, data| {
                    if let Err(err) = data
                        .display_handle
                        .insert_client(client_stream, Arc::new(ClientState::default()))
                    {
                        tracing::warn!("client wayland refusé : {}", err);
                    };
                })
                .expect("insertion de la source socket");
            Some(socket_name)
        } else {
            None
        };

        handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, data| {
                    // Safety: on ne dépose pas le display
                    unsafe {
                        display.get_mut().dispatch_clients(data).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("insertion de la source display");

        let compositor_state = CompositorState::new::<Yaor>(&dh);
        let xdg_shell_state = XdgShellState::new::<Yaor>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Yaor>(&dh);
        let shm_state = ShmState::new::<Yaor>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Yaor>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Yaor>(&dh);
        let xwayland_shell_state = XWaylandShellState::new::<Yaor>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Yaor>(&dh);
        let popups = PopupManager::default();

        let seat_name = backend_data.session.seat().clone();
        let mut seat: Seat<Yaor> = seat_state.new_wl_seat(&dh, seat_name);
        // Clavier + pointeur branchés d'office (les dispositifs libinput
        // sont reconnus à chaud dans session.rs).
        seat.add_keyboard(Default::default(), 200, 25).expect("clavier du seat");
        seat.add_pointer();

        Yaor {
            start_time: std::time::Instant::now(),
            socket_name,
            display_handle: dh,
            handle,
            loop_signal,
            running: true.into(),
            layout: Default::default(),
            space: Space::default(),
            popups,
            pointer_location: (0.0, 0.0).into(),
            cursor_status: crate::render::CursorImageStatus::default_named(),
            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            xwayland_shell_state,
            primary_selection_state,
            seat,
            backend_data: Some(backend_data),
            xwm: None,
            xdisplay: None,
        }
    }

    /// Démarrage de Xwayland (v0.4 étape 4, portage anvil start_xwayland).
    ///
    /// Smithay prépare les sockets UNIX `/tmp/.X11-unix/X%N`
    /// (`prepare_x11_sockets`) et le verrou `/tmp/.X%N-lock`, puis lance le
    /// binaire ; le client Wayland correspondant est inséré par spawn. Le
    /// WM (X11Wm) n'existe qu'à l'événement Ready, quand les sockets X11
    /// sont connectées (x11rb) — d'ici là les clients X11 n'ont rien à
    /// attaquer.
    pub fn start_xwayland(&mut self) {
        use std::process::Stdio;

        // Le répertoire des sockets X11 des sessions locales : smithay le
        // suppose existant (convention FHS fixée par l'amont Xorg) ; le
        // live ne crée que /tmp au boot. Mode 1777 (collaboratif, comme
        // /tmp) pour un partage multi-sessions ultérieur.
        match std::fs::create_dir_all("/tmp/.X11-unix") {
            Ok(()) => {
                if let Err(err) = std::fs::set_permissions(
                    "/tmp/.X11-unix",
                    std::fs::Permissions::from_mode(0o1777),
                ) {
                    tracing::warn!("permissions /tmp/.X11-unix : {}", err);
                }
            }
            Err(err) => tracing::warn!("répertoire /tmp/.X11-unix : {}", err),
        }

        let (xwayland, client) = match XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        ) {
            Ok(ret) => ret,
            Err(err) => {
                tracing::error!("échec du démarrage de Xwayland : {}", err);
                return;
            }
        };

        let ret = self.handle.insert_source(xwayland, move |event, _, data| match event {
            XWaylandEvent::Ready {
                x11_socket,
                display_number,
            } => match X11Wm::start_wm(data.handle.clone(), x11_socket, client.clone()) {
                Ok(wm) => {
                    tracing::info!("WM X11 attaché (display :{display_number})");
                    data.xdisplay = Some(display_number);
                    data.xwm = Some(wm);
                }
                Err(err) => {
                    tracing::error!("échec du WM X11 : {err}");
                }
            },
            XWaylandEvent::Error => {
                tracing::warn!("Xwayland a planté au démarrage");
            }
        });

        if let Err(err) = ret {
            tracing::error!("source Xwayland : {}", err);
        }
    }

    /// Nom du seat (attribué par libseat à l'ouverture de session).
    pub fn seat_name(&self) -> String {
        self.backend_data.as_ref().unwrap().session.seat().clone()
    }

    /// Surface sous le pointeur : l'élément le plus haut (xdg_popup
    /// inclus), avec sa position absolue en logique f64 (convention
    /// smallvil pour le focus pointeur).
    ///
    /// Ordre de superposition (v0.6 étape 1) : layers Overlay/Top
    /// au-dessus des fenêtres (panneaux, docks), puis fenêtres de
    /// l'espace, puis layers Bottom/Background (fond — atteint
    /// seulement là où aucune fenêtre ne couvre le point ; conforme
    /// à nos clients : panneaux en Top, aucun client Background en
    /// v0.6).
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        // 1) couches hautes (panneaux) — au-dessus des fenêtres
        if let Some(found) = self.layer_under(pos, &[WlrLayer::Overlay, WlrLayer::Top]) {
            return Some(found);
        }
        // 2) fenêtres de l'espace
        if let Some(found) = self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        }) {
            return Some(found);
        }
        // 3) couches basses (fond, décor éventuel)
        self.layer_under(pos, &[WlrLayer::Bottom, WlrLayer::Background])
    }

    /// Recherche d'une surface layer-shell sous le pointeur, dans les
    /// couches demandées, sortie par sortie (coordonnées locales de
    /// sortie, comme anvil input_handler).
    fn layer_under(
        &self,
        pos: Point<f64, Logical>,
        kinds: &[WlrLayer],
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        for output in self.space.outputs() {
            let output_geo = match self.space.output_geometry(output) {
                Some(geo) => geo,
                None => continue,
            };
            let local = pos - output_geo.loc.to_f64();
            let map = layer_map_for_output(output);
            for kind in kinds {
                if let Some(layer) = map.layer_under(*kind, local) {
                    let layer_geo = map.layer_geometry(layer).unwrap_or_default();
                    if let Some((s, p)) = layer.surface_under(
                        local - layer_geo.loc.to_f64(),
                        WindowSurfaceType::ALL,
                    ) {
                        let abs = p.to_f64() + layer_geo.loc.to_f64() + output_geo.loc.to_f64();
                        return Some((s, abs));
                    }
                }
            }
        }
        None
    }

    /// La surface est-elle un layer-shell à clavier exclusif (Top ou
    /// Overlay) ? Les touches lui sont routées AVANT les fenêtres
    /// (menu de panneau ouvert — modèle anvil input_handler).
    pub fn exclusive_layer_surface(&self) -> Option<smithay::desktop::LayerSurface> {
        use smithay::wayland::shell::wlr_layer::LayerSurfaceCachedState;
        for layer in self.layer_shell_state.layer_surfaces().rev() {
            let data = smithay::wayland::compositor::with_states(layer.wl_surface(), |states| {
                *states.cached_state.get::<LayerSurfaceCachedState>().current()
            });
            if data.keyboard_interactivity == KeyboardInteractivity::Exclusive
                && (data.layer == WlrLayer::Top || data.layer == WlrLayer::Overlay)
            {
                return self.space.outputs().find_map(|o| {
                    layer_map_for_output(o)
                        .layers()
                        .find(|l| l.layer_surface() == &layer)
                        .cloned()
                });
            }
        }
        None
    }
}

/// Mise à jour de la sortie de scan-out primaire de chaque surface
/// (utilisé pour les frame callbacks et la presentation, anvil).
pub fn update_primary_scanout_output(
    space: &SpaceWindow,
    output: &Output,
    cursor_status: &crate::render::CursorImageStatus,
    render_element_states: &RenderElementStates,
) {
    space.elements().for_each(|window| {
        window.with_surfaces(|surface, states| {
            update_surface_primary_scanout_output(
                surface,
                output,
                states,
                render_element_states,
                default_primary_scanout_output_compare,
            );
        });
    });

    if let crate::render::CursorImageStatus::Surface(ref surface) = cursor_status {
        with_surfaces_surface_tree(surface, |surface, states| {
            update_surface_primary_scanout_output(
                surface,
                output,
                states,
                render_element_states,
                default_primary_scanout_output_compare,
            );
        });
    }
}

/// Feedback de présentation agrégé d'une sortie (anvil, sans layer).
pub fn take_presentation_feedback(
    output: &Output,
    space: &SpaceWindow,
    render_element_states: &RenderElementStates,
) -> OutputPresentationFeedback {
    let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

    space.elements().for_each(|window| {
        if space.outputs_for_element(window).contains(output) {
            window.take_presentation_feedback(
                &mut output_presentation_feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, render_element_states)
                },
            );
        }
    });

    output_presentation_feedback
}

/// Post-rendu : échelle fractionnelle + frame callbacks + feedback
/// dmabuf (adapté d'anvil post_repaint, sans layer ni fifo).
pub fn post_repaint(
    state: &mut Yaor,
    output: &Output,
    time: impl Into<Duration>,
    dmabuf_feedback: Option<crate::session::SurfaceDmabufFeedback>,
    render_element_states: &RenderElementStates,
) {
    let time = time.into();
    let throttle = Some(Duration::from_secs(1));

    state.space.elements().for_each(|window| {
        window.with_surfaces(|surface, states| {
            let primary_scanout_output = surface_primary_scanout_output(surface, states);

            if let Some(output) = primary_scanout_output.as_ref() {
                smithay::wayland::fractional_scale::with_fractional_scale(states, |fraction_scale| {
                    fraction_scale.set_preferred_scale(output.current_scale().fractional_scale());
                });
            }
        });

        if state.space.outputs_for_element(window).contains(output) {
            window.send_frame(output, time, throttle, surface_primary_scanout_output);
            if let Some(dmabuf_feedback) = dmabuf_feedback.as_ref() {
                window.send_dmabuf_feedback(
                    output,
                    surface_primary_scanout_output,
                    |surface, _| {
                        smithay::backend::renderer::element::utils::select_dmabuf_feedback(
                            surface,
                            render_element_states,
                            &dmabuf_feedback.render_feedback,
                            &dmabuf_feedback.scanout_feedback,
                        )
                    },
                );
            }
        }
    });
}
