//! `yaor-slot` — sélection du slot A/B et retour arrière avec Limine.
//!
//! `systemd-sysupdate` écrit les images dans les slots mais ne connaît pas le
//! chargeur d'amorçage ; `yaor-slot` fait le lien : il tient l'état des slots
//! (`/boot/yaor/slots.state`), réécrit les entrées de `limine.conf` et pilote
//! les changements de slot (ADR-015, REQ-SW-020).

use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use yaor_common::limine::Config;
use yaor_common::partitions;
use yaor_common::paths;
use yaor_common::state::SlotState;
use yaor_common::{Slot, DEFAULT_ATTEMPTS, ESP_MOUNT, ROOT_LABEL_PREFIX};

const USAGE: &str = "\
yaor-slot — sélection du slot A/B et retour arrière (YAOR-OS)

USAGE :
    yaor-slot <COMMANDE> [options]

COMMANDES :
    status                 affiche l'état des slots et l'entrée par défaut
    update                 met à jour (systemd-sysupdate) puis essaie le nouveau slot
    try <A|B>              essaie un slot précis (sans mise à jour)
    startup                décrémente le compteur d'essais (démarrage) et revient
                           en arrière s'il est épuisé
    ok                     confirme le slot en cours d'essai comme sain
    revert                 revient au slot confirmé (et redémarre)

OPTIONS :
    --esp <DIR>            point de montage de l'ESP (défaut : /boot)
    --attempts <N>         essais avant retour arrière (défaut : 3)
    --source <CHEMIN|URL>  force la source des transferts (systemd-sysupdate)
    --no-verify            désactive la vérification des signatures (tests)
    --dry-run              n'écrit rien ; montre les actions
    --no-reboot            pour « revert », ne redémarre pas
    -h, --help             affiche cette aide
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("yaor-slot : erreur : {e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    esp: PathBuf,
    attempts: u32,
    source: Option<String>,
    verify: bool,
    dry_run: bool,
    no_reboot: bool,
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print!("{USAGE}");
        return Ok(());
    }
    let command = args.remove(0);

    // Analyse des options communes et de l'éventuel argument positionnel.
    let mut opts = Opts {
        esp: PathBuf::from(ESP_MOUNT),
        attempts: DEFAULT_ATTEMPTS,
        source: None,
        verify: true,
        dry_run: false,
        no_reboot: false,
    };
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--esp" => opts.esp = PathBuf::from(take(&args, &mut i, "--esp")?),
            "--attempts" => {
                let v = take(&args, &mut i, "--attempts")?;
                opts.attempts = v
                    .parse()
                    .map_err(|_| format!("essais invalides : « {v} »"))?;
            }
            "--source" => opts.source = Some(take(&args, &mut i, "--source")?),
            "--no-verify" => {
                opts.verify = false;
                i += 1;
            }
            "--dry-run" => {
                opts.dry_run = true;
                i += 1;
            }
            "--no-reboot" => {
                opts.no_reboot = true;
                i += 1;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("option inconnue : « {other} »"));
            }
            other => {
                positional.push(other.to_string());
                i += 1;
            }
        }
    }

    match command.as_str() {
        "status" => status(&opts),
        "update" => update(&opts),
        "try" => {
            let s = positional
                .first()
                .ok_or("« try » exige un slot : A ou B")?;
            let slot = Slot::parse(s).ok_or_else(|| format!("slot invalide : « {s} »"))?;
            try_slot(&opts, slot)
        }
        "startup" => startup(&opts),
        "ok" => confirm(&opts),
        "revert" => revert(&opts),
        other => Err(format!("commande inconnue : « {other} » (voir --help)")),
    }
}

fn take(args: &[String], i: &mut usize, name: &str) -> Result<String, String> {
    if *i + 1 >= args.len() {
        return Err(format!("« {name} » attend une valeur"));
    }
    let v = args[*i + 1].clone();
    *i += 2;
    Ok(v)
}

fn load_state(esp: &std::path::Path) -> Result<SlotState, String> {
    SlotState::load(esp).map_err(|e| {
        format!(
            "état des slots illisible ({}) : {e}",
            paths::state_file(esp).display()
        )
    })
}

fn load_limine(esp: &std::path::Path) -> Result<Config, String> {
    let path = paths::limine_conf(esp);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("limine.conf illisible ({}) : {e}", path.display()))?;
    Ok(Config::parse(&text))
}

fn save_limine(esp: &std::path::Path, cfg: &Config, dry_run: bool) -> Result<(), String> {
    let path = paths::limine_conf(esp);
    if dry_run {
        eprintln!("[dry-run] écrirait {}", path.display());
        return Ok(());
    }
    std::fs::write(&path, cfg.render())
        .map_err(|e| format!("écriture de {} impossible : {e}", path.display()))
}

fn exec(program: &str, args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args.iter().map(OsStr::new));
    let status = cmd
        .status()
        .map_err(|e| format!("lancement de {program} impossible : {e}"))?;
    if !status.success() {
        return Err(format!("{program} a échoué ({status})"));
    }
    Ok(())
}

/// Affiche l'état courant.
fn status(opts: &Opts) -> Result<(), String> {
    let state = load_state(&opts.esp)?;
    let cfg = load_limine(&opts.esp).ok();
    println!("ESP            : {}", opts.esp.display());
    println!("slot actif     : {}", state.active);
    println!("slot confirmé  : {}", state.confirmed);
    match state.trying {
        Some(s) => println!("slot en essai  : {s} ({} essais restants)", state.attempts_left),
        None => println!("slot en essai  : aucun"),
    }
    for s in Slot::all() {
        let v = state.version_of(s);
        println!(
            "version {}      : {}",
            s,
            if v.is_empty() { "(vide)" } else { v }
        );
    }
    match cfg.as_ref().and_then(|c| c.default_entry()) {
        Some(n) => println!("entrée défaut  : {n}"),
        None => println!("entrée défaut  : (non déclarée)"),
    }
    Ok(())
}

/// `update` : lance systemd-sysupdate, identifie le slot écrit, l'inscrit dans
/// l'état et l'essaie.
fn update(opts: &Opts) -> Result<(), String> {
    let state = load_state(&opts.esp)?;

    let mut args: Vec<String> = vec!["update".to_string()];
    if !opts.verify {
        args.push("--verify=no".to_string());
    }
    if let Some(src) = &opts.source {
        args.push(format!("--transfer-source={src}"));
    }
    if opts.dry_run {
        args.push("--dry-run".to_string());
    }
    exec("systemd-sysupdate", &args)?;

    let present = partitions::list_root_versions_default();
    let (slot, version) = state.reconcile(&present).ok_or_else(|| {
        format!(
            "impossible d'identifier le slot écrit (versions présentes : {:?})",
            present
        )
    })?;
    eprintln!(
        "slot {slot} mis à jour vers la version « {version} » (essai en cours)"
    );
    record_and_promote_default(opts, &state, slot, Some(&version))
}

/// `try <slot>` : essaie un slot sans mise à jour.
fn try_slot(opts: &Opts, slot: Slot) -> Result<(), String> {
    let state = load_state(&opts.esp)?;
    record_and_promote_default(opts, &state, slot, None)
}

/// Écrit l'état (essai en cours) et pointe l'entrée Limine par défaut sur le
/// slot visé.
fn record_and_promote_default(
    opts: &Opts,
    state: &SlotState,
    slot: Slot,
    new_version: Option<&str>,
) -> Result<(), String> {
    let mut state = state.clone();
    if let Some(v) = new_version {
        state.set_version(slot, v);
    }
    let version = state.version_of(slot).to_string();
    if version.is_empty() {
        return Err(format!("le slot {slot} ne contient aucune version"));
    }
    state.trying = Some(slot);
    state.attempts_left = opts.attempts;

    let mut cfg = load_limine(&opts.esp)?;
    if !cfg.entry_has_cmdline(slot.index()) {
        return Err(format!(
            "limine.conf : l'entrée du slot {slot} n'a pas de ligne « cmdline: »"
        ));
    }
    cfg.set_entry_cmdline_root(slot.index(), &format!("{ROOT_LABEL_PREFIX}{version}"));
    cfg.set_default_entry(slot.index() + 1);

    if !opts.dry_run {
        state.save(&opts.esp).map_err(|e| format!("écriture de l'état impossible : {e}"))?;
    } else {
        eprintln!("[dry-run] état (essai) :\n{}", state.render());
    }
    save_limine(&opts.esp, &cfg, opts.dry_run)
}

/// `startup` : décrémente le compteur d'essais ; revenu à zéro, retour arrière.
fn startup(opts: &Opts) -> Result<(), String> {
    let mut state = load_state(&opts.esp)?;
    let Some(trying) = state.trying else {
        // Aucun essai en cours : rien à faire.
        return Ok(());
    };

    if state.attempts_left <= 1 {
        eprintln!("slot {trying} : essais épuisés, retour arrière vers {}", state.confirmed);
        return revert(opts);
    }

    state.attempts_left -= 1;
    if !opts.dry_run {
        state.save(&opts.esp).map_err(|e| format!("écriture de l'état impossible : {e}"))?;
    }
    eprintln!(
        "slot {trying} : démarrage ({} essais restants)",
        state.attempts_left
    );
    Ok(())
}

/// `ok` : confirme le slot en cours d'essai.
fn confirm(opts: &Opts) -> Result<(), String> {
    let mut state = load_state(&opts.esp)?;
    let Some(trying) = state.trying else {
        // Rien à confirmer : succès silencieux (idempotent).
        return Ok(());
    };
    state.confirmed = trying;
    state.active = trying;
    state.trying = None;
    state.attempts_left = 0;

    let mut cfg = load_limine(&opts.esp)?;
    cfg.set_default_entry(state.confirmed.index() + 1);

    if !opts.dry_run {
        state.save(&opts.esp).map_err(|e| format!("écriture de l'état impossible : {e}"))?;
    }
    save_limine(&opts.esp, &cfg, opts.dry_run)?;
    eprintln!("slot {trying} confirmé");
    Ok(())
}

/// `revert` : repointe le défilement sur le slot confirmé et redémarre.
fn revert(opts: &Opts) -> Result<(), String> {
    let mut state = load_state(&opts.esp)?;
    state.active = state.confirmed;
    state.trying = None;
    state.attempts_left = 0;

    let mut cfg = load_limine(&opts.esp)?;
    cfg.set_default_entry(state.confirmed.index() + 1);

    if !opts.dry_run {
        state.save(&opts.esp).map_err(|e| format!("écriture de l'état impossible : {e}"))?;
    }
    save_limine(&opts.esp, &cfg, opts.dry_run)?;
    eprintln!("retour arrière vers le slot {}", state.confirmed);

    if !opts.dry_run && !opts.no_reboot {
        let _ = exec("systemctl", &["reboot".to_string()]);
    }
    Ok(())
}
