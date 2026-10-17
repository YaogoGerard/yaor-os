# Deployment & Operations

**Statut :** Accepté
**Version :** 0.23
**Dernière mise à jour :** 2026-10-15
**Décisions liées :** ADR-001, ADR-003, ADR-004, ADR-007, ADR-009, ADR-011, ADR-012, ADR-013, ADR-014, ADR-015, ADR-016, ADR-017, ADR-018, ADR-019, ADR-020, ADR-021

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
`build/logs/<étape>.log`. Les phases de l'étape 04 laissent un marqueur
(`build/stamps/ch7.<phase>`) : quand une **recette change** entre deux jalons (par
exemple les options de `systemd` ou `util-linux` en v0.2), il faut **supprimer le
marqueur** de la phase concernée, ou repartir d'un `build/` vierge, faute de quoi elle
n'est pas reconstruite.

### Configuration système (arborescence `config/rootfs/`)

La configuration fournie par YAOR vit dans **`config/rootfs/`**, une arborescence qui
**reflète la racine du système** (`config/rootfs/etc/…` → `/etc/…`). Elle est copiée
telle quelle par la **phase de configuration** de l'étape 04, **après** l'installation
des paquets : la configuration de YAOR prime ainsi sur les fichiers par défaut des
paquets. La copie ne conserve **pas** l'appartenance du dépôt (`cp -a
--no-preserve=ownership`) : les fichiers appartiennent à `root`, ce qu'exigent `sudo`
(`/etc/sudoers`) et polkit (règles de `/etc/polkit-1/rules.d`). Cette phase **active
aussi les services** (`systemctl enable`) et pose les liens (`/etc/resolv.conf`). Le
détail figure dans [`config/rootfs/README.md`](../../config/rootfs/README.md).

Le **noyau**, lui, est configuré par `config/kernel-live.fragment` (variables fusionnées
par `merge_config.sh`).

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

### Composants réseau (v0.2)

Le **réseau filaire** s'appuie sur **`systemd-networkd`** et **`systemd-resolved`**, déjà
présents (systemd 262) : **aucune source nouvelle**. Il faut les **activer** à la
construction — systemd est bâti `-Dauto_features=disabled` — via `-Dnetworkd=true` et
`-Dresolve=true`, puis fournir une configuration par défaut
(DHCP filaire et Wi-Fi) et `resolved.conf.d` vivent dans `config/rootfs/etc/systemd/`.

Le **Wi-Fi** (ADR-013, ADR-020) ajoute deux sources, épinglées au manifeste :

- **`iwd`** 3.12 — démon Wi-Fi. Il **embarque `ell`** (construction par défaut, sans
  `--enable-external-ell`) : **aucun paquet `ell` séparé**, conforme au principe de
  non-duplication (ADR-017). Son client **`iwctl`** exige **`readline`** 8.3 (lui-même
  au-dessus de `ncurses`, déjà au lot).
- **`linux-firmware`** 20260916 — microprogrammes, installés **en entier** (Wi-Fi
  et cartes filaires) pour maximiser la prise en charge matérielle. Le tarball est
  monolithique (~662 Mio compressés) ; l'image EROFS de l'étape 06 (zstd) en assure
  la compression, sans quoi l'ISO dépasserait de loin la cible de légèreté.

`iwd` est configuré en **couche 2 seule** (`config/rootfs/etc/iwd/main.conf`,
`EnableNetworkConfiguration=false`) : l'adressage IP, filaire comme Wi-Fi, est assuré par
`systemd-networkd` — un seul fournisseur par besoin.

Le Wi-Fi exige enfin d'**activer le noyau**, que le jalon 0.1 écartait entièrement ;
c'est désormais le cas dans `config/kernel-live.fragment` : `CONFIG_CFG80211=y` avec
**`CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` désactivé** (rendu paramétrable par
`CONFIG_CFG80211_CERTIFICATION_ONUS=y`, ADR-020), `CONFIG_MAC80211`, `CONFIG_FW_LOADER`
et les pilotes sans fil courants (modules chargés par udev).

### Composants sessions (v0.2)

Les sessions et comptes (ADR-014) reposent sur la pile Linux standard :

- **D-Bus** (`dbus` 1.16.2), bus système et utilisateur, sur **`expat`** 2.9.0
  (analyse XML) ; nécessaire à `systemd-logind` et aux services de session.
- **`linux-pam`** 1.7.3 (meson) : modules d'authentification. Les fonctionnalités
  optionnelles sont coupées (`-Daudit=disabled -Dselinux=disabled -Ddocs=disabled
  -Di18n=disabled`) ; `libeconf` reste optionnel (`-Deconf=auto`), aucune source en plus.
- **`shadow`** 4.20.3 : `useradd`/`usermod`/`passwd` et `/etc/shadow` (comptes,
  répertoires personnels).
- **`sudo-rs`** 0.2.15 (Rust, ADR-012) : élévation de privilèges, liée à `libpam` ;
  la source est épinglée au manifeste (les archives de release ne sont que des binaires).

Deux reconfigurations suffisent, sans source nouvelle :

- **systemd** : ajouter `-Dlogind=true -Dpam=enabled` (systemd est bâti
  `-Dauto_features=disabled`).
- **util-linux** : retirer `--disable-login` pour fournir **`agetty`** et **`login`** ;
  `su`/`nologin`/`runuser`/`setpriv` restent désactivés, l'élévation passant par
  `sudo-rs` (un besoin, un fournisseur).

La configuration associée est fournie dans `config/rootfs/etc/` : **chaînes PAM**
(`pam.d/`, dont `pam_systemd` — sans quoi `logind` ne suit pas les sessions,
`REQ-SW-021`), **`sudoers`** (administration par le groupe `wheel`) et **règle polkit**
accordant les actions `udisks2` à `wheel` (suivi d'ADR-014). Le groupe `wheel` et
`/var/lib/iwd` sont créés par la phase de configuration.

### Composants stockage (v0.2)

Le montage automatique des volumes amovibles (ADR-015) s'appuie sur **`udisks2`**
2.11.2, qui tire une partie de l'écosystème GLib :

- **`glib`** 2.88.4 (sur **`libffi`** 3.8.0) et **`libgudev`** 238 ;
- **`polkit`** 124 pour l'autorisation des actions privilégiées, avec **`duktape`**
  2.7.0 comme moteur de règles (`js_engine=duktape`). `os_type=lfs` lui fait inclure
  les chaînes PAM de YAOR (`/usr/lib/pam.d/polkit-1`) ; un **correctif** remplace la
  fusion de la politique (`i18n.merge_file`, type XML) par une copie, écartant
  `itstool` et `libxml2`. Son binaire **`pkexec`** (setuid-root) est **retiré** :
  l'élévation de privilèges reste le seul fait de `sudo-rs` (ADR-012/014) ;
- **`libblockdev`** 3.5.0 (sur **`libbytesize`** 2.12). `udisks2`, bâti
  `--enable-daemon`, appelle sans garde les symboles `bd_md_`, `bd_crypto_`,
  `bd_nvme_`, `bd_smart_`, `bd_fs_`, `bd_part_`, `bd_loop_` et `bd_swap_` : le **jeu
  de greffons est donc imposé**, et non choisi — `part`, `fs`, `loop`, `swap`,
  `mdraid`, `crypto`, `nvme`, `smart` sont **obligatoires** (chacun manquant fait
  échouer le `configure` de `udisks`). Seuls `lvm`, `dm`, `mpath`, `nvdimm` et
  `tools` sont désactivés (`udisks` n'emploie ni `bd_dm_` ni `bd_lvm_` ; `tools`
  tirerait `libparted`). L'introspection GObject y est écartée
  (`--disable-introspection`) : `gobject-introspection` est absent du chroot et la
  détection partielle amont casse la construction (cible `BlockDev-3.0.typelib`
  sans règle).
- **`udisks`** 2.11.2 : démon de montage automatique (ADR-015) ; manuels, gtk-doc,
  introspection et ACL sont écartés (`--disable-man --disable-gtk-doc
  --disable-introspection --disable-acl`). `udisks2.service` est de type **`dbus`**
  et s'accompagne d'un fichier d'activation D-Bus : le démon est lancé **à la
  demande**, sans activation au démarrage (sa section `[Install]` ne vise que
  `graphical.target`, jamais atteint par la racine live).
- **`gettext`** 1.0 : seuls ses outils (`msgfmt`, `xgettext`, `msgmerge`) sont utiles,
  mais `polkit` (`i18n.gettext`), `libbytesize`, `udisks` et `cryptsetup` les réclament
  **en dur** au configure. Les traductions restent désactivées (`--disable-nls`) partout
  ailleurs.

`libblockdev` et `udisks` sont les seuls paquets Autotools du lot sans `configure`
dans l'arbre Git. Ils sont donc pris dans leurs **archives de publication**
(`releases/download/…`, issues de `make dist`), qui **embarquent un `configure`
pré-généré** et les macros `m4` requises (`ax_check_enable_debug`, `ax_recursive_eval`,
`gtk-doc.m4`, et les macros gettext) : la toolchain n'a ainsi **pas** besoin
d'`autoconf`/`automake`/`libtool`/`autoconf-archive`.

Le greffon `crypto` de `libblockdev` (et le build `--enable-daemon` d'`udisks2`)
entraîne une **chaîne de dépendances cryptographiques** supplémentaire, indépendante du
périmètre LUKS (hors v0.2) :

- **`openssl`** 3.6.5 : backend crypto de `libcryptsetup` (`--with-crypto_backend=openssl`),
  et — voir ADR-021 — brique TLS/signature de `systemd-sysupdate` et
  `systemd-importd` (mode installé). Il est donc bâti **tôt**, avant systemd ;
- **`json-c`** 0.19 et **`popt`** 1.19 : exigés **en dur** par le `configure` de
  `libcryptsetup` (`PKG_CHECK_MODULES([JSON_C], [json-c])`, `popt`) ; `json-c` sert
  aussi à `libnvme` ;
- **`LVM2`** 2.03.43 : seul **`libdevmapper`** est construit et installé ; `libcryptsetup`
  en fait une dépendance dure (`AC_CHECK_HEADER(libdevmapper.h)`) ;
- **`libcryptsetup`** 2.8.8 : construit en **bibliothèque seule** (les outils
  `cryptsetup`/`veritysetup`/`integritysetup` restent désactivés, LUKS étant hors v0.2) ;
- **`keyutils`** 1.6.3 : le greffon `crypto` inclut `<keyutils.h>` et lie `-lkeyutils`
  sans garde ; les aides *upcall* (`request-key`, `key.dns_resolver`) sont écartées ;
- **`libnvme`** 1.16.2 (greffon `nvme`) et **`libatasmart`** 0.19 (greffon `smart`).

Les systèmes de fichiers exigent leurs outils :

- **`e2fsprogs`** 1.47.4 (ext4 ; fournit aussi `libext2fs` requise par le greffon `fs`),
  **`btrfs-progs`** 7.1, **`dosfstools`** 4.2 (vfat) et **`exfatprogs`** 1.4.3 (exFAT).
- **NTFS** est assuré par le **noyau `ntfs3`** (lecture/écriture) : `ntfs-3g` n'est
  **pas** embarqué (aucun besoin de formatage en v0.2 ; un besoin, un fournisseur).

Côté noyau, `config/kernel-live.fragment` active `CONFIG_EXT4_FS`, `CONFIG_BTRFS_FS`,
`CONFIG_VFAT_FS` (avec `CONFIG_NLS_CODEPAGE_437`/`CONFIG_NLS_ISO8859_1`), `CONFIG_EXFAT_FS`
et `CONFIG_NTFS3_FS` (le NTFS historique `CONFIG_NTFS_FS` reste désactivé). Le greffon `fs`
de `libblockdev` exige que les outils `mkfs`/`fsck` correspondants soient présents au runtime.

### Mode installé et mises à jour (v0.2)

La conception détaillée (installateur, schéma GPT, transferts `systemd-sysupdate`, machine
à états A/B et retour arrière avec Limine) est décrite dans
[`../04-design-lld/installer/installer.md`](../04-design-lld/installer/installer.md).

Sans **source nouvelle** pour l'installateur et la sélection de slot — `yaor-install` et
`yaor-slot` sont des composants **internes en Rust** — le mode installé ajoute toutefois
**deux sources** au socle (ADR-021), exigées par les fonctions de systemd mobilisées :

- **`openssl`** 3.6.5 — requis par `systemd-sysupdate` (transport TLS et vérification de
  signature), en plus d'être le backend crypto de `libcryptsetup` ;
- **`curl`** 8.23.0 — fournit **`libcurl`**, exigé par **`systemd-importd`** dont dépend
  `systemd-sysupdate` (source « url-file »).

Ces deux sources, ainsi que `zlib` et `liblzma` (déjà présents), sont bâties **avant**
systemd. systemd 262 est ensuite bâti avec **`-Dimportd=enabled -Dsysupdate=enabled
-Drepart=enabled`** (dont les dépendances de compilation sont respectivement
`libcurl` + `openssl` + `zlib` + `liblzma`, plus `libfdisk` pour les deux derniers) et
**`-Dlibcurl=enabled -Dopenssl=enabled -Dzlib=enabled -Dxz=enabled -Dfdisk=enabled`** ;
`libfdisk` est fourni par `util-linux` (`fdisk.pc`). `libcryptsetup` reste coupé : LUKS est
hors v0.2 (ADR-015) et `systemd-repart` n'en a pas besoin pour le partitionnement de base.

La configuration associée (`/etc/repart.d/*.conf`, `/etc/sysupdate.d/*.transfer`, unités de
`yaor-slot`) est fournie dans `config/rootfs/`, voir le LLD.

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

### Correctif appliqué à polkit 124

`actions/meson.build` publie la politique de polkit via `i18n.merge_file()`. Sans
`type:` explicite, meson retient le type `xml` : il invoque alors `itstool`, qui
réclame le module Python `libxml2` (liaisons de libxml2). Or polkit n'utilise que
`expat` ; tirer `libxml2` et ses liaisons Python pour fusionner un unique fichier
de politique — qui ne porte aucun repère de traduction — contredirait l'objectif
de légèreté. Le correctif
`patches/polkit-124-merge-file-sans-itstool.patch` remplace `i18n.merge_file()`
par `configure_file(copy: true)`, au résultat identique (le `.policy.in` ne
contient aucune balise ITS). Les catalogues de polkit (`po/`, via
`i18n.gettext`) continuent d'être compilés par gettext.

Au-delà du correctif, polkit installe `pkexec`, binaire **setuid-root** qui exécute
une commande sous une autre identité après autorisation polkit ; c'est une seconde
voie d'élévation de privilèges. ADR-012/014 font de `sudo-rs` le **seul** fournisseur
d'élévation : `phase_polkit` supprime `/usr/bin/pkexec` après installation.
`polkit-agent-helper-1` (setuid lui aussi) est conservé : il n'assure que la
conversation PAM d'authentification polkit, pas l'exécution d'une commande arbitraire.

### Correctif appliqué à json-c 0.19

`json-c` 0.19 est livré avec un `meson.build` (les versions antérieures ne
connaissaient que CMake). Ce script compile bien la bibliothèque mais **n'installe
pas une arborescence exploitable** : `install_headers()` dépose les en-têtes à plat
dans `/usr/include` (alors que tout le monde les inclut sous `<json-c/json.h>`, comme
le fait le CMake amont) et **aucun `json-c.pc` n'est généré** — or `cryptsetup` exige
ce fichier pkg-config, et `libnvme` s'en sert. Le correctif
`patches/json-c-0.19-meson-pc.patch` installe les en-têtes sous `<json-c/>`, génère
`json-c.pc` (mêmes drapeaux que le `json-c.pc.in` amont) et rétablit au passage la
version du projet, restée à `0.18.99` dans le fichier meson.

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
  `--disable-nls` (les traductions restent écartées).
- **zstd** est requis au moment de compiler le noyau : `CONFIG_KERNEL_ZSTD`
  fait compresser `arch/x86/boot/compressed/vmlinux.bin` par l'outil `zstd`,
  absent du lot. Il est donc construit juste avant le noyau (phase `zstd`,
  groupe `base` du manifeste). L'image du noyau comprimée en zstd est plus
  petite que la sortie gzip par défaut et se décompresse plus vite au démarrage.
- `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` (actif par défaut) sélectionne
  `SYSTEM_TRUSTED_KEYRING`, ce qui ferait construire les certificats du noyau
  avec **OpenSSL**. Depuis ADR-020 il est **désactivé** — son invite n'existe que
  si `CONFIG_CFG80211_CERTIFICATION_ONUS=y`, activé précisément pour cela — et le
  Wi-Fi (`CFG80211`, `MAC80211`, pilotes) est donc construit **sans qu'OpenSSL
  soit requis par le noyau** (OpenSSL est désormais présent côté espace
  utilisateur pour le mode installé, ADR-021, ce qui est indépendant) ; le réseau
  filaire (virtio-net) reste disponible.
- `CONFIG_WERROR` (avertissements = erreurs) est désactivé : un compilateur plus
  récent que celui des développeurs du noyau peut introduire de nouveaux
  avertissements, ce qui casserait la compilation — recommandation explicite de
  LFS.
- **kmod** 34.2 expose ses compressions et la signature OpenSSL en options
  « feature » (actives par défaut) : `zstd`, `xz` et `openssl` sont
  **explicitement désactivés**. Les modules du noyau sont livrés **non
  compressés** (défaut `CONFIG_MODULE_COMPRESS_NONE`) et la signature de modules
  est coupée (`CONFIG_MODULE_SIG=n`, ADR-020) : kmod n'a donc besoin d'aucun
  backend de décompression ni de `libcrypto`, même si `liblzma` et OpenSSL sont
  désormais présents dans le socle v0.2 (ADR-021). `zlib` reste actif et les
  manpages sont désactivées (`scdoc` absent).
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
  absente du lot (selinux, seccomp, `bootloader`, etc.) ; `blkid`, `kmod`,
  `pcre2` et **`libmount`** sont explicitement activées.
  `libmount` est une *fonctionnalité* touchée par `auto_features=disabled` ; sans
  elle, le montage des systèmes de fichiers d'API (`/dev`, `/run`, `cgroup2`…)
  échoue au démarrage et systemd s'arrête sur « Failed to mount API
  filesystems ». En complément, `-Dtranslations=false` évite `msgfmt` (gettext),
  `-Dman=false` la documentation, `-Dhwdb=false` la base matérielle et
  `-Dtests=false` les tests.
  **En v0.2**, ces fonctions sont activées : `-Dnetworkd=true -Dresolve=true`
  (réseau), `-Dlogind=true -Dpam=enabled` (sessions) et, pour le mode installé
  (ADR-015, ADR-021), `-Dimportd=enabled -Dsysupdate=enabled -Drepart=enabled`
  avec `-Dlibcurl=enabled -Dopenssl=enabled -Dzlib=enabled -Dxz=enabled
  -Dfdisk=enabled`.
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
  `version`, `url`, `sha256`, `fichier`). Seul ce fichier est versionné ; les archives
  elles-mêmes sont ignorées par Git.
- Les dépendances cargo des composants Rust ne sont pas versionnées : elles sont
  **vendues** au téléchargement (étape 01, voir « Chaîne Rust » et ADR-018/ADR-019).
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
