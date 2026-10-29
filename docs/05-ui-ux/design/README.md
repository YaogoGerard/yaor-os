# Design — Les 3 bureaux (rendus cibles)

Rendus haute-fidélité de l'expérience cible de chaque profil,
identité « ambre sur noir » ([`../branding/`](../branding/)). Ce sont
des **cibles design** : ils montrent le produit visé, pas l'état
livré — l'état réel de chaque profil est suivi dans
[`../desktop-modes.md`](../desktop-modes.md) et les jalons associés.

| Fichier | Profil | Exigence | Jalon |
|---|---|---|---|
| `Classique.png` | Classique (Windows-like) | REQ-SW-008 | v0.6 étape 1b |
| `macOS-like.png` | macOS-like | REQ-SW-009 | v0.6 étape 2 |
| `Tiling.png` | Tiling | REQ-SW-010 | v0.6 étape 3 |
| `YAOR OS — Les 3 bureaux.html` | Export interactif des trois rendus (auto-extractible) | — | — |

## Ce que montrent les rendus

### `Classique.png` — taskbar basse, menu, fenêtres flottantes
- **Taskbar en bas** : emblème YAOR, boutons des fenêtres ouvertes
  (« Fichiers », « Terminal » actif), zone d'état à droite
  (langue FR, batterie 82 %, horloge 12:34, date) ;
- **Menu d'applications** : volet flottant en bas à gauche — logo
  YAOR OS, champ de recherche, grille « Épinglées » (tuiles
  monogrammes : Fichiers, Terminal, Web, Éditeur, Réglages, Profils,
  Musique, Calcul), ligne utilisateur + bouton d'alimentation ;
- **Fenêtres flottantes** avec **décorations serveur** : barre de
  titre « Fichiers — Documents » / « Terminal », boutons réduire (–),
  agrandir (□), fermer (×) ; fenêtre focusée à la bordure ambre ;
- **Terminal** illustrant le changement de profil en CLI :
  `yaor-ctl profile set classic` → « Profil appliqué : Classique ».

### `macOS-like.png` — barre de menus, dock, sélecteur de profils
- **Barre de menus supérieure** : emblème, menus de l'application
  active (« Profils » : Fichier, Édition, Affichage, Fenêtre, Aide),
  zone d'état à droite (FR, 82 %, jeu. 29 oct. 12:34) ;
- **Dock** centré en bas : icônes d'applications, points d'état
  (lancée), séparateurs ;
- **Fenêtres flottantes** aux boutons « feux tricolores » en version
  ambre (fermer, minimiser, agrandir) ;
- **Dialog « Profils de bureau »** (le cœur « Caméléon ») : trois
  cartes — Classique (barre des tâches et menu), macOS-like (barre de
  menus et dock, badge **Actif**), Tiling (fenêtres au clavier) —
  avec `Annuler` / `Appliquer`. Sous-titre : « Choisissez
  l'organisation des fenêtres. Le cœur du bureau reste identique. »

### `Tiling.png` — maître + pile, espaces de travail
- **Barre supérieure** : emblème, **espaces de travail 1-5** (2
  actif), titre de la fenêtre focusée (« éditeur —
  ~/yaor/desktop-core »), indicateur de layout « [ ] = maître+pile »,
  CPU 12 %, batterie 82 %, 12:34 ;
- **Disposition maître + pile** : fenêtre maître à gauche (éditeur),
  pile à droite (Moniteur : jauges CPU/Mémoire/Disque ; Terminal) ;
  fenêtre focusée à la bordure ambre ;
- **Terminal** illustrant la navigation d'espaces :
  `yaor-ctl workspace 2` → « Espace 2 actif — 2 fenêtres ».

## Relations

- Maquettes schématiques QA : [`../mockups/`](../mockups/)
  (zones de clic, géométries mesurables par le harnais) ;
- Palette canonique et emblème : [`../branding/`](../branding/) ;
- Cas de test : `TEST-UI-001/002/003` dans
  [`../../06-testing/test-cases.md`](../../06-testing/test-cases.md).
