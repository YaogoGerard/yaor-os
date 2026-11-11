// Protocole privé yaor_workspace_v1 (jalon v0.6 étape 3) — canal
// espaces de travail entre le compositor et les clients de session :
// yaor-panel (indicateurs 1-5 de la barre, cliquables — maquette profil
// tiling) et yaor-ctl (commande en ligne).
//
// Modèle : toplevels.rs (protocole wlr-foreign-toplevel, même structure
// GlobalDispatch/Dispatch) ; code généré par yaor-protocol (schéma des
// crates wayland-protocols-* vendues — ADR-018).
//
// Publication : `current` part au bind (état initial) puis à chaque
// changement réel d'espace (switch_workspace). Les requêtes des clients
// passent par run_action — mêmes gardes que les raccourcis clavier
// (hors bornes, espace courant, hors maître+pile : no-op).

use smithay::reexports::wayland_server::{
    backend::{ClientId, GlobalId},
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};

use yaor_protocol::workspace::server::yaor_workspace_v1::{
    self, YaorWorkspaceV1,
};

use crate::{policy, state::Yaor};

/// Version du global annoncé (l'interface en est à v1).
const VERSION: u32 = 1;

/// État du protocole côté compositor : l'identifiant du global
/// (conservé vivant toute la session) et les clients liés (panel,
/// yaor-ctl).
pub struct WorkspaceState {
    #[allow(dead_code)]
    pub global: GlobalId,
    pub clients: Vec<YaorWorkspaceV1>,
}

/// Données du global (aucun filtrage par client — surface de session de
/// confiance, mêmes droits que foreign-toplevel).
pub struct WorkspaceGlobalData;

/// Données d'une ressource client (aucune).
pub type WorkspaceData = ();

impl WorkspaceState {
    pub fn new(dh: &DisplayHandle) -> Self {
        let global =
            dh.create_global::<Yaor, YaorWorkspaceV1, WorkspaceGlobalData>(
                VERSION,
                WorkspaceGlobalData,
            );
        WorkspaceState {
            global,
            clients: Vec::new(),
        }
    }
}

/// Publie l'espace courant à tous les clients liés (bind + bascules).
pub fn publish_current(state: &mut Yaor) {
    let idx = state.layout.current_workspace as u32;
    for client in &state.workspace_state.clients {
        client.current(idx);
    }
}

impl GlobalDispatch<YaorWorkspaceV1, WorkspaceGlobalData, Yaor> for Yaor {
    fn bind(
        state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<YaorWorkspaceV1>,
        _global_data: &WorkspaceGlobalData,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let ws = data_init.init(resource, ());
        tracing::info!(
            "espaces : client lié (espace courant {})",
            state.layout.current_workspace + 1
        );
        state.workspace_state.clients.push(ws);
        // Publication initiale : l'état courant, même si le client se
        // lie en cours de session.
        publish_current(state);
    }
}

impl Dispatch<YaorWorkspaceV1, WorkspaceData, Yaor> for Yaor {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &YaorWorkspaceV1,
        request: <YaorWorkspaceV1 as Resource>::Request,
        _data: &WorkspaceData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            yaor_workspace_v1::Request::Switch { index } => {
                tracing::info!("espaces : demande client — bascule {}", index + 1);
                policy::run_action(state, policy::Action::Workspace(index as usize));
            }
            yaor_workspace_v1::Request::SendFocused { index } => {
                tracing::info!(
                    "espaces : demande client — envoi du focus à l'espace {}",
                    index + 1
                );
                policy::run_action(state, policy::Action::WorkspaceSend(index as usize));
            }
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &YaorWorkspaceV1,
        _data: &WorkspaceData,
    ) {
        state
            .workspace_state
            .clients
            .retain(|c| c.id() != resource.id());
    }
}
