# ADR-021 — OpenSSL et curl dans le socle v0.2 (mode installé)

**Statut :** Accepté
**Date :** 2026-10-17
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-002, ADR-004, ADR-007, ADR-009, ADR-013, ADR-015, ADR-020

## Contexte

ADR-015 retient **`systemd-sysupdate`** (slots A/B, retour arrière) pour le mode **installé**
du jalon v0.2, avec **`systemd-repart`** pour le partitionnement idempotent. Or ces deux
programmes ne sont pas construits par défaut par un systemd configuré
`-Dauto_features=disabled`, et leurs dépendances de compilation sont impératives.

Vérifié dans les sources de **systemd 262** (`meson.build`, options `feature`) :

- `ENABLE_REPART` exige **`HAVE_LIBFDISK`** ;
- `ENABLE_IMPORTD` exige **`HAVE_LIBCURL`**, **`HAVE_OPENSSL`**, **`HAVE_ZLIB`** et
  **`HAVE_XZ`** ;
- `ENABLE_SYSUPDATE` exige **`ENABLE_IMPORTD`**, **`HAVE_OPENSSL`** et **`HAVE_LIBFDISK`**.

La source « url-file » de `systemd-sysupdate` délègue le téléchargement à `systemd-importd`
(`systemd-pull`), d'où l'exigence de `libcurl` et d'OpenSSL pour le transport TLS. La
**signature** des images de mise à jour (`Verify=signature`, ADR-007) est également assurée
par OpenSSL.

ADR-020 avait volontairement tenu **OpenSSL hors du lot v0.2** (le Wi-Fi fonctionnant sans
registre réglementaire signé, `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB=n`) tout en notant, dans
son suivi, qu'il faudrait l'y introduire « lorsqu'un besoin avéré le justifiera — au premier
chef la **signature des images / `systemd-sysupdate`** ». ADR-013 laissait explicitement ce
point « à trancher lors de l'implémentation ». Ce besoin est désormais avéré : sans OpenSSL,
le mode installé et ses mises à jour (ADR-015) sont impossibles.

## Options envisagées

- **Se passer de `systemd-sysupdate`** (réinstallation, script maison) — contraire à ADR-015
  et à `REQ-SW-020` ; réinvente ce que systemd fournit (ADR-009).
- **Embarquer un client TLS et une bibliothèque crypto alternatifs** (par ex. `libtls`,
  `mbedTLS`) — écarte le socle requis par systemd lui-même ; aucune bibliothèque n'est
  proposée par systemd en remplacement d'OpenSSL. Écarté.
- **Introduire **OpenSSL** (et **curl** pour `libcurl`)** — socle minimal imposé par systemd
  pour `importd`/`sysupdate`/`repart`, cohérent avec la préférence « réutiliser l'existant »
  (ADR-002, ADR-009). **Retenu.**

## Décision

- **OpenSSL 3.6.5** entre dans le socle v0.2. Il est bâti **tôt** (avant systemd) puisqu'il
  est exigé par `systemd-sysupdate`/`systemd-importd` (TLS + signature) en plus de
  `libcryptsetup`.
- **curl 8.23.0** entre dans le socle v0.2 pour fournir **`libcurl`** (transport de
  `systemd-importd`). Il est bâti sur OpenSSL et zlib, avec les protocoles et backends
  inutiles écartés (LDAP, brotli, zstd, nghttp2, idn2, rtmp, ssh2).
- **systemd est bâti avec** `-Dimportd=enabled -Dsysupdate=enabled -Drepart=enabled`
  ainsi que `-Dlibcurl=enabled -Dopenssl=enabled -Dzlib=enabled -Dxz=enabled
  -Dfdisk=enabled` (les fonctionnalités optionnelles étant coupées par
  `-Dauto_features=disabled`).
- **`libcryptsetup` reste coupé** : le chiffrement LUKS est hors v0.2 (ADR-015) et
  `systemd-repart` n'en a pas besoin pour le partitionnement de base.
- La clause d'ADR-020 « **OpenSSL reste hors du lot v0.2** » est **remplacée** par le
  présent ADR. Le reste d'ADR-020 — Wi-Fi sans registre réglementaire signé
  (`CONFIG_CFG80211_REQUIRE_SIGNED_REGDB=n`) et `CONFIG_MODULE_SIG=n` — **demeure en vigueur**.

## Conséquences

- **Positives :** `systemd-sysupdate`, `systemd-repart` et `systemd-importd` disponibles, ce
  qui rend le mode installé (ADR-015) réalisable ; socle de **signature des images** en place
  pour ADR-007 ; `systemd-resolved` peut faire du DNSSEC (OpenSSL présent) ; cohérent avec
  ADR-009 (« réutiliser l'existant »).
- **Négatives / risques :** le socle v0.2 s'alourdit (OpenSSL et curl) et croît en surface
  d'attaque ; le Wi-Fi reste **sans** registre réglementaire signé (ADR-020 inchangé), donc
  OpenSSL n'y sert pas ; la vérification de signature des mises à jour n'est utile qu'une
  fois les clés définies (ADR-007).
- **Suivi :** aligner `deployment-ops.md` (lot v0.2, sortie d'OpenSSL du hors-lot),
  `installer.md` et la traçabilité (`requirements-traceability.md`, `test-cases.md`) ;
  spécifier `/etc/repart.d` et `/etc/sysupdate.d` ; activer la **vérification de signature**
  des images lorsque les clés d'ADR-007 seront arbitrées.
