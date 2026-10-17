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
| [ADR-009](ADR-009-selection-composants-modernes.md) | Sélection des composants : modernité, maintenance, performance | Accepté |
| [ADR-010](ADR-010-demarrage-live-limine-initramfs-systemd-erofs.md) | Démarrage live : Limine, initramfs systemd, EROFS | Remplacé par ADR-011 |
| [ADR-011](ADR-011-demarrage-live-limine-initramfs-shell-erofs.md) | Démarrage live : Limine, initramfs shell, EROFS | Accepté |
| [ADR-012](ADR-012-amorce-et-integration-de-rust.md) | Amorce et intégration de Rust | Accepté |
| [ADR-013](ADR-013-reseau-systemd-networkd-iwd.md) | Réseau : systemd-networkd, systemd-resolved, iwd | Accepté |
| [ADR-014](ADR-014-sessions-comptes-et-services.md) | Sessions, comptes et services de session | Accepté |
| [ADR-015](ADR-015-cycle-de-vie-installe-et-mises-a-jour.md) | Cycle de vie installé et mises à jour par image | Accepté |
| [ADR-016](ADR-016-reproductibilite.md) | Reproductibilité : niveau fonctionnel | Accepté |
| [ADR-017](ADR-017-lot-rust-findutils-et-non-duplication.md) | Lot Rust : findutils et non-duplication des outils | Accepté |
| [ADR-018](ADR-018-dependances-cargo-vendoring.md) | Dépendances cargo : vendoring | Accepté |
| [ADR-019](ADR-019-chaine-rust-build-toolchain-hote-et-vendoring.md) | Chaîne Rust : toolchain hôte et vendoring au téléchargement | Accepté |
| [ADR-020](ADR-020-wifi-sans-regdb-signe.md) | Wi-Fi : cfg80211 sans registre réglementaire signé | Accepté (clause OpenSSL remplacée par ADR-021) |
| [ADR-021](ADR-021-openssl-et-curl-dans-le-socle-v0-2.md) | OpenSSL et curl dans le socle v0.2 (mode installé) | Accepté |

## Principe de décision

> Réutiliser l'existant. Préférer Rust lorsque c'est approprié. Construire ce qui manque.

Toute création d'un composant propre à YAOR doit être justifiée par un ADR
(voir ADR-002 et ADR-000).
