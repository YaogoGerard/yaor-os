# ADR-005 — Pile graphique Wayland

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-002, ADR-004, ADR-006

## Contexte

`graphics-architecture.md` évoquait Smithay comme base « candidate », sans trancher la
base du compositor, la compatibilité X11 (XWayland) ni la gestion de session.

## Options envisagées

- **Option A — Smithay + XWayland + logind.**
- **Option B — Wayland pur, sans X11.**
- **Option C — Rester au niveau HLD, sans figer.**

## Décision

**Option A.**

- Le compositor YAOR est bâti sur **Smithay** (bibliothèque Rust), ce qui évite de
  réimplémenter les primitives Wayland (`RG-001`, ADR-002).
- **XWayland** est intégré pour la compatibilité des applications X11.
- Les sessions s'appuient sur **`logind`** (systemd, ADR-004) : l'acquisition
  du siège passe par `libseat` en **mode logind par défaut** (variable
  `YAOR_LIBSEAT_BACKEND`, réalisée v0.4 étape 5) ; `seatd` reste le repli
  documenté (chaînement de validation du v0.3).
- Le compositor **assure la gestion des fenêtres** : il n'y a pas de « Window Manager »
  distinct (voir aussi ADR-000, décision 12).

## Conséquences

- **Positives :** base Rust mature pour l'essentiel ; compatibilité applicative large.
- **Négatives / risques :** Smithay impose une courbe d'apprentissage ; XWayland ajoute
  de la complexité.
- **Suivi :** mettre à jour `graphics-architecture.md`, `desktop-architecture.md`,
  `compositor.md` (retirer la case « Window Manager »).
