//! Chargement d'assets image pour le bureau YAOR (jalon v0.6 étape 1e
//! — « bureau réel »).
//!
//! Décodage PNG/JPEG par la crate `image` (vendue dans l'arbre Smithay
//! — ADR-018 : aucun accès réseau). Un besoin = un fournisseur : le
//! module expose le minimum consommé par le compositor (fond d'écran)
//! et le panel (emblème) — pas de dossier d'assets, pas de cache
//! disque : un chemin lu depuis `desktop.toml`, un RGBA8 en mémoire.
//!
//! Toute erreur (fichier absent, format inconnu) renvoie `None` : le
//! consommateur applique son repli (dégradé procédural, bitmap
//! incorporé) — le bureau démarre toujours.

use image::ImageReader;

/// Image décodée en RGBA8 (une ligne = `w * 4` octets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRgba {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

/// Fond d'écran plein écran `w`×`h` en **Argb8888 opaque**, prêt à être
/// flouté par `yaor_ui::glass` (v0.7). `cover` : cadrage « cover »
/// (sinon « fit », marges noires). `None` si le chemin est vide ou
/// l'image illisible — l'appelant retombe alors sur le fond procédural.
pub fn wallpaper_argb(path: &str, cover_fit: bool, w: u32, h: u32) -> Option<Vec<u8>> {
    if path.is_empty() {
        return None;
    }
    let img = load_image(std::path::Path::new(path))?;
    let fitted = if cover_fit { cover(&img, w, h) } else { contain(&img, w, h) };
    let mut px = fitted.to_argb8888();
    for p in px.chunks_exact_mut(4) {
        p[3] = 0xFF;
    }
    Some(px)
}

impl ImageRgba {
    /// Pixel (x, y) → (r, g, b, a) ; (0, 0, 0, 0) hors bornes.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.w || y >= self.h {
            return [0, 0, 0, 0];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }
}

/// Décode une image depuis le disque (PNG, JPEG — formats activés à
/// la compilation) et la convertit en RGBA8. `None` si le fichier
/// manque ou est illisible (repli du consommateur).
pub fn load_image(path: &std::path::Path) -> Option<ImageRgba> {
    let file = std::fs::File::open(path).ok()?;
    let reader = ImageReader::new(std::io::BufReader::new(file))
        .with_guessed_format()
        .ok()?;
    let img = reader.decode().ok()?;
    let rgba = img.to_rgba8();
    Some(ImageRgba {
        w: rgba.width(),
        h: rgba.height(),
        data: rgba.into_raw(),
    })
}

/// Ré-échantillonne (lanczos3) vers la taille demandée — déformation
/// simple (les consommateurs calculent eux-mêmes la fenêtre « cover »).
pub fn resize(img: &ImageRgba, w: u32, h: u32) -> ImageRgba {
    let src = image::RgbaImage::from_raw(img.w, img.h, img.data.clone())
        .expect("ImageRgba → RgbaImage");
    let dst = image::imageops::resize(&src, w.max(1), h.max(1), image::imageops::FilterType::Lanczos3);
    ImageRgba {
        w: dst.width(),
        h: dst.height(),
        data: dst.into_raw(),
    }
}

/// Fenêtre « cover » : le plus grand rectangle de ratio `iw/ih` qui
/// couvre `(tw, th)` — centre + recadrage.
pub fn cover_size(iw: u32, ih: u32, tw: u32, th: u32) -> (u32, u32) {
    // La source est plus « large » que la cible → la hauteur impose
    // la taille ; plus « haute » → la largeur l'impose.
    if ih * tw >= iw * th {
        (tw, (ih * tw) / iw.max(1))
    } else {
        ((iw * th) / ih.max(1), th)
    }
}

/// Rend `img` en exactement `tw` x `th` en **couvrant** la cible :
/// agrandissement au plus petit facteur qui couvre (cf. [`cover_size`])
/// puis recadrage centré du débordement — aucun bord visible. C'est la
/// mise à l'échelle `cover` du fond d'écran (1e.fond).
pub fn cover(img: &ImageRgba, tw: u32, th: u32) -> ImageRgba {
    let (tw, th) = (tw.max(1), th.max(1));
    let (sw, sh) = cover_size(img.w.max(1), img.h.max(1), tw, th);
    let scaled = resize(img, sw, sh);
    crop_centered(&scaled, tw, th)
}

/// Rend `img` en exactement `tw` x `th` en la **contenant** : échelle au
/// plus grand facteur qui la fait tenir, centrée, marges transparentes
/// (le consommateur compose sur son propre fond) — mise à l'échelle
/// `fit` du fond d'écran (1e.fond).
pub fn contain(img: &ImageRgba, tw: u32, th: u32) -> ImageRgba {
    let (tw, th) = (tw.max(1), th.max(1));
    // Contrainte de largeur (tw/iw <= th/ih) comparée sans flottants.
    let (sw, sh) = if tw * img.h.max(1) <= th * img.w.max(1) {
        (tw, (img.h * tw) / img.w.max(1))
    } else {
        ((img.w * th) / img.h.max(1), th)
    };
    let scaled = resize(img, sw.max(1), sh.max(1));
    let (ox, oy) = ((tw.saturating_sub(scaled.w)) / 2, (th.saturating_sub(scaled.h)) / 2);
    let mut out = ImageRgba {
        w: tw,
        h: th,
        data: vec![0u8; (tw * th * 4) as usize],
    };
    for y in 0..scaled.h.min(th) {
        for x in 0..scaled.w.min(tw) {
            let px = scaled.pixel(x, y);
            let i = (((oy + y) * tw + (ox + x)) * 4) as usize;
            out.data[i..i + 4].copy_from_slice(&px);
        }
    }
    out
}

/// Recadrage centré `tw` x `th` (pixels manquants laissés transparents).
fn crop_centered(img: &ImageRgba, tw: u32, th: u32) -> ImageRgba {
    let ox = img.w.saturating_sub(tw) / 2;
    let oy = img.h.saturating_sub(th) / 2;
    let mut data = vec![0u8; (tw * th * 4) as usize];
    let cw = tw.min(img.w.saturating_sub(ox));
    let ch = th.min(img.h.saturating_sub(oy));
    for y in 0..ch {
        for x in 0..cw {
            let px = img.pixel(ox + x, oy + y);
            let i = ((y * tw + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&px);
        }
    }
    ImageRgba { w: tw, h: th, data }
}

impl ImageRgba {
    /// Octets d'un buffer **Argb8888** (ordre mémoire B, G, R, A —
    /// petit boutisme, celui de `MemoryRenderBuffer` et de wl_shm).
    /// L'alpha est celui de la source : l'appelant compose (fond
    /// d'écran) ou force l'opacité (canevas de panneau).
    pub fn to_argb8888(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len());
        for px in self.data.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_paysage_vers_ecran_large() {
        // 2050x1367 vers 1280x800 : ratio source (1.50) < ratio cible
        // (1.60) → la largeur impose la taille.
        let (w, h) = cover_size(2050, 1367, 1280, 800);
        assert_eq!((w, h), (1280, (1367 * 1280) / 2050));
    }

    #[test]
    fn cover_portrait_depasse_en_hauteur() {
        // 1000x2000 vers 1280x800 : la source est plus « haute » que la
        // cible (ratio 0.5 < 1.6) → la largeur impose la taille et la
        // hauteur DÉBORDE (2 560 > 800) : c'est ce débordement que le
        // recadrage centré supprime (couverture complète de la cible).
        let (w, h) = cover_size(1000, 2000, 1280, 800);
        assert_eq!((w, h), (1280, 2560));
        assert!(w >= 1280 && h >= 800, "cover couvre la cible");
    }

    #[test]
    fn cover_rend_exactement_la_cible() {
        // Image unie 100x100 → 64x32 : recadrage centré, taille exacte.
        let img = ImageRgba {
            w: 100,
            h: 100,
            data: vec![10, 20, 30, 255].repeat(100 * 100),
        };
        let out = cover(&img, 64, 32);
        assert_eq!((out.w, out.h), (64, 32));
        assert_eq!(out.data.len(), 64 * 32 * 4);
        assert_eq!(out.pixel(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn contain_centre_et_laisse_des_marges() {
        // Source 200x100 (2:1) dans 100x100 : tient en 100x50, centrée
        // verticalement → marges transparentes en haut et en bas.
        let img = ImageRgba {
            w: 200,
            h: 100,
            data: vec![1, 2, 3, 255].repeat(200 * 100),
        };
        let out = contain(&img, 100, 100);
        assert_eq!((out.w, out.h), (100, 100));
        assert_eq!(out.pixel(50, 0)[3], 0, "marge haute transparente");
        assert_eq!(out.pixel(50, 50)[3], 255, "centre opaque");
        assert_eq!(out.pixel(50, 99)[3], 0, "marge basse transparente");
    }

    #[test]
    fn argb8888_inverse_r_et_b() {
        let img = ImageRgba {
            w: 2,
            h: 1,
            data: vec![11, 22, 33, 44, 55, 66, 77, 88],
        };
        assert_eq!(img.to_argb8888(), vec![33, 22, 11, 44, 77, 66, 55, 88]);
    }

    #[test]
    fn pixel_hors_bornes_transparent() {
        let img = ImageRgba { w: 1, h: 1, data: vec![9, 9, 9, 255] };
        assert_eq!(img.pixel(0, 0), [9, 9, 9, 255]);
        assert_eq!(img.pixel(5, 5), [0, 0, 0, 0]);
    }
}
