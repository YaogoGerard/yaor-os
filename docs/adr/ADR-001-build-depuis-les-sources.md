# ADR-001 — Build depuis les sources (LFS)

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-007, ADR-003

## Contexte

Deux documents se contredisaient : `deployment-ops.md` décrivait un build « depuis les
sources » (façon Linux From Scratch : toolchain → rootfs → noyau → initramfs → ISO),
tandis que `package-architecture.md` évoquait APT/dpkg comme socle. Ces deux modèles
sont incompatibles dans leur logistique (bootstrap, cross-compilation, reproductibilité).

## Options envisagées

- **Option A — Depuis les sources (LFS).** Tout est compilé depuis les sources avec une
  toolchain dédiée. Reproductible et maîtrisé, mais build long.
- **Option B — Assemblage de paquets.** Base binaire existante + paquets. Rapide, moins
  « from scratch ».
- **Option C — Hybride.** Base binaire + composants d'identité compilés.

## Décision

**Option A : YAOR est construit depuis les sources.** Aucun paquet binaire d'une
distribution tierce n'est utilisé pour le système de base.

## Conséquences

- **Positives :** maîtrise complète, reproductibilité, cohérence avec la philosophie du
  projet.
- **Négatives / risques :** build long ; nécessite une toolchain et un bootstrap Rust
  soignés ; effort CI important.
- **Suivi :** `package-architecture.md` est réécrit pour refléter ce choix ; le
  « packaging » devient un mécanisme d'intégration, pas le socle de construction.
