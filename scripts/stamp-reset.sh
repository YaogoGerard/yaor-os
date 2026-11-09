#!/usr/bin/env bash
# stamp-reset.sh — retire des marqueurs de build (ch7.*, ch8.initramfs,
# ch9.iso, ch10.publish). Utilitaire de développement : « sudo
# scripts/stamp-reset.sh ch7.yaor-components ch8.initramfs » invalide la
# cascade avant un « sudo ./scripts/build.sh --from 04 ».
# Exige la création de l'utilisateur root (comme les phases de build).
set -euo pipefail

STEP=stamp-reset
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "stamp-reset : exécution en root requise. Utiliser : sudo $0 …"
fi

if (( $# == 0 )); then
    die "usage : sudo $0 <marqueur>… (ex. ch7.yaor-components ch9.iso)"
fi

for stamp in "$@"; do
    case "$stamp" in
        (ch[0-9]*|*.publish) ;;
        *) die "marqueur suspecté invalide : $stamp (format ch<9>* attendu)" ;;
    esac
    if [[ -f "$STAMP_DIR/$stamp" ]]; then
        rm -f "$STAMP_DIR/$stamp"
        log "marqueur retiré : $stamp"
    else
        log "marqueur déjà absent : $stamp"
    fi
done
