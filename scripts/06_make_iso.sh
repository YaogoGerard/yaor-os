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
# Aucun binaire GNU ne doit subsister : contrôle par marqueurs discriminants
# (check_uutils_replacement, common.sh — un grep de « GNU coreutils » est
# faux, les aides uutils le citent comme référence de compatibilité).
if ! check_uutils_replacement "$ROOTFS_CLEAN"; then
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
cp -a --no-preserve=ownership "$LIMINE_DIR/." "$ROOTFS_CLEAN/usr/share/limine/"
log "limine embarqué : /usr/bin/limine + $(ls "$LIMINE_DIR" | wc -l) fichiers dans /usr/share/limine"

section "Création de l'image EROFS (zstd)"
rm -f "$EROFS_IMG"
# zstd niveau 19 : bon compromis taille / vitesse (ADR-011) ; -x -1 désactive
# les xattrs, inutiles ici et sources d'échecs sur les arbres de build.
mkfs.erofs -zzstd,level=19 -x -1 "$EROFS_IMG" "$ROOTFS_CLEAN"

# Limine v8+ : syntaxe « option: valeur », entrées introduites par « / ».
section "Génération des ISO (xorriso, hybride BIOS+UEFI)"
ISOWORK="$BUILD_DIR/iso"
rm -rf "$ISOWORK"
mkdir -p "$ISOWORK"/{boot/limine,live,EFI/BOOT}

cp "$VMLINUZ" "$ISOWORK/boot/vmlinuz"
cp "$INITRD"   "$ISOWORK/boot/initrd"

cp "$LIMINE_DIR/limine-bios.sys"    "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/limine-bios-cd.bin" "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/limine-uefi-cd.bin" "$ISOWORK/boot/limine/"
cp "$LIMINE_DIR/BOOTX64.EFI"        "$ISOWORK/EFI/BOOT/"

build_iso() {
    # $1 = nom ISO, $2 = libellé volume, $3 = chemin EROFS racine.
    # L'arborescence ISOWORK est réutilisée : seul le rootfs variant
    # change (le bureau embarque la porte yaor-session ON).
    local iso_name="$1" vol_label="$2" erofs_path="$3"
    local iso_out="$OUT_DIR/$iso_name"
    mkdir -p "$OUT_DIR"
    rm -f "$iso_out"
    cp -f "$erofs_path" "$ISOWORK/live/rootfs.erofs"
    xorriso -as mkisofs -R -r -J \
        -b boot/limine/limine-bios-cd.bin \
        -no-emul-boot -boot-load-size 4 -boot-info-table \
        -hfsplus -apm-block-size 2048 \
        --efi-boot boot/limine/limine-uefi-cd.bin \
        -efi-boot-part --efi-boot-image --protective-msdos-label \
        -A "$vol_label" -V "$vol_label" \
        -o "$iso_out" "$ISOWORK"
    log "installation des étages Limine BIOS"
    limine bios-install "$iso_out"
    ( cd "$OUT_DIR" && sha256sum "$iso_name" > "$iso_name.sha256" )
    log "empreinte : $iso_out.sha256"
    if [[ -n "${YAOR_GPG_KEY:-}" ]]; then
        need_cmd gpg
        gpg --batch --yes --armor --detach-sign --local-user "$YAOR_GPG_KEY" \
            --output "$iso_out.asc" "$iso_out"
        log "signature : $iso_out.asc"
    else
        log "signature ignorée : YAOR_GPG_KEY non défini (image de développement)"
    fi
    log "ISO générée : $iso_out"
}

# Variante desktop (post-v0.6) : la porte /etc/yaor/desktop-autostart
# met le boot direct sur le bureau (l'unité yaor-session s'exécute au
# multi-user). Deux racines, deux images : la volée standard reste
# inchangée pour les bancs QA (instance unique du compositor).
build_erofs() {
    # $1 = chemin de l'image EROFS à produire (propriété root exigée,
    # comme l'étape 04).
    mkfs.erofs -zzstd,level=19 -x -1 "$1" "$ROOTFS_CLEAN"
}

# Configuration du chargeur Limine (ADR-011), partagée par les deux
# ISO : la conf pointe sur boot/vmlinuz + initrd déjà copiés.
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

section "Génération de l'ISO standard (console, bancs QA)"
# L'EROFS standard vient d'être produite (au-dessus, section « Création
# de l'image EROFS ») ; seule la variante desktop en refait une.
build_iso "$ISO_NAME" "YAOR-LIVE" "$EROFS_IMG"

DESKTOP_NAME="yaor-desktop-x86_64-$(yaor_version).iso"
section "Génération de l'ISO desktop (boot direct, bureau)"
mkdir -p "$ROOTFS_CLEAN/etc/yaor"
touch "$ROOTFS_CLEAN/etc/yaor/desktop-autostart"
DESKTOP_EROFS="$OUT_DIR/rootfs-desktop.erofs"
rm -f "$DESKTOP_EROFS"
build_erofs "$DESKTOP_EROFS"
build_iso "$DESKTOP_NAME" "YAOR-DESKTOP" "$DESKTOP_EROFS"
rm -f "$DESKTOP_EROFS"

stamp_done "ch9.iso"
