# Architecture Decision Records (ADR) — YAOR OS

Ce dossier consigne les **décisions structurantes** du projet : pourquoi un choix a
été fait, quelles options ont été écartées et quelles en sont les conséquences.

## Convention

- Un fichier par décision : `ADR-NNN-titre-court.md`.
- Numérotation séquentielle, jamais réutilisée.
- Un ADR accepté n'est pas modifié : s'il change, il est **remplacé** par un nouvel
  ADR qui le référence.
- Modèle : [`template.md`](template.md).

## Statuts

| Statut | Sens |
|---|---|
| Proposé | En discussion |
| Accepté | Fait autorité |
| Remplacé | Remplacé par un ADR plus récent (référencé) |

## Index

| ADR | Titre | Statut |
|---|---|---|
| [ADR-000](ADR-000-registre-des-decisions.md) | Registre des décisions fondatrices | Accepté |
| [ADR-001](ADR-001-build-depuis-les-sources.md) | Build depuis les sources (LFS) | Accepté |
| [ADR-002](ADR-002-preference-rust-et-criteres-de-maturite.md) | Préférence Rust et critères de maturité | Accepté |
| [ADR-003](ADR-003-cycle-de-vie-live-installe-image-immuable.md) | Cycle de vie : live + installé, image immuable | Accepté |
| [ADR-004](ADR-004-init-systemd.md) | Init et gestion de services : systemd | Accepté |
| [ADR-005](ADR-005-pile-graphique-wayland.md) | Pile graphique Wayland | Accepté |
| [ADR-006](ADR-006-accessibilite-at-spi.md) | Accessibilité par réutilisation d'AT-SPI | Accepté |
| [ADR-007](ADR-007-securite-approvisionnement.md) | Sécurité de la chaîne d'approvisionnement | Accepté |
| [ADR-008](ADR-008-convention-de-commit.md) | Convention de commit | Accepté |

## Principe de décision

> Réutiliser l'existant. Préférer Rust lorsque c'est approprié. Construire ce qui manque.

Toute création d'un composant propre à YAOR doit être justifiée par un ADR
(voir ADR-002 et ADR-000).
