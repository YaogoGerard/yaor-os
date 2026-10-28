# Desktop Core — LLD (jalon v0.5, étape 1)

## Responsabilité

Le « Desktop Core » est la couche qui rend le bureau vivable : placement des
fenêtres (tiling), raccourcis clavier de bureau et comportement piloté par la
configuration. En v0.5 il est réalisé **dans le compositor** (ADR-005 : pas de
Window Manager distinct) via deux modules de la crate `components/compositor`
(`policy.rs`, `config.rs`) et la crate de configuration `components/yaor-config`.

Historique : ce LLD portait la vue « sous-modules caméléon » (profiles/themes/
plugins). Cette vue reste décrite par `05-ui-ux/desktop-modes.md` ; la
réalisation v0.5 est limitée au périmètre ci-dessous (REQ-SW-006, REQ-SW-015).
REQ-SW-007 (profils) reste v0.6.

## Périmètre v0.5 étape 1 (livré)

```text
components/yaor-config/     # lecture TOML + journalisation (crate du workspace)
└── src/lib.rs             # DesktopConfig [layout] + [keys], défauts compilés
└── default.toml           # la valeur d'usine posée dans le live
components/compositor/src/
├── policy.rs              # moteur de layout maître+pile + find_action/run_action
├── config.rs              # chargement, SIGHUP (rechargement vivant), mutateur
├── input.rs               # intercept clavier (FilterResult::Intercept)
├── shell.rs / x11.rs      # adoption/purge des fenêtres (xdg + X11)
├── session.rs / state.rs  # boucle recompute au cycle d'événements, sortie
└── scripts/04_chroot_build.sh  # pose /etc/yaor/desktop.toml (valeur d'usine)
```

## Layout maître + pile (policy.rs)

- **Mode par défaut : `cascade`** (le placement v0.4, inchangé) ; le mode
  `master-stack` est choisi par configuration — les `desktop.toml` d'usine
  (cascade) ne changent strictement rien aux QA v0.4.
- **Sortie par sortie** indépendance : la première fenêtre (« maître »)
  de chaque sortie occupe la colonne de gauche, largeur
  `ratio × (W − 2·gaps)` ; les suivantes (« pile ») partagent la colonne
  droite à parts égales verticales. Le recompute est par sortie
  (2 têtes virtio-gpu à l'étape 2).
- Fenêtres **flottantes** : une fenêtre déplacée/redimensionnée via grab
  interactif (QA v0.4 `t-yaor-comp-wm`) n'entre pas dans le tiling ; les
  fenêtres Xwayland restent flottantes en v0.5 (hors tiling, étape 1).
- Adoption (nouvelle toplevel xdg → adoption dans la sortie du focus à
  master-stack) et purge (transitions Dead pour checker close/destroy).
- Recompute journalisé au PIXEL : « tiling : géométrie WxH à (x,y) » —
  preuve QA (jetons, t-yaor-tiling 26/26).

## Raccourcis de bureau (style sway/Hyprland, Mod = Super)

| Affectation d'usine | Action |
|---|---|
| Mod+Right / Mod+Left | focus suivant / précédent (tuile) |
| Mod+Shift+Right / Mod+Shift+Left | échange deux fenêtres du tiling |
| Mod+Tab | échange la fenêtre focusée avec le maître |
| Mod+Q | `xdg_toplevel` close de la fenêtre focusée |
| Mod+g / Mod+Shift+g | ratio maître ± 0.05 (borné [0.2, 0.8]) |
| Mod+Shift+E | arrêt propre (équivalent SIGTERM) |

- Évaluation à la presse dans le filtre clavier Smithay ;
  `FilterResult::Intercept` consomme la touche (jamais transmise au
  client), sinon `FilterResult::Forward`.
- `parse_shortcut` : la lettre est **insensible à la casse** (leçon
  68238f3 : « Mod+Q » = la touche *q* — le keysym majuscule ne matche
  jamais la touche pressée sans Shift).
- En mode cascade, seuls Close et Quit sont actifs (garde `run_action`) —
  aucune régression permise sur le comportement v0.4.

## Configuration (desktop.toml) — interplay

- Chemins candidats hiérarchisés : `YAOR_DESKTOP_CONFIG` >
  `$XDG_CONFIG_HOME/yaor/desktop.toml` (root dans le live :
  `/root/.config/yaor/desktop.toml`) > `/etc/yaor/desktop.toml`
  (valeur d'usine posée par la phase de build depuis
  `components/yaor-config/default.toml`).
- Le journal du compositor nomme la source (« configuration chargée :
  … ») et les options (mode, ratio, gaps, raccourcis).
- SIGHUP → rechargement sans quitter la session (TEST-CONFIG-001 :
  ratio/gaps/rebind modifiés à chaud, disposition re-mesurée).

## QA (preuves étape 1)

Voir [`06-testing/graphics-tests.md`](../../06-testing/graphics-tests.md)
(v0.5 étape 1) : `t-yaor-tiling.py` 26/26 mesures + 6/6 sortie,
`t-yaor-config.py` 20/20, régressions v0.4 vertes (16/16, 23/23 ×2,
11/11, 6/6). Multi-écrans (2 sorties virtio-gpu) : harnais dédié à
l'étape 2.

## Hors périmètre v0.5 étape 1

layer-shell/barre, launcher, espaces de travail, DnD, profils Caméléon,
plugins ; X11 dans le tiling (les fenêtres Xwayland restent flottantes) ;
fenêtre « active » suivie du pointeur d'une sortie à l'autre (étape 2).
