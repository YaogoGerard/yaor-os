// Aperçu hors session du verre v0.7 : rend un bureau factice (barre, dock,
// fenêtre à barre de titre, volet menu) pour chaque préset, avec le VRAI
// matériau `yaor_ui::glass`. Écrit out/glass-<préset>.ppm.
// Usage : cargo run -p yaor-ui --example glass_preview -- <out_dir> [fond.bgra LxH]
//   fond.bgra : pixels Argb8888 bruts plein écran (optionnel ; sinon procédural).
use yaor_config::presets;
use yaor_ui::glass::{paint, procedural_backdrop, Backdrop, GlassParams, Rect};
use yaor_ui::text::{self, Weight};
use yaor_ui::{fill_circle, fill_rrect};

const W: i32 = 1280;
const H: i32 = 720;

fn save_ppm(path: &std::path::Path, px: &[u8]) {
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    for p in px.chunks_exact(4) {
        out.extend_from_slice(&[p[2], p[1], p[0]]);
    }
    std::fs::write(path, out).unwrap();
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let wall_file = args.next();

    for name in presets::NAMES {
        let cfg = presets::preset(name).unwrap();
        let accent = yaor_config::parse_color(&cfg.accent).unwrap();
        let text_fg = yaor_config::parse_color(&cfg.text_fg).unwrap();
        let text_gray = yaor_config::parse_color(&cfg.text_gray).unwrap();
        let panel_bg = yaor_config::parse_color(&cfg.panel_bg).unwrap();
        let button_bg = yaor_config::parse_color(&cfg.button_bg).unwrap();

        let wall = match &wall_file {
            Some(f) => std::fs::read(f).expect("fond"),
            None => procedural_backdrop(W, H, accent),
        };
        let mut cv = wall.clone();
        let glass = GlassParams::from_config(&cfg);
        let bd = glass.map(|g| Backdrop::new(&wall, W, H, g.blur, g.saturation));
        let r = cfg.corner_radius as i32;

        // surface : verre ou aplat v0.6
        let surf = |cv: &mut [u8], rect: Rect, radius: i32| match (&bd, &glass) {
            (Some(b), Some(g)) => paint(cv, W, H, rect, radius, (0, 0), b, g, false),
            _ => fill_rrect(cv, W, H, rect.x, rect.y, rect.w, rect.h, radius, panel_bg),
        };

        // ombre portée simple sous la fenêtre
        for y in 120..470 {
            for x in 330..780 {
                let i = ((y * W + x) * 4) as usize;
                for k in 0..3 {
                    cv[i + k] = (cv[i + k] as f32 * 0.78) as u8;
                }
            }
        }
        // fenêtre : corps sombre + barre de titre en verre
        fill_rrect(&mut cv, W, H, 340, 130, 420, 330, r, 0xFF0E0E12);
        surf(&mut cv, Rect { x: 340, y: 130, w: 420, h: 30 }, r);
        for (i, c) in [accent, 0xFFEFAA6D, 0xFF26262E].iter().enumerate() {
            fill_circle(&mut cv, W, H, 358 + i as i32 * 20, 145, 6, *c);
        }
        text::draw_text(&mut cv, W, H, "Terminal", 530, 138, 12, Weight::Bold, text_fg);
        text::draw_text(&mut cv, W, H, "yaor@bop:~$ yaor-ctl theme set", 356, 180, 13, Weight::Regular, accent);
        text::draw_text(&mut cv, W, H, "thème appliqué : ", 356, 202, 13, Weight::Regular, text_fg);
        text::draw_text(&mut cv, W, H, label(name), 470, 202, 13, Weight::Bold, text_fg);

        // barre du haut
        surf(&mut cv, Rect { x: 8, y: 6, w: W - 16, h: 32 }, r.min(16));
        text::draw_text(&mut cv, W, H, "YAOR", 24, 14, 13, Weight::Bold, text_fg);
        text::draw_text(&mut cv, W, H, "Fichier   Édition   Affichage   Fenêtre   Aide", 90, 15, 12, Weight::Regular, text_gray);
        text::draw_text(&mut cv, W, H, "FR · 82 %   mar. 10 nov.  12:34", W - 250, 15, 12, Weight::Regular, text_fg);

        // volet menu
        surf(&mut cv, Rect { x: 8, y: 48, w: 300, h: 420 }, r);
        fill_rrect(&mut cv, W, H, 22, 62, 272, 30, 15, with_alpha(button_bg, 0.8));
        text::draw_text(&mut cv, W, H, "Rechercher une application…", 38, 70, 12, Weight::Regular, text_gray);
        text::draw_text(&mut cv, W, H, "ÉPINGLÉES", 24, 110, 10, Weight::Bold, text_gray);
        for (i, l) in ["Fichiers", "Terminal", "Web", "Éditeur", "Réglages", "Profils", "Musique", "Calcul"].iter().enumerate() {
            let (cx, cy) = (30 + (i as i32 % 4) * 68, 134 + (i as i32 / 4) * 84);
            let c = if i == 1 || i == 4 { accent } else { button_bg };
            fill_rrect(&mut cv, W, H, cx, cy, 52, 52, 14, c);
            text::draw_text(&mut cv, W, H, l, cx + 2, cy + 60, 11, Weight::Regular, text_fg);
        }

        // dock (pilule)
        surf(&mut cv, Rect { x: 400, y: 628, w: 480, h: 72 }, r + 6);
        for i in 0..6 {
            let c = match i { 0 => accent, 1 | 2 => dim_acc(accent), _ => button_bg };
            fill_rrect(&mut cv, W, H, 420 + i * 74, 640, 56, 48, 14, c);
        }

        save_ppm(&dir.join(format!("glass-{name}.ppm")), &cv);
        println!("écrit glass-{name}.ppm");
    }
}

fn label(n: &str) -> &'static str { presets::label(n) }

fn with_alpha(c: u32, a: f32) -> u32 {
    // sur fond sombre, une couleur « atténuée » suffit pour l'aperçu
    yaor_ui::dim(c, a as f64)
}

fn dim_acc(c: u32) -> u32 { yaor_ui::dim(c, 0.55f64) }
