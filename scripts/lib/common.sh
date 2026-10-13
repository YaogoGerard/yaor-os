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
LOG_DIR="$BUILD_DIR/logs"

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
# Colonnes : 1=groupe, 2=nom, 3=version, 4=url, 5=sha256.
manifest_field() {
    awk -F'\t' -v n="$1" -v c="$2" '$2 == n { print $c; exit }' "$SOURCES_DIR/manifest.tsv"
}

# source_archive <nom> : chemin local de l'archive téléchargée.
source_archive() {
    local url
    url="$(manifest_field "$1" 4)"
    printf '%s\n' "$SOURCES_DIR/$(basename "$url")"
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
    dir="${base%.tar.xz}"; dir="${dir%.tar.gz}"; dir="${dir%.tgz}"
    if [[ ! -d "$dest/$dir" ]]; then
        log "extraction : $base"
        mkdir -p "$dest"
        tar -xf "$tarball" -C "$dest"
    fi
    printf '%s\n' "$dest/$dir"
}

# src_dir <nom> : dossier des sources extraites d'un paquet du manifeste.
src_dir() { extract_tarball "$(source_archive "$1")"; }
