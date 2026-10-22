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

Session + client + entrées (`t-anvil.py`, vert en v0.4 — 10/10, RC=0) :

- anvil 0.7.0 tient la session (KMS master via seatd), socket `wayland-1`
  publiée sous `/run/user/0`, screendump QMP pendant la session (scanout du
  compositor, rendu softpipe) ;
- **client de validation `waytest`** (sources internes du dépôt,
  `components/waytest/`) : fenêtre xdg-shell rendue via wl_shm (XRGB8888) —
  rectangle bordé, horloge digitale, compteurs d'événements, carré suiveur
  du pointeur ; preuve pixel : screendump post-injection = 456 845 px
  sombres (seuil 50 000) avec horloge qui tourne et carré sous le curseur ;
- **entrées réelles livrées à la session** : injection QEMU via QMP
  `input-send-event` (pointeur PS/2 relatif borné, touches a/b/c) → les
  événements traversent evdev → libinput → anvil → waytest qui les affiche
  (compteurs K/P/B, carré déplacé) et les journalise (`/tmp/wt.log`) :
  4 clics et 3 touches livrés au client dans le run vert (boucle à retour
  d'information : injection cible par cible, comptage en direct) ;
- sortie propre sur SIGTERM (processus disparu, console restituée).

Repli simpledrm (`t-anvil-simpledrm.py`, vert — 11/11, RC=0, `-vga std`) :

- dmesg confirme le pilote `simpledrm` (pas de virtio-gpu) ;
- connecteur KMS `Unknown-1` (au lieu de `Virtual-1`) ;
- même session tenue : anvil rendu logiciel (simpledrm n'expose pas de render
  node), socket publiée, client connecté, entrées livrées (2 clics,
  3 touches), screendump 284 425 px sombres, sortie propre.

## TEST-GFX-001 — Compositor

**Vérifie :** REQ-SW-005

- **Fenêtre ouverte** : ✓ démontré par la QA `t-anvil` (client xdg-shell
  `waytest` mappé, screendump avec contenu client, focus clavier obtenu au
  clic) ;
- **déplacer/redimensionner** : ✗ non couvert par anvil 0.7 — le compositor
  d'exemple n'implémente pas de grabs move/resize (`MoveSurfaceGrab`/`Resize-
  SurfaceGrab` absents de `anvil/src/input_handler.rs`) ; ces gestes sont
  attendus du compositor YAOR (jalons suivants) ;
- **rendu souris/clavier livré** : ✓ démontré par l'injection QMP (les
  événements traversent evdev → libinput → compositor → client).

## Non couvert / à faire (v0.4+)

- TEST-GFX-001 move/resize : nécessite un compositor avec grabs (YAOR).
- Multi-écrans, workspaces, changement de profil graphique, accessibilité.
