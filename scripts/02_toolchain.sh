#!/usr/bin/env bash
# 02_toolchain.sh — construit la toolchain temporaire (chapitre 5 de LFS).
#
# Produit un compilateur croisé dans build/rootfs/tools (soit /tools vu depuis
# la cible, comme dans LFS), capable de bâtir le système cible sans dépendre des
# outils de l'hôte. La toolchain temporaire vit *dans* la racine cible : c'est ce
# qui permet au compilateur croisé (dont le « sysroot » est build/rootfs) de
# retrouver ses en-têtes C++ et de bâtir ensuite la cible par chroot. Phases,
# dans l'ordre :
#   binutils1  -> binutils (assembleur, éditeur de liens) pour la cible
#   gcc1       -> gcc croisé C/C++ (bâti avec gmp, mpfr, mpc)
#   headers    -> en-têtes d'API du noyau Linux
#   glibc      -> bibliothèque C cible
#   libstdcxx  -> bibliothèque standard C++ cible
#
# Usage :
#   ./scripts/02_toolchain.sh              # enchaîne toutes les phases
#   ./scripts/02_toolchain.sh glibc        # relance une seule phase
#   JOBS=6 ./scripts/02_toolchain.sh       # fixe le parallélisme
#
# Chaque phase réussie est marquée dans build/stamps/ : relancer le script ne
# refait que ce qui manque. Supprimer build/ repart de zéro.

set -euo pipefail
STEP=02_toolchain
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

# --- Environnement de construction (chapitre 5 de LFS) ----------------------

LFS="$BUILD_DIR/rootfs"          # racine du système cible
TOOLS="$LFS/tools"               # toolchain temporaire (dans la cible, cf. LFS)
WORK="$BUILD_DIR/work"           # sources extraites
LFS_TGT="$(uname -m)-lfs-linux-gnu"
JOBS="${JOBS:-$(nproc)}"
BUILD_LOG="$LOG_DIR/${STEP}.build.log"

export LFS TOOLS LFS_TGT MAKEFLAGS="-j$JOBS"
export LC_ALL=POSIX
export PATH="$TOOLS/bin:$PATH"

# Neutralise toute variable de l'hôte qui fausserait la construction cible.
unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS ZONEINFO 2>/dev/null || true

mkdir -p "$LFS" "$TOOLS" "$WORK"

[[ "$(uname -m)" == x86_64 ]] || die "jalon 0.1 : l'hôte cible doit être x86_64"
need_cmd gcc g++ make bison m4 patch perl python3 tar xz gzip bzip2 \
         awk sed grep find readelf

for name in binutils gcc gmp mpfr mpc glibc linux; do
    [[ -f "$(source_archive "$name")" ]] \
        || die "archive manquante pour '$name' (lancer scripts/01_download.sh)"
done

GCC_VER="$(manifest_field gcc 3)"
GLIBC_VER="$(manifest_field glibc 3)"

log "cible=$LFS_TGT jobs=$JOBS ram=$(awk '/MemAvailable/{printf "%.1fG", $2/1048576}' /proc/meminfo)"

# --- Helpers ----------------------------------------------------------------

# run <commande...> : exécute en dupliquant la sortie vers le journal de build.
run() { "$@" 2>&1 | tee -a "$BUILD_LOG"; }

require_phase() { stamp_exists "$1" || die "phase prérequise manquante : $1"; }

# --- Phases -----------------------------------------------------------------

phase_binutils1() {
    local d; d="$(src_dir binutils)"
    rm -rf "$d/build"; mkdir -p "$d/build"
    log "configure binutils (pass 1)"
    pushd "$d/build" >/dev/null
    run "$d/configure" \
        --prefix="$TOOLS" \
        --with-sysroot="$LFS" \
        --target="$LFS_TGT" \
        --disable-nls \
        --enable-gprofng=no \
        --disable-werror \
        --enable-new-dtags \
        --enable-default-hash-style=gnu
    log "compilation + installation binutils (pass 1)"
    run make
    run make install
    popd >/dev/null
}

phase_gcc1() {
    local d; d="$(src_dir gcc)"
    local gmp mpfr mpc
    gmp="$(extract_tarball "$(source_archive gmp)")"
    mpfr="$(extract_tarball "$(source_archive mpfr)")"
    mpc="$(extract_tarball "$(source_archive mpc)")"
    [[ -d "$d/gmp"  ]] || mv "$gmp"  "$d/gmp"
    [[ -d "$d/mpfr" ]] || mv "$mpfr" "$d/mpfr"
    [[ -d "$d/mpc"  ]] || mv "$mpc"  "$d/mpc"

    # Sur x86_64, LFS place les bibliothèques 64 bits dans lib, pas lib64.
    sed -e '/m64=/s/lib64/lib/' -i.orig "$d/gcc/config/i386/t-linux64"

    rm -rf "$d/build"; mkdir -p "$d/build"
    log "configure gcc (pass 1)"
    pushd "$d/build" >/dev/null
    # Pass 1 ne produit que le compilateur (cc1/cc1plus) et libgcc. On
    # désactive donc la bibliothèque standard C++ (bâtie séparément en fin de
    # chapitre 5) : GCC en déduit automatiquement l'abandon de libitm et
    # libsanitizer (cf. configure.ac, « Disable libitm, libsanitizer if we're
    # not building libstdc++ »). Sans cela, la compilation échoue faute de libc :
    # « Link tests are not allowed after GCC_NO_EXECUTABLES ».
    run "$d/configure" \
        --target="$LFS_TGT" \
        --prefix="$TOOLS" \
        --with-glibc-version="$GLIBC_VER" \
        --with-sysroot="$LFS" \
        --with-newlib \
        --without-headers \
        --enable-default-pie \
        --enable-default-ssp \
        --disable-nls \
        --disable-shared \
        --disable-multilib \
        --disable-threads \
        --disable-libatomic \
        --disable-libgomp \
        --disable-libquadmath \
        --disable-libssp \
        --disable-libvtv \
        --disable-libstdcxx \
        --enable-languages=c,c++
    log "compilation + installation gcc (pass 1)"
    run make
    run make install
    popd >/dev/null

    local hdr
    hdr="$(dirname "$("$TOOLS/bin/$LFS_TGT-gcc" -print-libgcc-file-name)")"
    log "en-tête limits.h -> $hdr/include/limits.h"
    cat "$d/gcc/limitx.h" "$d/gcc/glimits.h" "$d/gcc/limity.h" > "$hdr/include/limits.h"
}

phase_headers() {
    local d; d="$(src_dir linux)"
    log "en-têtes d'API du noyau (headers_install)"
    run make -C "$d" mrproper
    run make -C "$d" headers_install ARCH=x86_64 INSTALL_HDR_PATH="$LFS/usr"
    find "$LFS/usr/include" \( -name '..install.cmd' -o -name '.install' \) -delete
}

phase_glibc() {
    local d; d="$(src_dir glibc)"
    require_phase gcc1
    require_phase headers
    rm -rf "$d/build"; mkdir -p "$d/build"
    log "configure glibc"
    pushd "$d/build" >/dev/null
    printf 'rootsbindir=/usr/sbin\n' > configparms
    run "$d/configure" \
        --prefix=/usr \
        --host="$LFS_TGT" \
        --build="$(sh "$d/scripts/config.guess")" \
        --enable-kernel=6.1 \
        --with-headers="$LFS/usr/include" \
        --disable-nscd \
        libc_cv_slibdir=/usr/lib
    log "compilation glibc"
    run make
    log "installation glibc"
    run make DESTDIR="$LFS" install
    popd >/dev/null

    [[ -f "$LFS/usr/bin/ldd" ]] && sed '/RTLDLIST=/s@/usr@@g' -i "$LFS/usr/bin/ldd"

    log "vérification : binaire cible lié à l'interpréteur dynamique ?"
    local probe="$WORK/_probe"
    printf 'int main(void){return 0;}\n' > "$probe.c"
    "$LFS_TGT-gcc" "$probe.c" -o "$probe"
    readelf -l "$probe" | grep -q 'ld-linux' \
        || die "l'interpréteur ld-linux est absent du binaire produit"
    rm -f "$probe" "$probe.c"
}

phase_libstdcxx() {
    local d="$WORK/gcc-$GCC_VER"
    require_phase gcc1
    [[ -d "$d/build" ]] || die "arbre de build gcc absent (relancer la phase gcc1)"
    log "configure libstdc++"
    pushd "$d/build" >/dev/null
    # On réutilise l'arbre de build de gcc pass 1 (comme LFS). Les en-têtes C++
    # sont installés dans $LFS/tools/$LFS_TGT/include/c++/$GCC_VER : c'est
    # exactement le dossier C++ par défaut du compilateur croisé (préfixe
    # $TOOLS = $LFS/tools), donc $LFS_TGT-g++ les trouvera ensuite.
    run "$d/libstdc++-v3/configure" \
        --host="$LFS_TGT" \
        --build="$(sh "$d/config.guess")" \
        --prefix=/usr \
        --disable-multilib \
        --disable-nls \
        --disable-libstdcxx-pch \
        --with-gxx-include-dir="/tools/$LFS_TGT/include/c++/$GCC_VER"
    log "compilation + installation libstdc++"
    run make
    run make DESTDIR="$LFS" install
    popd >/dev/null
    rm -fv "$LFS/usr/lib/libstdc++.la" "$LFS/usr/lib/libstdc++exp.la" \
           "$LFS/usr/lib/libstdc++fs.la" "$LFS/usr/lib/libsupc++.la" 2>/dev/null || true
}

# --- Exécution --------------------------------------------------------------

ALL_PHASES=(binutils1 gcc1 headers glibc libstdcxx)

run_phase() {
    local p="$1"
    if stamp_exists "$p"; then
        log "phase déjà faite : $p (ignorée)"
        return 0
    fi
    section "phase : $p"
    "phase_$p"
    stamp_done "$p"
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

log "Toolchain temporaire prête dans $TOOLS"
