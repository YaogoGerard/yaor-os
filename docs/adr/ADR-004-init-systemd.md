# ADR-004 — Init et gestion de services : systemd

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-005

## Contexte

Le document `system-services.md` ne décidait rien, alors que l'init et la gestion des
services sont le cœur d'un OS. Le choix conditionne les sessions (`logind`), le réseau,
le stockage et l'intégration de la pile graphique.

## Options envisagées

- **Option A — systemd.** Standard de facto, écosystème large ; ce n'est pas du Rust.
- **Option B — OpenRC / BusyBox init.** Léger, adapté à un système minimal.
- **Option C — Init YAOR en Rust.** Cohérent avec la préférence Rust, mais effort important.

## Décision

**Option A : systemd.** Cette décision est cohérente avec la préférence pour l'existant
(`RG-001`) : systemd étant mature, la préférence Rust ne s'applique pas (ADR-002).

## Conséquences

- **Positives :** intégration directe de `logind` (sessions), du réseau et du stockage ;
  écosystème et documentation abondants.
- **Négatives / risques :** composant volumineux ; dépendance forte ; moins « minimaliste ».
- **Suivi :** lister les services de base requis dans `system-services.md` et
  `deployment-ops.md`.
