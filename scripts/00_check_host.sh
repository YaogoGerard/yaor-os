#!/usr/bin/env bash
# 00_check_host.sh — vérifie que l'hôte dispose des outils nécessaires.
#
# Les outils « requis » doivent être présents pour bâtir la toolchain et le
# système (étapes 01→03). Les outils « ultérieurs » ne servent qu'à assembler
# et tester l'image live (étapes 04→05) : leur absence est signalée sans
# interrompre le pipeline.

set -euo pipefail
STEP=00_check_host
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

section "Vérification de l'hôte"

arch="$(uname -m)"
log "système : $(uname -srm)"
[[ "$arch" == "x86_64" ]] || log "AVERTISSEMENT : cible attendue x86_64, hôte '$arch'"

# Outils nécessaires pour construire la toolchain et le système.
NOW=(gcc make bison flex m4 patch perl python3 tar xz gzip sed grep awk
     sha256sum cpio zstd find xargs)

# Outils nécessaires pour l'assemblage et le test de l'image live.
LATER=(xorriso mkfs.erofs limine qemu-system-x86_64 gpg)

# Outil de téléchargement : au moins l'un des deux.
DOWNLOAD=(wget curl)

missing_now=() missing_later=()
for c in "${NOW[@]}"; do command -v "$c" >/dev/null 2>&1 || missing_now+=("$c"); done
for c in "${LATER[@]}"; do command -v "$c" >/dev/null 2>&1 || missing_later+=("$c"); done

have_dl=0
for c in "${DOWNLOAD[@]}"; do
    if command -v "$c" >/dev/null 2>&1; then have_dl=1; fi
done
((have_dl == 1)) || missing_now+=("wget|curl")

if ((${#missing_now[@]} == 0)); then
    log "Outils requis : OK"
else
    log "Outils requis MANQUANTS : ${missing_now[*]}"
fi

if ((${#missing_later[@]})); then
    log "Outils ultérieurs absents (04→05) : ${missing_later[*]}"
    log "  → installe-les avant d'assembler/tester l'ISO."
else
    log "Outils ultérieurs : OK"
fi

if ((${#missing_now[@]})); then
    die "hôte non prêt : ${#missing_now[@]} outil(s) requis manquant(s)"
fi

log "Hôte prêt pour le pipeline."
