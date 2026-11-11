//! Matériau « verre » v0.7 : fond flouté + teinte + reflets de bord.
//!
//! Pur CPU, zéro dépendance, testable hors session (même règle que le
//! reste de `yaor-ui`). Le principe :
//!
//! 1. un [`Backdrop`] est le fond d'écran (pixels Argb8888) **flouté une
//!    seule fois** (réduit ÷2/÷4, 3 passes de flou boîte ≈ gaussienne,
//!    saturation relevée) — le flou n'est jamais refait par image ;
//! 2. [`paint`] découpe dans ce fond la zone sous la surface, y pose la
//!    teinte du thème, un reflet de haut, un liseré spéculaire éclairé
//!    d'en haut (le « rim light » qui fait le verre liquide) et un grain
//!    anti-banding, le tout dans un rectangle arrondi anti-aliasé ;
//! 3. `style = "flat"` ne passe jamais ici : [`GlassParams::from_config`]
//!    renvoie `None`, les clients gardent leur chemin v0.6 inchangé.
//!
//! Le panel, le dock, Réglages et le compositor (barres de titre) appellent
//! la **même** fonction : un seul fournisseur de matériau.

use crate::raster::rrect_cov;

/// Paramètres du matériau, résolus depuis `[theme]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassParams {
    /// Rayon du flou en pixels (0 = pas de flou).
    pub blur: u32,
    /// Teinte 0xAARRGGBB : l'alpha porte l'opacité du verre.
    pub tint: u32,
    /// Saturation du fond flouté.
    pub saturation: f32,
    /// Liseré 0xAARRGGBB.
    pub border: u32,
    /// Reflet de bord haut (0..1).
    pub highlight: f32,
    /// Grain (0..1).
    pub grain: f32,
}

impl GlassParams {
    /// `None` si le thème est plat. Valeurs invalides : replis sûrs
    /// (teinte = `panel_bg`, liseré blanc translucide).
    pub fn from_config(t: &yaor_config::ThemeConfig) -> Option<Self> {
        if !t.is_glass() {
            return None;
        }
        let panel = yaor_config::parse_color(&t.panel_bg).unwrap_or(0xFF10_1014);
        let tint_rgb = yaor_config::parse_color(&t.glass_tint).unwrap_or(panel) & 0x00FF_FFFF;
        let opacity = t.glass_opacity.clamp(0.10, 0.95);
        let a = (opacity * 255.0).round() as u32;
        let border = yaor_config::parse_color(&t.glass_border).unwrap_or(0x2EFF_FFFF);
        Some(GlassParams {
            blur: t.glass_blur.min(48),
            tint: (a << 24) | tint_rgb,
            saturation: t.glass_saturation.clamp(0.5, 2.5) as f32,
            border,
            highlight: t.glass_highlight.clamp(0.0, 1.0) as f32,
            grain: t.glass_grain.clamp(0.0, 1.0) as f32,
        })
    }
}

// ---------------------------------------------------------------------------
// Flou boîte séparable (3 passes ≈ gaussienne), O(n) par passe
// ---------------------------------------------------------------------------

/// Une passe de flou boîte de rayon `r` sur les canaux B,G,R d'un canvas
/// 4 octets/pixel ; `horizontal` choisit l'axe. Bords : pixel répété.
fn box_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (lines, len, line_stride, step) = if horizontal {
        (h, w, w * 4, 4)
    } else {
        (w, h, 4, w * 4)
    };
    let n = (2 * r + 1) as i32;
    for line in 0..lines {
        let base = line * line_stride;
        for c in 0..3 {
            let get = |i: isize| -> i32 {
                let k = i.clamp(0, len as isize - 1) as usize;
                src[base + k * step + c] as i32
            };
            let mut sum: i32 = 0;
            for k in -(r as isize)..=(r as isize) {
                sum += get(k);
            }
            for i in 0..len {
                dst[base + i * step + c] = ((sum + n / 2) / n) as u8;
                sum += get(i as isize + r as isize + 1) - get(i as isize - r as isize);
            }
        }
        // alpha : opaque
        for i in 0..len {
            dst[base + i * step + 3] = 0xFF;
        }
    }
}

/// Flou « gaussien » = 3 passes boîte H+V, en place sur `px`.
pub fn blur_in_place(px: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0u8; px.len()];
    for _ in 0..3 {
        box_pass(px, &mut tmp, w, h, r, true);
        box_pass(&tmp, px, w, h, r, false);
    }
}

// ---------------------------------------------------------------------------
// Fond flouté
// ---------------------------------------------------------------------------

/// Fond d'écran flouté une fois, stocké à basse résolution.
#[derive(Debug, Clone)]
pub struct Backdrop {
    /// Taille plein écran d'origine.
    pub w: i32,
    pub h: i32,
    scale: usize,
    lw: usize,
    lh: usize,
    /// Pixels B,G,R,A basse résolution, floutés, saturés, opaques.
    low: Vec<u8>,
}

impl Backdrop {
    /// Construit le fond flouté depuis des pixels Argb8888 plein écran.
    pub fn new(src: &[u8], w: i32, h: i32, blur: u32, saturation: f32) -> Backdrop {
        let (w, h) = (w.max(1), h.max(1));
        let scale: usize = if blur >= 12 { 4 } else if blur >= 5 { 2 } else { 1 };
        let lw = (w as usize).div_ceil(scale);
        let lh = (h as usize).div_ceil(scale);
        let mut low = vec![0u8; lw * lh * 4];
        // réduction par moyenne de blocs
        for ly in 0..lh {
            for lx in 0..lw {
                let mut acc = [0u32; 3];
                let mut n = 0u32;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (x, y) = (lx * scale + dx, ly * scale + dy);
                        if x < w as usize && y < h as usize {
                            let i = (y * w as usize + x) * 4;
                            if i + 3 < src.len() {
                                for c in 0..3 {
                                    acc[c] += src[i + c] as u32;
                                }
                                n += 1;
                            }
                        }
                    }
                }
                let o = (ly * lw + lx) * 4;
                if n > 0 {
                    for c in 0..3 {
                        low[o + c] = (acc[c] / n) as u8;
                    }
                }
                low[o + 3] = 0xFF;
            }
        }
        let r = ((blur as usize) / scale).max(if blur > 0 { 1 } else { 0 });
        blur_in_place(&mut low, lw, lh, r);
        // saturation
        if (saturation - 1.0).abs() > 0.01 {
            for p in low.chunks_exact_mut(4) {
                let (b, g, rr) = (p[0] as f32, p[1] as f32, p[2] as f32);
                let l = 0.114 * b + 0.587 * g + 0.299 * rr;
                p[0] = (l + (b - l) * saturation).clamp(0.0, 255.0) as u8;
                p[1] = (l + (g - l) * saturation).clamp(0.0, 255.0) as u8;
                p[2] = (l + (rr - l) * saturation).clamp(0.0, 255.0) as u8;
            }
        }
        Backdrop { w, h, scale, lw, lh, low }
    }

    /// Couleur (B,G,R en flottants 0..255) au pixel plein écran (x, y),
    /// interpolation bilinéaire ; coordonnées bornées au fond.
    pub fn sample(&self, x: i32, y: i32) -> [f32; 3] {
        let s = self.scale as f32;
        let fx = ((x.clamp(0, self.w - 1) as f32 + 0.5) / s - 0.5).max(0.0);
        let fy = ((y.clamp(0, self.h - 1) as f32 + 0.5) / s - 0.5).max(0.0);
        let x0 = (fx.floor() as usize).min(self.lw - 1);
        let y0 = (fy.floor() as usize).min(self.lh - 1);
        let x1 = (x0 + 1).min(self.lw - 1);
        let y1 = (y0 + 1).min(self.lh - 1);
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let at = |xx: usize, yy: usize, c: usize| self.low[(yy * self.lw + xx) * 4 + c] as f32;
        let mut out = [0.0f32; 3];
        for (c, o) in out.iter_mut().enumerate() {
            let top = at(x0, y0, c) * (1.0 - tx) + at(x1, y0, c) * tx;
            let bot = at(x0, y1, c) * (1.0 - tx) + at(x1, y1, c) * tx;
            *o = top * (1.0 - ty) + bot * ty;
        }
        out
    }
}

/// Fond procédural sombre (dégradé + halos de l'accent) : sert de fond
/// quand `wallpaper.path` est vide, pour que le verre ait toujours
/// quelque chose à flouter. Pixels Argb8888 opaques.
pub fn procedural_backdrop(w: i32, h: i32, accent: u32) -> Vec<u8> {
    let (w, h) = (w.max(1), h.max(1));
    let mut px = vec![0u8; (w * h * 4) as usize];
    let (ar, ag, ab) = (
        ((accent >> 16) & 0xFF) as f32,
        ((accent >> 8) & 0xFF) as f32,
        (accent & 0xFF) as f32,
    );
    // trois halos : (cx, cy, rayon relatif, intensité)
    let blobs = [(0.18f32, 0.25f32, 0.55f32, 0.34f32), (0.82, 0.70, 0.60, 0.26), (0.52, 1.05, 0.50, 0.22)];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            // base : noir bleuté qui s'éclaircit vers le bas
            let base = 14.0 + 10.0 * v;
            let mut rgb = [base * 0.95, base * 0.95, base * 1.1];
            for (cx, cy, rad, k) in blobs {
                let d = (((u - cx) * w as f32 / h as f32).powi(2) + (v - cy).powi(2)).sqrt() / rad;
                let f = (1.0 - d).clamp(0.0, 1.0);
                let f = f * f * (3.0 - 2.0 * f) * k;
                rgb[0] += ar * f;
                rgb[1] += ag * f;
                rgb[2] += ab * f;
            }
            let i = ((y * w + x) * 4) as usize;
            px[i] = rgb[2].clamp(0.0, 255.0) as u8;
            px[i + 1] = rgb[1].clamp(0.0, 255.0) as u8;
            px[i + 2] = rgb[0].clamp(0.0, 255.0) as u8;
            px[i + 3] = 0xFF;
        }
    }
    px
}

// ---------------------------------------------------------------------------
// Peinture d'une surface de verre
// ---------------------------------------------------------------------------

/// Rectangle en pixels du canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Bruit déterministe dans [-1, 1] (aucun état, aucun RNG).
fn hash_noise(x: i32, y: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Profondeur signée à l'intérieur d'un rectangle arrondi (>0 dedans,
/// distance au bord en pixels ; <0 dehors).
fn depth_in_rrect(px: f32, py: f32, r: &Rect, radius: f32) -> f32 {
    let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
    let (hx, hy) = (r.w as f32 / 2.0 - radius, r.h as f32 / 2.0 - radius);
    let (qx, qy) = ((px - cx).abs() - hx, (py - cy).abs() - hy);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let inside = qx.max(qy).min(0.0);
    -(outside + inside - radius)
}

/// Peint une surface de verre dans `canvas` (Argb8888 `cw`×`ch`).
///
/// * `rect` : zone peinte, en pixels du canvas ;
/// * `origin` : position **écran** du pixel (0,0) du canvas — c'est elle
///   qui aligne le flou sur le fond (la barre du haut est en (0,0), le
///   dock en bas au centre, etc.) ;
/// * `premul` : `true` pour un canvas transparent prémultiplié (dock) —
///   les coins hors forme restent transparents ; `false` pour un canvas
///   opaque (le fond existant est mélangé au bord).
#[allow(clippy::too_many_arguments)]
pub fn paint(
    canvas: &mut [u8],
    cw: i32,
    ch: i32,
    rect: Rect,
    radius: i32,
    origin: (i32, i32),
    bd: &Backdrop,
    p: &GlassParams,
    premul: bool,
) {
    let radius = (radius.max(0) as f32).min(rect.w.min(rect.h) as f32 / 2.0);
    let tint_a = ((p.tint >> 24) & 0xFF) as f32 / 255.0;
    let tint = [
        (p.tint & 0xFF) as f32,
        ((p.tint >> 8) & 0xFF) as f32,
        ((p.tint >> 16) & 0xFF) as f32,
    ]; // B,G,R
    let rim_a = ((p.border >> 24) & 0xFF) as f32 / 255.0;
    let rim = [
        (p.border & 0xFF) as f32,
        ((p.border >> 8) & 0xFF) as f32,
        ((p.border >> 16) & 0xFF) as f32,
    ];
    let half_h = (rect.h as f32 / 2.0).max(1.0);
    let cy = rect.y as f32 + half_h;

    for y in rect.y.max(0)..(rect.y + rect.h).min(ch) {
        for x in rect.x.max(0)..(rect.x + rect.w).min(cw) {
            let cov = rrect_cov(
                x as f64 + 0.5,
                y as f64 + 0.5,
                rect.x as f64,
                rect.y as f64,
                rect.w as f64,
                rect.h as f64,
                radius as f64,
            ) as f32;
            if cov <= 0.0 {
                continue;
            }
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // 1) fond flouté sous la surface
            let mut c = bd.sample(origin.0 + x, origin.1 + y);
            // 2) teinte du thème, ADAPTATIVE : sur un fond clair (ciel de
            //    la photo, fenêtre blanche derrière) l'opacité monte pour
            //    garantir le contraste du texte clair ; sur fond sombre
            //    elle reste celle du thème.
            let lum = 0.114 * c[0] + 0.587 * c[1] + 0.299 * c[2];
            let boost = ((lum - 70.0) / 185.0).clamp(0.0, 1.0) * 0.60;
            let a_eff = tint_a + (1.0 - tint_a) * boost;
            for k in 0..3 {
                c[k] = c[k] * (1.0 - a_eff) + tint[k] * a_eff;
            }
            // 3) reflet de verre : voile blanc qui s'estompe vers le bas
            let t = ((fy - rect.y as f32) / rect.h.max(1) as f32).clamp(0.0, 1.0);
            let sheen = p.highlight * 0.10 * (1.0 - t) * (1.0 - t);
            for v in c.iter_mut() {
                *v += (255.0 - *v) * sheen;
            }
            // 4) liseré spéculaire éclairé d'en haut + halo interne doux
            let depth = depth_in_rrect(fx, fy, &rect, radius);
            let ny = ((fy - cy) / half_h).clamp(-1.0, 1.0);
            let lit = (0.55 - 0.45 * ny).clamp(0.12, 1.0);
            let line = (1.0 - (depth - 0.5).max(0.0)).clamp(0.0, 1.0);
            let glow = (1.0 - depth / 7.0).clamp(0.0, 1.0).powi(2) * 0.18 * p.highlight;
            let rim_k = (rim_a * lit * line + glow * lit * rim_a.max(0.2)).clamp(0.0, 1.0);
            for k in 0..3 {
                c[k] += (rim[k] - c[k]) * rim_k;
            }
            // 5) grain
            if p.grain > 0.0 {
                let n = hash_noise(origin.0 + x, origin.1 + y) * p.grain * 5.0;
                for v in c.iter_mut() {
                    *v += n;
                }
            }
            let q = |v: f32| v.clamp(0.0, 255.0);
            let i = ((y * cw + x) * 4) as usize;
            if i + 3 >= canvas.len() {
                continue;
            }
            if premul {
                for k in 0..3 {
                    canvas[i + k] = (q(c[k]) * cov).round() as u8;
                }
                canvas[i + 3] = (255.0 * cov).round() as u8;
            } else {
                for k in 0..3 {
                    let bg = canvas[i + k] as f32;
                    canvas[i + k] = (bg + (q(c[k]) - bg) * cov).round() as u8;
                }
                canvas[i + 3] = 0xFF;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Kit de verre pour les clients (panel, dock, Réglages)
// ---------------------------------------------------------------------------

/// Paramètres + fond flouté, construits une fois au démarrage et à chaque
/// SIGHUP. `None` = thème plat : le client garde son rendu v0.6.
#[derive(Debug, Clone)]
pub struct GlassKit {
    pub params: GlassParams,
    pub backdrop: Backdrop,
    /// Rayon des coins du thème (px).
    pub radius: i32,
}

impl GlassKit {
    /// `wall` : fond plein écran Argb8888 (`yaor_assets::wallpaper_argb`) ;
    /// `None` → fond procédural aux couleurs de l'accent.
    pub fn build(
        theme: &yaor_config::ThemeConfig,
        screen: (i32, i32),
        wall: Option<Vec<u8>>,
    ) -> Option<GlassKit> {
        let params = GlassParams::from_config(theme)?;
        let (w, h) = (screen.0.max(1), screen.1.max(1));
        let px = match wall {
            Some(p) if p.len() >= (w * h * 4) as usize => p,
            _ => {
                let accent = yaor_config::parse_color(&theme.accent).unwrap_or(0xFFE8862E);
                procedural_backdrop(w, h, accent)
            }
        };
        let backdrop = Backdrop::new(&px, w, h, params.blur, params.saturation);
        Some(GlassKit { params, backdrop, radius: theme.corner_radius.clamp(2, 24) as i32 })
    }

    /// Peint une surface de verre (voir [`paint`]).
    pub fn paint(
        &self,
        canvas: &mut [u8],
        cw: i32,
        ch: i32,
        rect: Rect,
        radius: i32,
        origin: (i32, i32),
        premul: bool,
    ) {
        paint(canvas, cw, ch, rect, radius, origin, &self.backdrop, &self.params, premul);
    }
}

// ---------------------------------------------------------------------------
// État partagé des clients (panel, dock, Réglages)
// ---------------------------------------------------------------------------

/// Chargeur de fond d'écran injecté par le client (`yaor_assets::
/// wallpaper_argb`) : `yaor-ui` reste sans dépendance image.
pub type WallLoader = fn(path: &str, cover: bool, w: u32, h: u32) -> Option<Vec<u8>>;

struct StateInner {
    theme: Option<yaor_config::ThemeConfig>,
    wall_path: String,
    cover: bool,
    screen: (i32, i32),
    kit: Option<std::sync::Arc<GlassKit>>,
}

/// État de verre d'un client : thème, taille d'écran, kit courant. Une
/// `static` par client ; le flou n'est refait qu'à `configure` (SIGHUP) ou
/// à un changement de taille de sortie — jamais par image.
pub struct GlassState {
    inner: std::sync::Mutex<StateInner>,
    loader: WallLoader,
}

impl GlassState {
    pub const fn new(loader: WallLoader) -> GlassState {
        GlassState {
            inner: std::sync::Mutex::new(StateInner {
                theme: None,
                wall_path: String::new(),
                cover: true,
                screen: (0, 0),
                kit: None,
            }),
            loader,
        }
    }

    fn rebuild(&self, g: &mut StateInner) {
        g.kit = match &g.theme {
            Some(t) if t.is_glass() => {
                let (sw, sh) = if g.screen.0 > 0 { g.screen } else { (1920, 1080) };
                let wall = (self.loader)(&g.wall_path, g.cover, sw as u32, sh as u32);
                GlassKit::build(t, (sw, sh), wall).map(std::sync::Arc::new)
            }
            _ => None,
        };
    }

    /// (Re)configure depuis `desktop.toml` ; reconstruit le kit.
    pub fn configure(&self, theme: &yaor_config::ThemeConfig, wall_path: &str, cover: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.theme = Some(theme.clone());
            g.wall_path = wall_path.to_string();
            g.cover = cover;
            self.rebuild(&mut g);
        }
    }

    /// Taille logique de la sortie ; renvoie vrai si elle a changé.
    pub fn set_screen(&self, w: i32, h: i32) -> bool {
        if w <= 0 || h <= 0 {
            return false;
        }
        let Ok(mut g) = self.inner.lock() else { return false };
        if g.screen == (w, h) {
            return false;
        }
        g.screen = (w, h);
        self.rebuild(&mut g);
        true
    }

    /// Taille d'écran connue (repli 1920x1080).
    pub fn screen(&self) -> (i32, i32) {
        match self.inner.lock() {
            Ok(g) if g.screen.0 > 0 => g.screen,
            _ => (1920, 1080),
        }
    }

    /// Kit courant (`None` = thème plat).
    pub fn kit(&self) -> Option<std::sync::Arc<GlassKit>> {
        self.inner.lock().ok().and_then(|g| g.kit.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaor_config::presets;

    fn flat_px(w: usize, h: usize, bgr: [u8; 3]) -> Vec<u8> {
        let mut v = vec![0u8; w * h * 4];
        for p in v.chunks_exact_mut(4) {
            p[0] = bgr[0];
            p[1] = bgr[1];
            p[2] = bgr[2];
            p[3] = 255;
        }
        v
    }

    #[test]
    fn flou_d_un_aplat_ne_change_rien() {
        let mut px = flat_px(40, 30, [10, 80, 200]);
        let avant = px.clone();
        blur_in_place(&mut px, 40, 30, 5);
        assert_eq!(px, avant);
    }

    #[test]
    fn flou_etale_une_impulsion_et_conserve_l_energie() {
        let (w, h) = (41usize, 41usize);
        let mut px = flat_px(w, h, [0, 0, 0]);
        let c = (20 * w + 20) * 4;
        px[c] = 255;
        px[c + 1] = 255;
        px[c + 2] = 255;
        blur_in_place(&mut px, w, h, 3);
        let centre = px[c];
        let voisin = px[(20 * w + 23) * 4];
        assert!(centre < 255 && centre > 0, "pic abaissé : {centre}");
        assert!(voisin > 0, "énergie étalée");
        let somme: u32 = px.chunks_exact(4).map(|p| p[0] as u32).sum();
        let attendu = 255u32;
        assert!(somme.abs_diff(attendu) < 60, "somme {somme} ~ {attendu}");
    }

    #[test]
    fn backdrop_echantillonne_dans_les_bornes() {
        let src = flat_px(64, 32, [30, 60, 90]);
        let bd = Backdrop::new(&src, 64, 32, 16, 1.0);
        for (x, y) in [(-50, -50), (0, 0), (63, 31), (500, 500)] {
            let c = bd.sample(x, y);
            assert!((c[0] - 30.0).abs() < 1.5 && (c[2] - 90.0).abs() < 1.5);
        }
    }

    #[test]
    fn saturation_accentue_l_ecart_au_gris() {
        let src = flat_px(32, 32, [60, 90, 160]);
        let neutre = Backdrop::new(&src, 32, 32, 0, 1.0).sample(10, 10);
        let sature = Backdrop::new(&src, 32, 32, 0, 1.8).sample(10, 10);
        assert!((sature[2] - sature[0]).abs() > (neutre[2] - neutre[0]).abs());
    }

    #[test]
    fn flat_n_a_pas_de_parametres_de_verre() {
        let flat = presets::preset("flat").unwrap();
        assert!(GlassParams::from_config(&flat).is_none());
        let g = presets::preset("glass-glacier").unwrap();
        let p = GlassParams::from_config(&g).unwrap();
        assert_eq!(p.blur, 24);
        assert_eq!(p.tint >> 24, (0.52f64 * 255.0).round() as u32);
        assert_eq!(p.tint & 0x00FF_FFFF, 0x0A1420);
    }

    #[test]
    fn paint_opaque_garde_les_coins_et_teinte_le_centre() {
        let (w, h) = (120, 60);
        let wall = flat_px(w as usize, h as usize, [200, 200, 200]); // fond clair
        let bd = Backdrop::new(&wall, w, h, 12, 1.0);
        let p = GlassParams::from_config(&presets::preset("glass-ambre").unwrap()).unwrap();
        let mut cv = flat_px(w as usize, h as usize, [1, 2, 3]);
        paint(&mut cv, w, h, Rect { x: 0, y: 0, w, h }, 14, (0, 0), &bd, &p, false);
        // coin extrême : hors forme → fond d'origine conservé
        assert_eq!(&cv[0..3], &[1, 2, 3]);
        // centre : le verre a assombri le fond clair via la teinte sombre
        let i = ((30 * w + 60) * 4) as usize;
        assert!(cv[i] < 200 && cv[i + 1] < 200 && cv[i + 2] < 200, "{:?}", &cv[i..i + 3]);
        assert_eq!(cv[i + 3], 255);
    }

    #[test]
    fn paint_premul_laisse_les_coins_transparents() {
        let (w, h) = (100, 50);
        let wall = flat_px(w as usize, h as usize, [90, 90, 90]);
        let bd = Backdrop::new(&wall, w, h, 8, 1.0);
        let p = GlassParams::from_config(&presets::preset("glass-emeraude").unwrap()).unwrap();
        let mut cv = vec![0u8; (w * h * 4) as usize];
        paint(&mut cv, w, h, Rect { x: 0, y: 0, w, h }, 20, (0, 0), &bd, &p, true);
        assert_eq!(&cv[0..4], &[0, 0, 0, 0], "coin transparent");
        let i = ((25 * w + 50) * 4) as usize;
        assert_eq!(cv[i + 3], 255, "centre opaque");
    }

    #[test]
    fn le_lisere_du_haut_est_plus_clair_que_celui_du_bas() {
        let (w, h) = (160, 80);
        let wall = flat_px(w as usize, h as usize, [40, 40, 40]);
        let bd = Backdrop::new(&wall, w, h, 8, 1.0);
        let mut p = GlassParams::from_config(&presets::preset("glass-glacier").unwrap()).unwrap();
        p.grain = 0.0;
        let mut cv = vec![0u8; (w * h * 4) as usize];
        paint(&mut cv, w, h, Rect { x: 0, y: 0, w, h }, 14, (0, 0), &bd, &p, false);
        let lum = |x: i32, y: i32| {
            let i = ((y * w + x) * 4) as usize;
            cv[i] as u32 + cv[i + 1] as u32 + cv[i + 2] as u32
        };
        assert!(lum(80, 0) > lum(80, h - 1), "rim light haut > bas");
    }

    #[test]
    fn l_origine_decale_le_fond_echantillonne() {
        // fond : moitié gauche sombre, moitié droite claire
        let (w, h) = (200usize, 20usize);
        let mut wall = flat_px(w, h, [10, 10, 10]);
        for y in 0..h {
            for x in 100..w {
                let i = (y * w + x) * 4;
                wall[i] = 240;
                wall[i + 1] = 240;
                wall[i + 2] = 240;
            }
        }
        let bd = Backdrop::new(&wall, w as i32, h as i32, 0, 1.0);
        let mut p = GlassParams::from_config(&presets::preset("glass-ambre").unwrap()).unwrap();
        p.grain = 0.0;
        p.highlight = 0.0;
        p.border = 0;
        let mut a = vec![0u8; 40 * 20 * 4];
        let mut b = a.clone();
        paint(&mut a, 40, 20, Rect { x: 0, y: 0, w: 40, h: 20 }, 4, (0, 0), &bd, &p, true);
        paint(&mut b, 40, 20, Rect { x: 0, y: 0, w: 40, h: 20 }, 4, (150, 0), &bd, &p, true);
        let ia = (10 * 40 + 20) * 4;
        assert!(b[ia] > a[ia] + 40, "zone claire plus claire : {} vs {}", b[ia], a[ia]);
    }

    #[test]
    fn kit_plat_est_none_et_verre_se_construit_sans_fond() {
        assert!(GlassKit::build(&presets::preset("flat").unwrap(), (64, 36), None).is_none());
        let k = GlassKit::build(&presets::preset("glass-ambre").unwrap(), (64, 36), None).unwrap();
        assert_eq!(k.radius, 14);
        // fond fourni trop court : repli procédural, pas de panique
        let k2 = GlassKit::build(&presets::preset("glass-ambre").unwrap(), (64, 36), Some(vec![0; 10]));
        assert!(k2.is_some());
    }

    fn nul_loader(_: &str, _: bool, _: u32, _: u32) -> Option<Vec<u8>> {
        None
    }

    #[test]
    fn etat_partage_reconstruit_au_changement_de_theme_et_d_ecran() {
        static ST: GlassState = GlassState::new(nul_loader);
        assert!(ST.kit().is_none(), "rien configuré : plat");
        ST.configure(&presets::preset("glass-glacier").unwrap(), "", true);
        assert!(ST.kit().is_some());
        assert_eq!(ST.screen(), (1920, 1080), "repli avant toute sortie");
        assert!(ST.set_screen(800, 600));
        assert!(!ST.set_screen(800, 600), "même taille : pas de reconstruction");
        assert_eq!(ST.screen(), (800, 600));
        assert!(!ST.set_screen(0, 10), "taille invalide ignorée");
        ST.configure(&presets::preset("flat").unwrap(), "", true);
        assert!(ST.kit().is_none(), "retour au plat");
    }

    #[test]
    fn procedural_est_opaque_et_porte_la_couleur_d_accent() {
        let px = procedural_backdrop(64, 36, 0xFFE8862E);
        assert!(px.chunks_exact(4).all(|p| p[3] == 255));
        // un halo ambre : le rouge (octet 2) dépasse le bleu (octet 0) quelque part
        assert!(px.chunks_exact(4).any(|p| p[2] > p[0] + 25));
    }
}
