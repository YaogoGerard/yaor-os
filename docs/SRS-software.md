# SRS — Exigences logicielles — yaor-os

**Statut :** Brouillon
**Version :** 0.2

Convention : DOIT (obligatoire) / DEVRAIT (recommandé) / PEUT (optionnel).

## REQ-SW-001
Le système live DOIT démarrer avec succès en mode BIOS (legacy) et en mode UEFI
(voir RG-005).

## REQ-SW-002
Le script `01_download.sh` DOIT vérifier l'intégrité de chaque archive téléchargée
via une somme de contrôle (SHA-256) avant de poursuivre le pipeline.

## REQ-SW-003
La toolchain temporaire (chapitre 5 LFS) DOIT être compilée avant toute tentative de
construction du système final (voir RG-003).

## REQ-SW-004
Le système final DOIT être construit en environnement `chroot`, isolé de la machine
hôte du pipeline CI.

## REQ-SW-005
Le rootfs final DOIT être compressé au format SquashFS avant intégration dans l'image
ISO.

## REQ-SW-006
L'image ISO finale DEVRAIT peser moins de 1 Go pour la version CLI minimale
(voir RG-001).

## REQ-SW-007
Le système live DOIT démarrer directement sur un interpréteur de commandes (shell),
sans interface graphique ni installateur, pour cette version.

## REQ-SW-008
Le pipeline de build DOIT s'exécuter de bout en bout via GitHub Actions, sans étape
manuelle requise.

## REQ-SW-009
Le userland du système live DOIT être composé d'outils écrits en Rust :
uutils/coreutils fournit les utilitaires de base sous leurs noms classiques, et les
outils complémentaires (bat, ripgrep, fd, dust, procs, hexyl) sont présents dans le
système.

## REQ-SW-010
Les équivalences vers les outils Rust complémentaires DOIVENT être mises en œuvre
par des aliases définis pour le shell interactif uniquement (`/etc/profile.d/`) ;
elles ne DOIVENT pas modifier le comportement des scripts non interactifs.

## REQ-SW-011
Un socle GNU minimal (bash, grep, findutils, sed, gawk, tar, gzip, xz) DOIT rester
installé sous ses vrais noms, garantissant la fiabilité des scripts et un moyen de
contourner tout alias (préfixe `\` ou chemin absolu).
