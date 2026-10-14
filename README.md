# YAOR OS

YAOR OS est une distribution Linux construite **depuis les sources**, destinée à
servir de fondation à un bureau hautement personnalisable. Ce dépôt contient le
pipeline de construction de la distribution ainsi que sa documentation
industrielle.

> **Reuse what exists. Prefer Rust when suitable. Build what is missing.**

Le noyau Linux est utilisé comme fondation ; les composants open source
appropriés sont réutilisés, et les implémentations propres à YAOR privilégient
Rust lorsqu'il est adapté.

## État du projet

**Jalon 0.1 — « minimal bootable » : atteint.**

- ISO live **x86_64**, amorçable en **BIOS** (El Torito) et **UEFI**, via **Limine**.
- Démarrage jusqu'à un **shell root** sous **systemd** (PID 1).
- Racine **immuable** : image **EROFS** compressée en zstd, superposée à un
  `tmpfs` inscriptible (`overlayfs`) par un **initramfs shell** minimal.
- Noyau Linux compressé en **zstd** (`CONFIG_KERNEL_ZSTD`).
- Image volontairement légère : **≈ 85 Mo** (l'image live est un système minimal
  amorçable, sans outils de compilation).

Les jalons suivants (bureau, compositor, sessions) sont décrits dans
`docs/01-product/roadmap.md`.

## Prérequis

Cible **x86_64**. L'hôte doit fournir :

- **Construction** : `gcc`, `make`, `bison`, `flex`, `m4`, `patch`, `perl`,
  `python3`, `tar`, `xz`, `gzip`, `sed`, `grep`, `awk`, `sha256sum`, `cpio`,
  `zstd`, `find`, `xargs`, et `wget` **ou** `curl`.
- **Assemblage / test de l'image** : `xorriso`, `mkfs.erofs`, `limine`,
  `qemu-system-x86_64` (et `gpg` pour signer l'ISO).

`scripts/00_check_host.sh` vérifie ces outils et s'arrête si un outil requis
manque.

## Construire

### En une commande

```bash
./scripts/build.sh
```

`build.sh` enchaîne les étapes 00 → 06 dans l'ordre, s'arrête à la première
erreur et n'élève les droits (via `sudo`) que pour les étapes qui l'exigent.

```bash
JOBS=6 ./scripts/build.sh          # parallélisme de compilation
SUDO=doas ./scripts/build.sh       # autre outil d'élévation de privilèges
./scripts/build.sh --from 04       # reprend à une étape donnée
./scripts/build.sh --list          # affiche les étapes
./scripts/build.sh --help
```

À lancer en **utilisateur normal** : les étapes 00–03 et 05 bâtissent sous ton
compte ; seules 04 et 06 passent par `sudo`. Bâtir tout en root laisserait des
fichiers appartenant à root dans `build/`.

### Étape par étape

Chaque étape est un script autonome, également utilisable seul :

| Étape | Rôle | Compte |
|---|---|---|
| `00_check_host.sh` | vérifie les outils de l'hôte | utilisateur |
| `01_download.sh` | télécharge et vérifie (SHA-256) les sources du manifeste | utilisateur |
| `02_toolchain.sh` | toolchain croisée temporaire (LFS chapitre 5) | utilisateur |
| `03_temp_tools.sh` | userland temporaire cross-compilé (LFS chapitre 6) | utilisateur |
| `04_chroot_build.sh` | système de base + systemd + noyau, dans un chroot | **root** |
| `05_initramfs.sh` | initramfs shell (script POSIX `/init`) | utilisateur |
| `06_make_iso.sh` | EROFS + Limine → ISO (BIOS/UEFI) | **root** |

```bash
./scripts/00_check_host.sh
./scripts/01_download.sh
./scripts/02_toolchain.sh
./scripts/03_temp_tools.sh
sudo ./scripts/04_chroot_build.sh
./scripts/05_initramfs.sh
sudo ./scripts/06_make_iso.sh
```

Le pipeline suit les grandes étapes de LFS. La **toolchain croisée** (02) puis
les **outils temporaires** (03) sont bâtis hors `chroot` par l'utilisateur ;
seule l'étape 04, qui bascule dans la racine cible (`chroot`), requiert root.
Une fois le système construit, la toolchain et les outils temporaires ne sont
plus nécessaires.

## Idempotence et reprise

Chaque étape réussie pose un **marqueur** dans `build/stamps/`. Relancer un
script (ou `build.sh`) **ne refait que ce qui manque** : il est donc sans danger
de relancer la chaîne entière.

Pour **forcer** la reconstruction d'un élément, supprimer son marqueur :

```bash
rm -f build/stamps/ch7.kernel    # puis relancer 04 : le noyau se recompilera
rm -f build/stamps/ch9.iso       # puis relancer 06 : l'ISO se reconstruira
rm -rf build/                    # tout reconstruire depuis zéro
```

Les journaux détaillés sont écrits dans `build/logs/<étape>.log`.

## Lancer l'image

Une fois l'ISO produite :

```bash
# Fenêtre graphique (GTK)
qemu-system-x86_64 -machine pc -enable-kvm -m 512 \
    -cdrom out/yaor-live-x86_64-<version>.iso -boot d -display gtk

# Console série (utile pour lire les messages de démarrage)
qemu-system-x86_64 -machine pc -enable-kvm -m 512 \
    -cdrom out/yaor-live-x86_64-<version>.iso -boot d -nographic
```

Le système démarre sur une console root (`bash-5.3#`). `poweroff` pour quitter.

## Artefacts

| Chemin | Contenu |
|---|---|
| `out/yaor-live-x86_64-<version>.iso` | ISO live (BIOS + UEFI) |
| `out/yaor-live-x86_64-<version>.iso.sha256` | empreinte SHA-256 (ADR-007) |
| `out/initrd.img-<version>` | initramfs |
| `out/rootfs.erofs` | image racine EROFS |
| `build/rootfs/` | racine cible (système construit) |
| `build/stamps/` | marqueurs d'idempotence |
| `build/logs/` | journaux du pipeline |
| `sources/` | archives téléchargées (voir `sources/manifest.tsv`) |

`build/`, `out/` et les archives de `sources/` ne sont pas versionnés ; seul le
manifeste `sources/manifest.tsv` l'est.

## Structure du dépôt

```text
config/          option fragment du noyau live (config/kernel-live.fragment)
docs/            documentation industrielle (voir docs/README.md)
patches/         correctifs appliqués aux sources amont
scripts/         pipeline de construction (00 → 06) + lib/ + build.sh
sources/         manifeste des sources (versions + empreintes épinglées)
```

## Documentation

- `docs/README.md` — index de la documentation.
- `docs/01-product/` — vision, glossaire, feuille de route.
- `docs/02-requirements/` — exigences.
- `docs/03-architecture-hld/deployment-ops.md` — modèle de construction et
  chaîne de démarrage live.
- `docs/adr/` — décisions d'architecture (ADR).
- `docs/07-contributing/commits.md` — convention de commit.

## Conventions

- Documentation en **français**, identifiants et code en **anglais**.
- Toute décision structurante fait l'objet d'un **ADR** ; un ADR accepté est
  remplacé (jamais réécrit) par un nouvel ADR.
- Commits atomiques, format `<type>(<zone>): <sujet>` avec les trailers
  `Jalon:`, `Feature:` et, le cas échéant, `Refs:` (voir `docs/07-contributing/commits.md`).
