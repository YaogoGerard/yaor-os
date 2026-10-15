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
ISO_NAME="yaor-live-x86_64-${KVER}.iso"
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
# base n'en dépendent au démarrage.
rsync -a --delete \
  --exclude='/proc/*' --exclude='/sys/*' --exclude='/dev/*' \
  --exclude='/run/*'  --exclude='/tmp/*' \
  --exclude='/build/work/*' --exclude='/build/config/*' \
  --exclude='/sources/*' --exclude='/tools/*' \
  --exclude='/opt/*' \
  --exclude='/.cache/*' --exclude='*.log' \
  --exclude='/boot/*' \
  --exclude='/usr/libexec/gcc/*' \
  --exclude='/usr/lib/gcc/*' \
  --exclude='/usr/x86_64-lfs-linux-gnu/*' \
  --exclude='/usr/include/*' \
  --exclude='/usr/share/doc/*' --exclude='/usr/share/man/*' --exclude='/usr/share/info/*' \
  --exclude='/usr/share/gtk-doc/*' --exclude='/usr/share/locale/*' --exclude='/usr/share/i18n/*' \
  --exclude='/usr/share/aclocal/*' --exclude='/usr/share/gettext/*' \
  --exclude='/usr/share/vim/*' \
  --exclude='/usr/lib/meson/*' \
  --exclude='/usr/bin/meson*' --exclude='/usr/bin/ninja*' \
  --exclude='*.a' --exclude='*.la' \
  --exclude='/usr/bin/*gcc*' --exclude='/usr/bin/*g++*' \
  --exclude='/usr/bin/*cpp*' --exclude='/usr/bin/*c++*' \
  --exclude='/usr/bin/cc' --exclude='/usr/bin/c99' \
  --exclude='/usr/bin/x86_64-pc-linux-gnu-*' \
  --exclude='/usr/bin/x86_64-lfs-linux-gnu-*' \
  --exclude='/usr/bin/ld' --exclude='/usr/bin/ld.bfd' --exclude='/usr/bin/ld.gold' \
  --exclude='/usr/bin/as' --exclude='/usr/bin/ar' --exclude='/usr/bin/ranlib' \
  --exclude='/usr/bin/nm' --exclude='/usr/bin/objcopy' --exclude='/usr/bin/objdump' \
  --exclude='/usr/bin/readelf' --exclude='/usr/bin/strip' --exclude='/usr/bin/size' \
  --exclude='/usr/bin/strings' --exclude='/usr/bin/addr2line' --exclude='/usr/bin/elfedit' \
  --exclude='/usr/bin/gprof*' --exclude='/usr/bin/c++filt' \
  --exclude='/usr/lib/bfd-plugins/*' \
  --exclude='/usr/bin/bison' --exclude='/usr/bin/yacc' --exclude='/usr/share/bison/*' \
  --exclude='/usr/bin/flex' --exclude='/usr/bin/lex' \
  --exclude='/usr/bin/python3*' --exclude='/usr/lib/python3.13/*' \
  --exclude='/usr/bin/perl*' --exclude='/usr/lib/perl5/*' \
  --exclude='/usr/bin/file' --exclude='/usr/lib/libmagic*' \
  --exclude='/usr/share/misc/magic.mgc' \
  "$LFS/" "$ROOTFS_CLEAN/"

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
