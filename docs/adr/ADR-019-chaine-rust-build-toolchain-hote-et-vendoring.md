# ADR-019 — Chaîne Rust au build : toolchain hôte et vendoring au téléchargement

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-002, ADR-012, ADR-016, ADR-017, ADR-018

## Contexte

ADR-012 prévoyait de **bâtir `rustc` depuis les sources** avec le `rustc` de l'hôte comme
amorce et de **le livrer** dans le système. Deux constats, faits en préparant le pipeline,
remettent en cause ce volet :

- les binaires Rust retenus (`uutils/coreutils`, `uutils/findutils`, `sudo-rs`) sont des
  **artefacts de build** : rien dans l'image finale n'a besoin de `rustc` ni de `cargo` ;
- embarquer `rustc` + LLVM pèse **~300 Mo** et coûte **plusieurs heures** de build, alors
  que l'exigence première est une ISO **extrêmement légère** — et que l'étape 06 retire
  déjà `gcc`, `python3`, `binutils`… pour cette raison.

Par ailleurs, ADR-018 a retenu le **vendoring** mais a laissé ouvert **où** vivent les
crates vendues.

## Options envisagées

- **Livrer `rustc` (ADR-012 initial)** — bâtir la toolchain depuis les sources ; ISO très
  lourde, amorçage lourd, aucun bénéfice pour une distribution live minimale.
- **Chaîne Rust au build seulement** — le `rustc`/`cargo` de l'hôte est un **prérequis de
  build** ; on ne livre que les binaires produits.
- **Crates sur le réseau au moment du build** — écarté par ADR-018 (non reproductible).

## Décision

1. **La toolchain Rust est un outil de build, pas un composant livré.** Le `rustc`/`cargo`
   de l'**hôte** est un **prérequis de build documenté et vérifié** (étape 00, version
   `≥ 1.99.0`) ; **aucun `rustc`/LLVM n'est embarqué ni bâti** dans le système. Ceci
   **remplace le volet « livrer `rustc` » d'ADR-012** (l'amorçage par le `rustc` de l'hôte,
   lui, est conservé).
2. **Compilation des composants Rust dans le chroot**, à partir de la toolchain de l'hôte
   montée **en lecture seule**. Les binaires `rustc`/`cargo` ne dépendent que de `libc`,
   `libgcc_s`, `libm`, `libdl`, `librt` et `libpthread` — tous présents dans la cible — et
   la glibc de l'hôte est compatible.
3. **Vendoring généré au téléchargement** (complète ADR-018) : l'étape 01 exécute
   `cargo vendor` à partir des `Cargo.lock` **épinglés** ; l'arbre vendu vit dans le
   **cache de build** (jamais dans le dépôt) et le build se fait **hors-ligne**
   (`CARGO_NET_OFFLINE=true`).

## Conséquences

- **Positives :** ISO légère, cohérente avec l'étape 06 ; pas de `rustc` à bâtir ni à
  maintenir ; build Rust hors-ligne et reproductible au sens fonctionnel (ADR-016).
- **Négatives / risques :** un `rustc`/`cargo` hôte est désormais requis (contrôlé en 00) ;
  le vendoring alourdit le dossier de build (plusieurs centaines de Mo par composant,
  régénérés au téléchargement).
- **Suivi :** `ADR-012` reste valable pour le reste (`uutils/coreutils`, `sudo-rs`) ; sa
  partie « livrer `rustc` » est remplacée par le présent ADR. `sudo-rs` requiert PAM et
  sera ajouté avec le sous-système de sessions (ADR-014).
