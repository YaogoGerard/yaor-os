// Canvas de rendu logiciel et formes anti-aliasées (étape 2) — extrait de
// `yaor-panel` pour être partagé avec `yaor-dock`.
//
// Convention unique : un canvas est une tranche d'octets `Argb8888` garnie
// par wl_shm (petit boutisme → octets B,G,R,A dans cet ordre), de largeur
// `w` et hauteur `h` en pixels logiques. Toutes les fonctions sont pures :
// elles n'allouent pas (sauf le blit, qui lit une source) et n'échouent
// jamais (les coordonnées hors canvas sont ignorées).
//
// Les formes sont couvertes par une fonction de distance signée (SDF)
// évaluée au centre du pixel : la couverture `clamp(0.5 - d, 0, 1)` donne
// un bord adouci d'un pixel, sans table de sinus ni sur-échantillonnage.

/// Écrit un pixel opaque (aucun mélange) — usage : fonds, glyphes.
pub fn put(canvas: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32) {
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

/// Mélange un pixel (couleur `color` Argb8888) sur le canvas avec la
/// couverture `cov` (0..1) — bords anti-aliasés des formes.
pub fn blend(canvas: &mut [u8], w: i32, h: i32, x: i32, y: i32, color: u32, cov: f64) {
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
    // alpha : rendu opaque dès qu'on touche (surface du bureau)
    canvas[i + 3] = 0xFF;
}

/// Rectangle plein, sans arrondi (aucun mélange).
pub fn fill_rect(
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

/// Couverture d'un rectangle arrondi (rayon `r`) au point (px, py) —
/// SDF de rectangle arrondi, AA 1 px.
pub fn rrect_cov(px: f64, py: f64, x0: f64, y0: f64, rw: f64, rh: f64, r: f64) -> f64 {
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
pub fn fill_rrect(
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
            blend(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Rectangle arrondi sur fond **transparent** : RGB prémultiplié par la
/// couverture et alpha = couverture (convention wl_shm prémultipliée).
///
/// Utilisé pour la barre du dock, dont la surface n'est pas opaque :
/// [`fill_rrect`] forcerait l'alpha à 255 sur les pixels de bord et
/// laisserait une frange sombre autour des coins. Une seule forme ici,
/// donc pas d'empilement à composer — l'image est exacte.
pub fn fill_rrect_premul(
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
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let cov = rrect_cov(
                x as f64 + 0.5,
                y as f64 + 0.5,
                x0 as f64,
                y0 as f64,
                rw as f64,
                rh as f64,
                r as f64,
            );
            if cov <= 0.0 {
                continue;
            }
            let i = ((y * w + x) * 4) as usize;
            if i + 3 >= canvas.len() {
                continue;
            }
            for k in 0..3 {
                let fg = ((color >> (8 * k)) & 0xFF) as f64;
                canvas[i + k] = (fg * cov).round() as u8;
            }
            canvas[i + 3] = (255.0 * cov).round() as u8;
        }
    }
}

/// Couverture d'une « pilule » (rectangle à extrémités semi-circulaires,
/// rayon = h/2) au point (px, py) — SDF de rectangle arrondi, AA 1 px.
pub fn pill_cov(px: f64, py: f64, x0: f64, y0: f64, pw: f64, ph: f64) -> f64 {
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
pub fn fill_pill(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    pw: i32,
    ph: i32,
    color: u32,
) {
    for y in y0..y0 + ph {
        for x in x0..x0 + pw {
            let cov = pill_cov(
                x as f64 + 0.5,
                y as f64 + 0.5,
                x0 as f64,
                y0 as f64,
                pw as f64,
                ph as f64,
            );
            blend(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Disque plein (bords adoucis).
pub fn fill_circle(canvas: &mut [u8], w: i32, h: i32, cx: i32, cy: i32, r: i32, color: u32) {
    for y in cy - r - 1..=cy + r + 1 {
        for x in cx - r - 1..=cx + r + 1 {
            let d = ((x - cx) as f64).hypot((y - cy) as f64);
            let cov = (r as f64 + 0.5 - d).clamp(0.0, 1.0);
            blend(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Anneau (rayon `r`, épaisseur `t`) — symbole power du volet du panel.
pub fn fill_ring(canvas: &mut [u8], w: i32, h: i32, cx: i32, cy: i32, r: i32, t: i32, color: u32) {
    let lo = (r - t) as f64;
    let hi = (r + t) as f64;
    for y in cy - r - t - 1..=cy + r + t + 1 {
        for x in cx - r - t - 1..=cx + r + t + 1 {
            let d = ((x - cx) as f64).hypot((y - cy) as f64);
            let cov = (hi - 0.5 - d).clamp(0.0, 1.0) * (d - lo + 0.5).clamp(0.0, 1.0);
            blend(canvas, w, h, x, y, color, cov);
        }
    }
}

/// Blit « source over » d'une image Argb8888 (`sw` x `sh`, octets B,G,R,A) :
/// pixel transparent sauté (le fond transparaît), pixel translucide mélangé
/// à l'existant, débordements ignorés.
pub fn blit(
    canvas: &mut [u8],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    src: &[u8],
    sw: i32,
    sh: i32,
) {
    if sw <= 0 || sh <= 0 || src.len() < (sw * sh * 4) as usize {
        return;
    }
    let stride = (sw * 4) as usize;
    for row in 0..sh {
        let y = y0 + row;
        if y < 0 || y >= h {
            continue;
        }
        let slice = &src[(row as usize) * stride..(row as usize + 1) * stride];
        for col in 0..sw {
            let a = slice[(col * 4 + 3) as usize] as f64 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let x = x0 + col;
            if x < 0 || x >= w {
                continue;
            }
            let off = (y as usize * w as usize + x as usize) * 4;
            if off + 3 >= canvas.len() {
                continue;
            }
            for k in 0..3 {
                let bg = canvas[off + k] as f64;
                let fg = slice[(col * 4 + k as i32) as usize] as f64;
                canvas[off + k] = (bg + (fg - bg) * a).round() as u8;
            }
            canvas[off + 3] = 0xFF;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrect_couvre_le_centre_et_adoucit_les_coins() {
        // Rectangle (0,0) 20x20, rayon 6 : centre plein, coin extérieur vide,
        // pixel juste à l'intérieur du rayon partiellement couvert.
        assert_eq!(rrect_cov(10.5, 10.5, 0.0, 0.0, 20.0, 20.0, 6.0), 1.0);
        assert_eq!(rrect_cov(0.5, 0.5, 0.0, 0.0, 20.0, 20.0, 6.0), 0.0);
        let bord = rrect_cov(1.5, 1.5, 0.0, 0.0, 20.0, 20.0, 6.0);
        assert!(bord > 0.0 && bord < 1.0, "couverture partielle : {bord}");
    }

    #[test]
    fn fill_rrect_laissent_les_coins_intacts() {
        let mut canvas = vec![0u8; 20 * 20 * 4];
        fill_rrect(&mut canvas, 20, 20, 0, 0, 20, 20, 6, 0xFF_E8_86_2E);
        // Coin : non touché (alpha 0) ; centre : ambre opaque.
        assert_eq!(canvas[3], 0, "coin laissé vide");
        let centre = ((10 * 20 + 10) * 4) as usize;
        assert_eq!(
            &canvas[centre..centre + 4],
            &[0x2E, 0x86, 0xE8, 0xFF],
            "ambre Argb8888 au centre"
        );
    }

    #[test]
    fn rrect_premul_prémultiplie_rgb_et_alpha() {
        let mut canvas = vec![0u8; 20 * 20 * 4];
        fill_rrect_premul(&mut canvas, 20, 20, 0, 0, 20, 20, 6, 0xFF_E8_86_2E);
        let centre = ((10 * 20 + 10) * 4) as usize;
        assert_eq!(
            &canvas[centre..centre + 4],
            &[0x2E, 0x86, 0xE8, 0xFF],
            "centre : couleur intacte, alpha plein"
        );
        // Coin extérieur au rayon : ni couleur ni alpha (surface transparente).
        assert_eq!(&canvas[0..4], &[0, 0, 0, 0]);
        // Bord : RGB proportionnel à l'alpha (prémultiplié), jamais 255.
        let bord = (0..20 * 20 * 4)
            .step_by(4)
            .map(|i| canvas[i + 3])
            .find(|a| *a > 0 && *a < 255)
            .expect("un pixel de bord partiellement couvert");
        assert!(bord < 255);
    }

    #[test]
    fn blit_melange_l_alpha() {
        // Canvas noir opaque 8x8, source rouge (B,G,R,A) 4x4 à alpha 128.
        let mut canvas = vec![0u8; 8 * 8 * 4];
        for px in canvas.chunks_exact_mut(4) {
            px[3] = 0xFF;
        }
        let mut src = vec![0u8; 4 * 4 * 4];
        for px in src.chunks_exact_mut(4) {
            px[2] = 0xFF;
            px[3] = 128;
        }
        blit(&mut canvas, 8, 8, 2, 2, &src, 4, 4);
        let off = ((2 * 8) + 2) * 4;
        assert_eq!(&canvas[off..off + 4], &[0, 0, 128, 0xFF]);
    }

    #[test]
    fn blit_ignore_le_transparent_et_les_debordements() {
        let mut canvas = vec![0u8; 8 * 8 * 4];
        canvas[0..4].copy_from_slice(&[9, 9, 9, 0xFF]);
        for px in canvas[4..].chunks_exact_mut(4) {
            px[3] = 0xFF;
        }
        let src = vec![0u8; 4 * 4 * 4]; // alpha 0 partout
        blit(&mut canvas, 8, 8, 0, 0, &src, 4, 4);
        assert_eq!(canvas[0..4], [9, 9, 9, 0xFF], "transparent : intact");
        // Ancrage négatif : aucun panic, canvas inchangé en taille.
        blit(&mut canvas, 8, 8, -3, 6, &src, 4, 4);
        assert_eq!(canvas.len(), 8 * 8 * 4);
    }

    #[test]
    fn blit_refuse_une_source_trop_courte() {
        let mut canvas = vec![0u8; 4 * 4 * 4];
        blit(&mut canvas, 4, 4, 0, 0, &[0u8; 8], 4, 4);
        assert_eq!(canvas[3], 0, "source invalide : rien n'est écrit");
    }
}
