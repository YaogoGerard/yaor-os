#!/usr/bin/env bash
set -euo pipefail

# variables
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$ROOT/sources"
MANIFEST="$ROOT/sources/manifest.tsv"
LOG_FILE="$ROOT/build/logs/01_download.log"
mkdir -p "$(dirname "$LOG_FILE")"

log() { tee -a "$LOG_FILE" <<< "$*"; }

while IFS=$'\t' read -r name version url expected; do
    [[ $name == nom ]] && continue
    log "--- $name $version ---"

    file="$SRC_DIR/$(basename "$url")"

    if [[ ! -f $file ]]; then
        wget -O "$file" "$url"
    fi

    actual="$(sha256sum "$file" | cut -d' ' -f1)"

    if [[ $expected == "-" ]]; then
        log "À épingler dans manifest.tsv : $actual  ($name)"
    elif [[ $actual != "$expected" ]]; then
        log "ERREUR : somme invalide pour $name"
        log "attendu : $expected"
        log "obtenu  : $actual"
        exit 1
    else
        log "OK : $name"
    fi
done < "$MANIFEST"

log "Terminé."
