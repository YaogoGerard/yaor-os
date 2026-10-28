# YAOR Compositor — LLD

## Objet

Spécifie le compositor graphique propre à YAOR (jalon v0.4) : sa constitution, ses
modules, sa politique de fenêtres et son intégration à la chaîne de build et à la QA.
Voir [`milestones.md`](../../01-product/milestones.md) (v0.4) et l'ADR-005.

## Décision (ADR-005, rappel)

- Base **Smithay** (bibliothèque Rust) : les primitives Wayland/DRM/GLES ne sont pas
  réimplémentées (ADR-002, RG-001).
- Le compositor **assure la gestion des fenêtres** : aucun « Window Manager » distinct.
- **XWayland** intégré pour la compatibilité X11 ; sessions via **logind** (ADR-004).
- La session de référence `anvil` 0.7 reste dans le live comme repli de validation
  (les QA v0.3 y sont rattachées) et comme référence d'implémentation.

## Constitution

| Élément | Source | Note |
|---|---|---|
| Smithay | archive épinglée 0.7.0 (étape 01, vendored) | cible seule `udev` (KMS) |
| `components/compositor` | sources du dépôt, workspace `components/` | toolchain hôte, hors-ligne, `Cargo.lock` committé |
| XWayland | archive épinglée xorg-server (étape 01) | nouveau fournisseur, uniquement ce paquet |
| wayland-protocols | archive déjà épinglée | XML `xdg-shell`, déjà consommé par waytest |

Contrairement à `yaor-common`/`yaor-slot`/`yaor-install` (zéro dépendance externe),
cette crate dépend de Smithay et consomme le vendoring de l'étape 01
(`CARGO_NET_OFFLINE=true`, ADR-018/019) : la phase de build doit donc tourner dans
l'arbre préparé par 01, sur le modèle de `phase_smithay`.

## Modules de la crate

```
components/compositor/src/
├── main.rs        # dispatch d'arguments (--tty-udev/--help/--version), journal
├── session.rs     # backend udev/KMS : libseat, GpuManager, DrmOutputManager,
│                  #   boucle vblank/rendu, entrées libinput, dmabuf
├── state.rs       # Yaor : globals Wayland, Seat, Space, popups, helpers scanout
├── render.rs      # éléments de rendu (OutputRenderElements, PointerElement),
│                  #   curseur mémoire statique, composition de la sortie
├── input.rs       # événements libinput → seat ; focus clavier/pointeur
├── shell.rs       # xdg-shell (toplevel/popup), seat, data device, shm
├── policy.rs      # politique de fenêtres : cascade (v0.4) et
│                  #   maître+pile + raccourcis de bureau (v0.5)
├── grabs.rs       # move/resize interactifs (référence : anvil/smallvil)
├── focus.rs       # cible de focus clavier (FocusTarget : Wayland ou X11)
├── x11.rs         # gestionnaire XWayland (XwmHandler) + sélections X11
└── config.rs      # desktop.toml : chargement au démarrage + SIGHUP (v0.5)
```

Détail par module :

- **session.rs** — l'acquisition du siège est faite par `libseat`, en **mode
  logind par défaut** (ADR-004, v0.4 étape 5) : le compositor lit
  `YAOR_LIBSEAT_BACKEND` (`logind` par défaut, `seatd` = repli explicite qui
  reste le chemin du live v0.3, valeur inconnue = repli logind avec
  avertissement), pose `LIBSEAT_BACKEND` en conséquence et journalise le
  choix (« backend de siège : logind/seatd » — preuve QA).
  Pour le mode logind, la **session logind attachée au siège** est résolue
  par le compositor avant l'ouverture (`seat_session_id`) : (1)
  `XDG_SESSION_ID` hérité, mais seulement s'il désigne une session logind
  existante attachée à un siège ; (2) la session du groupe cgroup du
  processus (`session-<id>.scope`, celle du lanceur) ; (3) en repli, la
  première session logind dont le fichier d'état `/run/systemd/sessions/<id>`
  porte un `SEAT=` non vide — privilégiée si son scope est le nôtre. Ce
  repli est nécessaire : un autologin peut être rattaché à la session du
  gestionnaire utilisateur (sans siège) et le nommage `c1`/`c2`… varie selon
  les boots — sans résolution, libseat logind échoue avec ENODATA (« No
  data available », `os error 61`). Le triplé (id, siège, VT) résolu est
  posé dans `XDG_SESSION_ID`/`XDG_SEAT`/`XDG_VTNR` et journalisé (« session
  logind : … »). Le compositor reprend alors le contrôle de la session
  existante (pas de session logind de plus) ; libseat y ouvre le siège et
  prend le master KMS, énumère les sorties (`display-info` via
  `smithay-drm-extras`), construit l'`EGLDisplay` (softpipe en QA, gate
  `YAOR_SOFTWARE_OK`) et pilote un `GpuManager`/`DrmOutputManager` par device.
  Lancement `--tty-udev`, conforme à anvil. Les vblanks (`DrmEvent::VBlank`) clôturent
  la frame et replanifient le rendu ; pas de leasing DRM, syncobj, DnD ni compteur
  FPS en v0.4.
  **Démontage** : sur SIGTERM/SIGINT, la boucle sort et `run_udev` démonte
  explicitement : retrait des notifiers DRM de la boucle (le
  `DrmDeviceNotifier` détient le dernier `Arc` sur l'`AtomicDrmDevice`), puis
  libération des sorties (le device est détruit ici, restauration CRTC et
  remise du maître comprises), puis de l'état Wayland, puis de la boucle.
  L'ordre est imposé par le code, pas par l'ordre des champs : libseat (logind
  ou seatd) révoque le maître DRM dès que le client libseat se ferme. La restauration de
  l'instantané fbcon capturé à l'ouverture échoue (EINVAL) — limite connue de
  Smithay, sans effet sur la remise du maître (le fd est fermé sans erreur) ni
  sur la console, que la QA vérifie au scanout après sortie. La socket Wayland
  et son verrou sont retirés explicitement en fin de démontage (le `Drop` de
  `ListeningSocket` de smithay n'est pas jugé fiable dans cet ordre de
  libération : QA étape 1/2 — socket restante constatée, corrigée par retrait
  explicite).
- **state.rs** — état partagé `Yaor` : globals (compositor, xdg-shell, shm, output,
  seat, data device, dmabuf), `Space` des fenêtres, `PopupManager`, position du
  pointeur, socket Wayland ; **XWayland** (v0.4 étape 4) : `start_xwayland` est
  appelé une fois les sorties primaires configurées (modèle anvil udev) —
  création de `/tmp/.X11-unix` (0o1777), `XWayland::spawn` de smithay (smithay
  lui-même publie la socket X `X%N` et le verrou `.X%N-lock`, insère le client
  Wayland de Xwayland et passe la connexion via `WAYLAND_SOCKET` fd), source
  source calloop branchée par `insert_source` ; l'événement `Ready` attache le
  WM (`X11Wm::start_wm`, `x11.rs`), `xdisplay`/`xwm` gardés dans l'état ;
  helpers de feedback de présentation et de scan-out primaire (transposition
  d'anvil, sans layer-shell).
- **render.rs** — assemblage des éléments d'une sortie : fenêtres de l'espace +
  curseur. Le curseur est rendu par le compositor (pas un client), en **buffer
  mémoire statique** (pas de thème xcursor au v0.4) ; le rythme vient des vblanks.
- **input.rs** — flux libinput : clavier, pointeur relatif, boutons, axes ; focus
  par surface sous le curseur (modèle smallvil/anvil). Le mouvement du pointeur
  cumule le delta **brut** (`delta_unaccel`) : déplacement 1:1 du dispositif,
  sans accélération matérielle — la politique d'accélération (modèles
  adaptables molette/trackpad) sera une option de configuration (v0.5).
- **shell.rs** — surfaces xdg-shell (toplevel/popup) insérées dans un `Space`
  Smithay ; mappage sur ack de configure, popup positionnée par le compositor.
  Les types de focus seat sont transposés au périmètre v0.4 étape 4 :
  `type KeyboardFocus = crate::focus::FocusTarget` (Wayland **ou** X11),
  `type PointerFocus = WlSurface` (une fenêtre X11 expose sa propre
  wl_surface côté Xwayland, donc le pointeur reste sur le chemin Wayland) ;
  `focus_changed` met le data device au client de la surface porteuse.
  Sélections : presse-papiers/primaire façon anvil étendu aux sélections X
  (`set_data_device_selection`/`set_primary_selection` vers le WM).
- **grabs.rs** — requêtes xdg interactives `xdg_toplevel_move/resize` armées sur
  le vérificateur `pointer.has_grab(serial)` (sérial de la pression, chaîné au
  grab implicite) — transposé de la référence anvil `shell/xdg.rs`
  (`PointerMoveSurfaceGrab` / `PointerResizeSurfaceGrab`, vérifiées en QA v0.3).
  Les grabs move/resize X11 réutilisent le `MoveSurfaceGrab` (le `Window` du
  `Space` porte un `X11Surface`, pas seulement un toplevel xdg).
- **focus.rs** (nouveau, étape 4) — `FocusTarget { Wl(WlSurface), X11(X11Surface) }`
  implémenté comme `KeyboardTarget`/`PointerTarget`/`WaylandFocus`/`IsAlive`
  (délégation par branche). La branche X11 délègue à smithay
  (`KeyboardTarget for X11Surface`) qui pose le focus côté X :
  `SetInputFocus` et `WM_TAKE_FOCUS` selon le mode d'entrée du client
  (`Passive` par défaut via WM_HINTS) — c'est le chemin de la livraison des
  touches aux applications X11. Le pointeur reste `WlSurface` (voie Wayland).
- **x11.rs** (nouveau, étape 4) — `XwmHandler` de YAOR : mapping des fenêtres
  (insertion dans le `Space` au lieu de la policy cascade, taille negociée
  comme les xdg), `configure_request`, fenêtre override-redirect (cercle
  intervention du WM), move-request réutilisant `MoveSurfaceGrab`,
  `destroyed/unmapped_window` purgeant le `Space`, sélections
  presse-papiers/primaire (accès limité aux clients d'une fenêtre X11 avec
  le focus clavier), `delegate_xwayland_shell!`.
- **policy.rs** — v0.4 : **empilement simple** (stacking) : placement en cascade
  au pas **320x200** (face aux tailles initiales 640x480 : chaque fenêtre laisse
  dépasser une bande franche de la précédente, les fenêtres empilées restent
  largement visibles en même temps — preuves QA observables à l'écran sans
  interaction), fenêtre active au sommet (stacking order du `Space`) ; le
  découpage tiling
  complet relève du Desktop Core v0.5 et des plugins v0.7 (ADR-000 décision 12 :
  le compositor garde le mécanisme, le Desktop Core branche la politique).
- **config.rs** — lecture d'un toml minimal (curseur, placement) ; la
  configuration complète sera branchée par `config-engine` v0.5.

## Gestion des fenêtres (ADR-005 — pas de WM distinct)

- tout mappage/unmapping/close passe par le compositor (pas de processus séparé) ;
- **taille initiale** : à `new_toplevel` le compositor écrit la taille
  640x480 dans l'état en attente du toplevel (avant le premier commit) —
  la configure initiale la porte, le client suit (waytest alloue son
  buffer à cette taille : journal `buffer 640x480 prêt`) ;
- interactifs xdg : move/resize par la voie v0.3 (grabs armés côté serveur) ;
  le TYPE (move puis resize) est découplé du bouton physique côté client —
  l'alternance milieu/droit de la QA ne fait que contourner les drops du
  canal PS/2 (SYN_DROPPED) ;
- fenêtre active = focus clavier (au clic gauche : raise du `Space` +
  `keyboard.set_focus`, puis reconfigure des fenêtres) ; visible = sommet
  du stacking ;
- close : délégation au client (`xdg_toplevel.close`) puis purge au délai.

### Fenêtres X11 (XWayland, v0.4 étape 4)

- la même politique et la même gestion de fenêtres s'appliquent : les surfaces
  X11 sont des `Window` du `Space` (mapping au `map_window_request`, taille
  posée à la policy cascade, raise au clic via `xwm.raise_window` en
  plus du raise du `Space` — le stacking du serveur X est tenu conforme au
  stacking compositor) ;
- le **focus clavier** est porté par `FocusTarget::X11` : à `keyboard.enter`
  smithay traduit (SetInputFocus + WM_TAKE_FOCUS) — le client X11 voit
  `FocusIn`/`KeyPress` dans son mode d'entrée sans code spécifique YAOR ;
- les **sélections** (presse-papiers et primaire) traversent le WM dans les
  deux sens (délégation smithay : `set_data_device_selection` côté Wayland,
  `send_selection` côté X) ;
- Xwayland lui-même est le serveur : YAOR n'implémente que le WM, pas le
  serveur X (ADR-005 : compositor seul, pas de serveur X séparé) ;
- parties hors périmètre v0.4 : thème xcursor (curseur X11 court en fil),
  clavier X11 par grab (`xwayland_keyboard_grab`), `set_client_scale`
  (HiDPI), XDMCP (IP/TCP désactivés à la construction).

## Intégration build (étape 04, conventions phases Rust)

- la crate est **membre du workspace `components/`** (comme yaor-common,
  yaor-slot, yaor-install) et bâtie par la phase `yaor-components` (binaire
  installé `/usr/bin/yaor-compositor`, test `--help` rc=0) ;
- Smithay arrive par **dépendance de chemin** vers l'arbre épinglé déjà
  décompressé (`/build/work/smithay-0.7.0`, racine de cet arbre = crate
  smithay, mêmes conventions que anvil) ; les sources du registre sont
  remplacées par le dossier vendored de cet arbre
  (`components/.cargo/config.toml`) : aucune résolution réseau, même
  environnement chroot et hôte (ADR-018) ;
- `Cargo.lock` du workspace **committé** (reproductible au sens
  fonctionnel, ADR-016) ; la cohérence de version produit reste contrôlée
  par la phase (ADR-022) ;
- lancement en session utilisateur selon ADR-014 (au pari des services
  de la session de référence).

## QA (TEST-GFX-003/004, v0.4)

- Harnais : `t-yaor-comp.py`, réutilisation de `t-anvil` (boot live, injections
  QMP, greps invités CNT-, screendumps QMP) et des leçons PS/2 v0.4 (alternance
  de boutons, reprises par round).
- Étape « session » (courante) : binaire/usage/version, siège via **libseat en
  mode logind** (défaut), session KMS
  tenue 12 s, socket Wayland publiée, client `waytest` connecté et fenêtre mappée
  (screendump : contenu client sur fond du compositor), sortie au SIGTERM.
- Étape « sessions — siège via logind » (harnais dédié `t-yaor-sess.py`,
  jalon v0.4 étape 5) **vert — 6/6, RC=0** : services actifs
  (`systemd-logind`, `dbus.socket`, `seatd`), lancement sans variable —
  journal « backend de siège : logind » + « session logind : … » (session à
  siège résolue, contrôle de la session existante repris, aucune session
  logind de plus, erreur d'ouverture : 0), socket Wayland publiée, SIGTERM
  propre (socket retirée), repli `YAOR_LIBSEAT_BACKEND=seatd` — journal
  « backend de siège : seatd », AUCUNE session logind de plus (le chemin
  v0.3 reste fonctionnel). Détail de la résolution : § session.rs.
- Étape « gestion des fenêtres » (harnais dédié `t-yaor-comp-wm.py`) :
  deux fenêtres `waytest --two` (W1 vert / W2 orange) au pas de cascade
  320x200, focus clavier par clic (compteurs par fenêtre du client),
  raise au clic (pixels verts dans la zone de chevauchement), move
  (`xdg_toplevel_move`, bbox verte déplacée) et resize
  (`xdg_toplevel_resize`, buffer client recréé à la taille mesurée).
- Étapes suivantes : fenêtre X11 (XWayland), profils graphiques —
  complètent TEST-GFX-003.
- Étape « compatibilité X11 » (harnais dédié `t-yaor-comp-xwl.py`,
  TEST-GFX-004) : le compositor attache Xwayland (`WM X11 attaché
  (display :0)`, socket `X0` + verrou), le client `xwtest` (libX11 seule)
  mappe sa fenêtre bleue 0x2A6ADB (carré blanc central), rendue au scanout
  (px bleus + blancs au screendump QMP), clic livré (`BTN n=1`) et
  touches livrées après focus par clic (`KEY keysym=0x6[12]`,
  journalisations côté invité dans `/tmp/xt.log`) ; sortie propre sur la
  touche q (DestroyWindow).
- Critère ouvert assumé : le canal PS/2 sacrifie occasionnellement un événement
  de bouton (leçon v0.4) ; la QA applique l'alternance + les reprises.

## Desktop Core v0.5 — tiling, raccourcis, configuration (étape 1)

Étape 1 du jalon v0.5 (commits `afbd82e`, `68238f3`) ; LLD et schéma :
[`desktop-core.md`](../desktop-core/desktop-core.md),
[`config-engine.md`](../config-engine/config-engine.md),
[`data-models/config-schema.md`](../data-models/config-schema.md).

- **policy.rs** : le placement devient paramétré — mode `cascade`
  (v0.4 strict : les QA établies restent exactement identiques, garde
  dans `run_action`) ou `master-stack` par sortie : maître à gauche
  (largeur `ratio·(W−2·gaps)`), pile à droite en parts égales ;
  fenêtres flottantes (grabs v0.4) et Xwayland hors du tiling ; le
  recompute est journalisé par tuile (« tiling : géométrie WxH à (x,y) »).
- **config.rs + crate yaor-config** : `desktop.toml` (TOML, serde du
  vendored Smithay), chemin candidat hiérarchisé (env > utilisateur >
  `/etc/yaor`, la phase de build pose la valeur d'usine), défauts
  compilés pour tout champ absent, **SIGHUP** → rechargement sans
  quitter la session ; le mutateur `config::update` sert aussi aux
  raccourcis Mod+g / Mod+Shift+g.
- **input.rs** : les touches sont évaluées à la presse ; un raccourci
  reconnu est intercepté (`FilterResult::Intercept`) et l'action
  exécutée (`policy::run_action`) — sinon la touche est transmise au
  client (comportement v0.4 inchangé).
- Correctif clavier `68238f3` : la lettre des affectations est
  insensible à la casse (`parse_shortcut`) — « Mod+Q » = la touche *q*
  (leçon diag : `from_char('Q')` donnait le keysym majuscule, jamais
  produit par une touche pressée sans Shift ; Mod+Shift+E ne matchait
  que par accident via le modified_sym). Test unitaire
  `lettre_inscensible_a_la_casse` dans `policy.rs`.
- QA étape 1 (détail :
  [`graphics-tests.md`](../../06-testing/graphics-tests.md), section
  « Desktop Core v0.5 ») : `t-yaor-tiling.py` 26/26 mesures + 6/6
  sortie (frontières au pixel), `t-yaor-config.py` 20/20
  (usine/SIGHUP/rebind/re-mesures), régressions v0.4 obligatoires
  vertes (16/16, 23/23 ×2, 11/11, 6/6).
- **Étape 2 — multi-écrans (2 sorties)** : `recompute` regroupe les
  fenêtres tuilées par sortie (chaque sortie dispose son propre
  maître+pile, aucune fusion) ; `master_slot_location` pose les
  fenêtres neuves sur la sortie sous le pointeur (`output_under`) —
  le focus suit le pointeur d'une sortie à l'autre ; le Mod+Tab
  devient **trans-sortie** quand la fenêtre focusée n'est pas sur
  celle du maître : échange des positions globales (`map_element`
  croisés) + re-tile, chaque fenêtre devenant maître de sa nouvelle
  sortie (journal « raccourci : maître trans-sortie — les fenêtres
  échangent de sortie ») ; même sortie, branchement étape 1 inchangé.
  QA : harnais 2 têtes `t-yaor-out2.py` 16/16 + 6/6 (recette QEMU et
  leçons : [`graphics-tests.md`](../../06-testing/graphics-tests.md)),
  régression `t-yaor-tiling` 26/26 + 6/6.

## Hors périmètre v0.4

- Décors de fenêtres (relèvent du Desktop Core et des couches plugins, v0.5+) ; profils
  Caméléon (v0.6) ; tiling complet (v0.5+) ; accessibilité AT-SPI (v0.8, ADR-006) ;
  multi-écran étendu (≥ 2 sorties) reporté.
