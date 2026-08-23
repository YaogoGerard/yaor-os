# Glossaire — yaor-os

| Terme | Définition dans ce projet |
|---|---|
| Live OS | Système d'exploitation qui s'exécute directement depuis une clé USB, sans installation sur le disque dur |
| Rootfs | L'arborescence complète du système de fichiers racine (`/`), avant compression |
| Toolchain | L'ensemble compilateur + bibliothèques utilisé pour construire le système (ici : GCC, Glibc, Binutils) |
| Toolchain temporaire | Étape intermédiaire de LFS (chapitre 5) : une toolchain isolée, utilisée uniquement pour construire le système final |
| Chroot | Environnement isolé où l'on "change de racine" (`/`) pour construire ou tester le système final sans affecter la machine hôte |
| SquashFS | Format de système de fichiers compressé en lecture seule, utilisé pour embarquer le rootfs dans l'ISO |
| Initramfs | Petit système de fichiers temporaire chargé en mémoire au tout début du démarrage, avant que le système réel soit monté |
| ISO | Format d'image disque bootable, le livrable final du projet |
| Voie « oxidized » | Direction du projet consistant à privilégier des outils écrits en Rust ; réalisée dès la v1 au niveau du userland (uutils, bat, ripgrep, fd...) |
| uutils/coreutils | Réécriture en Rust des GNU Coreutils : fournit les commandes de base (`ls`, `cp`, `mv`...) sous leurs noms classiques, sans alias |
| Alias (shell) | Redirection définie uniquement dans le shell interactif (ex. `cat` → `bat`) ; les scripts utilisent toujours les binaires réels |
