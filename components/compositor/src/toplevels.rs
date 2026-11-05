// Protocole wlr-foreign-toplevel-management (jalon v0.6 étape 1b.c) —
// canal compositor → panel : la taskbar basse (client layer-shell)
// reçoit la liste des fenêtres (title/app_id/états) et leur envoie
// activate / set_minimized / unset_minimized / close.
//
// Modèle smithay/src/wayland/foreign_toplevel_list/mod.rs (licence
// MIT © Smithay contributors) adapté au protocole wlr : Smithay 0.7
// n'implémente que la variante « list » (lecture seule) — la gestion
// (activate/minimize/close) est écrite ici avec le code généré de
// wayland-protocols-wlr (vendu dans l'arbre, ADR-018).
//
// Publication : chaque fenêtre mappée (Wayland ou X11) ET minimisée
// est publiée ; les événements ne partent QUE sur changement réel
// (sent_* mémorisé par handle) — publish_all peut donc être appelée
// à chaque commit sans boucle de repaint panel (le done d'un état
// inchangé ne re-rend pas le client).

use std::{
    sync::{Arc, Mutex},
};

use smithay::{
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_protocols_wlr::foreign_toplevel::v1::server::{
        zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
        zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
    },
    reexports::wayland_server::{
        backend::{ClientId, GlobalId},
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
    },
    utils::IsAlive,
};

use crate::{
    element::WindowElement,
    policy,
    state::Yaor,
};

/// Version du global annoncé : v1 suffit (activate/minimize/close y
/// sont — set_rectangle/fullscreen v2+ ignorés).
const VERSION: u32 = 1;

/// Handles d'un client manager (udata de la ressource manager).
type ManagerHandles = Arc<Mutex<Vec<ZwlrForeignToplevelHandleV1>>>;

/// État du protocole côté compositor : l'identifiant du global
/// (conservé vivant toute la session) et les clients managers liés
/// (le panel).
pub struct ForeignToplevelState {
    #[allow(dead_code)]
    pub global: GlobalId,
    pub managers: Vec<ZwlrForeignToplevelManagerV1>,
}

/// Données du global (aucun filtrage par client).
pub struct ManagerGlobalData;

/// Données d'un handle : la fenêtre publiée + le dernier état envoyé
/// (les événements ne partent que sur changement — voir en tête).
/// Mutex : les données de ressource wayland exigent Send + Sync.
pub struct HandleData {
    pub window: WindowElement,
    sent: Mutex<Sent>,
}

#[derive(Default, PartialEq)]
struct Sent {
    title: String,
    app_id: String,
    states: Vec<u8>,
}

impl ForeignToplevelState {
    pub fn new(dh: &DisplayHandle) -> Self {
        let global = dh
            .create_global::<Yaor, ZwlrForeignToplevelManagerV1, ManagerGlobalData>(
                VERSION,
                ManagerGlobalData,
            );
        ForeignToplevelState {
            global,
            managers: Vec::new(),
        }
    }
}

/// Fenêtres publiées : mappées (espace) + minimisées (hors espace),
/// dédoublonnées — l'ordre d'arrivée est celui de l'affichage panel.
fn toplevel_windows(state: &Yaor) -> Vec<WindowElement> {
    let mut out: Vec<WindowElement> = Vec::new();
    let push = |w: WindowElement, out: &mut Vec<WindowElement>| {
        if !out.iter().any(|e| policy::same_window(e, &w)) {
            out.push(w);
        }
    };
    for w in state.space.elements().cloned().collect::<Vec<_>>() {
        push(w, &mut out);
    }
    for (w, _, _) in state.layout.minimized.iter() {
        push(w.clone(), &mut out);
    }
    out
}

/// Fenêtre activée (focus clavier), pour l'état Activated des handles.
fn activated_window(state: &Yaor) -> Option<WindowElement> {
    let focus = state.seat.get_keyboard()?.current_focus()?;
    for w in state.space.elements().cloned().collect::<Vec<_>>() {
        let matched = if let crate::focus::FocusTarget::Wl(ws) = &focus {
            w.toplevel().map(|t| t.wl_surface() == ws).unwrap_or(false)
        } else if let crate::focus::FocusTarget::X11(xs) = &focus {
            w.x11_surface().map(|s| s == xs).unwrap_or(false)
        } else {
            false
        };
        if matched {
            return Some(w);
        }
    }
    None
}

/// Titre/app_id publiés (Wayland : attributs xdg ; X11 : WM_NAME/WM_CLASS).
fn identity(window: &WindowElement) -> (String, String) {
    if let Some(toplevel) = window.toplevel() {
        let surface = toplevel.wl_surface();
        smithay::wayland::compositor::with_states(&surface, |states| {
            let data = states
                .data_map
                .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                .expect("toplevel sans données xdg")
                .lock()
                .unwrap();
            (
                data.title.clone().unwrap_or_default(),
                data.app_id.clone().unwrap_or_default(),
            )
        })
    } else if let Some(surface) = window.x11_surface() {
        (surface.title(), surface.class())
    } else {
        (String::new(), String::new())
    }
}

/// Tableau d'états du protocole : couples (état, valeur) u32 —
/// maximized=0, minimized=1, activated=2.
fn states_bytes(window: &WindowElement, minimized: bool, activated: bool) -> Vec<u8> {
    let maximized = window
        .toplevel()
        .map(|t| {
            t.current_state()
                .states
                .contains(xdg_toplevel::State::Maximized)
        })
        .unwrap_or(false);
    let mut v = Vec::new();
    for (flag, val) in [(maximized, 0u32), (minimized, 1u32), (activated, 2u32)] {
        if flag {
            v.extend_from_slice(&val.to_ne_bytes());
            v.extend_from_slice(&0u32.to_ne_bytes());
        }
    }
    v
}

/// (Re)publie la liste des fenêtres : nouveaux handles, rafraîchissement
/// title/app_id/états (sur changement uniquement), closed des mortes.
pub fn publish_all(state: &mut Yaor) {
    let windows = toplevel_windows(state);
    let activated = activated_window(state);
    let dh = state.display_handle.clone();

    state.foreign_toplevel.managers.retain(|m| m.is_alive());
    for manager in state.foreign_toplevel.managers.clone() {
        let handles_cell = match manager.data::<ManagerHandles>() {
            Some(d) => d.clone(),
            None => continue,
        };
        let mut handles = handles_cell.lock().unwrap();
        // handles des fenêtres détruites : closed + oubli.
        handles.retain(|h| {
            let alive = h
                .data::<HandleData>()
                .map(|d| d.window.alive())
                .unwrap_or(false);
            if !alive {
                h.closed();
            }
            alive
        });
        for window in &windows {
            let known = handles
                .iter()
                .find(|h| {
                    h.data::<HandleData>()
                        .map(|d| policy::same_window(&d.window, window))
                        .unwrap_or(false)
                })
                .cloned();
            let handle = match known {
                Some(h) => h,
                None => {
                    let client = match manager.client() {
                        Some(c) => c,
                        None => continue,
                    };
                    let Ok(h) = client.create_resource::<ZwlrForeignToplevelHandleV1, _, Yaor>(
                        &dh,
                        manager.version(),
                        HandleData {
                            window: window.clone(),
                            sent: Mutex::new(Sent::default()),
                        },
                    ) else {
                        continue;
                    };
                    manager.toplevel(&h);
                    handles.push(h.clone());
                    h
                }
            };
            refresh_handle(state, &handle, window, &activated);
        }
    }
}

/// Rafraîchit un handle : n'envoie que les événements dont la valeur a
/// changé, terminés par done (contrat du protocole).
fn refresh_handle(
    state: &Yaor,
    handle: &ZwlrForeignToplevelHandleV1,
    window: &WindowElement,
    activated: &Option<WindowElement>,
) {
    let (title, app_id) = identity(window);
    let minimized = state
        .layout
        .minimized
        .iter()
        .any(|(w, _, _)| policy::same_window(w, window));
    let is_activated = activated
        .as_ref()
        .map(|w| policy::same_window(w, window))
        .unwrap_or(false);
    let states = states_bytes(window, minimized, is_activated);

    let data = match handle.data::<HandleData>() {
        Some(d) => d,
        None => return,
    };
    let mut sent = data.sent.lock().unwrap();
    let mut changed = false;
    if sent.title != title {
        handle.title(title.clone());
        sent.title = title;
        changed = true;
    }
    if sent.app_id != app_id {
        handle.app_id(app_id.clone());
        sent.app_id = app_id;
        changed = true;
    }
    if sent.states != states {
        handle.state(states.clone());
        sent.states = states;
        changed = true;
    }
    if changed {
        handle.done();
    }
}

// ---------------------------------------------------------------------------
// Actions (requêtes du panel) — jetons journal pour le QA.
// ---------------------------------------------------------------------------

/// Focus clavier + raise + décoration (Wayland et X11).
fn activate_now(state: &mut Yaor, window: &WindowElement) {
    state.space.raise_element(window, true);
    if let Some(surface) = window.x11_surface() {
        if let Some(xwm) = state.xwm.as_mut() {
            let _ = xwm.raise_window(surface);
        }
    }
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    if let Some(surface) = window.wl_surface() {
        let focus = crate::focus::FocusTarget::Wl(surface.into_owned());
        state
            .seat
            .get_keyboard()
            .unwrap()
            .set_focus(state, Some(focus), serial);
    } else if let Some(surface) = window.x11_surface() {
        let focus = crate::focus::FocusTarget::X11(surface.clone());
        state
            .seat
            .get_keyboard()
            .unwrap()
            .set_focus(state, Some(focus), serial);
    }
    crate::element::update_focused_decoration(state, Some(window));
}

fn close_window(_state: &mut Yaor, window: &WindowElement) {
    if let Some(toplevel) = window.toplevel() {
        toplevel.send_close();
    } else if let Some(surface) = window.x11_surface() {
        let _ = surface.close();
    }
}

fn maximized(window: &WindowElement) -> bool {
    window
        .toplevel()
        .map(|t| t.current_state().states.contains(xdg_toplevel::State::Maximized))
        .unwrap_or(false)
}

impl GlobalDispatch<ZwlrForeignToplevelManagerV1, ManagerGlobalData, Yaor> for Yaor {
    fn bind(
        state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrForeignToplevelManagerV1>,
        _global_data: &ManagerGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, Arc::new(Mutex::new(Vec::new())));
        // Publication initiale : les fenêtres déjà ouvertes (le panel
        // se lie typiquement au démarrage de session, avant les apps).
        let n = toplevel_windows(state).len();
        tracing::info!("taskbar : client lié ({} fenêtre(s) publiée(s))", n);
        state.foreign_toplevel.managers.push(manager);
        publish_all(state);
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ManagerHandles, Yaor> for Yaor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &ZwlrForeignToplevelManagerV1,
        request: <ZwlrForeignToplevelManagerV1 as Resource>::Request,
        _data: &ManagerHandles,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        // Stop : le client cesse de s'intéresser aux nouveaux toplevels
        // (non utilisé par YAOR — le panel vit toute la session).
        match request {
            zwlr_foreign_toplevel_manager_v1::Request::Stop => {}
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &ZwlrForeignToplevelManagerV1,
        _data: &ManagerHandles,
    ) {
        state
            .foreign_toplevel
            .managers
            .retain(|m| m.id() != resource.id());
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, HandleData, Yaor> for Yaor {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrForeignToplevelHandleV1,
        request: <ZwlrForeignToplevelHandleV1 as Resource>::Request,
        data: &HandleData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let window = data.window.clone();
        let mut mutated = false;
        match request {
            zwlr_foreign_toplevel_handle_v1::Request::Activate { seat: _ } => {
                // minimisée ? l'activation la restaure d'abord (sémantique
                // dock/taskbar attendue par le protocole). Seat unique en
                // v0.6 — la ressource wl_seat reçue n'est pas vérifiée.
                if let Some(idx) = state
                    .layout
                    .minimized
                    .iter()
                    .position(|(w, _, _)| policy::same_window(w, &window))
                {
                    tracing::info!("taskbar : restauration");
                    policy::restore_minimized(state, idx);
                } else {
                    tracing::info!("taskbar : activation");
                }
                activate_now(state, &window);
                mutated = true;
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetMinimized => {
                if state.space.element_geometry(&window).is_some() {
                    tracing::info!("taskbar : minimisation");
                    policy::minimize_window(state, &window);
                    mutated = true;
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMinimized => {
                if let Some(idx) = state
                    .layout
                    .minimized
                    .iter()
                    .position(|(w, _, _)| policy::same_window(w, &window))
                {
                    tracing::info!("taskbar : restauration");
                    policy::restore_minimized(state, idx);
                    mutated = true;
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::Close => {
                tracing::info!("taskbar : fermeture");
                close_window(state, &window);
                mutated = true;
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetMaximized => {
                if !maximized(&window) {
                    crate::ssd::toggle_maximized(state, &window);
                    mutated = true;
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::UnsetMaximized => {
                if maximized(&window) {
                    crate::ssd::toggle_maximized(state, &window);
                    mutated = true;
                }
            }
            zwlr_foreign_toplevel_handle_v1::Request::SetRectangle { .. }
            | zwlr_foreign_toplevel_handle_v1::Request::SetFullscreen { .. }
            | zwlr_foreign_toplevel_handle_v1::Request::UnsetFullscreen
            | zwlr_foreign_toplevel_handle_v1::Request::Destroy => {}
            _ => {}
        }
        if mutated {
            publish_all(state);
        }
    }
}
