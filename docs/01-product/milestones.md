# Jalons — YAOR OS

**Statut :** Accepté
**Version :** 0.1
**Dernière mise à jour :** 2026-10-14
**Décisions liées :** ADR-000 (#21), ADR-003, ADR-011

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
| v0.2 | Userland fonctionnel et composants sélectionnés | Phase 1 — System | En cours |
| v0.3 | Base graphique | Phase 2 — Graphics | À venir |
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

## v0.2 — Userland fonctionnel et composants sélectionnés (en cours)

**Périmètre :** rendre le système utilisable au quotidien en mode **live** et **installé** :
réseau, sessions et comptes, firmware, stockage amovible, toolchain Rust, cycle de vie
installé et mises à jour par image. Correspond à la Phase 1 « System ».

**Critères de sortie :**

### Construction et toolchain

- [ ] Rust est construit depuis les sources, amorcé par le `rustc` de l'hôte, avec le LLVM
  embarqué (ADR-012).
- [ ] `uutils/coreutils` remplace GNU coreutils dans l'image finale ; GNU reste les outils
  temporaires de construction (ADR-012).
- [ ] Le build est **reproductible au sens fonctionnel** : versions et empreintes épinglées,
  même résultat pour une même révision (ADR-016, REQ-SW-016).

### Réseau (ADR-013)

- [ ] Le réseau filaire est opérationnel via `systemd-networkd` (DHCP), la résolution via
  `systemd-resolved`.
- [ ] Le Wi-Fi matériel fonctionne via `iwd` et `linux-firmware`.
- [ ] Aucun démon réseau secondaire (NetworkManager) n'est embarqué.

### Sessions, comptes et services (ADR-014)

- [ ] D-Bus (bus système et utilisateur) est présent ; `systemd-logind` est réactivé et
  opérationnel (plus masqué).
- [ ] Le système est multi-utilisateur POSIX (REQ-SW-015, REQ-SW-023) : création de
  comptes, `/home`, `getty`/`login`, sessions suivies par `logind` (REQ-SW-021).
- [ ] L'élévation de privilèges fonctionne via `sudo-rs` (ADR-012).

### Stockage (ADR-015)

- [ ] Le montage automatique des volumes amovibles fonctionne via `udisks2`.
- [ ] Les systèmes de fichiers `ext4`, `btrfs`, `vfat`, `exFAT` et `NTFS`
  (lecture/écriture) sont supportés.
- [ ] Le chiffrement LUKS reste **hors** périmètre de v0.2 (reporté).

### Mode installé et mises à jour (ADR-015)

- [ ] Un système installé sur disque démarre sans le média live, en BIOS **et** UEFI
  (REQ-SW-019).
- [ ] Les mises à jour se font par image, avec deux slots A/B et retour arrière, via
  `systemd-sysupdate` (REQ-SW-020).

### Tests et traçabilité (REQ-SW-018)

- [ ] Les cas de test manquants sont rédigés et la matrice de traçabilité
  (`requirements-traceability.md`) couvre les exigences du jalon.
- [ ] Les tests réseau, sessions, stockage, installation et mise à jour sont exécutés et
  passent.

### Documentation

- [ ] Les ADR, la sélection des composants, l'architecture de déploiement et les services
  système reflètent l'état atteint.

## v0.3 — Base graphique

**Périmètre :** pile graphique, entrée, audio, Wayland (Phase 2 — Graphics, ADR-005).

**Critères de sortie :** à détailler à l'ouverture du jalon (composants retenus, session
Wayland de référence, validation sous QEMU).

## v0.4 → v1.0

Les périmètres sont fixés par [`roadmap.md`](roadmap.md) ; les critères de sortie sont à
détailler à l'ouverture de chaque jalon (compositor, Desktop Core, profils Caméléon,
plugins, accessibilité, stabilisation).
