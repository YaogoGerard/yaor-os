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
