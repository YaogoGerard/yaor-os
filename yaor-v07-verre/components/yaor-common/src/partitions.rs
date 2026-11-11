//! Découverte des slots racine installés.
//!
//! udev expose une étiquette de partition (GPT) sous forme de lien symbolique
//! dans `/dev/disk/by-partlabel/`. Les slots racine y apparaissent sous
//! `yaor-root_<version>` ; c'est cette liste qui permet à `yaor-slot` de savoir
//! quelles versions sont présentes sur le disque.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{PARTLABEL_DIR, ROOT_LABEL_PREFIX};

/// Liste triée des **versions** des slots racine présents, déduites des
/// étiquettes de partition de `dir` (par défaut `/dev/disk/by-partlabel`).
///
/// Un dossier illisible donne une liste vide : l'appelant décide s'il s'agit
/// d'une anomalie.
pub fn list_root_versions(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut versions = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if let Some(v) = name.strip_prefix(ROOT_LABEL_PREFIX) {
            if !v.is_empty() {
                versions.push(v.to_string());
            }
        }
    }
    versions.sort();
    versions.dedup();
    versions
}

/// Comme [`list_root_versions`], avec le dossier conventionnel.
pub fn list_root_versions_default() -> Vec<String> {
    list_root_versions(Path::new(PARTLABEL_DIR))
}

/// Chemin du lien udev pour une version donnée (utile pour monter une partition
/// sans dépendre de son nom de périphérique).
pub fn partlabel_path(version: &str) -> PathBuf {
    Path::new(PARTLABEL_DIR).join(format!("{ROOT_LABEL_PREFIX}{version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtre_et_trie() {
        let dir = std::env::temp_dir().join(format!("yaor-partlab-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("yaor-root_2026.10.12"), "").unwrap();
        fs::write(dir.join("yaor-root_2026.10.17"), "").unwrap();
        fs::write(dir.join("yaor-var_1"), "").unwrap();
        fs::write(dir.join("other"), "").unwrap();

        let v = list_root_versions(&dir);
        assert_eq!(v, vec!["2026.10.12".to_string(), "2026.10.17".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dossier_absent_donne_liste_vide() {
        assert!(list_root_versions(Path::new("/n-existe-pas-yaor")).is_empty());
    }
}
