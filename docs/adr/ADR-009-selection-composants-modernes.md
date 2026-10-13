# ADR-009 — Sélection des composants : modernité, maintenance, performance

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-002, ADR-010

## Contexte

Le principe « réutiliser l'existant » (`RG-001`) et la préférence Rust (ADR-002) peuvent
être lus comme une préférence pour le conservatisme : choisir des composants anciens mais
éprouvés, quitte à les remplacer plus tard. Pour un OS réel, ce compromis coûte cher : il
impose une dette technique immédiate (double modèle d'init, bootloader obsolète, formats
d'image lents) et un travail de migration non planifié.

La sélection par défaut d'un composant doit donc reposer sur des critères explicites, sans
confondre **maturité** et **ancienneté**.

## Options envisagées

- **Option A — Défauts conservateurs, migration ultérieure.** Démarrer sur des briques
  anciennes (BusyBox, GRUB, SquashFS) puis migrer après la v1.
- **Option B — Défauts modernes dès le départ.** Retenir des composants récents, activement
  maintenus et performants, en appliquant les critères de maturité (ADR-002).

## Décision

**Option B.** Tout composant choisi par défaut DOIT satisfaire les critères suivants :

1. **Maintenance active** — upstream vivant (versions récentes, correctifs de sécurité).
2. **Pas de technologie dépréciée** — aucune brique dont la trajectoire est l'abandon.
3. **Performance** — coût (démarrage, mémoire, E/S) compatible avec les objectifs du projet.
4. **Maturité** — satisfait les critères objectifs d'ADR-002 ; « mature » ≠ « ancien ».
5. **Cohérence** — s'intègre à l'écosystème déjà retenu (systemd, Linux, Wayland).

Un composant plus ancien n'est retenu que s'il n'existe pas d'alternative moderne satisfaisant
ces critères (ex. glibc, noyau Linux, systemd).

## Conséquences

- **Positives :** pas de dette technique « provisoire » ; cohérence des choix ; les jalons
  construisent directement la cible, sans étape de migration.
- **Négatives / risques :** certains composants modernes ont un écosystème plus restreint
  (ex. Limine face à GRUB) ; risque de retenir une brique trop jeune → mitigé par les
  critères d'ADR-002 et par les tests de boot.
- **Suivi :** appliqué d'abord à la chaîne de démarrage (ADR-010) ; à vérifier pour chaque
  nouvelle brique (compression `zstd`, outils utilisateur, etc.).
