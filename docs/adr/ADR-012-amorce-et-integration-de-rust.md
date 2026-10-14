# ADR-012 — Amorce et intégration de Rust

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-002, ADR-009

## Contexte

Le projet **préfère les composants Rust** lorsqu'ils sont matures (ADR-002) et retient par
défaut des composants modernes, maintenus et performants (ADR-009). Le jalon v0.2
introduit deux composants Rust dans le système :

- **`uutils/coreutils`** (core utilities) ;
- **`sudo-rs`** (élévation de privilèges, voir ADR-014).

Or Rust est **auto-hébergeur** : `rustc` est écrit en Rust et ne peut pas être compilé
sans un compilateur Rust préexistant. L'ADR-001 impose de construire le système **depuis
les sources** et n'autorise pas l'usage d'un paquet binaire d'une distribution tierce pour
le système de base. Il faut donc trancher la question de l'**amorçage**, ainsi que celle
du **LLVM** sur lequel Rust repose.

Le bootstrap de la chaîne C suit déjà cette logique : le `gcc` de l'hôte sert à
construire la toolchain cible (ADR-001) ; l'hôte n'est qu'un **outil d'amorçage**, les
artefacts livrés restent issus des sources.

## Options envisagées

- **Option A — `mrustc`.** Compilateur C→Rust permettant un bootstrap 100 % source, mais
  lent, incomplet (contrôle d'emprunt partiel) et peu maintenu ; risque d'échec.
- **Option B — binaire `stage0` officiel de rust-lang, épinglé.** Simple, mais introduit
  un binaire tiers précompilé, contraire à l'esprit d'ADR-001.
- **Option C — `rustc` de l'hôte comme compilateur d'amorçage.** Analogue au `gcc` de
  l'hôte : le `rustc` cible est **construit depuis les sources** ; l'hôte ne fournit que
  le bootstrap.

## Décision

**Option C.**

- Le **`rustc` de l'hôte** sert de **compilateur d'amorçage** : il construit le `rustc`
  cible à partir des sources Rust épinglées. Le `rustc` livré dans YAOR est donc bâti
  depuis les sources ; l'hôte n'est qu'un outil de bootstrap, comme le `gcc` de l'hôte
  pour la chaîne C.
- Le **LLVM embarqué par Rust** est utilisé ; **aucun LLVM/Clang système** n'est ajouté au
  lot pour l'instant.
- **`uutils/coreutils`** remplace GNU coreutils dans l'image finale ; GNU coreutils reste
  les **outils temporaires** de construction (le pipeline en dépend) et n'est pas livré au
  userland final.
- **`sudo-rs`** est retenu pour l'élévation de privilèges (voir ADR-014).

Les composants retenus DOIVENT satisfaire les critères de maturité et de stabilité
d'ADR-002.

## Conséquences

- **Positives :** cohérence avec ADR-002 et ADR-009 ; aucun binaire tiers dans le système
  livré ; les composants restent construits depuis les sources.
- **Négatives / risques :** le bootstrap de Rust est **lourd** et allonge sensiblement le
  pipeline ; il introduit une dépendance à l'outillage de l'hôte (`rustc`) à documenter ;
  `uutils/coreutils` est plus jeune que GNU coreutils et peut présenter des écarts de
  comportement.
- **Suivi :** documenter la procédure d'amorçage dans `deployment-ops.md` ; vérifier les
  écarts `uutils`/GNU en phase de tests ; si `uutils` ou `sudo-rs` ne satisfait pas les
  critères d'ADR-002 à l'usage, un ADR ultérieur tranche le repli.
