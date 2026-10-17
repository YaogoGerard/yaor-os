//! État des slots A/B, persisté sur l'ESP (`/boot/yaor/slots.state`).
//!
//! Format « `clé=valeur` », une par ligne ; les lignes vides et les
//! commentaires commençant par `#` sont ignorés. Exemple :
//!
//! ```ini
//! active=A
//! trying=
//! attempts_left=0
//! confirmed=A
//! version_a=2026.10.17
//! version_b=2026.10.12
//! ```

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::Slot;
use crate::paths;

/// État complet de la sélection A/B.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotState {
    /// Slot démarré (entrée Limine par défaut).
    pub active: Slot,
    /// Slot en cours d'essai (mise à jour non encore confirmée).
    pub trying: Option<Slot>,
    /// Essais de démarrage restants avant le retour arrière automatique.
    pub attempts_left: u32,
    /// Slot confirmé sain — cible du retour arrière.
    pub confirmed: Slot,
    /// Version hébergée par chaque slot, indexée par [`Slot::index`]. Chaîne
    /// vide tant que le slot n'a pas reçu d'image.
    pub version: [String; 2],
}

impl SlotState {
    /// État initial : `active` est aussi `confirmed`, aucun essai en cours.
    pub fn new(active: Slot) -> SlotState {
        SlotState {
            active,
            trying: None,
            attempts_left: 0,
            confirmed: active,
            version: [String::new(), String::new()],
        }
    }

    /// Version hébergée par un slot (chaîne vide si aucune).
    pub fn version_of(&self, slot: Slot) -> &str {
        &self.version[slot.index()]
    }

    /// Enregistre la version d'un slot.
    pub fn set_version(&mut self, slot: Slot, version: &str) {
        self.version[slot.index()] = version.to_string();
    }

    /// Analyse le contenu de `slots.state`. Tolère les clés inconnues (pour la
    /// compatibilité ascendante) mais refuse des valeurs de slot invalides.
    pub fn parse(text: &str) -> Result<SlotState, String> {
        let mut active = None;
        let mut confirmed = None;
        let mut trying = None;
        let mut attempts_left = 0u32;
        let mut version = [String::new(), String::new()];

        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("ligne {} : attendu « clé=valeur »", n + 1));
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "active" => {
                    active = Some(
                        Slot::parse(value)
                            .ok_or_else(|| format!("ligne {} : slot invalide « {} »", n + 1, value))?,
                    )
                }
                "confirmed" => {
                    confirmed = Some(
                        Slot::parse(value)
                            .ok_or_else(|| format!("ligne {} : slot invalide « {} »", n + 1, value))?,
                    )
                }
                "trying" => {
                    trying = if value.is_empty() {
                        None
                    } else {
                        Some(
                            Slot::parse(value).ok_or_else(|| {
                                format!("ligne {} : slot invalide « {} »", n + 1, value)
                            })?,
                        )
                    }
                }
                "attempts_left" => {
                    attempts_left = value.parse().map_err(|_| {
                        format!("ligne {} : entier invalide « {} »", n + 1, value)
                    })?
                }
                "version_a" => version[0] = value.to_string(),
                "version_b" => version[1] = value.to_string(),
                _ => {} // clé inconnue : ignorée
            }
        }

        let confirmed = confirmed.ok_or("clé « confirmed » manquante")?;
        Ok(SlotState {
            active: active.unwrap_or(confirmed),
            trying,
            attempts_left,
            confirmed,
            version,
        })
    }

    /// Sérialise l'état au format `slots.state`.
    pub fn render(&self) -> String {
        let trying = match self.trying {
            Some(s) => s.to_string(),
            None => String::new(),
        };
        format!(
            "active={}\ntrying={}\nattempts_left={}\nconfirmed={}\nversion_a={}\nversion_b={}\n",
            self.active,
            trying,
            self.attempts_left,
            self.confirmed,
            self.version[0],
            self.version[1],
        )
    }

    /// Charge l'état depuis `<esp>/yaor/slots.state`.
    pub fn load(esp: &Path) -> io::Result<SlotState> {
        let path = paths::state_file(esp);
        let text = fs::read_to_string(&path)?;
        SlotState::parse(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Enregistre l'état dans `<esp>/yaor/slots.state`, en créant le dossier si
    /// nécessaire. L'écriture passe par un fichier temporaire puis un renommage,
    /// afin qu'un plantage ne laisse jamais un état tronqué.
    pub fn save(&self, esp: &Path) -> io::Result<()> {
        let path = paths::state_file(esp);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("state.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(self.render().as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &path)?;
        // Le fichier d'état n'est lu que par root ; on évite un état lisible
        // par tous.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Confronte l'état à la liste des versions **présentes sur le disque**
    /// (issue de [`crate::partitions::list_root_versions`]) et détermine le slot
    /// qui vient d'être écrit par `systemd-sysupdate`, ainsi que sa nouvelle
    /// version.
    ///
    /// `systemd-sysupdate` réétiquette le slot qu'il remplit ; le slot visé est
    /// donc celui dont l'ancienne version a disparu — ou, lors du premier
    /// remplissage, le seul slot encore vide. Renvoie `None` si la différence
    /// n'est pas exactement celle d'un unique slot (aucun changement, ou
    /// changement ambigu).
    pub fn reconcile(&self, present: &[String]) -> Option<(Slot, String)> {
        let old: Vec<&str> = Slot::all()
            .iter()
            .map(|s| self.version_of(*s))
            .filter(|v| !v.is_empty())
            .collect();
        let added: Vec<&String> = present
            .iter()
            .filter(|v| !old.contains(&v.as_str()))
            .collect();
        if added.len() != 1 {
            return None;
        }
        let new = added[0].clone();

        let removed: Vec<Slot> = Slot::all()
            .into_iter()
            .filter(|s| {
                let v = self.version_of(*s);
                !v.is_empty() && !present.iter().any(|p| p == v)
            })
            .collect();

        let target = if removed.len() == 1 {
            removed[0]
        } else if removed.is_empty() {
            // Aucune version n'a disparu : un slot vide a été rempli.
            match (
                self.version_of(Slot::A).is_empty(),
                self.version_of(Slot::B).is_empty(),
            ) {
                (true, false) => Slot::A,
                (false, true) => Slot::B,
                _ => return None,
            }
        } else {
            return None;
        };

        Some((target, new))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn etat() -> SlotState {
        let mut s = SlotState::new(Slot::A);
        s.set_version(Slot::A, "2026.10.17");
        s.set_version(Slot::B, "2026.10.12");
        s
    }

    #[test]
    fn aller_retour_render_parse() {
        let s = etat();
        assert_eq!(SlotState::parse(&s.render()).unwrap(), s);
    }

    #[test]
    fn parse_tolere_commentaires_et_cles_inconnues() {
        let s = SlotState::parse("# commentaire\n\nactive=B\nconfirmed=B\nfuture=1\n").unwrap();
        assert_eq!(s.active, Slot::B);
        assert_eq!(s.confirmed, Slot::B);
        assert_eq!(s.trying, None);
        assert_eq!(s.attempts_left, 0);
    }

    #[test]
    fn parse_refuse_une_valeur_de_slot_invalide() {
        assert!(SlotState::parse("active=Z\nconfirmed=A\n").is_err());
        assert!(SlotState::parse("active=A\n").is_err()); // confirmed manquant
    }

    #[test]
    fn trying_vide_equivaut_a_aucun() {
        let s = SlotState::parse("active=A\nconfirmed=A\ntrying=\n").unwrap();
        assert_eq!(s.trying, None);
        let s = SlotState::parse("active=A\nconfirmed=A\ntrying=B\n").unwrap();
        assert_eq!(s.trying, Some(Slot::B));
    }

    #[test]
    fn reconcile_slot_ecrase() {
        // B (2026.10.12) remplacé par 2026.11.01 : A reste, la nouvelle
        // version apparaît.
        let s = etat();
        let present = vec!["2026.10.17".to_string(), "2026.11.01".to_string()];
        assert_eq!(s.reconcile(&present), Some((Slot::B, "2026.11.01".into())));
    }

    #[test]
    fn reconcile_premier_remplissage_d_un_slot_vide() {
        // A porte une version, B est vide ; B reçoit 2026.11.01.
        let mut s = SlotState::new(Slot::A);
        s.set_version(Slot::A, "2026.10.17");
        let present = vec!["2026.10.17".to_string(), "2026.11.01".to_string()];
        assert_eq!(s.reconcile(&present), Some((Slot::B, "2026.11.01".into())));
    }

    #[test]
    fn reconcile_sans_changement() {
        let s = etat();
        let same = vec!["2026.10.17".to_string(), "2026.10.12".to_string()];
        assert_eq!(s.reconcile(&same), None);
    }

    #[test]
    fn reconcile_ambigu() {
        // Les deux slots ont changé : cas non géré.
        let s = etat();
        let present = vec!["2026.11.01".to_string(), "2026.11.02".to_string()];
        assert_eq!(s.reconcile(&present), None);
    }
}
