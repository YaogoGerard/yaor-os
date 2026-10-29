# Desktop Modes

**Statut :** en cours (jalon v0.6 « Caméléon ») — *mise à jour :
29 octobre 2026*

Le bureau YAOR offre trois profils d'organisation des fenêtres
(REQ-SW-008/009/010). Les trois modes utilisent le même **Desktop
Core** : mêmes primitives du design system, même gestion des
surfaces Wayland/X11, même format de configuration déclarative
(REQ-SW-015) — seul le choix de layout et les éléments de shell
changent.

Rendus cibles : [`design/`](design/) — maquettes QA :
[`mockups/`](mockups/).

## Classic / Windows-like — REQ-SW-008, TEST-UI-001

Rendu : [`design/Classique.png`](design/Classique.png).

- **Taskbar en bas** (emblem, fenêtres ouvertes, zone d'état) ;
- **Menu d'applications** (recherche + épinglées) ;
- **Fenêtres flottantes** : placement libre centré, empilement,
  raise au clic, move/resize interactifs, minimiser/restaurer ;
- **Décorations serveur** : barre de titre + réduire/agrandir/fermer
  (le compositor dessine, conforme au modèle Windows-like).

État d'implémentation (jalon v0.6) :

| Étape | Contenu | État |
|---|---|---|
| 1 | Socle layer-shell (barre haute, volet menu, habillage ambre) | Livrée (TEST-UI-001 vert) |
| 1b.a | Fenêtres flottantes + minimiser/restaurer (`Mod+m`) | **Livrée** (`t-yaor-float` 25/25 ×2) |
| 1b.b | Décorations serveur (barre de titre, boutons) | À venir |
| 1b.c | Taskbar basse (liste de fenêtres, restauration au clic) | À venir |

## macOS-like — REQ-SW-009, TEST-UI-002

Rendu : [`design/macOS-like.png`](design/macOS-like.png).

- **Barre de menus supérieure** : menus de l'application active +
  zone d'état ;
- **Dock** centré en bas : lanceurs, fenêtres lancées (point d'état) ;
- **Fenêtres flottantes** aux « feux tricolores » ambre ;
- **Dialog « Profils de bureau »** : cartes Classique / macOS-like /
  Tiling avec badge **Actif**, `Annuler` / `Appliquer`.

État : jalon v0.6 **étape 2** — non commencé ; le dialog de profils
et le changement à chaud y sont associés.

## Tiling — REQ-SW-010, TEST-UI-003

Rendu : [`design/Tiling.png`](design/Tiling.png).

- **Fenêtres organisées automatiquement** : maître + pile (layout
  `master-stack`, ratio et gaps configurables) ;
- **Barre supérieure** : espaces de travail (1-5), titre de la
  fenêtre focusée, indicateur de layout, état système ;
- **Navigation clavier** : focus, ratio, close/quit (raccourcis
  `keys.*` de la configuration) ;
- **Espaces de travail** (`yaor-ctl workspace N`).

État : layout maître+pile **livré** (v0.5, `t-yaor-tiling` 26/26 +
6/6) ; espaces de travail et changement de profil à chaud : jalon
v0.6 **étape 3**.

## Changement de profil

Les trois modes partagent le Desktop Core : changer de profil
applique une autre configuration de layout et de shell **sans
changer la logique** (voir [`user-flows.md`](user-flows.md)).
Un profil ne modifie jamais la présentation d'un autre ni le cœur.
