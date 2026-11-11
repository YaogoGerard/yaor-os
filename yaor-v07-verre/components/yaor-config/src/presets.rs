//! Présets de thème v0.7 « Verre » : `flat` (rendu v0.6, conforme aux PNG
//! du design) et quatre variantes de verre. Un préset est un
//! `ThemeConfig` complet ; l'appliquer = écrire ses clés dans `[theme]`
//! (voir [`apply_to_text`]) puis envoyer SIGHUP — script, `yaor-ctl theme
//! set` et l'app Réglages passent par le même chemin.

use crate::{edit, ThemeConfig};

/// Noms des présets, dans l'ordre d'affichage des cartes de Réglages.
pub const NAMES: &[&str] = &[
    "flat",
    "glass-ambre",
    "glass-glacier",
    "glass-amethyste",
    "glass-emeraude",
];

/// Libellé affichable d'un préset.
pub fn label(name: &str) -> &'static str {
    match name {
        "flat" => "Classique (plat)",
        "glass-ambre" => "Verre ambre",
        "glass-glacier" => "Verre glacier",
        "glass-amethyste" => "Verre améthyste",
        "glass-emeraude" => "Verre émeraude",
        _ => "Personnalisé",
    }
}

#[allow(clippy::too_many_arguments)]
fn glass(
    name: &str,
    accent: &str,
    accent_dark: &str,
    panel_bg: &str,
    button_bg: &str,
    text_fg: &str,
    text_gray: &str,
    separator: &str,
    tint: &str,
    border: &str,
) -> ThemeConfig {
    ThemeConfig {
        name: name.to_string(),
        style: "glass".to_string(),
        accent: accent.to_string(),
        accent_dark: accent_dark.to_string(),
        panel_bg: panel_bg.to_string(),
        button_bg: button_bg.to_string(),
        text_fg: text_fg.to_string(),
        text_gray: text_gray.to_string(),
        separator: separator.to_string(),
        glass_tint: tint.to_string(),
        glass_border: border.to_string(),
        corner_radius: 14,
        shadow: 0.70,
        glass_blur: 24,
        glass_opacity: 0.52,
        glass_saturation: 1.40,
        glass_highlight: 0.55,
        glass_grain: 0.03,
        anim_ms: 150,
    }
}

/// Résout un préset par son nom (insensible à la casse).
pub fn preset(name: &str) -> Option<ThemeConfig> {
    match name.trim().to_ascii_lowercase().as_str() {
        "flat" => Some(ThemeConfig::default()),
        "glass-ambre" => Some(glass(
            "glass-ambre", "#E8862E", "#B96A1F", "#101014", "#26262E",
            "#F2E8DC", "#8A8A92", "#2A2A33", "#17120E", "#FFE6C833",
        )),
        "glass-glacier" => Some(glass(
            "glass-glacier", "#5AB8FF", "#2F7FBF", "#0B1018", "#1B2533",
            "#E6F1FA", "#8797A8", "#223044", "#0A1420", "#CFE8FF38",
        )),
        "glass-amethyste" => Some(glass(
            "glass-amethyste", "#B48CFF", "#7F58C9", "#120E1A", "#261E36",
            "#EEE6FA", "#9890A8", "#2E2540", "#130E1E", "#E4D4FF38",
        )),
        "glass-emeraude" => Some(glass(
            "glass-emeraude", "#3DDC97", "#1F9C6A", "#0A1411", "#16261F",
            "#E3F5EC", "#85A094", "#1F332A", "#091612", "#C8FFE638",
        )),
        _ => None,
    }
}

fn f(v: f64) -> String {
    // 0.5 → « 0.5 », 1.0 → « 1.0 » : toujours un flottant TOML valide.
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') { s } else { format!("{s}.0") }
}

/// Toutes les clés `[theme]` d'un thème, prêtes pour `edit::set_keys`.
pub fn keys(t: &ThemeConfig) -> Vec<(&'static str, String)> {
    vec![
        ("name", edit::q(&t.name)),
        ("style", edit::q(&t.style)),
        ("accent", edit::q(&t.accent)),
        ("accent_dark", edit::q(&t.accent_dark)),
        ("panel_bg", edit::q(&t.panel_bg)),
        ("button_bg", edit::q(&t.button_bg)),
        ("text_fg", edit::q(&t.text_fg)),
        ("text_gray", edit::q(&t.text_gray)),
        ("separator", edit::q(&t.separator)),
        ("corner_radius", t.corner_radius.to_string()),
        ("shadow", f(t.shadow)),
        ("glass_blur", t.glass_blur.to_string()),
        ("glass_opacity", f(t.glass_opacity)),
        ("glass_saturation", f(t.glass_saturation)),
        ("glass_tint", edit::q(&t.glass_tint)),
        ("glass_border", edit::q(&t.glass_border)),
        ("glass_highlight", f(t.glass_highlight)),
        ("glass_grain", f(t.glass_grain)),
        ("anim_ms", t.anim_ms.to_string()),
    ]
}

/// Applique un préset au texte d'un `desktop.toml` (section `[theme]`).
/// `None` si le nom est inconnu.
pub fn apply_to_text(text: &str, name: &str) -> Option<String> {
    let t = preset(name)?;
    Some(edit::set_keys(text, "theme", &keys(&t)))
}

/// Bornes des curseurs de Réglages et de `yaor-ctl theme tune`.
pub fn clamp_theme(t: &mut ThemeConfig) {
    t.glass_blur = t.glass_blur.min(48);
    t.glass_opacity = t.glass_opacity.clamp(0.10, 0.95);
    t.glass_saturation = t.glass_saturation.clamp(0.5, 2.5);
    t.glass_highlight = t.glass_highlight.clamp(0.0, 1.0);
    t.glass_grain = t.glass_grain.clamp(0.0, 1.0);
    t.corner_radius = t.corner_radius.clamp(2, 24);
    t.shadow = t.shadow.clamp(0.0, 1.0);
    t.anim_ms = t.anim_ms.min(600);
}

/// Clés réglables à l'unité (ordre d'affichage de `theme get`).
pub const TUNABLE: &[&str] = &[
    "style", "accent", "accent_dark", "panel_bg", "button_bg", "text_fg", "text_gray",
    "separator", "corner_radius", "shadow", "glass_blur", "glass_opacity",
    "glass_saturation", "glass_tint", "glass_border", "glass_highlight", "glass_grain",
    "anim_ms",
];

/// Valeur d'une clé sous forme d'un texte lisible (sans guillemets).
pub fn get_key(t: &ThemeConfig, k: &str) -> Option<String> {
    keys(t).into_iter().find(|(name, _)| *name == k).map(|(_, v)| v.trim_matches('"').to_string())
}

/// Pose une clé depuis du texte (script, CLI). Validation stricte : un
/// style inconnu, une couleur invalide ou un nombre illisible sont
/// REFUSÉS (jamais de repli silencieux). Les bornes sont appliquées
/// ensuite par [`clamp_theme`].
pub fn set_key(t: &mut ThemeConfig, k: &str, v: &str) -> Result<(), String> {
    let v = v.trim();
    let color = |v: &str| -> Result<String, String> {
        if crate::parse_color(v).is_some() {
            Ok(v.to_string())
        } else {
            Err(format!("couleur invalide : {v} (attendu #RRGGBB ou #RRGGBBAA)"))
        }
    };
    let uint = |v: &str| v.parse::<u32>().map_err(|_| format!("entier attendu : {v}"));
    let float = |v: &str| {
        v.parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .ok_or_else(|| format!("nombre attendu : {v}"))
    };
    match k {
        "style" => match v {
            "flat" | "glass" => t.style = v.to_string(),
            _ => return Err(format!("style inconnu : {v} (flat | glass)")),
        },
        "accent" => t.accent = color(v)?,
        "accent_dark" => t.accent_dark = color(v)?,
        "panel_bg" => t.panel_bg = color(v)?,
        "button_bg" => t.button_bg = color(v)?,
        "text_fg" => t.text_fg = color(v)?,
        "text_gray" => t.text_gray = color(v)?,
        "separator" => t.separator = color(v)?,
        "glass_tint" => t.glass_tint = if v.is_empty() { String::new() } else { color(v)? },
        "glass_border" => t.glass_border = color(v)?,
        "corner_radius" => t.corner_radius = uint(v)?,
        "glass_blur" => t.glass_blur = uint(v)?,
        "anim_ms" => t.anim_ms = uint(v)?,
        "shadow" => t.shadow = float(v)?,
        "glass_opacity" => t.glass_opacity = float(v)?,
        "glass_saturation" => t.glass_saturation = float(v)?,
        "glass_highlight" => t.glass_highlight = float(v)?,
        "glass_grain" => t.glass_grain = float(v)?,
        _ => return Err(format!("clé inconnue : {k}")),
    }
    Ok(())
}

/// Applique des réglages `clé=valeur` au `[theme]` d'un desktop.toml :
/// valide, borne, passe le nom à « custom » et n'écrit QUE les clés
/// touchées (le reste du fichier et du thème est conservé).
pub fn tune_text(text: &str, changes: &[(&str, &str)]) -> Result<String, String> {
    let cfg = crate::DesktopConfig::parse_str(text).map_err(|e| format!("desktop.toml illisible : {e}"))?;
    let mut t = cfg.theme;
    let mut touched: Vec<&str> = Vec::new();
    for (k, v) in changes {
        set_key(&mut t, k, v)?;
        touched.push(k);
    }
    clamp_theme(&mut t);
    t.name = "custom".to_string();
    let all = keys(&t);
    let mut pairs: Vec<(&str, String)> = Vec::new();
    for (k, v) in all {
        if k == "name" || touched.contains(&k) {
            pairs.push((k, v));
        }
    }
    Ok(edit::set_keys(text, "theme", &pairs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_color, DesktopConfig};

    #[test]
    fn tous_les_presets_se_resolvent_et_ont_des_couleurs_valides() {
        for n in NAMES {
            let t = preset(n).unwrap_or_else(|| panic!("préset {n}"));
            assert_eq!(&t.name, n);
            for c in [&t.accent, &t.accent_dark, &t.panel_bg, &t.button_bg, &t.text_fg, &t.text_gray, &t.separator, &t.glass_border] {
                assert!(parse_color(c).is_some(), "{n}: couleur {c}");
            }
            if !t.glass_tint.is_empty() {
                assert!(parse_color(&t.glass_tint).is_some(), "{n}: teinte");
            }
        }
        assert!(preset("inconnu").is_none());
    }

    #[test]
    fn flat_est_exactement_le_defaut_v06() {
        let t = preset("flat").unwrap();
        assert_eq!(t, ThemeConfig::default());
        assert!(!t.is_glass());
        assert_eq!(t.accent, "#E8862E");
    }

    #[test]
    fn apply_round_trip_par_le_vrai_parseur() {
        for n in NAMES {
            let txt = apply_to_text("# perso\n[dock]\nheight = 80\n", n).unwrap();
            let cfg = DesktopConfig::parse_str(&txt).unwrap_or_else(|e| panic!("{n}: {e}\n{txt}"));
            assert_eq!(cfg.theme, preset(n).unwrap(), "{n}");
            assert_eq!(cfg.dock.height, 80, "autres sections intactes");
            assert!(txt.contains("# perso"));
        }
    }

    #[test]
    fn apply_est_idempotent_et_ecrase_l_ancien_preset() {
        let a = apply_to_text("", "glass-glacier").unwrap();
        let b = apply_to_text(&a, "glass-glacier").unwrap();
        assert_eq!(a, b);
        let c = apply_to_text(&a, "flat").unwrap();
        let cfg = DesktopConfig::parse_str(&c).unwrap();
        assert!(!cfg.theme.is_glass());
        assert_eq!(c.matches("[theme]").count(), 1);
    }

    #[test]
    fn clamp_borne_les_curseurs() {
        let mut t = ThemeConfig { glass_blur: 999, glass_opacity: 5.0, corner_radius: 99, ..Default::default() };
        clamp_theme(&mut t);
        assert_eq!((t.glass_blur, t.corner_radius), (48, 24));
        assert_eq!(t.glass_opacity, 0.95);
    }

    #[test]
    fn tune_valide_borne_et_n_ecrit_que_le_necessaire() {
        let base = apply_to_text("# garde\n", "glass-ambre").unwrap();
        let o = tune_text(&base, &[("glass_blur", "999"), ("accent", "#112233")]).unwrap();
        let cfg = DesktopConfig::parse_str(&o).unwrap();
        assert_eq!(cfg.theme.glass_blur, 48, "borné");
        assert_eq!(cfg.theme.accent, "#112233");
        assert_eq!(cfg.theme.name, "custom");
        assert_eq!(cfg.theme.glass_opacity, preset("glass-ambre").unwrap().glass_opacity, "intact");
        assert!(o.contains("# garde"));
    }

    #[test]
    fn tune_refuse_sans_repli_silencieux() {
        assert!(tune_text("", &[("accent", "rouge")]).is_err());
        assert!(tune_text("", &[("style", "liquide")]).is_err());
        assert!(tune_text("", &[("glass_blur", "-3")]).is_err());
        assert!(tune_text("", &[("inconnue", "1")]).is_err());
        assert!(tune_text("[theme
", &[("glass_blur", "3")]).is_err(), "fichier illisible");
    }

    #[test]
    fn get_key_et_tunable_coherents() {
        let t = preset("glass-glacier").unwrap();
        assert_eq!(get_key(&t, "accent").as_deref(), Some("#5AB8FF"));
        assert_eq!(get_key(&t, "glass_blur").as_deref(), Some("24"));
        for k in TUNABLE {
            assert!(get_key(&t, k).is_some(), "{k}");
            let mut c = t.clone();
            let v = get_key(&t, k).unwrap();
            assert!(set_key(&mut c, k, &v).is_ok(), "aller-retour {k}={v}");
        }
    }

    #[test]
    fn partiel_v06_reste_valable_et_plat() {
        let cfg = DesktopConfig::parse_str("[theme]\naccent = \"#00AAFF\"\n").unwrap();
        assert!(!cfg.theme.is_glass());
        assert_eq!(cfg.theme.anim_ms, 150);
    }
}
