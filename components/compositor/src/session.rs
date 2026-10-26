// Session udev/KMS du compositor YAOR (jalon v0.4, étape 2).
//
// Adapté de anvil/src/udev.rs (licence MIT © Smithay contributors) :
// session libseat, backend udev, GpuManager/GbmGlesBackend, sorties DRM
// (DrmOutputManager + DrmOutput + DrmCompositor), boucle de rendu
// pilotée par les vblank. Allégements v0.4 : pas de leasing DRM, pas de
// syncobj, pas de DnD, pas d'élément FPS — le curseur est un buffer
// mémoire statique ; XWayland et la config viennent aux étapes suivantes.

use std::{
    collections::hash_map::HashMap,
    io,
    ops::Not,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once,
    },
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        allocator::{
            dmabuf::Dmabuf,
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc, Modifier,
        },
        drm::{
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            CreateDrmNodeError, DrmAccessError, DrmDevice, DrmDeviceFd, DrmError, DrmEvent,
            DrmEventMetadata, DrmEventTime, DrmNode, DrmSurface, NodeType,
        },
        egl::{context::ContextPriority, EGLDevice, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::Error as OutputDamageTrackerError,
            element::{memory::MemoryRenderBuffer, RenderElementStates},
            gles::GlesRenderer,
            multigpu::{gbm::GbmGlesBackend, GpuManager},
            ImportDma, ImportEgl, ImportMemWl,
        },
        session::{
            libseat::{self, LibSeatSession},
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
        SwapBuffersError,
    },
    delegate_dmabuf,
    backend::renderer::element::AsRenderElements,
    desktop::utils::OutputPresentationFeedback,
    input::pointer::CursorImageAttributes,
    output::{Mode as WlMode, Output, PhysicalProperties},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop, RegistrationToken,
        },
        drm::control::{connector, crtc, Device, ModeTypeFlags},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
        wayland_protocols::wp::{
            linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1,
            presentation_time::server::wp_presentation_feedback,
        },
        wayland_server::{backend::GlobalId, Display, DisplayHandle},
    },
    utils::{DeviceFd, IsAlive, Monotonic, Point, Scale, Time},
    wayland::{
        compositor,
        dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        presentation::Refresh,
    },
};
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};
use tracing::{debug, error, info, trace, warn};

use crate::{
    input,
    render::{
        output_elements, CursorImageStatus, CustomRenderElements, PointerElement, UdevRenderer,
    },
    state::Yaor,
};

// Formats de sortie : 8 bits seulement (virtio-gpu / QEMU, comme les
// SUPPORTED_FORMATS_8BIT_ONLY d'anvil — Anvil maintient les 10 bits pour
// au matériel moderne ; v0.4 cible spécifiquement QEMU).
const SUPPORTED_FORMATS_8BIT_ONLY: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

/// Arrêt demandé par un signal (SIGTERM, SIGINT). Un gestionnaire de
/// signal ne peut faire qu'un nombre restreint d'opérations
/// (async-signal-safe) : on se contente d'un store atomique, consulté
/// par la boucle principale à chaque réveil.
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn request_stop(_signal: libc::c_int) {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
}

/// Arrêt propre : la boucle sort, l'état est détruit, `libseat` ferme la
/// session (master KMS rendu, console restituée, socket Wayland retirée).
fn install_signal_handlers() {
    // SAFETY : le gestionnaire ne fait qu'un store atomique (donc
    // async-signal-safe) ; la valeur de retour (ancien gestionnaire) est
    // inutilisée, et les deux signaux sont déjà ceux d'un arrêt demandé.
    unsafe {
        libc::signal(libc::SIGTERM, request_stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, request_stop as *const () as libc::sighandler_t);
    }
}

/// Identité (device, crtc) d'une sortie udev — stockée dans les
/// user_data de la sortie (modèle anvil UdevOutputId).
#[derive(Debug, PartialEq)]
pub struct UdevOutputId {
    pub device_id: DrmNode,
    pub crtc: crtc::Handle,
}

/// Feedbacks dmabuf d'une surface (modèle anvil SurfaceDmabufFeedback).
#[derive(Debug, Clone)]
pub struct SurfaceDmabufFeedback {
    pub render_feedback: DmabufFeedback,
    pub scanout_feedback: DmabufFeedback,
}

struct SurfaceData {
    dh: DisplayHandle,
    device_id: DrmNode,
    render_node: Option<DrmNode>,
    global: Option<GlobalId>,
    drm_output: DrmOutput<
        GbmAllocator<DrmDeviceFd>,
        GbmFramebufferExporter<DrmDeviceFd>,
        Option<OutputPresentationFeedback>,
        DrmDeviceFd,
    >,
    dmabuf_feedback: Option<SurfaceDmabufFeedback>,
    last_presentation_time: Option<Time<Monotonic>>,
    vblank_throttle_timer: Option<RegistrationToken>,
}

impl Drop for SurfaceData {
    fn drop(&mut self) {
        if let Some(global) = self.global.take() {
            self.dh.remove_global::<Yaor>(global);
        }
    }
}

struct BackendData {
    surfaces: HashMap<crtc::Handle, SurfaceData>,
    non_desktop_connectors: Vec<(connector::Handle, crtc::Handle)>,
    drm_output_manager: DrmOutputManager<
        GbmAllocator<DrmDeviceFd>,
        GbmFramebufferExporter<DrmDeviceFd>,
        Option<OutputPresentationFeedback>,
        DrmDeviceFd,
    >,
    drm_scanner: DrmScanner,
    render_node: Option<DrmNode>,
    registration_token: RegistrationToken,
}

/// Données backend udev — portées par l'état (`Yaor::backend_data`).
///
/// `session` est déclarée en dernier par lisibilité : la session n'est
/// qu'un `Weak` vers l'implémentation libseat, dont le propriétaire réel
/// est le notifier inséré dans la boucle. C'est le démontage explicite de
/// `run_udev` qui garantit que les sorties DRM (et le `DrmDeviceNotifier`
/// qui détient le device) sont libérées avant la fermeture du siège —
/// sinon seatd révoque le maître DRM trop tôt et la restauration échoue.
pub struct UdevData {
    dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
    primary_gpu: DrmNode,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    backends: HashMap<DrmNode, BackendData>,
    pointer_image: MemoryRenderBuffer,
    pointer_element: PointerElement,
    clock: smithay::utils::Clock<Monotonic>,
    pub session: LibSeatSession,
}

#[derive(Debug, thiserror::Error)]
enum DeviceAddError {
    #[error("ouverture du périphérique via libseat impossible : {0}")]
    DeviceOpen(libseat::Error),
    #[error("initialisation du périphérique drm impossible : {0}")]
    DrmDevice(DrmError),
    #[error("initialisation du périphérique gbm impossible : {0}")]
    GbmDevice(std::io::Error),
    #[error("accès au nœud drm impossible : {0}")]
    DrmNode(CreateDrmNodeError),
    #[error("ajout du périphérique au GpuManager impossible : {0}")]
    AddNode(smithay::backend::egl::Error),
    #[error("le périphérique n'a pas de render node")]
    NoRenderNode,
    #[error("GPU primaire manquant")]
    PrimaryGpuMissing,
}

fn get_surface_dmabuf_feedback(
    primary_gpu: DrmNode,
    render_node: Option<DrmNode>,
    scanout_node: DrmNode,
    gpus: &mut GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    surface: &DrmSurface,
) -> Option<SurfaceDmabufFeedback> {
    let primary_formats = gpus.single_renderer(&primary_gpu).ok()?.dmabuf_formats();
    let render_formats = if let Some(render_node) = render_node {
        gpus.single_renderer(&render_node).ok()?.dmabuf_formats()
    } else {
        FormatSet::default()
    };

    let all_render_formats = primary_formats
        .iter()
        .chain(render_formats.iter())
        .copied()
        .collect::<FormatSet>();

    let planes = surface.planes().clone();

    // La tranche scan-out est limitée aux formats dont on sait aussi
    // rendre, comme la remarque d'anvil (fallback rendu possible).
    let planes_formats = surface
        .plane_info()
        .formats
        .iter()
        .copied()
        .chain(planes.overlay.into_iter().flat_map(|p| p.formats))
        .collect::<FormatSet>()
        .intersection(&all_render_formats)
        .copied()
        .collect::<FormatSet>();

    let builder = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), primary_formats);
    let render_feedback = if let Some(render_node) = render_node {
        builder
            .clone()
            .add_preference_tranche(render_node.dev_id(), None, render_formats.clone())
            .build()
            .unwrap()
    } else {
        builder.clone().build().unwrap()
    };

    let scanout_feedback = builder
        .add_preference_tranche(
            surface.device_fd().dev_id().unwrap(),
            Some(zwp_linux_dmabuf_feedback_v1::TrancheFlags::Scanout),
            planes_formats,
        )
        .add_preference_tranche(scanout_node.dev_id(), None, render_formats)
        .build()
        .unwrap();

    Some(SurfaceDmabufFeedback {
        render_feedback,
        scanout_feedback,
    })
}

impl DmabufHandler for Yaor {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self
            .backend_data
            .as_mut()
            .unwrap()
            .dmabuf_state
            .as_mut()
            .unwrap()
            .0
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        let backend_data = self.backend_data.as_mut().unwrap();
        if backend_data
            .gpus
            .single_renderer(&backend_data.primary_gpu)
            .and_then(|mut renderer| renderer.import_dmabuf(&dmabuf, None))
            .is_ok()
        {
            dmabuf.set_node(backend_data.primary_gpu);
            let _ = notifier.successful::<Yaor>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(Yaor);

/// Point d'entrée `--tty-udev` : session libseat + backend udev + boucle.
pub fn run_udev() {
    install_signal_handlers();
    let mut event_loop = EventLoop::try_new().unwrap();
    let display = Display::new().unwrap();
    let mut display_handle = display.handle();

    // Session libseat (droit DRM master, VTY, etc.).
    let (session, notifier) = match LibSeatSession::new() {
        Ok(ret) => ret,
        Err(err) => {
            error!("ne peut pas ouvrir la session : {}", err);
            return;
        }
    };

    // GPU primaire : variable d'environnement de test, sinon énumération.
    let primary_gpu = if let Ok(var) = std::env::var("YAOR_DRM_DEVICE") {
        DrmNode::from_path(var).expect("chemin d'unité drm invalide")
    } else {
        primary_gpu(session.seat())
            .unwrap()
            .and_then(|x| DrmNode::from_path(x).ok()?.node_with_type(NodeType::Render)?.ok())
            .unwrap_or_else(|| {
                all_gpus(session.seat())
                    .unwrap()
                    .into_iter()
                    .find_map(|x| DrmNode::from_path(x).ok())
                    .expect("aucun GPU KMS")
            })
    };
    info!("GPU primaire : {}", primary_gpu);

    let gpus =
        GpuManager::new(GbmGlesBackend::with_context_priority(ContextPriority::High))
            .unwrap();

    let pointer_image = crate::render::CursorImage::arrow_static();
    let mut pointer_element = PointerElement::default();
    pointer_element.set_buffer(pointer_image.clone());

    let data = UdevData {
        dmabuf_state: None,
        session,
        primary_gpu,
        gpus,
        backends: HashMap::new(),
        pointer_image,
        pointer_element,
        clock: smithay::utils::Clock::new(),
    };
    let mut state = Yaor::init(
        display,
        display_handle.clone(),
        event_loop.handle(),
        event_loop.get_signal(),
        data,
        true,
    );

    if let Some(socket_name) = state.socket_name.as_ref() {
        info!(socket = %socket_name.to_string_lossy(), "socket wayland publiée");
    }

    // Backend udev.
    let udev_backend = match UdevBackend::new(state.seat_name()) {
        Ok(ret) => ret,
        Err(err) => {
            error!(error = ?err, "échec de l'initialisation du backend udev");
            return;
        }
    };

    // Backend entrées libinput.
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
            state.backend_data.as_ref().unwrap().session.clone().into(),
        );
    libinput_context
        .udev_assign_seat(&state.seat_name())
        .expect("attribution du siège libinput");
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());

    // Les objets pilotés par la boucle : les événements d'entrée.
    event_loop
        .handle()
        .insert_source(libinput_backend, move |mut event, _, data| {
            if let InputEvent::DeviceAdded { device } = &mut event {
                if device.has_capability(DeviceCapability::Keyboard) {
                    if let Some(led_state) =
                        data.seat.get_keyboard().map(|keyboard| keyboard.led_state())
                    {
                        device.led_update(led_state.into());
                    }
                }
            }

            input::process_input_event(data, event)
        })
        .unwrap();

    // Notifications de session : pause/relance du tty.
    event_loop
        .handle()
        .insert_source(notifier, move |event, &mut (), data| match event {
            SessionEvent::PauseSession => {
                libinput_context.suspend();
                info!("session suspendue (bascule de tty)");

                for backend in data.backend_data.as_mut().unwrap().backends.values_mut() {
                    backend.drm_output_manager.pause();
                }
            }
            SessionEvent::ActivateSession => {
                info!("session relancée");

                let now = data.backend_data.as_ref().unwrap().clock.now();
                if let Err(err) = libinput_context.resume() {
                    error!("échec de reprise de libinput : {:?}", err);
                }
                for (node, backend) in data
                    .backend_data
                    .as_mut()
                    .unwrap()
                    .backends
                    .iter_mut()
                    .map(|(handle, backend)| (*handle, backend))
                {
                    backend.drm_output_manager.activate(false).ok();

                    data.handle.insert_idle(move |data| {
                        data.render(node, None, now);
                    });
                }
            }
        })
        .unwrap();

    // Le nœud primaire est initialisé d'abord pour que les nœuds
    // « display only » puissent retomber dessus pour le rendu (anvil).
    let primary_node = primary_gpu
        .node_with_type(NodeType::Primary)
        .and_then(|node| node.ok());
    let primary_device = udev_backend.device_list().find(|(device_id, _)| {
        primary_node
            .map(|primary_node| *device_id == primary_node.dev_id())
            .unwrap_or(false)
            || *device_id == primary_gpu.dev_id()
    });

    if let Some((device_id, path)) = primary_device {
        let node = DrmNode::from_dev_id(device_id).expect("nœud primaire");
        state
            .device_added(node, path)
            .expect("échec d'initialisation du nœud primaire");
    }

    let primary_device_id = primary_device.map(|(device_id, _)| device_id);
    for (device_id, path) in udev_backend.device_list() {
        if Some(device_id) == primary_device_id {
            continue;
        }

        if let Err(err) = DrmNode::from_dev_id(device_id)
            .map_err(DeviceAddError::DrmNode)
            .and_then(|node| state.device_added(node, path))
        {
            error!("périphérique Deutschen {device_id} ignoré : {err}");
        }
    }

    // formats shm disponibles selon le rendu primaire (comme anvil)
    state.shm_state.update_formats(
        state
            .backend_data
            .as_mut()
            .unwrap()
            .gpus
            .single_renderer(&primary_gpu)
            .unwrap()
            .shm_formats(),
    );

    // Accélération EGL matérielle (comme anvil, feature egl)
    {
        let mut renderer = state
            .backend_data
            .as_mut()
            .unwrap()
            .gpus
            .single_renderer(&primary_gpu)
            .unwrap();
        if renderer.bind_wl_display(&display_handle).is_ok() {
            info!("accélération EGL matérielle activée");
        } else {
            info!("pas d'accélération EGL (chemin mémoire)");
        }

        let dmabuf_formats = renderer.dmabuf_formats();
        let default_feedback =
            DmabufFeedbackBuilder::new(primary_gpu.dev_id(), dmabuf_formats).build().unwrap();
        let mut dmabuf_state = DmabufState::new();
        let global = dmabuf_state
            .create_global_with_default_feedback::<Yaor>(&state.display_handle, &default_feedback);
        state.backend_data.as_mut().unwrap().dmabuf_state = Some((dmabuf_state, global));
    }

    // Feedbacks dmabuf par surface (anvil), après les surfaces initiales.
    {
        let backend_data = state.backend_data.as_mut().unwrap();
        let primary_gpu_ = backend_data.primary_gpu;
        let gpus = &mut backend_data.gpus;
        backend_data
            .backends
            .iter_mut()
            .for_each(|(node, backend_data)| {
                backend_data.surfaces.values_mut().for_each(|surface_data| {
                    surface_data.dmabuf_feedback = surface_data.dmabuf_feedback.take().or_else(
                        || {
                            surface_data.drm_output.with_compositor(|compositor| {
                                get_surface_dmabuf_feedback(
                                    primary_gpu_,
                                    surface_data.render_node,
                                    *node,
                                    gpus,
                                    compositor.surface(),
                                )
                            })
                        },
                    );
                });
            });
    }

    // Source des événements udev (ajout/modif/suppression d'unités).
    event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, data| match event {
            UdevEvent::Added { device_id, path } => {
                if let Err(err) = DrmNode::from_dev_id(device_id)
                    .map_err(DeviceAddError::DrmNode)
                    .and_then(|node| data.device_added(node, &path))
                {
                    error!("périphérique {device_id} ignoré : {err}");
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    data.device_changed(node)
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    data.device_removed(node)
                }
            }
        })
        .unwrap();

    // Xwayland (v0.4 étape 4) : lancé une fois les sources branchées, la
    // sortie primaire configurée par l'ajout du device ci-dessus
    // (modèle anvil udev.rs : après les sortie « up »).
    state.start_xwayland();

    // Boucle principale (anvil : dispatch 16 ms puis refresh + flush).
    // `STOP_REQUESTED` permet un arrêt propre sur SIGTERM/SIGINT.
    while state.running.load(Ordering::SeqCst) && !STOP_REQUESTED.load(Ordering::SeqCst) {
        let result = event_loop.dispatch(Some(Duration::from_millis(16)), &mut state);
        if result.is_err() {
            state.running.store(false, Ordering::SeqCst);
        } else {
            state.space.refresh();
            state.popups.cleanup();
            display_handle.flush_clients().unwrap();
        }
    }

    if STOP_REQUESTED.load(Ordering::SeqCst) {
        info!("arrêt demandé par un signal — libération de la session");
    }

    // Démontage explicite, dans l'ordre des dépendances : le device DRM
    // (donc le maître KMS et la restauration de l'état CRTC précédent) doit
    // être détruit AVANT l'état Wayland et la boucle. Deux pièges :
    // - seatd révoque le maître DRM dès que le client libseat se ferme
    //   (restauration en EACCES), or ce client est détruit avec la boucle ;
    // - le `DrmDeviceNotifier` inséré dans la boucle détient le dernier
    //   `Arc` sur l'`AtomicDrmDevice` : tant qu'il vit, libérer les sorties
    //   ne détruit pas le device.
    // On retire donc les notifiers DRM de la boucle, puis on libère les
    // sorties : le device tombe ici, session libseat encore ouverte.
    let mut udev = state.backend_data.take().expect("backend udev monté");
    info!("démontage : retrait des notifiers DRM de la boucle");
    for backend in udev.backends.values() {
        state.handle.remove(backend.registration_token);
    }
    info!("démontage : notifiers DRM retirés");
    info!("démontage : libération des sorties DRM");
    udev.backends.clear();
    info!("démontage : sorties DRM libérées");
    drop(udev);
    info!("démontage : données udev libérées");
    let socket_name = state.socket_name.take();
    drop(state);
    info!("démontage : état Wayland libéré");
    drop(event_loop);
    info!("démontage : boucle et siège libérés");
    // Retrait explicite de la socket (`ListeningSocket::drop` de smithay ne
    // s'est pas exécuté ab initio : ses clones résident dans des sources
    // Calloop encore référencées par l'état du compositor — constat QA
    // TEST-GFX-003/004). Le retrait à la main garantit la promesse
    // « session fermée proprement » (pas de socket résiduelle) et évite
    // qu'un prochain client se connecte à un compositor mort.
    if let Some(name) = socket_name {
        if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
            let mut path = std::path::PathBuf::from(rt);
            path.push(&name);
            match std::fs::remove_file(&path) {
                Ok(()) => info!("socket wayland retirée : {}", path.display()),
                Err(err) => tracing::warn!("retrait socket wayland : {}", err),
            }
            let mut lock = path;
            lock.as_mut_os_string().push(".lock");
            match std::fs::remove_file(&lock) {
                Ok(()) => info!("verrou socket retiré : {}", lock.display()),
                Err(err) => tracing::warn!("retrait verrou socket : {}", err),
            }
        }
    }
}

impl Yaor {
    fn device_added(&mut self, node: DrmNode, path: &Path) -> Result<(), DeviceAddError> {
        // Ouverture via libseat (droits KMS de la session).
        let fd = self
            .backend_data
            .as_mut()
            .unwrap()
            .session
            .open(
                path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            )
            .map_err(DeviceAddError::DeviceOpen)?;

        let fd = DrmDeviceFd::new(DeviceFd::from(fd));

        let (drm, notifier) =
            DrmDevice::new(fd.clone(), true).map_err(DeviceAddError::DrmDevice)?;
        let gbm = GbmDevice::new(fd).map_err(DeviceAddError::GbmDevice)?;

        let registration_token = self
            .handle
            .insert_source(
                notifier,
                move |event, metadata, data: &mut Yaor| match event {
                    DrmEvent::VBlank(crtc) => {
                        data.frame_finish(node, crtc, metadata);
                    }
                    DrmEvent::Error(error) => {
                        error!("{:?}", error);
                    }
                },
            )
            .unwrap();

        let mut try_initialize_gpu = || {
            let display =
                unsafe { EGLDisplay::new(gbm.clone()).map_err(DeviceAddError::AddNode)? };
            let egl_device =
                EGLDevice::device_for_display(&display).map_err(DeviceAddError::AddNode)?;

            if egl_device.is_software() {
                if std::env::var("YAOR_SOFTWARE_OK").is_err() {
                    return Err(DeviceAddError::NoRenderNode);
                }
                warn!("rendu logiciel accepté (YAOR_SOFTWARE_OK) — pas de GPU matériel");
            }

            // Le rendu logiciel expose rarement un render node ; on
            // retombe sur le nœud render du device, puis sur le nœud
            // primaire (même repli qu'anvil).
            let render_node = egl_device
                .try_get_render_node()
                .ok()
                .flatten()
                .or_else(|| node.node_with_type(NodeType::Render).and_then(|x| x.ok()))
                .unwrap_or(node);
            self.backend_data
                .as_mut()
                .unwrap()
                .gpus
                .as_mut()
                .add_node(render_node, gbm.clone())
                .map_err(DeviceAddError::AddNode)?;

            Ok(render_node)
        };

        let render_node = try_initialize_gpu()
            .inspect_err(|err| warn!(?err, "échec d'init GPU"))
            .ok();

        let primary_gpu = self.backend_data.as_ref().unwrap().primary_gpu;
        let allocator = render_node
            .is_some()
            .then(|| {
                GbmAllocator::new(
                    gbm.clone(),
                    GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
                )
            })
            .or_else(|| {
                self.backend_data
                    .as_ref()
                    .unwrap()
                    .backends
                    .get(&primary_gpu)
                    .or_else(|| {
                        self.backend_data
                            .as_ref()
                            .unwrap()
                            .backends
                            .values()
                            .find(|backend| backend.render_node == Some(primary_gpu))
                    })
                    .map(|backend| backend.drm_output_manager.allocator().clone())
            })
            .ok_or(DeviceAddError::PrimaryGpuMissing)?;

        let framebuffer_exporter = GbmFramebufferExporter::new(gbm.clone(), render_node);

        let color_formats = SUPPORTED_FORMATS_8BIT_ONLY;
        let mut renderer = self
            .backend_data
            .as_mut()
            .unwrap()
            .gpus
            .single_renderer(&render_node.unwrap_or(primary_gpu))
            .unwrap();
        let render_formats = renderer
            .as_mut()
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .filter(|format| render_node.is_some() || format.modifier == Modifier::Linear)
            .copied()
            .collect::<FormatSet>();

        let drm_output_manager = DrmOutputManager::new(
            drm,
            allocator,
            framebuffer_exporter,
            Some(gbm),
            color_formats.iter().copied(),
            render_formats,
        );

        self.backend_data.as_mut().unwrap().backends.insert(
            node,
            BackendData {
                registration_token,
                drm_output_manager,
                drm_scanner: DrmScanner::new(),
                non_desktop_connectors: Vec::new(),
                render_node,
                surfaces: HashMap::new(),
            },
        );

        self.device_changed(node);

        Ok(())
    }

    fn connector_connected(
        &mut self,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        let backend_data = self.backend_data.as_mut().unwrap();
        let primary_gpu = backend_data.primary_gpu;
        let clock = &backend_data.clock;
        // Emprunts de champs disjoints : le renderer (gpus) et le
        // périphérique (backends) coexistent comme dans anvil.
        let gpus = &mut backend_data.gpus;
        let device = match backend_data.backends.get_mut(&node) {
            Some(device) => device,
            None => return,
        };

        let render_node = device.render_node.unwrap_or(primary_gpu);
        let mut renderer = gpus.single_renderer(&render_node).unwrap();

        let output_name =
            format!("{}-{}", connector.interface().as_str(), connector.interface_id());
        info!(?crtc, "configuration du connecteur {}", output_name);

        let drm_device = device.drm_output_manager.device();

        let non_desktop = drm_device
            .get_properties(connector.handle())
            .ok()
            .and_then(|props| {
                let (info, value) = props
                    .into_iter()
                    .filter_map(|(handle, value)| {
                        let info = drm_device.get_property(handle).ok()?;
                        Some((info, value))
                    })
                    .find(|(info, _)| info.name().to_str() == Ok("non-desktop"))?;

                info.value_type().convert_value(value).as_boolean()
            })
            .unwrap_or(false);

        if non_desktop {
            info!("connecteur {} non-bureau, ignoré (pas de leasing en v0.4)", output_name);
            device.non_desktop_connectors.push((connector.handle(), crtc));
            return;
        }

        let display_info =
            smithay_drm_extras::display_info::for_connector(drm_device, connector.handle());
        let make = display_info
            .as_ref()
            .and_then(|info| info.make())
            .unwrap_or_else(|| "Unknown".into());
        let model = display_info
            .as_ref()
            .and_then(|info| info.model())
            .unwrap_or_else(|| "Unknown".into());

        let mode_id = connector
            .modes()
            .iter()
            .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(0);

        let drm_mode = connector.modes()[mode_id];
        let wl_mode = WlMode::from(drm_mode);

        let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            output_name,
            PhysicalProperties {
                size: (phys_w as i32, phys_h as i32).into(),
                subpixel: connector.subpixel().into(),
                make,
                model,
            },
        );
        let global = output.create_global::<Yaor>(&self.display_handle);

        // Position en cascade à droite de la dernière sortie.
        let x = self
            .space
            .outputs()
            .fold(0, |acc, o| acc + self.space.output_geometry(o).unwrap().size.w);
        let position = (x, 0).into();

        output.set_preferred(wl_mode);
        output.change_current_state(Some(wl_mode), None, None, Some(position));
        self.space.map_output(&output, position);

        output
            .user_data()
            .insert_if_missing(|| UdevOutputId { crtc, device_id: node });

        let planes = match drm_device.planes(&crtc) {
            Ok(planes) => planes,
            Err(err) => {
                warn!("échec d'énumération des plans du crtc : {}", err);
                return;
            }
        };

        let drm_output = match device
            .drm_output_manager
            .initialize_output::<_, crate::render::UdevOutputElements<'_>>(
                crtc,
                drm_mode,
                &[connector.handle()],
                &output,
                Some(planes),
                &mut renderer,
                &DrmOutputRenderElements::default(),
            ) {
            Ok(drm_output) => drm_output,
            Err(err) => {
                warn!("échec d'initialisation de la sortie drm : {}", err);
                return;
            }
        };

        let render_node_for_feedback = device.render_node;
        let dmabuf_feedback = drm_output.with_compositor(|compositor| {
            get_surface_dmabuf_feedback(
                primary_gpu,
                render_node_for_feedback,
                node,
                gpus,
                compositor.surface(),
            )
        });

        let surface = SurfaceData {
            dh: self.display_handle.clone(),
            device_id: node,
            render_node: device.render_node,
            global: Some(global),
            drm_output,
            dmabuf_feedback,
            last_presentation_time: None,
            vblank_throttle_timer: None,
        };

        device.surfaces.insert(crtc, surface);

        // Kick-off du rendu.
        let now = clock.now();
        self.handle.insert_idle(move |state| {
            state.render_surface(node, crtc, now);
        });
    }

    fn connector_disconnected(
        &mut self,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        let backend_data = self.backend_data.as_mut().unwrap();
        let primary_gpu = backend_data.primary_gpu;
        let gpus = &mut backend_data.gpus;
        let device = match backend_data.backends.get_mut(&node) {
            Some(device) => device,
            None => return,
        };

        if let Some(pos) = device
            .non_desktop_connectors
            .iter()
            .position(|(handle, _)| *handle == connector.handle())
        {
            let _ = device.non_desktop_connectors.remove(pos);
            return;
        }

        device.surfaces.remove(&crtc);

        let output = self
            .space
            .outputs()
            .find(|o| {
                o.user_data()
                    .get::<UdevOutputId>()
                    .map(|id| id.device_id == node && id.crtc == crtc)
                    .unwrap_or(false)
            })
            .cloned();

        if let Some(output) = output {
            self.space.unmap_output(&output);
        }

        let render_node = device.render_node.unwrap_or(primary_gpu);
        let mut renderer = gpus.single_renderer(&render_node).unwrap();
        // remarque anvil : écran noir « simule » un modeset ci-dessous
        let _ = device.drm_output_manager.try_to_restore_modifiers::<_, crate::render::UdevOutputElements<'_>>(
            &mut renderer,
            &DrmOutputRenderElements::default(),
        );
    }

    fn device_changed(&mut self, node: DrmNode) {
        let scan_result = match self.backend_data.as_mut().unwrap().backends.get_mut(&node) {
            None => return,
            Some(device) => match device
                .drm_scanner
                .scan_connectors(device.drm_output_manager.device())
            {
                Ok(res) => res,
                Err(err) => {
                    warn!(?err, "échec de scan des connecteurs");
                    return;
                }
            },
        };

        for event in scan_result {
            match event {
                DrmScanEvent::Connected { connector, crtc: Some(crtc) } => {
                    self.connector_connected(node, connector, crtc);
                }
                DrmScanEvent::Disconnected { connector, crtc: Some(crtc) } => {
                    self.connector_disconnected(node, connector, crtc);
                }
                _ => {}
            }
        }

        // Repositionnement des fenêtres dans les sorties (policy).
        crate::policy::fixup_positions(self);
    }

    fn device_removed(&mut self, node: DrmNode) {
        let crtcs = match self.backend_data.as_mut().unwrap().backends.get_mut(&node) {
            Some(device) => device
                .drm_scanner
                .crtcs()
                .map(|(info, crtc)| (info.clone(), crtc))
                .collect::<Vec<_>>(),
            None => return,
        };

        for (connector, crtc) in crtcs {
            self.connector_disconnected(node, connector, crtc);
        }

        debug!("surfaces abandonnées");

        if let Some(mut backend_data) = self.backend_data.as_mut().unwrap().backends.remove(&node) {
            let registration_token = backend_data.registration_token;
            if let Some(render_node) = backend_data.render_node {
                self.backend_data
                    .as_mut()
                    .unwrap()
                    .gpus
                    .as_mut()
                    .remove_node(&render_node);
                backend_data.render_node = None;
            }
            self.handle.remove(registration_token);
        }

        crate::policy::fixup_positions(self);
    }

    /// Vblank reçu : clôture la frame pilotée par la surface, et
    /// planifie le rendu suivant (anvil frame_finish).
    fn frame_finish(
        &mut self,
        dev_id: DrmNode,
        crtc: crtc::Handle,
        metadata: &mut Option<DrmEventMetadata>,
    ) {
        let backend_data = self.backend_data.as_mut().unwrap();
        let clock_ref = &backend_data.clock;
        let primary_gpu = backend_data.primary_gpu;
        let device_backend = match backend_data.backends.get_mut(&dev_id) {
            Some(backend) => backend,
            None => {
                error!("frame sur backend absent {}", dev_id);
                return;
            }
        };

        let surface = match device_backend.surfaces.get_mut(&crtc) {
            Some(surface) => surface,
            None => {
                error!("frame sur crtc absent {:?}", crtc);
                return;
            }
        };

        if let Some(timer_token) = surface.vblank_throttle_timer.take() {
            self.handle.remove(timer_token);
        }

        let output = match self.space.outputs().find(|o| {
            o.user_data().get::<UdevOutputId>()
                == Some(&UdevOutputId {
                    device_id: surface.device_id,
                    crtc,
                })
        }) {
            Some(output) => output.clone(),
            None => return,
        };

        let frame_duration = match output.current_mode() {
            Some(mode) => Duration::from_secs_f64(1_000f64 / mode.refresh as f64),
            None => return,
        };

        let tp = metadata.as_ref().and_then(|metadata| match metadata.time {
            smithay::backend::drm::DrmEventTime::Monotonic(tp) => {
                tp.is_zero().not().then_some(tp)
            }
            smithay::backend::drm::DrmEventTime::Realtime(_) => None,
        });

        let seq = metadata.as_ref().map(|metadata| metadata.sequence).unwrap_or(0);

        let (clock, flags) = if let Some(tp) = tp {
            (
                tp.into(),
                wp_presentation_feedback::Kind::Vsync
                    | wp_presentation_feedback::Kind::HwClock
                    | wp_presentation_feedback::Kind::HwCompletion,
            )
        } else {
            (
                clock_ref.now(),
                wp_presentation_feedback::Kind::Vsync,
            )
        };

        let vblank_remaining_time = surface
            .last_presentation_time
            .map(|last_presentation_time| {
                frame_duration.saturating_sub(Time::elapsed(&last_presentation_time, clock))
            });

        if let Some(vblank_remaining_time) = vblank_remaining_time {
            if vblank_remaining_time > frame_duration / 2 {
                // anvil : affichage plus rapide qu'attendu, on amortit.
                static WARN_ONCE: Once = Once::new();
                WARN_ONCE.call_once(|| {
                    warn!("affichage plus rapide qu'attendu, vblanks amortis");
                });
                let throttled_time = tp
                    .map(|tp| tp.saturating_add(vblank_remaining_time))
                    .unwrap_or(Duration::ZERO);
                let throttled_metadata = DrmEventMetadata {
                    sequence: seq,
                    time: DrmEventTime::Monotonic(throttled_time),
                };
                let timer_token = self
                    .handle
                    .insert_source(
                        Timer::from_duration(vblank_remaining_time),
                        move |_, _, data| {
                            data.frame_finish(dev_id, crtc, &mut Some(throttled_metadata));
                            TimeoutAction::Drop
                        },
                    )
                    .expect("minuteur amorti vblank");
                surface.vblank_throttle_timer = Some(timer_token);
                return;
            }
        }
        surface.last_presentation_time = Some(clock);

        let submit_result = surface
            .drm_output
            .frame_submitted()
            .map_err(Into::<SwapBuffersError>::into);

        let schedule_render = match submit_result {
            Ok(user_data) => {
                if let Some(mut feedback) = user_data.flatten() {
                    feedback.presented(
                        clock,
                        Refresh::fixed(frame_duration),
                        seq as u64,
                        flags,
                    );
                }
                true
            }
            Err(err) => {
                warn!("erreur de rendu : {:?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => true,
                    SwapBuffersError::TemporaryFailure(err)
                        if matches!(
                            err.downcast_ref::<DrmError>(),
                            Some(&DrmError::DeviceInactive)
                        ) =>
                    {
                        // device suspendu ; la reprise de session
                        // re-planifiera (anvil)
                        false
                    }
                    SwapBuffersError::TemporaryFailure(err) => matches!(
                        err.downcast_ref::<DrmError>(),
                        Some(DrmError::Access(DrmAccessError { source, .. }))
                            if source.kind() == io::ErrorKind::PermissionDenied
                    ),
                    SwapBuffersError::ContextLost(err) => {
                        panic!("boucle de rendu perdue : {}", err)
                    }
                }
            }
        };

        if !schedule_render {
            return;
        }

        let next_frame_target = clock + frame_duration;

        // anvil : latence client limitée en scindant la durée inter-
        // vblank ; détail affine hors v0.4.
        let repaint_delay = Duration::from_secs_f64(frame_duration.as_secs_f64() * 0.6f64);

        let render_node = surface.render_node;
        let timer = if render_node
            .map(|render_node| render_node != primary_gpu)
            .unwrap_or(true)
        {
            Timer::immediate()
        } else {
            Timer::from_duration(repaint_delay)
        };

        self.handle
            .insert_source(timer, move |_, _, data| {
                data.render(dev_id, Some(crtc), next_frame_target);
                TimeoutAction::Drop
            })
            .expect("minuteur de rendu");
    }

    /// Rendu : d'un crtc précis ou de tous (anvil render).
    pub fn render(
        &mut self,
        node: DrmNode,
        crtc: Option<crtc::Handle>,
        frame_target: Time<Monotonic>,
    ) {
        let crtcs = match self.backend_data.as_mut().unwrap().backends.get_mut(&node) {
            Some(backend) => backend.surfaces.keys().copied().collect::<Vec<_>>(),
            None => {
                error!("rendu sur backend absent {}", node);
                return;
            }
        };

        match crtc {
            Some(crtc) => self.render_surface(node, crtc, frame_target),
            None => {
                for crtc in crtcs {
                    self.render_surface(node, crtc, frame_target);
                }
            }
        }
    }

    fn render_surface(&mut self, node: DrmNode, crtc: crtc::Handle, frame_target: Time<Monotonic>) {
        let output = match self.space.outputs().find(|o| {
            o.user_data().get::<UdevOutputId>() == Some(&UdevOutputId { device_id: node, crtc })
        }) {
            Some(output) => output.clone(),
            None => return,
        };

        self.pre_repaint(&output, frame_target);

        let pointer_location = self.pointer_location;

        let (result, dmabuf_feedback) = {
            let backend_data = self.backend_data.as_mut().unwrap();
            let device = match backend_data.backends.get_mut(&node) {
                Some(device) => device,
                None => return,
            };

            let surface = match device.surfaces.get_mut(&crtc) {
                Some(surface) => surface,
                None => return,
            };
            let primary_gpu = backend_data.primary_gpu;

            let start = Instant::now();

            let render_node = surface.render_node.unwrap_or(primary_gpu);
            let mut renderer = if primary_gpu == render_node {
                backend_data.gpus.single_renderer(&render_node)
            } else {
                let format = surface.drm_output.format();
                backend_data.gpus.renderer(&primary_gpu, &render_node, format)
            }
            .unwrap();

            let pointer_image = backend_data.pointer_image.clone();

            let result = render_surface_inner(
                surface,
                &mut renderer,
                &self.space,
                &output,
                pointer_location,
                &mut self.cursor_status,
                &mut backend_data.pointer_element,
                &pointer_image,
            );
            trace!(elapsed = ?start.elapsed(), "rendu de surface");

            (result, surface.dmabuf_feedback.clone())
        };

        let reschedule = match result {
            Ok((has_rendered, states)) => {
                crate::state::post_repaint(self, &output, frame_target, dmabuf_feedback, &states);
                !has_rendered
            }
            Err(err) => {
                warn!("erreur de rendu : {:#?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => false,
                    SwapBuffersError::TemporaryFailure(err) => {
                        match err.downcast_ref::<DrmError>() {
                            Some(DrmError::DeviceInactive) => true,
                            Some(DrmError::Access(DrmAccessError { source, .. })) => {
                                source.kind() == io::ErrorKind::PermissionDenied
                            }
                            _ => false,
                        }
                    }
                    SwapBuffersError::ContextLost(err) => {
                        match err.downcast_ref::<DrmError>() {
                            Some(DrmError::TestFailed(_)) => self
                                .backend_data
                                .as_mut()
                                .unwrap()
                                .backends
                                .get_mut(&node)
                                .map(|device| {
                                    device
                                        .drm_output_manager
                                        .device_mut()
                                        .reset_state()
                                        .expect("réinitialisation du device drm")
                                })
                                .is_some(),
                            _ => panic!("boucle de rendu perdue : {}", err),
                        }
                    }
                }
            }
        };

        if reschedule {
            let output_refresh = match output.current_mode() {
                Some(mode) => mode.refresh,
                None => return,
            };

            // anvil : re-planification à ~ une frame pour retester le
            // dommage.
            let next_frame_target =
                frame_target + Duration::from_millis(1_000_000 / output_refresh as u64);
            let reschedule_timeout = Duration::from(next_frame_target)
                .saturating_sub(self.backend_data.as_ref().unwrap().clock.now().into());
            let timer = Timer::from_duration(reschedule_timeout);
            self.handle
                .insert_source(
                    timer,
                    move |_, _, data| {
                        data.render(node, Some(crtc), next_frame_target);
                        TimeoutAction::Drop
                    },
                )
                .expect("minuteur de re-planification du rendu");
        }
    }

    fn pre_repaint(&mut self, output: &Output, frame_target: impl Into<Time<Monotonic>>) {
        // v0.4 : pas de commit-timer ni de fifo (allègemment du jalon) ;
        // la lane reste ouverte pour les étapes suivantes.
        let _ = (output, frame_target);
    }
}

/// Une frame de sortie : éléments curseur + fenêtres, puis rendu DRM
/// (adapté de anvil fn render_surface en free function).
#[allow(clippy::too_many_arguments)]
fn render_surface_inner(
    surface: &mut SurfaceData,
    renderer: &mut UdevRenderer<'_>,
    space: &crate::state::SpaceWindow,
    output: &Output,
    pointer_location: Point<f64, smithay::utils::Logical>,
    cursor_status: &mut CursorImageStatus,
    pointer_element: &mut PointerElement,
    pointer_image: &MemoryRenderBuffer,
) -> Result<(bool, RenderElementStates), SwapBuffersError> {
    let output_geometry = space.output_geometry(output).unwrap();
    let scale = Scale::from(output.current_scale().fractional_scale());

    let mut custom_elements: Vec<CustomRenderElements<_>> = Vec::new();

    if output_geometry.to_f64().contains(pointer_location) {
        let cursor_hotspot = if let CursorImageStatus::Surface(ref surface) = *cursor_status {
            compositor::with_states(surface, |states| {
                states
                    .data_map
                    .get::<Mutex<CursorImageAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .hotspot
            })
        } else {
            (0, 0).into()
        };
        let cursor_pos = pointer_location - output_geometry.loc.to_f64();

        // le curseur est défini une fois pour toute la session (image statique)
        pointer_element.set_buffer(pointer_image.clone());

        // rendu du curseur si pertinent
        {
            // reset if the cursor surface is no longer alive
            let mut reset = false;
            if let CursorImageStatus::Surface(ref surface) = *cursor_status {
                reset = !surface.alive();
            }
            if reset {
                *cursor_status = CursorImageStatus::default_named();
            }

            pointer_element.set_status(cursor_status.clone());
        }

        custom_elements.extend(
            pointer_element.render_elements::<CustomRenderElements<_>>(
                renderer,
                (cursor_pos - cursor_hotspot.to_f64())
                    .to_physical(scale)
                    .to_i32_round(),
                scale,
                1.0,
            ),
        );
    }

    let (elements, clear_color) = output_elements(output, space, custom_elements, renderer);

    let frame_mode = FrameFlags::DEFAULT;
    let (rendered, states) = surface
        .drm_output
        .render_frame(renderer, &elements, clear_color, frame_mode)
        .map(|render_frame_result| (!render_frame_result.is_empty, render_frame_result.states))
        .map_err(|err| match err {
            smithay::backend::drm::compositor::RenderFrameError::PrepareFrame(err) => {
                SwapBuffersError::from(err)
            }
            smithay::backend::drm::compositor::RenderFrameError::RenderFrame(
                OutputDamageTrackerError::Rendering(err),
            ) => SwapBuffersError::from(err),
            _ => unreachable!(),
        })?;

    crate::state::update_primary_scanout_output(space, output, cursor_status, &states);

    if rendered {
        let output_presentation_feedback =
            crate::state::take_presentation_feedback(output, space, &states);
        surface
            .drm_output
            .queue_frame(Some(output_presentation_feedback))
            .map_err(Into::<SwapBuffersError>::into)?;
    }

    Ok((rendered, states))
}
