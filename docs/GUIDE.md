# yaor-os — Guide de prise en main

## 1. C'est quoi ?

Une mini-distribution GNU/Linux « live » que tu construis toi-même, entièrement,
en lançant 5 scripts. Elle tient sur une clé USB, démarre sur n'importe quel PC
x86_64 (vieux BIOS ou UEFI récent) sans rien installer, et te donne un shell
équipé d'outils modernes écrits en Rust, accessibles sous les noms classiques.

Deux objectifs : **comprendre** comment un système d'exploitation s'assemble,
et **obtenir** un outil utile (dépannage, démo).

## 2. Structure du projet

```
scripts/          Les 5 étapes du pipeline (voir §3)
config/           Fichiers de configuration (voir §4)
sources/          Cache des archives téléchargées + manifest.tsv (liste versionnée)
build/            Zone de travail : toolchain, chroot, journaux — supprimable/refaisable
out/              Résultat final : l'ISO + SHA256SUMS
live-skeleton/    Squelette du contenu du système live (boot/, live/)
docs/             Cette documentation
```

## 3. Le pipeline : 5 étapes, dans l'ordre

```bash
./scripts/01_download.sh            # 1. télécharge les sources + vérifie SHA256
./scripts/02_toolchain.sh           # 2. fabrique le compilateur croisé
sudo ./scripts/03_chroot_build.sh   # 3. compile le noyau + BusyBox dans une racine isolée
./scripts/04_initramfs.sh           # 4. fabrique le mini-système qui trouve l'ISO au boot
./scripts/05_make_iso.sh            # 5. assemble l'ISO finale dans out/
```

- Chaque étape écrit son journal dans `build/logs/<étape>.log`.
- Chaque étape peut être relancée seule après un échec (pas besoin de tout refaire).
- Compte ~1 h pour un build complet (cache des sources conservé).
- Le même pipeline tourne automatiquement sur GitHub Actions à chaque push :
  l'ISO est produite et testée sans intervention (voir `deployment-ops.md`).

## 4. À quoi sert le dossier `config/` ?

Ce sont les fichiers de configuration **déclaratifs** que lisent les scripts :

| Fichier | Rôle |
|---|---|
| `kernel.config` | Configuration du noyau Linux (format Kconfig). Décide ce qui est inclus dans le noyau : pilotes disques (AHCI/SATA), clés USB, lecture CD (ISO9660), SquashFS, OverlayFS, console série pour les tests… |
| `grub.cfg` | Menu du chargeur de démarrage GRUB inscrit dans l'ISO : l'entrée « yaor-os » qui charge le noyau et l'initramfs. |
| `fstab` | Réservé au futur mode « installé sur disque ». La V1 live ne l'utilise pas. |

Modifier `kernel.config` change ce que saura faire ton OS ; modifier `grub.cfg`
change ce qui se passe au démarrage.

## 5. Outils et équivalences (couche « oxidized »)

Le système embarque des outils écrits en Rust. Les commandes de base (`ls`, `cp`,
`mv`, `chmod`...) sont fournies par **uutils/coreutils**, qui remplace nativement
les GNU Coreutils sous les mêmes noms — pas d'alias nécessaire. Les outils
complémentaires, eux, sont accessibles depuis le shell interactif via des aliases
qui reprennent les noms classiques :

| Commande classique | Outil derrière l'alias | Effet |
|---|---|---|
| `cat` | `bat` | Affichage coloré, pagination désactivée |
| `grep` | `rg` (ripgrep) | Recherche rapide, options courantes compatibles |
| `find` | `fd` | Syntaxe simplifiée : `fd motif` au lieu de `find . -name motif` |
| `du` | `dust` | Vue triée et lisible des tailles |
| `ps` | `procs` | Tableau clair des processus |
| `hexdump` | `hexyl` | Hexadécimal coloré |

Trois points importants :

- Ces aliases ne s'appliquent **qu'au shell interactif** : un script utilise
  toujours les vrais binaires installés sur le disque (RG-006).
- Le socle GNU minimal reste présent sous ses vrais noms (`grep`, `find`, `sed`,
  `awk`, `tar`...). Préfixer une commande d'un anti-slash (`\grep`) ou utiliser
  son chemin absolu contourne l'alias (RG-007).
- `sed`, `awk` et `tar` ne sont volontairement pas remplacés : leurs équivalents
  Rust changeraient trop les habitudes.

## 6. Tester ton ISO (machine virtuelle)

```bash
# Mode BIOS
qemu-system-x86_64 -m 1024 -cdrom out/yaor-os-<VERSION>.iso

# Mode UEFI
qemu-system-x86_64 -m 1024 -cdrom out/yaor-os-<VERSION>.iso \
  -bios /usr/share/OVMF/OVMF_CODE.fd
```

Résultat attendu : menu GRUB → messages discrets → invite de shell BusyBox.

## 7. Outils à installer (prérequis)

Tout se fait depuis une machine **Linux x86_64**. Aucun droit root nécessaire,
sauf pour l'étape 3 (`chroot_build.sh`).

Installation en une commande — Debian / Ubuntu / Mint :

```bash
sudo apt install build-essential bison flex texinfo m4 patch perl python3 \
  bc libelf-dev libssl-dev wget xorriso mtools dosfstools \
  grub-pc-bin grub-efi-amd64-bin qemu-system-x86 ovmf shellcheck shfmt
```

Équivalents : Fedora (`dnf install …`) et Arch (`pacman -S …`) — mêmes noms
d'outils, paquets parfois regroupés différemment.

| Outil | À quoi il sert dans ce projet |
|---|---|
| `build-essential` | gcc, g++, make : compilateurs de base de l'hôte |
| `bison`, `flex`, `texinfo`, `m4`, `patch`, `perl`, `python3` | Dépendances classiques pour compiler gcc/glibc/noyau |
| `bc`, `libelf-dev`, `libssl-dev` | Bibliothèques exigées par la compilation du noyau Linux |
| `wget` | Télécharge les sources (étape 1) |
| `xorriso` | Assemble l'image ISO (étape 5) |
| `grub-pc-bin` + `grub-efi-amd64-bin` | GRUB pour BIOS **et** UEFI (une seule ISO pour les deux) |
| `mtools`, `dosfstools` | Manipulent la petite partition FAT qui contient GRUB/UEFI |
| `qemu-system-x86` + `ovmf` | Testent l'ISO en machine virtuelle (BIOS et UEFI) sans graver |
| `shellcheck`, `shfmt` | Vérifient la qualité des scripts Bash |

Vérification rapide que tout est là :

```bash
command -v gcc make wget xorriso mformat qemu-system-x86_64 shellcheck \
  && echo "OK: outils présents"
```

Espace disque : ~20 Go libres sous `build/`.

## 8. Vocabulaire minimum

| Mot | Sens ici |
|---|---|
| Toolchain croisée | Compilateur qui fabrique du code pour l'OS cible, pas pour ta machine |
| Chroot | Racine isolée où on installe le futur contenu de l'OS |
| Initramfs | Mini-archive chargée en RAM au boot : trouve l'ISO, monte le système, lance le shell |
| SquashFS | Le système de fichiers compressé en lecture seule qui contient tout l'OS |
| Live | Ça tourne depuis l'USB/CD, rien n'est installé, rien n'est modifié sur le PC |
