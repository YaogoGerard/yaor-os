# ADR-018 — Dépendances cargo : vendoring

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-012, ADR-016, ADR-017

## Contexte

Les composants Rust retenus (`uutils/coreutils`, `uutils/findutils`, `sudo-rs`, et `rustc`
lui-même) se compilent avec `cargo` et dépendent de **plusieurs centaines de crates**
publiées sur crates.io. Le build de YAOR est un build **depuis les sources** (ADR-001),
**fonctionnellement reproductible** (ADR-016), à versions et empreintes épinglées. Tirer
les crates du réseau au moment du build introduirait une dépendance à un service tiers,
non maîtrisée et non reproductible.

## Options envisagées

- **Réseau (crates.io) pendant le build** — simple, mais non reproductible et dépendant d'un
  service tiers.
- **Vendoring (`cargo vendor`)** — crates figées dans les artefacts et compilées hors-ligne
  (`--offline`).
- **Miroir interne de crates** — équivalent à un miroir de sources, à maintenir.

## Décision

**Vendoring.** Les dépendances des composants Rust sont **vendues** (`cargo vendor`) et
**épinglées** (fichiers `Cargo.lock` + arbre de crates figé), puis compilées **hors-ligne**.
Le pipeline ne contacte pas crates.io pendant la construction : la disponibilité des sources
reste alignée sur le modèle d'ADR-001 (sources vérifiées, versions fixes).

## Conséquences

- **Positives :** build hors-ligne et reproductible ; cohérent avec ADR-001 et ADR-016 ;
  aucune dépendance à crates.io au moment du build.
- **Négatives / risques :** volume de sources vendues important ; toute mise à jour de
  dépendance impose de re-vendre et re-épingler l'arbre.
- **Suivi :** définir l'emplacement des crates vendues et leur vérification ; documenter la
  procédure dans `deployment-ops.md`.
