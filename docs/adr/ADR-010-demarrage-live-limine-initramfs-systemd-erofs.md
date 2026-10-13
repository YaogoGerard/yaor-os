# ADR-010 — Démarrage live : Limine, initramfs systemd, EROFS

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-003, ADR-004, ADR-007, ADR-009

## Contexte

Le jalon 0.1 doit produire une **ISO live amorçable** en **BIOS et UEFI** (REQ-SW-017), avec
**systemd** comme init (ADR-004) et construite **depuis les sources** (ADR-001). Trois choix
conditionnent la chaîne de démarrage : le bootloader, la nature de l'initramfs et le format
de l'image racine en lecture seule. Conformément à ADR-009, ces choix doivent être modernes,
maintenus et performants.

## Options envisagées

- **Bootloader** — GRUB (universel, base ancienne) ; **Limine** (moderne, BIOS + UEFI) ;
  systemd-boot (moderne, mais **UEFI uniquement**, incompatible avec BIOS).
- **Initramfs** — BusyBox (léger, mais second modèle d'init) ; dracut (mature, framework en
  shell) ; **initramfs systemd** (systemd PID 1 en initramfs).
- **Image racine** — SquashFS (mature, universel) ; **EROFS** (moderne, rapide, natif noyau).

## Décision

- **Bootloader : Limine**, qui gère BIOS et UEFI et préserve REQ-SW-017.
- **Initramfs : systemd** (systemd est PID 1 dans l'initramfs) — pas de BusyBox.
- **Image racine : EROFS**, compressée en **zstd**.
- **Compression** : `zstd` pour le noyau, l'initramfs et l'image racine.

**Flux de démarrage :** Limine charge le noyau et l'initramfs ; l'initramfs (systemd) localise
l'image EROFS sur le support, la monte en lecture seule, empile un `overlayfs` avec une couche
supérieure `tmpfs` pour la session live inscriptible, puis `switch_root` vers le systemd du
système.

## Conséquences

- **Positives :** un seul modèle d'init (systemd partout) ; démarrage et décompression rapides ;
  empreinte mémoire réduite pour l'image racine.
- **Négatives / risques :** écosystème Limine plus restreint que GRUB ; outillage EROFS moins
  répandu côté live (non bloquant, l'initramfs est le nôtre) ; l'initramfs systemd demande plus
  de travail qu'un initramfs BusyBox.
- **Suivi :** dépendances hôte `limine` et `erofs-utils` requises aux étapes 04/05 ; activer dans
  la configuration noyau `EROFS`, `zstd`, `overlayfs` et les prérequis systemd ; valider le boot
  BIOS **et** UEFI sous QEMU.
