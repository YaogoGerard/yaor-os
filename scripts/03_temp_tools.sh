#!/usr/bin/env bash
# 03_temp_tools.sh — compile les outils temporaires (chapitre 6 de LFS).
#
# Depuis la toolchain croisée (chapitre 5) on bâtit un petit userland pour la
# cible : m4, ncurses, bash, coreutils, diffutils, file, findutils, gawk, grep,
# gzip, make, patch, sed, tar, xz, puis binutils et gcc (pass 2). Ces outils
# sont installés dans la racine cible (DESTDIR=$LFS, soit /usr vu depuis la
# cible) et servent à bâtir le système final par chroot à l'étape suivante.
#
# Cette étape s'exécute en tant qu'utilisateur normal : elle n'installe rien
# hors de build/ et ne nécessite donc pas root.
#
# Usage :
#   ./scripts/03_temp_tools.sh              # enchaîne toutes les phases
#   ./scripts/03_temp_tools.sh bash coreutils   # relance des phases choisies
#   JOBS=6 ./scripts/03_temp_tools.sh       # fixe le parallélisme
#
# Chaque phase réussie est marquée dans build/stamps/ : relancer le script ne
# refait que ce qui manque.

set -euo pipefail
STEP=03_temp_tools
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

# --- Environnement de construction ------------------------------------------

LFS="$BUILD_DIR/rootfs"          # racine du système cible
TOOLS="$LFS/tools"               # toolchain temporaire (chapitre 5)
WORK="$BUILD_DIR/work"           # sources extraites
LFS_TGT="$(uname -m)-lfs-linux-gnu"
JOBS="${JOBS:-$(nproc)}"
BUILD_LOG="$LOG_DIR/${STEP}.build.log"

export LFS TOOLS LFS_TGT MAKEFLAGS="-j$JOBS"
export LC_ALL=POSIX
export PATH="$TOOLS/bin:$PATH"

unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS ZONEINFO 2>/dev/null || true

mkdir -p "$LFS" "$TOOLS" "$WORK"

[[ "$(uname -m)" == x86_64 ]] || die "jalon 0.1 : l'hôte cible doit être x86_64"

# Triple de la machine de construction (l'hôte), pour les configure croisés.
BUILD_TRIPLE="$(gcc -dumpmachine)"

# --- Prérequis --------------------------------------------------------------

# La toolchain temporaire (étape précédente) doit être complète.
for st in binutils1 gcc1 headers glibc libstdcxx; do
    stamp_exists "$st" || die "toolchain incomplète : marqueur '$st' manquant (lancer scripts/02_toolchain.sh)"
done

# Le triples cible attendu par la toolchain.
[[ -x "$TOOLS/bin/$LFS_TGT-gcc" ]] \
    || die "compilateur croisé introuvable : $TOOLS/bin/$LFS_TGT-gcc"

# Toutes les archives doivent être téléchargées et vérifiées.
CH6=(m4 ncurses bash coreutils diffutils file findutils gawk grep gzip make patch sed tar xz binutils gcc)
for name in "${CH6[@]}"; do
    [[ -f "$(source_archive "$name")" ]] \
        || die "archive manquante pour '$name' (lancer scripts/01_download.sh)"
done

log "cible=$LFS_TGT jobs=$JOBS ram=$(awk '/MemAvailable/{printf "%.1fG", $2/1048576}' /proc/meminfo)"

# --- Helpers ----------------------------------------------------------------

run() { "$@" 2>&1 | tee -a "$BUILD_LOG"; }

# build_simple <nom> [options configure...] : configure + make + install d'un
# paquet autotools « standard », compilé pour la cible et installé dans $LFS.
build_simple() {
    local name="$1"; shift
    local d; d="$(src_dir "$name")"
    pushd "$d" >/dev/null
    log "configure $name"
    run ./configure --prefix=/usr --host="$LFS_TGT" --build="$BUILD_TRIPLE" "$@"
    log "compilation $name"
    run make
    log "installation $name"
    run make DESTDIR="$LFS" install
    popd >/dev/null
}

# --- Phases : paquets autotools courants ------------------------------------

phase_m4()        { build_simple m4; }

# diffutils 3.12 : gnulib vérifie « whether strcasecmp works » par un programme
# d'essai dont la branche cross-compilation de configure échoue au lieu de
# retenir la valeur par défaut. On fournit directement la valeur devinée.
phase_diffutils() {
    gl_cv_func_strcasecmp_works="guessing yes" build_simple diffutils
}
phase_findutils() { build_simple findutils --localstatedir=/var/lib/locate; }
phase_gzip()      { build_simple gzip; }
phase_make()      { build_simple make; }
phase_patch()     { build_simple patch; }
phase_sed()       { build_simple sed; }
phase_tar()       { build_simple tar; }

phase_gawk() {
    local d; d="$(src_dir gawk)"
    sed -i 's/extras//' "$d/Makefile.in"
    # gawk 5.4.1 construit sans MPFR partage un même emplacement entre
    # `vname` (sub.nodep.name) et `stfmt` (sub.val.idx). Sortir `vname` de
    # l'union corrige à la fois le formatage des éléments de tableau vides et
    # les `free()` invalides lors de la promotion d'un élément en variable
    # (voir patches/gawk-5.4.1-vname-out-of-union.patch).
    if ! grep -q 'char \*vname;' "$d/awk.h"; then
        patch -p1 -d "$d" -i "$ROOT/patches/gawk-5.4.1-vname-out-of-union.patch"
    fi
    build_simple gawk
}

phase_grep() {
    local d; d="$(src_dir grep)"
    sed -i 's/echo/#echo/' "$d/src/egrep.sh"
    build_simple grep
}

phase_xz() {
    build_simple xz --disable-static --docdir="/usr/share/doc/xz-$(manifest_field xz 3)"
    rm -fv "$LFS/usr/lib/liblzma.la" 2>/dev/null || true
}

# --- Phases : paquets à traitement particulier ------------------------------

phase_ncurses() {
    local d; d="$(src_dir ncurses)"
    pushd "$d" >/dev/null
    log "configure ncurses"
    run ./configure --prefix=/usr \
        --host="$LFS_TGT" \
        --build="$BUILD_TRIPLE" \
        --mandir=/usr/share/man \
        --with-shared \
        --without-debug \
        --without-normal \
        --with-cxx-shared \
        --enable-pc-files \
        --with-pkg-config-libdir=/usr/lib/pkgconfig \
        --enable-widec
    log "compilation ncurses"
    run make
    log "installation ncurses"
    run make DESTDIR="$LFS" install
    popd >/dev/null

    # ncurses n'installe que les bibliothèques « wide » (--without-normal) :
    # on expose des alias non-wide qui pointent vers libncursesw, comme LFS.
    local libdir="$LFS/usr/lib" real
    real="$(basename "$(ls "$libdir"/libncursesw.so.6.* 2>/dev/null | head -1)" 2>/dev/null || true)"
    if [[ -n "$real" ]]; then
        for name in ncurses tinfo; do
            [[ -e "$libdir/lib$name.so" ]] && continue
            log "alias ncurses : lib$name.so -> $real"
            ln -sf "$real"          "$libdir/lib$name.so"
            ln -sf "$real"          "$libdir/lib$name.so.6"
        done
    fi
}

phase_bash() {
    local d; d="$(src_dir bash)"
    pushd "$d" >/dev/null
    log "configure bash"
    run ./configure --prefix=/usr \
        --host="$LFS_TGT" \
        --build="$BUILD_TRIPLE" \
        --without-bash-malloc
    log "compilation bash"
    run make
    log "installation bash"
    run make DESTDIR="$LFS" install
    popd >/dev/null
    # /bin/sh : shell par défaut. On installe le lien à côté de bash dans
    # /usr/bin (rappel : /bin deviendra un lien vers usr/bin à l'étape chroot).
    ln -sf bash "$LFS/usr/bin/sh"
}

phase_coreutils() {
    local d; d="$(src_dir coreutils)"
    pushd "$d" >/dev/null
    log "configure coreutils"
    run ./configure --prefix=/usr \
        --host="$LFS_TGT" \
        --build="$BUILD_TRIPLE" \
        --enable-install-program=hostname \
        --enable-no-install-program=kill,uptime
    log "compilation coreutils"
    run make
    log "installation coreutils"
    run make DESTDIR="$LFS" install
    popd >/dev/null
    # chroot appartient à /usr/sbin (et sa page de manuel en section 8).
    mkdir -p "$LFS/usr/sbin" "$LFS/usr/share/man/man8"
    [[ -f "$LFS/usr/bin/chroot" ]] && mv -v "$LFS/usr/bin/chroot" "$LFS/usr/sbin/"
    if [[ -f "$LFS/usr/share/man/man1/chroot.1" ]]; then
        mv -v "$LFS/usr/share/man/man1/chroot.1" "$LFS/usr/share/man/man8/chroot.8"
        sed -i 's/"1"/"8"/' "$LFS/usr/share/man/man8/chroot.8"
    fi
}

phase_file() {
    local d; d="$(src_dir file)"
    # Premier passage : un « file » natif pour l'hôte, utilisé comme FILE_COMPILE
    # (le file cible ne peut pas s'exécuter pendant la compilation).
    pushd "$d" >/dev/null
    rm -rf build; mkdir build
    pushd build >/dev/null
    log "configure file (outil hôte)"
    run ../configure --disable-bzlib --disable-libseccomp --disable-xzlib --disable-zlib
    run make
    popd >/dev/null
    log "configure file (cible)"
    run ./configure --prefix=/usr --host="$LFS_TGT" --build="$BUILD_TRIPLE"
    log "compilation file"
    run make FILE_COMPILE="$(pwd)/build/src/file"
    log "installation file"
    run make DESTDIR="$LFS" install
    rm -fv "$LFS/usr/lib/libmagic.la" 2>/dev/null || true
    popd >/dev/null
}

# --- Phases : second passage binutils et gcc --------------------------------

phase_binutils2() {
    local d; d="$(src_dir binutils)"
    rm -rf "$d/build2"; mkdir -p "$d/build2"
    pushd "$d/build2" >/dev/null
    log "configure binutils (pass 2)"
    run "$d/configure" --prefix=/usr \
        --build="$BUILD_TRIPLE" \
        --host="$LFS_TGT" \
        --disable-nls \
        --enable-gprofng=no \
        --disable-werror \
        --enable-64-bit-bfd \
        --enable-new-dtags \
        --enable-default-hash-style=gnu
    log "compilation binutils (pass 2)"
    run make
    log "installation binutils (pass 2)"
    run make DESTDIR="$LFS" install
    popd >/dev/null
    # Sans --enable-shared, ld est lié statiquement à bfd (et à zlib embarqué) :
    # aucune dépendance libz.so.1 à l'exécution. On purge néanmoins tout objet
    # partagé qu'une exécution antérieure aurait pu installer.
    rm -fv "$LFS"/usr/lib/lib{bfd,ctf,ctf-nobfd,opcodes,sframe}*.{a,la,so,so.*} 2>/dev/null || true
}

phase_gcc2() {
    local d; d="$(src_dir gcc)"
    # gmp/mpfr/mpc ont été déplacés dans le dossier gcc lors du pass 1 ; on les
    # retrouve donc ici sans rien refaire.
    for sub in gmp mpfr mpc; do
        [[ -d "$d/$sub" ]] || die "$sub absent de l'arbre gcc (relancer la phase gcc1)"
    done
    sed -e '/m64=/s/lib64/lib/' -i.orig "$d/gcc/config/i386/t-linux64"

    rm -rf "$d/build2"; mkdir -p "$d/build2"
    pushd "$d/build2" >/dev/null
    log "configure gcc (pass 2)"
    run "$d/configure" --build="$BUILD_TRIPLE" \
        --host="$LFS_TGT" \
        --target="$LFS_TGT" \
        LDFLAGS_FOR_TARGET="-L$PWD/$LFS_TGT/libgcc" \
        --prefix=/usr \
        --with-build-sysroot="$LFS" \
        --enable-default-pie \
        --enable-default-ssp \
        --disable-nls \
        --disable-multilib \
        --disable-libatomic \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libsanitizer \
        --disable-libssp \
        --disable-libvtv \
        --enable-languages=c,c++
    log "compilation gcc (pass 2)"
    run make
    log "installation gcc (pass 2)"
    run make DESTDIR="$LFS" install
    popd >/dev/null
    ln -sf gcc "$LFS/usr/bin/cc"
}

# --- Exécution --------------------------------------------------------------

ALL_PHASES=(m4 ncurses bash coreutils diffutils file findutils gawk grep \
            gzip make patch sed tar xz binutils2 gcc2)

run_phase() {
    local p="$1"
    if stamp_exists "ch6.$p"; then
        log "phase déjà faite : $p (ignorée)"
        return 0
    fi
    section "phase : $p"
    "phase_$p"
    stamp_done "ch6.$p"
    log "phase terminée : $p"
}

phases=("$@")
((${#phases[@]})) || phases=("${ALL_PHASES[@]}")

for p in "${phases[@]}"; do
    case " ${ALL_PHASES[*]} " in
        *" $p "*) run_phase "$p" ;;
        *) die "phase inconnue : '$p' (valides : ${ALL_PHASES[*]})" ;;
    esac
done

log "Outils temporaires installés dans $LFS"
