# Graphics Tests

Tests de la base graphique (ADR-024, jalon v0.3) : pile KMS/mesa/smithay, session
de référence, client Wayland, entrées réelles, audio et repli `simpledrm`.

## Environnement

- Invité QEMU `-enable-kvm -smp 4 -m 2048`, console série sur stdio,
  ISO live `out/yaor-live-x86_64-0.2.0.iso` (100 % construite depuis les
  sources, ADR-018 — hors-ligne).
- Harnais QA : `t-gfx-live.py` (console + vérifications génériques),
  `t-anvil.py` (session + client + entrées), variante `t-anvil-simpledrm.py`
  (repli `-vga std`). Screendumps et entrées via QMP `input-send-event`.

## TEST-GFX-002 — Pile graphique et session de référence (v0.3)

**Vérifie :** ADR-024, REQ-SW-005 (partie pile)

Points vérifiés sous QEMU (`t-gfx-live.py`, vert en v0.4 — 8/8, RC=0) :

- le KMS prend l'affichage au boot : connecteur `connected`, pilote identifié
  dans dmesg, fbcon basculé sur le framebuffer KMS ;
- mesa (softpipe) : renderer `softpipe`, GL 4.5/ES 3.1... (vérifié par les
  logs EGL de la session) ;
- session de référence anvil (Smithay 0.7) : seatd actif, prise du siège,
  tenant KMS master, socket `wayland-1` publiée, sortie propre au SIGTERM ;
- audio : PipeWire + WirePlumber, son synthétique HDA (WAV non vide) ;
- sortie propre : FB console restituée après SIGTERM.

Session + client + entrées (`t-anvil.py`, vert en v0.4 — 12/12, RC=0) :

- anvil 0.7.0 tient la session (KMS master via seatd), socket `wayland-1`
  publiée sous `/run/user/0`, screendump QMP pendant la session (scanout du
  compositor, rendu softpipe) ;
- **client de validation `waytest`** (sources internes du dépôt,
  `components/waytest/`) : fenêtre xdg-shell rendue via wl_shm (XRGB8888) —
  rectangle bordé, horloge digitale, compteurs d'événements, carré suiveur
  du pointeur ; preuve pixel : screendump post-injection (px sombres >
  seuil) avec horloge qui tourne et carré sous le curseur ;
- **entrées réelles livrées à la session** : injection QEMU via QMP
  `input-send-event` (pointeur PS/2 relatif borné, touches a/b/c) → les
  événements traversent evdev → libinput → anvil → waytest qui les affiche
  (compteurs K/P/B, carré déplacé) et les journalise (`/tmp/wt.log`) :
  2 clics et 3 touches livrés au client dans le run vert (boucle à retour
  d'information : injection cible par cible, comptage en direct via greps
  côté invité, préfixe `CNT-`) ;
- **déplacement + redimensionnement de la fenêtre** : ✓ démontré par les
  grabs move/resize d'anvil (`shell/xdg.rs`), déclenchés PAR LE CLIENT via
  le protocole xdg interactif (`xdg_toplevel.move`/`resize` avec le serial
  de la pression). Le déclencheur est **le rang de la pression, pas le
  bouton** : waytest envoie `move` sur la 1re pression interactive livrée
  (milieu ou droit) et `resize` (bord droit + bas) sur la 2e — type
  découplé du bouton physique car le canal PS/2 sacrifie environ un
  événement de bouton par run côté evdev/libinput (SYN_DROPPED pendant les
  rafales de mouvement, journal invité : « event processing lagging
  behind » ; cf. leçons v0.4). La QA alterne donc milieu et droit à chaque
  essai (`press_until`, 8 essais max, vérification de livraison au journal
  invité, drain de 2,2 s entre essais, équilibrage press/release avant
  chaque round) et reprend chaque round au plus une fois selon le compte
  de pressions livrées (le retry ne part que si le type de la prochaine
  livraison est sûr : 0 livraison pour move, 1 livraison cumulée pour
  resize). Preuves : déplacement lu au pixel (bbox du scanout avant/après,
  exigée avec la requête `move` loguée — 507 px dans le run vert, seuil
  80) ; redimensionnement lu côté client (buffer wl_shm recréé à chaque
  configure de taille — 8 re-créations dans le run vert) ;
- sortie propre sur SIGTERM (processus disparu, console restituée).

Repli simpledrm (`t-anvil-simpledrm.py`, vert — 13/13, RC=0, `-vga std`) :

- dmesg confirme le pilote `simpledrm` (pas de virtio-gpu) ;
- connecteur KMS `Unknown-1` (au lieu de `Virtual-1`) ;
- même session tenue : anvil rendu logiciel (simpledrm n'expose pas de render
  node), socket publiée, client connecté, entrées livrées, screendump avec
  contenu client, sortie propre ;
- move/resize couverts aussi sur le repli (run vert : requêtes `move` et
  `resize` loguées, déplacement 722 px, 8 buffers client recréés) ; les
  runs ultérieurs marquent 12/13 (round move non livré dans une fenêtre
  de run, resize bien démontré par la reprise) — fragilité résiduelle du
  canal PS/2 documentée aux leçons v0.4, le couvert fonctionnel
  move/resize est démontré dans les runs verts.

## TEST-GFX-003 — Compositor YAOR (v0.4)

**Vérifie :** ADR-005, REQ-SW-005 (partie compositor), ADR-022

Harnais : `t-yaor-comp.py` (virtio-gpu) et sa variante
`t-yaor-comp-simpledrm.py` (repli `-vga std`), bâtis sur `t-gfx-live`
(boot live, console série, screendump QMP, greps invités).

### Étape 1 — binaire livré

- la crate `components/compositor` est bâtie par la phase `yaor-components`
  et installée dans le live (`/usr/bin/yaor-compositor`) ;
- la version annoncée est celle du produit (`0.2.0`, ADR-022) ;
- `--help` sort en 0 ; un argument inconnu est refusé avec rc=2.

### Étape 2 — session KMS réelle (virtio-gpu)

`t-yaor-comp.py` **vert — 16/16, RC=0** :

- siège via **libseat en mode logind** (défaut, v0.4 étape 5) : le compositor
  résout la session logind attachée au siège (détail en § session.rs du LLD),
  ouvre la session libseat, prend le maître DRM
  et tient la session 12 s (`GPU primaire : renderD128`, `DrmDevice
  initializing`, EGL `PLATFORM_GBM_KHR`, connecteur `Virtual-1` 1280×800@75) ;
- socket Wayland `wayland-1` publiée ; `waytest` s'y connecte, lie
  `xdg_wm_base`/`wl_seat` et mappe sa fenêtre ;
- screendump QMP avant sortie : 55,4 % de pixels clairs (fond du compositor)
  contre 44,6 % sombres (fenêtre client) — contenu client rendu au scanout ;
- SIGTERM : le processus s'arrête, le journal prend acte de l'arrêt et la
  socket Wayland est retirée ;
- **console restituée** : screendump QMP après sortie — 0,2 % de pixels
  clairs / 99,8 % sombres (fbcon noir, texte clair) ; le scanout n'est plus
  le rendu du compositor, la main est bien rendue à la console ;
- **démontage DRM** : les notifiers DRM sont retirés de la boucle, puis les
  sorties sont libérées (le `DrmDeviceNotifier` détenait le dernier `Arc` sur
  l'`AtomicDrmDevice`) et le device est fermé tant que la session libseat est
  ouverte : `Dropping device: /dev/dri/card0`, maître rendu **sans erreur de
  permission** (aucun `EACCES`, aucun `Failed to drop drm master`).

Repli simpledrm (`t-yaor-comp-simpledrm.py`, `-vga std`) **vert — 18/18,
RC=0** :

- dmesg charge `simpledrm` (pas de virtio-gpu) ; connecteur `Unknown-1` ;
- mêmes preuves de session (screendumps identiques : 55,4 % clairs /
  44,6 % sombres, puis 0,2 % / 99,8 % après sortie), même démontage propre.

### Étape 2bis — sessions : siège via logind (v0.4, étape 5)

`t-yaor-sess.py` **vert — 6/6, RC=0** (harnais dédié, reprend le module
commun `t-gfx-live`) :

- services actifs : `systemd-logind`, `dbus.socket` (et `seatd`, qui reste
  le service du repli), sessions logind initiales de l'autologin (3,
  via PAM) ;
- lancement **sans variable** : journal « backend de siège : logind » et
  « session logind : … » — la session logind attachée au siège est résolue
  (triplé id/siège/VT journalisé) et son contrôle est repris : socket
  Wayland publiée, **aucune session logind de plus**, aucune erreur
  d'ouverture (`ERR-0`) ;
- SIGTERM : arrêt propre (`arrêt demandé par un signal`), socket Wayland
  retirée (0 socket `wayland-*` restante) ;
- repli explicite : relance avec `YAOR_LIBSEAT_BACKEND=seatd` — journal
  « backend de siège : seatd », session KMS tenue, **toujours aucune
  session logind de plus** (seatd ne s'interface pas avec logind) ; le
  chemin v0.3 reste opérationnel.

Le piège documenté : l'`XDG_SESSION_ID` hérité d'un autologin peut pointer
vers la session logind du gestionnaire utilisateur (ttyS0, **sans siège**)
et le nommage `c1`/`c2`… varie selon les boots — d'où la résolution tri
parties du compositor (§ session.rs du LLD) ; sans elle, libseat logind
échoue avec ENODATA (`os error 61`).

Notes de triage (non scorées) :

- `virtio_gpu: driver missing` au premier rendu est un **stub mesa** : le
  pilote gallium `virtio_gpu` n'est pas bâti (`-Dgallium-drivers=softpipe`) ;
  l'avertissement est bénin, le rendu se fait par softpipe (GL_RENDERER
  `softpipe`) ;
- `Erroneous EGL call didn't set EGLError` apparaît au modeset initial et
  n'affecte pas la session ;
- **restauration de l'instantané DRM** : au drop, Smithay rejoue l'état
  capturé à l'ouverture (`Failed to restore previous state. Error: Invalid
  argument`) ; l'instantané de la console (mode du CRTC, blob associé) n'est
  plus rejouable après la prise de contrôle du KMS. Ce n'est ni une erreur
  de permission ni une fuite du maître : le device est fermé et le maître
  rendu (contrôles `DM`/`DP`), et la console reprend l'écran (screendump
  post-sortie). Limite connue de Smithay sur ce chemin, documentée ;
- `Failed to destroy old mode property blob` (ENOENT) au modeset initial :
  le CRTC n'a pas de mode à la création de la surface (Smithay détruit alors
  un blob nul).

Les entrées réelles et les grabs xdg (move/resize, focus) relèvent de
l'étape « gestion des fenêtres » du jalon : couvertes par l'étape 3
ci-dessous.

### Étape 3 — gestion des fenêtres (2 fenêtres : focus, raise, move, resize)

Harnais dédié : `t-yaor-comp-wm.py` (base `t-yaor-comp.py` + injections
QMP adaptatives façon `t-anvil` ; pression PS/2 ⇒ arrivée au journal
client « W… pointer button ... press » ⇒ décision, jamais un timer).

Côté client :

- `waytest --two` crée **deux toplevels** — W1 (bord vert) puis
  W2 (bord orange) — chaque fenêtre dispose de ses compteurs et de son
  buffer (journal préfixé `W1 :`/`W2 :`) ;
- le compositor guide la **taille initiale** (640x480 envoyé à la
  configure initiale — le client suit) et **place en cascade** avec un
  pas de 320x200 : W2 recouvre largement W1, mais une bande franche de
  W1 reste visible (clic possible sur la fenêtre du fond) ;
- les preuves pixels classent les couleurs du screendump : verts
  (accent W1) et oranges (accent W2) sont comptés avec bboxes,
  le chevauchement est lu pixel par pixel.

Preuves de la phase interactive (scénario unique par boot) :

`t-yaor-comp-wm.py` **vert — 23/23, RC=0** (virtio-gpu) ; **repli
simpledrm : 23/23, RC=0** (`t-yaor-comp-wm-simpledrm.py`, `-vga std`).
Chiffres du run vert (virtio-gpu) :

- **deux fenêtres distinctes rendues** : 11 924 px verts / 16 352 px
  oranges, bboxes W1 (20,20,659,499) et W2 (340,220,979,699) — deux
  fenêtres 640x480 décalées du pas de cascade 320x200 ; baseline du
  chevauchement (340,220)-(660,500) : oranges > 0 ET verts == 0
  (W2 au premier plan, W1 totalement en dessous) ;
- **focus clavier au clic** : clic gauche sur W2 → `keyboard enter`
  reçu par W2 (focus posé par le compositor), touches a/b/c →
  `W2 KEY press`=3 et AUCUN `W1 KEY press` (aucune fuite) ;
- **clic + raise de la fenêtre du fond** : clic gauche sur la bande
  visible de W1 (30,30) → `keyboard enter` reçu par W1 (focus
  transféré), touches d/e → `W1 KEY press`=2 ; screendump : 4 428 px
  verts et 0 px oranges dans le chevauchement, bbox verte restreinte
  (340,220,659,499) — W1 est passée au premier plan au-dessus de W2
  (preuve purement pixel) ;
- **déplacer** : pression interactive (bouton milieu) sur W1 → journal
  client `W1 move request envoyé` → glisser +200,+140 → bbox verte
  (20,20,659,499) → (221,141,860,620) : déplacement mesuré de 644 px ;
- **redimensionner** : pression interactive suivante sur W1 → journal
  client `W1 resize request envoyé` → glisser +170,+120 : le client
  recrée ses buffers à la taille propagée (cascade 709x529 → … →
  794x589, jeton invité `BFR1-1` calculé sur le journal client) et la
  bbox verte passe à (221,141,1014,729) (largeur +153 px).

Fiabilité des pressions (canal PS/2) : le harnais draine, alterne les
boutons et reprend par round lorsque AUCUNE pression n'a été livrée ;
la séquence des types (1re interactive = move, 2e = resize) reste du
côté client (compteur `icount` PAR FENÊTRE), et la QA verrouille la
cible de chaque pression par le marqueur `W1 pointer button 274/273
press` du journal client (pression livrée DANS la fenêtre visée) —
jamais un timer. La position du pointeur est commandée exactement :
le compositor cumule le delta **brut** de libinput (`delta_unaccel`,
sans accélération matérielle — cf. LLD), donc les déplacements
commandés en unités PS/2 tombent au pixel près à l'écran. Pour le
clic gauche, la preuve primaire est la variation de `keyboard enter`
de la fenêtre visée (le focus n'est posé que par le compositor, sur
la pression SOUS cette surface) : le canal PS/2 peut avaler la ligne
« pointer button 272 press » côté journal client (le harnais affine
alors : pression confirmée par l'effet, jamais par un timer).

## TEST-GFX-004 — Compatibilité X11 via XWayland (v0.4, étape 4)

**Vérifie :** ADR-005, REQ-SW-005 (partie compositor), ADR-024 (validation QEMU)

Chaîne X épinglée dans `sources/manifest.tsv` (14 archives, SHA-256) : xorgproto,
xtrans, libpthread-stubs, xcb-proto, libXau, libxcb, libX11, libxkbfile,
libfontenc, libXfont2 (`--disable-freetype --disable-fc`), libxcvt, libxshmfence,
xkbcomp et Xwayland 24.1.14 (options meson : sans glamor/GLX/Xvfb/EGL,
`-Dsha1=libcrypto`, écoute TCP désactivée). Étendue réduite au socle QA : la validation porte sur le chemin « fenêtre
rendue, cliquable, clavier livré », pas sur toute la surface du serveur X
(extensions optionnelles, IPv6, XDMCP, etc. hors périmètre v0.4). Les phases
chroot `ch7.*` suivent l'ordre des dépendances (libXau avant libxcb, libxcb
avant libX11) et la phase `ch7.xwtest` bâtit le client de validation.

Client de validation `xwtest` (`components/xwtest/`, libX11 seule) : fenêtre
bleue 0x2A6ADB avec carré blanc central, journal `xwtest: MAP id=…`,
`xwtest: BTN n=…`, `xwtest: KEY keysym=… char=…` — les preuves texte sont
lues côté invité (`grep /tmp/xt.log`), l'écho console hôte pouvant être
tronqué.

Harnais : `t-yaor-comp-xwl.py` (virtio-gpu, base `t-gfx-live` + QMP local pour
les injections `input-send-event` et les screendumps). Comportement :
après le compositor (`/tmp/yc.log`), `xwtest` est lancé sur le display choisi
(`DISPLAY=:N`), puis phase interactive (screendump pixel, clic adaptatif au
centre de la fenêtre bleue, touches a/b/c, sortie sur la touche q).

`t-yaor-comp-xwl.py` **vert — 11/11, RC=0** (ISO 0.2.0, run du jalon v0.4
étape 4) :

- **Xwayland attaché** : le compositor prépare `/tmp/.X11-unix` (0o1777) et
  publie `WM X11 attaché (display :0)`, socket `X0` et verrou `.X0-lock`
  (jetons `XM+1`, `XS-X0`, `XLOCK+0`, `display :0`) ;
- **session KMS** : siège via libseat en mode logind (défaut), session
  tenue 12 s, socket
  `wayland-1` publiée (comme TEST-GFX-003 étape 2) ;
- **xwtest mappée** : connexion X réussie, `xwtest: MAP id=0x400001`
  (MapNotify client) ;
- **fenêtre rendue au scanout** : px bleus ≥ 8 000 (bord et fond de la
  fenêtre) et contenu client dessiné (carré blanc ≥ 5 000 px) au screendump
  QMP — le rendu passe Xwayland → wl_surface → compositor → DRM ;
- **clic livré** : clic gauche adaptatif (canal PS/2, même leçon que l'étape 3)
  au centre de la fenêtre → `xwtest: BTN n=1` (ButtonPress client) ;
- **clavier livré après focus** : touches a/b/c →
  `KEY keysym=0x61/0x62/0x63` (jeton `KEY keysym=0x6[12]` = 3 livraisons) ;
  le focus clavier est posé par le compositor sur la cible `FocusTarget::X11`
  (délégation smithay `KeyboardTarget for X11Surface` : `SetInputFocus` et
  `WM_TAKE_FOCUS` côté X, modèle anvil) ;
- **sortie propre** : la touche q ferme la fenêtre (DestroyWindow côté X) ;
  xwtest sort de lui-même (processus disparu, journaux complets).

Notes de triage (non scorées) :

- le canal PS/2 peut sacrifier un événement de bouton (cf. étape 3) : le
  harnais reprend jusqu'à 6 essais, vérifiés au journal client (`XT+1`), et
  draine les boutons avant chaque essai ;
- le client X11 détruit à sa sortie relâche sa wl_surface : les fenêtres
  suivantes du `Space` ne sont pas perturbées (chemins `_shell_commit`
  défensifs sur `toplevel()`) ;
- retrait de la socket Wayland rendu **déterministe** au démontage du
  compositor (socket `wayland-N` et verrou retirés à la main après la boucle :
  le `Drop` de la source smithay n'a pas été observé comme exécuté dans tous
  les chemins de libération, avec Xwayland en arrière-plan) — la régression
  constatée au harnais étapes 1/2 (KO « socket Wayland retirée ») est repassée
  au vert avec ce correctif (`t-yaor-comp.py` 16/16).

Pas de repli simpledrm dédié pour cette étape : la couverture de session du
repli est déjà démontrée par TEST-GFX-003 (session + entrées + console), et
la partie X11 de cette QA n'en dépend pas (chemin Xwayland identique).

## TEST-GFX-001 — Compositor

**Vérifie :** REQ-SW-005

- **Fenêtre ouverte** : ✓ démontré par la QA `t-anvil` (client xdg-shell
  `waytest` mappé, screendump avec contenu client, focus clavier obtenu au
  clic) ; **assumé en v0.4 par le compositor livré** (TEST-GFX-003 : fenêtre
  `waytest` mappée au scanout, socket Wayland publiée, sessions logind) ;
- **déplacer/redimensionner** : ✓ démontré via les requêtes xdg interactives
  standard (`xdg_toplevel_move`/`resize`) reçues par anvil qui arme ses
  grabs `PointerMoveSurfaceGrab`/`PointerResizeSurfaceGrab` (`shell/xdg.rs`)
  et suit le pointeur jusqu'au relâchement — le client `waytest` déclenche
  lui-même ces requêtes (alternance : 1re pression interactive livrée =
  move, 2e = resize) ; **assumé en v0.4 par le compositor livré**
  (TEST-GFX-003 étape 3 : bbox déplacée 644 px, resize à la taille mesurée) ;
- **rendu souris/clavier livré** : ✓ démontré par l'injection QMP (les
  événements traversent evdev → libinput → compositor → client) ;
  **assumé en v0.4 par le compositor livré** (TEST-GFX-003 étape 3 : focus
  clavier par clic au client, touches livrées à la fenêtre cliquée).

## Non couvert / à faire (v0.4+)

- Stabilisation du canal PS/2 injections boutons (un événement de bouton
  par run parfois sacrifié par evdev/libinput — contourné côté QA par
  l'alternance milieu/droit, les drains et la reprise par round).

## Desktop Core v0.5 (étape 1) — TEST-DESK-001, TEST-CONFIG-001

**Vérifient :** REQ-SW-006 (TEST-DESK-001), REQ-SW-015 (TEST-CONFIG-001).
Harnais : `t-yaor-tiling.py` (26/26 mesures + 6/6 sortie, RC=0) et
`t-yaor-config.py` (20/20, RC=0) — virtio 1280x800 mesuré au screendump
QMP (jamais forcé), config banc `/root/.config/yaor/desktop.toml`
(master-stack, ratio 0.5, gaps 0), jetons calculés côté invité
(`echo X-$?` / `echo X-$n` — jamais littéraux dans la ligne tapée,
leçon du run initial : l'écho tty « MQ+1 » validait un échec).

### Tiling maître + pile (`t-yaor-tiling`, vert)

- **2 fenêtres** : moitié/moitié — frontière mesurée à W/2 exact
  (SD1 : vert 0..639, orange dès 640, ±4 px) ;
- **4 fenêtres** : maître W1 (vert) à gauche + pile partagée au tiers :
  bands orange (0..265)/(532..799) encadrant le tiers central vert
  (266..531, ±4 px) — SD2 ;
- **Mod+Q** : le maître focusé au clic (keyboard enter +1 client) est
  fermé : journal compositor « raccourci : fermeture de la fenêtre
  focusée », client « W1 toplevel close » ; le reste re-tile (4 → 2 :
  waytest QUITTE le processus entier au close — S.quit) : maître W3
  vert 0..639 pleine hauteur, pile W4 orange dès 640 — SD3 ;
- **Mod+Tab** : la pile W4 échangée au maître (SD5 : orange 0..639,
  vert à droite) ; **Mod+g** : ratio 0.5 → 0.55 (SD6 : maître 703 px
  mesurés / 704 attendus, ±6 px) ;
- **Mod+Shift+E** : arrêt propre (« arrêt demandé par un raccourci »),
  processus sorti, socket retirée, console restituée, maître DRM rendu
  sans erreur (4/4 « démontage » — 6/6 sortie).

### Moteur de configuration (`t-yaor-config`, vert — 20/20)

- Démarrage SANS fichier utilisateur : la valeur d'usine
  `/etc/yaor/desktop.toml` est chargée et le journalise (« configuration chargée :
  /etc/yaor/desktop.toml », « mode cascade ») — REQ-SW-006 ;
- écriture de `/root/.config/yaor/desktop.toml` (master-stack, ratio
  0.7, gaps 10, close=Mod+X) puis **SIGHUP** : rechargement vivant
  (« configuration rechargée (SIGHUP) »), session + socket intactes,
  nouvelle config journalisée ;
- disposition re-mesurée aux fenêtres suivantes : frontière maître/pile
  à 897 px (attendu 0.7·1260+10 = 892, gaps 10 ±4 px) — SD2 ;
- **rebind à l'usage** : Mod+Q inactif (Q1-1), Mod+X ferme le maître
  focusé (QX-0 + « toplevel close » côté client) ;
- **2e SIGHUP** (ratio 0.5, gaps 0) : frontière re-mesurée à W/2 (±4 px)
  — SD3.

### Régressions v0.4 (obligatoires, toutes vertes)

`t-yaor-comp.py` 16/16 · `t-yaor-comp-wm.py` 23/23 (virtio) et
`t-yaor-comp-wm-simpledrm.py` 23/23 (repli simpledrm) ·
`t-yaor-comp-xwl.py` 11/11 · `t-yaor-sess.py` 6/6.
ISO de référence : `out/yaor-live-x86_64-0.2.0.iso` (SHA-256 vérifiée,
compositor du 28 oct. 04:01 — commits `afbd82e`, `68238f3`, `fdd002a`).

## Desktop Core v0.5 (étape 2) — multi-écrans, 2 sorties virtio-gpu

**Vérifie :** REQ-SW-006 (partie multi-écrans de TEST-DESK-001).
Harnais : `t-yaor-out2.py` (16/16 mesures + 6/6 sortie, RC=0) — 2
têtes virtio-gpu 1280x800, mesure couleur au screendump QMP **par
tête** (`device:"vgpu", head:0/1`), instance A (waytest, W1 vert/W2
orange) sur la tête 0, instance B (W3/W4) sur la tête 1. Sondes :
`t-probe-ptr.py` (adoption des fenêtres neuves par la sortie sous le
pointeur) et `t-probe-click.py` (où tombent les clics, matrice des
`keyboard enter` des deux journaux waytest).

### Recette QEMU multi-têtes headless (QEMU 11.1.2 — leçons du diagnostic)

1. **`-vga none` obligatoire** : sans lui, QEMU crée un VGA std par
   défaut EN PLUS du périphérique 2 têtes — deux cartes DRM coexistent
   et Smithay retient la mauvaise (une seule sortie, `Unknown-1`, sans
   EDID). Preuve : `query-pci` — VGA controller + Display controller
   avant correctif, un seul `vgpu` après.
2. **Chaque tête doit être déclarée** : dans `virtio-gpu-base.c`, une
   tête n'est « enabled » dans GET_DISPLAY_INFO que si
   `outputs[i].xres/yres` est posé (ou qu'un backend graphique pousse
   un `ui_info` — absent en headless) ; sinon le connecteur noyau
   (`Virtual-2`) reste « disconnected ». Syntaxe **JSON** de `-device`
   (la forme plate `outputs.1.xres` n'existe pas en CLI, l'élément
   exige `name`) :
   `-device '{"driver":"virtio-gpu-pci","max_outputs":2,"id":"vgpu","outputs":[{"name":"H0","xres":1280,"yres":800},{"name":"H1","xres":1280,"yres":800}]}'`
3. **Canal pointeur PS/2 : petits pas uniquement.** Les deltas rel
   sont livrés 1:1 (`delta_unaccel`, sans accélération) MAIS la file
   PS/2 de QEMU ne fait que 16 octets et les paquets sont serrés à
   ±127 (`hw/input/ps2.c`) : un grand delta en un évènement se
   bufferise et se rejoue hors phase (pointeur qui « rate » x=1280 ou
   retombe sur une position erronée). Recette : reset ET déplacement
   en pas de ~66 px espacés de 40 ms (`_goto` du harnais).

### Résultats (`t-yaor-out2`, vert — 16/16 + 6/6)

- **2 sorties** : connecteurs KMS `Virtual-1`/`Virtual-2` connected,
  2 globales `wl_output` énumérées, noms vérifiés, mode 1280x800@75 ;
- **tiling indépendant par tête** : tête 0 — maître vert [0..640] /
  pile orange [640..1280] (20 560/20 192 px) ; tête 1 — même
  disposition locale (20 256/20 192 px) ; journal « tiling : géométrie
  640x800 à (1280,0)/(1920,0) » pour les fenêtres de la tête 1 ;
- **adoption au pointeur trans-sortie** (`t-probe-ptr`) : pointeur
  tête 1 → fenêtres de la tête 1, tête 0 vide ; pointeur tête 0 →
  tête 0 tuilée, tête 1 intacte — **pas de mirroring** ;
- **clic de focus trans-sortie** (`t-probe-click`) : clic sur la pile
  de B (tête 1) → `keyboard enter` dans le journal de B ;
- **swap trans-sortie** (Mod+Tab sur W4, pile de la tête 1) :
  journal « raccourci : maître trans-sortie — les fenêtres échangent
  de sortie » ; **tête 0 tout orange** (19 984/20 384, plus aucun
  vert), **tête 1 tout vert** (20 400/20 720, plus aucun orange) —
  chaque fenêtre est devenue maître de sa nouvelle sortie ;
- **sortie propre** : Mod+Shift+E → processus arrêté, socket retirée,
  console restituée, maître DRM rendu sans erreur (6/6).

Régression single-sortie sur le même binaire : `t-yaor-tiling`
26/26 + 6/6 (le branchement même-sortie de Mod+Tab est inchangé).

### Non couverts (v0.6+)

Multi-écran étendu (≥ 3 sorties), raccourcis focus suivant/précédent
et swap Mod+Shift+Left/Right (mêmes mécanismes que Mod+Tab, couverts
unitairement par `parse_shortcut`), X11 dans le tiling (fenêtres
Xwayland flottantes en v0.4-étape-1).

## v0.6 étape 1 — socle layer-shell + yaor-panel (TEST-UI-001)

### Leçons de diagnostic (v0.6 — panel)

Trois leçons, acquises au prix de quatre runs QA ; à réutiliser :

1. **Configure initial d'une layer surface** : un client layer-shell
   n'est JAMAIS configuré spontanément — le compositor doit envoyer le
   configure **en réponse au premier commit** (modèle
   `ensure_initial_configure` d'anvil, copié dans `shell.rs`). Sans
   lui : la barre ne rend pas, le client attend indéfiniment
   (symptôme : run QA 1 à 13/22, « menu ouvert » sans « menu
   configuré »).
2. **Géométrie des layers : marge ET zone exclusive s'empilent.** Le
   layer map Smithay soustrait DÉJÀ la zone exclusive des couches
   inférieures ; un `set_margin(BAR_H, …)` du volet la rajoutait
   (volet à y=48 au lieu de 24). Les clics d'entrée du menu tombaient
   alors dans le vide ou sur la mauvaise entrée — et le « décrochage
   de 15 s » du panel présumé pendant trois runs n'a **jamais existé** :
   les horodatages uptime invité (panel + compositor) ont montré des
   clics traités à la seconde, simplement pas là où on les croyait.
   Correctif : `set_margin(0, 0, 0, 4)` — délai clic → app prête
   mesuré à 2,7 s.
3. **Console série QEMU : octets dupliqués sur les lignes longues.**
   L'écho tty révèle des duplications (« eccho », « 2>&11 »,
   « prêt ((W1) ») — un motif de grep long est donc suspect. Règle :
   écrire la sonde DANS l'invité par courtes lignes (< 75 caractères),
   motifs sans accent ni parenthèse, horodatage en `/proc/uptime` ;
   un motif accentué n'est fiable que si l'écho est vérifié.
   Diagnostique aussi : SlotPool — pré-dimensionner (2×barre + menu)
   plutôt que laisser croître (set_len + remap mmap +
   `wl_shm_pool.resize` à chaque besoin).

### Résultats (`t-yaor-panel`, vert — 22/22 + 4/4)

Banc : QEMU virtio-gpu 2 têtes 1280x800 (`-vga none`, VGPU_JSON),
session KMS + seatd, panel lancé à la main avec env hérité
(`WAYLAND_DISPLAY` + `DISPLAY=:0`). Palette de détection : barre/volet
(32,51,77) — `_is_bar` serré (b<130 exclut le bleu xwtest),
vert/orange xwtest, fond (204,204,229) ; ~20 000 px par demi-tête.

- **barre layer-shell** : rendue en haut de la tête 0 (25 396 px) et
  PAS sur la tête 1 (panneau = première sortie, Virtual-1) ; configure
  layer-shell reçu (journal « barre configurée 1280x24 ») ;
- **zone exclusive** : preuve différentielle — tête 0 : maître vert
  jamais au-dessus de y=22 ; tête 1 (sans panneau) : maître vert
  jusqu'à y=0 ; swap trans-sortie intact ;
- **volet menu Overlay** : clic bouton (20,12) → volet affiché sous la
  barre (5 184 px mesurés au screendump) ; clavier exclusif : Escape
  referme le volet et le panel journalise « menu fermé » (la touche ne
  pouvait atteindre le client QUE par le routage compositor) ;
- **lancement d'apps par le menu** : entrée WAYTEST → waytest lancé
  par le panel (« lancement waytest (pid …) »), 2 fenêtres de plus
  (0 → 2), tuilées sous la barre (640x388) ; entrée XWTEST → client
  X11 lancé via Xwayland, fenêtre bleue rendue sous la barre
  (161 746 px) ; délai clic → app prête 2,7 s (sonde uptime invité) ;
- **sortie propre** : Mod+Shift+E → compositor arrêté, panel détecte
  la coupure de socket (« arrêt »), socket retirée, maître DRM rendu
  sans erreur (4/4).

Régressions obligatoires vertes sur le même ISO : `t-yaor-tiling`
26/26 + 6/6, `t-yaor-comp` 16/16, `t-yaor-comp-wm` 23/23 ×2 (virtio +
simpledrm), `t-yaor-comp-xwl` 11/11, `t-yaor-sess` 6/6.
