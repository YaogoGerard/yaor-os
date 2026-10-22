# Jalons — YAOR OS

**Statut :** Accepté
**Version :** 0.1
**Dernière mise à jour :** 2026-10-17
**Décisions liées :** ADR-000 (#21), ADR-003, ADR-011, ADR-015, ADR-022

## Objet

Ce document matérialise la décision **ADR-000 #21** (« Jalons numérotés + critères de
sortie »). Il fixe, pour chaque jalon, un **périmètre** et des **critères de sortie
vérifiables** : un jalon n'est déclaré atteint que lorsque **tous** ses critères sont
satisfaits et vérifiés.

Les jalons sont numérotés `v0.x` (alignés sur la roadmap indicative du PRD §7) ; chaque
jalon correspond à une ou plusieurs phases de [`roadmap.md`](roadmap.md).

Un critère de sortie est **vérifiable** s'il peut être constaté par un test, une commande
ou un artefact (image, journal). Les critères d'un jalon lointain ne sont détaillés qu'à
l'approche de ce jalon : seuls `v0.1` (atteint) et `v0.2` (en cours) sont figés ici.

## Vue d'ensemble

| Jalon | Objet | Phase roadmap | État |
|---|---|---|---|
| v0.1 | Système de construction + boot Linux minimal | Phase 0 — Foundation | Atteint |
| v0.2 | Userland fonctionnel et composants sélectionnés | Phase 1 — System | Atteint |
| v0.3 | Base graphique | Phase 2 — Graphics | En cours |
| v0.4 | Compositor YAOR | Phase 2 — Graphics | À venir |
| v0.5 | Desktop Core configurable | Phase 3 — YAOR Desktop | À venir |
| v0.6 | Profils Caméléon | Phase 4 — Caméléon | À venir |
| v0.7 | Système de plugins | Phase 5 — Extensibilité | À venir |
| v0.8 | Accessibilité | Phase 6 — Accessibility | À venir |
| v1.0 | Système intégré et documenté | Phase 7 — Stabilisation | À venir |

## v0.1 — Système de construction + boot Linux minimal (atteint)

**Périmètre :** build depuis les sources (ADR-001), système de base minimal, noyau Linux,
ISO live amorçable en BIOS et UEFI (ADR-011).

**Critères de sortie (vérifiés) :**

- [x] La toolchain et le système de base se construisent depuis les sources vérifiées
  (SHA-256).
- [x] L'ISO live démarre en BIOS **et** en UEFI sous QEMU jusqu'à un shell.
- [x] `systemd` est le PID 1 du système réel (ADR-004) ; la racine est un `overlayfs` sur
  une EROFS compressée en zstd.
- [x] `systemctl is-system-running` retourne `running` ; `poweroff` est propre.
- [x] L'orchestrateur `scripts/build.sh` enchaîne les étapes 00 → 06 et s'arrête à la
  première erreur.
- [x] Une empreinte SHA-256 de l'ISO est produite (la signature GPG reste à activer,
  ADR-007).

## v0.2 — Userland fonctionnel et composants sélectionnés (atteint)

**Périmètre :** rendre le système utilisable au quotidien en mode **live** et **installé** :
réseau, sessions et comptes, firmware, stockage amovible, toolchain Rust, cycle de vie
installé et mises à jour par image. Correspond à la Phase 1 « System ».

**Critères de sortie :**

### Construction et toolchain

- [x] La toolchain Rust (`rustc`/`cargo` de l'hôte) est un **prérequis de build** vérifié à
  l'étape 00 ; aucun `rustc`/LLVM n'est embarqué (ADR-019, remplace en partie ADR-012).
- [x] `uutils/coreutils` remplace GNU coreutils dans l'image finale (ADR-012) : aucun
  binaire portant la chaîne « GNU coreutils » ne subsiste dans `/usr/bin` (contrôlé aux
  étapes 04 et 06).
- [x] `uutils/findutils` remplace GNU findutils (ADR-017) : aucun binaire portant
  « GNU findutils » ne subsiste ; un besoin n'a **qu'un seul fournisseur**.
- [x] Les dépendances cargo sont **vendues au téléchargement** (`cargo vendor`, étape 01)
  et les composants Rust se compilent **hors-ligne** (`CARGO_NET_OFFLINE=true`,
  ADR-018/ADR-019).
- [x] Le build est **reproductible au sens fonctionnel** : versions et empreintes épinglées,
  même résultat pour une même révision (ADR-016, REQ-SW-016).

### Réseau (ADR-013)

- [x] Le réseau filaire est opérationnel via `systemd-networkd` (DHCP), la résolution via
  `systemd-resolved`.
- [x] Le Wi-Fi via `iwd` et `linux-firmware` est embarqué (ADR-013, ADR-020) ; la
  validation sur matériel réel reste à planifier (la QA sous QEMU n'expose pas de
  radio Wi-Fi).
- [x] Aucun démon réseau secondaire (NetworkManager) n'est embarqué.

### Sessions, comptes et services (ADR-014)

- [x] D-Bus (bus système et utilisateur) est présent ; `systemd-logind` est réactivé et
  opérationnel (plus masqué).
- [x] Le système est multi-utilisateur POSIX (REQ-SW-015, REQ-SW-023) : création de
  comptes, `/home`, `getty`/`login`, sessions suivies par `logind` (REQ-SW-021).
- [x] L'élévation de privilèges fonctionne via `sudo-rs` (ADR-012).

### Stockage (ADR-015)

- [x] Le montage automatique des volumes amovibles fonctionne via `udisks2`.
- [x] Les systèmes de fichiers `ext4`, `btrfs`, `vfat`, `exFAT` et `NTFS`
  (lecture/écriture) sont supportés.
- [x] Le chiffrement LUKS reste **hors** périmètre de v0.2 (reporté).

### Mode installé et mises à jour (ADR-015)

- [x] Un système installé sur disque démarre sans le média live, en BIOS **et** UEFI
  (REQ-SW-019).
- [x] Les mises à jour se font par image, avec deux slots A/B et retour arrière, via
  `systemd-sysupdate` (REQ-SW-020).

### Tests et traçabilité (REQ-SW-018)

- [x] Les cas de test manquants sont rédigés et la matrice de traçabilité
  (`requirements-traceability.md`) couvre les exigences du jalon.
- [x] Les tests réseau, sessions, stockage, installation et mise à jour sont exécutés et
  passent.

### Documentation

- [x] Les ADR, la sélection des composants, l'architecture de déploiement et les services
  système reflètent l'état atteint.

## v0.3 — Base graphique

**Périmètre :** la pile sous le compositor — rendu GPU (mesa + libdrm), protocoles
Wayland (libwayland), périphériques d'entrée (libinput + libevdev) et audio
(pipewire + wireplumber) — validée sous QEMU par la session Wayland de référence
(ADR-024). Le compositor YAOR, les profils et XWayland relèvent de v0.4 (ADR-005).

**Critères de sortie :**

### Kernel et affichage

- [x] Le KMS prend l'affichage nativement dès le boot (fbcon sur `simpledrm` /
  `virtio-gpu`) ; les consoles tty restent accessibles (bascule VT propre).

### Pile userland (ADR-024)

- [x] `mesa` (EGL/GLES2, softpipe — llvmpipe écarté du live sans LLVM) et
  `libdrm` épinglés dans `sources/manifest.tsv` (SHA-256), construits
  hors-ligne ; une surface EGL s'initialise et rend (GLES2) — QA anvil :
  EGL 1.5 initialisé sur `PLATFORM_GBM_KHR`.
- [ ] `libwayland` (client + serveur) épinglée et construite ; un client de test
  dialogue avec le compositor de validation.
- [ ] `libinput` (+ `libevdev`) livre les événements clavier/pointeur à la session
  graphique sous QEMU.

### Audio

- [x] `pipewire` daemon + `wireplumber` politique : un son synthétique est audible
  via la carte HDA générique (`pw-cat` → `intel-hda`, QEMU).

### Session graphique de référence

- [x] Le compositor d'exemple Smithay tient une session sur KMS dans le système
  installé : rendu (rectangle + horloge), entrée (déplacement d'un objet), sortie
  propre vers la console ; la validation couvre virtio-gpu (KMS) et le repli
  `simpledrm`.

  Preuves v0.4 (QA `t-anvil` 12/12 virtio, `t-anvil-simpledrm` 13/13 —
  RC=0) : anvil 0.7.0 tient la session (KMS master via seatd), socket
  `wayland-1` publiée, screendump QMP avec contenu client (client
  `waytest` : rectangle + horloge digitale + compteurs, carré qui suit le
  pointeur) ; entrées réelles injectées par QEMU (QMP `input-send-event`,
  pointeur PS/2) traversant evdev → libinput → anvil → waytest (2 clics,
  3 touches livrés) ; **fenêtre déplacée (507 px, bbox du scanout) et
  redimensionnée (8 buffers recréés) via les requêtes xdg interactives
  déclenchées par waytest** — anvil arme ses grabs serveur côté
  `shell/xdg.rs` ; sortie propre au SIGTERM. Repli `simpledrm` couvert
  (`-vga std`, dmesg + connecteur `Unknown-1`, move/resize aussi verts :
  722 px + buffers). Détails : `docs/06-testing/graphics-tests.md`.

### Tests et traçabilité (REQ-SW-018)

- [ ] Les cas TEST-GFX-001/002 et les cas audio sont exécutés et passent ; la
  matrice de traçabilité couvre les exigences graphiques du jalon.

  Avancement (v0.4) : TEST-GFX-002 **vert** (session de référence complète :
  KMS boot, mesa softpipe, session anvil, client Wayland, entrées libinput,
  audio, repli simpledrm) ; TEST-AUD-001 vert ; TEST-GFX-001 **vert**
  (fenêtre ouverte ✓, entrées ✓, déplacer/redimensionner ✓ via les requêtes
  xdg interactives — grabs d'anvil armés côté serveur) ; audio vert. Matrice
  de traçabilité : REQ-SW-005 → TEST-GFX-001 + TEST-GFX-002 (ADR-024).

## v0.4 → v1.0

Les périmètres sont fixés par [`roadmap.md`](roadmap.md) ; les critères de sortie sont à
détailler à l'ouverture de chaque jalon (compositor, Desktop Core, profils Caméléon,
plugins, accessibilité, stabilisation).
