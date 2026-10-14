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
#   - libelf (Elfutils) pour objtool, puis systemd (PID 1) et le noyau Linux.
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
    libcap libxcrypt util-linux kmod pcre2
    bc gperf
    # systemd 262 exige jinja2 (rendu des .pc et de divers en-têtes, dont
    # libudev.pc) ; markupsafe est sa dépendance d'import.
    markupsafe jinja2
    systemd
    libelf
    kernel
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
    mkdir -pv "$LFS"/{etc,usr/{bin,lib,sbin,include},var/{log,tmp,lib,spool},run/lock,srv,opt,root,home,mnt,proc,sys,dev/{pts,shm},tmp,boot,sources,build/{work,config}}

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
    mkdir -pv "$LFS/run/lock"

    section "Vérification du chroot"
    run chroot_run "echo 'chroot opérationnel' && uname -m && gcc --version | head -1"
}

phase_teardown() {
    section "Démontage de la cible"
    local mp
    for mp in "$LFS/run" "$LFS/dev/pts" "$LFS/dev" "$LFS/proc" "$LFS/sys" \
              "$LFS/sources" "$LFS/build/work" "$LFS/build/config"; do
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

phase_util_linux() {
    require_archive util-linux
    local d; d="$(ci_work util-linux)"
    log "configure util-linux"
    run chroot_run "cd '$d' && ./configure --bindir=/usr/bin --sbindir=/usr/sbin --libdir=/usr/lib --runstatedir=/run --disable-chfn-chsh --disable-login --disable-nologin --disable-su --disable-setpriv --disable-runuser --disable-pylibmount --disable-liblastlog2 --disable-static --without-python ADJTIME_PATH=/var/run/adjtime"
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
    run chroot_run "rm -rf '$b' && cd '$d' && meson setup build7 --prefix=/usr --libdir=/usr/lib --buildtype=release -Dmode=release -Dauto_features=disabled -Dblkid=enabled -Dkmod=enabled -Dpcre2=enabled -Dman=false -Dtranslations=false -Dtests=false -Dhwdb=false"
    log "compilation systemd"
    run chroot_run "cd '$d' && ninja -C build7"
    log "installation systemd"
    run chroot_run "cd '$d' && ninja -C build7 install"

    log "réglages de démarrage systemd"
    mkdir -pv "$LFS/etc/systemd/system/getty.target.wants" "$LFS/etc/systemd/system/getty@tty1.service.d"
    ln -sf /usr/lib/systemd/system/multi-user.target "$LFS/etc/systemd/system/default.target"
    ln -sf /usr/lib/systemd/system/getty@.service   "$LFS/etc/systemd/system/getty.target.wants/getty@tty1.service"
    cat > "$LFS/etc/systemd/system/getty@tty1.service.d/autologin.conf" <<'EOF'
[Service]
ExecStart=
ExecStart=-/usr/sbin/agetty --autologin root --noclear %I $TERM
EOF
    ln -sf /usr/lib/systemd/systemd "$LFS/usr/sbin/init"
    run chroot_run "systemd-machine-id-setup"
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
