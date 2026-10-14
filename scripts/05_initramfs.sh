#!/usr/bin/env bash
# 05_initramfs.sh — génère un initramfs minimal pour le système live YAOR.
#
# L'initramfs contient un /init (script POSIX) qui :
#   1. monte proc/sys/dev
#   2. localise le média live (LABEL=YAOR-LIVE)
#   3. monte rootfs.erofs (EROFS) en lecture seule
#   4. superpose un overlayfs inscriptible sur tmpfs
#   5. bascule (switch_root) vers systemd du système réel
#
# Les modules nécessaires (erofs, overlayfs, loop, iso9660, sr, virtio...)
# sont compilés en dur dans le noyau : aucun module externe n'est requis.

set -euo pipefail
STEP=05_initramfs
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

LFS="$(realpath "$BUILD_DIR/rootfs")"
KVER="$(manifest_field linux 3)"
LOADER="$LFS/usr/lib/ld-linux-x86-64.so.2"

need_cmd cpio zstd readelf

if ! stamp_exists "ch7.kernel"; then
    die "étape 04 incomplète : marqueur ch7.kernel manquant"
fi
[[ -x "$LFS/usr/lib/systemd/systemd" ]] || die "systemd introuvable dans $LFS"
[[ -f "$LFS/boot/vmlinuz-$KVER" ]] || die "noyau introuvable : /boot/vmlinuz-$KVER"
[[ -x "$LOADER" ]] || die "chargeur dynamique introuvable : $LOADER"

if stamp_exists "ch8.initramfs"; then
    log "phase déjà faite : initramfs (ch8.initramfs)"
    exit 0
fi

section "Génération de l'initramfs"

WORKDIR="$BUILD_DIR/initramfs-$KVER"
rm -rf "$WORKDIR"

# Arborescence de base
mkdir -p "$WORKDIR"/{usr/bin,usr/sbin,usr/lib,proc,sys,dev,run,tmp,mnt/{ro,rw,root}}
ln -sf usr/bin   "$WORKDIR/bin"
ln -sf usr/sbin  "$WORKDIR/sbin"
ln -sf usr/lib   "$WORKDIR/lib"
ln -sf usr/lib   "$WORKDIR/lib64"

# Ajoute un binaire + toutes ses dépendances partagées (résolues par le
# chargeur du système cible, pas celui de l'hôte).
add_bin() {
    local src="$1"
    [[ -f "$src" ]] || return 0
    install -Dm755 "$src" "$WORKDIR${src#"$LFS"}"
    "$LOADER" --library-path "$LFS/usr/lib" --list "$src" 2>/dev/null |
        awk -F'=>' '/=>/ { print $2 }' |
        awk '{ print $1 }' |
        while IFS= read -r lib; do
            case "$lib" in
                "$LFS"/*)
                    install -Dm755 "$lib" "$WORKDIR${lib#"$LFS"}"
                    ;;
            esac
        done
}

# Chargeur dynamique explicitement
install -Dm755 "$LOADER" "$WORKDIR/usr/lib/ld-linux-x86-64.so.2"

# Shell + outils essentiels
add_bin "$LFS/usr/bin/bash"
ln -sf bash "$WORKDIR/usr/bin/sh"
for b in mount umount mkdir ln cat ls sleep dmesg; do
    add_bin "$LFS/usr/bin/$b"
done
for b in blkid switch_root modprobe; do
    add_bin "$LFS/usr/sbin/$b"
done

# Script /init
cat > "$WORKDIR/init" << 'INIT'
#!/bin/sh
export PATH=/usr/bin:/usr/sbin:/bin:/sbin

mount -t proc    proc    /proc
mount -t sysfs   sysfs   /sys
mount -t devtmpfs devtmpfs /dev

DEV="$(blkid -L YAOR-LIVE 2>/dev/null)"
[ -n "$DEV" ] || DEV=/dev/sr0

mkdir -p /mnt/ro /mnt/rw /mnt/root

if ! mount -o ro "$DEV" /mnt/ro; then
    echo "YAOR: impossible de monter le média live ($DEV)"
    exec sh
fi

ROOTFS=""
for p in /mnt/ro/rootfs.erofs /mnt/ro/live/rootfs.erofs /mnt/ro/boot/rootfs.erofs; do
    [ -f "$p" ] && ROOTFS="$p" && break
done
if [ -z "$ROOTFS" ]; then
    echo "YAOR: rootfs.erofs introuvable sur le média live"
    exec sh
fi

if ! mount -o ro,loop "$ROOTFS" /mnt/ro; then
    echo "YAOR: montage de l'image EROFS échoué ($ROOTFS)"
    exec sh
fi

if ! mount -t tmpfs tmpfs /mnt/rw; then
    echo "YAOR: montage du tmpfs échoué"
    exec sh
fi
mkdir -p /mnt/rw/upper /mnt/rw/work

if ! mount -t overlay overlay \
        -o lowerdir=/mnt/ro,upperdir=/mnt/rw/upper,workdir=/mnt/rw/work /mnt/root; then
    echo "YAOR: montage overlayfs échoué"
    exec sh
fi

echo "YAOR: bascule vers le système live..."
exec switch_root /mnt/root /usr/lib/systemd/systemd
INIT
chmod +x "$WORKDIR/init"

section "Assemblage de l'initramfs (cpio + zstd)"
mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR/initrd.img-$KVER"
pushd "$WORKDIR" >/dev/null
find . -print0 | cpio --null -H newc -o 2>/dev/null |
    zstd -19 -T0 -o "$OUT_DIR/initrd.img-$KVER"
popd >/dev/null

if cp "$OUT_DIR/initrd.img-$KVER" "$LFS/boot/initrd.img-$KVER" 2>/dev/null; then
    chown root:root "$LFS/boot/initrd.img-$KVER" 2>/dev/null || true
    log "initramfs copié dans $LFS/boot/initrd.img-$KVER"
else
    log "initramfs dans $OUT_DIR/initrd.img-$KVER (copie dans /boot nécessite root)"
fi

stamp_done "ch8.initramfs"
log "initramfs créé : $OUT_DIR/initrd.img-$KVER"
