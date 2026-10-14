# Deployment & Operations

**Statut :** Accepté
**Version :** 0.9
**Dernière mise à jour :** 2026-10-14
**Décisions liées :** ADR-001, ADR-003, ADR-004, ADR-007, ADR-009, ADR-010

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
scripts/05_initramfs.sh    -> initramfs systemd
scripts/06_make_iso.sh     -> EROFS + Limine (BIOS/UEFI) -> ISO signée
```

La construction suit les grandes étapes de LFS : la **toolchain croisée** (étape 02)
puis les **outils temporaires** (étape 03) sont bâtis hors `chroot` par l'utilisateur ;
seule l'étape 04, qui bascule dans la racine cible (`chroot`), requiert les privilèges
root. Une fois le système final construit, la toolchain et les outils temporaires ne sont
plus nécessaires.

Chaque étape est idempotente et peut être relancée seule. Les journaux sont écrits dans
`build/logs/<étape>.log`.

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

Un défaut de gawk 5.4.1 construit **sans MPFR** (le cas de la cible) fait qu'un
élément de tableau non initialisé, après une comparaison à la chaîne vide, se
reformate en `"0"` au lieu de rester vide. En cause : dans la disposition de la
structure interne `NODE` (sans MPFR), `sub.nodep.name` (`vname`) et
`sub.val.idx` (`stfmt`) partagent le même décalage ; `elem_new_reset()`, qui
convertit un `Node_elem_new` en scalaire, écrasait `stfmt` (`STFMT_UNUSED`), ce
qui forçait un reformatage depuis la valeur numérique 0. La construction avec
MPFR agrandit la structure et masque le recouvrement.

GCC en dépend directement : ses scripts `optc-gen.awk`/`opth-gen.awk`
(`if (enables[...] == "")` suivi d'une concaténation) produisaient alors des
`options.cc`/`options.h` parsemés de `#error ... does not have a Var() flag`,
faisant échouer la compilation finale de gcc (étape 04). Le correctif
`patches/gawk-5.4.1-elem-new-stfmt.patch` supprime l'écriture fautive ;
l'étape 03 l'applique dans `phase_gawk`.

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

### Contraintes du système de base (étape 04)

Le défconfig x86_64 active plusieurs fonctions qui tirent des dépendances de
compilation ; on les traite explicitement pour garder le lot minimal :

- `CONFIG_OBJTOOL` (tiré par le dérouleur ORC, l'IBT et les thunks de retour)
  se lie à **libelf** : `libelf` (Elfutils 0.193) est donc construit juste avant
  le noyau (cf. LFS 12.4, section 8.49).
- `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` (actif par défaut, sans invite de
  configuration) sélectionne `SYSTEM_TRUSTED_KEYRING`, ce qui ferait construire
  les certificats du noyau avec **OpenSSL**. OpenSSL étant écarté du lot
  minimal, tout le WiFi (`CFG80211`) est désactivé au jalon 0.1 ; le réseau
  filaire (virtio-net) reste disponible. Le WiFi reviendra avec OpenSSL.
- `CONFIG_WERROR` (avertissements = erreurs) est désactivé : un compilateur plus
  récent que celui des développeurs du noyau peut introduire de nouveaux
  avertissements, ce qui casserait la compilation — recommandation explicite de
  LFS.

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

- Modèle **image immuable** : mise à jour par remplacement d'image (A/B ou
  réinstallation), pas de gestionnaire de paquets classique (ADR-003).

## CI (ultérieure)

Une fois le pipeline local stabilisé, la CI devrait vérifier le formatage, exécuter les
tests, construire l'image, tester le boot (BIOS + UEFI) et conserver les artefacts.
