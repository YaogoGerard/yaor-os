# User Flows

**Statut :** v0.6 — *mise à jour : 29 octobre 2026*

Flux utilisateurs du bureau. Les écrans de référence sont dans
[`design/`](design/) ; les composants dans
[`design-system.md`](design-system.md).

## Changement de profil

Deux voies équivalentes vers le même résultat — le Desktop Core
recharge la configuration et le comportement change sans toucher à
la logique (REQ-SW-015) :

```text
Utilisateur -> Menu YAOR -> Desktop Profiles -> Sélection -> Application de la configuration -> Nouveau comportement
```

1. **GUI — dialog « Profils de bureau »** (rendu
   [`design/macOS-like.png`](design/macOS-like.png)) : trois cartes
   Classique / macOS-like / Tiling, badge « Actif » sur le profil en
   cours, boutons `Annuler` / `Appliquer` — l'application du profil
   n'a lieu qu'à la confirmation ;
2. **CLI** (rendu [`design/Classique.png`](design/Classique.png)) :
   `yaor-ctl profile set classic` → « Profil appliqué : Classique ».

À la validation :

```text
Édition configuration -> Validation -> Reload -> Desktop Core -> Nouveau comportement
```

État : le rechargement de configuration existe (SIGHUP) ; le
**changement de profil à chaud** (permutation flottant ↔ tiling
sans relance) est l'étape 3 du jalon v0.6.

## Lancement d'applications (profil classique)

```text
Taskbar/Menu YAOR -> Volet menu -> Recherche ou Épinglées -> Lancement -> Fenêtre flottante centrée
```

Rendu : [`design/Classique.png`](design/Classique.png) — volet avec
recherche, grille « Épinglées » (Fichiers, Terminal, Web, Éditeur,
Réglages, Profils, Musique, Calcul).

## Manipulation d'une fenêtre (profil classique)

```text
Clic (focus + raise) -> Move (grab barre de titre) / Resize (grab bord) -> Minimiser (Mod+m ou bouton) -> Restaurer (taskbar ou Mod+m)
```

État : focus/raise/move/resize/minimiser-restaurer livrés (étape
1b.a, `t-yaor-float` 25/25) ; décorations serveur (étape 1b.b) et
taskbar basse (1b.c) à venir.

## Espaces de travail (profil tiling)

```text
Barre supérieure (1-5) ou `yaor-ctl workspace N` -> Espace actif -> Fenêtres maître+pile réorganisées
```

Rendu : [`design/Tiling.png`](design/Tiling.png) — « Espace 2 actif
— 2 fenêtres ». État : layout maître+pile livré (v0.5) ; espaces de
travail livrés avec l'étape 3.

## Modification manuelle de la présentation

```text
Édition desktop.toml (layout, keys, thème) -> Reload (SIGHUP) -> Desktop Core -> Nouvelle présentation
```

Un thème ([`themes/`](themes/)) ne modifie que la présentation ;
jamais la logique du Desktop Core.
