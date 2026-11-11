//! Édition ciblée de `desktop.toml` (v0.7 « Verre »).
//!
//! Même principe que `yaor-ctl profile set` (v0.6) : on ne réécrit PAS le
//! fichier avec un sérialiseur TOML (il perdrait les commentaires de
//! l'utilisateur). On remplace ligne par ligne les clés demandées dans une
//! section donnée, on insère celles qui manquent en fin de section, et on
//! crée la section en fin de fichier si elle est absente. Zéro dépendance
//! (« un besoin = un fournisseur » : ctl, Réglages et scripts partagent
//! cette fonction).

use std::path::PathBuf;

/// Met une chaîne entre guillemets TOML (échappe `\` et `"`).
pub fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Vrai si `line` est l'en-tête de section `[name]` (commentaire de fin
/// toléré).
fn is_header(line: &str, name: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.strip_prefix('[') else { return false };
    let Some(end) = rest.find(']') else { return false };
    if rest[..end].trim() != name {
        return false;
    }
    let tail = rest[end + 1..].trim();
    tail.is_empty() || tail.starts_with('#')
}

fn is_any_header(line: &str) -> bool {
    line.trim_start().starts_with('[')
}

/// Vrai si `line` définit la clé `key` (`key = …`), commentaires exclus.
fn defines_key(line: &str, key: &str) -> bool {
    let t = line.trim_start();
    match t.strip_prefix(key) {
        Some(rest) => rest.trim_start().starts_with('='),
        None => false,
    }
}

/// Remplace ou insère `pairs` (clé, valeur TOML déjà formatée) dans la
/// section `[section]` de `text` et renvoie le nouveau texte.
pub fn set_keys(text: &str, section: &str, pairs: &[(&str, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let start = lines.iter().position(|l| is_header(l, section));

    let Some(start) = start else {
        // Section absente : créée en fin de fichier.
        let mut out = lines.join("\n");
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("[{section}]\n"));
        for (k, v) in pairs {
            out.push_str(&format!("{k} = {v}\n"));
        }
        return out;
    };

    // fin de section = prochain en-tête ou fin de fichier
    let end = lines[start + 1..]
        .iter()
        .position(|l| is_any_header(l))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());

    let mut insert_at = end;
    // on remonte au-dessus des lignes vides finales de la section
    while insert_at > start + 1 && lines[insert_at - 1].trim().is_empty() {
        insert_at -= 1;
    }

    let mut missing: Vec<String> = Vec::new();
    for (k, v) in pairs {
        let new_line = format!("{k} = {v}");
        match lines[start + 1..end].iter().position(|l| defines_key(l, k)) {
            Some(i) => lines[start + 1 + i] = new_line,
            None => missing.push(new_line),
        }
    }
    for (i, l) in missing.into_iter().enumerate() {
        lines.insert(insert_at + i, l);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Fichier à ÉCRIRE : le premier candidat existant — exactement celui que
/// `DesktopConfig::load()` lira (il ne fusionne pas les fichiers : écrire
/// ailleurs masquerait `/etc/yaor/desktop.toml`, profil compris) ; à
/// défaut, le premier candidat (créé). Même règle que `yaor-ctl profile set`.
pub fn target_path() -> PathBuf {
    let candidats = crate::candidate_paths();
    candidats
        .iter()
        .find(|p| p.exists())
        .or_else(|| candidats.first())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("/root/.config/yaor/desktop.toml"))
}

/// Lit le fichier cible (vide s'il n'existe pas), applique `f` et réécrit
/// atomiquement (fichier temporaire + rename). Renvoie `(chemin, ancien
/// texte)` : l'appelant peut restaurer l'ancien si la relecture échoue.
pub fn update_config(f: impl FnOnce(&str) -> String) -> std::io::Result<(PathBuf, String)> {
    let path = target_path();
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let new = f(&old);
    write_atomic(&path, &new)?;
    Ok((path, old))
}

/// Écriture atomique (tmp + rename), répertoire créé au besoin.
pub fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remplace_une_cle_et_garde_les_commentaires() {
        let t = "# mon fond\n[theme]\n# accent perso\naccent = \"#111111\" \nshadow = 0.5\n\n[dock]\nheight = 72\n";
        let o = set_keys(t, "theme", &[("accent", q("#E8862E"))]);
        assert!(o.contains("# mon fond"));
        assert!(o.contains("# accent perso"));
        assert!(o.contains("accent = \"#E8862E\""));
        assert!(!o.contains("#111111"));
        assert!(o.contains("shadow = 0.5"));
        assert!(o.contains("[dock]\nheight = 72"));
    }

    #[test]
    fn insere_les_cles_manquantes_en_fin_de_section() {
        let t = "[theme]\naccent = \"#111111\"\n\n[dock]\nheight = 72\n";
        let o = set_keys(t, "theme", &[("style", q("glass")), ("glass_blur", "24".into())]);
        let theme_pos = o.find("[theme]").unwrap();
        let dock_pos = o.find("[dock]").unwrap();
        let style_pos = o.find("style = \"glass\"").unwrap();
        assert!(theme_pos < style_pos && style_pos < dock_pos, "dans [theme]");
        assert!(o.contains("glass_blur = 24"));
    }

    #[test]
    fn cree_la_section_si_absente() {
        let o = set_keys("[dock]\nheight = 72\n", "theme", &[("style", q("glass"))]);
        assert!(o.ends_with("[theme]\nstyle = \"glass\"\n"));
        let vide = set_keys("", "theme", &[("style", q("flat"))]);
        assert_eq!(vide, "[theme]\nstyle = \"flat\"\n");
    }

    #[test]
    fn ne_confond_pas_les_prefixes_ni_les_commentaires() {
        let t = "[theme]\n# glass_blur = 99\nglass_blur_x = 1\n";
        let o = set_keys(t, "theme", &[("glass_blur", "12".into())]);
        assert!(o.contains("# glass_blur = 99"), "commentaire intact");
        assert!(o.contains("glass_blur_x = 1"), "préfixe distinct intact");
        assert!(o.contains("\nglass_blur = 12"), "clé insérée");
    }

    #[test]
    fn section_en_tete_avec_commentaire_de_fin() {
        let o = set_keys("[theme] # couleurs\naccent = \"#1\"\n", "theme", &[("accent", q("#2"))]);
        assert!(o.contains("accent = \"#2\""));
        assert_eq!(o.matches("[theme]").count(), 1);
    }

    #[test]
    fn sans_retour_ligne_final() {
        let o = set_keys("[theme]\naccent = \"#1\"", "theme", &[("accent", q("#2"))]);
        assert_eq!(o, "[theme]\naccent = \"#2\"\n");
    }

    #[test]
    fn q_echappe() {
        assert_eq!(q("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
