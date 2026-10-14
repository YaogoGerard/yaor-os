#!/usr/bin/env bash
# build.sh — orchestre le pipeline complet de YAOR OS (étapes 00 → 06).
#
# Enchaîne les scripts d'étape dans l'ordre, s'arrête à la première erreur et
# n'élève les droits que pour les étapes qui l'exigent (04 et 06). Chaque étape
# reste idempotente : relancer ce script ne refait que ce qui manque (marqueurs
# dans build/stamps/).
#
# Usage :
#   ./scripts/build.sh                 # pipeline complet
#   JOBS=6 ./scripts/build.sh          # parallélisme de compilation
#   SUDO=doas ./scripts/build.sh       # autre outil d'élévation de privilèges
#   ./scripts/build.sh --from 04       # reprend à une étape donnée
#   ./scripts/build.sh --list          # affiche les étapes
#   ./scripts/build.sh --help
#
# À lancer en UTILISATEUR NORMAL : les étapes 00–03 et 05 bâtissent sous ton
# compte ; seules 04 (chroot) et 06 (image EROFS/ISO) passent par sudo.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Étapes du pipeline. Suffixe « :root » pour celles qui exigent les privilèges.
STEPS=(
    00_check_host.sh
    01_download.sh
    02_toolchain.sh
    03_temp_tools.sh
    04_chroot_build.sh:root
    05_initramfs.sh
    06_make_iso.sh:root
)

SUDO="${SUDO:-sudo}"

usage() {
    # Affiche le bloc d'en-tête : toutes les lignes de commentaire qui suivent
    # le shebang, jusqu'à la première ligne de code.
    awk 'NR == 1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
}

list_steps() {
    local entry script
    for entry in "${STEPS[@]}"; do
        script="${entry%%:*}"
        [[ "$entry" == *:root ]] && printf '  %s  (root)\n' "$script" || printf '  %s\n' "$script"
    done
}

FROM=""
while (($#)); do
    case "$1" in
        --from)    FROM="${2:?--from attend une étape valide}"; shift 2 ;;
        --from=*)  FROM="${1#*=}"; shift ;;
        --list)    list_steps; exit 0 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "build.sh : option inconnue « $1 »" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
    echo "build.sh : lance ce script en utilisateur normal (les étapes 04 et 06" >&2
    echo "           passent par '$SUDO'). Bâtir tout en root laisserait des fichiers" >&2
    echo "           appartenant à root dans build/." >&2
    exit 1
fi

# --from : ne démarrer qu'à l'étape dont le nom commence par la valeur donnée.
if [[ -n "$FROM" ]]; then
    found=0
    for entry in "${STEPS[@]}"; do
        [[ "${entry%%:*}" == "$FROM"* ]] && found=1
    done
    if ((found == 0)); then
        echo "build.sh : aucune étape ne commence par « $FROM »." >&2
        list_steps >&2
        exit 2
    fi
fi

started=0
n=0
total=${#STEPS[@]}
for entry in "${STEPS[@]}"; do
    script="${entry%%:*}"
    need_root=0
    [[ "$entry" == *:root ]] && need_root=1

    if [[ -n "$FROM" && $started -eq 0 ]]; then
        if [[ "$script" == "$FROM"* ]]; then
            started=1
        else
            continue
        fi
    fi

    n=$((n + 1))
    if (( need_root )); then
        printf '\n=== [%d/%d] %s (root via %s) ===\n' "$n" "$total" "$script" "$SUDO"
        "$SUDO" "$ROOT/scripts/$script"
    else
        printf '\n=== [%d/%d] %s ===\n' "$n" "$total" "$script"
        "$ROOT/scripts/$script"
    fi
done

printf '\n=== Pipeline terminé ===\n'
ls -lh "$ROOT/out/"
