// yaor-ctl — commande de contrôle du bureau YAOR (v0.6 étape 3).
//
// Client wayland minimal : bind yaor_workspace_v1 (protocole privé du
// bureau, crate yaor-protocol) et requêtes d'espaces de travail du
// profil tiling. Aucun rendu, aucun toolkit — « un besoin = un
// fournisseur » : wayland-client direct, comme le panel parle sctk.
//
// Le compositor publie l'espace actif (event `current`) au bind puis à
// chaque bascule ; les requêtes hors bornes ou hors tiling sont des
// no-op côté compositor (mêmes gardes que les raccourcis clavier).
// Journal : préfixe « yaor-ctl: » (jetons QA courts, grep-ables).

use wayland_client::{
    protocol::wl_registry, Connection, Dispatch, Proxy, QueueHandle,
};

use yaor_protocol::workspace::client::yaor_workspace_v1::{
    Event as WorkspaceEvent, YaorWorkspaceV1,
};

const PROG: &str = "yaor-ctl";

/// État du client : le proxy espaces + le dernier espace actif reçu.
struct Ctl {
    ws: Option<YaorWorkspaceV1>,
    current: Option<u32>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Ctl {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Bind du global yaor_workspace_v1 à l'annonce (version 1).
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == YaorWorkspaceV1::interface().name {
                let ws =
                    registry.bind::<YaorWorkspaceV1, (), Ctl>(name, version.min(1), qh, ());
                state.ws = Some(ws);
            }
        }
    }
}

impl Dispatch<YaorWorkspaceV1, ()> for Ctl {
    fn event(
        state: &mut Self,
        _proxy: &YaorWorkspaceV1,
        event: WorkspaceEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // `current` : publié au bind puis à chaque bascule.
        if let WorkspaceEvent::Current { index } = event {
            state.current = Some(index);
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "{PROG}: usage — yaor-ctl workspace | yaor-ctl workspace N | yaor-ctl send N  (N = 1..5)"
    );
    std::process::exit(2);
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    // Index d'espace demandé (base 0) : None = simple consultation.
    let (request, index): (Option<&str>, Option<u32>) = match args[0].as_str() {
        "workspace" if args.len() == 1 => (None, None),
        "workspace" if args.len() == 2 => (Some("switch"), parse_index(&args[1])),
        "send" if args.len() == 2 => (Some("send_focused"), parse_index(&args[1])),
        _ => usage(),
    };

    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("{PROG}: connexion au compositor impossible : {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut queue = conn.new_event_queue::<Ctl>();
    let qh = queue.handle();
    let mut state = Ctl {
        ws: None,
        current: None,
    };
    // Registre : les Global arrivent au roundtrip (bind inclus).
    conn.display().get_registry(&qh, ());
    if let Err(e) = queue.roundtrip(&mut state) {
        eprintln!("{PROG}: registre inaccessible : {e}");
        return std::process::ExitCode::FAILURE;
    }
    let Some(ws) = state.ws.clone() else {
        eprintln!("{PROG}: yaor_workspace_v1 absent (compositor ancien ou hors session)");
        return std::process::ExitCode::FAILURE;
    };

    match (request, index) {
        (None, _) => {
            // Consultation : le bind est parti au premier roundtrip,
            // l'état publié au bind revient au SECOND (aller-retour de
            // plus — sinon « aucun état reçu » à coup sûr).
            let _ = queue.roundtrip(&mut state);
            match state.current {
                Some(idx) => {
                    println!("{PROG}: espace actif : {}", idx + 1);
                    std::process::ExitCode::SUCCESS
                }
                None => {
                    eprintln!("{PROG}: aucun état reçu");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        (Some(req), Some(idx)) => {
            if req == "switch" {
                ws.switch(idx);
            } else {
                ws.send_focused(idx);
            }
            // Roundtrip : la requête part, l'éventuel `current` de
            // bascule revient (no-op côté compositor : rien ne revient,
            // l'état affiché reste celui du bind).
            let _ = queue.roundtrip(&mut state);
            match state.current {
                Some(cur) => {
                    println!("{PROG}: espace actif : {}", cur + 1);
                    std::process::ExitCode::SUCCESS
                }
                None => std::process::ExitCode::SUCCESS,
            }
        }
        (Some(_), None) => usage(),
    }
}

/// Parse « N » (1-5, base utilisateur) → indice base 0. Rejet silencieux
/// (None) : la requête partira avec un hors-borne et le compositor
/// l'ignorera — mais autant ne rien envoyer.
fn parse_index(s: &str) -> Option<u32> {
    s.parse::<u32>().ok().filter(|n| (1..=5).contains(n)).map(|n| n - 1)
}
