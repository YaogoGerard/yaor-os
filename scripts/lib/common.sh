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
    printf '%s\n' "$msg"
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
