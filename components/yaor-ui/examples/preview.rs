// Aperçu hors session (dev) : rend des libellés yaor-ui dans des canvas
// Argb8888 et écrit des PPM dans out/preview-*.ppm — convertis en PNG
// avec ffmpeg pour contrôle visuel des étapes de redesign. Usage :
//   cargo run -p yaor-ui --example preview -- <out_dir>
//
// N'est ni embarqué dans l'ISO ni exécuté en QA : c'est le banc de
// développement du rendu (panel/dock/partagé), comme les rendus
// docs/05-ui-ux/design/ servent de cible.

use yaor_ui::text::{self, Weight};
use yaor_ui::{fill_rect, fill_rrect, Theme};

fn save_ppm(path: &std::path::Path, px: &[u8], w: i32, h: i32) -> std::io::Result<()> {
    let mut out = Vec::with_capacity((w * h * 3) as usize + 16);
    out.extend_from_slice(format!("P6\n{w} {h}\n255\n").as_bytes());
    for p in px.chunks_exact(4) {
        out.push(p[2]); // R
        out.push(p[1]); // G
        out.push(p[0]); // B
    }
    std::fs::write(path, &out)
}

fn main() -> std::process::ExitCode {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("répertoire de sortie");
    let t = Theme::CANONIQUE;
    let (w, h) = (360, 340);
    let mut px = vec![0u8; (w * h * 4) as usize];
    fill_rect(&mut px, w, h, 0, 0, w, h, t.panel_bg);

    // Échantillon : libellés du design à toutes les tailles utiles.
    fill_rrect(&mut px, w, h, 16, 20, 328, 30, 15, t.button_bg);
    text::draw_text(
        &mut px, w, h, "Rechercher une application…", 30, 28, 12, Weight::Regular, t.text_gray,
    );
    text::draw_text(
        &mut px, w, h, "ÉPINGLÉES", 18, 66, 10, Weight::Bold, t.text_gray,
    );
    text::draw_text(&mut px, w, h, "Fichiers", 18, 90, 12, Weight::Regular, t.text_fg);
    text::draw_text(&mut px, w, h, "Terminal", 18, 110, 12, Weight::Regular, t.text_fg);
    text::draw_text(&mut px, w, h, "éditeur — ~/yaor/desktop-core", 18, 130, 12, Weight::Regular, t.text_fg);
    text::draw_text(&mut px, w, h, "Profils de bureau", 18, 150, 17, Weight::Bold, t.text_fg);
    text::draw_text(&mut px, w, h, "Yaor 0.2.0 · [ ] = maître+pile", 18, 176, 10, Weight::Regular, t.text_gray);
    text::draw_text(&mut px, w, h, "FR · 82 %   12:34   29 oct. 2026", 18, 194, 11, Weight::Regular, t.text_fg);
    text::draw_text(&mut px, w, h, "YAOR", 18, 216, 30, Weight::Bold, t.text_fg);
    text::draw_text(&mut px, w, h, "ÉÙÇÀÔÏ«»'", 18, 256, 14, Weight::Regular, t.accent);

    save_ppm(&dir.join("preview-text.ppm"), &px, w, h).expect("ppm");
    println!("écrit : {}", dir.join("preview-text.ppm").display());
    std::process::ExitCode::SUCCESS
}
