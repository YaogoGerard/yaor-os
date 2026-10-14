# ADR-011 — Démarrage live : Limine, initramfs shell, EROFS

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-003, ADR-004, ADR-007, ADR-009, ADR-010 (remplacé)

## Contexte

L'ADR-010 fixait la chaîne de démarrage live du jalon 0.1 : **Limine**, **initramfs
systemd** et **image racine EROFS** (zstd). Sa mise en œuvre a montré que le volet
« initramfs systemd » — faire de systemd le PID 1 de l'initramfs — est inutilement lourd
pour le besoin réel du jalon. Un initramfs systemd entraîne notamment la machinerie des
unités, des sessions et, en pratique, une dépendance à **D-Bus** et à `systemd-logind`
sans rapport avec la seule tâche demandée : localiser le média, monter une image EROFS en
lecture seule, empiler un `overlayfs` sur `tmpfs`, puis basculer vers le système réel.

Deux exigences structurantes pèsent sur ce choix : l'ISO doit être **extrêmement
légère** et la consommation RAM **extrêmement faible**. Un initramfs réduit à un script
POSIX et à quelques binaires va directement dans ce sens, sans introduire de second
modèle de service.

Les décisions de l'ADR-010 sur le **bootloader (Limine)** et sur **l'image racine
(EROFS compressée en zstd)** restent valables : elles ne sont pas remises en cause.
Conformément à la convention du registre (un ADR accepté n'est pas modifié, il est
remplacé), le présent ADR remplace intégralement l'ADR-010.

## Options envisagées

- **Initramfs systemd** (choix de l'ADR-010) — un seul modèle d'init, mais embarque la
  logique d'unités/sessions et, en pratique, la dépendance D-Bus/logind ; plus de
  travail et d'octets pour une tâche séquentielle et linéaire.
- **BusyBox** — très léger, mais ajoute un second jeu d'outils (applets) à intégrer.
- **Script POSIX minimal** (`/init` en `sh`, quelques binaires du système cible) —
  strictement le nécessaire, aucune dépendance à D-Bus, empreinte minimale.

## Décision

- **Bootloader : Limine** (BIOS et UEFI) — inchangé (ADR-010).
- **Initramfs : script POSIX minimal** exécuté par le `sh` du système cible. Le
  script `/init` : monte `proc`, `sysfs` et `devtmpfs` ; localise le média live
  (`blkid -L YAOR-LIVE`, repli `/dev/sr0`) ; monte `rootfs.erofs` en lecture seule ;
  empile un `overlayfs` dont la couche supérieure est un `tmpfs` ; puis `switch_root`
  vers `/usr/lib/systemd/systemd` du système réel. BusyBox n'est pas utilisé.
- **Image racine : EROFS**, compressée en **zstd** — inchangé (ADR-010).
- **Compression** : zstd pour l'initramfs (cpio) et l'image racine ; le noyau est
  compressé en zstd via `CONFIG_KERNEL_ZSTD`.
- **Modules** : `erofs`, `overlayfs`, `loop`, `iso9660`, `sr` et `virtio` sont
  compilés **en dur** dans le noyau ; l'initramfs n'a donc aucun module à charger.

**Flux de démarrage :** Limine charge le noyau et l'initramfs ; le script `/init`
localise l'image EROFS sur le support, la monte en lecture seule, empile un `overlayfs`
sur une couche supérieure `tmpfs` pour la session live inscriptible, puis `switch_root`
vers le systemd du système (ADR-004), qui reste l'init unique du **système installé**.

## Conséquences

- **Positives :** initramfs minimal (empreinte et RAM réduites) ; aucune dépendance à
  D-Bus ou à `systemd-logind` dans l'initramfs ; la chaîne est facile à lire et à
  auditer ; le système installé conserve bien systemd comme unique init (ADR-004).
- **Négatives / risques :** le script `/init` est propre à YAOR et doit être maintenu
  (il n'existe pas de modèle d'init unique partagé avec le système) ; il est moins
  extensible qu'un initramfs à base d'outils si des cas futurs (chiffrement, RAID,
  réveil réseau) apparaissent — un ADR ultérieur tranchera alors l'outillage.
- **Suivi :** le noyau est encore compressé en gzip dans le jalon 0.1 et sera
  reconstruit avec `CONFIG_KERNEL_ZSTD=y` ; l'ISO reste à signer (ADR-007) et à valider
  en BIOS **et** UEFI sous QEMU.
