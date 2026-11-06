//! Configuration déclarative du bureau YAOR (jalon v0.5, REQ-SW-006/015).
//!
//! `desktop.toml` décrit le comportement du bureau : mode de disposition
//! des fenêtres (cascade héritée du v0.4, maître+pile), géométrie du
//! tiling (ratio maître, espacement) et raccourcis clavier (chaînes du
//! type `Mod+Q`, Mod = Super). Tout fichier partiel est valable : les
//! champs manquants prennent la valeur par défaut (v0.4 pour le mode —
//! les QA v0.4 restent exactement elles-mêmes).
//!
//! Le compositor lit cette configuration au démarrage et au SIGHUP ;
//! les options routées sont journalisées (preuve QA « configuration :
//! … »). Aucun changement de format sans version : le fichier est
//! versionné par son existence (v0.5).

use std::path::PathBuf;

use serde::Deserialize;

/// Chemins candidats du fichier de configuration (par ordre de
/// priorité). `default_base_dir` : $XDG_CONFIG_HOME sinon ~/.config.
pub fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // priorité au banc QA : YAOR_DESKTOP_CONFIG
    if let Ok(p) = std::env::var("YAOR_DESKTOP_CONFIG") {
        paths.push(PathBuf::from(p));
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    paths.push(base.join("yaor").join("desktop.toml"));
    // valeur d'usine posée par le build (phase yaor-components)
    paths.push(PathBuf::from("/etc/yaor/desktop.toml"));
    paths
}

/// Mode de disposition des fenêtres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayoutMode {
    /// Cascade du v0.4 (décalage constant) — comportement historique.
    #[default]
    Cascade,
    /// Tiling maître + pile (jalon v0.5).
    MasterStack,
    /// Fenêtres flottantes (jalon v0.6 étape 1b — profil classique
    /// Windows-like) : positionnement libre, empilement, resize par
    /// les bords ; aucune arrangement automatique.
    Floating,
}

/// Géométrie du tiling maître + pile.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// Mode de disposition (cascade par défaut : QA v0.4 inchangées).
    pub mode: LayoutMode,
    /// Largeur relative du maître, 0.0..1.0 (par défaut 1/2).
    pub ratio: f64,
    /// Espacement entre tuiles et aux bords, en pixels logiques.
    pub gaps: u32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Cascade,
            ratio: 0.5,
            gaps: 0,
        }
    }
}

/// Raccourcis clavier par défaut (chaînes `Mod+…`, Mod = Super).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct KeysConfig {
    pub focus_next: String,
    pub focus_prev: String,
    pub swap_next: String,
    pub swap_prev: String,
    pub master_swap: String,
    pub close: String,
    pub ratio_grow: String,
    pub ratio_shrink: String,
    pub quit: String,
    /// Minimiser/restaurer la fenêtre focusée (jalon v0.6 étape 1b :
    /// bureau flottant — la taskbar le restaure ; ici, raccourci).
    pub minimize: String,
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            focus_next: "Mod+Right".to_string(),
            focus_prev: "Mod+Left".to_string(),
            swap_next: "Mod+Shift+Right".to_string(),
            swap_prev: "Mod+Shift+Left".to_string(),
            master_swap: "Mod+Tab".to_string(),
            close: "Mod+Q".to_string(),
            ratio_grow: "Mod+g".to_string(),
            ratio_shrink: "Mod+Shift+g".to_string(),
            quit: "Mod+Shift+E".to_string(),
            minimize: "Mod+m".to_string(),
        }
    }
}

/// Configuration complète du bureau.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct DesktopConfig {
    pub layout: LayoutConfig,
    pub keys: KeysConfig,
    pub wallpaper: WallpaperConfig,
    pub logo: LogoConfig,
    pub theme: ThemeConfig,
    pub dock: DockConfig,
}

/// Mise à l'échelle d'une image à l'écran (1e.fond).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageFit {
    /// Couvre toute la surface (recadre ce qui dépasse) — défaut.
    #[default]
    Cover,
    /// Entière, centrée, bords éventuels du fond visibles.
    Fit,
}

/// Fond d'écran (1e.fond) : image réelle affichée par le compositor.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct WallpaperConfig {
    /// Chemin de l'image (PNG/JPEG) ; vide = dégradé procédural
    /// (comportement 1d.fond, repli si le fichier est absent).
    pub path: String,
    /// Mise à l'échelle : « cover » (défaut) ou « fit ».
    pub fit: ImageFit,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            path: "/usr/share/yaor/fond_ecran.png".to_string(),
            fit: ImageFit::Cover,
        }
    }
}

/// Emblème du panel (1e.logo) : image réelle affichée dans la barre,
/// la pilule taskbar et l'entête du menu.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct LogoConfig {
    /// Chemin de l'image (PNG, carré de préférence) ; vide = bitmap
    /// incorporé (logo.rs 24x24, repli historique).
    pub path: String,
}

impl Default for LogoConfig {
    fn default() -> Self {
        Self {
            path: "/usr/share/yaor/logo.png".to_string(),
        }
    }
}

/// Dock du profil macOS-like (v0.6 étape 2) : barre flottante ancrée en
/// bas, centrée, peinte sur fond transparent (pas de zone exclusive —
/// les fenêtres gardent tout l'écran, comme le dock macOS).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct DockConfig {
    /// Hauteur de la barre, en pixels logiques (le dock la borne à
    /// 40..200 — une valeur aberrante ne casse pas le bureau).
    pub height: u32,
    /// Côté d'une tuile (emblème ou fenêtre), en pixels logiques
    /// (borné à 24..hauteur-16 pour garder une marge dans la barre).
    pub icon: u32,
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            height: 72,
            icon: 56,
        }
    }
}

/// Thème de l'interface (1e.theme) : « ambre sur noir » paramétrable.
/// Couleurs en chaînes « #RRGGBB » (ou « #RRGGBBAA ») — le repli est
/// la palette canonique (branding/README.md).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Accent actif (boutons, tuiles, liseré de fenêtre focusée).
    pub accent: String,
    /// Accent sombre (bordure du volet, contour de tuile).
    pub accent_dark: String,
    /// Fonds des panneaux (barre, volet, taskbar).
    pub panel_bg: String,
    /// Fond des boutons / tuiles sombres.
    pub button_bg: String,
    /// Texte clair principal.
    pub text_fg: String,
    /// Texte secondaire (gris).
    pub text_gray: String,
    /// Séparateurs.
    pub separator: String,
    /// Rayon des coins de fenêtre (SSD), en pixels.
    pub corner_radius: u32,
    /// Intensité de l'ombre des fenêtres (0.0 = pas d'ombre, 1.0 = max).
    pub shadow: f64,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            accent: "#E8862E".to_string(),
            accent_dark: "#B96A1F".to_string(),
            panel_bg: "#101014".to_string(),
            button_bg: "#26262E".to_string(),
            text_fg: "#F2E8DC".to_string(),
            text_gray: "#8A8A92".to_string(),
            separator: "#2A2A33".to_string(),
            corner_radius: 10,
            shadow: 0.55,
        }
    }
}

/// Analyse une couleur « #RRGGBB » ou « #RRGGBBAA » → 0xAARRGGBB
/// (None si le format est invalide — le consommateur appliquera son
/// repli de palette).
pub fn parse_color(s: &str) -> Option<u32> {
    let hex = s.strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some(0xFF00_0000 | v)
        }
        // « #RRGGBBAA » : l'ordre mémoire est AARRGGBB (convention des
        // buffers Argb8888 du panel et du compositor) — l'alpha se
        // déplace donc en tête, il n'est pas rendu tel quel.
        8 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some(((v & 0xFF) << 24) | (v >> 8))
        }
        _ => None,
    }
}

impl DesktopConfig {
    /// Parse une chaîne [desktop.toml] (utilisé en test et par le loader).
    pub fn parse_str(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    /// Charge la configuration : premier fichier candidat existant et
    /// lisible, sinon les défauts compilés. Renvoie aussi le chemin
    /// trouvé (journal du compositor — préuve QA source du fichier).
    pub fn load() -> (Self, Option<PathBuf>) {
        for path in candidate_paths() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                match Self::parse_str(&text) {
                    Ok(cfg) => return (cfg, Some(path)),
                    Err(err) => {
                        eprintln!("desktop.toml illisible ({}) : {err}", path.display());
                        // on tente les candidats suivants
                    }
                }
            }
        }
        (Self::default(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defauts_cascade_v0_4() {
        let cfg = DesktopConfig::parse_str("").unwrap();
        assert_eq!(cfg.layout.mode, LayoutMode::Cascade);
        assert_eq!(cfg.layout.ratio, 0.5);
        assert_eq!(cfg.layout.gaps, 0);
        assert_eq!(cfg.keys.close, "Mod+Q");
    }

    #[test]
    fn fichier_partiel_complète_par_defauts() {
        let cfg = DesktopConfig::parse_str("[layout]\nmode = \"master-stack\"\n").unwrap();
        assert_eq!(cfg.layout.mode, LayoutMode::MasterStack);
        assert_eq!(cfg.layout.ratio, 0.5);
        assert_eq!(cfg.keys.quit, "Mod+Shift+E");
    }

    #[test]
    fn fichier_complet_lit() {
        let cfg = DesktopConfig::parse_str(
            "[layout]\nmode = \"master-stack\"\nratio = 0.6\ngaps = 8\n\n\
             [keys]\nclose = \"Mod+X\"\n",
        )
        .unwrap();
        assert_eq!(cfg.layout.ratio, 0.6);
        assert_eq!(cfg.layout.gaps, 8);
        assert_eq!(cfg.keys.close, "Mod+X");
    }

    #[test]
    fn mode_inconnu_rejette() {
        assert!(DesktopConfig::parse_str("[layout]\nmode = \"warp\"\n").is_err());
    }

    #[test]
    fn couleurs_parsees() {
        assert_eq!(parse_color("#E8862E"), Some(0xFFE8862E));
        assert_eq!(parse_color("#e8862e"), Some(0xFFE8862E));
        assert_eq!(parse_color("#E8862E80"), Some(0x80E8862E));
        assert_eq!(parse_color("E8862E"), None);
        assert_eq!(parse_color("#XYZ"), None);
        assert_eq!(parse_color("#12345"), None);
    }

    #[test]
    fn theme_defaut_ambre_sur_noir() {
        let cfg = DesktopConfig::parse_str("").unwrap();
        assert_eq!(parse_color(&cfg.theme.accent), Some(0xFFE8862E));
        assert_eq!(cfg.theme.corner_radius, 10);
        assert!(cfg.theme.shadow > 0.0 && cfg.theme.shadow <= 1.0);
    }

    #[test]
    fn dock_defauts_profil_macos() {
        let cfg = DesktopConfig::parse_str("").unwrap();
        assert_eq!(cfg.dock.height, 72);
        assert_eq!(cfg.dock.icon, 56);
        // Section partielle : `icon` seul est admis, `height` garde son défaut.
        let partiel = DesktopConfig::parse_str("[dock]\nicon = 48\n").unwrap();
        assert_eq!(partiel.dock.icon, 48);
        assert_eq!(partiel.dock.height, 72);
    }

    #[test]
    fn wallpaper_et_logo_defauts_assets_rootfs() {
        let cfg = DesktopConfig::parse_str("").unwrap();
        assert_eq!(cfg.wallpaper.path, "/usr/share/yaor/fond_ecran.png");
        assert_eq!(cfg.wallpaper.fit, ImageFit::Cover);
        assert_eq!(cfg.logo.path, "/usr/share/yaor/logo.png");
    }

    #[test]
    fn sections_completes_lues() {
        let cfg = DesktopConfig::parse_str(
            "[wallpaper]\npath = \"/home/user/mur.png\"\nfit = \"fit\"\n\n\
             [logo]\npath = \"/home/user/logo.png\"\n\n\
             [theme]\naccent = \"#00AAFF\"\ncorner_radius = 4\nshadow = 0.0\n",
        )
        .unwrap();
        assert_eq!(cfg.wallpaper.path, "/home/user/mur.png");
        assert_eq!(cfg.wallpaper.fit, ImageFit::Fit);
        assert_eq!(cfg.logo.path, "/home/user/logo.png");
        assert_eq!(parse_color(&cfg.theme.accent), Some(0xFF00AAFF));
        assert_eq!(cfg.theme.corner_radius, 4);
        assert_eq!(cfg.theme.shadow, 0.0);
    }
}
