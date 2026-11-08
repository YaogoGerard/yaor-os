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
    /// Bascule vers l'espace de travail N (0-4 — maquette « 1-5 en
    /// barre », étape 3) : indice 0 = « Mod+1 ». Une entrée absente
    /// (liste plus courte) laisse l'espace sans raccourci. Sans objet
    /// hors maître+pile (actions no-op — v0.4/v0.5 inchangés).
    pub workspace: Vec<String>,
    /// Envoie la fenêtre focusée à l'espace N (0-4) et bascule avec
    /// elle (convention sway). Même convention d'indices que
    /// `workspace`.
    pub workspace_send: Vec<String>,
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
            workspace: ["Mod+1", "Mod+2", "Mod+3", "Mod+4", "Mod+5"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            workspace_send: ["Mod+Shift+1", "Mod+Shift+2", "Mod+Shift+3", "Mod+Shift+4", "Mod+Shift+5"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Profil d'interface (jalon v0.6 étape 3, REQ-SW-009/010).
///
/// Un profil est une **configuration de l'expérience de bureau**, pas un
/// système distinct (HLD desktop-architecture) : il choisit la
/// disposition des fenêtres et quelles surfaces de l'UI Engine sont
/// visibles. Le compositor n'en change pas — seul ce que pilotent la
/// politique de placement et les clients (panel, dock) change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// Windows-like : barre supérieure + menu + taskbar basse, fenêtres
    /// flottantes.
    Classic,
    /// macOS-like : barre supérieure + dock centré, fenêtres flottantes.
    Macos,
    /// Tiling : barre supérieure, maître + pile (v0.5), taskbar.
    Tiling,
}

impl Profile {
    /// Nom de sérialisation (`[profile] name`), journalisable.
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Classic => "classic",
            Profile::Macos => "macos",
            Profile::Tiling => "tiling",
        }
    }

    /// Disposition des fenêtres imposée par le profil.
    pub fn layout_mode(self) -> LayoutMode {
        match self {
            Profile::Classic | Profile::Macos => LayoutMode::Floating,
            Profile::Tiling => LayoutMode::MasterStack,
        }
    }

    /// Taskbar basse (`yaor-panel`, zone exclusive 24 px) visible ?
    pub fn wants_taskbar(self) -> bool {
        match self {
            Profile::Classic | Profile::Tiling => true,
            Profile::Macos => false,
        }
    }

    /// Dock flottant (`yaor-dock`) visible ?
    pub fn wants_dock(self) -> bool {
        self == Profile::Macos
    }
}

/// Section `[profile]` : profil élu. `name = None` (section absente)
/// conserve le comportement historique — `[layout] mode` seul décide,
/// aucune surface n'est masquée (QA v0.4/v0.5 inchangées).
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub name: Option<Profile>,
}

/// Configuration complète du bureau.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct DesktopConfig {
    pub profile: ProfileConfig,
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
    /// Mode de disposition effectif : le profil, quand il est élu,
    /// impose sa disposition ; sinon `[layout] mode` décide (défaut
    /// cascade — comportement v0.4 conservé).
    pub fn layout_mode(&self) -> LayoutMode {
        match self.profile.name {
            Some(p) => p.layout_mode(),
            None => self.layout.mode,
        }
    }

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

// ---------------------------------------------------------------------------
// Réveil par SIGHUP (clients du bureau)
// ---------------------------------------------------------------------------

/// Réveil des clients du bureau (panel, dock) au SIGHUP.
///
/// Le compositor recharge `desktop.toml` de son côté (v0.5) ; les
/// clients doivent en faire autant pour suivre le profil élu (étape 3 :
/// une taskbar masquée hors profil classique, un dock hors profil
/// macOS-like). Le gestionnaire de signal ne fait qu'un store atomique
/// et une écriture d'un octet dans un tube (`write` est
/// async-signal-safe) ; la boucle d'événements attend avec `poll` sur le
/// fd Wayland **et** sur le tube — le rechargement n'ajoute donc ni
/// minuterie ni scrutation de fichier.
pub mod hup {
    use std::os::fd::RawFd;
    use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

    static REQUESTED: AtomicBool = AtomicBool::new(false);
    static PIPE_W: AtomicI32 = AtomicI32::new(-1);

    extern "C" fn handler(_sig: libc::c_int) {
        REQUESTED.store(true, Ordering::SeqCst);
        let fd = PIPE_W.load(Ordering::SeqCst);
        if fd >= 0 {
            let octet = 1u8;
            // SAFETY : `write` est async-signal-safe et le tube ne bloque
            // pas (lecture vidée à chaque tour de boucle).
            unsafe {
                libc::write(fd, &octet as *const u8 as *const libc::c_void, 1);
            }
        }
    }

    /// Installe le gestionnaire SIGHUP et renvoie le fd de lecture du
    /// tube (extrémité à surveiller). `None` si le tube n'a pas pu être
    /// créé : l'appelant garde alors sa boucle bloquante d'origine.
    pub fn install() -> Option<RawFd> {
        let mut fds = [0 as RawFd; 2];
        // SAFETY : deux entiers pour pipe2 ; O_CLOEXEC évite la fuite du
        // tube vers les clients lancés ensuite.
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return None;
        }
        // Lecture non bloquante : `drain` rend toujours la main.
        // SAFETY : fcntl sur un fd qu'on vient d'ouvrir.
        unsafe {
            libc::fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK);
        }
        PIPE_W.store(fds[1], Ordering::SeqCst);
        // SAFETY : le gestionnaire ne fait qu'un store atomique et un write.
        unsafe { libc::signal(libc::SIGHUP, handler as *const () as libc::sighandler_t) };
        Some(fds[0])
    }

    /// Vrai si un SIGHUP est arrivé depuis le dernier appel.
    pub fn requested() -> bool {
        REQUESTED.swap(false, Ordering::SeqCst)
    }

    /// Vide le tube (une écriture par signal reçu).
    pub fn drain(tube: RawFd) {
        let mut buf = [0u8; 64];
        loop {
            // SAFETY : lecture dans un tampon local, fd valide et non
            // bloquant.
            let n = unsafe { libc::read(tube, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 || (n as usize) < buf.len() {
                break;
            }
        }
    }

    /// Attend un réveil : données Wayland (`wayland`) ou SIGHUP (`tube`).
    /// Renvoie `true` quand un SIGHUP est à traiter (tube vidé, drapeau
    /// remis à zéro). Un fd négatif est ignoré par `poll`.
    pub fn wait(wayland: RawFd, tube: RawFd) -> bool {
        let mut fds = [
            libc::pollfd {
                fd: wayland,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: tube,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        loop {
            // SAFETY : tableau de deux pollfd initialisés.
            let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
            if n < 0 {
                // Interruption par un autre signal : on réessaie.
                continue;
            }
            break;
        }
        let mut signale = false;
        if fds[1].revents != 0 {
            drain(tube);
            signale = true;
        }
        // Le `|` (et non `||`) garantit la remise à zéro du drapeau.
        signale | requested()
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
    fn espaces_defauts_et_rebind() {
        // Défauts : cinq espaces (maquette 1-5), bascule Mod+N,
        // envoi Mod+Shift+N (étape 3).
        let cfg = DesktopConfig::parse_str("").unwrap();
        assert_eq!(cfg.keys.workspace.len(), 5);
        assert_eq!(cfg.keys.workspace[1], "Mod+2");
        assert_eq!(cfg.keys.workspace_send[0], "Mod+Shift+1");
        // Rebind partiel : liste plus courte = espaces restants sans
        // raccourci ; l'autre tableau garde ses défauts (fichier
        // partiel, convention du crate).
        let cfg = DesktopConfig::parse_str("[keys]\nworkspace = [\"Mod+F1\"]\n").unwrap();
        assert_eq!(cfg.keys.workspace, vec!["Mod+F1".to_string()]);
        assert_eq!(cfg.keys.workspace_send[0], "Mod+Shift+1");
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
    fn profil_absent_conserve_le_comportement_historique() {
        let cfg = DesktopConfig::parse_str("[layout]\nmode = \"cascade\"\n").unwrap();
        assert_eq!(cfg.profile.name, None);
        // Aucun profil élu : `[layout] mode` décide seul (QA v0.4/v0.5).
        assert_eq!(cfg.layout_mode(), LayoutMode::Cascade);
        let vide = DesktopConfig::parse_str("").unwrap();
        assert_eq!(vide.layout_mode(), LayoutMode::Cascade);
    }

    #[test]
    fn profil_elit_disposition_et_surfaces() {
        let classic = DesktopConfig::parse_str("[profile]\nname = \"classic\"\n").unwrap();
        assert_eq!(classic.profile.name, Some(Profile::Classic));
        assert_eq!(classic.layout_mode(), LayoutMode::Floating);
        assert!(Profile::Classic.wants_taskbar());
        assert!(!Profile::Classic.wants_dock());

        let macos = DesktopConfig::parse_str("[profile]\nname = \"macos\"\n").unwrap();
        assert_eq!(macos.profile.name, Some(Profile::Macos));
        assert_eq!(macos.layout_mode(), LayoutMode::Floating);
        assert!(!Profile::Macos.wants_taskbar(), "pas de taskbar macOS-like");
        assert!(Profile::Macos.wants_dock());

        let tiling = DesktopConfig::parse_str("[profile]\nname = \"tiling\"\n").unwrap();
        assert_eq!(tiling.profile.name, Some(Profile::Tiling));
        assert_eq!(tiling.layout_mode(), LayoutMode::MasterStack);
        assert!(Profile::Tiling.wants_taskbar());
        assert!(!Profile::Tiling.wants_dock());
    }

    #[test]
    fn profil_prime_sur_le_mode_de_disposition() {
        // Profil élu : il impose sa disposition, même si `[layout] mode`
        // dit autre chose (le profil est le niveau le plus haut).
        let cfg = DesktopConfig::parse_str(
            "[profile]\nname = \"tiling\"\n\n[layout]\nmode = \"cascade\"\n",
        )
        .unwrap();
        assert_eq!(cfg.layout.mode, LayoutMode::Cascade, "valeur du fichier conservée");
        assert_eq!(cfg.layout_mode(), LayoutMode::MasterStack);
    }

    #[test]
    fn profil_inconnu_rejette() {
        assert!(DesktopConfig::parse_str("[profile]\nname = \"gnome\"\n").is_err());
    }

    #[test]
    fn sighup_reveille_la_boucle() {
        // Un seul test touche au signal process-wide (les tests du binaire
        // partagent le processus) : install, drain à vide, puis SIGHUP.
        let tube = hup::install().expect("tube de réveil");
        hup::drain(tube);
        assert!(!hup::requested(), "aucun signal reçu pour l'instant");
        // SAFETY : le gestionnaire installé ne fait qu'un store et un write.
        unsafe { libc::raise(libc::SIGHUP) };
        // fd wayland -1 : ignoré par poll ; le tube doit réveiller.
        assert!(hup::wait(-1, tube), "le SIGHUP doit réveiller la boucle");
        assert!(!hup::requested(), "drapeau remis à zéro après traitement");
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
