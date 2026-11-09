// yaor-panel — panneau du bureau YAOR (jalon v0.6 « Caméléon », étape 1).
//
// Premier client layer-shell du système : barre supérieure ancrée en haut
// (couche Top, zone exclusive de 24 px — le compositor soustrait la bande
// du tiling, cf. policy.rs). Bouton menu ouvrant un volet (couche Overlay,
// clavier exclusif pendant l'ouverture) peuplé des apps YAOR de démo
// (waytest, xwtest) — décision produit v0.6 : pas de scan .desktop.
//
// Redesign v0.6 (passation) : la barre suit le PROFIL — volet menu +
// taskbar dessinées d'après docs/05-ui-ux/design/ (Classique.png : pas de
// barre haute, menu bas-gauche, taskbar basse ; macOS-like.png : barre de
// menus + dialog « Profils de bureau » ; Tiling.png : espaces 1-5 + tray).
// Les libellés sont rendus par la police réelle yaor-ui::text (Noto Sans
// Regular/Bold embarquée — minuscules + accents), plus de bitmap 5x7.
//
// Modèle : examples/simple_layer.rs de smithay-client-toolkit 0.19.2
// (licence MIT © Smithay contributors). Rendu logiciel wl_shm (Argb8888).
// Journal : stderr préfixé « yaor-panel: » (jetons QA courts).
//
// 1e.logo : l'emblème affiché dans la barre est l'image de
// `desktop.toml` (`logo.path`, décodée par yaor-assets et réduite à
// 24 px) ; le bitmap incorporé de `logo.rs` ne sert plus que de repli
// (chemin vide, fichier absent, format illisible) — le panneau démarre
// toujours.
//
// Connexion : WAYLAND_DISPLAY sous XDG_RUNTIME_DIR (socket du compositor).

use std::os::fd::AsRawFd;
use std::process::Command;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{Event as ToplevelEvent, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{Event as ManagerEvent, ZwlrForeignToplevelManagerV1},
};
// Protocole privé yaor_workspace_v1 (étape 3) : indicateurs 1-5 de la
// barre — l'espace actif vient du compositor (event `current`), le clic
// sur un chiffre bascule (request `switch`).
use yaor_protocol::workspace::client::yaor_workspace_v1::{
    Event as WorkspaceEvent, YaorWorkspaceV1,
};
// Primitives de rendu partagées avec yaor-dock (étape 2) : canvas Argb8888,
// formes anti-aliasées et thème « ambre sur noir ». Le texte vient du
// moteur TrueType de yaor-ui::text (police réelle, minuscules + accents) —
// la bitmap 5x7 n'est plus consommée ici.
use yaor_ui::{
    blend, blit, fill_circle, fill_pill, fill_rect, fill_rrect, fill_ring, Theme,
};
use yaor_ui::text::{self, Weight};

mod logo;

const PROG: &str = "yaor-panel";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Uptime invité (s) — horodatage des journaux, corrélable avec le
/// compositor et les sondages du harnais QA (diagnostic v0.6-étape 1).
fn uptime() -> f64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse().ok()))
        .unwrap_or(0.0)
}

/// Hauteur de la barre (zone exclusive posée au compositor).
const BAR_H: u32 = 24;
/// Largeur du bouton menu (px logiques) — profil sans élection.
const BTN_W: u32 = 60;
/// Dimensions du volet menu (design Classique à l'échelle 1280 :
/// 450x528 à 1920 → 300x352) : ancré bas-gauche au-dessus de la
/// taskbar (marge 10 px au-dessus de la zone exclusive de la taskbar).
const MENU_W: u32 = 300;
const MENU_H: u32 = 352;
const MENU_GAP: i32 = 10;
/// Géométrie de la grille ÉPINGLÉES (coordonnées volet) : tuiles 50 px
/// arrondies, 4 colonnes (pas 62 — gouttière 12), 2 rangées (pas 78 :
/// tuile + libellé + espace).
const MENU_TILE: u32 = 50;
const MENU_TILE_STEP: u32 = 62;
const MENU_TILE_PITCH: u32 = 78;
const MENU_GRID_X: i32 = 32;
const MENU_GRID_Y: i32 = 120;
/// Tuiles épinglées (1d.menu + design Classique) : (libellé, monogramme,
/// style : 0 sombre, 1 ambre plein, 2 contour ambre). Les libellés sont
/// en minuscule accentuée comme le design (« Fichiers », « Éditeur »).
const MENU_PINS: &[(&str, &str, u8)] = &[
    ("Fichiers", "F", 1),
    ("Terminal", ">", 2),
    ("Web", "W", 0),
    ("Éditeur", "É", 1),
    ("Réglages", "R", 0),
    ("Profils", "D", 2),
    ("Musique", "M", 0),
    ("Calcul", "C", 0),
];

// Taskbar basse (jalon v0.6 étape 1d.tb — design Classique) : pilule
// emblème à gauche (OUVRE le volet menu — la barre haute est absente du
// profil classique), pilules à libellés, état à droite (« FR · 82 % » +
// horloge « 12:34 » sur « 29 oct. 2026 »).
const TASKBAR_H: u32 = 30;
/// X de départ des pilules fenêtres (après la pilule emblème + filet).
const TB_X0: i32 = 34;
const TB_EMBLEM_W: i32 = 26;
const TB_BTN_W: u32 = 120;
const TB_BTN_H: u32 = 18;
/// Pas d'un bouton à l'autre (120 + 6 px d'espacement).
const TB_BTN_STEP: u32 = TB_BTN_W + 6;

/// Dialog « Profils de bureau » (étape macOS-like — design, c'est l'UI
/// de changement de profil) : overlay centré du panel, trois cartes +
/// Annuler/Appliquer. « Appliquer » écrit `[profile]` via yaor-ctl.
const DLG_W: u32 = 720;
const DLG_H: u32 = 470;
/// Cartes : 216x252, pas 232, à partir de x=24, y=104.
const DLG_CARD_W: i32 = 216;
const DLG_CARD_H: i32 = 252;
const DLG_CARD_STEP: i32 = 232;
const DLG_CARD_X: i32 = 24;
const DLG_CARD_Y: i32 = 104;
/// Noms des profils dans l'ordre des cartes (design).
const DLG_PROFILES: [&str; 3] = ["classic", "macos", "tiling"];
const DLG_TITLES: [&str; 3] = ["Classique", "macOS-like", "Tiling"];
const DLG_SUBTITLES: [&str; 3] = [
    "Barre des tâches et menu",
    "Barre de menus et dock",
    "Fenêtres au clavier",
];

/// Apps de démo du menu : supprimées en 1d.menu — les 8 tuiles du
/// design sont visuelles (décision produit), le lancement sera câblé
/// pour les apps YAOR internes dans une étape ultérieure.

/// Thème courant, rechargé au SIGHUP (étape 3) : palette canonique tant
/// que [`init_assets`] n'a pas tourné.
static THEME: std::sync::RwLock<Theme> = std::sync::RwLock::new(Theme::CANONIQUE);

/// Thème courant (copie : `Theme` est `Copy`).
fn th() -> Theme {
    *THEME.read().expect("thème verrouillé")
}

/// Position du bouton menu : le logo 24x24 occupe x=2..26, le bouton
/// le suit (QA : clic au centre (60,12)).
const BTN_X: u32 = 30;

/// Indicateurs d'espaces de travail (étape 3) : pilule à droite du
/// bouton menu (x=100..178). Cinq chiffres, pas de 14 px (police 5x7),
/// 4 px de marge interne — cf. draw_bar et pointer_frame (clic).
const WS_X: u32 = BTN_X + BTN_W + 10;
const WS_COUNT: usize = 5;
const WS_PITCH: u32 = 14;

/// Emblème 24x24 en Argb8888 (B,G,R,A), chargé une seule fois au
/// démarrage (1e.logo). Vide tant que [`init_logo`] n'a pas tourné :
/// le bitmap incorporé de `logo.rs` prend alors le relais.
static LOGO: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Charge la configuration du bureau une fois au démarrage : thème
/// (`[theme]`, 1e.theme) puis emblème (`logo.path`, 1e.logo). Aucun
/// échec bloquant : chaque élément retombe sur sa valeur canonique.
/// Journalisé en jetons courts — preuve QA de la source utilisée.
/// Renvoie la configuration lue (profil élu — étape 3).
fn init_assets() -> yaor_config::DesktopConfig {
    let (cfg, src) = yaor_config::DesktopConfig::load();
    init_theme(&cfg, src.as_deref());
    init_logo(&cfg);
    cfg
}

/// Thème de l'interface (`desktop.toml` [theme], repli canonique couleur
/// par couleur) — réappliqué à chaque rechargement (SIGHUP).
fn init_theme(cfg: &yaor_config::DesktopConfig, src: Option<&std::path::Path>) {
    let theme = Theme::from_config(&cfg.theme);
    *THEME.write().expect("thème verrouillé") = theme;
    eprintln!(
        "{PROG}: [{:.3}] thème : accent #{:06X} panel #{:06X} source {}",
        uptime(),
        theme.accent & 0x00FF_FFFF,
        theme.panel_bg & 0x00FF_FFFF,
        match src {
            Some(p) => p.display().to_string(),
            None => "défauts".to_string(),
        }
    );
}

/// Emblème de la barre : `logo.path` de `desktop.toml` (yaor-assets,
/// ré-échantillonné à 24x24), sinon bitmap incorporé.
fn init_logo(cfg: &yaor_config::DesktopConfig) {
    if !cfg.logo.path.is_empty() {
        if let Some(img) = yaor_assets::load_image(std::path::Path::new(&cfg.logo.path)) {
            let px = yaor_assets::resize(&img, 24, 24).to_argb8888();
            eprintln!(
                "{PROG}: [{:.3}] emblème : {} {}x{} -> 24x24",
                uptime(),
                cfg.logo.path,
                img.w,
                img.h
            );
            let _ = LOGO.set(px);
            return;
        }
        eprintln!(
            "{PROG}: [{:.3}] emblème : {} illisible - bitmap incorporé",
            uptime(),
            cfg.logo.path
        );
        return;
    }
    eprintln!("{PROG}: emblème : bitmap incorporé - logo.path vide");
}

fn draw_logo(canvas: &mut [u8], w: i32, h: i32, x0: i32, y0: i32) {
    let src = LOGO.get().map(|v| v.as_slice()).unwrap_or(&logo::LOGO_24[..]);
    blit(canvas, w, h, x0, y0, src, 24, 24);
}

/// Mini-flamme ambre (silhouette du fond d'écran, réduite) : triangle
/// pointe en haut, bord adouci — dessinée dans la pilule emblème.
fn fill_flame(canvas: &mut [u8], w: i32, h: i32, cx: i32, y_tip: i32, base_w: i32, base_h: i32, color: u32) {
    let tip = (cx as f64, y_tip as f64);
    let bl = ((cx - base_w / 2) as f64, (y_tip + base_h) as f64);
    let br = ((cx + base_w / 2) as f64, (y_tip + base_h) as f64);
    let tri = [tip, bl, br];
    for y in y_tip..y_tip + base_h {
        for x in cx - base_w / 2..=cx + base_w / 2 {
            // dedans si du même côté que le 3e sommet pour chaque arête
            // (supersampling 2x2 pour le bord).
            let mut hit = 0;
            for sy in 0..2 {
                for sx in 0..2 {
                    let fx = x as f64 + 0.25 + 0.5 * sx as f64;
                    let fy = y as f64 + 0.25 + 0.5 * sy as f64;
                    let mut inside = true;
                    for k in 0..3 {
                        let (ax, ay) = tri[k];
                        let (bx, by) = tri[(k + 1) % 3];
                        let (cx2, cy2) = tri[(k + 2) % 3];
                        let sp = (by - ay) * (fx - ax) - (bx - ax) * (fy - ay);
                        let sc = (by - ay) * (cx2 - ax) - (bx - ax) * (cy2 - ay);
                        if sp * sc <= 0.0 {
                            inside = false;
                            break;
                        }
                    }
                    if inside {
                        hit += 1;
                    }
                }
            }
            if hit > 0 {
                let cov = hit as f64 / 4.0;
                blend(canvas, w, h, x, y, color, cov);
            }
        }
    }
}

/// Heure locale du panel : (année, mois, jour, h, min) — conversion
/// civile d'UNIX (algorithme de H. Hinnant). L'invité vise UTC (QEMU) ;
/// une vraie gestion du fuseau passera par timedatectl (étape ultérieure).
#[allow(dead_code)]
fn civil_now() -> (i64, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_unix(secs)
}

/// Conversion civile pure d'un instant UNIX (séparée de l'horloge pour
/// être testable) — algorithme de H. Hinnant `civil_from_days`.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    let hh = (sod / 3_600) as u32;
    let mm = ((sod % 3_600) / 60) as u32;
    // civil_from_days :
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, hh, mm)
}

/// Horloge en trois formats du design : (« HH:MM », « JJ mmm. AAAA »,
/// « jjj. JJ mmm. HH:MM ») — mois/jours abrégés français accentués.
fn clock_text() -> (String, String, String) {
    const MONTHS: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.",
        "nov.", "déc.",
    ];
    const DAYS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d, hh, mm) = civil_from_unix(secs);
    // 1970-01-01 = jeudi → (jours + 3) % 7 avec 0 = lundi.
    let dow = ((secs.div_euclid(86_400) + 3).rem_euclid(7)) as usize;
    (
        format!("{:02}:{:02}", hh, mm),
        format!("{} {}. {}", d, MONTHS[(m as usize - 1) % 12], y),
        format!("{} {} {}. {:02}:{:02}", DAYS[dow], d, MONTHS[(m as usize - 1) % 12], hh, mm),
    )
}

/// Horloge longue de la barre de menus (macOS-like / tiling).
fn right_clock() -> String {
    clock_text().2
}

/// Batterie affichée : /sys/class/power_supply (première BAT*) — repli
/// « 82 % », la valeur du design (les bancs QEMU n'ont pas de batterie).
fn battery_text() -> String {
    if let Ok(dir) = std::fs::read_dir("/sys/class/power_supply") {
        let mut caps: Vec<String> = Vec::new();
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("BAT") {
                let p = entry.path().join("capacity");
                if let Ok(v) = std::fs::read_to_string(&p) {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        caps.push(format!("{} %", n.min(100)));
                    }
                }
                break;
            }
        }
        if let Some(c) = caps.first() {
            return c.clone();
        }
    }
    "82 %".to_string()
}

/// Disposition clavier affichée (FR) — statique : la session QEMU garde
/// une disposition unique ; le suivi xkb layout viendra avec la config.
fn layout_text() -> String {
    "FR".to_string()
}

/// Charge CPU (%) : deltas de /proc/stat entre deux appels (état statique
/// « dernier échantillon » — le premier appel échantillonne à 100 ms).
fn cpu_percent() -> u32 {
    fn sample() -> Option<(u64, u64)> {
        let line = std::fs::read_to_string("/proc/stat").ok()?;
        let line = line.lines().next()?;
        let vals: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|v| v.parse().ok())
            .collect();
        if vals.len() < 4 {
            return None;
        }
        let total: u64 = vals.iter().sum();
        let idle = vals[3] + vals.get(4).copied().unwrap_or(0);
        Some((total, idle))
    }
    static PREV: std::sync::Mutex<Option<(u64, u64, u32)>> = std::sync::Mutex::new(None);
    let Some(cur) = sample() else {
        return 0;
    };
    let mut prev = PREV.lock().expect("échantillon CPU verrouillé");
    let pct = match *prev {
        Some((pt, pi, last)) if cur.0 > pt => {
            let dt = cur.0 - pt;
            let di = cur.1.saturating_sub(pi);
            if dt > 0 {
                (((dt - di) * 100) / dt) as u32
            } else {
                last
            }
        }
        Some((_, _, last)) => last,
        None => {
            // Premier appel : second échantillon rapproché (100 ms).
            std::thread::sleep(std::time::Duration::from_millis(100));
            match sample() {
                Some((t2, i2)) if t2 > cur.0 => {
                    let dt = t2 - cur.0;
                    let di = i2.saturating_sub(cur.1);
                    ((dt.saturating_sub(di) * 100) / dt.max(1)) as u32
                }
                _ => 0,
            }
        }
    };
    *prev = Some((cur.0, cur.1, pct));
    pct.min(100)
}

/// Charge mémoire (%) : /proc/meminfo (MemTotal - MemAvailable)/MemTotal.
#[allow(dead_code)]
fn ram_percent() -> u32 {
    let txt = match std::fs::read_to_string("/proc/meminfo") {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let get = |key: &str| -> Option<u64> {
        txt.lines().find_map(|l| {
            let l = l.strip_prefix(key)?;
            let v = l.trim_start().trim_end_matches(" kB").trim();
            v.parse().ok()
        })
    };
    match (get("MemTotal:"), get("MemAvailable:")) {
        (Some(total), Some(avail)) if total > 0 => (((total - avail) * 100) / total) as u32,
        _ => 0,
    }
}

/// Taskbar visible pour un profil donné : elle appartient aux profils
/// classique et tiling ; le profil macOS-like la remplace par le dock
/// (étape 3). Profil absent = comportement historique (taskbar).
fn taskbar_visible(profile: Option<yaor_config::Profile>) -> bool {
    profile.map_or(true, |p| p.wants_taskbar())
}

/// Libellé d'un bouton de taskbar : minuscules (police réelle), borné à
/// la largeur utile du bouton — un titre trop long est tronqué, jamais
/// débordé sur le bouton voisin (mesuré par le harnais QA).
fn taskbar_label(title: &str) -> String {
    let mut out = String::new();
    let budget = TB_BTN_W as i32 - 12;
    for ch in title.chars() {
        let next_len = text::text_width(&(out.clone() + &ch.to_string()), 11, Weight::Regular);
        if next_len > budget {
            break;
        }
        out.push(ch);
    }
    out
}

// ---------------------------------------------------------------------------
// État du panneau
// ---------------------------------------------------------------------------
struct Panel {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    compositor: CompositorState,
    layer_shell: LayerShell,

    /// Barre supérieure (couche Top, zone exclusive BAR_H).
    bar: LayerSurface,
    /// Barre affichée ? (design Classique : PAS de barre haute — le
    /// volet s'ouvre par la pilule emblème de la taskbar.)
    bar_visible: bool,
    /// Profil élu (rendu de la barre par variante + visibilités).
    profile: Option<yaor_config::Profile>,
    /// Volet menu ouvert (couche Overlay) — créé à l'ouverture, détruit
    /// à la fermeture (la destruction des objets wayland démappe la
    /// couche côté compositor, cf. shell.rs layer_destroyed).
    menu: Option<LayerSurface>,
    /// Dialog « Profils de bureau » (overlay, profil macOS-like) —
    /// carte sélectionnée 0..3, appliquée via yaor-ctl.
    dialog: Option<LayerSurface>,
    dialog_sel: usize,
    /// Taskbar basse (couche Top, ancrée bas, zone exclusive TASKBAR_H
    /// — jalon 1b.c).
    taskbar: Option<LayerSurface>,
    tb_width: u32,
    /// Taskbar affichée ? (étape 3 : masquée hors profils classique et
    /// tiling — la surface reste mappée, transparente et sans entrée).
    taskbar_visible: bool,
    /// Région d'entrée vide des états masqués (détruite au retour).
    empty_region: Option<Region>,

    /// Global zwlr_foreign_toplevel_manager_v1 (liste des fenêtres +
    /// activation/restauration — jalon 1b.c).
    foreign_manager: Option<ZwlrForeignToplevelManagerV1>,
    /// Global yaor_workspace_v1 (étape 3) : bascule au clic des
    /// indicateurs — absent (compositor ancien) : indicateurs visuels
    /// seulement.
    workspace: Option<YaorWorkspaceV1>,
    /// Espace de travail actif (base 0, publié par le compositor).
    workspace_current: usize,
    /// Indicateurs d'espaces affichés ? (profil tiling seul — étape 3).
    workspace_visible: bool,
    /// Fenêtres publiées par le compositor (ordre d'affichage).
    entries: Vec<TaskbarEntry>,
    /// Seat (l'activation d'une fenêtre exige la ressource wl_seat).
    seat_object: Option<wl_seat::WlSeat>,

    pool: SlotPool,
    pool_capacity: usize,
    width: u32,
    height: u32,
    configured: bool,
    exit: bool,

    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
}

/// Une fenêtre de la taskbar : handle foreign-toplevel + état rendu.
struct TaskbarEntry {
    handle: ZwlrForeignToplevelHandleV1,
    title: String,
    activated: bool,
    minimized: bool,
}

impl Panel {
    fn ensure_pool(&mut self, w: i32, h: i32) {
        let need = (w * h * 4) as usize;
        if self.pool_capacity < need {
            self.pool = SlotPool::new(need, &self.shm).expect("création du slot pool");
            self.pool_capacity = need;
        }
    }

    /// Rendu de la barre par PROFIL (passation v0.6) :
    /// - Classique : pas de barre (design Classique.png — la taskbar
    ///   porte l'ouverture du volet) : buffer transparent, zone nulle ;
    /// - macOS-like : barre de menus — emblème, menus de l'app active
    ///   (« Profils » ouvre le dialog), état à droite (FR, 82 %, date) ;
    /// - Tiling : espaces 1-5 + titre de la fenêtre focusée + tray
    ///   (layout maître+pile, CPU, batterie, heure) ;
    /// - profil non élu : barre historique (bouton YAOR + espaces +
    ///   version). Journal : jetons « barre : rendu » / « barre : masquée ».
    fn draw_bar(&mut self, _qh: &QueueHandle<Self>) {
        let w = self.width;
        let h = self.height;
        if w == 0 || h == 0 {
            return;
        }
        self.ensure_pool(w as i32, h as i32);
        let stride = w as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de barre");
        {
            let (cw, ch) = (w as i32, h as i32);
            if !self.bar_visible {
                // Classique (design) : aucune barre haute.
                canvas.fill(0);
                eprintln!("{PROG}: [{:.3}] barre : masquée (profil classique)", uptime());
            } else {
                fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, th().panel_bg);
                // Emblème à gauche (toutes les variantes).
                draw_logo(&mut canvas, cw, ch, 4, 0);
                match self.profile {
                    Some(yaor_config::Profile::Macos) => {
                        Self::draw_menubar(&mut canvas, cw, ch);
                    }
                    Some(yaor_config::Profile::Tiling) => {
                        Self::draw_tilingbar(&mut canvas, cw, ch, self.workspace_current, &self.entries);
                    }
                    _ => {
                        // Barre historique (profil non élu) : bouton menu
                        // + indicateurs d'espaces + version.
                        fill_rect(&mut canvas, cw, ch, BTN_X as i32, 4, BTN_W as i32, 16, th().accent);
                        let label = "YAOR";
                        let tw = text::text_width(label, 10, Weight::Bold);
                        text::draw_text(
                            &mut canvas,
                            cw,
                            ch,
                            label,
                            BTN_X as i32 + (BTN_W as i32 - tw) / 2,
                            7,
                            10,
                            Weight::Bold,
                            th().panel_bg,
                        );
                        if self.workspace_visible {
                            let px = WS_X as i32;
                            let pw = (WS_COUNT as i32) * WS_PITCH as i32 + 8;
                            fill_rrect(&mut canvas, cw, ch, px, 3, pw, 18, 4, th().button_bg);
                            for i in 0..WS_COUNT {
                                let active = i == self.workspace_current;
                                let digit = format!("{}", i + 1);
                                let dx = px + 4 + (i as i32) * WS_PITCH as i32;
                                text::draw_text(
                                    &mut canvas,
                                    cw,
                                    ch,
                                    &digit,
                                    dx,
                                    8,
                                    10,
                                    Weight::Regular,
                                    if active { th().accent } else { th().text_gray },
                                );
                                if active {
                                    fill_rect(&mut canvas, cw, ch, dx - 1, 17, 6, 2, th().accent);
                                }
                            }
                        }
                        let vers = format!("YAOR {VERSION}");
                        let tw = text::text_width(&vers, 10, Weight::Regular);
                        text::draw_text(
                            &mut canvas,
                            cw,
                            ch,
                            &vers,
                            cw - tw - 6,
                            8,
                            10,
                            Weight::Regular,
                            th().text_fg,
                        );
                    }
                }
                eprintln!("{PROG}: [{:.3}] barre : rendu {}x{}", uptime(), w, h);
            }
        }
        self.bar
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(self.bar.wl_surface()).expect("attach barre");
        self.bar.commit();
    }

    /// Barre de menus du profil macOS-like (design macOS-like.png) :
    /// menus de l'app active à gauche, état à droite. Le menu « Profils »
    /// ouvre le dialog de changement de profil.
    fn draw_menubar(canvas: &mut [u8], cw: i32, ch: i32) {
        const MENUS: [&str; 6] = ["Profils", "Fichier", "Édition", "Affichage", "Fenêtre", "Aide"];
        let mut x = 34;
        for (i, m) in MENUS.iter().enumerate() {
            let color = if i == 0 { th().text_fg } else { th().text_gray };
            text::draw_text(canvas, cw, ch, m, x, 7, 11, Weight::Regular, color);
            x += text::text_width(m, 11, Weight::Regular) + 14;
        }
        // État à droite : FR, batterie, date complète.
        let status = format!("{}   {}   {}", layout_text(), battery_text(), right_clock());
        let tw = text::text_width(&status, 11, Weight::Regular);
        text::draw_text(canvas, cw, ch, &status, cw - tw - 8, 7, 11, Weight::Regular, th().text_fg);
    }

    /// Barre du profil tiling (design Tiling.png) : emblème, espaces
    /// 1-5 (actif = badge ambre), titre de la fenêtre focusée centré,
    /// tray à droite (« [ ] = maître+pile », CPU, batterie, heure).
    fn draw_tilingbar(
        canvas: &mut [u8],
        cw: i32,
        ch: i32,
        workspace_current: usize,
        entries: &[TaskbarEntry],
    ) {
        // Espaces 1-5 à droite de l'emblème.
        let px = WS_X as i32;
        let pw = (WS_COUNT as i32) * WS_PITCH as i32 + 8;
        fill_rrect(canvas, cw, ch, px, 3, pw, 18, 4, th().button_bg);
        for i in 0..WS_COUNT {
            let active = i == workspace_current;
            let digit = format!("{}", i + 1);
            let dx = px + 4 + (i as i32) * WS_PITCH as i32;
            if active {
                fill_rrect(canvas, cw, ch, dx - 3, 4, WS_PITCH as i32, 16, 4, th().accent);
            }
            text::draw_text(
                canvas,
                cw,
                ch,
                &digit,
                dx,
                8,
                10,
                Weight::Bold,
                if active { th().panel_bg } else { th().text_gray },
            );
        }
        // Titre de la fenêtre focusée, centré (design : « éditeur —
        // ~/yaor/desktop-core »).
        let title = entries
            .iter()
            .find(|e| e.activated)
            .map(|e| e.title.clone())
            .unwrap_or_default();
        if !title.is_empty() {
            let mut t = title;
            let max = cw / 2 - 40;
            while text::text_width(&t, 11, Weight::Regular) > max && t.chars().count() > 4 {
                t.pop();
            }
            let tw = text::text_width(&t, 11, Weight::Regular);
            text::draw_text(canvas, cw, ch, &t, (cw - tw) / 2, 7, 11, Weight::Regular, th().text_fg);
        }
        // Tray à droite : layout + CPU + batterie + heure.
        let tray = format!(
            "[ ] = maître+pile   CPU {} %   {}   {}",
            cpu_percent(),
            battery_text(),
            right_clock()
        );
        let tw = text::text_width(&tray, 11, Weight::Regular);
        text::draw_text(canvas, cw, ch, &tray, cw - tw - 8, 7, 11, Weight::Regular, th().text_fg);
    }

    /// Rendu du volet menu 1d.menu (design Classique.png) : wordmark
    /// « YAOR / OS », champ de recherche, grille ÉPINGLÉES 4x2 avec
    /// monogrammes et libellés minuscules, séparateur, rangée
    /// Utilisateur + bouton power. Journal : jeton « menu : rendu ».
    fn draw_menu(&mut self, layer: &LayerSurface) {
        let w = MENU_W;
        let h = MENU_H;
        self.ensure_pool(w as i32, h as i32);
        let stride = w as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de menu");
        {
            let (cw, ch) = (w as i32, h as i32);
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, th().panel_bg);

            // En-tête : wordmark « YAOR » gras + « OS » espacé, filets
            // latéraux (design Classique — pas de médaillon rond).
            text::draw_text(&mut canvas, cw, ch, "YAOR", 20, 16, 26, Weight::Bold, th().text_fg);
            let os = "OS";
            let ow = text::text_width(os, 9, Weight::Bold) + 8; // interlettrage
            let ox = 22 + (text::text_width("YAOR", 26, Weight::Bold) - ow) / 2;
            text::draw_text(&mut canvas, cw, ch, os, ox, 48, 9, Weight::Bold, th().accent);
            fill_rect(&mut canvas, cw, ch, 20, 54, ox - 32, 1, th().separator);
            fill_rect(
                &mut canvas,
                cw,
                ch,
                ox + ow + 10,
                54,
                20 + text::text_width("YAOR", 26, Weight::Bold) - ox,
                1,
                th().separator,
            );

            // Champ de recherche : pilule sombre, placeholder grisé.
            fill_rrect(&mut canvas, cw, ch, 20, 72, MENU_W as i32 - 40, 30, 15, th().button_bg);
            text::draw_text(
                &mut canvas,
                cw,
                ch,
                "Rechercher une application…",
                36,
                81,
                12,
                Weight::Regular,
                th().text_gray,
            );

            // Section ÉPINGLÉES + grille 4x2 (tuiles 50 px, 3 styles,
            // libellés minuscules sous chaque tuile).
            let eping = "ÉPINGLÉES";
            text::draw_text_baseline_tracked(
                &mut canvas,
                cw,
                ch,
                eping,
                22,
                112,
                9,
                Weight::Bold,
                2,
                th().text_gray,
            );
            for (idx, (label, sym, style)) in MENU_PINS.iter().enumerate() {
                let col = (idx % 4) as i32;
                let row = (idx / 4) as i32;
                let tx = MENU_GRID_X + col * MENU_TILE_STEP as i32;
                let ty = MENU_GRID_Y + row * MENU_TILE_PITCH as i32;
                let (fg, tile_r) = match style {
                    1 => (th().panel_bg, th().accent),
                    2 => (th().accent, th().accent_dark),
                    _ => (th().text_fg, th().button_bg),
                };
                fill_rrect(
                    &mut canvas,
                    cw,
                    ch,
                    tx,
                    ty,
                    MENU_TILE as i32,
                    MENU_TILE as i32,
                    12,
                    tile_r,
                );
                if *style == 2 {
                    // Contour ambre : fond sombre encarté de 2 px.
                    fill_rrect(
                        &mut canvas,
                        cw,
                        ch,
                        tx + 2,
                        ty + 2,
                        MENU_TILE as i32 - 4,
                        MENU_TILE as i32 - 4,
                        10,
                        th().panel_bg,
                    );
                }
                // Monogramme centré (police réelle, gras 20 px).
                let gw = text::text_width(sym, 20, Weight::Bold);
                text::draw_text(
                    &mut canvas,
                    cw,
                    ch,
                    sym,
                    tx + (MENU_TILE as i32 - gw) / 2,
                    ty + 12,
                    20,
                    Weight::Bold,
                    fg,
                );
                // Libellé centré sous la tuile.
                let lw = text::text_width(label, 10, Weight::Regular);
                text::draw_text(
                    &mut canvas,
                    cw,
                    ch,
                    label,
                    tx + (MENU_TILE as i32 - lw) / 2,
                    ty + MENU_TILE as i32 + 5,
                    10,
                    Weight::Regular,
                    th().text_fg,
                );
            }

            // Séparateur + rangée utilisateur : avatar + nom, bouton
            // power (anneau + barre verticale).
            let uy = MENU_H as i32 - 50;
            fill_rect(&mut canvas, cw, ch, 20, uy, MENU_W as i32 - 40, 1, th().separator);
            fill_circle(&mut canvas, cw, ch, 33, uy + 25, 13, th().accent);
            text::draw_text(
                &mut canvas,
                cw,
                ch,
                "Utilisateur",
                56,
                uy + 18,
                12,
                Weight::Regular,
                th().text_fg,
            );
            fill_rrect(
                &mut canvas,
                cw,
                ch,
                MENU_W as i32 - 54,
                uy + 8,
                34,
                34,
                9,
                th().button_bg,
            );
            let pcx = MENU_W as i32 - 37;
            let pcy = uy + 25;
            fill_ring(&mut canvas, cw, ch, pcx, pcy, 7, 2, th().accent);
            fill_rect(&mut canvas, cw, ch, pcx, pcy - 11, 1, 9, th().accent);
        }
        layer
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(layer.wl_surface()).expect("attach menu");
        layer.commit();
        eprintln!("{PROG}: [{:.3}] menu : rendu {}x{}", uptime(), w, h);
    }

    /// Rendu de la taskbar basse (1d.tb — maquette profil classique) :
    /// pilule emblème + flamme à gauche, pilules à libellés par fenêtre
    /// (coins arrondis AA, liseré ambre si activée), ligne d'état
    /// (heure + date) à droite. Journal : jeton « taskbar : rendu ».
    fn draw_taskbar(&mut self) {
        let Some(taskbar) = self.taskbar.clone() else {
            return;
        };
        let w = self.tb_width;
        let h = TASKBAR_H;
        if w == 0 || h == 0 {
            return;
        }
        self.ensure_pool(w as i32, h as i32);
        let stride = w as i32 * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer de taskbar");
        if !self.taskbar_visible {
            // Profil macOS-like (étape 3) : la taskbar n'existe pas. La
            // surface reste mappée — elle revient au SIGHUP si le profil
            // change — mais elle est invisible : buffer entièrement
            // transparent et région d'entrée vide (les clics traversent
            // vers les fenêtres, cf. apply_taskbar).
            canvas.fill(0);
            taskbar
                .wl_surface()
                .damage_buffer(0, 0, w as i32, h as i32);
            buffer
                .attach_to(taskbar.wl_surface())
                .expect("attach taskbar");
            taskbar.commit();
            return;
        }
        {
            let (cw, ch) = (w as i32, h as i32);
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, th().panel_bg);

            // Pilule emblème : fond sombre + flamme ambre (OUVRE le
            // volet menu en profil classique) + filet séparateur.
            let ey0 = ((TASKBAR_H as i32 - TB_BTN_H as i32) / 2) as i32;
            fill_pill(&mut canvas, cw, ch, 4, ey0, TB_EMBLEM_W, TB_BTN_H as i32, th().button_bg);
            fill_flame(
                &mut canvas,
                cw,
                ch,
                4 + TB_EMBLEM_W / 2,
                ey0 + 4,
                10,
                10,
                th().accent,
            );
            fill_rect(&mut canvas, cw, ch, TB_X0 - 7, ey0 + 2, 1, TB_BTN_H as i32 - 4, th().separator);

            for (idx, entry) in self.entries.iter().enumerate() {
                let x0 = TB_X0 + idx as i32 * TB_BTN_STEP as i32;
                let y0 = ((TASKBAR_H as i32 - TB_BTN_H as i32) / 2) as i32;
                let border = if entry.activated {
                    th().accent
                } else {
                    th().rest_border()
                };
                // Pilule : liseré 1 px (ambre si focusée) + fond intérieur,
                // coins arrondis AA (1d.tb).
                fill_pill(&mut canvas, cw, ch, x0, y0, TB_BTN_W as i32, TB_BTN_H as i32, border);
                fill_pill(
                    &mut canvas,
                    cw,
                    ch,
                    x0 + 1,
                    y0 + 1,
                    TB_BTN_W as i32 - 2,
                    TB_BTN_H as i32 - 2,
                    th().button_bg,
                );
                // titre (minuscules — police réelle), borné au bouton.
                let label = taskbar_label(&entry.title);
                let tw = text::text_width(&label, 11, Weight::Regular);
                let tx = x0 + (TB_BTN_W as i32 - tw) / 2;
                let ty = y0 + (TB_BTN_H as i32 - 11) / 2;
                text::draw_text(&mut canvas, cw, ch, &label, tx, ty, 11, Weight::Regular, th().text_fg);
            }

            // Ligne d'état à droite (design Classique) : « FR · 82 % »
            // centré verticalement, horloge sur deux lignes (heure
            // grasse au-dessus, date en dessous).
            let (time, date, _) = clock_text();
            let status = format!("{} · {}", layout_text(), battery_text());
            let sw = text::text_width(&status, 10, Weight::Regular);
            text::draw_text(
                &mut canvas,
                cw,
                ch,
                &status,
                cw - text::text_width(&time, 11, Weight::Bold) - 8 - sw,
                (TASKBAR_H as i32 - 10) / 2,
                10,
                Weight::Regular,
                th().text_gray,
            );
            let tw = text::text_width(&time, 11, Weight::Bold);
            text::draw_text(&mut canvas, cw, ch, &time, cw - tw - 6, 2, 11, Weight::Bold, th().text_fg);
            let dw = text::text_width(&date, 8, Weight::Regular);
            text::draw_text(&mut canvas, cw, ch, &date, cw - dw - 6, 18, 8, Weight::Regular, th().text_gray);
        }
        taskbar
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer
            .attach_to(taskbar.wl_surface())
            .expect("attach taskbar");
        taskbar.commit();
        let focus = self
            .entries
            .iter()
            .position(|e| e.activated)
            .map(|i| i.to_string())
            .unwrap_or_else(|| "-".into());
        let minimized = self.entries.iter().filter(|e| e.minimized).count();
        eprintln!(
            "{PROG}: [{:.3}] taskbar : rendu {} bouton(s) focus={} min={}",
            uptime(),
            self.entries.len(),
            focus,
            minimized
        );
    }

    /// Applique la visibilité courante de la taskbar : zone exclusive
    /// (les fenêtres récupèrent la bande de 24 px quand elle est masquée)
    /// et région d'entrée (les clics traversent une taskbar masquée).
    /// Le commit déclenche l'arrange du compositor, qui renvoie un
    /// configure — le redessin part de là.
    fn apply_taskbar(&mut self) {
        let Some(taskbar) = self.taskbar.clone() else {
            return;
        };
        if self.taskbar_visible {
            taskbar.set_exclusive_zone(TASKBAR_H as i32);
            // `None` = région infinie (toute la surface prend les clics).
            taskbar.wl_surface().set_input_region(None);
            self.empty_region = None;
        } else {
            taskbar.set_exclusive_zone(0);
            let region = match self.empty_region.take() {
                Some(r) => r,
                None => Region::new(&self.compositor).expect("région d'entrée vide"),
            };
            // Région créée sans rectangle : entièrement vide (aucune entrée).
            taskbar.wl_surface().set_input_region(Some(region.wl_region()));
            self.empty_region = Some(region);
        }
        taskbar.commit();
    }

    /// Rechargement au SIGHUP (étape 3) : le compositor applique le nouveau
    /// profil, le panel suit — thème ([theme]) et visibilité de la taskbar
    /// (profil macOS-like : masquée). Journal : jeton « taskbar : masquée ».
    fn reload_config(&mut self, qh: &QueueHandle<Self>) {
        let (cfg, src) = yaor_config::DesktopConfig::load();
        init_theme(&cfg, src.as_deref());
        let visible = taskbar_visible(cfg.profile.name);
        let profile = match cfg.profile.name {
            Some(p) => p.as_str(),
            None => "aucun",
        };
        if visible != self.taskbar_visible {
            self.taskbar_visible = visible;
            eprintln!(
                "{PROG}: [{:.3}] taskbar : {} (profil {profile})",
                uptime(),
                if visible { "affichée" } else { "masquée" }
            );
            self.apply_taskbar();
        } else {
            eprintln!(
                "{PROG}: [{:.3}] rechargement : profil {profile}, taskbar inchangée",
                uptime()
            );
        }
        // Indicateurs d'espaces (étape 3) : profil tiling seul — la
        // visibilité suit le profil au même titre que la taskbar.
        let ws_visible = cfg.profile.name == Some(yaor_config::Profile::Tiling);
        if ws_visible != self.workspace_visible {
            self.workspace_visible = ws_visible;
            eprintln!(
                "{PROG}: [{:.3}] espaces : indicateurs {} (profil {profile})",
                uptime(),
                if ws_visible { "affichés" } else { "masqués" }
            );
        }
        self.draw_bar(qh);
        self.draw_taskbar();
    }

    /// Clic sur un bouton de la taskbar : restauration (si minimisée)
    /// puis activation — via le protocole foreign-toplevel (1b.c).
    /// 1d.tb : les pilules démarrent après la pilule emblème (TB_X0).
    fn taskbar_click(&mut self, x: u32) {
        let idx = x.saturating_sub(TB_X0 as u32) / TB_BTN_STEP;
        let within = x.saturating_sub(TB_X0 as u32) % TB_BTN_STEP < TB_BTN_W;
        if !within {
            eprintln!("{PROG}: [{:.3}] taskbar : clic hors bouton x={x}", uptime());
            return;
        }
        let Some(seat) = self.seat_object.clone() else {
            eprintln!("{PROG}: [{:.3}] taskbar : seat absent", uptime());
            return;
        };
        let Some(entry) = self.entries.get(idx as usize) else {
            eprintln!("{PROG}: [{:.3}] taskbar : entree absente idx={idx}", uptime());
            return;
        };
        if entry.minimized {
            entry.handle.unset_minimized();
            eprintln!("{PROG}: [{:.3}] taskbar : restauration envoyee idx={idx}", uptime());
        }
        entry.handle.activate(&seat);
        eprintln!("{PROG}: [{:.3}] taskbar : activation envoyee idx={idx}", uptime());
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            // Destruction des objets wayland : le compositor reçoit
            // layer_destroyed et démappe la couche (retour du clavier
            // aux fenêtres à la prochaine touche).
            drop(menu);
            eprintln!("{PROG}: [{:.3}] menu fermé", uptime());
        }
    }

    // ------------------------------------------------------------------
    // Dialog « Profils de bureau » (étape macOS-like — le cœur
    // « Caméléon » du design) : overlay centré, trois cartes, Annuler /
    // Appliquer. Appliquer écrit `[profile]` via `yaor-ctl profile set`
    // (le compositor relaie le SIGHUP au panel et au dock — rechargement
    // à chaud, cf. launcher.rs).
    // ------------------------------------------------------------------

    fn close_dialog(&mut self) {
        if let Some(dlg) = self.dialog.take() {
            drop(dlg);
            eprintln!("{PROG}: [{:.3}] dialog profils fermé", uptime());
        }
    }

    fn open_dialog(&mut self, qh: &QueueHandle<Self>) {
        if self.dialog.is_some() {
            return;
        }
        // Carte sélectionnée = profil courant.
        self.dialog_sel = match self.profile {
            Some(yaor_config::Profile::Classic) => 0,
            Some(yaor_config::Profile::Macos) => 1,
            Some(yaor_config::Profile::Tiling) => 2,
            None => 1,
        };
        let surface = self.compositor.create_surface(qh);
        let layer = self
            .layer_shell
            .create_layer_surface(qh, surface, Layer::Overlay, Some("yaor-panel-dialog"), None);
        // Centrage : ancré aux 4 bords, taille fixe — le layer map le
        // place au centre de la zone utile.
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(DLG_W, DLG_H);
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        eprintln!("{PROG}: [{:.3}] dialog profils ouvert", uptime());
        self.dialog = Some(layer);
    }

    /// Rendu du dialog (design macOS-like.png) : titre « Profil de
    /// bureau », sous-titre, trois cartes (vignette + nom + sous-titre,
    /// badge **Actif**), boutons Annuler / Appliquer.
    fn draw_dialog(&mut self, layer: &LayerSurface) {
        let (w, h) = (DLG_W as i32, DLG_H as i32);
        self.ensure_pool(DLG_W as i32, DLG_H as i32);
        let stride = w * 4;
        let (buffer, mut canvas) = self
            .pool
            .create_buffer(w, h, stride, wl_shm::Format::Argb8888)
            .expect("création du buffer du dialog");
        {
            fill_rect(&mut canvas, w, h, 0, 0, w, h, th().panel_bg);
            // Cadre du dialog (liseré ambre).
            fill_rrect(&mut canvas, w, h, 0, 0, DLG_W as i32, DLG_H as i32, 14, th().accent);
            fill_rrect(&mut canvas, w, h, 1, 1, DLG_W as i32 - 2, DLG_H as i32 - 2, 13, th().panel_bg);
            // Entête : pastilles feux tricolores + titre centré.
            fill_circle(&mut canvas, w, h, 26, 18, 5, th().accent);
            fill_circle(&mut canvas, w, h, 46, 18, 5, th().accent_dark);
            fill_circle(&mut canvas, w, h, 66, 18, 5, th().button_bg);
            let title = "Profils de bureau";
            let tw = text::text_width(title, 12, Weight::Regular);
            text::draw_text(&mut canvas, w, h, title, (w - tw) / 2, 12, 12, Weight::Regular, th().text_fg);
            // Corps : grand titre + sous-titre.
            text::draw_text(&mut canvas, w, h, "Profil de bureau", 24, 52, 17, Weight::Bold, th().text_fg);
            text::draw_text(
                &mut canvas,
                w,
                h,
                "Choisissez l'organisation des fenêtres. Le cœur du bureau reste identique.",
                24,
                80,
                11,
                Weight::Regular,
                th().text_gray,
            );
            // Cartes.
            for (i, (name, sub)) in DLG_TITLES.iter().zip(DLG_SUBTITLES.iter()).enumerate() {
                let x = DLG_CARD_X + i as i32 * DLG_CARD_STEP;
                let y = DLG_CARD_Y;
                let selected = i == self.dialog_sel;
                let active = self.profile == yaor_config::Profile::from_index(i);
                let border = if selected { th().accent } else { th().rest_border() };
                fill_rrect(&mut canvas, w, h, x, y, DLG_CARD_W, DLG_CARD_H, 12, border);
                fill_rrect(&mut canvas, w, h, x + 1, y + 1, DLG_CARD_W - 2, DLG_CARD_H - 2, 11, th().panel_bg);
                // Vignette : silhouette de chaque profil (fenêtres).
                let vx = x + 12;
                let vy = y + 12;
                let vw = DLG_CARD_W - 24;
                let vh = 130;
                fill_rrect(&mut canvas, w, h, vx, vy, vw, vh, 8, th().button_bg);
                match i {
                    0 => {
                        // Classique : deux fenêtres flottantes.
                        fill_rrect(&mut canvas, w, h, vx + 14, vy + 16, 70, 44, 4, th().text_gray);
                        fill_rrect(
                            &mut canvas,
                            w,
                            h,
                            vx + 58,
                            vy + 34,
                            76,
                            50,
                            4,
                            if active { th().accent } else { th().text_fg },
                        );
                    }
                    1 => {
                        // macOS-like : fenêtre + dock.
                        fill_rrect(
                            &mut canvas,
                            w,
                            h,
                            vx + 20,
                            vy + 14,
                            84,
                            54,
                            4,
                            if active { th().accent } else { th().text_fg },
                        );
                        fill_rrect(&mut canvas, w, h, vx + 46, vy + vh - 26, 40, 12, 6, th().text_gray);
                    }
                    _ => {
                        // Tiling : maître + pile.
                        fill_rrect(
                            &mut canvas,
                            w,
                            h,
                            vx + 12,
                            vy + 12,
                            62,
                            vh - 24,
                            4,
                            if active { th().accent } else { th().text_fg },
                        );
                        fill_rrect(&mut canvas, w, h, vx + 80, vy + 12, 40, 42, 4, th().text_gray);
                        fill_rrect(&mut canvas, w, h, vx + 80, vy + 60, 40, 42, 4, th().text_gray);
                    }
                }
                // Badge Actif (pilule ambre en haut à droite).
                if active {
                    let bw = text::text_width("Actif", 9, Weight::Bold) + 12;
                    fill_pill(
                        &mut canvas,
                        w,
                        h,
                        x + DLG_CARD_W - bw - 10,
                        y + 6,
                        bw,
                        18,
                        th().accent,
                    );
                    text::draw_text(
                        &mut canvas,
                        w,
                        h,
                        "Actif",
                        x + DLG_CARD_W - bw + 1,
                        y + 10,
                        9,
                        Weight::Bold,
                        th().panel_bg,
                    );
                }
                // Nom + sous-titre.
                text::draw_text(&mut canvas, w, h, name, x + 12, y + 154, 13, Weight::Bold, th().text_fg);
                text::draw_text(&mut canvas, w, h, sub, x + 12, y + 176, 10, Weight::Regular, th().text_gray);
            }
            // Boutons bas-droite : Annuler (sombre) / Appliquer (ambre).
            let by = DLG_H as i32 - 56;
            let bw_a = text::text_width("Annuler", 11, Weight::Regular) + 32;
            let bw_p = text::text_width("Appliquer", 11, Weight::Bold) + 32;
            let ax = DLG_W as i32 - 24 - bw_p - 12 - bw_a;
            fill_rrect(&mut canvas, w, h, ax, by, bw_a, 36, 8, th().button_bg);
            text::draw_text(
                &mut canvas,
                w,
                h,
                "Annuler",
                ax + 16,
                by + 12,
                11,
                Weight::Regular,
                th().text_fg,
            );
            let px0 = DLG_W as i32 - 24 - bw_p;
            fill_rrect(&mut canvas, w, h, px0, by, bw_p, 36, 8, th().accent);
            text::draw_text(
                &mut canvas,
                w,
                h,
                "Appliquer",
                px0 + 16,
                by + 12,
                11,
                Weight::Bold,
                th().panel_bg,
            );
        }
        layer.wl_surface().damage_buffer(0, 0, w, h);
        buffer.attach_to(layer.wl_surface()).expect("attach dialog");
        layer.commit();
        eprintln!("{PROG}: [{:.3}] dialog profils : rendu {}x{}", uptime(), w, h);
    }

    /// Applique le profil sélectionné : `yaor-ctl profile set <nom>`
    /// (écrit la config puis réveille le compositor — SIGHUP relayé
    /// aux clients, cf. launcher.rs). Puis referme le dialog.
    fn apply_profile(&mut self) {
        let name = DLG_PROFILES[self.dialog_sel.min(2)];
        match Command::new("yaor-ctl").arg("profile").arg("set").arg(name).spawn() {
            Ok(child) => eprintln!(
                "{PROG}: [{:.3}] profil : application « {} » (yaor-ctl pid {})",
                uptime(),
                name,
                child.id()
            ),
            Err(e) => eprintln!(
                "{PROG}: [{:.3}] profil : échec lancement yaor-ctl : {e}",
                uptime()
            ),
        }
        self.close_dialog();
    }

    fn open_menu(&mut self, qh: &QueueHandle<Self>) {
        if self.menu.is_some() {
            return;
        }
        let surface = self.compositor.create_surface(qh);
        let layer = self
            .layer_shell
            .create_layer_surface(qh, surface, Layer::Overlay, Some("yaor-panel-menu"), None);
        // 1d.menu : ancré BAS-GAUCHE, au-dessus de la taskbar. La zone
        // exclusive de la taskbar (24 px) est DÉJÀ soustraite par le
        // layer map (même mécanisme que la barre haute, cf. commit de
        // l'étape 1). Signature sctk : set_margin(top, right, bottom,
        // left) — bottom = gap du design, left = retrait du volet
        // (maquette : volet à (20, 436)).
        layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT);
        layer.set_margin(0, 0, MENU_GAP, 20);
        layer.set_size(MENU_W, MENU_H);
        // Clavier exclusif pendant l'ouverture : les touches sont
        // routées au volet par le compositor (Escape referme — preuve
        // du routage clavier layer-shell de l'étape 1).
        layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer.commit();
        eprintln!("{PROG}: [{:.3}] menu ouvert", uptime());
        self.menu = Some(layer);
    }

    /// Lance une app de démo du menu (env hérité : WAYLAND_DISPLAY,
    /// DISPLAY — le compositor et Xwayland servent les clients).
    #[allow(dead_code)] // 1d.menu : tuiles visuelles — câblage au fil
    // des apps YAOR internes réelles (étape ultérieure).
    fn launch(&self, bin: &str) {
        eprintln!("{PROG}: [{:.3}] lancement {bin} demandé (fork…)", uptime());
        match Command::new(bin).spawn() {
            Ok(child) => eprintln!(
                "{PROG}: [{:.3}] lancement {bin} (pid {})",
                uptime(),
                child.id()
            ),
            Err(e) => eprintln!("{PROG}: [{:.3}] échec lancement {bin} : {e}", uptime()),
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers sctk
// ---------------------------------------------------------------------------

impl CompositorHandler for Panel {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Panel {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Panel {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        eprintln!("{PROG}: couche fermée par le compositor");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if layer.wl_surface() == self.bar.wl_surface() {
            // Barre : largeur attribuée par le compositor (ancrée
            // gauche+droite, largeur demandée 0 = pleine largeur).
            self.width = if configure.new_size.0 == 0 { 1280 } else { configure.new_size.0 };
            self.height = if configure.new_size.1 == 0 { BAR_H } else { configure.new_size.1 };
            eprintln!("{PROG}: [{:.3}] barre configurée {}x{}", uptime(), self.width, self.height);
            if !self.configured {
                self.configured = true;
            }
            self.draw_bar(qh);
        } else if Some(layer.wl_surface()) == self.taskbar.as_ref().map(|t| t.wl_surface()) {
            // Taskbar : pleine largeur (ancrée bas gauche+droite).
            self.tb_width = if configure.new_size.0 == 0 { 1280 } else { configure.new_size.0 };
            eprintln!("{PROG}: [{:.3}] taskbar configuree {}x{}", uptime(), self.tb_width, configure.new_size.1);
            self.draw_taskbar();
        } else if self.menu.is_some() {
            // Volet menu : taille demandée honorée → premier rendu.
            let (mw, mh) = (configure.new_size.0, configure.new_size.1);
            eprintln!("{PROG}: [{:.3}] menu configuré {}x{}", uptime(), mw, mh);
            if let Some(menu) = self.menu.clone() {
                self.draw_menu(&menu);
            }
        }
    }
}

impl SeatHandler for Panel {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        // Ressource conservée : l'activation foreign-toplevel (taskbar)
        // exige la référence wl_seat (jalon 1b.c).
        self.seat_object = Some(seat);
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        // Ressource conservée : l'activation foreign-toplevel (taskbar)
        // exige la référence wl_seat (jalon 1b.c). ATTENTION : new_seat
        // n'est PAS invoqué pour les seats liés par SeatState::new
        // (burst initial — bind_all sans callback), seule la capability
        // arrive — toujours avant tout événement pointeur/clavier.
        self.seat_object = Some(seat.clone());
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            let keyboard = self
                .seat_state
                .get_keyboard(qh, &seat, None)
                .expect("clavier indisponible");
            self.keyboard = Some(keyboard);
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self.seat_state.get_pointer(qh, &seat).expect("pointeur indisponible");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            self.keyboard.take().unwrap().release();
        }
        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Panel {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _keysyms: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // Menu ouvert : Escape referme (le clavier nous est exclusif).
        if self.menu.is_some() && event.keysym == Keysym::Escape {
            self.close_menu();
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for Panel {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            let (x, y) = event.position;
            if let Some(dlg) = self.dialog.clone() {
                if event.surface == *dlg.wl_surface() {
                    // Dialog profils : cartes (sélection), Annuler,
                    // Appliquer. Fermer par la pastille gauche.
                    if let Press { button, .. } = event.kind {
                        if button == 0x110 {
                            let (xi, yi) = (x as i32, y as i32);
                            if xi < 90 && yi < 40 {
                                self.close_dialog();
                                return;
                            }
                            for (i, &title) in DLG_TITLES.iter().enumerate() {
                                let cx = DLG_CARD_X + i as i32 * DLG_CARD_STEP;
                                if xi >= cx
                                    && xi < cx + DLG_CARD_W
                                    && yi >= DLG_CARD_Y
                                    && yi < DLG_CARD_Y + DLG_CARD_H
                                {
                                    self.dialog_sel = i;
                                    eprintln!(
                                        "{PROG}: [{:.3}] dialog profils : carte {title} sélectionnée",
                                        uptime()
                                    );
                                    self.draw_dialog(&dlg);
                                    return;
                                }
                            }
                            let by = DLG_H as i32 - 56;
                            if yi >= by && yi < by + 36 {
                                let bw_a =
                                    text::text_width("Annuler", 11, Weight::Regular) + 32;
                                let bw_p =
                                    text::text_width("Appliquer", 11, Weight::Bold) + 32;
                                let ax = DLG_W as i32 - 24 - bw_p - 12 - bw_a;
                                let px0 = DLG_W as i32 - 24 - bw_p;
                                if xi >= ax && xi < ax + bw_a {
                                    eprintln!("{PROG}: [{:.3}] dialog profils : annulé", uptime());
                                    self.close_dialog();
                                    return;
                                }
                                if xi >= px0 && xi < px0 + bw_p {
                                    self.apply_profile();
                                    return;
                                }
                            }
                        }
                    }
                    return;
                }
            } else if event.surface == *self.bar.wl_surface() {
                if let Press { button, .. } = event.kind {
                    if button != 0x110 {
                        continue;
                    }
                    match self.profile {
                        Some(yaor_config::Profile::Macos) => {
                            // Menu « Profils » : premier de la liste —
                            // ouvre le dialog (design macOS-like).
                            let w = text::text_width("Profils", 11, Weight::Regular);
                            if x >= 34.0 && x < (34 + w + 14) as f64 {
                                self.open_dialog(qh);
                            }
                        }
                        _ => {
                            // Bouton menu historique (profil non élu).
                            if x >= BTN_X as f64
                                && x < (BTN_X + BTN_W) as f64
                                && y >= 4.0
                                && y < 20.0
                            {
                                if self.menu.is_some() {
                                    self.close_menu();
                                } else {
                                    self.open_menu(qh);
                                }
                            }
                        }
                    }
                    // Indicateurs d'espaces (cliquables — tiling et
                    // barre historique).
                    if self.workspace_visible
                        && x >= WS_X as f64
                        && x < (WS_X + 8 + (WS_COUNT as u32 * WS_PITCH)) as f64
                        && y >= 3.0
                        && y < 21.0
                    {
                        let idx = ((x as u32 - WS_X - 4) / WS_PITCH) as usize;
                        eprintln!(
                            "{PROG}: [{:.3}] espaces : clic indicateur {}",
                            uptime(),
                            idx + 1
                        );
                        if let Some(ws) = self.workspace.as_ref() {
                            ws.switch(idx as u32);
                        }
                    }
                }
            } else if event.surface == *self.taskbar.as_ref().unwrap().wl_surface() {
                if let Press { button, .. } = event.kind {
                    if button == 0x110 {
                        // Pilule emblème (x < TB_X0-7) : ouvre/ferme le
                        // volet menu en profil classique (design — pas
                        // de barre haute).
                        if (x as u32) < (TB_X0 - 7) as u32 {
                            if self.menu.is_some() {
                                self.close_menu();
                            } else {
                                self.open_menu(qh);
                            }
                        } else {
                            self.taskbar_click(x as u32);
                        }
                    }
                }
            } else if let Some(menu) = self.menu.clone() {
                if event.surface == *menu.wl_surface() {
                    if let Press { button, .. } = event.kind {
                        if button == 0x110 {
                            let (x, y) = (x as i32, y as i32);
                            // Grille ÉPINGLÉES : tuiles (même géométrie
                            // que draw_menu). Câblage : Fichiers et
                            // Terminal lancent les apps internes,
                            // Profils ouvre le dialog ; les autres
                            // tuiles restent visuelles.
                            let gx = x - MENU_GRID_X;
                            let gy = y - MENU_GRID_Y;
                            if gx >= 0
                                && gy >= 0
                                && gx < 4 * MENU_TILE_STEP as i32
                                && gy < 2 * MENU_TILE_PITCH as i32
                            {
                                let col = gx / MENU_TILE_STEP as i32;
                                let row = gy / MENU_TILE_PITCH as i32;
                                let lx = gx - col * MENU_TILE_STEP as i32;
                                let ly = gy - row * MENU_TILE_PITCH as i32;
                                if lx < MENU_TILE as i32 && ly < MENU_TILE as i32 {
                                    let idx = (row * 4 + col) as usize;
                                    if let Some((label, _, _)) = MENU_PINS.get(idx) {
                                        eprintln!(
                                            "{PROG}: [{:.3}] menu : tuile {label}",
                                            uptime()
                                        );
                                        match *label {
                                            "Fichiers" => self.launch("yaor-files"),
                                            "Terminal" => self.launch("yaor-term"),
                                            "Profils" => {
                                                self.close_menu();
                                                self.open_dialog(qh);
                                                return;
                                            }
                                            _ => {}
                                        }
                                        self.close_menu();
                                        return;
                                    }
                                }
                            }
                            // Bouton power : visuel (journal + fermeture).
                            let uy = MENU_H as i32 - 50;
                            if x >= MENU_W as i32 - 54
                                && x < MENU_W as i32 - 20
                                && y >= uy + 8
                                && y < uy + 42
                            {
                                eprintln!("{PROG}: [{:.3}] menu : power (visuel)", uptime());
                                self.close_menu();
                            }
                        }
                    }
                }
            }
        }
    }
}

impl ShmHandler for Panel {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

// ---------------------------------------------------------------------------
// Foreign-toplevel (jalon 1b.c) : liste des fenêtres publiées par le
// compositor + actions (activate / unset_minimized).
// ---------------------------------------------------------------------------

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Panel {
    // L'événement toplevel crée un objet (new_id) : la spécialisation
    // event_created_child est OBLIGATOIRE (sinon panic « Missing
    // event_created_child specialization » au premier toplevel) —
    // la macro génère la méthode de trait, DANS le bloc impl.
    wayland_client::event_created_child!(Panel, ZwlrForeignToplevelManagerV1, [
        wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (
            ZwlrForeignToplevelHandleV1,
            ()
        ),
    ]);

    fn event(
        state: &mut Self,
        _manager: &ZwlrForeignToplevelManagerV1,
        event: ManagerEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ManagerEvent::Toplevel { toplevel } => {
                state.entries.push(TaskbarEntry {
                    handle: toplevel,
                    title: String::new(),
                    activated: false,
                    minimized: false,
                });
                eprintln!(
                    "{PROG}: [{:.3}] taskbar : fenetre ajoutee (total={})",
                    uptime(),
                    state.entries.len()
                );
            }
            ManagerEvent::Finished => {
                state.entries.clear();
                state.draw_taskbar();
            }
            _ => {}
        }
    }
}

impl Dispatch<YaorWorkspaceV1, ()> for Panel {
    fn event(
        state: &mut Self,
        _proxy: &YaorWorkspaceV1,
        event: WorkspaceEvent,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // `current` : publié au bind puis à chaque bascule — l'indicateur
        // suit (jeton journal + redessin de la barre). Autres événements
        // (aucun en v1) : ignorés.
        if let WorkspaceEvent::Current { index } = event {
            let idx = index as usize;
            if state.workspace_current != idx {
                state.workspace_current = idx;
                if state.workspace_visible {
                    eprintln!(
                        "{PROG}: [{:.3}] espaces : actif {} (indicateur)",
                        uptime(),
                        idx + 1
                    );
                    state.draw_bar(qh);
                }
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Panel {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: ToplevelEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Tableau d'états : couples (état, valeur) u32 — maximized=0,
        // minimized=1, activated=2.
        const MINIMIZED: u32 = 1;
        const ACTIVATED: u32 = 2;
        let decode = |bytes: &Vec<u8>| -> (bool, bool) {
            let mut min = false;
            let mut act = false;
            for chunk in bytes.chunks_exact(8) {
                let s = u32::from_ne_bytes(chunk[0..4].try_into().unwrap());
                match s {
                    MINIMIZED => min = true,
                    ACTIVATED => act = true,
                    _ => {}
                }
            }
            (min, act)
        };
        match event {
            ToplevelEvent::Title { title } => {
                if let Some(e) = state.entries.iter_mut().find(|e| e.handle == *handle) {
                    e.title = title;
                }
            }
            ToplevelEvent::State { state: bytes } => {
                let (minimized, activated) = decode(&bytes);
                if let Some(e) = state.entries.iter_mut().find(|e| e.handle == *handle) {
                    e.minimized = minimized;
                    e.activated = activated;
                }
            }
            ToplevelEvent::Done => {
                state.draw_taskbar();
            }
            ToplevelEvent::Closed => {
                if let Some(idx) = state.entries.iter().position(|e| e.handle == *handle) {
                    state.entries.remove(idx);
                }
                eprintln!(
                    "{PROG}: [{:.3}] taskbar : fenetre fermee (total={})",
                    uptime(),
                    state.entries.len()
                );
                state.draw_taskbar();
            }
            _ => {}
        }
    }
}

delegate_compositor!(Panel);
delegate_output!(Panel);
delegate_shm!(Panel);
delegate_seat!(Panel);
delegate_keyboard!(Panel);
delegate_pointer!(Panel);
delegate_layer!(Panel);
delegate_registry!(Panel);

impl ProvidesRegistryState for Panel {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

// ---------------------------------------------------------------------------
// Démarrage
// ---------------------------------------------------------------------------

fn usage(out: &mut dyn std::io::Write) {
    let _ = writeln!(out, "Usage: {PROG} [--help] [--version]");
    let _ = writeln!(
        out,
        "Panneau du bureau YAOR (barre layer-shell + menu d'apps de démo)"
    );
    let _ = writeln!(out, "  --help      affiche cette aide");
    let _ = writeln!(out, "  --version   affiche la version du produit");
}

fn main() -> std::process::ExitCode {
    // Arguments : pas de connexion Wayland pour --help/--version (les
    // contrôles de build interrogent le binaire sans session).
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => {}
        Some("--help" | "-h") => {
            usage(&mut std::io::stdout());
            return std::process::ExitCode::SUCCESS;
        }
        Some("--version" | "-V") => {
            println!("{PROG} {VERSION}");
            return std::process::ExitCode::SUCCESS;
        }
        Some(other) => {
            usage(&mut std::io::stderr());
            eprintln!("{PROG}: argument inconnu : {other}");
            return std::process::ExitCode::FAILURE;
        }
    }

    // Thème (1e.theme) et emblème (1e.logo) : chargés avant la session
    // — une lecture de configuration et un décodage PNG uniques, la
    // suite du rendu lit la palette et le bitmap en mémoire.
    let cfg = init_assets();
    let taskbar_shown = taskbar_visible(cfg.profile.name);
    // Indicateurs d'espaces (étape 3) : profil tiling seul.
    let workspace_shown = cfg.profile.name == Some(yaor_config::Profile::Tiling);
    // Profil élu (étape 3) : journalisé avant la connexion, donc visible
    // même sans compositor (contrôle de configuration hors session).
    eprintln!(
        "{PROG}: [{:.3}] profil : {} - taskbar {}",
        uptime(),
        match cfg.profile.name {
            Some(p) => p.as_str(),
            None => "aucun",
        },
        if taskbar_shown { "visible" } else { "masquée" }
    );

    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("{PROG}: connexion au compositor impossible : {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let (globals, mut event_queue) = registry_queue_init(&conn).expect("registre wayland");
    let qh = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor indisponible");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer-shell indisponible");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm indisponible");

    // Barre : couche Top, ancrée haut + pleine largeur, zone
    // exclusive BAR_H (le tiling du compositor se retire de la bande).
    let bar_surface = compositor.create_surface(&qh);
    let bar = layer_shell.create_layer_surface(&qh, bar_surface, Layer::Top, Some("yaor-panel"), None);
    bar.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    bar.set_size(0, BAR_H);
    bar.set_exclusive_zone(BAR_H as i32);
    bar.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    // Commit initial sans buffer : déclenche la configure du compositor,
    // qui pilote le premier rendu (modèle simple_layer).
    bar.commit();

    // Taskbar basse : couche Top ancrée bas, zone exclusive TASKBAR_H
    // (la zone utile du compositor se retire les DEUX bandes — jalon
    // 1b.c, maquette profil classique).
    let tb_surface = compositor.create_surface(&qh);
    let taskbar =
        layer_shell.create_layer_surface(&qh, tb_surface, Layer::Top, Some("yaor-panel-taskbar"), None);
    taskbar.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    taskbar.set_size(0, TASKBAR_H);
    // Profil macOS-like (étape 3) : la taskbar n'est pas visible — aucune
    // bande réservée et région d'entrée vide, dès la première configure.
    taskbar.set_exclusive_zone(if taskbar_shown { TASKBAR_H as i32 } else { 0 });
    let empty_region = if taskbar_shown {
        None
    } else {
        let region = Region::new(&compositor).expect("région d'entrée vide");
        taskbar.wl_surface().set_input_region(Some(region.wl_region()));
        Some(region)
    };
    taskbar.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    taskbar.commit();

    // Pool pré-dimensionné pour 2 slots de barre + 1 slot de menu :
    // la croissance d'un SlotPool (set_len + remap mmap + requête
    // wl_shm_pool.resize) a provoqué un décrochage du panel au premier
    // rendu du menu (QA v0.6-étape 1) — on la rend impossible.
    let bar_need = 1280 * BAR_H as usize * 4;
    let menu_need = MENU_W as usize * MENU_H as usize * 4;
    let pool_capacity = 2 * bar_need + menu_need + 2 * bar_need;
    let pool = SlotPool::new(pool_capacity, &shm).expect("création du slot pool");

    let bar_shown = cfg.profile.name != Some(yaor_config::Profile::Classic);
    let mut panel = Panel {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        layer_shell,
        bar,
        bar_visible: bar_shown,
        profile: cfg.profile.name,
        menu: None,
        dialog: None,
        dialog_sel: 0,
        taskbar: Some(taskbar),
        tb_width: 0,
        taskbar_visible: taskbar_shown,
        empty_region,
        foreign_manager: None,
        workspace: None,
        workspace_current: 0,
        workspace_visible: workspace_shown,
        entries: Vec::new(),
        seat_object: None,
        pool,
        pool_capacity,
        width: 0,
        height: BAR_H,
        configured: false,
        exit: false,
        keyboard: None,
        pointer: None,
    };
    eprintln!("{PROG}: [{:.3}] démarré (version {VERSION})", uptime());

    // Foreign-toplevel : liste des fenêtres + activation/restauration
    // (jalon 1b.c). Le global est publié par le compositor ; absent
    // (compositor ancien) : la taskbar reste vide, non bloquant.
    match panel.registry_state.bind_one::<ZwlrForeignToplevelManagerV1, Panel, ()>(&qh, 1..=1, ())
    {
        Ok(manager) => panel.foreign_manager = Some(manager),
        Err(e) => eprintln!("{PROG}: foreign-toplevel indisponible : {e:?}"),
    }

    // Espaces de travail (étape 3) : le compositor publie l'espace
    // actif au bind puis à chaque bascule. Absent (compositor ancien) :
    // indicateurs visuels seulement, non bloquant.
    match panel.registry_state.bind_one::<YaorWorkspaceV1, Panel, ()>(&qh, 1..=1, ()) {
        Ok(ws) => panel.workspace = Some(ws),
        Err(e) => eprintln!("{PROG}: yaor-workspace indisponible : {e:?}"),
    }

    // SIGHUP (étape 3) : le client recharge `desktop.toml` comme le
    // compositor. L'attente surveille le fd Wayland ET le tube du signal
    // (pas de minuterie ni de scrutation) ; sans tube, la boucle reste
    // strictement bloquante comme en v0.5.
    let hup_tube = yaor_config::hup::install();
    // Flush AVANT la première attente : commit initial + bind
    // foreign-toplevel dorment dans le tampon local du socket —
    // `hup::wait` polle un socket silencieux sinon (le configure du
    // compositor n'arrive jamais, la barre reste noire). Le flush du
    // `blocking_dispatch` n'est atteint qu'APRÈS cette attente.
    let _ = conn.flush();
    while !panel.exit {
        if let Some(tube) = hup_tube {
            let wl_fd = conn.backend().poll_fd().as_raw_fd();
            if yaor_config::hup::wait(wl_fd, tube) {
                panel.reload_config(&qh);
                let _ = conn.flush();
            }
        }
        if event_queue.blocking_dispatch(&mut panel).is_err() {
            break;
        }
        // Flush de fin d'itération : les commits des handlers (barre,
        // taskbar, set_size au changement de liste) partent aussitôt —
        // sinon ils dorment dans le tampon local jusqu'à un événement
        // fortuit (leçon t-yaor-dock, partagée avec yaor-dock).
        let _ = conn.flush();
    }
    eprintln!("{PROG}: [{:.3}] arrêt", uptime());
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taskbar_masquee_hors_profils_classique_et_tiling() {
        use yaor_config::Profile;
        // Profil absent : comportement historique (taskbar présente).
        assert!(taskbar_visible(None));
        assert!(taskbar_visible(Some(Profile::Classic)));
        assert!(taskbar_visible(Some(Profile::Tiling)));
        // macOS-like : le dock remplace la taskbar.
        assert!(!taskbar_visible(Some(Profile::Macos)));
    }

    #[test]
    fn libelle_de_taskbar_tronque_et_garde_la_casse() {
        assert_eq!(taskbar_label("waytest"), "waytest");
        let long = taskbar_label(&"a".repeat(60));
        assert!(long.chars().count() > 0 && long.chars().count() < 60);
        assert!(text::text_width(&long, 11, Weight::Regular) <= TB_BTN_W as i32 - 12);
    }

    #[test]
    fn conversion_civile_d_un_instant_connu() {
        // 2026-11-06T00:00:00Z (date de l'étape 1e) et l'époque UNIX.
        assert_eq!(civil_from_unix(1_793_923_200), (2026, 11, 6, 0, 0));
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0));
        // Fin de journée : les minutes ne débordent pas sur l'heure.
        assert_eq!(civil_from_unix(86_399), (1970, 1, 1, 23, 59));
    }
}
