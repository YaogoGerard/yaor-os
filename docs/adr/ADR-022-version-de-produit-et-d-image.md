# ADR-022 — Version de produit et version d'image

**Statut :** Accepté
**Date :** 2026-10-17
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-003, ADR-007, ADR-015, ADR-016

## Contexte

Le mode installé (ADR-015) repose sur deux mécanismes qui exigent une **version
d'image** explicite :

- `ProtectVersion=%A` dans les transferts `systemd-sysupdate` : la version
  protégée est lue dans **`IMAGE_VERSION=`** d'`/etc/os-release` ; faute de cette
  clé, l'option est inopérante et la version en cours pourrait être écrasée ;
- l'**appariement source/cible** : `@v` identifie une version d'image, qui sert
  à la fois d'étiquette de partition (`yaor-root_<v>`) et de nom de fichier
  (`yaor-root_x86-64_<v>.raw.xz`, `vmlinuz-<v>`).

Or `/etc/os-release` (étape 04) ne définissait ni `IMAGE_ID` ni `IMAGE_VERSION` :
`VERSION_ID` portait la **version du noyau**, sans rapport avec le produit. Le
LLD installateur relevait ce point comme ouvert. Aucun schéma de version produit
n'était défini.

## Options envisagées

- **Pas de version produit ; utiliser la version du noyau comme `@v`** — le noyau
  et la racine évoluent indépendamment ; une même racine rebâtie avec un autre
  noyau changerait de version ; inadapté à `ProtectVersion=`.
- **Version calculée à la construction** (horodatage, empreinte Git) — non
  déterministe, contraire à la reproductibilité fonctionnelle (ADR-016).
- **Version produit explicite, source unique dans un fichier `VERSION`** — simple,
  déterministe, lisible par le shell (build) et reportée dans `os-release`.
  **Retenu.**

## Décision

- La **version produit** vit dans un fichier **`VERSION`** à la racine du dépôt,
  source unique de vérité. Elle suit le **jalon** (`MAJEUR.MINEUR`, p. ex. `0.2`)
  et reçoit une composante patch pour une **mise à jour publiée** (`0.2.1`).
  Elle est donc **monotone**, condition requise par `systemd-sysupdate` (qui
  n'installe qu'une version strictement plus récente).
- Le helper `yaor_version()` (`scripts/lib/common.sh`) lit ce fichier ; les étapes
  du pipeline s'y réfèrent (nom de l'ISO, `IMAGE_VERSION`, images de mise à jour).
- `/etc/os-release` gagne **`IMAGE_ID=yaor`** et **`IMAGE_VERSION="<v>"`** ;
  `VERSION_ID`/`VERSION` portent désormais la **version produit** (et non plus la
  version du noyau, qui reste visible via `/boot`).
- La version produit **apparie** l'image racine et son noyau : le transfert de la
  racine (`10-root.transfer`) et celui du noyau (`20-boot.transfer`) publient la
  **même** `@v`.
- La version du **workspace cargo** des composants internes
  (`components/Cargo.toml`, `[workspace.package]`) doit rester **égale** à
  `VERSION` ; la phase `yaor-components` le vérifie et échoue sinon.
- `VERSION_ID` **n'est plus** la version du noyau : ce dernier reste consigné par
  `/boot/vmlinuz-<version-noyau>`, `System.map-<version-noyau>` et
  `config-<version-noyau>`.

## Conséquences

- **Positives :** `ProtectVersion=%A` devient opérant ; l'appariement
  racine/noyau/slots est net ; version déterministe et unique, alignée sur les
  jalons ; cohérence vérifiée automatiquement entre `VERSION` et les composants.
- **Négatives / risques :** une publication doit **incrémenter** `VERSION` (sinon
  `systemd-sysupdate` considère l'image comme déjà installée) ; deux sources
  (`VERSION` et `Cargo.toml`) subsistent mais leur égalité est contrôlée au build.
- **Suivi :** la politique de numérotation fine (pré-versions, versions de
  développement) reste à préciser si le besoin apparaît ; aligner
  `installer.md`, `deployment-ops.md` et `README.md`.
