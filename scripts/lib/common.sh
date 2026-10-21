#!/usr/bin/env bash
# lib/common.sh — helpers partagés par le pipeline de build YAOR.
#
# Utilisation, en tête de chaque script d'étape :
#     STEP=01_download
#     source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

set -euo pipefail

# Racine du dépôt : dossier parent de scripts/.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

SOURCES_DIR="$ROOT/sources"
BUILD_DIR="$ROOT/build"
OUT_DIR="$ROOT/out"
CONFIG_DIR="$ROOT/config"
# Composants internes (Rust) livrés par le dépôt : sources, pas des archives
# téléchargées.
COMPONENTS_DIR="$ROOT/components"
LOG_DIR="$BUILD_DIR/logs"

# Identité du produit. La version vit dans le fichier VERSION à la racine du
# dépôt (source unique, ADR-022) ; elle devient IMAGE_VERSION dans
# /etc/os-release et sert à nommer l'ISO et les images de mise à jour.
YAOR_IMAGE_ID="yaor"
yaor_version() {
    local f="$ROOT/VERSION" v
    [[ -f "$f" ]] || die "fichier VERSION introuvable : $f"
    v="$(tr -d '[:space:]' < "$f")"
    [[ -n "$v" ]] || die "fichier VERSION vide : $f"
    printf '%s\n' "$v"
}

# Motifs exclus de l'arborescence livrée : tout ce qui relève de la construction
# (toolchain, sources, artefacts de compilation, outils de développement) et non
# du système en fonctionnement. Partagés par l'étape 06 (image live EROFS) et
# l'étape 07 (image installable), qui livrent le MÊME système. Seul /boot diffère
# (présent dans l'image installable, retiré du live où Limine charge le noyau
# depuis l'ISO).
YAOR_IMAGE_EXCLUDES=(
    '/proc/*' '/sys/*' '/dev/*' '/run/*' '/tmp/*'
    '/build/work/*' '/build/config/*' '/build/components/*'
    '/sources/*' '/tools/*' '/opt/*'
    '/.cache/*' '*.log'
    '/usr/libexec/gcc/*' '/usr/lib/gcc/*'
    '/usr/x86_64-lfs-linux-gnu/*'
    '/usr/include/*'
    '/usr/share/doc/*' '/usr/share/man/*' '/usr/share/info/*'
    '/usr/share/gtk-doc/*' '/usr/share/locale/*' '/usr/share/i18n/*'
    '/usr/share/aclocal/*' '/usr/share/gettext/*'
    '/usr/share/vim/*'
    '/usr/lib/meson/*'
    '/usr/bin/meson*' '/usr/bin/ninja*'
    '*.a' '*.la'
    '/usr/bin/*gcc*' '/usr/bin/*g++*'
    '/usr/bin/*cpp*' '/usr/bin/*c++*'
    '/usr/bin/cc' '/usr/bin/c99'
    '/usr/bin/x86_64-pc-linux-gnu-*'
    '/usr/bin/x86_64-lfs-linux-gnu-*'
    '/usr/bin/ld' '/usr/bin/ld.bfd' '/usr/bin/ld.gold'
    '/usr/bin/as' '/usr/bin/ar' '/usr/bin/ranlib'
    '/usr/bin/nm' '/usr/bin/objcopy' '/usr/bin/objdump'
    '/usr/bin/readelf' '/usr/bin/strip' '/usr/bin/size'
    '/usr/bin/strings' '/usr/bin/addr2line' '/usr/bin/elfedit'
    '/usr/bin/gprof*' '/usr/bin/c++filt'
    '/usr/lib/bfd-plugins/*'
    '/usr/bin/bison' '/usr/bin/yacc' '/usr/share/bison/*'
    '/usr/bin/flex' '/usr/bin/lex'
    '/usr/bin/python3*' '/usr/lib/python3.13/*'
    '/usr/bin/perl*' '/usr/lib/perl5/*'
    '/usr/bin/file' '/usr/lib/libmagic*'
    '/usr/share/misc/magic.mgc'
)

# yaor_rsync_image <src> <dst> [keep|drop] — synchronise l'arborescence livrée en
# appliquant YAOR_IMAGE_EXCLUDES. « drop » retire en outre /boot (image live) ;
# « keep » (défaut) le conserve (image installable).
yaor_rsync_image() {
    local src="$1" dst="$2" boot="${3:-keep}" e
    local args=(-a --delete)
    for e in "${YAOR_IMAGE_EXCLUDES[@]}"; do args+=(--exclude="$e"); done
    [[ "$boot" == "drop" ]] && args+=(--exclude='/boot/*')
    rsync "${args[@]}" "$src/" "$dst/"
}

# Nom de l'étape courante (surchargé avant le source) et son journal.
STEP="${STEP:-pipeline}"
LOG_FILE="$LOG_DIR/${STEP}.log"

mkdir -p "$LOG_DIR"

log() {
    local msg
    msg="[$(date '+%Y-%m-%d %H:%M:%S')] $*"
    printf '%s\n' "$msg" >&2
    printf '%s\n' "$msg" >> "$LOG_FILE"
}

die() {
    log "ERREUR : $*"
    exit 1
}

section() {
    log "----- $* -----"
}

# need_cmd <commande...> : échoue si l'une des commandes est absente.
need_cmd() {
    local missing=() c
    for c in "$@"; do
        command -v "$c" >/dev/null 2>&1 || missing+=("$c")
    done
    ((${#missing[@]} == 0)) || die "commande(s) manquante(s) : ${missing[*]}"
}

# manifest_field <nom> <colonne> : lit une colonne du manifeste.
# Colonnes : 1=groupe, 2=nom, 3=version, 4=url, 5=sha256, 6=fichier (nom local,
# optionnel). Une colonne absente est renvoyée vide.
manifest_field() {
    awk -F'\t' -v n="$1" -v c="$2" '$2 == n { print $c; exit }' "$SOURCES_DIR/manifest.tsv"
}

# source_archive <nom> : chemin local de l'archive téléchargée. Le nom du
# fichier est celui de la colonne 6 s'il est fourni, sinon celui de l'URL.
source_archive() {
    local url file
    url="$(manifest_field "$1" 4)"
    file="$(manifest_field "$1" 6)"
    [[ -n "$file" ]] || file="$(basename "$url")"
    printf '%s\n' "$SOURCES_DIR/$file"
}

# stamp_done <étape> : marque une étape terminée (idempotence).
STAMP_DIR="$BUILD_DIR/stamps"
stamp_done() { mkdir -p "$STAMP_DIR"; touch "$STAMP_DIR/$1"; }
stamp_exists() { [[ -f "$STAMP_DIR/$1" ]]; }

# extract_tarball <archive> [dest] : extrait une archive si nécessaire et
# affiche le dossier obtenu. dest vaut $WORK (ou build/work) par défaut.
extract_tarball() {
    local tarball="$1" dest="${2:-${WORK:-$BUILD_DIR/work}}" base dir
    base="$(basename "$tarball")"
    dir="${base%.tar.xz}"; dir="${dir%.tar.gz}"; dir="${dir%.tgz}"; dir="${dir%.tar.bz2}"
    # Fiabilité : le nom dérivé peut différer de la racine réelle (archives
    # « .orig » de Debian, keyutils_1.6.3.orig.tar.gz → keyutils-1.6.3) ; la
    # véritable racine est le premier composant de la liste des membres.
    if [[ ! -d "$dest/$dir" ]]; then
        top="$(tar -tf "$tarball" 2>/dev/null | head -1)"
        top="${top%%/*}"
        [[ -n "$top" ]] && dir="$top"
    fi
    if [[ ! -d "$dest/$dir" ]]; then
        log "extraction : $base"
        mkdir -p "$dest"
        # NB : --no-same-owner — dans un build sans privilèges réels (espace
        # d'utilisateurs, ADR-001), « chown » vers un uid/gid de l'archive non
        # mappé échoue (EINVAL) ; tout le contenu appartient à l'utilisateur
        # courant (vu root=0 dans le chroot), ce qui est correct pour l'image.
        tar --no-same-owner -xf "$tarball" -C "$dest"
    fi
    printf '%s\n' "$dest/$dir"
}

# src_dir <nom> : dossier des sources extraites d'un paquet du manifeste.
src_dir() { extract_tarball "$(source_archive "$1")"; }
