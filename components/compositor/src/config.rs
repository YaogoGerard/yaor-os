//! Configuration du compositor YAOR (jalon v0.5, REQ-SW-006/015).
//!
//! État global partagé, lecture au démarrage (crate `yaor-config` :
//! `desktop.toml` ou défauts compilés) et rafraîchissement au SIGHUP —
//! la boucle principale (cycle ≤16 ms) constate l'atomique
//! `RELOAD_REQUESTED` et re-tile toutes les fenêtres selon la nouvelle
//! configuration, **sans quitter la session** (TEST-DESK-001).
//!
//! Journal : la source du fichier et les options routées sortent
//! explicitement (preuve QA « configuration chargée : … »).

use std::sync::atomic::{AtomicBool, Ordering};

pub use yaor_config::{DesktopConfig, LayoutMode};

/// Demande de rechargement (SIGHUP → gestionnaire asynchrone-sûr).
pub static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn request_reload(_signal: libc::c_int) {
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
}

fn config_state() -> &'static std::sync::RwLock<DesktopConfig> {
    static CONFIG: std::sync::OnceLock<std::sync::RwLock<DesktopConfig>> =
        std::sync::OnceLock::new();
    CONFIG.get_or_init(|| std::sync::RwLock::new(DesktopConfig::default()))
}

/// Charge la configuration au démarrage et journalise sa provenance.
pub fn init() {
    let (cfg, path) = DesktopConfig::load();
    journal(&cfg, path.as_deref(), "configuration chargée");
    *config_state().write().unwrap() = cfg;
}

/// Installation du gestionnaire SIGHUP (rechargement, pas d'arrêt).
pub fn install_reload_handler() {
    // SAFETY : le gestionnaire ne fait qu'un store atomique (async-signal-safe).
    unsafe {
        libc::signal(libc::SIGHUP, request_reload as *const () as libc::sighandler_t);
    }
}

/// Recharge la configuration (SIGHUP constaté par la boucle) : renvoie la
/// configuration une fois l'état global actualisé.
pub fn reload() -> DesktopConfig {
    let (cfg, path) = DesktopConfig::load();
    journal(&cfg, path.as_deref(), "configuration rechargée (SIGHUP)");
    *config_state().write().unwrap() = cfg.clone();
    cfg
}

/// Configuration courante (copie légère pour la suite du code).
pub fn get() -> DesktopConfig {
    config_state().read().unwrap().clone()
}

/// Modifie la configuration en place (raccourci Mod+g sur le ratio).
pub fn update(f: impl FnOnce(&mut DesktopConfig)) {
    let mut cfg = config_state().write().unwrap();
    f(&mut cfg);
}

/// Trace la configuration (source + options routées) — preuve QA.
fn journal(cfg: &DesktopConfig, path: Option<&std::path::Path>, motif: &str) {
    match path {
        Some(p) => tracing::info!("{motif} : {}", p.display()),
        None => tracing::info!("{motif} : défauts compilés (aucun fichier)"),
    }
    let mode = match cfg.layout.mode {
        LayoutMode::Cascade => "cascade",
        LayoutMode::MasterStack => "master-stack",
        LayoutMode::Floating => "floating",
    };
    tracing::info!(
        "configuration : mode {mode}, ratio {:.3}, gaps {}, raccourcis close={} quit={} focus={}/{} minimize={}",
        cfg.layout.ratio,
        cfg.layout.gaps,
        cfg.keys.close,
        cfg.keys.quit,
        cfg.keys.focus_next,
        cfg.keys.focus_prev,
        cfg.keys.minimize,
    );
}
