// Thème « ambre sur noir » partagé par les clients du bureau (étape 2) —
// extrait de `yaor-panel`, désormais lu par le panel **et** le dock.
//
// Couleurs Argb8888 (0xAARRGGBB, converties en octets B,G,R,A au rendu)
// lues dans `[theme]` de `desktop.toml` : même contrat que le compositor,
// donc même crate `yaor-config`, aucun thème dupliqué. Une chaîne
// invalide retombe sur la palette canonique du branding **couleur par
// couleur** (jamais sur du noir par accident).

/// Palette du bureau : les sept couleurs du design system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub panel_bg: u32,
    pub accent: u32,
    pub accent_dark: u32,
    pub button_bg: u32,
    pub text_fg: u32,
    pub text_gray: u32,
    pub separator: u32,
}

impl Theme {
    /// Palette canonique (docs/05-ui-ux/branding/) : repli de chaque
    /// couleur et valeurs d'usine de `default.toml`.
    pub const CANONIQUE: Theme = Theme {
        panel_bg: 0xFF_10_10_14,
        accent: 0xFF_E8_86_2E,
        accent_dark: 0xFF_B9_6A_1F,
        button_bg: 0xFF_26_26_2E,
        text_fg: 0xFF_F2_E8_DC,
        text_gray: 0xFF_8A_8A_92,
        separator: 0xFF_2A_2A_33,
    };

    /// Résout `[theme]` : chaque couleur valide est reprise, chaque
    /// chaîne invalide retombe sur la canonique correspondante.
    pub fn from_config(t: &yaor_config::ThemeConfig) -> Theme {
        let d = Theme::CANONIQUE;
        let c = |s: &str, repli: u32| yaor_config::parse_color(s).unwrap_or(repli);
        Theme {
            panel_bg: c(&t.panel_bg, d.panel_bg),
            accent: c(&t.accent, d.accent),
            accent_dark: c(&t.accent_dark, d.accent_dark),
            button_bg: c(&t.button_bg, d.button_bg),
            text_fg: c(&t.text_fg, d.text_fg),
            text_gray: c(&t.text_gray, d.text_gray),
            separator: c(&t.separator, d.separator),
        }
    }

    /// Liseré des surfaces au repos (pilules de la taskbar, tuiles du
    /// dock) : gris secondaire atténué — le thème ne porte pas de
    /// couleur dédiée.
    pub fn rest_border(&self) -> u32 {
        dim(self.text_gray, 0.62)
    }
}

/// Atténue les composantes RVB d'une couleur 0xAARRGGBB (alpha
/// conservé) — dérivations de palette (liseré au repos, survol).
pub fn dim(color: u32, factor: f64) -> u32 {
    let f = factor.clamp(0.0, 1.0);
    let ch = |shift: u32| {
        let v = ((color >> shift) & 0xFF) as f64 * f;
        ((v.round() as u32) & 0xFF) << shift
    };
    (color & 0xFF00_0000) | ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dim_attenue_les_composantes_sans_toucher_l_alpha() {
        assert_eq!(dim(0xFF8A8A92, 1.0), 0xFF8A8A92);
        assert_eq!(dim(0xFF8A8A92, 0.5), 0xFF454549);
        assert_eq!(dim(0xFFFF_FFFF, 0.0), 0xFF000000);
        assert_eq!(dim(0x808A8A92, 2.0), 0x808A8A92, "facteur borné à 1");
    }

    #[test]
    fn theme_replie_couleur_par_couleur() {
        let cfg = yaor_config::ThemeConfig {
            accent: "#00AAFF".to_string(),
            panel_bg: "invalide".to_string(),
            ..Default::default()
        };
        let t = Theme::from_config(&cfg);
        assert_eq!(t.accent, 0xFF00AAFF, "couleur valide reprise");
        assert_eq!(t.panel_bg, Theme::CANONIQUE.panel_bg, "repli canonique");
        assert_eq!(t.text_fg, Theme::CANONIQUE.text_fg, "champ absent : défaut");
    }

    #[test]
    fn liseré_au_repos_derive_du_gris_secondaire() {
        let t = Theme::CANONIQUE;
        assert_eq!(t.rest_border(), dim(t.text_gray, 0.62));
        assert_ne!(t.rest_border(), t.text_gray);
    }
}
