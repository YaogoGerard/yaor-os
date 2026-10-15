# Deployment & Operations

**Statut :** Accepté
**Version :** 0.17
**Dernière mise à jour :** 2026-10-14
**Décisions liées :** ADR-001, ADR-003, ADR-004, ADR-007, ADR-009, ADR-011, ADR-012, ADR-013, ADR-014, ADR-015, ADR-016, ADR-017, ADR-018, ADR-019

## Modèle de construction

YAOR est construit **depuis les sources** (ADR-001) : aucun paquet binaire d'une
distribution tierce n'est utilisé pour le système de base.

```text
Sources (versions + empreintes épinglées)
  -> Toolchain
  -> Root filesystem
  -> Linux kernel (systemd, ADR-004)
  -> Initramfs
  -> Filesystem image (immuable, ADR-003)
  -> ISO (signée, ADR-007)
```

## Pipeline (scripts)

```text
scripts/00_check_host.sh   -> vérifie les outils hôte
scripts/01_download.sh     -> télécharge et vérifie les sources (SHA-256)
scripts/02_toolchain.sh    -> toolchain croisée temporaire (chapitre 5, sans root)
scripts/03_temp_tools.sh   -> outils temporaires cross-compilés (chapitre 6, sans root)
scripts/04_chroot_build.sh -> chroot : système de base + systemd + noyau (root)
scripts/05_initramfs.sh    -> initramfs shell (script POSIX, ADR-011)
scripts/06_make_iso.sh     -> EROFS + Limine (BIOS/UEFI) -> ISO (empreinte SHA-256 ; signature ADR-007)
```

L'ensemble s'enchaîne avec `scripts/build.sh`, qui exécute ces étapes dans
l'ordre (en élevant les droits pour 04 et 06) et s'arrête à la première erreur.

La construction suit les grandes étapes de LFS : la **toolchain croisée** (étape 02)
puis les **outils temporaires** (étape 03) sont bâtis hors `chroot` par l'utilisateur ;
seule l'étape 04, qui bascule dans la racine cible (`chroot`), requiert les privilèges
root. Une fois le système final construit, la toolchain et les outils temporaires ne sont
plus nécessaires.

Chaque étape est idempotente et peut être relancée seule. Les journaux sont écrits dans
`build/logs/<étape>.log`.

### Chaîne Rust et composants Rust

La toolchain Rust est un **outil de build**, non embarqué : le `rustc`/`cargo` de l'hôte
(prérequis vérifié à l'étape 00) est monté **en lecture seule** dans le chroot (ADR-019).
Les composants Rust retenus (`uutils/coreutils`, `uutils/findutils`, `sudo-rs`) y sont
compilés avec `cargo`, à partir de dépendances **vendues au téléchargement**
(`cargo vendor` sur les `Cargo.lock` épinglés) et en **hors-ligne** (`CARGO_NET_OFFLINE`,
ADR-018). Ils **remplacent** leurs homologues GNU dans l'image finale : un besoin n'a
**qu'un seul fournisseur**, conformément à ADR-017 ; les binaires GNU remplacés sont
écrasés à l'installation, dans le chroot, après les compilations C.

### Chaîne de démarrage live

L'image racine est une **EROFS** compressée en zstd et l'image du noyau est
compressée en zstd (`CONFIG_KERNEL_ZSTD`) ; l'ISO est hybride (El Torito en
BIOS + UEFI) et amorcée par **Limine** (ADR-011). L'initramfs est un **script POSIX**
minimal (`/init`) : il monte `proc`/`sysfs`/`devtmpfs`, localise le média live par son
étiquette (`YAOR-LIVE`, repli `/dev/sr0`), monte `rootfs.erofs` en lecture seule, empile
un `overlayfs` dont la couche supérieure est un `tmpfs` (session live inscriptible),
puis `switch_root` vers le systemd du système réel (ADR-004). Les modules `erofs`,
`overlayfs`, `loop`, `iso9660`, `sr` et `virtio` sont compilés **en dur** dans le noyau :
l'initramfs n'a aucun module à charger.

L'image racine livrée est allégée au staging (étape 06) : l'image live est un
**système minimal amorçable**, pas un environnement de développement. Les outils
de compilation (gcc, binutils, python3, perl, bison, flex, `file`) et les copies
de `/boot` — déjà portées par l'ISO — en sont écartés, ce qui retire l'essentiel
du volume. Ni systemd, ni glibc, ni les bibliothèques de base n'en dépendent au
démarrage.

### Système installé, réseau et mises à jour

Le mode **installé** (ADR-015) dépose le système sur disque et configure **Limine** en
BIOS et UEFI ; les mises à jour se font par **image**, avec deux slots A/B et retour
arrière, via **`systemd-sysupdate`**. Les systèmes de fichiers supportés sont `ext4`
(racine par défaut), `btrfs`, `vfat`, `exFAT` et `NTFS` (rw) ; le chiffrement LUKS est
reporté.

Le réseau est assuré par **`systemd-networkd`** et **`systemd-resolved`** (filaire) et par
**`iwd`** avec `linux-firmware` (Wi-Fi), voir ADR-013.

### Adaptations de compilation croisée

Certains paquets récents embarquent une gnulib dont le script `configure` **échoue**
sous compilation croisée au lieu de retomber sur sa valeur devinée, pour des tests
d'exécution (`AC_RUN_IFELSE`). LFS les contourne via des variables de cache. On applique
la même méthode, en documentant chaque cas :

- `diffutils` 3.12 : `gl_cv_func_strcasecmp_works="guessing yes"` (test « whether
  strcasecmp works »).

Tout cas de ce type doit être signalé et documenté ici au fur et à mesure.

### Amorçage exigé par glibc 2.44

Depuis la version 2.44, le `configure` de glibc refuse de poursuivre si `bison`
(≥ 2.7) ou `python` (≥ 3.4) sont absents, et sa compilation les emploie réellement :

- `intl/plural.c` n'est plus livré : il est régénéré par **bison** depuis
  `intl/plural.y` (`intl/Makefile`) ;
- `locale/C-translit.h` est produit par **python** via `locale/gen-translit.py`.

La toolchain temporaire de l'étape 03 n'inclut ni l'un ni l'autre — comme le
chapitre 6 de LFS, conçu pour glibc 2.42. L'étape 04 construit donc, **dans le
chroot et avant glibc**, `bison`, `zlib` puis `python3` — zlib devant précéder
python3 pour que celui-ci construise son module `_zlib`, exigé par meson
(`import gzip`). Ces paquets s'appuient sur la glibc temporaire de l'étape 02 ;
l'ABI étant identique, ils restent valides après l'installation de la glibc
finale.

Le script 03 devait également être corrigé : `binutils` (pass 2) ne doit **pas**
être configuré `--enable-shared`, sous peine de produire un `ld` lié dynamiquement
à `libbfd`/`libz.so.1` — bibliothèque absente de la cible avant glibc, ce qui
casserait toute édition de liens dans le chroot.

### Correctif appliqué à gawk 5.4.1

Un seul défaut d'origine, mais **deux** symptômes opposés. Dans la disposition de
la structure interne `NODE` compilée **sans MPFR** (le cas de la cible),
`sub.nodep.name` (alias de `vname`) et `sub.val.idx` (alias de `stfmt`) tombent
au même décalage ; les deux champs se recouvrent alors qu'ils servent à des
nœuds différents. La construction avec MPFR agrandit `val` (`mpfr_t`) et
supprime ce recouvrement — c'est la configuration que l'amont teste.

- `elem_new_reset()`, qui convertit un `Node_elem_new` non initialisé en
  scalaire, écrivait `n->vname = NULL` : cela écrasait `stfmt`
  (`STFMT_UNUSED` devenait 0) et une chaîne vide se reformatait en `"0"`. GCC en
  dépendait directement : ses scripts `optc-gen.awk`/`opth-gen.awk` produisaient
  des `options.cc`/`options.h` parsemés de `#error ... does not have a Var()
  flag`, faisant échouer la compilation finale de gcc (étape 04).
- À l'inverse, ne pas écrire ce `vname = NULL` laisse `stfmt = STFMT_UNUSED`
  (`0xffffffff`) dans l'emplacement de `vname`. La promotion d'un
  `Node_elem_new` en variable (`r_get_lhs`, `adjust_param_node`,
  `split`/`patsplit`, API d'extension) donne alors à `vname` un pointeur
  invalide ; `r_unref()` / `efree(vname)` provoque `free(): invalid pointer`
  (SIGABRT). Observé sur la génération, par le noyau Linux, de
  `tools/objtool/arch/x86/lib/inat-tables.c`.

Aucune valeur unique ne peut convenir aux deux usages tant que les champs se
recouvrent. Le correctif `patches/gawk-5.4.1-vname-out-of-union.patch` **sort
`vname` de l'union** : il devient un champ de premier niveau, commun à plusieurs
types de nœuds, sans plus recouvrir `stfmt`. C'est la correction retenue par
l'amont (git de gawk). Les deux bogues disparaissent simultanément et
`elem_new_reset()` conserve son `n->vname = NULL` d'origine. L'étape 03
l'applique dans `phase_gawk` ; le correctif a été vérifié par la suite de tests
amont (`make check`, « ALL TESTS PASSED ») et par comparaison, octet pour octet,
de `options.cc` avec celui produit par un gawk lié à MPFR.

### Correctif appliqué à pkgconf 3.0.8

Le tarball de release de pkgconf 3.0.8 omet `tests/api/test-util.c`, pourtant
référencé par son propre `meson.build` : `meson setup` échoue alors sur
« File tests/api/test-util.c does not exist », avant toute compilation. Le
fichier existe dans le dépôt Git au tag `pkgconf-3.0.8` ; le correctif
`patches/pkgconf-3.0.8-missing-test-util.patch` le recrée à l'identique. Le test
étant déclaré `build_by_default : false`, il n'est jamais compilé par la
construction normale.

### Correctif appliqué à libxcrypt 4.5.2

GCC 16 compile par défaut en C23 (`-std=gnu23`, `__STDC_VERSION__ == 202311L`),
ce qui active la fonction `strchr` générique de constance livrée par la glibc
2.44 : pour un argument `const char *`, `strchr` renvoie désormais un
`const char *`. libxcrypt passait `(const char *) intbuf->retval` (un
`uint8_t *` pourtant mutable) puis affectait le résultat à un `char *`, ce qui
déclenche `-Wdiscarded-qualifiers` — fatal car libxcrypt active `-Werror`. Le
correctif `patches/libxcrypt-4.5.2-c23-strchr-const.patch` retire le `const` du
cast (correction identique à celle de l'amont, branche `develop`).

### Utilisateurs et groupes du système de base (étape 04)

L'étape 04 installe dans le chroot les fichiers `/etc/passwd` et `/etc/group`,
couvrant les utilisateurs et groupes standard (dont `tty`, `utmp`, `input`,
`kvm`, `render`, `audio`, `video`…). Plusieurs `make install` s'y appuient :
util-linux termine notamment par `chgrp tty /usr/bin/wall`, qui échoue si le
groupe `tty` est absent.

### Contraintes du système de base (étape 04)

Le défconfig x86_64 active plusieurs fonctions qui tirent des dépendances de
compilation ; on les traite explicitement pour garder le lot minimal :

- `CONFIG_OBJTOOL` (tiré par le dérouleur ORC, l'IBT et les thunks de retour)
  se lie à **libelf** : `libelf` (Elfutils 0.193) est donc construit juste avant
  le noyau (cf. LFS 12.4, section 8.49). Comme seul `libelf` est installé, sa
  compilation est restreinte aux sous-répertoires `config`, `lib` et `libelf` :
  `libcpu` (non installé) échoue en effet sur `bsearch`, que glibc 2.44 rend
  « const-générique » en C23 — son retour devient `const void *` quand la base
  est `const` — sous `-Werror`.
- **flex** est requis par le `kconfig` du noyau : les fichiers générés
  `scripts/kconfig/lexer.lex.c` et `parser.tab.c` ne sont pas livrés, le noyau
  les régénère depuis `lexer.l`/`parser.y` (`LEX = flex`). L'archive de release
  fournit ses propres sources déjà analysées (`src/scan.c`, `src/parse.c`) : flex
  s'amorce donc sans flex ni bison préalables, et il est configuré avec
  `--disable-nls` (gettext absent du lot).
- **zstd** est requis au moment de compiler le noyau : `CONFIG_KERNEL_ZSTD`
  fait compresser `arch/x86/boot/compressed/vmlinux.bin` par l'outil `zstd`,
  absent du lot. Il est donc construit juste avant le noyau (phase `zstd`,
  groupe `base` du manifeste). L'image du noyau comprimée en zstd est plus
  petite que la sortie gzip par défaut et se décompresse plus vite au démarrage.
- `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` (actif par défaut, sans invite de
  configuration) sélectionne `SYSTEM_TRUSTED_KEYRING`, ce qui ferait construire
  les certificats du noyau avec **OpenSSL**. OpenSSL étant écarté du lot
  minimal, tout le WiFi (`CFG80211`) est désactivé au jalon 0.1 ; le réseau
  filaire (virtio-net) reste disponible. Le WiFi reviendra avec OpenSSL.
- `CONFIG_WERROR` (avertissements = erreurs) est désactivé : un compilateur plus
  récent que celui des développeurs du noyau peut introduire de nouveaux
  avertissements, ce qui casserait la compilation — recommandation explicite de
  LFS.
- **kmod** 34.2 expose ses compressions et la signature OpenSSL en options
  « feature » (actives par défaut) : `zstd` et `xz` sont désactivés faute de
  `libzstd` et `liblzma`, `openssl` faute de `libcrypto` (OpenSSL écarté) ;
  `zlib` reste actif et les manpages sont désactivées (`scdoc` absent). Seuls
  les modules gzip et non compressés sont donc gérés au jalon 0.1.
- **gperf** est requis sans condition par systemd 262 (détermination de
  `GPERF_LEN_TYPE` et génération des tables `.gperf` de udev, networkd, etc.) :
  il est donc ajouté au lot, construit juste avant systemd.
- **Jinja2** (et sa dépendance d'import **MarkupSafe**) est requis sans
  condition par systemd 262, qui rend ses gabarits `.pc` et divers en-têtes avec
  `find_installation('python3', modules : ['jinja2'])`. Ces deux paquets Python
  purs sont installés par copie dans `site-packages`, comme meson, sans `pip`
  ni backend PEP 517 ; l'extension C optionnelle de MarkupSafe (`_speedups`)
  n'est pas compilée, son repli `_native` suffisant.
- **systemd** 262 est configuré en « minimal » : `-Dauto_features=disabled`
  coupe toutes les fonctions optionnelles (`auto`) dont la dépendance est
  absente du lot (selinux, seccomp, openssl, pam, networkd, `bootloader`, etc.) ;
  seules `blkid`, `kmod`, `pcre2` et **`libmount`** sont explicitement activées.
  `libmount` est une *fonctionnalité* touchée par `auto_features=disabled` ; sans
  elle, le montage des systèmes de fichiers d'API (`/dev`, `/run`, `cgroup2`…)
  échoue au démarrage et systemd s'arrête sur « Failed to mount API
  filesystems ». En complément, `-Dtranslations=false` évite `msgfmt` (gettext),
  `-Dman=false` la documentation, `-Dhwdb=false` la base matérielle et
  `-Dtests=false` les tests.
- **D-Bus** n'est pas encore construit : `systemd-logind` ne peut donc pas
  s'attacher au bus système et échoue en boucle. Comme une console root unique
  ne requiert pas de gestion de sessions, l'unité est **masquée**
  (`systemd-logind.service` → `/dev/null`) au jalon 0.1 ; elle sera réactivée
  avec D-Bus. De même,
  le lot minimal ne construit ni `agetty` ni `login` (util-linux configuré
  `--disable-login`) : `getty`/`serial-getty` lancent directement `bash -l`
  (console locale `tty1` et console série `ttyS0`, utile en QEMU `-nographic`).

## Répartition des répertoires de construction

```text
build/rootfs/         racine du système cible (deviendra /)
build/rootfs/tools/   toolchain temporaire (deviendra /tools) — cf. ci-dessous
build/work/           sources extraites
build/stamps/         marqueurs d'idempotence par étape
build/logs/           journaux de build
out/                  artefacts produits (image, ISO)
```

La toolchain temporaire (chapitre 5) est installée **dans** la racine cible
(`build/rootfs/tools`, soit `/tools` vu depuis le système cible), et non à côté.
Ce n'est pas un détail cosmétique : le compilateur croisé est configuré avec
`build/rootfs` comme *sysroot*, et il retrouve ses en-têtes C++ à un chemin
calculé à partir de son préfixe d'installation. Placer le préfixe dans le
sysroot (comme LFS) rend ce chemin cohérent et permet de bâtir ensuite le
système cible par `chroot` sans montage supplémentaire. La toolchain temporaire
sera supprimée (`/tools`) une fois le système final bâti.

## Environnement de développement

- Cible de test initiale : **QEMU** (BIOS et UEFI).
- Le pipeline est d'abord exécutable **en local** (scripts) ; la CI vient ensuite
  (décision 26, ADR-000).

## Reproductibilité

- Les versions et les sources utilisées DOIVENT être explicitement définies.
- Chaque source est vérifiée par empreinte **SHA-256** et les empreintes sont épinglées
  (ADR-007).
- Le fichier `sources/manifest.tsv` liste les sources (colonnes `groupe`, `nom`,
  `version`, `url`, `sha256`). Seul ce fichier est versionné ; les archives elles-mêmes
  sont ignorées par Git.
- Le téléchargement utilise les URL canoniques des amonts (upstream). Un miroir GNU peut être fourni
  pour accélérer sans changer les empreintes :
  `GNU_MIRROR=https://mirrors.kernel.org/gnu ./scripts/01_download.sh`.

## Sécurité de l'approvisionnement

- Vérification par empreinte de toutes les sources avant usage.
- Aucun binaire non vérifié n'entre dans le build.
- L'image ISO finale est **signée** (ADR-007).

## Mises à jour

- Modèle **image immuable** : mise à jour par remplacement d'image, pas de gestionnaire
  de paquets classique (ADR-003).
- Mécanisme : **`systemd-sysupdate`**, deux slots A/B et retour arrière (ADR-015,
  REQ-SW-020).

## CI (ultérieure)

Une fois le pipeline local stabilisé, la CI devrait vérifier le formatage, exécuter les
tests, construire l'image, tester le boot (BIOS + UEFI) et conserver les artefacts.
