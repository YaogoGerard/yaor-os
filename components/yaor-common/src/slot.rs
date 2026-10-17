//! Les deux slots racine A/B (ADR-015, REQ-SW-026).

use std::fmt;

/// Slot racine. `A` est la **première** partition `root-x86-64` sur le disque,
/// `B` la seconde ; l'ordre est stable et sert d'identité.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    A,
    B,
}

impl Slot {
    /// Lettre du slot (`A` ou `B`).
    pub fn letter(self) -> char {
        match self {
            Slot::A => 'A',
            Slot::B => 'B',
        }
    }

    /// Index 0/1 (ordre sur le disque).
    pub fn index(self) -> usize {
        match self {
            Slot::A => 0,
            Slot::B => 1,
        }
    }

    /// L'autre slot.
    pub fn other(self) -> Slot {
        match self {
            Slot::A => Slot::B,
            Slot::B => Slot::A,
        }
    }

    /// Les deux slots, dans l'ordre.
    pub fn all() -> [Slot; 2] {
        [Slot::A, Slot::B]
    }

    /// Analyse `A`/`B`, insensible à la casse. `None` si la chaîne est vide ou
    /// ne correspond à aucun slot.
    pub fn parse(s: &str) -> Option<Slot> {
        match s.trim() {
            "A" | "a" => Some(Slot::A),
            "B" | "b" => Some(Slot::B),
            _ => None,
        }
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Slot::A => "A",
            Slot::B => "B",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letter_and_index() {
        assert_eq!(Slot::A.letter(), 'A');
        assert_eq!(Slot::B.index(), 1);
        assert_eq!(Slot::A.other(), Slot::B);
        assert_eq!(Slot::B.other(), Slot::A);
    }

    #[test]
    fn parse_accepte_les_deux_casses() {
        assert_eq!(Slot::parse("a"), Some(Slot::A));
        assert_eq!(Slot::parse(" B "), Some(Slot::B));
        assert_eq!(Slot::parse(""), None);
        assert_eq!(Slot::parse("C"), None);
    }

    #[test]
    fn display() {
        assert_eq!(Slot::A.to_string(), "A");
        assert_eq!(Slot::B.to_string(), "B");
    }
}
