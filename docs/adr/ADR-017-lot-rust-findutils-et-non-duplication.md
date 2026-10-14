# ADR-017 — Lot Rust : findutils et non-duplication des outils

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-002, ADR-009, ADR-012

## Contexte

ADR-012 retient `uutils/coreutils` et `sudo-rs`. Restent à statuer, au regard des critères
de maturité (ADR-002), les autres outils susceptibles d'être remplacés par une
implémentation Rust : `findutils`, `diffutils`, `grep`, `sed`, `util-linux`.

L'état 2026 est contrasté : `uutils/findutils` est un projet actif visant le remplacement à
l'identique ; `uutils/diffutils`, `uutils/grep`, `uutils/sed` et `uutils/util-linux` sont
nettement moins mûrs (versions initiales ou précoces). Par ailleurs, la coexistence d'un
outil et de son équivalent (deux `ls`, deux `find`…) constitue un **doublon** : surface
d'attaque accrue, confusion d'usage et poids inutile.

## Options envisagées

- **Ajouter tout ce qui existe en Rust** — retient des briques trop jeunes.
- **Ajouter seulement les briques Rust mûres** et **remplacer** leurs homologues C/GNU.
- **Faire coexister** GNU et Rust — exclu par le principe de non-duplication.

## Décision

- **Retenu : `uutils/findutils`** (`find`, `xargs`, `locate`, `updatedb`) **remplace** GNU
  findutils dans l'image finale.
- **Non retenu pour v0.2** (maturité insuffisante) : `uutils/diffutils`, `uutils/grep`,
  `uutils/sed`, `uutils/util-linux` — leurs homologues GNU restent utilisés.
- **Principe de non-duplication :** dans l'image finale, **un besoin correspond à un seul
  fournisseur**. Tout composant Rust retenu **remplace** son homologue C/GNU, qui n'est
  **pas** embarqué. Aucun outil ne doit être fourni deux fois.

## Conséquences

- **Positives :** userland plus homogène et sûr ; aucun doublon ; choix guidés par la
  maturité réelle, pas par le langage seul.
- **Négatives / risques :** l'assemblage doit **retirer** les binaires GNU remplacés
  (coreutils, findutils) pour éviter les doublons ; `uutils` étant plus jeune que GNU, les
  écarts de comportement doivent être couverts par des tests.
- **Suivi :** réévaluer `diffutils`, `grep`, `sed` et `util-linux` à chaque jalon ; mettre à
  jour `component-selection.md` et l'étape 06 (staging) en conséquence.
