#!/usr/bin/env bash
# 07_publish.sh — produit les images de mise à jour et leurs manifestes signés.
#
# À partir du rootfs construit (étapes 04→05), assemble le jeu d'artefacts servi
# par le dépôt de mises à jour (ADR-007, ADR-015, ADR-022) :
#
#   out/update/root/yaor-root_x86-64_<v>.raw.xz   image racine (slots A/B)
#   out/update/root/SHA256SUMS[.gpg]              manifeste + signature
#   out/update/boot/vmlinuz-<v>                   noyau destiné à l'ESP
#   out/update/boot/SHA256SUMS[.gpg]              manifeste + signature
#
# <v> est la version produit (fichier VERSION, ADR-022). Elle apparie l'image
# racine et son noyau : les deux transferts de /etc/sysupdate.d les publient
# sous la même version.
#
# L'image racine est un système de fichiers **ext4** peuplé depuis le rootfs
# livré (mêmes exclusions que l'image live, « /boot » en plus), dimensionné pour
# tenir exactement dans un slot de 2 Gio (voir /etc/repart.d/30-root-a.conf).
#
# Cette étape n'est PAS dans le pipeline par défaut (00→06) : elle ne sert qu'à
# publier une mise à jour. Comme l'étape 06, elle exige root (propriété root des
# fichiers de l'image).
#
# Signature : détachée, produite lorsque YAOR_GPG_KEY désigne une clé GPG
# (ADR-007). Une image publiée DOIT être signée — systemd-sysupdate la vérifie
# (Verify=signature dans /etc/sysupdate.d).

set -euo pipefail
STEP=07_publish
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

LFS="$(realpath "$BUILD_DIR/rootfs")"
KVER="$(manifest_field linux 3)"
PRODVER="$(yaor_version)"
PUB_DIR="$OUT_DIR/update"
ROOT_DIR="$PUB_DIR/root"
BOOT_DIR="$PUB_DIR/boot"
STAGE="$BUILD_DIR/rootfs-install"
ROOT_IMG="$ROOT_DIR/yaor-root_x86-64_${PRODVER}.raw"
# Taille du slot racine (SizeMinBytes=SizeMaxBytes=2G, 30-root-a.conf) : l'image
# doit tenir exactement dedans.
SLOT_BYTES=$((2 * 1024 * 1024 * 1024))

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "étape 07 : exécution en root requise (propriété root de l'image). Utiliser : sudo $0"
fi

need_cmd rsync mkfs.ext4 truncate xz sha256sum

if ! stamp_exists "ch7.kernel"; then
    die "étape 04 incomplète : marqueur ch7.kernel manquant"
fi
[[ -f "$LFS/boot/vmlinuz-$KVER" ]] || die "noyau introuvable : $LFS/boot/vmlinuz-$KVER"

section "Staging du rootfs installable (mêmes exclusions que le live, /boot gardé)"
rm -rf "$STAGE"
mkdir -p "$STAGE"
yaor_rsync_image "$LFS/" "$STAGE/" keep

section "Vérification : fournisseurs non dupliqués (ADR-017)"
# Comme l'étape 06 : aucun binaire GNU coreutils/findutils ne doit subsister.
leftovers="$(grep -rl -e 'GNU coreutils' -e 'GNU findutils' \
    "$STAGE"/usr/bin "$STAGE"/usr/sbin 2>/dev/null || true)"
[[ -z "$leftovers" ]] || die "binaires GNU coreutils/findutils résiduels : $leftovers"

section "fstab de l'image installable"
# Contrairement à l'image live (où ces partitions n'existent pas), l'image
# installable porte un /etc/fstab : « systemd-gpt-auto-generator » ne monte pas
# un point de montage déjà peuplé (/boot, /var). Une mise à jour écrit une
# nouvelle racine, qui doit donc déjà monter l'ESP et /var (REQ-SW-026) ; le
# contenu est identique à celui écrit par yaor-install à la première
# installation (qui le conserve s'il est présent).
cat > "$STAGE/etc/fstab" <<'FSTAB'
# YAOR — partitions retrouvées par étiquette (schéma GPT créé par systemd-repart).
# /boot (ESP) et /var (données persistantes) survivent aux mises à jour et aux
# retours arrière, qui ne touchent que les slots racine (REQ-SW-026).
PARTLABEL=yaor-esp /boot vfat umask=0077,shortname=winnt 0 2
PARTLABEL=yaor-var /var  ext4 defaults                    0 2
FSTAB

section "Création de l'image racine ext4 ($((SLOT_BYTES / 1024 / 1024)) Mio)"
mkdir -p "$ROOT_DIR" "$BOOT_DIR"
rm -f "$ROOT_IMG" "$ROOT_IMG.xz"
truncate -s "$SLOT_BYTES" "$ROOT_IMG"
# -d peuple l'image sans montage (préserve la propriété des fichiers).
mkfs.ext4 -q -F -L yaor-root -d "$STAGE" "$ROOT_IMG"

section "Compression de l'image racine (xz)"
xz -T0 -9 "$ROOT_IMG"

section "Noyau destiné à l'ESP (vmlinuz-$PRODVER)"
install -m 0644 "$LFS/boot/vmlinuz-$KVER" "$BOOT_DIR/vmlinuz-$PRODVER"

section "Manifestes SHA-256 et signatures (ADR-007)"
sign_dir() {
    local d="$1"
    ( cd "$d" && rm -f SHA256SUMS SHA256SUMS.gpg && sha256sum --binary -- * > SHA256SUMS )
    log "manifeste : $d/SHA256SUMS"
    if [[ -n "${YAOR_GPG_KEY:-}" ]]; then
        need_cmd gpg
        gpg --batch --yes --armor --detach-sign --local-user "$YAOR_GPG_KEY" \
            --output "$d/SHA256SUMS.gpg" "$d/SHA256SUMS"
        log "signature : $d/SHA256SUMS.gpg"
    else
        log "signature ignorée : YAOR_GPG_KEY non défini (publication de développement)"
    fi
}
sign_dir "$ROOT_DIR"
sign_dir "$BOOT_DIR"

stamp_done "ch10.publish"
log "artefacts de mise à jour dans $PUB_DIR :"
ls -lh "$ROOT_DIR" "$BOOT_DIR" >&2
