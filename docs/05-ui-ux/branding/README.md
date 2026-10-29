# Identité visuelle YAOR OS

## Emblème

`yaor-os-amber-emblem.png` (1254×1254, RGB) — fourni par le produit le
28 octobre 2026 : cathédrale de canyon lumineuse ambre sur fond noir,
wordmark « YAOR OS ». Référence de l'identité « **ambre sur noir** ».

Déclinaisons en produit :

- **Panel (`yaor-panel`)** : version 24×24 incorporée en bitmap
  littéral — `components/yaor-panel/src/logo.rs` (généré : recadrage
  de l'emblème, ré-échantillonnage lanczos 24×24, fond noir rendu
  transparent par rampe douce 30..60 de luminance ; convention
  « un besoin = un fournisseur », pas de décodeur PNG). Affiché dans
  la barre à gauche du bouton menu.

## Palette de l'interface (« ambre sur noir »)

| Rôle | Valeur | Constante (`yaor-panel`) |
|---|---|---|
| Fond panneau (barre, volet) | `#101014` | `BAR_BG`, `MENU_BG` |
| Bouton / accent actif | `#E8862E` | `BTN_BG` |
| Texte sur accent | `#101014` | `BTN_FG` |
| Bordure du volet / accent sombre | `#B96A1F` | `BORDER` |
| Texte clair chaud | `#F2E8DC` | `TEXT_FG` |
| Fond de bureau (maquettes) | `#16161C` | — (choix d'étape ultérieure pour le compositor) |
| Fenêtre maquette (titres) | `#1A1A20` / `#1E1E26` | — |

## Maquettes

Dans [`../mockups/`](../mockups/) :

- `mockup-profil-classique.svg` — taskbar basse + menu Démarrer +
  fenêtres flottantes avec décorations serveur (cible REQ-SW-008,
  étape 1b) ;
- `mockup-profil-macos.svg` — barre de menus + dock (v0.6 étape 2) ;
- `mockup-profil-tiling.svg` — maître+pile + espaces de travail
  (v0.6 étape 3).

Les critères TEST-UI-001/002/003 (`test-cases.md`) réfèrent à ces
maquettes.
