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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub use yaor_config::{DesktopConfig, ImageFit, LayoutMode, Profile, WallpaperConfig};

/// Demande de rechargement (SIGHUP → gestionnaire asynchrone-sûr).
pub static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Version du thème (1e.theme) : incrémentée à chaque rechargement. Les
/// buffers de décoration (mémoire, SolidColorBuffer) portent leur clé
/// de cache sur cette valeur — un SIGHUP qui change `[theme]` les
/// reconstruit à l'image suivante, sans quitter la session.
static THEME_EPOCH: AtomicU64 = AtomicU64::new(1);

/// Version courante du thème (voir [`THEME_EPOCH`]).
pub fn theme_epoch() -> u64 {
    THEME_EPOCH.load(Ordering::Relaxed)
}

/// Palette et métriques résolues du thème « ambre sur noir » : couleurs
/// 0xAARRGGBB, rayon des coins, intensité d'ombre. Chaque couleur est
/// validée séparément — une chaîne invalide retombe sur la palette
/// canonique du branding (docs/05-ui-ux/branding/), pas sur du noir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub accent: u32,
    pub accent_dark: u32,
    pub panel_bg: u32,
    pub button_bg: u32,
    pub text_fg: u32,
    pub text_gray: u32,
    pub separator: u32,
    /// Rayon des coins de fenêtre, borné à [`Palette::RAYON_MIN`]..=
    /// [`Palette::RAYON_MAX`] (les coins sont des buffers carrés R x R).
    pub corner_radius: i32,
    /// Intensité de l'ombre des fenêtres (0.0 = aucune, 1.0 = max).
    pub shadow: f64,
}

impl Palette {
    pub const RAYON_MIN: i32 = 2;
    pub const RAYON_MAX: i32 = 24;

    fn color(s: &str, repli: u32) -> u32 {
        yaor_config::parse_color(s).unwrap_or(repli)
    }

    /// Composantes normalisées `[r, g, b, a]` (0..1) d'une couleur
    /// 0xAARRGGBB — format attendu par `SolidColorBuffer` de Smithay.
    pub fn rgba(c: u32) -> [f32; 4] {
        let ch = |shift: u32| ((c >> shift) & 0xFF) as f32 / 255.0;
        [ch(16), ch(8), ch(0), ch(24)]
    }

    /// Résout la section `[theme]` : bornes du rayon et de l'ombre,
    /// repli par couleur.
    pub fn from_config(t: &yaor_config::ThemeConfig) -> Palette {
        Palette {
            accent: Palette::color(&t.accent, 0xFFE8_862E),
            accent_dark: Palette::color(&t.accent_dark, 0xFFB9_6A1F),
            panel_bg: Palette::color(&t.panel_bg, 0xFF10_1014),
            button_bg: Palette::color(&t.button_bg, 0xFF26_262E),
            text_fg: Palette::color(&t.text_fg, 0xFFF2_E8DC),
            text_gray: Palette::color(&t.text_gray, 0xFF8A_8A92),
            separator: Palette::color(&t.separator, 0xFF2A_2A33),
            corner_radius: (t.corner_radius as i32).clamp(Palette::RAYON_MIN, Palette::RAYON_MAX),
            shadow: if t.shadow.is_finite() {
                t.shadow.clamp(0.0, 1.0)
            } else {
                0.55
            },
        }
    }
}

/// Palette courante (relue à chaque appel : le thème est petit et les
/// buffers la lisent à leur reconstruction, pas par image). Lecture
/// sous verrou **sans copie** de la configuration : appelée aussi par
/// les éléments à chaque image (rayon des coins).
pub fn palette() -> Palette {
    let guard = config_state().read().unwrap();
    Palette::from_config(&guard.theme)
}

/// Vrai si le thème courant est en style verre (lecture sans copie).
pub fn theme_glass() -> bool {
    config_state().read().unwrap().theme.is_glass()
}

/// Durée des animations de fenêtre (ms) ; 0 = désactivées.
pub fn anim_ms() -> u32 {
    config_state().read().unwrap().theme.anim_ms.min(600)
}

/// Paramètres du matériau verre du thème courant (None = plat).
pub fn glass_params() -> Option<yaor_ui::GlassParams> {
    yaor_ui::GlassParams::from_config(&config_state().read().unwrap().theme)
}

/// Section `[wallpaper]` courante — seule copie nécessaire à chaque
/// image (clé de cache du fond d'écran).
pub fn wallpaper() -> WallpaperConfig {
    let guard = config_state().read().unwrap();
    guard.wallpaper.clone()
}

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
    // Bascule de profil (étape 3) : elle se lit AVANT l'écriture, d'où
    // le journal dédié — le compositor applique la disposition du
    // nouveau profil, les clients suivent par leur propre rechargement.
    let avant = config_state().read().unwrap().profile.name;
    journal(&cfg, path.as_deref(), "configuration rechargée (SIGHUP)");
    if avant != cfg.profile.name {
        tracing::info!(
            "profil : {} -> {} (disposition {})",
            profile_label(avant),
            profile_label(cfg.profile.name),
            mode_label(cfg.layout_mode()),
        );
    }
    *config_state().write().unwrap() = cfg.clone();
    // Thème éventuellement nouveau : les buffers de décoration doivent
    // se reconstruire (clé de cache = theme_epoch()).
    THEME_EPOCH.fetch_add(1, Ordering::Relaxed);
    cfg
}

/// Configuration courante (copie légère pour la suite du code).
pub fn get() -> DesktopConfig {
    config_state().read().unwrap().clone()
}

/// Mode de disposition **effectif** : le profil élu, quand il y en a un,
/// impose sa disposition (étape 3) ; sinon `[layout] mode` décide
/// (comportement historique). Point unique de vérité pour la politique
/// de placement et les décorations serveur.
pub fn layout_mode() -> LayoutMode {
    config_state().read().unwrap().layout_mode()
}

/// Libellé d'un mode de disposition (journal).
fn mode_label(mode: LayoutMode) -> &'static str {
    match mode {
        LayoutMode::Cascade => "cascade",
        LayoutMode::MasterStack => "master-stack",
        LayoutMode::Floating => "floating",
    }
}

/// Libellé d'un profil (ou de son absence).
fn profile_label(profile: Option<Profile>) -> &'static str {
    match profile {
        Some(p) => p.as_str(),
        None => "aucun",
    }
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
    let mode = mode_label(cfg.layout_mode());
    tracing::info!(
        "profil : {} (disposition {mode})",
        profile_label(cfg.profile.name)
    );
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
    // Thème résolu (1e.theme) : valeurs effectivement utilisées par le
    // rendu, après repli et bornes — preuve QA du rechargement.
    let p = Palette::from_config(&cfg.theme);
    tracing::info!(
        "thème : accent #{:06X} sombre #{:06X} panel #{:06X} bouton #{:06X} texte #{:06X} gris #{:06X} sep #{:06X} rayon {} ombre {:.2}",
        p.accent & 0x00FF_FFFF,
        p.accent_dark & 0x00FF_FFFF,
        p.panel_bg & 0x00FF_FFFF,
        p.button_bg & 0x00FF_FFFF,
        p.text_fg & 0x00FF_FFFF,
        p.text_gray & 0x00FF_FFFF,
        p.separator & 0x00FF_FFFF,
        p.corner_radius,
        p.shadow,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_normalise_les_composantes() {
        assert_eq!(Palette::rgba(0xFFFF_FFFF), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(Palette::rgba(0x0000_0000), [0.0, 0.0, 0.0, 0.0]);
        let ambre = Palette::rgba(0xFFE8_862E);
        assert!((ambre[0] - 0.9098).abs() < 0.001);
        assert!((ambre[1] - 0.5255).abs() < 0.001);
        assert_eq!(ambre[3], 1.0, "alpha opaque");
    }

    #[test]
    fn profil_elit_impose_la_disposition_effective() {
        // Sans profil : `[layout] mode` décide (compatibilité v0.4/v0.5).
        update(|c| {
            c.profile.name = None;
            c.layout.mode = LayoutMode::Cascade;
        });
        assert_eq!(layout_mode(), LayoutMode::Cascade);

        // Profil tiling : maître+pile, quel que soit `[layout] mode`.
        update(|c| c.profile.name = Some(Profile::Tiling));
        assert_eq!(layout_mode(), LayoutMode::MasterStack);

        // Profils classiques : flottant.
        update(|c| c.profile.name = Some(Profile::Classic));
        assert_eq!(layout_mode(), LayoutMode::Floating);
        update(|c| c.profile.name = Some(Profile::Macos));
        assert_eq!(layout_mode(), LayoutMode::Floating);

        // Remise en état pour les autres tests du binaire.
        update(|c| c.profile.name = None);
    }

    #[test]
    fn palette_borne_rayon_et_ombre_et_replie_les_couleurs() {
        update(|c| {
            c.theme.corner_radius = 99;
            c.theme.shadow = 3.0;
            c.theme.accent = "pas-une-couleur".to_string();
        });
        let p = palette();
        assert_eq!(p.corner_radius, Palette::RAYON_MAX);
        assert_eq!(p.shadow, 1.0);
        // Chaîne invalide : repli canonique, jamais du noir.
        assert_eq!(p.accent, 0xFFE8_862E);

        update(|c| {
            c.theme.corner_radius = 0;
            c.theme.shadow = -1.0;
        });
        let p = palette();
        assert_eq!(p.corner_radius, Palette::RAYON_MIN);
        assert_eq!(p.shadow, 0.0);
    }
}
