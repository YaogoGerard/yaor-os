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

- seatd actif ; le compositor ouvre une session libseat, prend le maître DRM
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
l'étape « gestion des fenêtres » du jalon et seront couverts par des cas
TEST-GFX-003 dédiés.

## TEST-GFX-001 — Compositor

**Vérifie :** REQ-SW-005

- **Fenêtre ouverte** : ✓ démontré par la QA `t-anvil` (client xdg-shell
  `waytest` mappé, screendump avec contenu client, focus clavier obtenu au
  clic) ;
- **déplacer/redimensionner** : ✓ démontré via les requêtes xdg interactives
  standard (`xdg_toplevel_move`/`resize`) reçues par anvil qui arme ses
  grabs `PointerMoveSurfaceGrab`/`PointerResizeSurfaceGrab` (`shell/xdg.rs`)
  et suit le pointeur jusqu'au relâchement — le client `waytest` déclenche
  lui-même ces requêtes (alternance : 1re pression interactive livrée =
  move, 2e = resize) ;
- **rendu souris/clavier livré** : ✓ démontré par l'injection QMP (les
  événements traversent evdev → libinput → compositor → client).

## Non couvert / à faire (v0.4+)

- Multi-écrans, workspaces, changement de profil graphique, accessibilité.
- Stabilisation du canal PS/2 injections boutons (un événement de bouton
  par run parfois sacrifié par evdev/libinput — contourné côté QA par
  l'alternance milieu/droit, les drains et la reprise par round).
