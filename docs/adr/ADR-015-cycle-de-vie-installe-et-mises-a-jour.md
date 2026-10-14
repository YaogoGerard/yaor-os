# ADR-015 — Cycle de vie installé et mises à jour par image

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-003, ADR-004, ADR-007, ADR-011, ADR-014

## Contexte

ADR-003 acte un cycle de vie **live + installé** dès la v1, avec des mises à jour par
**image immuable** (remplacement, A/B ou réinstallation). Le jalon 0.1 n'a produit qu'un
système **live** (EROFS + overlayfs, ADR-011). `REQ-SW-019` exige le mode **installé** et
`REQ-SW-020` impose des mises à jour **par image**, sans gestionnaire de paquets
classique.

Le jalon v0.2 inclut le passage au mode **installé** et le mécanisme de mise à jour.

## Options envisagées

- **Gestionnaire de paquets classique** (APT/dpkg) — exclu par ADR-003 et `REQ-SW-020`.
- **`systemd-sysupdate`** — intégré à systemd, gère des **slots A/B** et le retour
  arrière ; cohérent avec l'écosystème retenu (ADR-004, ADR-009).
- **OSTree** — éprouvé mais introduit une pile supplémentaire.
- **Réinstallation systématique** — possible, mais sans mise à jour atomique ni retour
  arrière.

## Décision

- **Mode installé :** un **installateur** dépose le système sur disque et **Limine** est
  configuré sur le disque (BIOS et UEFI, `REQ-SW-019`).
- **Mises à jour : `systemd-sysupdate`**, avec **deux slots A/B** et **retour arrière**
  (`REQ-SW-020`). Aucun gestionnaire de paquets classique.
- **Systèmes de fichiers supportés :** `ext4` (racine par défaut), `btrfs`,
  `vfat` + `exFAT` (volumes amovibles), `NTFS` en lecture/écriture.
- **Chiffrement LUKS :** **reporté** hors de v0.2 ; un ADR ultérieur le tranchera.

## Conséquences

- **Positives :** cycle de vie conforme à ADR-003 ; mises à jour atomiques et
  réversibles ; périmètre de stockage clair.
- **Négatives / risques :** l'installateur et le partitionnement (ESP + slots A/B)
  représentent une part importante du travail ; `systemd-sysupdate` doit être configuré et
  testé (signature des mises à jour à préciser, cf. ADR-007) ; sans LUKS, le disque
  installé n'est pas chiffré.
- **Suivi :** préciser la **forme de l'installateur** (script/TUI) dans un document LLD ;
  documenter le schéma de partitionnement et le mécanisme de retour arrière ; activer la
  signature des images de mise à jour (ADR-007).
