# Architecture logicielle (SAD) — yaor-os

**Statut :** Brouillon
**Version :** 0.2

## 1. Découpage en modules

Le système est construit par une chaîne de scripts Bash séquentiels, chacun
responsable d'une seule étape, avec une entrée et une sortie claires.

| Script | Entrée | Sortie |
|---|---|---|
| `01_download.sh` | Liste de sources (URLs + versions) | Archives vérifiées (SHA-256) |
| `02_toolchain.sh` | Archives sources | Toolchain temporaire compilée |
| `03_chroot_build.sh` | Toolchain temporaire | `rootfs/` complet |
| `04_initramfs.sh` | `rootfs/` | `initramfs` embarqué dans `boot/` |
| `05_make_iso.sh` | `rootfs/` + `initramfs` | Image `.iso` finale |

## 2. Choix techniques

- **Toolchain de construction** : GNU classique (GCC, Glibc, Binutils) — voir RG-003.
- **Userland** : outils Rust — uutils/coreutils en remplacement natif des GNU
  Coreutils, complétés par bat, ripgrep, fd, dust, procs et hexyl, exposés via
  aliases interactifs (voir RG-006, RG-007) ; socle GNU minimal (bash, grep,
  findutils, sed, gawk, tar) conservé pour la fiabilité des scripts.
- **Langage des scripts** : Bash, pour rester proche des standards LFS et facilement
  auditable par tout contributeur.
- **Format du rootfs compressé** : SquashFS (lecture seule, standard pour les live OS).
- **Génération de l'ISO** : `xorriso`, avec options hybrides BIOS + UEFI.

## 3. Flux de données

Voir `system-overview.md` (niveau 2, C4) pour le schéma complet du flux entre scripts
et artefacts.
