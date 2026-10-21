#!/usr/bin/env bash
# 04_chroot_build.sh — entre dans le chroot et bâtit le système de base.
#
# C'est la seule étape du pipeline qui exige root : elle crée l'arborescence
# FHS de la cible, y monte /dev, /proc, /sys et /run, puis exécute toutes les
# compilations DANS le chroot, en compilation native (les sources sont
# accessibles via des montages --bind, voir ci-dessous).
#
# Le script reste piloté depuis l'hôte : la logique des phases, les marqueurs
# d'idempotence et les journaux restent sur l'hôte ; seules les commandes de
# build (configure/make/meson) sont déléguées au chroot via chroot_run().
#
# Périmètre (jalon 0.1, « minimal bootable ») :
#   - bison + zlib + python3 construits AVANT glibc : glibc 2.44 exige bison
#     (intl/plural.c) et python (locale/C-translit.h) au configure comme au
#     build, tous deux absents de la toolchain temporaire de l'étape 03 ;
#     zlib précède python3 pour que celui-ci construise son module _zlib,
#     exigé par meson (import gzip) ;
#   - toolchain finale autoporteuse : glibc (final) puis gcc (final) ;
#   - outils de build exigés par systemd : perl, python3, ninja, meson, pkgconf ;
#   - bibliothèques requises : zlib, libcap, libxcrypt, util-linux, kmod, pcre2 ;
#   - libelf (Elfutils) pour objtool, flex pour le kconfig du noyau et zstd
#     pour la compression de l'image du noyau (CONFIG_KERNEL_ZSTD) ;
#   - systemd (PID 1) puis le noyau Linux ;
#   - composants Rust (uutils coreutils et findutils) compilés EN DERNIER, dans
#     le chroot, avec la toolchain Rust de l'hôte montée en lecture seule
#     (ADR-019) : elle n'est jamais embarquée dans l'image.
#
# Périmètre v0.2 (« userland fonctionnel », phase 1 — ADR-013/014/015) :
#   - sessions : expat et linux-pam, D-Bus (expat + libsystemd), shadow (comptes)
#     et sudo-rs (élévation) ; util-linux fournit login/agetty et systemd est
#     bâti avec PAM et logind (plus de session root « nue ») ;
#   - mode installé : openssl et curl (bâtis avant systemd) puis systemd avec
#     « importd », « sysupdate » et « repart » activés, ce qui fournit
#     systemd-repart et systemd-sysupdate pour le schéma A/B (ADR-015) ;
#   - la phase « configure-system » (en fin de liste) copie l'arborescence
#     config/rootfs et active les services, une fois tous les paquets installés.
#
# Usage :
#   sudo ./scripts/04_chroot_build.sh                 # enchaîne toutes les phases
#   sudo ./scripts/04_chroot_build.sh setup systemd   # relance des phases choisies
#   sudo ./scripts/04_chroot_build.sh teardown        # démonte la cible
#   JOBS=6 sudo ./scripts/04_chroot_build.sh
#
# Les montages restent en place entre deux exécutions (et après un reboot, il
# suffit de relancer « setup » pour les rétablir). Chaque phase réussie est
# marquée dans build/stamps/ch7.* : relancer le script ne refait que ce qui
# manque.

set -euo pipefail
STEP=04_chroot_build
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

# --- Env --------------------------------------------------------------------
LFS="$BUILD_DIR/rootfs"          # racine cible (vue comme « / » dans le chroot)
WORK="${WORK:-$BUILD_DIR/work}"  # sources extraites (hôte) → /build/work (chroot)
JOBS="${JOBS:-4}"
LFS_TGT="${LFS_TGT:-$(uname -m)-lfs-linux-gnu}"
BUILD_LOG="$LOG_DIR/${STEP}.build.log"

export LFS WORK JOBS LFS_TGT LC_ALL=POSIX

[[ "$(id -u)" -eq 0 ]] || die "cette étape exige root (montages + chroot) ; relance avec sudo"
need_cmd chroot mountpoint mount umount

# Ordre d'exécution. Les noms contenant un tiret deviennent des fonctions
# phase_* à tirets convertis en soulignés (util-linux → phase_util_linux).
ALL_PHASES=(
    setup
    # glibc 2.44 exige bison et python3 (voir l'en-tête). zlib précède python3 :
    # meson (import gzip) exige le module _zlib du python d'amorçage.
    bison zlib python
    glibc
    gcc
    perl
    ninja meson pkgconf
    libcap libxcrypt
    # Mode installé (ADR-015) : openssl puis curl sont bâtis AVANT systemd, qui
    # les exige pour ses fonctions « sysupdate » et « repart ». systemd-importd
    # (dont dépend systemd-sysupdate) réclame libcurl, openssl, zlib et liblzma ;
    # systemd-sysupdate réclame en outre libfdisk. openssl n'exige que perl (déjà
    # bâti) et zlib ; curl n'exige qu'openssl et zlib.
    openssl
    curl
    # Sessions (ADR-014) : expat (analyse XML de D-Bus) et linux-pam doivent
    # précéder util-linux (dont « login » s'appuie sur PAM) et systemd, bâti
    # -Dpam=enabled (la fonctionnalité « pam » est coupée par
    # -Dauto_features=disabled).
    expat linux-pam
    util-linux
    kmod pcre2
    bc gperf
    # systemd 262 exige jinja2 (rendu des .pc et de divers en-têtes, dont
    # libudev.pc) ; markupsafe est sa dépendance d'import.
    markupsafe jinja2
    systemd
    # Suite des sessions (ADR-014) : D-Bus (sur expat, lié à libsystemd),
    # shadow (comptes et /etc/shadow) et sudo-rs (élévation, lié à libpam).
    dbus shadow sudo-rs
    # Réseau (ADR-013, ADR-020) : iwd (Wi-Fi, couche 2 seule) s'appuie sur
    # libreadline pour « iwctl » et détecte D-Bus et systemd via leurs fichiers
    # pkg-config (dbus-1, systemd) — donc après la phase systemd. linux-firmware
    # fournit les microprogrammes des pilotes activés dans le noyau.
    readline iwd linux-firmware
    # Lot stockage (1/3) : socle GLib et moteurs requis par polkit, libblockdev
    # et udisks. gettext fournit les outils (msgfmt) qu'ils exigent.
    gettext libffi glib libgudev duktape
    libelf
    # Le kconfig du noyau régénère lexer.lex.c (et parser.tab.c) depuis
    # lexer.l/parser.y : flex est donc requis. L'archive flex fournit ses
    # sources déjà analysées (src/scan.c, src/parse.c), il s'amorce seul.
    flex
    # CONFIG_KERNEL_ZSTD fait compresser l'image du noyau par l'outil « zstd » :
    # il doit être présent dans le chroot au moment de la phase kernel.
    zstd
    # Lot stockage (2/3) : outils de systèmes de fichiers (btrfs-progs exige
    # libzstd, donc après zstd).
    e2fsprogs btrfs-progs dosfstools exfatprogs
    # Lot stockage (3/3) : autorisation des actions privilégiées (polkit), puis
    # les bibliothèques de bloc et le démon de montage.
    polkit
    # Lot stockage (3/3, suite) : bibliothèques de bloc et dépendances (openssl
    # et curl ont été bâtis plus tôt, avant systemd).
    json-c popt keyutils
    lvm2
    libcryptsetup
    libnvme
    libatasmart
    # Lot stockage (3/3, fin) : socle de stockage d'udisks2, puis le démon de
    # montage automatique lui-même (ADR-015).
    libbytesize
    libblockdev
    udisks
    kernel
    # Phase de configuration système : copie config/rootfs et active les
    # services. Elle précède les composants Rust pour s'appuyer sur les outils
    # de base encore en place.
    configure-system
    # Composants Rust, en dernier : ils écrasent leurs homologues GNU dans
    # /usr/bin, une fois que plus aucune phase C n'en dépend (ADR-017). La
    # toolchain de l'hôte sert seule à les compiler (ADR-019).
    rust-findutils
    rust-coreutils
    # Composants internes YAOR (yaor-install, yaor-slot), en tout dernier :
    # sources du dépôt (components/), bâties par la toolchain de l'hôte
    # (ADR-002, ADR-019), sans dépendance externe (ADR-018).
    yaor-components
)

# --- Outillage --------------------------------------------------------------
run() { "$@" 2>&1 | tee -a "$BUILD_LOG"; }

# Environnement minimal et reproductible à l'intérieur du chroot.
chroot_env=(
    HOME=/root
    TERM=xterm-256color
    PATH=/usr/bin:/usr/sbin
    MAKEFLAGS="-j$JOBS"
    LC_ALL=POSIX
)

# chroot_run <snippet shell> : exécute un bout de shell dans le chroot, en
# environnement propre, et abandonne à la première erreur.
chroot_run() {
    chroot "$LFS" /usr/bin/env -i "${chroot_env[@]}" /usr/bin/bash -e -c "$1"
}

# --- Toolchain Rust de l'hôte (ADR-019) -------------------------------------
# Point de montage, dans le chroot, du « sysroot » de rustc.
RUST_BIND="$LFS/opt/host-rust/toolchain"

# host_rust_dir : dossier de la toolchain Rust de l'hôte, c'est-à-dire le
# « sysroot » de rustc. La toolchain est un OUTIL DE BUILD (ADR-019) : on
# cherche celle de l'utilisateur qui a lancé sudo (rustup ou path hérité). Une
# installation de distribution (sysroot = /usr) est refusée : son bin/ contient
# des outils de l'hôte (cc…) qui masqueraient ceux du chroot.
host_rust_dir() {
    local user home cand root rr=""
    user="${SUDO_USER:-$(id -un)}"
    home="$(getent passwd "$user" 2>/dev/null | cut -d: -f6)"
    [[ -n "$home" ]] || home="${HOME:-/root}"

    if command -v rustc >/dev/null 2>&1; then
        rr="$(rustc --print sysroot 2>/dev/null || true)"
    elif [[ -x "$home/.cargo/bin/rustc" ]]; then
        rr="$("$home/.cargo/bin/rustc" --print sysroot 2>/dev/null || true)"
    else
        for root in "${RUSTUP_HOME:-$home/.rustup}" "$home/.rustup" /root/.rustup; do
            for cand in "$root"/toolchains/*; do
                if [[ -x "$cand/bin/rustc" ]]; then rr="$cand"; break 2; fi
            done
        done
    fi

    [[ -n "$rr" && -x "$rr/bin/rustc" ]] || return 1
    rr="$(readlink -f "$rr")"
    case "$rr" in
        /|/usr|/usr/local) return 1 ;;   # installation de distribution
    esac
    printf '%s\n' "$rr"
}

# ensure_host_rust : monte (une seule fois) la toolchain de l'hôte, en lecture
# seule, dans le chroot. Idempotent : peut être rappelé par une phase Rust même
# si « setup » a déjà été exécuté.
ensure_host_rust() {
    local src
    src="$(host_rust_dir)" \
        || die "toolchain Rust introuvable : installe rustc/cargo via rustup (ADR-019)"
    mkdir -pv "$RUST_BIND"
    if ! mountpoint -q "$RUST_BIND"; then
        mount -v --bind "$src" "$RUST_BIND"
        mount -o remount,ro,bind "$RUST_BIND" 2>/dev/null \
            || log "remontage lecture seule impossible : $RUST_BIND"
    fi
}

# chroot_rust <snippet shell> : exécute du Rust dans le chroot avec la toolchain
# de l'hôte et un CARGO_HOME dédié, en mode hors-ligne (ADR-018/019). Le
# CARGO_HOME et les « target/ » vivent sous /build/work : jamais dans le dépôt.
chroot_rust() {
    chroot "$LFS" /usr/bin/env -i \
        HOME=/root \
        TERM=xterm-256color \
        PATH="/opt/host-rust/toolchain/bin:/usr/bin:/usr/sbin" \
        LC_ALL=POSIX \
        CARGO_HOME=/build/work/.cargo-home \
        CARGO_NET_OFFLINE=true \
        CARGO_BUILD_JOBS="$JOBS" \
        /usr/bin/bash -e -c "$1"
}

require_archive() {
    local a
    a="$(source_archive "$1")"
    [[ -f "$a" ]] || die "archive manquante : $a (lancer scripts/01_download.sh)"
}

# ci_work <nom> : chemin du dossier source tel que vu DANS le chroot.
ci_work() {
    local d
    d="$(src_dir "$1")"
    printf '/build/work/%s\n' "$(basename "$d")"
}

# install_pure_python <nom> <sous-dossier> : installe un paquet Python « pur »
# (sans partie compilée) en copiant son paquet depuis l'arbre extrait vers le
# site-packages de la cible. On évite ainsi d'introduire pip et un backend
# PEP 517 dans le système minimal : ces paquets ne servent que d'outils de build.
install_pure_python() {
    local name="$1" pkgdir="$2" d sp
    require_archive "$name"
    d="$(ci_work "$name")"
    sp="$(chroot_run "python3 -c 'import sysconfig; print(sysconfig.get_paths()[\"purelib\"])'")"
    run chroot_run "install -d '$sp' && cp -a '$d/$pkgdir' '$sp/'"
}

mount_if() {
    local mp="$1"; shift
    if mountpoint -q "$mp"; then
        log "déjà monté : $mp"
    else
        mount -v "$@" "$mp"
    fi
}

# --- Phase : préparation du chroot -----------------------------------------
phase_setup() {
    section "Arborescence FHS de la cible"
    mkdir -pv "$LFS"/{etc,usr/{bin,lib,sbin,include},var/{log,tmp,lib,spool},run/lock,srv,opt,root,home,mnt,proc,sys,dev/{pts,shm},tmp,boot,sources,build/{work,config,components}}

    local l name tgt
    for l in bin:usr/bin sbin:usr/sbin lib:usr/lib lib64:usr/lib; do
        name="${l%%:*}"; tgt="${l#*:}"
        if [[ ! -e "$LFS/$name" && ! -L "$LFS/$name" ]]; then
            ln -sv "$tgt" "$LFS/$name"
        fi
    done
    # /var/run -> /run et /var/lock -> /run/lock (chemins transitoires).
    [[ -L "$LFS/var/run"  ]] || { rm -rf "$LFS/var/run";  ln -sv /run       "$LFS/var/run";  }
    [[ -L "$LFS/var/lock" ]] || { rm -rf "$LFS/var/lock"; ln -sv /run/lock  "$LFS/var/lock"; }
    # /etc/mtab -> /proc/self/mounts (mtab n'est plus un fichier réel).
    [[ -e "$LFS/etc/mtab" || -L "$LFS/etc/mtab" ]] || ln -sv /proc/self/mounts "$LFS/etc/mtab"
    # Compilateur C/C++ générique exigé par de nombreux configure/meson.
    ln -sf gcc "$LFS/usr/bin/cc"
    [[ -e "$LFS/usr/bin/g++" ]] && ln -sf g++ "$LFS/usr/bin/c++" || true

    section "Fichiers /etc minimaux"
    cat > "$LFS/etc/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/bash
bin:x:1:1:bin:/dev/null:/usr/bin/false
daemon:x:6:6:daemon:/dev/null:/usr/bin/false
messagebus:x:18:18:D-Bus Message Daemon User:/run/dbus:/usr/bin/false
systemd-journal-gateway:x:73:73:systemd Journal Gateway:/:/usr/bin/false
systemd-journal-remote:x:74:74:systemd Journal Remote:/:/usr/bin/false
systemd-journal-upload:x:75:75:systemd Journal Upload:/:/usr/bin/false
systemd-timesync:x:76:76:systemd Time Synchronization:/:/usr/bin/false
systemd-network:x:77:77:systemd Network Management:/:/usr/bin/false
systemd-resolve:x:78:78:systemd Resolver:/:/usr/bin/false
systemd-coredump:x:79:79:systemd Core Dumper:/:/usr/bin/false
systemd-oom:x:80:80:systemd Userspace OOM Killer:/:/usr/bin/false
nobody:x:65534:65534:Unprivileged User:/dev/null:/usr/bin/false
EOF
    cat > "$LFS/etc/group" <<'EOF'
root:x:0:
bin:x:1:
sys:x:2:
kmem:x:3:
tape:x:4:
tty:x:5:
daemon:x:6:
floppy:x:7:
disk:x:8:
lp:x:9:
dialout:x:10:
audio:x:11:
video:x:12:
utmp:x:13:
usb:x:14:
cdrom:x:15:
adm:x:16:
messagebus:x:18:
systemd-journal:x:23:
input:x:24:
mail:x:34:
kvm:x:61:
systemd-journal-gateway:x:73:
systemd-journal-remote:x:74:
systemd-journal-upload:x:75:
systemd-timesync:x:76:
systemd-network:x:77:
systemd-resolve:x:78:
systemd-coredump:x:79:
systemd-oom:x:80:
render:x:105:
users:x:999:
nobody:x:65534:
EOF
    cat > "$LFS/etc/hosts" <<'EOF'
127.0.0.1  localhost
::1        localhost
EOF
    cat > "$LFS/etc/ld.so.conf" <<'EOF'
/usr/local/lib
/opt/lib
/usr/lib
EOF
    cat > "$LFS/etc/nsswitch.conf" <<'EOF'
passwd: files
group: files
shadow: files
hosts: files dns
networks: files
protocols: files
services: files
ethers: files
rpc: files
EOF
    cat > "$LFS/etc/inputrc" <<'EOF'
set bell-style none
set meta-flag on
set input-meta on
set convert-meta off
set output-meta on
"\e[1~": beginning-of-line
"\e[4~": end-of-line
"\e[5~": beginning-of-history
"\e[6~": end-of-history
"\e[3~": delete-char
"\e[2~": quoted-insert
EOF
    # Identité du produit (systemd, /etc/os-release). Volontairement minimale :
    # les distributions et outils tiers s'appuient sur ID/ID_LIKE/VERSION_ID.
    # IMAGE_ID/IMAGE_VERSION (ADR-022) identifient l'image pour systemd :
    # « %A » dans les transferts sysupdate se développe en IMAGE_VERSION, ce qui
    # protège la version en cours (ProtectVersion=) et apparie les slots A/B.
    local prodver; prodver="$(yaor_version)"
    cat > "$LFS/etc/os-release" <<EOF
NAME="YAOR-OS"
PRETTY_NAME="YAOR-OS $prodver"
ID=yaor
ID_LIKE="linux"
VERSION_ID="$prodver"
VERSION="$prodver"
IMAGE_ID=$YAOR_IMAGE_ID
IMAGE_VERSION="$prodver"
HOME_URL="https://example.invalid/yaor-os"
ANSI_COLOR="0;36"
EOF
    [[ -f "$LFS/etc/resolv.conf" ]] || cp -v /etc/resolv.conf "$LFS/etc/resolv.conf"

    section "Montages du chroot"
    mkdir -pv "$LFS/dev/pts" "$LFS/dev/shm"
    mount_if "$LFS/dev"           --bind /dev
    mount_if "$LFS/dev/pts"       --bind /dev/pts
    mount_if "$LFS/proc"          -t proc  proc
    mount_if "$LFS/sys"           -t sysfs sysfs
    mount_if "$LFS/run"           -t tmpfs tmpfs
    mount_if "$LFS/sources"       --bind "$SOURCES_DIR"
    mount_if "$LFS/build/work"    --bind "$WORK"
    mount_if "$LFS/build/config"  --bind "$CONFIG_DIR"
    # Composants internes (Rust) : sources du dépôt, montées pour compilation.
    mount_if "$LFS/build/components" --bind "$COMPONENTS_DIR"
    # Toolchain Rust de l'hôte, montée en lecture seule (ADR-019).
    ensure_host_rust
    mkdir -pv "$LFS/run/lock"

    section "Vérification du chroot"
    run chroot_run "echo 'chroot opérationnel' && uname -m && gcc --version | head -1"
}

phase_teardown() {
    section "Démontage de la cible"
    local mp
    for mp in "$LFS/run" "$LFS/dev/pts" "$LFS/dev" "$LFS/proc" "$LFS/sys" \
              "$LFS/sources" "$LFS/build/work" "$LFS/build/config" \
              "$LFS/build/components" \
              "$RUST_BIND"; do
        if mountpoint -q "$mp"; then
            umount -v "$mp" || log "échec du démontage de $mp"
        fi
    done
}

# --- Phase : bison (exigé par glibc 2.44) ----------------------------------
# glibc ne livre plus intl/plural.c : bison le régénère depuis intl/plural.y.
# La construction est native (elle s'appuie sur la glibc temporaire de l'étape
# 02) et n'a pas besoin d'un bison préexistant : l'archive fournit les sources
# déjà analysées (src/parse-gram.c, src/scan-gram.c). m4, requis à l'exécution
# par bison, provient de la toolchain temporaire.
phase_bison() {
    require_archive bison
    local d; d="$(ci_work bison)"
    log "configure bison (exigé par glibc 2.44)"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --disable-nls"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : toolchain finale ----------------------------------------------
phase_glibc() {
    require_archive glibc
    local d; d="$(ci_work glibc)"
    local b="$d/build7"
    log "configure glibc (final, natif)"
    run chroot_run "rm -rf '$b' && mkdir -p '$b' && cd '$b' && ../configure --prefix=/usr --disable-werror --disable-nscd --enable-stack-protector=strong --enable-kernel=6.1 libc_cv_slibdir=/usr/lib"
    log "compilation glibc (final)"
    run chroot_run "cd '$b' && make"
    log "installation glibc (final)"
    run chroot_run "cd '$b' && make install"
}

phase_zlib() {
    require_archive zlib
    local d; d="$(ci_work zlib)"
    log "configure zlib"
    run chroot_run "cd '$d' && ./configure --prefix=/usr"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
    run chroot_run "rm -fv /usr/lib/libz.a"
}

phase_gcc() {
    require_archive gcc
    local d; d="$(ci_work gcc)"
    local b="$d/build7"
    log "configure gcc (final, C/C++, --with-system-zlib)"
    run chroot_run "mkdir -p '$b' && cd '$b' && ../configure --prefix=/usr LD=ld --enable-languages=c,c++ --enable-default-pie --enable-default-ssp --enable-host-pie --disable-multilib --disable-bootstrap --disable-fixincludes --with-system-zlib"
    log "compilation gcc (final)"
    run chroot_run "cd '$b' && make"
    log "installation gcc (final)"
    run chroot_run "cd '$b' && make install"
    run chroot_run "ln -sf gcc /usr/bin/cc && ln -sf g++ /usr/bin/c++"
}

# --- Phase : outils de build -----------------------------------------------
phase_perl() {
    require_archive perl
    local d; d="$(ci_work perl)"
    log "configure perl (exigé par libxcrypt)"
    run chroot_run "cd '$d' && ./configure.gnu -des -Dprefix=/usr -Dvendorprefix=/usr -Duseshrplib"
    log "compilation perl (sérielle)"
    run chroot_run "cd '$d' && make -j1"
    log "installation perl"
    run chroot_run "cd '$d' && make install"
}

phase_python() {
    require_archive python
    local d; d="$(ci_work python)"
    log "configure python3"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --without-ensurepip"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

phase_ninja() {
    require_archive ninja
    local d; d="$(ci_work ninja)"
    log "amorçage de ninja (configure.py --bootstrap)"
    run chroot_run "cd '$d' && python3 configure.py --bootstrap"
    run chroot_run "install -vDm755 '$d/ninja' /usr/bin/ninja"
}

phase_meson() {
    require_archive meson
    local d; d="$(ci_work meson)"
    log "installation de meson (depuis l'arbre source, sans pip)"
    run chroot_run "rm -rf /usr/lib/meson && mkdir -p /usr/lib/meson && cp -a '$d'/. /usr/lib/meson/"
    # Un petit lanceur garantit que Python trouve le paquet mesonbuild voisin.
    cat > "$LFS/usr/bin/meson" <<'EOF'
#!/bin/sh
exec python3 /usr/lib/meson/meson.py "$@"
EOF
    chmod 755 "$LFS/usr/bin/meson"
}

phase_pkgconf() {
    require_archive pkgconf
    local d h; d="$(ci_work pkgconf)"; h="$(src_dir pkgconf)"
    local b="$d/build7"
    # Le tarball 3.0.8 omet tests/api/test-util.c, pourtant référencé par son
    # meson.build ; on recrée le fichier amont (voir patches/).
    if [[ ! -f "$h/tests/api/test-util.c" ]]; then
        patch -p1 -d "$h" -i "$ROOT/patches/pkgconf-3.0.8-missing-test-util.patch"
    fi
    log "configure pkgconf (meson)"
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release"
    run chroot_run "cd '$d' && ninja -C build7 && ninja -C build7 install"
    run chroot_run "ln -sf pkgconf /usr/bin/pkg-config"
}

# --- Phase : bibliothèques --------------------------------------------------
phase_libcap() {
    require_archive libcap
    local d; d="$(ci_work libcap)"
    log "compilation libcap"
    run chroot_run "cd '$d' && make prefix=/usr lib=lib"
    run chroot_run "cd '$d' && make prefix=/usr lib=lib install"
    run chroot_run "rm -fv /usr/lib/libcap.a"
}

phase_libxcrypt() {
    require_archive libxcrypt
    local d h; d="$(ci_work libxcrypt)"; h="$(src_dir libxcrypt)"
    # GCC 16 (C23) + glibc 2.44 : strchr renvoie const char * pour un argument
    # const ; le cast d'origine déclenche -Wdiscarded-qualifiers (-Werror).
    if grep -q 'strchr ((const char \*) intbuf->retval' "$h/lib/crypt-gost-yescrypt.c" 2>/dev/null; then
        patch -p1 -d "$h" -i "$ROOT/patches/libxcrypt-4.5.2-c23-strchr-const.patch"
    fi
    log "configure libxcrypt"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --enable-hashes=strong,glibc --enable-obsolete-api=no --disable-static --disable-failure-tokens"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : expat (analyse XML, requise par D-Bus) -------------------------
phase_expat() {
    require_archive expat
    local d; d="$(ci_work expat)"
    log "configure expat"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --disable-static --without-xmlwf --without-examples --without-tests --without-docbook"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : linux-pam (authentification, ADR-014) --------------------------
# À précéder util-linux (dont « login » s'appuie sur PAM) et systemd, bâti
# -Dpam=enabled. Les fonctionnalités optionnelles sont coupées ; les modules
# s'installent dans /usr/lib/security, comme pam_systemd.so (fourni par
# systemd) : un seul répertoire de modules pour PAM.
phase_linux_pam() {
    require_archive linux-pam
    local d; d="$(ci_work linux-pam)"
    local b="$d/build7"
    log "configure linux-pam (meson)"
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release -Ddocs=disabled -Di18n=disabled -Daudit=disabled -Dselinux=disabled -Deconf=auto -Delogind=disabled -Dlogind=disabled -Dopenssl=disabled -Dpwaccess=disabled -Dpam_userdb=disabled -Dexamples=false"
    run chroot_run "cd '$d' && ninja -C build7 && ninja -C build7 install"
}

phase_util_linux() {
    require_archive util-linux
    local d; d="$(ci_work util-linux)"
    # NB : --disable-wall — « wall » s'installe sgid tty, et dans un build
    # sans privilèges réels (espace d'utilisateurs restreint, ADR-001) chgrp
    # vers un gid non mappé échoue (EINVAL) ; wall n'est pas requis (logind /
    # agetty suffisent). su/nologin/runuser/setpriv restent désactivés,
    # l'élévation passant par sudo-rs (un besoin, un fournisseur — ADR-014).
    log "configure util-linux"
    run chroot_run "cd '$d' && ./configure --bindir=/usr/bin --sbindir=/usr/sbin --libdir=/usr/lib --runstatedir=/run --disable-chfn-chsh --disable-nologin --disable-su --disable-setpriv --disable-runuser --disable-pylibmount --disable-liblastlog2 --disable-wall --disable-static --without-python ADJTIME_PATH=/var/run/adjtime"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

phase_kmod() {
    require_archive kmod
    local d; d="$(ci_work kmod)"
    local b="$d/build7"
    # Options « feature » de kmod 34.x : le lot minimal n'a ni libzstd ni
    # liblzma ni OpenSSL, et scdoc (manpages) est absent.
    log "configure kmod (meson)"
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release -Dzstd=disabled -Dxz=disabled -Dzlib=enabled -Dopenssl=disabled -Dmanpages=false"
    run chroot_run "cd '$d' && ninja -C build7 && ninja -C build7 install"
}

phase_pcre2() {
    require_archive pcre2
    local d; d="$(ci_work pcre2)"
    log "configure pcre2"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static --enable-unicode"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : bc (requis par la compilation du noyau) ------------------------
phase_bc() {
    require_archive bc
    local d; d="$(ci_work bc)"
    log "configure bc"
    run chroot_run "cd '$d' && ./configure --prefix=/usr"
    # texinfo (makeinfo) n'est pas dans le lot minimal : MAKEINFO=true neutralise
    # la génération des pages info, sans effet sur les binaires bc/dc.
    run chroot_run "cd '$d' && make MAKEINFO=true"
    run chroot_run "cd '$d' && make install MAKEINFO=true"
}

# --- Phase : systemd --------------------------------------------------------
# MarkupSafe puis Jinja2 : requis sans condition par systemd 262, qui rend ses
# gabarits (.pc, dont libudev.pc, et divers en-têtes) via
# find_installation('python3', modules : ['jinja2']). Paquets Python purs :
# installés par copie, sans pip ni backend PEP 517 ; l'extension C optionnelle
# de MarkupSafe (_speedups) n'est pas compilée, son repli _native suffit.
phase_markupsafe() {
    log "installation de MarkupSafe (paquet Python pur)"
    install_pure_python markupsafe src/markupsafe
}

phase_jinja2() {
    log "installation de Jinja2 (paquet Python pur)"
    install_pure_python jinja2 src/jinja2
}

phase_gperf() {
    require_archive gperf
    local d; d="$(ci_work gperf)"
    # Outil de génération requis par systemd 262 (GPERF_LEN_TYPE et tables
    # .gperf de udev/networkd/…) ; sa documentation est livrée pré-générée.
    log "configure gperf"
    run chroot_run "cd '$d' && ./configure --prefix=/usr"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

phase_systemd() {
    require_archive systemd
    local d; d="$(ci_work systemd)"
    local b="$d/build7"
    log "configure systemd (meson, fonctionnalités optionnelles désactivées)"
    # meson n'est pas idempotent sur un dossier déjà configuré : on le recrée.
    # translations=false : le rendu des traductions réclame msgfmt (gettext),
    # absent du lot minimal ; le garde-fou est le même que pour gperf/jinja2.
    # -Dauto_features=disabled coupe toutes les *fonctionnalités* optionnelles
    # (les options booléennes, elles, gardent leur défaut : activé). libmount en
    # fait partie et doit être réactivé explicitement : sans lui, le montage des
    # systèmes de fichiers d'API (/dev, /run, cgroup2…) échoue au démarrage et
    # systemd s'arrête sur « Failed to mount API filesystems ». « pam » est une
    # fonctionnalité : on l'active pour logind et les sessions (ADR-014).
    # networkd/resolve/logind sont des booléens : on les fixe explicitement.
    # Mode installé (ADR-015) : « importd », « sysupdate » et « repart » sont des
    # fonctionnalités, donc coupées par auto_features=disabled. systemd-importd
    # (dont dépend systemd-sysupdate, y compris pour la source « url-file ») exige
    # libcurl, openssl, zlib et liblzma ; systemd-sysupdate et systemd-repart
    # exigent en outre libfdisk. openssl et curl sont bâtis plus tôt (voir
    # ALL_PHASES) ; zlib est bâti en tête de cette étape 04, liblzma est fourni
    # par l'étape 03 (xz du chapitre 6), libfdisk par util-linux (fdisk.pc).
    # « libcryptsetup » reste coupé : le chiffrement LUKS est hors v0.2 et repart
    # n'en a pas besoin pour le partitionnement de base.
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release -Dmode=release -Dauto_features=disabled -Dlibmount=enabled -Dblkid=enabled -Dkmod=enabled -Dpcre2=enabled -Dpam=enabled -Dzlib=enabled -Dxz=enabled -Dfdisk=enabled -Dlibcurl=enabled -Dopenssl=enabled -Dimportd=enabled -Dsysupdate=enabled -Drepart=enabled -Dnetworkd=true -Dresolve=true -Dlogind=true -Dman=false -Dtranslations=false -Dtests=false -Dhwdb=false"
    log "compilation systemd"
    run chroot_run "cd '$d' && ninja -C build7"
    log "installation systemd"
    run chroot_run "cd '$d' && ninja -C build7 install"

    log "réglages de démarrage systemd"
    # Consoles locale (tty1) et série (ttyS0, utile en QEMU -nographic) :
    # agetty + login (util-linux), avec connexion automatique de root pour le
    # média live. « login -f » traverse PAM : la session est enregistrée auprès
    # de systemd-logind via pam_systemd, même sans mot de passe (REQ-SW-021).
    mkdir -pv "$LFS/etc/systemd/system/getty.target.wants" \
              "$LFS/etc/systemd/system/getty@tty1.service.d" \
              "$LFS/etc/systemd/system/serial-getty@ttyS0.service.d"
    ln -sf /usr/lib/systemd/system/multi-user.target   "$LFS/etc/systemd/system/default.target"
    ln -sf /usr/lib/systemd/system/getty@.service        "$LFS/etc/systemd/system/getty.target.wants/getty@tty1.service"
    ln -sf /usr/lib/systemd/system/serial-getty@.service "$LFS/etc/systemd/system/getty.target.wants/serial-getty@ttyS0.service"
    cat > "$LFS/etc/systemd/system/getty@tty1.service.d/autologin.conf" <<'EOF'
[Service]
ExecStart=
ExecStart=-/usr/sbin/agetty --autologin root --noclear %I $TERM
Environment=TERM=linux
EOF
    cat > "$LFS/etc/systemd/system/serial-getty@ttyS0.service.d/autologin.conf" <<'EOF'
[Service]
ExecStart=
ExecStart=-/usr/sbin/agetty --autologin root --noclear %I $TERM
Environment=TERM=linux
EOF
    ln -sf /usr/lib/systemd/systemd "$LFS/usr/sbin/init"
    run chroot_run "systemd-machine-id-setup"
}

# --- Phase : D-Bus (bus système et utilisateur) -----------------------------
# D-Bus 1.16.2 est bâti par meson et lié à libsystemd (sd-daemon) : il suit donc
# systemd et expat (analyse XML des politiques). GLib n'étant pas encore
# présent, l'option « modular_tests » (qui l'exige) est coupée.
phase_dbus() {
    require_archive dbus
    local d; d="$(ci_work dbus)"
    local b="$d/build7"
    log "configure D-Bus (meson, bus système sur libsystemd)"
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release -Dsystemd=enabled -Dselinux=disabled -Dapparmor=disabled -Dmodular_tests=disabled -Dinstalled_tests=false -Dintrusive_tests=false -Ddoxygen_docs=disabled -Dducktype_docs=disabled -Dxml_docs=disabled"
    run chroot_run "cd '$d' && ninja -C build7 && ninja -C build7 install"
}

# --- Phase : shadow (comptes et /etc/shadow) --------------------------------
# shadow 4.20.3 fournit useradd/usermod/passwd ; /etc/shadow et /etc/gshadow en
# sont dérivés (pwconv/grpconv, phase de configuration). Il est lié à libpam
# (--with-libpam) et n'embarque PAS « su » : l'élévation passe par sudo-rs (un
# besoin, un fournisseur — ADR-014). yescrypt est retenu pour les mots de passe.
phase_shadow() {
    require_archive shadow
    local d; d="$(ci_work shadow)"
    log "configure shadow"
    run chroot_run "cd '$d' && ./configure --sysconfdir=/etc --disable-static --without-su --with-libpam --with-yescrypt --disable-lastlog --with-group-name-max-length=32"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : sudo-rs (élévation de privilèges) ------------------------------
# sudo-rs 0.2.15 (Rust, ADR-012) se compile hors-ligne (ADR-018/019) ; il se lie
# nativement à libpam (#[link(name = "pam")]). Le service PAM est « sudo »
# (sudoers-rs puis sudoers, voir config/rootfs). « sudo » est setuid root ;
# « sudoedit » n'est qu'une invocation de sudo (détectée sur argv[0]), d'où le
# lien symbolique. « su » n'est pas installé (ADR-014).
phase_sudo_rs() {
    require_archive sudo-rs
    ensure_host_rust
    local d; d="$(ci_work sudo-rs)"
    log "compilation de sudo-rs (Rust, hors-ligne)"
    run chroot_rust "cd '$d' && cargo build --release --locked"
    log "installation de sudo/visudo (sudo setuid, sudoedit en lien)"
    run chroot_run "install -Dm755 '$d/target/release/sudo' /usr/bin/sudo && install -Dm755 '$d/target/release/visudo' /usr/bin/visudo && ln -sf sudo /usr/bin/sudoedit && chmod 4755 /usr/bin/sudo"
}

# --- Phases : réseau (ADR-013, ADR-020) -------------------------------------

# readline : bibliothèque de saisie de « iwctl ». ncurses (bâti en « wide » à
# l'étape 03) est référencé sous son nom ncursesw ; on lie la bibliothèque
# partagée à libncursesw (SHLIB_LIBS), que configure ne détecte pas seul.
phase_readline() {
    require_archive readline
    local d; d="$(ci_work readline)"
    log "configure readline"
    run chroot_run "cd '$d' && sed -i '/MV.*old/d' Makefile.in && sed -i '/{OLDSUFF}/c:' support/shlib-install && ./configure --prefix=/usr --with-curses"
    log "compilation readline"
    run chroot_run "cd '$d' && make SHLIB_LIBS='-lncursesw'"
    log "installation readline"
    run chroot_run "cd '$d' && make install"
}

# iwd : démon Wi-Fi en couche 2 seule (ADR-013). Il embarque « ell » (aucun
# paquet externe, ADR-017) et « iwctl » exige readline. Les emplacements D-Bus
# et systemd sont lus via pkg-config (dbus-1, systemd).
phase_iwd() {
    require_archive iwd
    local d; d="$(ci_work iwd)"
    log "configure iwd"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --localstatedir=/var --disable-manual-pages"
    log "compilation iwd"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation iwd"
    run chroot_run "cd '$d' && make install"
}

# linux-firmware : microprogrammes des pilotes activés au noyau (Wi-Fi et
# quelques cartes filaires). Le tarball est monolithique ; on l'installe en
# entier, l'image EROFS (étape 06, zstd) assurant seule la compression.
phase_linux_firmware() {
    require_archive linux-firmware
    local d; d="$(ci_work linux-firmware)"
    log "installation des microprogrammes (linux-firmware)"
    run chroot_run "cd '$d' && make install DESTDIR= FIRMWAREDIR=/lib/firmware"
}

# === Lot stockage (1/3) : socle GLib et moteurs ============================
# Ces composants n'ont rien de « stockage » en soi : ils forment la pile dont
# polkit, libbytesize, libblockdev et udisks dépendent. Ils viennent après
# systemd (libudev) et pcre2, avant le noyau.

# gettext n'est pas un confort : polkit (i18n.merge_file), udisks (msgfmt) et
# libbytesize le réclament en dur à la configuration. Seuls les outils
# (msgfmt/msgmerge/xgettext) servent ; les traductions restent désactivées
# partout ailleurs (--disable-nls).
phase_gettext() {
    require_archive gettext
    local d; d="$(ci_work gettext)"
    log "configure gettext (outils msgfmt/msgmerge/xgettext)"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --disable-static --disable-nls --disable-rpath --disable-libasprintf --disable-c++ --disable-java --disable-d --disable-modula2 --disable-openmp"
    log "compilation gettext"
    run chroot_run "cd '$d' && make"
    log "installation gettext"
    run chroot_run "cd '$d' && make install"
}

# libffi : requis par GObject. Les structures (ffi_type, défaut) restent
# activées, GObject en dépend à l'exécution.
phase_libffi() {
    require_archive libffi
    local d; d="$(ci_work libffi)"
    log "configure libffi"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static --disable-docs"
    log "compilation libffi"
    run chroot_run "cd '$d' && make"
    log "installation libffi"
    run chroot_run "cd '$d' && make install"
}

# glib : socle GObject/GIO dont dépendent libgudev, libbytesize, polkit et
# udisks. « intl » est résolu par le libc (glibc fournit libintl.h et les
# symboles gettext) : aucun proxy-libintl n'est téléchargé. --wrap-mode=
# nodownload autorise le sous-projet gvdb, vendorisé dans le tarball, tout en
# interdisant tout accès réseau.
phase_glib() {
    require_archive glib
    local d; d="$(ci_work glib)"
    local b="$d/build7"
    log "configure glib (meson)"
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release --wrap-mode=nodownload -Dtests=false -Dnls=disabled -Dman-pages=disabled -Ddocumentation=false -Dintrospection=disabled -Dlibmount=disabled -Dselinux=disabled -Dlibelf=disabled -Ddtrace=disabled -Dsystemtap=disabled -Dsysprof=disabled"
    log "compilation glib"
    run chroot_run "cd '$d' && ninja -C build7"
    log "installation glib"
    run chroot_run "cd '$d' && ninja -C build7 install"
}

# libgudev : passerelle GObject au-dessus de libudev (gudev-1.0), requise par
# udisks pour observer les périphériques de bloc.
phase_libgudev() {
    require_archive libgudev
    local d; d="$(ci_work libgudev)"
    log "configure libgudev (meson)"
    run chroot_run "rm -rf '$d/build7' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release --wrap-mode=nodownload -Dtests=disabled -Dintrospection=disabled -Dvapi=disabled"
    log "compilation libgudev"
    run chroot_run "cd '$d' && ninja -C build7"
    log "installation libgudev"
    run chroot_run "cd '$d' && ninja -C build7 install"
}

# duktape : moteur JavaScript embarqué (greffon « duktape » de polkit). Le
# Makefile.sharedlibrary installe bibliothèque, en-tête et .pc ; le binaire de
# débogage (libduktaped) est écarté.
phase_duktape() {
    require_archive duktape
    local d; d="$(ci_work duktape)"
    log "compilation duktape (bibliothèque partagée)"
    run chroot_run "cd '$d' && make -f Makefile.sharedlibrary INSTALL_PREFIX=/usr LIBDIR=/lib"
    log "installation duktape"
    run chroot_run "cd '$d' && make -f Makefile.sharedlibrary INSTALL_PREFIX=/usr LIBDIR=/lib install"
    run chroot_run "rm -f /usr/lib/libduktaped.so*"
}

# --- Phase : libelf (Elfutils, requis par objtool du noyau) -----------------
phase_libelf() {
    require_archive libelf
    local d; d="$(ci_work libelf)"
    log "configure libelf (Elfutils, requis par objtool)"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --disable-nls --disable-debuginfod --enable-libdebuginfod=dummy"
    # On ne bâtit que « config », « lib » et « libelf » : les autres
    # sous-répertoires (libcpu, backends, libdw, src) ne sont pas installés.
    # Sous C23, glibc 2.44 rend bsearch « const-générique »
    # (__glibc_const_generic) : son retour est const void* quand la base est
    # const, ce qui déclenche -Wdiscarded-qualifiers puis échoue sous -Werror
    # (libcpu/riscv_disasm.c). « lib » fournit libeu.a, requis par libelf.
    log "compilation de libelf (sous-ensemble requis par objtool)"
    run chroot_run "cd '$d' && make -C config && make -C lib && make -C libelf"
    log "installation de libelf seul"
    run chroot_run "cd '$d' && make -C libelf install && install -vDm644 config/libelf.pc /usr/lib/pkgconfig/libelf.pc && rm -fv /usr/lib/libelf.a"
}

# --- Phase : flex (requis par le kconfig du noyau) --------------------------
# Le noyau ne livre pas scripts/kconfig/{lexer.lex.c,parser.tab.c} : il les
# régénère depuis lexer.l/parser.y via « LEX = flex » et « YACC = bison ». Flex
# est donc un outil de build du noyau. L'archive de release fournit ses sources
# déjà analysées (src/scan.c, src/parse.c), si bien que flex s'amorce sans flex
# ni bison préalables (comme bison, cf. ci-dessus).
phase_flex() {
    require_archive flex
    local d; d="$(ci_work flex)"
    log "configure flex (requis par le kconfig du noyau)"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --disable-nls"
    run chroot_run "cd '$d' && make"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : zstd (compression de l'image du noyau) -------------------------
# CONFIG_KERNEL_ZSTD fait compresser arch/x86/boot/compressed/vmlinux.bin par
# l'outil « zstd », qui doit donc exister dans le chroot. zstd n'a aucune
# dépendance externe ; son Makefile installe binaire et bibliothèque.
phase_zstd() {
    require_archive zstd
    local d; d="$(ci_work zstd)"
    log "compilation zstd (requis par le noyau, CONFIG_KERNEL_ZSTD)"
    run chroot_run "cd '$d' && make -j$JOBS prefix=/usr"
    log "installation zstd"
    run chroot_run "cd '$d' && make prefix=/usr install"
}

# === Lot stockage (2/3) : outils de systèmes de fichiers ===================
# Placés après zstd (btrfs-progs exige libzstd) et avant le noyau. Un besoin,
# un fournisseur : e2fsprogs pour ext2/3/4 (fournit aussi libext2fs et e2p,
# requises par le greffon « fs » de libblockdev), btrfs-progs, dosfstools et
# exfatprogs. NTFS reste assuré par le noyau (ntfs3).

# e2fsprogs : ext2/3/4. Les bibliothèques privées blkid/uuid ne sont PAS
# bâties (défaut) : ce sont celles d'util-linux. --enable-elf-shlibs fournit
# libext2fs/e2p partagées et leurs fichiers pkg-config. --disable-uuidd laisse
# de côté le démon d'UUID.
phase_e2fsprogs() {
    require_archive e2fsprogs
    local d; d="$(ci_work e2fsprogs)"
    log "configure e2fsprogs"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --bindir=/usr/bin --sbindir=/usr/sbin --libdir=/usr/lib --sysconfdir=/etc --enable-elf-shlibs --disable-uuidd --disable-nls"
    log "compilation e2fsprogs"
    run chroot_run "cd '$d' && make"
    log "installation e2fsprogs"
    run chroot_run "cd '$d' && make install"
    run chroot_run "rm -f /usr/lib/libcom_err.a /usr/lib/libe2p.a /usr/lib/libext2fs.a /usr/lib/libss.a"
}

# btrfs-progs : btrfs. --disable-convert évite ext2fs/reiserfs ; --disable-lzo
# évite liblzo2 (non embarquée) ; la compression zstd reste disponible.
phase_btrfs_progs() {
    require_archive btrfs-progs
    local d; d="$(ci_work btrfs-progs)"
    log "configure btrfs-progs"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-documentation --disable-python --disable-convert --disable-lzo"
    log "compilation btrfs-progs"
    run chroot_run "cd '$d' && make"
    log "installation btrfs-progs"
    run chroot_run "cd '$d' && make install"
}

# dosfstools : FAT12/16/32. --enable-compat-symlinks fournit mkfs.vfat/fsck.vfat,
# réclamés par le greffon « fs » de libblockdev.
phase_dosfstools() {
    require_archive dosfstools
    local d; d="$(ci_work dosfstools)"
    log "configure dosfstools"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --enable-compat-symlinks"
    log "compilation dosfstools"
    run chroot_run "cd '$d' && make"
    log "installation dosfstools"
    run chroot_run "cd '$d' && make install"
}

# exfatprogs : exFAT (mkfs.exfat/fsck.exfat).
phase_exfatprogs() {
    require_archive exfatprogs
    local d; d="$(ci_work exfatprogs)"
    log "configure exfatprogs"
    run chroot_run "cd '$d' && ./configure --prefix=/usr"
    log "compilation exfatprogs"
    run chroot_run "cd '$d' && make"
    log "installation exfatprogs"
    run chroot_run "cd '$d' && make install"
}

# === Lot stockage (3/3) : autorisation, bibliothèques de bloc et démon =====

# polkit : autorisation des actions privilégiées, socle d'udisks2. Moteur de
# règles duktape (pas de mozjs), session suivie via logind
# (session_tracking=libsystemd-login) et authentification PAM. os_type=lfs fait
# générer /usr/lib/pam.d/polkit-1, qui inclut les chaînes PAM de YAOR
# (system-auth/system-account/system-password/system-session) — Linux-PAM
# cherche /etc/pam.d puis /usr/lib/pam.d. L'introspection GObject, les manuels,
# la documentation et les exemples sont écartés. Le correctif remplace le
# i18n.merge_file() (type XML) par une simple copie : ni itstool ni libxml2
# (voir patches/). polkit.service est activé par D-Bus (aucune section
# [Install]) ; l'utilisateur polkitd est créé par sysusers.d au démarrage.
# « pkexec », binaire setuid-root installé par polkit, ouvre une seconde voie
# d'élévation de privilèges : on le retire, l'élévation étant le seul fait de
# sudo-rs (ADR-012/014). L'agent PAM de polkit (polkit-agent-helper-1, setuid)
# reste, lui, strictement borné à l'authentification polkit.
phase_polkit() {
    require_archive polkit
    local d h; d="$(ci_work polkit)"; h="$(src_dir polkit)"
    if grep -q 'i18n.merge_file' "$h/actions/meson.build" 2>/dev/null; then
        patch -p1 -d "$h" -i "$ROOT/patches/polkit-124-merge-file-sans-itstool.patch"
    fi
    log "configure polkit (meson)"
    run chroot_run "rm -rf '$d/build7' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release --wrap-mode=nodownload -Dos_type=lfs -Dsession_tracking=libsystemd-login -Djs_engine=duktape -Dintrospection=false -Dman=false -Dgtk_doc=false -Dtests=false -Dexamples=false"
    log "compilation polkit"
    run chroot_run "cd '$d' && ninja -C build7"
    log "installation polkit"
    run chroot_run "cd '$d' && ninja -C build7 install"
    run chroot_run "rm -f /usr/bin/pkexec"
}

# === Mode installé : cryptographie (openssl) et HTTP (curl) =================
# Ces deux composants servent aussi bien le lot stockage (openssl pour
# libcryptsetup) que le mode installé (curl/openssl pour systemd-sysupdate).
# Ils sont BÂTIS TÔT (voir ALL_PHASES) car systemd les exige ; l'ordre du
# fichier suit l'historique, pas l'ordre d'exécution.

# openssl : bibliothèque cryptographique, backend de libcryptsetup et de
# systemd-sysupdate. Bâtie TÔT (avant systemd, qui l'exige pour sysupdate et
# DNSSEC) ; partagée, sans tests, pages de man écartées.
phase_openssl() {
    require_archive openssl
    local d; d="$(ci_work openssl)"
    log "configure openssl"
    run chroot_run "cd '$d' && ./config --prefix=/usr --openssldir=/etc/ssl --libdir=/usr/lib shared zlib-dynamic no-tests"
    log "compilation openssl"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation openssl"
    run chroot_run "cd '$d' && make install_sw"
    run chroot_run "mkdir -p /etc/ssl/certs /etc/ssl/private"
    run chroot_run "rm -f /usr/lib/libcrypto.a /usr/lib/libssl.a 2>/dev/null; true"
}

# curl : client HTTP(S) exigé par systemd-importd et systemd-sysupdate (source
# « url-file », ADR-015). Bâti sur OpenSSL (TLS) et zlib ; les protocoles et
# backends inutiles sont écartés pour garder l'empreinte minimale.
phase_curl() {
    require_archive curl
    local d; d="$(ci_work curl)"
    log "configure curl"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static --disable-docs --disable-manual --disable-ldap --disable-ldaps --with-openssl --with-zlib --without-libpsl --without-brotli --without-zstd --without-nghttp2 --without-libidn2 --without-librtmp --without-libssh2"
    log "compilation curl"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation curl"
    run chroot_run "cd '$d' && make install"
}

# json-c 0.19 : requis par cryptsetup et libnvme. Correctif pour headers/pkg-config.
phase_json_c() {
    require_archive json-c
    local d h; d="$(ci_work json-c)"; h="$(src_dir json-c)"
    if [ -f "$ROOT/patches/json-c-0.19-meson-pc.patch" ]; then
        patch -p1 -d "$h" -i "$ROOT/patches/json-c-0.19-meson-pc.patch"
    fi
    log "configure json-c (meson)"
    run chroot_run "rm -rf '$d/build' && cd '$d' && meson setup build --prefix=/usr --libdir=/usr/lib --buildtype=release -Dbuild_apps=false"
    log "compilation json-c"
    run chroot_run "cd '$d' && ninja -C build"
    log "installation json-c"
    run chroot_run "cd '$d' && ninja -C build install"
}

# popt : requis par cryptsetup.
phase_popt() {
    require_archive popt
    local d; d="$(ci_work popt)"
    log "configure popt"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static --disable-nls"
    log "compilation popt"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation popt"
    run chroot_run "cd '$d' && make install"
}

# keyutils : requis par libblockdev crypto.
phase_keyutils() {
    require_archive keyutils
    local d; d="$(ci_work keyutils)"
    log "compilation keyutils"
    run chroot_run "cd '$d' && make -j$JOBS NO_ARLIB=1 LIBDIR=/usr/lib USRLIBDIR=/usr/lib"
    log "installation keyutils"
    run chroot_run "cd '$d' && make NO_ARLIB=1 LIBDIR=/usr/lib USRLIBDIR=/usr/lib BINDIR=/usr/bin SBINDIR=/usr/sbin ETCDIR=/etc INCLUDEDIR=/usr/include MANDIR=/usr/share/man SHAREDIR=/usr/share/keyutils install"
    run chroot_run "rm -f /usr/sbin/request-key /usr/sbin/key.dns_resolver /etc/request-key.conf && rm -rf /etc/request-key.d /usr/share/keyutils"
}

# LVM2 libdevmapper : requis par libcryptsetup.
phase_lvm2() {
    require_archive LVM2
    local d; d="$(ci_work LVM2)"
    log "configure libdevmapper"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --sbindir=/usr/sbin --sysconfdir=/etc --disable-nls --enable-pkgconfig"
    log "compilation libdevmapper"
    run chroot_run "cd '$d' && make -C libdm -j$JOBS"
    log "installation libdevmapper"
    run chroot_run "cd '$d' && make -C libdm install"
}

# libcryptsetup : requis par libblockdev crypto plugin (bibliothèque uniquement).
phase_libcryptsetup() {
    require_archive libcryptsetup
    local d; d="$(ci_work libcryptsetup)"
    log "configure libcryptsetup"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --sbindir=/usr/sbin --sysconfdir=/etc --disable-static --with-crypto_backend=openssl --disable-cryptsetup --disable-veritysetup --disable-integritysetup --disable-ssh-token --disable-asciidoc --disable-nls --disable-selinux"
    log "compilation libcryptsetup"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation libcryptsetup"
    run chroot_run "cd '$d' && make install"
}

# libnvme : requis par libblockdev nvme plugin.
phase_libnvme() {
    require_archive libnvme
    local d; d="$(ci_work libnvme)"
    log "configure libnvme (meson)"
    run chroot_run "rm -rf '$d/build' && cd '$d' && meson setup build --prefix=/usr --libdir=/usr/lib -Djson-c=enabled -Dopenssl=disabled -Dkeyutils=disabled -Dlibdbus=disabled -Dliburing=disabled -Dpython=disabled -Ddocs=false -Dexamples=false -Dtests=false"
    log "compilation libnvme"
    run chroot_run "cd '$d' && ninja -C build"
    log "installation libnvme"
    run chroot_run "cd '$d' && ninja -C build install"
}

# libatasmart : requis par libblockdev smart plugin.
phase_libatasmart() {
    require_archive libatasmart
    local d; d="$(ci_work libatasmart)"
    log "configure libatasmart"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static"
    log "compilation libatasmart"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation libatasmart"
    run chroot_run "cd '$d' && make install"
}

# libbytesize : conversions de tailles en octets, exigée par les greffons
# « btrfs » et « mdraid » de libblockdev (sur gmp/mpfr).
phase_libbytesize() {
    require_archive libbytesize
    local d; d="$(ci_work libbytesize)"
    log "configure libbytesize"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --disable-static --disable-nls"
    log "compilation libbytesize"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation libbytesize"
    run chroot_run "cd '$d' && make install"
}

# libblockdev : socle de manipulation des périphériques de bloc d'udisks2. Le
# jeu de greffons est imposé par udisks (voir les notes de l'en-tête du lot),
# et l'introspection GObject est écartée (g-i est absent du chroot, et un
# g-ir-scanner présent sans g-ir-compiler casse la génération du typelib).
phase_libblockdev() {
    require_archive libblockdev
    local d; d="$(ci_work libblockdev)"
    log "configure libblockdev"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --sysconfdir=/etc --without-s390 --without-python3 --without-gtk-doc --disable-introspection --disable-tests --without-escrow --without-tools --without-smartmontools --without-dm --without-lvm --without-lvm_dbus --without-mpath --without-nvdimm"
    log "compilation libblockdev"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation libblockdev"
    run chroot_run "cd '$d' && make install"
}

# udisks2 : démon de montage automatique des volumes amovibles (ADR-015). Le
# drapeau « btrfs » est activé pour rester cohérent avec le greffon homonyme de
# libblockdev ; l'ACL, les manuels, gtk-doc et l'introspection sont écartés.
phase_udisks() {
    require_archive udisks
    local d; d="$(ci_work udisks)"
    log "configure udisks"
    run chroot_run "cd '$d' && ./configure --prefix=/usr --libdir=/usr/lib --sysconfdir=/etc --localstatedir=/var --disable-man --disable-gtk-doc --disable-introspection --disable-acl --enable-btrfs --disable-nls"
    log "compilation udisks"
    run chroot_run "cd '$d' && make -j$JOBS"
    log "installation udisks"
    run chroot_run "cd '$d' && make install"
}

# --- Phase : noyau ----------------------------------------------------------
phase_kernel() {
    require_archive linux
    local d ver
    d="$(ci_work linux)"
    ver="$(manifest_field linux 3)"
    log "configuration du noyau (defconfig + config/kernel-live.fragment)"
    run chroot_run "cd '$d' && make mrproper"
    run chroot_run "cd '$d' && make defconfig"
    run chroot_run "cd '$d' && ./scripts/kconfig/merge_config.sh -m .config /build/config/kernel-live.fragment && make olddefconfig"
    log "compilation du noyau"
    run chroot_run "cd '$d' && make"
    log "installation des modules"
    run chroot_run "cd '$d' && make modules_install"
    run chroot_run "cd '$d' && cp -v arch/x86/boot/bzImage /boot/vmlinuz-$ver && cp -v System.map /boot/System.map-$ver && cp -v .config /boot/config-$ver"
}

# --- Phase : configuration système (ADR-013/014/015) ------------------------
# Copie l'arborescence de configuration fournie par YAOR (config/rootfs, miroir
# de la racine) APRÈS les paquets, pour primer sur leurs fichiers par défaut.
# Puis traite ce qui n'est pas un simple fichier : groupe « wheel », dérivés de
# /etc/passwd et /etc/group, /etc/resolv.conf et activation des services.
phase_configure_system() {
    section "Configuration système (config/rootfs → /)"
    # --no-preserve=ownership : le dépôt est consulté par l'utilisateur de
    # compilation (uid 1000) ; sans cela « cp -a » recopierait cet uid dans le
    # système construit. Or sudo refuse un /etc/sudoers qui n'appartient pas à
    # root, et polkit n'inspecte que les règles déposées par root. Les fichiers
    # ainsi posés appartiennent donc au compte qui exécute la construction
    # (root, la phase 04 étant privilégiée).
    run cp -a --no-preserve=ownership "$CONFIG_DIR/rootfs/." "$LFS/"
    # sudoers et son complément ne doivent pas être lisibles par tous.
    chroot_run "chmod 440 /etc/sudoers /etc/sudoers.d/00-wheel"

    section "Comptes : groupe wheel et ombres"
    # « wheel » porte l'administration (sudoers, règle polkit — ADR-014), GID 998.
    chroot_run "grep -q '^wheel:' /etc/group || echo 'wheel:x:998:' >> /etc/group"
    # pam_unix exige une entrée shadow (même verrouillée) ; on dérive /etc/shadow
    # et /etc/gshadow de /etc/passwd et /etc/group.
    chroot_run "[ -f /etc/shadow ]  || pwconv"
    chroot_run "[ -f /etc/gshadow ] || grpconv"

    # Sessions : dbus.socket (paquet dbus) et systemd-logind.service (systemd)
    # sont activés par les liens symboliques posés à l'installation de leurs
    # paquets (sockets.target.wants, multi-user.target.wants) ; ces unités n'ont
    # pas de section [Install], donc aucun « systemctl enable » n'est nécessaire.

    # Stockage : udisks2.service est de type « dbus » et accompagne un fichier
    # d'activation D-Bus (org.freedesktop.UDisks2.service dans
    # /usr/share/dbus-1/system-services). Le démon est donc lancé à la demande,
    # dès qu'un client réclame le nom org.freedesktop.UDisks2 — utile sur une
    # racine live qui n'atteint pas graphical.target (cible de son [Install]).
    # Aucune activation au démarrage n'est requise.

    section "Activation des services (réseau)"
    # systemd-networkd et systemd-resolved portent [Install] (multi-user.target
    # et sysinit.target) ; iwd aussi (multi-user.target). Seules ces unités
    # s'activent donc par « systemctl enable ».
    chroot_run "systemctl enable systemd-networkd systemd-resolved iwd"

    section "Activation des services (mode installé)"
    # yaor-slot.service (essai/retour arrière, avant sysinit) et
    # yaor-slot-confirm.service (confirmation, après multi-user) portent
    # [Install] ; ils restent inactifs sur le média live grâce à leurs
    # conditions (ADR-015). yaor-install est une commande, pas un service.
    chroot_run "systemctl enable yaor-slot.service yaor-slot-confirm.service"

    section "Résolution de noms et profils Wi-Fi"
    # systemd-resolved expose son résolveur local : /etc/resolv.conf doit y
    # pointer (le fichier n'est fourni par aucun paquet).
    chroot_run "ln -sf /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf"
    # Profils Wi-Fi persistants d'iwd (l'unité les crée aussi à l'exécution).
    chroot_run "mkdir -p -m 0700 /var/lib/iwd"
}

# --- Phases : composants Rust (uutils) --------------------------------------
# Les binaires Rust remplacent leurs homologues GNU (ADR-017) ; ils sont
# compilés avec la toolchain Rust de l'hôte (ADR-019) et hors-ligne (ADR-018).
# SKIP_UTILS écarte les programmes que util-linux fournit déjà (more, kill) :
# un besoin n'a qu'un seul fournisseur (ADR-017).

phase_rust_findutils() {
    require_archive uutils-findutils
    ensure_host_rust
    local d; d="$(ci_work uutils-findutils)"
    log "compilation uutils/findutils (Rust, remplace GNU findutils)"
    run chroot_rust "cd '$d' && cargo build --release --locked"
    # findutils n'a pas de cible « install » : on pose les quatre binaires
    # (GNU find, xargs, locate, updatedb sont écrasés).
    log "installation findutils (find, xargs, locate, updatedb)"
    run chroot_run "cd '$d/target/release' && cp -v find xargs locate updatedb /usr/bin/ && chmod 755 /usr/bin/find /usr/bin/xargs /usr/bin/locate /usr/bin/updatedb"
}

phase_rust_coreutils() {
    require_archive uutils-coreutils
    ensure_host_rust
    local d; d="$(ci_work uutils-coreutils)"
    # GNU « install » est sur le point d'être remplacé : on en garde une copie
    # hors de /usr/bin pour que « make install » l'utilise jusqu'au bout.
    run chroot_run "cp -v /usr/bin/install /build/work/.gnu-install && chmod 755 /build/work/.gnu-install"
    log "compilation et installation uutils/coreutils (Rust, remplace GNU coreutils)"
    # PROFILE=release est implicite pour « install ». SKIP_UTILS écarte more/kill
    # (fournis par util-linux). Ne PAS surcharger CARGOFLAGS : le GNUmakefile y
    # ajoute feat_external_libstdbuf (exigé par le binaire stdbuf). Le hors-ligne
    # passe par l'environnement (CARGO_NET_OFFLINE), pas par une variable de make.
    run chroot_rust "cd '$d' && make install PREFIX=/usr MULTICALL=n SKIP_UTILS='more kill' MANPAGES=n COMPLETIONS=n LOCALES=n INSTALL=/build/work/.gnu-install"
    # « make install » couvre aussi « [ » (copie de « test »), comme GNU.
    # Contrôle de non-duplication : aucun binaire GNU ne doit subsister dans le
    # PATH (la chaîne « GNU coreutils »/« GNU findutils » figure dans leur
    # --version, et jamais dans les binaires Rust).
    log "vérification : plus aucun binaire GNU coreutils/findutils dans /usr/bin"
    run chroot_run "if grep -rl -e 'GNU coreutils' -e 'GNU findutils' /usr/bin /usr/sbin 2>/dev/null; then echo 'ERREUR : binaires GNU résiduels ci-dessus' >&2; exit 1; fi"
    run chroot_run "ls --version | head -1; find --version | head -1"
}

# --- Phase : composants internes YAOR (Rust) --------------------------------
# `yaor-install` et `yaor-slot` (ADR-002) : leurs sources vivent dans le dépôt
# (components/), montées dans le chroot sur /build/components. Ils sont bâtis
# par la toolchain Rust de l'hôte (ADR-019), hors-ligne (ADR-018) — sans aucune
# dépendance externe, donc sans vendoring. Les « target/ » et le CARGO_HOME
# restent sous /build/work, jamais dans le dépôt.
phase_yaor_components() {
    ensure_host_rust
    # Cohérence de version (ADR-022) : le fichier VERSION (version produit,
    # reportée en IMAGE_VERSION) et le workspace cargo des composants doivent
    # concorder — sans quoi l'image et ses binaires s'annonceraient différemment.
    local prodver cargover
    prodver="$(yaor_version)"
    cargover="$(awk -F'"' '/^version[[:space:]]*=/{print $2; exit}' "$COMPONENTS_DIR/Cargo.toml")"
    [[ "$cargover" == "$prodver" ]] \
        || die "version des composants ($cargover) ≠ VERSION ($prodver) — ADR-022"
    log "compilation des composants internes YAOR (yaor-install, yaor-slot) version $prodver"
    run chroot_rust "cd /build/components && CARGO_TARGET_DIR=/build/work/yaor-target cargo build --release --locked"
    log "installation de yaor-install et yaor-slot dans /usr/bin"
    run chroot_run "install -m 0755 /build/work/yaor-target/release/yaor-install /build/work/yaor-target/release/yaor-slot /usr/bin/"
    run chroot_run "yaor-slot --help >/dev/null && yaor-install --help >/dev/null && echo 'composants YAOR opérationnels'"
}

# --- Exécution --------------------------------------------------------------
run_phase() {
    local p="$1"
    if [[ "$p" == "teardown" ]]; then
        phase_teardown
        return
    fi
    local fn="phase_${p//-/_}"
    declare -F "$fn" >/dev/null || die "phase inconnue : $p"
    if stamp_exists "ch7.$p"; then
        log "phase déjà faite : $p (marqueur ch7.$p)"
        return
    fi
    section "Phase $p"
    "$fn"
    stamp_done "ch7.$p"
    log "phase terminée : $p"
}

# Garde-fou : les étapes 02 et 03 doivent être terminées.
for s in binutils1 gcc1 headers glibc libstdcxx; do
    stamp_exists "$s" || die "étape 02 incomplète (marqueur manquant : $s)"
done
stamp_exists "ch6.gcc2" || die "étape 03 incomplète (marqueur manquant : ch6.gcc2)"

if (($# == 0)); then
    for p in "${ALL_PHASES[@]}"; do run_phase "$p"; done
else
    for p in "$@"; do run_phase "$p"; done
fi

log "Terminé. Les montages de $LFS restent en place : 'sudo $0 teardown' pour les retirer."
