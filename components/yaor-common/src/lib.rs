//! Code partagé par les composants internes YAOR (ADR-002).
//!
//! Réunit :
//!
//! - l'**état des slots A/B** persisté sur l'ESP (`/boot/yaor/slots.state`) ;
//! - la manipulation de **`limine.conf`** (Limine 8) ;
//! - la découverte des **slots racine** installés.
//!
//! Aucune dépendance externe : le module n'utilise que la bibliothèque
//! standard, ce qui garantit un build hors-ligne (ADR-018) et reproductible.

pub mod limine;
pub mod partitions;
pub mod slot;
pub mod state;

pub use slot::Slot;
pub use state::SlotState;

/// Nombre d'essais de démarrage accordés à un nouveau slot avant le retour
/// arrière automatique (voir ADR-015 et `yaor-slot`).
pub const DEFAULT_ATTEMPTS: u32 = 3;

/// Préfixe des étiquettes (GPT) des slots racine. L'étiquette complète porte la
/// **version** : `yaor-root_<version>` (voir le LLD installateur).
pub const ROOT_LABEL_PREFIX: &str = "yaor-root_";

/// Sous-répertoire de l'ESP regroupant les fichiers de démarrage YAOR : noyaux,
/// initramfs et état des slots (`/boot/yaor`).
pub const ESP_YAOR_DIR: &str = "yaor";

/// Protocole Limine désignant la partition d'amorçage — `$BOOT`, c'est-à-dire
/// l'ESP dans le schéma YAOR (`boot():`).
pub const LIMINE_BOOT: &str = "boot():";

/// Chemin du noyau d'une version, tel qu'écrit dans `limine.conf` :
/// `boot():/yaor/vmlinuz-<version>`. Le noyau d'un slot porte la **version**
/// du slot, comme son étiquette de partition (voir le LLD installateur).
pub fn limine_kernel_path(version: &str) -> String {
    format!("{LIMINE_BOOT}/{ESP_YAOR_DIR}/vmlinuz-{version}")
}

/// Emplacement conventionnel du répertoire des étiquettes de partition, peuplé
/// par udev.
pub const PARTLABEL_DIR: &str = "/dev/disk/by-partlabel";

/// Point de montage de l'ESP sur le système installé.
pub const ESP_MOUNT: &str = "/boot";

/// Chemins, relatifs à la racine de l'ESP.
pub mod paths {
    use std::path::{Path, PathBuf};

    /// Fichier d'état des slots (`/boot/yaor/slots.state`).
    pub const STATE: &str = "yaor/slots.state";
    /// Configuration Limine, à la racine de l'ESP (`/boot/limine.conf`).
    pub const LIMINE_CONF: &str = "limine.conf";

    /// `/boot/yaor/slots.state` pour une racine d'ESP donnée.
    pub fn state_file(esp: &Path) -> PathBuf {
        esp.join(STATE)
    }

    /// `/boot/limine.conf` pour une racine d'ESP donnée.
    pub fn limine_conf(esp: &Path) -> PathBuf {
        esp.join(LIMINE_CONF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemin_noyau_limine() {
        assert_eq!(limine_kernel_path("0.2"), "boot():/yaor/vmlinuz-0.2");
    }

    #[test]
    fn fichier_etat_sous_yaor() {
        assert_eq!(paths::STATE, "yaor/slots.state");
        assert_eq!(paths::state_file(std::path::Path::new("/boot")), std::path::PathBuf::from("/boot/yaor/slots.state"));
    }
}
