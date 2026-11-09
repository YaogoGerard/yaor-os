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

use std::path::PathBuf;
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
        "{PROG}: usage — yaor-ctl workspace | yaor-ctl workspace N | yaor-ctl send N | \
         yaor-ctl profile set classic|macos|tiling"
    );
    std::process::exit(2);
}

// ---------------------------------------------------------------------------
// Sous-commande `profile set` (v0.6 Caméléon) : le dialog « Profils de
// bureau » du panel écrit `[profile]` via cette commande. Écriture du
// desktop.toml SONDE par sonde (pas de writer toml : la réécriture
// complète écraserait les commentaires utilisateur) puis SIGHUP au
// compositor — qui recharge sa configuration et relaie le SIGHUP à ses
// enfants launcher (panel, dock) ; les clients lancés hors launcher
// lisent le fichier à leur propre réveil.
// ---------------------------------------------------------------------------

/// Envoie SIGHUP à chaque processus dont /proc/<pid>/comm vaut
/// « yaor-compositor » et renvoie le nombre de réveils réussis
/// (aucun ps dans l'ISO — /proc scan, leçon QA probe-dialog).
fn hup_compositor() -> usize {
    let mut n = 0;
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return 0;
    };
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(path.join("comm")) else {
            continue;
        };
        if comm.trim_end() != "yaor-compositor" {
            continue;
        }
        let pid = name.parse::<i32>().unwrap_or(0);
        if pid > 0 {
            // SAFETY : kill(2) rc0/no-async — l'appelant est mono-tâche.
            if unsafe { libc::kill(pid, libc::SIGHUP) } == 0 {
                n += 1;
            }
        }
    }
    n
}

/// Remplace (ou insère) `name = "<nouveau>"` dans la section
/// `[profile]` du desktop.toml, section créée en fin de fichier si
/// absente. Renvoie le texte écrit.
fn profile_section_name(text: &str, new: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_profile = false;
    let mut profile_seen = false;
    let mut pending = false; // [profile] ouvert, name pas encore posé
    let mut name_done = false;
    for line in text.lines() {
        if line.starts_with('[') {
            if pending && !name_done {
                // fin de la section sans name : insérer avant la suivante
                out.push(format!("name = \"{new}\""));
                name_done = true;
            }
            in_profile = line == "[profile]";
            if in_profile {
                profile_seen = true;
                pending = true;
            } else {
                pending = false;
            }
        } else if in_profile && line.starts_with("name") && !name_done {
            out.push(format!("name = \"{new}\""));
            name_done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !name_done {
        if !profile_seen {
            out.push("[profile]".to_string());
        }
        out.push(format!("name = \"{new}\""));
    }
    let mut content = out.join("\n");
    content.push('\n');
    content
}

/// `yaor-ctl profile set <nom>` : premier candidat de configuration
/// existant (sinon le premier candidat, créé), écriture, puis SIGHUP
/// aux compositors connus. Ne parle PAS au wayland : fonctionne même
/// si le compositor est hors service (le bureau rattrapera à son
/// prochain démarrage). Sortie 0 si la configuration est relue
/// correctement, sinon 1.
fn profile_set(new: &str) -> ! {
    if !matches!(new, "classic" | "macos" | "tiling") {
        eprintln!("{PROG}: profil inconnu : {new} (classic | macos | tiling)");
        std::process::exit(2);
    }
    let candidats = yaor_config::candidate_paths();
    let path = candidats
        .iter()
        .find(|p| p.exists())
        .or_else(|| candidats.first())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("/root/.config/yaor/desktop.toml"));
    let ancien = std::fs::read_to_string(&path).unwrap_or_default();
    let content = profile_section_name(&ancien, new);
    if let Err(e) = std::fs::write(&path, &content) {
        eprintln!("{PROG}: écriture {} impossible : {e}", path.display());
        std::process::exit(1);
    }
    // Auto-contrôle : un desktop.toml casse-cul (indentation perdue,
    // guillemet manquant) ne doit jamais être publié — re-parse avant
    // de pousser le SIGHUP, sinon la relise viderait les défauts.
    match yaor_config::DesktopConfig::parse_str(&content) {
        Err(e) => {
            eprintln!("{PROG}: configuration invalide après écriture : {e}");
            std::fs::write(&path, &ancien).ok();
            std::process::exit(1);
        }
        Ok(cfg) => {
            // Le jeton QA attend le profil réel dans desktop.toml — pas
            // l'argument (garde contre une écriture partielle).
            let relu = cfg.profile.name.map(|p| p.as_str()).unwrap_or("aucun");
            if relu != new {
                eprintln!("{PROG}: profil relu « {relu} » ≠ demandé");
                std::fs::write(&path, &ancien).ok();
                std::process::exit(1);
            }
        }
    }
    let hups = hup_compositor();
    eprintln!(
        "{PROG}: profil : {new} pose ({}, hup {hups})",
        path.display()
    );
    std::process::exit(0);
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    // Sous-commande `profile set` (v0.6 Caméléon) : écriture du
    // desktop.toml + SIGHUP au compositor (sans wayland — utilise
    // yaor-config pour la même résolution de chemin que le bureau).
    if args[0] == "profile" {
        if args.len() == 3 && args[1] == "set" {
            profile_set(&args[2]);
        }
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
