#!/usr/bin/env bash
# 01_download.sh — télécharge les sources du manifeste et vérifie leur SHA-256.
#
# Le manifeste (sources/manifest.tsv) liste, séparés par tabulation :
#     groupe, nom, version, url, sha256
# Un champ sha256 « - » demande à épingler l'empreinte : le script affiche la
# valeur calculée au lieu d'échouer (utile lors de l'ajout d'un paquet).
#
# Les archives manquantes sont téléchargées EN PARALLÈLE (JOBS connexions, 8 par
# défaut) : les miroirs limitent souvent le débit d'une connexion unique, et le
# parallélisme augmente fortement le débit agrégé. La vérification des empreintes
# reste séquentielle (rapide).

set -euo pipefail
STEP=01_download
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

MANIFEST="$SOURCES_DIR/manifest.tsv"
[[ -f "$MANIFEST" ]] || die "manifeste introuvable : $MANIFEST"

need_cmd sha256sum
mkdir -p "$SOURCES_DIR"

JOBS="${JOBS:-8}"

# Miroir GNU optionnel. Le manifeste référence les URL canoniques (ftp.gnu.org) ;
# définir GNU_MIRROR accélère le téléchargement sans changer les empreintes :
#   GNU_MIRROR=https://mirrors.kernel.org/gnu ./scripts/01_download.sh
GNU_MIRROR="${GNU_MIRROR:-}"

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fL -sS --retry 3 --retry-delay 2 -C - -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q --show-progress -c -O "$2" "$1"; }
else
    die "ni curl ni wget disponibles"
fi

resolve_url() {
    local url="$1"
    if [[ -n "$GNU_MIRROR" && "$url" == https://ftp.gnu.org/gnu/* ]]; then
        printf '%s%s\n' "${GNU_MIRROR%/}" "${url#https://ftp.gnu.org/gnu}"
    else
        printf '%s\n' "$url"
    fi
}

# --- 1) Téléchargement parallèle des archives manquantes --------------------
#
# Les champs (url, fichier, nom) ne contiennent pas d'espace ; le tabulateur et
# l'espace servent donc tous deux de séparateur pour xargs.

dl_list="$(mktemp)"
dl_worker=""
trap 'rm -f "$dl_list" "$dl_worker"' EXIT

while IFS=$'\t' read -r group name version url expected; do
    [[ -z "$name" || "$group" == "groupe" || "$group" == \#* ]] && continue
    file="$SOURCES_DIR/$(basename "$url")"
    [[ -f "$file" ]] && continue
    printf '%s\t%s\t%s\n' "$(resolve_url "$url")" "$file" "$name" >> "$dl_list"
done < "$MANIFEST"

if [[ -s "$dl_list" ]]; then
    # curl/wget de secours selon l'outil disponible.
    if command -v curl >/dev/null 2>&1; then
        worker_fetch='curl -fL -sS --retry 8 --retry-all-errors --retry-delay 3 -C - -o "$part" "$url"'
    else
        worker_fetch='wget -q -c -O "$part" "$url"'
    fi

    dl_worker="$(mktemp)"
    cat > "$dl_worker" <<HELPER
#!/usr/bin/env bash
set -u
url="\$1"; file="\$2"; name="\$3"; part="\$file.part"
if $worker_fetch; then
    mv -f "\$part" "\$file" && { echo "OK   \$name"; exit 0; }
fi
echo "ÉCHEC \$name (\$url)" >&2
exit 1
HELPER
    chmod +x "$dl_worker"

    section "Téléchargement parallèle ($(wc -l < "$dl_list") archive(s), $JOBS connexions)"
    # shellcheck disable=SC2016
    xargs -P "$JOBS" -n3 "$dl_worker" < "$dl_list" || die "un ou plusieurs téléchargements ont échoué"
fi

# --- 2) Vérification séquentielle des empreintes ----------------------------

section "Vérification des sources"

fail=0
count=0
while IFS=$'\t' read -r group name version url expected; do
    [[ -z "$name" || "$group" == "groupe" || "$group" == \#* ]] && continue
    count=$((count + 1))

    file="$SOURCES_DIR/$(basename "$url")"
    log "--- $name $version ($group) ---"

    if [[ ! -f "$file" ]]; then
        src="$(resolve_url "$url")"
        log "téléchargement : $src"
        fetch "$src" "$file.part" || die "échec du téléchargement de $name"
        mv "$file.part" "$file"
    fi

    actual="$(sha256sum "$file" | cut -d' ' -f1)"
    if [[ "$expected" == "-" ]]; then
        log "À ÉPINGLER dans manifest.tsv : $actual  ($name)"
    elif [[ "$actual" != "$expected" ]]; then
        log "SOMME INVALIDE pour $name"
        log "  attendu : $expected"
        log "  obtenu  : $actual"
        fail=1
    else
        log "OK : $name"
    fi
done < "$MANIFEST"

((count > 0)) || die "aucune entrée lue dans $MANIFEST"
((fail == 0)) || die "une ou plusieurs archives sont invalides"
log "Terminé : $count archive(s) vérifiée(s)."
