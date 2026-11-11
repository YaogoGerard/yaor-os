//! Lecture et écriture minimale de `limine.conf` (Limine 8).
//!
//! Le format retenu par YAOR est volontairement simple : un **préambule**
//! d'options globales (`timeout:`, `default_entry:`, …) puis des **entrées**
//! introduites par `/`. Chaque entrée porte un nom et un corps de lignes
//! indentées (`protocol:`, `path:`, `cmdline:`, …).
//!
//! `yaor-slot` s'appuie sur la convention suivante : l'entrée d'indice `i`
//! correspond au slot d'indice `i` (`A` puis `B`, voir [`crate::Slot`]).

/// Une entrée de menu Limine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Nom affiché (sans le `/` initial).
    pub name: String,
    /// Lignes du corps, indentation comprise.
    pub body: Vec<String>,
}

/// Un fichier `limine.conf` décomposé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Options globales précédant la première entrée.
    pub preamble: Vec<String>,
    /// Entrées de menu, dans l'ordre du fichier.
    pub entries: Vec<Entry>,
}

impl Config {
    /// Analyse un `limine.conf`.
    pub fn parse(text: &str) -> Config {
        let mut preamble = Vec::new();
        let mut entries: Vec<Entry> = Vec::new();
        for raw in text.lines() {
            let trimmed = raw.trim_start();
            if let Some(rest) = trimmed.strip_prefix('/') {
                entries.push(Entry {
                    name: rest.trim().to_string(),
                    body: Vec::new(),
                });
            } else if let Some(last) = entries.last_mut() {
                last.body.push(raw.to_string());
            } else {
                preamble.push(raw.to_string());
            }
        }
        Config { preamble, entries }
    }

    /// Sérialise la configuration.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.preamble {
            out.push_str(line);
            out.push('\n');
        }
        for e in &self.entries {
            out.push('/');
            out.push_str(&e.name);
            out.push('\n');
            for line in &e.body {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    /// Indice (1-based, comme Limine) de l'entrée par défaut, si déclarée.
    pub fn default_entry(&self) -> Option<usize> {
        self.preamble.iter().find_map(|l| {
            let t = l.trim();
            let (k, v) = t.split_once(':')?;
            if k.trim().eq_ignore_ascii_case("default_entry") {
                v.trim().parse().ok()
            } else {
                None
            }
        })
    }

    /// Impose l'entrée par défaut (indice 1-based). Remplace une éventuelle
    /// déclaration existante, sinon l'ajoute au préambule.
    pub fn set_default_entry(&mut self, n: usize) {
        let line = format!("default_entry: {n}");
        for l in self.preamble.iter_mut() {
            let t = l.trim_start();
            if let Some((k, _)) = t.split_once(':') {
                if k.trim().eq_ignore_ascii_case("default_entry") {
                    *l = line;
                    return;
                }
            }
        }
        self.preamble.push(line);
    }

    /// Valeur d'un champ du corps d'une entrée (ex. `path`, `cmdline`), sans le
    /// préfixe, si présente.
    pub fn entry_field(&self, index: usize, key: &str) -> Option<&str> {
        let body = &self.entries.get(index)?.body;
        body.iter().find_map(|l| {
            let t = l.trim();
            let (k, v) = t.split_once(':')?;
            if k.trim().eq_ignore_ascii_case(key) {
                Some(v.trim())
            } else {
                None
            }
        })
    }

    /// Indique si une entrée déclare un champ donné.
    pub fn entry_has_field(&self, index: usize, key: &str) -> bool {
        self.entry_field(index, key).is_some()
    }

    /// Remplace (ou ajoute) un champ du corps d'une entrée, en conservant
    /// l'indentation des lignes existantes.
    pub fn set_entry_field(&mut self, index: usize, key: &str, value: &str) {
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        for line in entry.body.iter_mut() {
            let indent_len = line.len() - line.trim_start().len();
            let t = line.trim_start();
            let Some((k, _)) = t.split_once(':') else {
                continue;
            };
            if k.trim().eq_ignore_ascii_case(key) {
                *line = format!("{}{}: {}", &line[..indent_len], key, value);
                return;
            }
        }
        // Champ absent : on l'ajoute à la fin du corps, avec l'indentation des
        // lignes déjà présentes (4 espaces par défaut).
        let indent = entry
            .body
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .unwrap_or(4);
        entry.body.push(format!("{}{}: {}", " ".repeat(indent), key, value));
    }

    /// Valeur de `cmdline:` d'une entrée (sans le préfixe), si présente.
    pub fn entry_cmdline(&self, index: usize) -> Option<&str> {
        self.entry_field(index, "cmdline")
    }

    /// Indique si une entrée déclare un `cmdline:`.
    pub fn entry_has_cmdline(&self, index: usize) -> bool {
        self.entry_has_field(index, "cmdline")
    }

    /// Chemin du noyau d'une entrée (`path:`), si présent.
    pub fn entry_path(&self, index: usize) -> Option<&str> {
        self.entry_field(index, "path")
    }

    /// Indique si une entrée déclare un `path:`.
    pub fn entry_has_path(&self, index: usize) -> bool {
        self.entry_has_field(index, "path")
    }

    /// Impose le chemin du noyau (`path:`) d'une entrée.
    pub fn set_entry_path(&mut self, index: usize, path: &str) {
        self.set_entry_field(index, "path", path);
    }

    /// Réécrit le `root=…` du `cmdline:` de l'entrée pour pointer la partition
    /// d'étiquette `partlabel` (`root=PARTLABEL=<partlabel>`). Ne fait rien si
    /// l'entrée n'a pas de `cmdline:`.
    pub fn set_entry_cmdline_root(&mut self, index: usize, partlabel: &str) {
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        for line in entry.body.iter_mut() {
            let t = line.trim_start();
            let Some((k, _)) = t.split_once(':') else {
                continue;
            };
            if !k.trim().eq_ignore_ascii_case("cmdline") {
                continue;
            }
            *line = rewrite_root(line, partlabel);
            return;
        }
    }
}

/// Remplace le jeton `root=…` d'une ligne `cmdline:` par
/// `root=PARTLABEL=<partlabel>` ; l'ajoute s'il est absent.
fn rewrite_root(line: &str, partlabel: &str) -> String {
    let colon = match line.find(':') {
        Some(i) => i,
        None => return line.to_string(),
    };
    let (prefix, rest) = line.split_at(colon + 1);
    let mut tokens: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
    let nouveau = format!("root=PARTLABEL={partlabel}");
    match tokens.iter_mut().find(|t| t.starts_with("root=")) {
        Some(t) => *t = nouveau,
        None => tokens.push(nouveau),
    }
    format!("{} {}", prefix, tokens.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXEMPLE: &str = "\
timeout: 3
default_entry: 1

/YAOR-OS (slot A)
    protocol: linux
    path: boot():/vmlinuz-a
    cmdline: root=PARTLABEL=yaor-root_2026.10.17 ro rootfstype=ext4

/YAOR-OS (slot B)
    protocol: linux
    path: boot():/vmlinuz-b
    cmdline: root=PARTLABEL=yaor-root_2026.10.12 ro rootfstype=ext4
";

    #[test]
    fn parse_preamble_et_entrees() {
        let c = Config::parse(EXEMPLE);
        assert_eq!(c.entries.len(), 2);
        assert_eq!(c.entries[0].name, "YAOR-OS (slot A)");
        assert_eq!(c.entries[1].name, "YAOR-OS (slot B)");
        assert_eq!(c.default_entry(), Some(1));
        assert!(c.preamble.iter().any(|l| l.contains("timeout")));
    }

    #[test]
    fn aller_retour_render_parse_stable() {
        let c = Config::parse(EXEMPLE);
        assert_eq!(Config::parse(&c.render()), c);
    }

    #[test]
    fn set_default_entry_remplace() {
        let mut c = Config::parse(EXEMPLE);
        c.set_default_entry(2);
        assert_eq!(c.default_entry(), Some(2));
        // la ligne d'origine a été remplacée, pas dupliquée
        assert_eq!(
            c.preamble.iter().filter(|l| l.contains("default_entry")).count(),
            1
        );
    }

    #[test]
    fn set_default_entry_ajoute_si_absent() {
        let mut c = Config::parse("/A\n    cmdline: root=x\n");
        assert_eq!(c.default_entry(), None);
        c.set_default_entry(1);
        assert_eq!(c.default_entry(), Some(1));
    }

    #[test]
    fn entry_cmdline() {
        let c = Config::parse(EXEMPLE);
        assert_eq!(
            c.entry_cmdline(0),
            Some("root=PARTLABEL=yaor-root_2026.10.17 ro rootfstype=ext4")
        );
        assert_eq!(c.entry_cmdline(5), None);
    }

    #[test]
    fn set_entry_cmdline_root_remplace_le_jeton() {
        let mut c = Config::parse(EXEMPLE);
        c.set_entry_cmdline_root(1, "yaor-root_2026.11.01");
        let cl = c.entry_cmdline(1).unwrap();
        assert!(cl.contains("root=PARTLABEL=yaor-root_2026.11.01"));
        assert!(cl.contains("rootfstype=ext4"));
        assert!(!cl.contains("2026.10.12"));
        // l'entrée 0 n'a pas bougé
        assert!(c.entry_cmdline(0).unwrap().contains("2026.10.17"));
    }

    #[test]
    fn set_entry_cmdline_root_ajoute_si_absent() {
        let mut c = Config::parse("/A\n    protocol: linux\n    cmdline: quiet\n");
        c.set_entry_cmdline_root(0, "yaor-root_1");
        assert_eq!(c.entry_cmdline(0), Some("quiet root=PARTLABEL=yaor-root_1"));
    }

    #[test]
    fn rewrite_root_ignore_rootfstype() {
        assert_eq!(
            rewrite_root("    cmdline: rootfstype=ext4 root=/dev/sda3", "L"),
            "    cmdline: rootfstype=ext4 root=PARTLABEL=L"
        );
    }

    #[test]
    fn entry_path_et_set_entry_path() {
        let c = Config::parse(EXEMPLE);
        assert_eq!(c.entry_path(0), Some("boot():/vmlinuz-a"));
        assert!(c.entry_has_path(0));
        assert_eq!(c.entry_path(9), None);

        let mut c = Config::parse(EXEMPLE);
        c.set_entry_path(1, "boot():/yaor/vmlinuz-0.2");
        assert_eq!(c.entry_path(1), Some("boot():/yaor/vmlinuz-0.2"));
        // préserve l'indentation d'origine
        assert!(c.render().contains("    path: boot():/yaor/vmlinuz-0.2\n"));
        // l'entrée 0 n'a pas bougé
        assert_eq!(c.entry_path(0), Some("boot():/vmlinuz-a"));
    }

    #[test]
    fn set_entry_field_ajoute_si_absent() {
        let mut c = Config::parse("/A\n    protocol: linux\n    cmdline: quiet\n");
        assert_eq!(c.entry_path(0), None);
        c.set_entry_path(0, "boot():/vmlinuz");
        assert_eq!(c.entry_path(0), Some("boot():/vmlinuz"));
        // ajouté avec l'indentation des lignes existantes (4 espaces)
        assert!(c.render().contains("\n    path: boot():/vmlinuz\n"));
        // les champs existants sont conservés
        assert_eq!(c.entry_cmdline(0), Some("quiet"));
    }
}
