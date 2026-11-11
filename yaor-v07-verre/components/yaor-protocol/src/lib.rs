// yaor-protocol — protocoles privés du bureau YAOR.
//
// Première entrée (jalon v0.6 étape 3) : yaor_workspace — canal
// espaces de travail entre le compositor et les clients de session
// (yaor-panel : indicateurs + clic ; yaor-ctl : commande en ligne).
//
// Schéma copié sur les crates wayland-protocols-* vendues (ADR-018) :
// code généré par wayland-scanner (déjà dans l'arbre Smithay, car
// wayland-client/serveur s'en servent au build) — aucune dépendance
// nouvelle, aucune résolution réseau (build hors-ligne, ADR-018).
//
// Consommateurs : compositor (serveur), yaor-panel et yaor-ctl
// (clients) — mêmes versions wayland-client/server que le reste du
// workspace (épinglées par le Cargo.lock de l'arbre Smithay).

//! Protocoles privés du bureau YAOR (générés depuis `yaor/*.xml` par
//! wayland-scanner — schéma des crates wayland-protocols-* vendues).

#![warn(missing_docs)]
#![forbid(unsafe_op_in_unsafe_fn)]

/// Espaces de travail (v0.6 étape 3) — interface `yaor_workspace_v1`.
pub mod workspace {
    //! Canal bi-directionnel compositor ↔ clients : espace actif
    //! (`current`), bascule (`switch`), envoi de la fenêtre focusée
    //! (`send_focused`). Voir `yaor/yaor-workspace-v1.xml`.

    pub use self::generated::client;

    #[cfg(feature = "server")]
    pub use self::generated::server;

    mod generated {
        #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
        #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
        #![allow(missing_docs, clippy::all)]

        pub mod client {
            //! API client du protocole.
            use wayland_client;
            use wayland_client::protocol::*;

            pub mod __interfaces {
                use wayland_client::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!("yaor/yaor-workspace-v1.xml");
            }
            use self::__interfaces::*;

            wayland_scanner::generate_client_code!("yaor/yaor-workspace-v1.xml");
        }

        #[cfg(feature = "server")]
        pub mod server {
            //! API serveur du protocole (compositor).
            use wayland_server;
            use wayland_server::protocol::*;

            pub mod __interfaces {
                use wayland_server::protocol::__interfaces::*;
                wayland_scanner::generate_interfaces!("yaor/yaor-workspace-v1.xml");
            }
            use self::__interfaces::*;

            wayland_scanner::generate_server_code!("yaor/yaor-workspace-v1.xml");
        }
    }
}
