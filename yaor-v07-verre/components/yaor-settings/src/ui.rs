// Modèle, mise en page, hit-test et dessin de l'app Réglages (v0.7).
//
// Pur : aucune dépendance Wayland — tout se teste (et se rend en image)
// hors session. `main.rs` ne fait que relier pointeur, wl_shm et fichier.

use yaor_config::{presets, ThemeConfig};
use yaor_ui::text::{self, Weight};
use yaor_ui::{dim, fill_circle, fill_rrect, Theme};

/// Taille préférée de la fenêtre (px logiques).
pub const PREF_W: i32 = 680;
pub const PREF_H: i32 = 600;
pub const MIN_W: i32 = 560;
pub const MIN_H: i32 = 560;

const M: i32 = 28; // marge
const CARD_H: i32 = 96;
const ROW_H: i32 = 38;

/// Curseurs de la matière.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slider {
    Blur,
    Opacity,
    Saturation,
    Radius,
    Shadow,
    Anim,
}

pub const SLIDERS: [Slider; 6] = [
    Slider::Blur,
    Slider::Opacity,
    Slider::Saturation,
    Slider::Radius,
    Slider::Shadow,
    Slider::Anim,
];

impl Slider {
    pub fn label(self) -> &'static str {
        match self {
            Slider::Blur => "Flou",
            Slider::Opacity => "Opacité du verre",
            Slider::Saturation => "Saturation",
            Slider::Radius => "Rayon des coins",
            Slider::Shadow => "Ombre",
            Slider::Anim => "Animations",
        }
    }
    /// Clé `[theme]` correspondante (même nom que `yaor-ctl theme tune`).
    pub fn key(self) -> &'static str {
        match self {
            Slider::Blur => "glass_blur",
            Slider::Opacity => "glass_opacity",
            Slider::Saturation => "glass_saturation",
            Slider::Radius => "corner_radius",
            Slider::Shadow => "shadow",
            Slider::Anim => "anim_ms",
        }
    }
    /// Curseur propre au verre (grisé en thème plat).
    pub fn glass_only(self) -> bool {
        matches!(self, Slider::Blur | Slider::Opacity | Slider::Saturation)
    }
    /// (min, max) de la valeur réelle.
    pub fn range(self) -> (f64, f64) {
        match self {
            Slider::Blur => (0.0, 48.0),
            Slider::Opacity => (0.10, 0.95),
            Slider::Saturation => (0.5, 2.5),
            Slider::Radius => (2.0, 24.0),
            Slider::Shadow => (0.0, 1.0),
            Slider::Anim => (0.0, 600.0),
        }
    }
}

/// Teintes d'accent proposées.
pub const SWATCHES: [&str; 8] = [
    "#E8862E", "#5AB8FF", "#B48CFF", "#3DDC97", "#FF6B6B", "#F5C542", "#FF7EB6", "#6EE7D8",
];

/// Cible d'un clic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    None,
    Preset(usize),
    Style(bool), // true = verre
    Swatch(usize),
    Slider(Slider),
}

/// État de l'app : le thème en cours d'édition + le curseur tiré.
#[derive(Debug, Clone)]
pub struct Model {
    pub theme: ThemeConfig,
    pub drag: Option<Slider>,
}

/// Rectangles calculés pour une taille de fenêtre.
struct Layout {
    cards: Vec<(i32, i32, i32, i32)>,
    swatches: Vec<(i32, i32)>, // centres
    seg_flat: (i32, i32, i32, i32),
    seg_glass: (i32, i32, i32, i32),
    rows_y: i32,
    track: (i32, i32), // x0, x1
}

fn layout(w: i32) -> Layout {
    let n = presets::NAMES.len() as i32;
    let gap = 10;
    let cw = (w - 2 * M - (n - 1) * gap) / n;
    let cards = (0..n).map(|i| (M + i * (cw + gap), 118, cw, CARD_H)).collect();
    let swatches = (0..SWATCHES.len() as i32).map(|i| (M + 14 + i * 40, 272)).collect();
    let (sw, sh) = (74, 28);
    let sx = w - M - 2 * sw;
    Layout {
        cards,
        swatches,
        seg_flat: (sx, 26, sw, sh),
        seg_glass: (sx + sw, 26, sw, sh),
        rows_y: 338,
        track: (M + 150, w - M - 78),
    }
}

fn inside(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

impl Model {
    pub fn new(theme: ThemeConfig) -> Model {
        Model { theme, drag: None }
    }

    pub fn is_glass(&self) -> bool {
        self.theme.is_glass()
    }

    /// Valeur réelle d'un curseur.
    pub fn value(&self, s: Slider) -> f64 {
        let t = &self.theme;
        match s {
            Slider::Blur => t.glass_blur as f64,
            Slider::Opacity => t.glass_opacity,
            Slider::Saturation => t.glass_saturation,
            Slider::Radius => t.corner_radius as f64,
            Slider::Shadow => t.shadow,
            Slider::Anim => t.anim_ms as f64,
        }
    }

    /// Position normalisée 0..1.
    pub fn fraction(&self, s: Slider) -> f64 {
        let (lo, hi) = s.range();
        ((self.value(s) - lo) / (hi - lo)).clamp(0.0, 1.0)
    }

    /// Pose la valeur depuis une fraction 0..1 (pas naturels : entiers pour
    /// flou/rayon, 10 ms pour animations, 0.01 sinon).
    pub fn set_fraction(&mut self, s: Slider, f: f64) {
        let (lo, hi) = s.range();
        let v = lo + f.clamp(0.0, 1.0) * (hi - lo);
        let t = &mut self.theme;
        match s {
            Slider::Blur => t.glass_blur = v.round() as u32,
            Slider::Opacity => t.glass_opacity = (v * 100.0).round() / 100.0,
            Slider::Saturation => t.glass_saturation = (v * 20.0).round() / 20.0,
            Slider::Radius => t.corner_radius = v.round() as u32,
            Slider::Shadow => t.shadow = (v * 100.0).round() / 100.0,
            Slider::Anim => t.anim_ms = ((v / 10.0).round() * 10.0) as u32,
        }
        presets::clamp_theme(t);
    }

    /// Valeur TOML-texte pour `tune_text` (même format que la CLI).
    pub fn value_text(&self, s: Slider) -> String {
        match s {
            Slider::Blur | Slider::Radius | Slider::Anim => format!("{}", self.value(s).round() as u32),
            _ => format!("{:.2}", self.value(s)),
        }
    }

    /// Texte affiché à droite du curseur.
    pub fn display(&self, s: Slider) -> String {
        let v = self.value(s);
        match s {
            Slider::Blur | Slider::Radius => format!("{} px", v.round() as u32),
            Slider::Opacity | Slider::Shadow => format!("{} %", (v * 100.0).round() as u32),
            Slider::Saturation => format!("{:.1}×", v),
            Slider::Anim => {
                if v < 1.0 {
                    "off".to_string()
                } else {
                    format!("{} ms", v.round() as u32)
                }
            }
        }
    }

    /// Fraction horizontale d'un point `x` sur la piste (pour le drag).
    pub fn fraction_at(&self, w: i32, x: i32) -> f64 {
        let (x0, x1) = layout(w).track;
        ((x - x0) as f64 / (x1 - x0).max(1) as f64).clamp(0.0, 1.0)
    }

    pub fn hit(&self, w: i32, x: i32, y: i32) -> Hit {
        let l = layout(w);
        if inside(l.seg_flat, x, y) {
            return Hit::Style(false);
        }
        if inside(l.seg_glass, x, y) {
            return Hit::Style(true);
        }
        for (i, c) in l.cards.iter().enumerate() {
            if inside(*c, x, y) {
                return Hit::Preset(i);
            }
        }
        for (i, (cx, cy)) in l.swatches.iter().enumerate() {
            if (x - cx).pow(2) + (y - cy).pow(2) <= 16 * 16 {
                return Hit::Swatch(i);
            }
        }
        for (i, s) in SLIDERS.iter().enumerate() {
            let ry = l.rows_y + i as i32 * ROW_H;
            if y >= ry - 4 && y < ry + 26 && x >= l.track.0 - 12 && x <= l.track.1 + 12 {
                return Hit::Slider(*s);
            }
        }
        Hit::None
    }

    /// Préset actif (nom du thème) s'il correspond à un préset connu.
    pub fn active_preset(&self) -> Option<usize> {
        presets::NAMES.iter().position(|n| *n == self.theme.name)
    }

    /// Dessin complet dans un canvas Argb8888 `w`×`h`.
    pub fn draw(&self, canvas: &mut [u8], w: i32, h: i32) {
        let th = Theme::from_config(&self.theme);
        let l = layout(w);
        yaor_ui::fill_rect(canvas, w, h, 0, 0, w, h, th.panel_bg);

        // En-tête
        text::draw_text(canvas, w, h, "Apparence", M, 24, 22, Weight::Bold, th.text_fg);
        text::draw_text(
            canvas, w, h, "Thème du bureau — appliqué immédiatement", M, 54, 12,
            Weight::Regular, th.text_gray,
        );
        // Plat | Verre
        let glass = self.is_glass();
        for (r, label, on) in [(l.seg_flat, "Plat", !glass), (l.seg_glass, "Verre", glass)] {
            let (x, y, rw, rh) = r;
            let bg = if on { th.accent } else { th.button_bg };
            fill_rrect(canvas, w, h, x, y, rw, rh, rh / 2, bg);
            let tw = text::text_width(label, 12, Weight::Bold);
            let fg = if on { th.panel_bg } else { th.text_gray };
            text::draw_text(canvas, w, h, label, x + (rw - tw) / 2, y + 7, 12, Weight::Bold, fg);
        }

        // Présets
        label(canvas, w, h, &th, "PRÉRÉGLAGES", M, 96);
        let active = self.active_preset();
        for (i, (x, y, cw, ch)) in l.cards.iter().copied().enumerate() {
            let name = presets::NAMES[i];
            let p = presets::preset(name).unwrap();
            let pa = yaor_config::parse_color(&p.accent).unwrap_or(th.accent);
            let pbg = yaor_config::parse_color(&p.panel_bg).unwrap_or(th.panel_bg);
            if active == Some(i) {
                fill_rrect(canvas, w, h, x, y, cw, ch, 14, th.accent);
                fill_rrect(canvas, w, h, x + 2, y + 2, cw - 4, ch - 4, 12, th.panel_bg);
            } else {
                fill_rrect(canvas, w, h, x, y, cw, ch, 14, th.separator);
                fill_rrect(canvas, w, h, x + 1, y + 1, cw - 2, ch - 2, 13, th.panel_bg);
            }
            // vignette : mini fenêtre de verre + pastille d'accent
            let (vx, vy, vw, vh) = (x + 10, y + 10, cw - 20, 44);
            fill_rrect(canvas, w, h, vx, vy, vw, vh, 10, dim(pa, 0.45));
            let chip = if p.is_glass() { lighten(pbg, 0.16) } else { pbg };
            fill_rrect(canvas, w, h, vx + 6, vy + 6, vw - 12, vh - 12, 7, chip);
            fill_circle(canvas, w, h, vx + 16, vy + vh / 2, 4, pa);
            let lab = presets::label(name);
            let lw = text::text_width(lab, 11, Weight::Regular);
            text::draw_text(
                canvas, w, h, lab, x + (cw - lw).max(0) / 2, y + ch - 28, 11, Weight::Regular,
                th.text_fg,
            );
            if active == Some(i) {
                let t = "Actif";
                let tw = text::text_width(t, 10, Weight::Bold);
                text::draw_text(canvas, w, h, t, x + (cw - tw) / 2, y + ch - 14, 10, Weight::Bold, th.accent);
            }
        }

        // Accent
        label(canvas, w, h, &th, "COULEUR D'ACCENT", M, 236);
        let cur = yaor_config::parse_color(&self.theme.accent).unwrap_or(th.accent);
        for (i, (cx, cy)) in l.swatches.iter().copied().enumerate() {
            let c = yaor_config::parse_color(SWATCHES[i]).unwrap();
            if c == cur {
                fill_circle(canvas, w, h, cx, cy, 16, th.text_fg);
                fill_circle(canvas, w, h, cx, cy, 14, th.panel_bg);
            }
            fill_circle(canvas, w, h, cx, cy, 11, c);
        }

        // Matière
        label(canvas, w, h, &th, "MATIÈRE", M, 312);
        for (i, s) in SLIDERS.iter().enumerate() {
            let y = l.rows_y + i as i32 * ROW_H;
            let off = s.glass_only() && !glass;
            let fg = if off { dim(th.text_gray, 0.7) } else { th.text_fg };
            text::draw_text(canvas, w, h, s.label(), M, y + 4, 12, Weight::Regular, fg);
            let (x0, x1) = l.track;
            fill_rrect(canvas, w, h, x0, y + 8, x1 - x0, 6, 3, th.button_bg);
            let kx = x0 + (self.fraction(*s) * (x1 - x0) as f64).round() as i32;
            let col = if off { th.text_gray } else { th.accent };
            fill_rrect(canvas, w, h, x0, y + 8, (kx - x0).max(0), 6, 3, col);
            fill_circle(canvas, w, h, kx, y + 11, 9, if off { dim(th.text_gray, 0.8) } else { th.text_fg });
            fill_circle(canvas, w, h, kx, y + 11, 5, col);
            let val = self.display(*s);
            let vw = text::text_width(&val, 12, Weight::Regular);
            text::draw_text(canvas, w, h, &val, w - M - vw, y + 4, 12, Weight::Regular, fg);
        }
        if !glass {
            text::draw_text(
                canvas, w, h, "Flou, opacité et saturation s'appliquent au style Verre.", M,
                l.rows_y + 6 * ROW_H, 11, Weight::Regular, th.text_gray,
            );
        }
    }
}

fn label(canvas: &mut [u8], w: i32, h: i32, th: &Theme, s: &str, x: i32, y: i32) {
    text::draw_text(canvas, w, h, s, x, y, 10, Weight::Bold, th.text_gray);
}

fn lighten(c: u32, a: f64) -> u32 {
    let ch = |sh: u32| {
        let v = ((c >> sh) & 0xFF) as f64;
        (((v + (255.0 - v) * a).round() as u32) & 0xFF) << sh
    };
    (c & 0xFF00_0000) | ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(name: &str) -> Model {
        Model::new(presets::preset(name).unwrap())
    }

    #[test]
    fn hit_cartes_swatches_curseurs_et_style() {
        let m = model("glass-ambre");
        let l = layout(PREF_W);
        for (i, c) in l.cards.iter().enumerate() {
            assert_eq!(m.hit(PREF_W, c.0 + c.2 / 2, c.1 + c.3 / 2), Hit::Preset(i));
        }
        let (cx, cy) = l.swatches[3];
        assert_eq!(m.hit(PREF_W, cx, cy), Hit::Swatch(3));
        for (i, s) in SLIDERS.iter().enumerate() {
            let y = l.rows_y + i as i32 * ROW_H + 11;
            assert_eq!(m.hit(PREF_W, (l.track.0 + l.track.1) / 2, y), Hit::Slider(*s));
        }
        assert_eq!(m.hit(PREF_W, l.seg_flat.0 + 5, l.seg_flat.1 + 5), Hit::Style(false));
        assert_eq!(m.hit(PREF_W, l.seg_glass.0 + 5, l.seg_glass.1 + 5), Hit::Style(true));
        assert_eq!(m.hit(PREF_W, 2, 2), Hit::None);
    }

    #[test]
    fn la_mise_en_page_reste_dans_la_fenetre_a_la_taille_minimale() {
        let l = layout(MIN_W);
        let last = l.cards.last().unwrap();
        assert!(last.0 + last.2 <= MIN_W - M + 1);
        assert!(l.track.1 > l.track.0 + 100, "piste exploitable");
        assert!(l.rows_y + 6 * ROW_H + 24 <= MIN_H + 40);
    }

    #[test]
    fn fraction_aller_retour_et_bornes() {
        let mut m = model("glass-glacier");
        for s in SLIDERS {
            m.set_fraction(s, 0.0);
            assert_eq!(m.fraction(s), 0.0, "{s:?} min");
            m.set_fraction(s, 1.0);
            assert_eq!(m.fraction(s), 1.0, "{s:?} max");
            m.set_fraction(s, 0.5);
            let f = m.fraction(s);
            assert!((f - 0.5).abs() < 0.04, "{s:?} milieu {f}");
        }
        m.set_fraction(Slider::Blur, 0.5);
        assert_eq!(m.theme.glass_blur, 24);
        m.set_fraction(Slider::Anim, 1.0);
        assert_eq!(m.theme.anim_ms, 600);
    }

    #[test]
    fn fraction_at_suit_la_piste() {
        let m = model("flat");
        let (x0, x1) = layout(PREF_W).track;
        assert_eq!(m.fraction_at(PREF_W, x0 - 50), 0.0);
        assert_eq!(m.fraction_at(PREF_W, x1 + 50), 1.0);
        assert!((m.fraction_at(PREF_W, (x0 + x1) / 2) - 0.5).abs() < 0.01);
    }

    #[test]
    fn les_textes_de_valeur_passent_par_le_validateur_de_la_cli() {
        // même chemin que Réglages → fichier : tune_text doit accepter ce
        // que `value_text` produit, pour chaque curseur.
        let mut m = model("glass-emeraude");
        let base = presets::apply_to_text("", "glass-emeraude").unwrap();
        for s in SLIDERS {
            m.set_fraction(s, 0.37);
            let v = m.value_text(s);
            let out = presets::tune_text(&base, &[(s.key(), &v)])
                .unwrap_or_else(|e| panic!("{s:?}={v} : {e}"));
            let cfg = yaor_config::DesktopConfig::parse_str(&out).unwrap();
            assert_eq!(presets::get_key(&cfg.theme, s.key()).unwrap().parse::<f64>().unwrap(),
                       v.parse::<f64>().unwrap(), "{s:?}");
        }
    }

    #[test]
    fn preset_actif_et_personnalise() {
        let mut m = model("glass-amethyste");
        assert_eq!(m.active_preset(), Some(3));
        m.theme.name = "custom".into();
        assert_eq!(m.active_preset(), None);
    }

    #[test]
    fn dessin_ne_panique_pas_et_remplit_le_fond() {
        for n in presets::NAMES {
            let m = model(n);
            let mut cv = vec![0u8; (PREF_W * PREF_H * 4) as usize];
            m.draw(&mut cv, PREF_W, PREF_H);
            assert_eq!(cv[3], 255, "{n}: fond opaque");
            assert!(cv.chunks_exact(4).filter(|p| p[2] > 200).count() > 50, "{n}: texte clair présent");
        }
        // tailles extrêmes : pas de panique
        let m = model("flat");
        let mut cv = vec![0u8; (MIN_W * MIN_H * 4) as usize];
        m.draw(&mut cv, MIN_W, MIN_H);
    }
}
