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
}
