#!/usr/bin/env bash
# 06_make_iso.sh — assemble l'ISO live YAOR (Limine, BIOS+UEFI, rootfs EROFS).
#
# Chaîne (ADR-011) :
#   - une image racine EROFS compressée (zstd) est produite depuis un rootfs
#     minimal (staging nettoyé, binaires strippés) ;
#   - Limine amorce le noyau + initramfs (initramfs shell, étape 05, ADR-011) ;
#   - l'initramfs monte l'EROFS en lecture seule et le superpose à un tmpfs
#     inscriptible (overlayfs), puis bascule vers systemd du système réel.
#
# L'ISO produite est hybride : amorçable en BIOS (El Torito) et en UEFI.
# Une empreinte SHA-256 est toujours écrite ; la signature détachée GPG est
# produite lorsque YAOR_GPG_KEY désigne une clé (ADR-007).

set -euo pipefail
STEP=06_make_iso
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

LFS="$(realpath "$BUILD_DIR/rootfs")"
KVER="$(manifest_field linux 3)"
# Nom de l'ISO par la version produit (fichier VERSION, ADR-022) ; la version du
# noyau reste consignée dans l'image (/etc/os-release, /boot).
ISO_NAME="yaor-live-x86_64-$(yaor_version).iso"
ISO_OUT="$OUT_DIR/$ISO_NAME"
EROFS_IMG="$OUT_DIR/rootfs.erofs"
INITRD="$OUT_DIR/initrd.img-$KVER"
VMLINUZ="$LFS/boot/vmlinuz-$KVER"
ROOTFS_CLEAN="$BUILD_DIR/rootfs-live"
LIMINE_DIR="/usr/share/limine"

# L'image EROFS doit préserver la propriété root des fichiers du système :
# la construction se fait donc en root (comme l'étape 04).
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    die "étape 06 : exécution en root requise (propriété root de l'image). Utiliser : sudo $0"
fi

need_cmd xorriso mkfs.erofs rsync limine

if ! stamp_exists "ch8.initramfs"; then
    die "étape 05 incomplète : ch8.initramfs manquant"
fi
[[ -f "$VMLINUZ" ]] || die "vmlinuz introuvable : $VMLINUZ"
[[ -f "$INITRD" ]]   || die "initrd introuvable : $INITRD"
[[ -f "$LIMINE_DIR/limine-bios.sys" ]] || die "fichiers Limine introuvables dans $LIMINE_DIR"

if stamp_exists "ch9.iso"; then
    log "phase déjà faite : iso (ch9.iso)"
    exit 0
fi

section "Préparation du rootfs minimal (staging)"
rm -rf "$ROOTFS_CLEAN"
mkdir -p "$ROOTFS_CLEAN"

# On ne conserve que ce qui est nécessaire au démarrage et à l'exécution :
# rien des artefacts de compilation (build/work), de la toolchain temporaire
# (tools), des sources, ni des outils de développement.
#
# L'image live est un système minimal amorçable, pas un environnement de
# développement : les outils de compilation sont écartés (gcc et binutils,
# python3, perl, bison, flex, file), de même que les copies de /boot, déjà
# portées séparément par l'ISO. Ni systemd, ni glibc, ni les bibliothèques de
# base n'en dépendent au démarrage. La liste des exclusions est partagée avec
# l'image installable (étape 07) — voir YAOR_IMAGE_EXCLUDES dans common.sh.
yaor_rsync_image "$LFS/" "$ROOTFS_CLEAN/" drop

mkdir -p "$ROOTFS_CLEAN"/{proc,sys,dev,run,tmp,mnt,build,sources}

section "Vérification : fournisseurs non dupliqués (ADR-017)"
# Les composants Rust (uutils) remplacent GNU coreutils/findutils (étape 04).
# Aucun binaire GNU ne doit subsister : les chaînes « GNU coreutils » /
# « GNU findutils » figurent dans leur --version et jamais dans les binaires Rust.
leftovers="$(grep -rl -e 'GNU coreutils' -e 'GNU findutils' \
    "$ROOTFS_CLEAN"/usr/bin "$ROOTFS_CLEAN"/usr/sbin 2>/dev/null || true)"
if [[ -n "$leftovers" ]]; then
    log "binaires GNU coreutils/findutils résiduels :"
    printf '%s\n' "$leftovers" | sed "s#$ROOTFS_CLEAN/##" >&2
    die "intégration Rust incomplète : des binaires GNU subsistent (ADR-017)"
fi
log "aucun binaire GNU coreutils/findutils résiduel"

section "Stripping des binaires"
find "$ROOTFS_CLEAN" -type f -executable 2>/dev/null | while read -r f; do
    case "$(file -b "$f")" in
        *ELF*executable*|*ELF*shared*object*)
            strip --strip-unneeded "$f" 2>/dev/null || true
            ;;
    esac
done

section "Limine embarqué dans le média live (ADR-010/ADR-011, ADR-023)"
# Limine est un outil de build de l'hôte : il n'est pas construit par le
# pipeline. Le média live doit l'embarquer pour que `yaor-install` (exécuté
# depuis le live) écrive le chargeur sur le disque cible : le binaire, qui sert
# au `bios-install`, et ses fichiers de démarrage (BOOTX64.EFI, limine-bios.sys).
install -Dm755 "$(command -v limine)" "$ROOTFS_CLEAN/usr/bin/limine"
mkdir -p "$ROOTFS_CLEAN/usr/share/limine"
cp -a "$LIMINE_DIR/." "$ROOTFS_CLEAN/usr/share/limine/"
log "limine embarqué : /usr/bin/limine + $(ls "$LIMINE_DIR" | wc -l) fichiers dans /usr/share/limine"

section "Création de l'image EROFS (zstd)"
rm -f "$EROFS_IMG"
# zstd niveau 19 : bon compromis taille / vitesse (ADR-011) ; -x -1 désactive
# les xattrs, inutiles ici et sources d'échecs sur les arbres de build.
mkfs.erofs -zzstd,level=19 -x -1 "$EROFS_IMG" "$ROOTFS_CLEAN"

section "Préparation de l'arborescence ISO"
ISOWORK="$BUILD_DIR/iso"
rm -rf "$ISOWORK"
mkdir -p "$ISOWORK"/{boot/limine,live,EFI/BOOT}

cp "$VMLINUZ" "$ISOWORK/boot/vmlinuz"
cp "$INITRD"   "$ISOWORK/boot/initrd"
cp "$EROFS_IMG" "$ISOWORK/live/rootfs.erofs"

cp "$LIMINE_DIR/limine-bios.sys"    "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/limine-bios-cd.bin" "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/limine-uefi-cd.bin" "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/BOOTX64.EFI"        "$ISOWORK/EFI/BOOT/"

# Limine v8+ : syntaxe « option: valeur », entrées introduites par « / ».
cat > "$ISOWORK/boot/limine/limine.conf" <<'LIM'
timeout: 3

/YAOR Live
    protocol: linux
    path: boot():/boot/vmlinuz
    module_path: boot():/boot/initrd
    cmdline: console=tty0 console=ttyS0 quiet

/YAOR Live (debug)
    protocol: linux
    path: boot():/boot/vmlinuz
    module_path: boot():/boot/initrd
    cmdline: console=tty0 console=ttyS0 systemd.log_level=debug loglevel=7
LIM

section "Génération de l'ISO (xorriso, hybride BIOS+UEFI)"
mkdir -p "$OUT_DIR"
rm -f "$ISO_OUT"
xorriso -as mkisofs -R -r -J \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -hfsplus -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    -A "YAOR Live" -V "YAOR-LIVE" \
    -o "$ISO_OUT" "$ISOWORK"

log "installation des étages Limine BIOS"
limine bios-install "$ISO_OUT"

section "Empreinte et signature (ADR-007)"
# L'empreinte est toujours produite : elle identifie l'image distribuée.
( cd "$OUT_DIR" && sha256sum "$ISO_NAME" > "$ISO_NAME.sha256" )
log "empreinte : $ISO_OUT.sha256"
# La signature détachée n'est produite que si une clé est fournie : les
# constructions de développement s'en passent, une image de release DOIT être
# signée. YAOR_GPG_KEY désigne la clé par son identifiant GPG (e-mail ou ID).
if [[ -n "${YAOR_GPG_KEY:-}" ]]; then
    need_cmd gpg
    gpg --batch --yes --armor --detach-sign --local-user "$YAOR_GPG_KEY" \
        --output "$ISO_OUT.asc" "$ISO_OUT"
    log "signature : $ISO_OUT.asc"
else
    log "signature ignorée : YAOR_GPG_KEY non défini (image de développement)"
fi

stamp_done "ch9.iso"
log "ISO générée : $ISO_OUT"
