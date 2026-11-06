// yaor-panel — panneau du bureau YAOR (jalon v0.6 « Caméléon », étape 1).
//
// Premier client layer-shell du système : barre supérieure ancrée en haut
// (couche Top, zone exclusive de 24 px — le compositor soustrait la bande
// du tiling, cf. policy.rs). Bouton menu ouvrant un volet (couche Overlay,
// clavier exclusif pendant l'ouverture) peuplé des apps YAOR de démo
// (waytest, xwtest) — décision produit v0.6 : pas de scan .desktop.
//
// Modèle : examples/simple_layer.rs de smithay-client-toolkit 0.19.2
// (licence MIT © Smithay contributors). Rendu logiciel wl_shm (Argb8888),
// police bitmap 5x7 intégrée (capitales + chiffres — mêmes conventions
// que waytest : « un besoin = un fournisseur », pas de bibliothèque de
// texte). Journal : stderr préfixé « yaor-panel: » (jetons QA courts).
//
// 1e.logo : l'emblème affiché dans la barre est l'image de
// `desktop.toml` (`logo.path`, décodée par yaor-assets et réduite à
// 24 px) ; le bitmap incorporé de `logo.rs` ne sert plus que de repli
// (chemin vide, fichier absent, format illisible) — le panneau démarre
// toujours.
//
// Connexion : WAYLAND_DISPLAY sous XDG_RUNTIME_DIR (socket du compositor).

use std::process::Command;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
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
/// Largeur du bouton menu (px logiques).
const BTN_W: u32 = 60;
/// Dimensions du volet menu 1d.menu (fidèle au design Classique) :
/// 360x330, ancré bas-gauche au-dessus de la taskbar (marge 10 px en
/// plus de la zone exclusive de 24 px de la taskbar).
const MENU_W: u32 = 360;
const MENU_H: u32 = 330;
const MENU_GAP: i32 = 10;
/// Géométrie de la grille ÉPINGLÉES (coordonnées volet) : tuiles 48 px
/// arrondies, 4 colonnes (pas 80 px), 2 rangées (pas 78 px).
const MENU_TILE: u32 = 48;
const MENU_TILE_STEP: u32 = 80;
const MENU_TILE_PITCH: u32 = 78;
const MENU_GRID_X: i32 = 36;
const MENU_GRID_Y: i32 = 118;
/// Tuiles épinglées (1d.menu — décision produit : visuelles, pas de
/// scan .desktop ; le lancement sera câblé pour les apps internes
/// seulement). (libellé, glyphe, style : 0 sombre, 1 ambre plein,
/// 2 contour ambre.)
const MENU_PINS: &[(&str, &str, u8)] = &[
    ("FICHIERS", "F", 1),
    ("TERMINAL", ">", 2),
    ("WEB", "W", 0),
    ("EDITEUR", "E", 1),
    ("REGLAGES", "R", 0),
    ("PROFILS", "D", 2),
    ("MUSIQUE", "M", 0),
    ("CALCUL", "C", 0),
];

// Taskbar basse (jalon v0.6 étape 1d.tb — maquette profil classique) :
// pilule emblème à gauche, pilules à libellés (coins arrondis AA),
// horloge à droite ; liseré ambre si focusée.
const TASKBAR_H: u32 = 24;
/// X de départ des pilules fenêtres (après la pilule emblème).
const TB_X0: i32 = 32;
const TB_EMBLEM_W: i32 = 24;
const TB_BTN_W: u32 = 120;
const TB_BTN_H: u32 = 16;
/// Pas d'un bouton à l'autre (120 + 4 px d'espacement).
const TB_BTN_STEP: u32 = TB_BTN_W + 4;
const TB_BTN_BG: u32 = 0xFF_1E_1E_26;
const TB_BORDER_FOCUS: u32 = 0xFF_E8_86_2E;
const TB_BORDER_REST: u32 = 0xFF_5A_5A_64;
/// Séparateur vertical emblème | fenêtres.
const TB_SEP: u32 = 0xFF_2A_2A_33;

/// Apps de démo du menu : supprimées en 1d.menu — les 8 tuiles du
/// design sont visuelles (décision produit), le lancement sera câblé
/// pour les apps YAOR internes dans une étape ultérieure.

// Couleurs Argb8888 (0xAARRGGBB — converties en octets BGR/A au
// rendu). Identité « ambre sur noir » d'après l'emblème du produit
// (docs/05-ui-ux/branding/yaor-os-amber-emblem.png).
const BAR_BG: u32 = 0xFF_10_10_14; // noir du panneau
const BTN_BG: u32 = 0xFF_E8_86_2E; // bouton menu (ambre)
const BTN_FG: u32 = 0xFF_10_10_14; // texte du bouton (noir)
const MENU_BG: u32 = 0xFF_10_10_14;
const BORDER: u32 = 0xFF_B9_6A_1F; // bordure du volet (ambre sombre)
const TEXT_FG: u32 = 0xFF_F2_E8_DC; // texte clair chaud
// 1d.menu : teintes du volet (maquette validée).
const MENU_TILE_BG: u32 = 0xFF_26_26_2E; // tuile sombre / pilule recherche
const MENU_GRAY: u32 = 0xFF_8A_8A_92; // textes secondaires
const MENU_SEP: u32 = 0xFF_2A_2A_33; // séparateur

/// Position du bouton menu : le logo 24x24 occupe x=2..26, le bouton
/// le suit (QA : clic au centre (60,12)).
const BTN_X: u32 = 30;

// ---------------------------------------------------------------------------
// Police bitmap 5x7 (capitales, chiffres, « . ») — dessin littéral.
// ---------------------------------------------------------------------------

/// Glyphes 5x7 : 7 lignes de 5 colonnes, « X » = pixel allumé.
const GLYPHS: &[(char, [&str; 7])] = &[
    (
        '0',
        ["0XXX0", "0..X0", "X..X0", "X..X0", "X..X0", "X..X0", "0XXX0"],
    ),
    (
        '1',
        ["..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", "0XXXX"],
    ),
    (
        '2',
        ["0XXX0", "X...X", "....X", "...X.", "..X..", ".X...", "XXXXX"],
    ),
    (
        '3',
        ["XXXXX", "....X", "...X.", "..XX.", "....X", "....X", "XXXXX"],
    ),
    (
        '4',
        ["X...X", "X...X", "X...X", "XXXXX", "....X", "....X", "....X"],
    ),
    (
        '5',
        ["XXXXX", "X....", "XXXX.", "....X", "....X", "X...X", "0XXX0"],
    ),
    (
        '6',
        [".XXX.", "X....", "X....", "XXXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '7',
        ["XXXXX", "....X", "...X.", "..X..", ".X...", ".X...", ".X..."],
    ),
    (
        '8',
        [".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", ".XXX."],
    ),
    (
        '9',
        [".XXX.", "X...X", "X...X", ".XXXX", "....X", "....X", ".XXX."],
    ),
    (
        'A',
        [".XXX.", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    ),
    (
        'E',
        ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "XXXXX"],
    ),
    (
        'M',
        ["X...X", "XX.XX", "X.X.X", "X.X.X", "X...X", "X...X", "X...X"],
    ),
    (
        'N',
        ["X...X", "XX..X", "XX..X", "X.X.X", "X..XX", "X..XX", "X...X"],
    ),
    (
        'O',
        [".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'R',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X.X..", "X..X.", "X...X"],
    ),
    (
        'S',
        [".XXXX", "X....", "X....", ".XXX.", "....X", "....X", "XXXX."],
    ),
    (
        'T',
        ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        'U',
        ["X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX."],
    ),
    (
        'W',
        ["X...X", "X...X", "X...X", "X.X.X", "X.X.X", "X.X.X", ".X.X."],
    ),
    (
        'X',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", ".X.X.", "X...X"],
    ),
    (
        'Y',
        ["X...X", "X...X", ".X.X.", "..X..", "..X..", "..X..", "..X.."],
    ),
    (
        '.',
        [".....", ".....", ".....", ".....", ".....", ".XX..", ".XX.."],
    ),
    (
        ':',
        [".....", ".XX..", ".XX..", ".....", ".....", ".XX..", ".XX.."],
    ),
    (
        'C',
        [".XXX.", "X...X", "X....", "X....", "X....", "X...X", ".XXX."],
    ),
    (
        'D',
        ["XXXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "XXXX."],
    ),
    (
        'F',
        ["XXXXX", "X....", "X....", "XXXX.", "X....", "X....", "X...."],
    ),
    (
        'I',
        ["XXXXX", "..X..", "..X..", "..X..", "..X..", "..X..", "XXXXX"],
    ),
    (
        'J',
        ["....X", "....X", "....X", "....X", "X...X", "X...X", ".XXX."],
    ),
    (
        'L',
        ["X....", "X....", "X....", "X....", "X....", "X....", "XXXXX"],
    ),
    (
        'V',
        ["X...X", "X...X", "X...X", "X...X", ".X.X.", ".X.X.", "..X.."],
    ),
    // 1d.menu : glyphes du volet (libellés + symboles de tuiles).
    (
        'B',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X...X", "X...X", "XXXX."],
    ),
    (
        'G',
        ["0XXX.", "X...X", "X....", "X..XX", "X...X", "X...X", "0XXX0"],
    ),
    (
        'H',
        ["X...X", "X...X", "X...X", "XXXXX", "X...X", "X...X", "X...X"],
    ),
    (
        'P',
        ["XXXX.", "X...X", "X...X", "XXXX.", "X....", "X....", "X...."],
    ),
    (
        'Q',
        ["0XXX0", "X...X", "X...X", "X...X", "X.X.X", "X..X.", "0XX.X"],
    ),
    (
        '>',
        ["X....", ".X...", "..X..", "...X.", "..X..", ".X...", "X...."],
    ),
];

fn glyph(ch: char) -> Option<&'static [&'static str; 7]> {
    let up = ch.to_ascii_uppercase();
    GLYPHS.iter().find(|(c, _)| *c == up).map(|(_, g)| g)
}
/// Largeur d'un texte rendu (6 px par glyphe : 5 + 1 d'espacement).
fn text_width(s: &str) -> i32 {
    (s.chars().count() as i32) * 6
}

// ---------------------------------------------------------------------------
// Rendu (canvas wl_shm, octets B,G,R,A — petit boutisme Argb8888).
// ---------------------------------------------------------------------------

fn put(canvas: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let i = ((y * w + x) * 4) as usize;
    if i + 3 < canvas.len() {
        canvas[i] = (color & 0xFF) as u8;
        canvas[i + 1] = ((color >> 8) & 0xFF) as u8;
        canvas[i + 2] = ((color >> 16) & 0xFF) as u8;
        canvas[i + 3] = ((color >> 24) & 0xFF) as u8;
    }
}

fn fill_rect(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    rw: i32,
    rh: i32,
    color: u32,
) {
    for y in y0..y0 + rh {
        for x in x0..x0 + rw {
            put(canvas, w, h, x, y, color);
        }
    }
}

/// Emblème 24x24 en Argb8888 (B,G,R,A), chargé une seule fois au
/// démarrage (1e.logo). Vide tant que [`init_logo`] n'a pas tourné :
/// le bitmap incorporé de `logo.rs` prend alors le relais.
static LOGO: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Charge l'emblème de la barre : `logo.path` de `desktop.toml`
/// (yaor-assets, ré-échantillonné à 24x24), sinon bitmap incorporé.
/// Journalisé en jetons courts — preuve QA de la source utilisée.
fn init_logo() {
    let (cfg, _src) = yaor_config::DesktopConfig::load();
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

/// Logo 24x24 : blit mélangé (source over) — pixel transparent sauté
/// (le noir du panneau transparaît), pixel translucide mélangé à
/// l'existant. `src` : 24x24 Argb8888 (B,G,R,A), `logo.rs` en repli.
fn blit_logo(canvas: &mut [u8], w: i32, h: i32, x0: i32, y0: i32, src: &[u8]) {
    for (row, slice) in src.chunks_exact(24 * 4).enumerate().take(24) {
        let y = y0 + row as i32;
        if y < 0 || y >= h {
            continue;
        }
        for col in 0..24usize {
            let a = slice[col * 4 + 3] as f64 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let x = x0 + col as i32;
            if x < 0 || x >= w {
                continue;
            }
            let off = (y as usize * w as usize + x as usize) * 4;
            for k in 0..3 {
                let bg = canvas[off + k] as f64;
                let fg = slice[col * 4 + k] as f64;
                canvas[off + k] = (bg + (fg - bg) * a).round() as u8;
            }
            canvas[off + 3] = 0xFF;
        }
    }
}

fn draw_logo(canvas: &mut [u8], w: i32, h: i32, x0: i32, y0: i32) {
    let src = LOGO.get().map(|v| v.as_slice()).unwrap_or(&logo::LOGO_24[..]);
    blit_logo(canvas, w, h, x0, y0, src);
}

fn draw_text(canvas: &mut [u8], w: i32, h: i32, text: &str, x0: i32, y0: i32, color: u32) {
    let mut x = x0;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c == 'X' {
                        put(canvas, w, h, x + rx as i32, y0 + ry as i32, color);
                    }
                }
            }
        }
        x += 6;
    }
}

// --- 1d.tb : pilules arrondies + emblème + horloge -------------------------

/// Mélange un pixel (couleur `color` Argb8888) sur le canvas avec la
/// couverture `cov` (0..1) — bords anti-aliasés des pilules.
fn blend_px(canvas: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32, cov: f64) {
    if cov <= 0.0 || x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let i = ((y * w + x) * 4) as usize;
    if i + 3 >= canvas.len() {
        return;
    }
    let a = cov.clamp(0.0, 1.0);
    // couleurs de fond et de premier plan (octets B,G,R)
    for k in 0..3 {
        let bg = canvas[i + k] as f64;
        let fg = ((color >> (8 * k)) & 0xFF) as f64;
        canvas[i + k] = (bg + (fg - bg) * a).round() as u8;
    }
    // alpha : rendu opaque dès qu'on touche (surface de la barre)
    canvas[i + 3] = 0xFF;
}

/// Couverture d'une « pilule » (rectangle à extrémités semi-circulaires,
/// rayon = h/2) au point (px, py) — SDF de rectangle arrondi, AA 1 px.
fn pill_cov(px: f64, py: f64, x0: f64, y0: f64, pw: f64, ph: f64) -> f64 {
    let r = ph / 2.0;
    let cx = x0 + pw / 2.0;
    let cy = y0 + r;
    let qx = (px - cx).abs() - (pw / 2.0 - r);
    let qy = (py - cy).abs() - (ph / 2.0 - r);
    let dx = qx.max(0.0);
    let dy = qy.max(0.0);
    let d = (dx * dx + dy * dy).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Pilule pleine (bords adoucis, fond existant conservé aux coins).
fn fill_pill(canvas: &mut [u8], w: i32, h: i32, x0: i32, y0: i32, pw: i32, ph: i32, color: u32) {
    for y in y0..y0 + ph {
        for x in x0..x0 + pw {
            let cov = pill_cov(x as f64 + 0.5, y as f64 + 0.5, x0 as f64, y0 as f64, pw as f64, ph as f64);
            blend_px(canvas, w, h, x, y, color, cov);
        }
    }
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
                blend_px(canvas, w, h, x, y, color, cov);
            }
        }
    }
}

// --- 1d.menu : formes du volet (rectangle arrondi, cercle, anneau,
// texte agrandi) — mêmes conventions AA 1 px que les pilules. ---------

/// Couverture d'un rectangle arrondi (rayon `r`) au point (px, py) —
/// SDF de rectangle arrondi, AA 1 px.
fn rrect_cov(px: f64, py: f64, x0: f64, y0: f64, rw: f64, rh: f64, r: f64) -> f64 {
    let dx = (x0 + r - px).max(px - (x0 + rw - r)).max(0.0);
    let dy = (y0 + r - py).max(py - (y0 + rh - r)).max(0.0);
    if dx == 0.0 && dy == 0.0 {
        return 1.0;
    }
    let d = if dx > 0.0 && dy > 0.0 {
        (dx * dx + dy * dy).sqrt()
    } else {
        dx.max(dy)
    };
    (0.5 + r - d).clamp(0.0, 1.0)
}

/// Rectangle arrondi plein (bords adoucis, fond existant conservé).
fn fill_rrect(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    rw: i32,
    rh: i32,
    r: i32,
    color: u32,
) {
    for y in y0..y0 + rh {
        for x in x0..x0 + rw {
            let cov = rrect_cov(
                x as f64 + 0.5,
                y as f64 + 0.5,
                x0 as f64,
                y0 as f64,
                rw as f64,
                rh as f64,
                r as f64,
            );
            blend_px(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Disque plein (bords adoucis).
fn fill_circle(canvas: &mut [u8], w: i32, h: i32, cx: i32, cy: i32, r: i32, color: u32) {
    for y in cy - r - 1..=cy + r + 1 {
        for x in cx - r - 1..=cx + r + 1 {
            let d = ((x - cx) as f64).hypot((y - cy) as f64);
            let cov = (r as f64 + 0.5 - d).clamp(0.0, 1.0);
            blend_px(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Anneau (rayon `r`, épaisseur `t`) — symbole power du volet.
fn fill_ring(canvas: &mut [u8], w: i32, h: i32, cx: i32, cy: i32, r: i32, t: i32, color: u32) {
    let lo = (r - t) as f64;
    let hi = (r + t) as f64;
    for y in cy - r - t - 1..=cy + r + t + 1 {
        for x in cx - r - t - 1..=cx + r + t + 1 {
            let d = ((x - cx) as f64).hypot((y - cy) as f64);
            let cov = (hi - 0.5 - d).clamp(0.0, 1.0) * (d - lo + 0.5).clamp(0.0, 1.0);
            blend_px(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Texte 5x7 agrandi d'un facteur `sc` (entête « YAOR » du volet).
fn draw_text_scaled(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    text: &str,
    x0: i32,
    y0: i32,
    color: u32,
    sc: i32,
) {
    let mut x = x0;
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c == 'X' {
                        for sy in 0..sc {
                            for sx in 0..sc {
                                put(canvas, w, h, x + rx as i32 * sc + sx, y0 + ry as i32 * sc + sy, color);
                            }
                        }
                    }
                }
            }
        }
        x += 6 * sc;
    }
}

/// Heure locale du panel : (année, mois, jour, h, min) — conversion
/// civile d'UNIX (algorithme de H. Hinnant). L'invité vise UTC (QEMU) ;
/// une vraie gestion du fuseau passera par timedatectl (étape ultérieure).
fn civil_now() -> (i64, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
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

/// Ligne d'état à droite : « HH:MM  JJ MMM. AAAA » (mois abrégés
/// sans accent — police bitmap capitales).
fn clock_text() -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEV", "MAR", "AVR", "MAI", "JUIN", "JUIL", "AOUT", "SEP", "OCT", "NOV", "DEC",
    ];
    let (y, m, d, hh, mm) = civil_now();
    format!("{:02}:{:02}  {:02} {}. {}", hh, mm, d, MONTHS[(m as usize - 1) % 12], y)
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
    /// Volet menu ouvert (couche Overlay) — créé à l'ouverture, détruit
    /// à la fermeture (la destruction des objets wayland démappe la
    /// couche côté compositor, cf. shell.rs layer_destroyed).
    menu: Option<LayerSurface>,
    /// Taskbar basse (couche Top, ancrée bas, zone exclusive TASKBAR_H
    /// — jalon 1b.c).
    taskbar: Option<LayerSurface>,
    tb_width: u32,

    /// Global zwlr_foreign_toplevel_manager_v1 (liste des fenêtres +
    /// activation/restauration — jalon 1b.c).
    foreign_manager: Option<ZwlrForeignToplevelManagerV1>,
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

    /// Rendu de la barre : logo + bouton menu (ambre) + version.
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
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, BAR_BG);
            // Logo du produit (emblème ambre 24x24, fond transparent).
            draw_logo(&mut canvas, cw, ch, 2, 0);
            fill_rect(&mut canvas, cw, ch, BTN_X as i32, 4, BTN_W as i32, 16, BTN_BG);
            let label = "YAOR";
            let tx = BTN_X as i32 + (BTN_W as i32 - text_width(label)) / 2;
            draw_text(&mut canvas, cw, ch, label, tx, 8, BTN_FG);
            // Version du produit à droite (ADR-022).
            let vers = format!("YAOR {VERSION}");
            draw_text(&mut canvas, cw, ch, &vers, cw - text_width(&vers) - 6, 8, TEXT_FG);
        }
        self.bar
            .wl_surface()
            .damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(self.bar.wl_surface()).expect("attach barre");
        self.bar.commit();
    }

    /// Rendu du volet menu 1d.menu (fidèle au design Classique,
    /// maquette validée) : emblème + YAOR OS, champ de recherche,
    /// grille ÉPINGLÉES 4x2, séparateur, rangée utilisateur + power.
    /// Journal : jetons « menu : rendu ».
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
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, MENU_BG);

            // En-tête : emblème ambre (disque + flamme découpée) +
            // « YAOR » agrandi + « OS » ambre dessous.
            fill_circle(&mut canvas, cw, ch, 30, 34, 14, BTN_BG);
            fill_flame(&mut canvas, cw, ch, 30, 24, 16, 20, MENU_BG);
            draw_text_scaled(&mut canvas, cw, ch, "YAOR", 56, 22, TEXT_FG, 2);
            draw_text(&mut canvas, cw, ch, "OS", 58, 42, BTN_BG);

            // Champ de recherche : pilule sombre, texte grisé.
            fill_rrect(&mut canvas, cw, ch, 16, 62, MENU_W as i32 - 32, 28, 14, MENU_TILE_BG);
            draw_text(
                &mut canvas,
                cw,
                ch,
                "RECHERCHER UNE APPLICATION...",
                28,
                72,
                MENU_GRAY,
            );

            // Section ÉPINGLÉES + grille 4x2 (tuiles 48 px, 3 styles).
            draw_text(&mut canvas, cw, ch, "EPINGLES", 16, 104, MENU_GRAY);
            for (idx, (label, sym, style)) in MENU_PINS.iter().enumerate() {
                let col = (idx % 4) as i32;
                let row = (idx / 4) as i32;
                let tx = MENU_GRID_X + col * MENU_TILE_STEP as i32;
                let ty = MENU_GRID_Y + row * MENU_TILE_PITCH as i32;
                let (fg, tile_r) = match style {
                    1 => (MENU_BG, BTN_BG),
                    2 => (BTN_BG, BORDER),
                    _ => (TEXT_FG, MENU_TILE_BG),
                };
                fill_rrect(
                    &mut canvas,
                    cw,
                    ch,
                    tx,
                    ty,
                    MENU_TILE as i32,
                    MENU_TILE as i32,
                    10,
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
                        8,
                        MENU_BG,
                    );
                }
                // Glyphe centré (agrandi x2 : lisibilité dans la tuile).
                let gx = tx + (MENU_TILE as i32 - 5 * 2) / 2;
                draw_text_scaled(&mut canvas, cw, ch, sym, gx, ty + 16, fg, 2);
                // Libellé centré sous la tuile.
                let lw = text_width(label);
                draw_text(
                    &mut canvas,
                    cw,
                    ch,
                    label,
                    tx + (MENU_TILE as i32 - lw) / 2,
                    ty + MENU_TILE as i32 + 6,
                    MENU_GRAY,
                );
            }

            // Séparateur + rangée utilisateur : avatar + nom, bouton
            // power (anneau + barre verticale).
            let uy = MENU_H as i32 - 52;
            fill_rect(&mut canvas, cw, ch, 16, uy, MENU_W as i32 - 32, 1, MENU_SEP);
            fill_circle(&mut canvas, cw, ch, 32, uy + 22, 12, BTN_BG);
            draw_text(&mut canvas, cw, ch, "UTILISATEUR", 54, uy + 18, TEXT_FG);
            fill_rrect(
                &mut canvas,
                cw,
                ch,
                MENU_W as i32 - 52,
                uy + 4,
                36,
                36,
                10,
                MENU_TILE_BG,
            );
            let pcx = MENU_W as i32 - 34;
            let pcy = uy + 24;
            fill_ring(&mut canvas, cw, ch, pcx, pcy, 7, 2, BTN_BG);
            fill_rect(&mut canvas, cw, ch, pcx, pcy - 11, 1, 9, BTN_BG);
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
        {
            let (cw, ch) = (w as i32, h as i32);
            fill_rect(&mut canvas, cw, ch, 0, 0, cw, ch, BAR_BG);

            // Pilule emblème : fond sombre + mini-flamme ambre + séparateur.
            let ey0 = ((TASKBAR_H as i32 - TB_BTN_H as i32) / 2) as i32;
            fill_pill(&mut canvas, cw, ch, 4, ey0, TB_EMBLEM_W, TB_BTN_H as i32, TB_BTN_BG);
            fill_flame(
                &mut canvas,
                cw,
                ch,
                4 + TB_EMBLEM_W / 2,
                ey0 + 3,
                10,
                10,
                TB_BORDER_FOCUS,
            );
            fill_rect(&mut canvas, cw, ch, TB_X0 - 6, ey0 + 2, 1, TB_BTN_H as i32 - 4, TB_SEP);

            for (idx, entry) in self.entries.iter().enumerate() {
                let x0 = TB_X0 + idx as i32 * TB_BTN_STEP as i32;
                let y0 = ((TASKBAR_H as i32 - TB_BTN_H as i32) / 2) as i32;
                let border = if entry.activated {
                    TB_BORDER_FOCUS
                } else {
                    TB_BORDER_REST
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
                    TB_BTN_BG,
                );
                // titre (capitales — police bitmap), borné au bouton.
                let label: String = entry
                    .title
                    .chars()
                    .take(((TB_BTN_W - 12) / 6) as usize)
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
                let tx = x0 + (TB_BTN_W as i32 - text_width(&label)) / 2;
                let ty = y0 + (TB_BTN_H as i32 - 7) / 2;
                draw_text(&mut canvas, cw, ch, &label, tx, ty, TEXT_FG);
            }

            // Ligne d'état à droite (1d.tb) : heure + date, blanc bitmap.
            let clock = clock_text();
            let tx = cw - text_width(&clock) - 4;
            let ty = (TASKBAR_H as i32 - 7) / 2;
            draw_text(&mut canvas, cw, ch, &clock, tx, ty, TEXT_FG);
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
            if event.surface == *self.bar.wl_surface() {
                // Bouton menu : (4,4)-(4+BTN_W,20).
                if let Press { button, .. } = event.kind {
                    if button == 0x110
                        && x >= BTN_X as f64
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
            } else if event.surface == *self.taskbar.as_ref().unwrap().wl_surface() {
                // Bouton de fenêtre : x → indice (bandes de TB_BTN_STEP).
                if let Enter { .. } = event.kind {
                    eprintln!("{PROG}: [{:.3}] taskbar : pointeur entre", uptime());
                }
                if let Press { button, .. } = event.kind {
                    eprintln!(
                        "{PROG}: [{:.3}] taskbar : presse a ({}, {})",
                        uptime(),
                        x as i32,
                        y as i32
                    );
                    if button == 0x110 {
                        self.taskbar_click(x as u32);
                    }
                }
            } else if let Some(menu) = self.menu.clone() {
                if event.surface == *menu.wl_surface() {
                    if let Press { button, .. } = event.kind {
                        if button == 0x110 {
                            let (x, y) = (x as i32, y as i32);
                            // Grille ÉPINGLÉES : tuiles 48 px, pas 80/78
                            // (même géométrie que draw_menu). Les tuiles
                            // sont visuelles — jeton journal + fermeture.
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
                                            "{PROG}: [{:.3}] menu : tuile {label} (visuelle)",
                                            uptime()
                                        );
                                        self.close_menu();
                                        return;
                                    }
                                }
                            }
                            // Bouton power : pilule (MW-52, uy+4) 36x36.
                            let uy = MENU_H as i32 - 52;
                            if x >= MENU_W as i32 - 52
                                && x < MENU_W as i32 - 16
                                && y >= uy + 4
                                && y < uy + 40
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

    // Emblème de la barre (1e.logo) : chargé avant la session — un
    // décodage PNG unique, la suite du rendu lit le bitmap en mémoire.
    init_logo();

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
    taskbar.set_exclusive_zone(TASKBAR_H as i32);
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

    let mut panel = Panel {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor,
        layer_shell,
        bar,
        menu: None,
        taskbar: Some(taskbar),
        tb_width: 0,
        foreign_manager: None,
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

    while !panel.exit {
        if event_queue.blocking_dispatch(&mut panel).is_err() {
            break;
        }
    }
    eprintln!("{PROG}: [{:.3}] arrêt", uptime());
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Emblème entièrement rouge à `alpha`, canvas noir opaque.
    fn canvas_et_logo(alpha: u8) -> (Vec<u8>, Vec<u8>) {
        let mut canvas = vec![0u8; 24 * 24 * 4];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 0xFF;
        }
        let mut logo = vec![0u8; 24 * 24 * 4];
        for px in logo.chunks_exact_mut(4) {
            px[2] = 0xFF; // rouge (ordre B,G,R,A)
            px[3] = alpha;
        }
        (canvas, logo)
    }

    #[test]
    fn blit_logo_melange_l_alpha() {
        let (mut canvas, logo) = canvas_et_logo(128);
        blit_logo(&mut canvas, 24, 24, 0, 0, &logo);
        assert_eq!(canvas[0..4], [0, 0, 128, 0xFF], "rouge mélangé sur noir");
    }

    #[test]
    fn blit_logo_ignore_le_transparent() {
        let (mut canvas, logo) = canvas_et_logo(0);
        canvas[0..4].copy_from_slice(&[9, 9, 9, 0xFF]);
        blit_logo(&mut canvas, 24, 24, 0, 0, &logo);
        assert_eq!(canvas[0..4], [9, 9, 9, 0xFF], "pixel transparent laissé tel quel");
    }

    #[test]
    fn blit_logo_hors_canvas_ne_panique_pas() {
        let (mut canvas, logo) = canvas_et_logo(255);
        // Ancrage négatif et à cheval sur le bord droit.
        blit_logo(&mut canvas, 24, 24, -10, 20, &logo);
        assert_eq!(canvas.len(), 24 * 24 * 4);
    }
}
