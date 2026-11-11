// Verre côté compositor (v0.7 « Verre »).
//
// Les barres de titre des fenêtres décorées (SSD) sont peintes avec le même
// matériau que le panel et le dock (`yaor_ui::glass`) : fond d'écran flouté
// UNE fois, découpé à la position de la fenêtre. Ce module garde ce fond
// flouté en cache, clé (taille de sortie, fond, cadrage, version du thème) :
// un SIGHUP qui change le thème ou le fond le reconstruit, jamais par image.
//
// Limite assumée : le flou porte sur le FOND D'ÉCRAN, pas sur les fenêtres
// qui passeraient derrière la barre (un vrai flou « backdrop » multi-couches
// demande un passage shader sur le framebuffer — hors périmètre v0.7).

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use yaor_ui::glass::{Backdrop, GlassParams};

type Key = (i32, i32, String, crate::config::ImageFit, u64);

struct Entry {
    key: Key,
    backdrop: Arc<Backdrop>,
}

static CACHE: Mutex<Option<Entry>> = Mutex::new(None);
/// Taille de la sortie principale : (largeur << 32) | hauteur ; 0 = inconnue.
static OUTPUT: AtomicU64 = AtomicU64::new(0);

/// Mémorise la taille de la sortie (appelé par le rendu du fond d'écran).
pub fn note_output_size(w: i32, h: i32) {
    if w > 0 && h > 0 {
        OUTPUT.store(((w as u64) << 32) | h as u64, Ordering::Relaxed);
    }
}

fn output_size() -> Option<(i32, i32)> {
    let v = OUTPUT.load(Ordering::Relaxed);
    (v != 0).then(|| ((v >> 32) as i32, (v & 0xFFFF_FFFF) as i32))
}

/// Fond flouté + paramètres du thème, ou `None` (thème plat, ou taille de
/// sortie pas encore connue : l'appelant retombe sur le rendu plat).
pub fn material() -> Option<(Arc<Backdrop>, GlassParams)> {
    let params = crate::config::glass_params()?;
    let (w, h) = output_size()?;
    let wall = crate::config::wallpaper();
    let key: Key = (w, h, wall.path.clone(), wall.fit, crate::config::theme_epoch());
    let mut cache = CACHE.lock().ok()?;
    if let Some(e) = cache.as_ref() {
        if e.key == key {
            return Some((e.backdrop.clone(), params));
        }
    }
    let px = crate::render::wallpaper_pixels(w, h, &wall);
    let backdrop = Arc::new(Backdrop::new(&px, w, h, params.blur, params.saturation));
    tracing::info!("verre : fond flouté {}x{} (flou {} px)", w, h, params.blur);
    *cache = Some(Entry { key, backdrop: backdrop.clone() });
    Some((backdrop, params))
}

/// Facteur d'animation d'ouverture 0..1 (ease-out cubique) pour une
/// fenêtre apparue il y a `elapsed_ms`. `anim_ms == 0` : toujours 1.
pub fn open_progress(elapsed_ms: u64, anim_ms: u32) -> f32 {
    if anim_ms == 0 {
        return 1.0;
    }
    let t = (elapsed_ms as f32 / anim_ms as f32).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progression_monotone_bornee() {
        assert_eq!(open_progress(0, 150), 0.0);
        assert_eq!(open_progress(150, 150), 1.0);
        assert_eq!(open_progress(9_999, 150), 1.0);
        assert_eq!(open_progress(10, 0), 1.0, "animations coupées");
        let mut prev = 0.0;
        for ms in (0..=150).step_by(10) {
            let v = open_progress(ms, 150);
            assert!(v >= prev);
            prev = v;
        }
        assert!(open_progress(75, 150) > 0.5, "ease-out : démarre vite");
    }
}
