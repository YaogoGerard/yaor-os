# YAOR Design System

**Statut :** v0.6 — *mise à jour : 29 octobre 2026*

Le design system définit les éléments visuels communs : couleurs,
typographie, espacements, icônes, boutons, panneaux, fenêtres, menus
et notifications. Les profils (bureaux) modifient leur présentation
tout en partageant ces primitives — un thème ([`themes/`](themes/))
ne touche jamais à la logique du Desktop Core.

## Identité « ambre sur noir »

La référence canonique est l'emblème produit :
[`branding/yaor-os-amber-emblem.png`](branding/yaor-os-amber-emblem.png)
(cathédrale de canyon lumineux ambre sur fond noir) et la palette de
[`branding/README.md`](branding/README.md).

| Rôle | Valeur | Usage dans les rendus ([`design/`](design/)) |
|---|---|---|
| Fond panneau / barres | `#101014` | taskbar, barre de menus, volet, dock |
| Accent actif | `#E8862E` | boutons, focus, fenêtre active, jauges |
| Accent sombre / bordure | `#B96A1F` | bordures de volets et de dock |
| Texte clair chaud | `#F2E8DC` | titres, libellés |
| Fond de bureau | `#16161C` | zone de travail (fond d'écran ambre atténué dans les rendus) |
| Surfaces de fenêtre | `#1A1A20` → `#14141A` | dégradé barre de titre → contenu |

Rôles dérivés (rendus) : fond de champ `#1E1E26`, texte secondaire
gris chaud (~`#9A938A`), état « fenêtre lancée » = point ambre,
surface au repos = gris neutre `#26262E`.

## Typographie

- **Produit (rendu actuel)** : police bitmap 5×7 incorporée
  (convention « un besoin = un fournisseur » — pas de rendu de
  polices vectorielles dans les composants Rust) ;
- **Rendus cibles** : sans-serif moderne pour l'interface,
  monospace pour le terminal — l'introduction d'une police produit
  vectorielle est un arbitrage d'étape ultérieure (elle devra
  suivre la même convention).

## Composants

Primitives communes à tous les profils ; le rendu de référence de
chaque composant est dans [`design/`](design/) :

| Composant | Rendu | Description |
|---|---|---|
| Fenêtre (décorations serveur) | Classique | barre de titre (titre, réduire –, agrandir □, fermer ×) dessinée par le compositor ; variante « feux tricolores » pour le profil macOS-like |
| Fenêtre focusée | tous | bordure ambre `#E8862E` ; au repos : bordure neutre |
| Taskbar | Classique | barre basse : emblem, boutons de fenêtres, zone d'état (langue, batterie, horloge, date) |
| Volet menu d'applications | Classique | recherche, grille « Épinglées » (tuiles monogrammes), utilisateur, alimentation |
| Barre de menus | macOS-like | menus de l'app active + zone d'état |
| Dock | macOS-like | lanceurs centrés, points d'état, séparateurs |
| Barre tiling | Tiling | espaces de travail 1-5, titre focusé, indicateur de layout, CPU/batterie/horloge |
| Dialog modal | macOS-like | carte centrée, boutons `Annuler` (neutre) / `Appliquer` (accent), badge « Actif » |
| Cartes d'état | Tiling | Moniteur : jauges CPU/Mémoire/Disque à l'accent ambre |
| Terminal | tous | fond `#14141A`, prompt et accents ambre, texte clair |

## Espacements et géométrie (repères QA)

- Barre/panel : zone exclusive de **24 px** (mesurable par
  différence de tiling, voir TEST-UI-001) ;
- Décalage de cascade / naissance flottante : **24 px** par fenêtre ;
- Boutons de fenêtre : carrés ~28 px (zones de clic QA des
  décorations, étape 1b.b) ;
- Rayons : fenêtres et cartes ~10-12 px (rendus) ; volet menu ~8 px.

## Icônes

- **Emblème** : bitmap 24×24 incorporé (`yaor-panel/src/logo.rs`),
  affiché à gauche du bouton menu et dans les barres des rendus ;
- **Tuiles monogrammes** : carrés arrondis (accent ambre ou gris)
  avec monogramme — Fichiers `F`, Terminal `>_`, Web `W`, Éditeur
  `É`, Réglages `R`, Profils `D`, Musique `M`, Calcul `C` ;
- Les icônes sont des primitives de présentation : aucune logique
  de profil ne leur est attachée.

## États interactifs

| État | Présentation |
|---|---|
| Focus fenêtre | bordure ambre + titre actif |
| Fenêtre lancée (dock/taskbar) | point ambre sous la tuile / bouton enfoncé |
| Profil actif | badge « Actif » ambre sur la carte |
| Survol (rendus) | surface éclaircie ~10 % |
| Danger (fermer) | croix au survol, sans couleur dédiée (restée neutre dans les rendus) |
