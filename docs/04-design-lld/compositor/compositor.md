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
├── policy.rs      # politique de fenêtres (placement en cascade) — ancre v1
├── grabs.rs       # move/resize interactifs (référence : anvil/smallvil)
├── xway.rs        # démarrage/arrêt XWayland + surfaces X11 (stub v0.4 étape 2)
└── config.rs      # façade de configuration minimale (stub ; Desktop Core v0.5)
```

Détail par module :

- **session.rs** — l'acquisition du siège est faite par `libseat` (`seatd` dans le
  live v0.3 ; le chemin **logind** de l'ADR-004 est branché à l'étape « sessions »).
  La session prend le master KMS, énumère les sorties (`display-info` via
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
  L'ordre est imposé par le code, pas par l'ordre des champs : seatd révoque
  le maître DRM dès que le client libseat se ferme. La restauration de
  l'instantané fbcon capturé à l'ouverture échoue (EINVAL) — limite connue de
  Smithay, sans effet sur la remise du maître (le fd est fermé sans erreur) ni
  sur la console, que la QA vérifie au scanout après sortie.
- **state.rs** — état partagé `Yaor` : globals (compositor, xdg-shell, shm, output,
  seat, data device, dmabuf), `Space` des fenêtres, `PopupManager`, position du
  pointeur, socket Wayland ; helpers de feedback de présentation et de scan-out
  primaire (transposition d'anvil, sans layer-shell).
- **render.rs** — assemblage des éléments d'une sortie : fenêtres de l'espace +
  curseur. Le curseur est rendu par le compositor (pas un client), en **buffer
  mémoire statique** (pas de thème xcursor au v0.4) ; le rythme vient des vblanks.
- **input.rs** — flux libinput : clavier, pointeur relatif, boutons, axes ; focus
  par surface sous le curseur (modèle smallvil/anvil).
- **shell.rs** — surfaces xdg-shell (toplevel/popup) insérées dans un `Space`
  Smithay ; mappage sur ack de configure, popup positionnée par le compositor.
- **grabs.rs** — requêtes xdg interactives `xdg_toplevel_move/resize` armées sur
  le vérificateur `pointer.has_grab(serial)` (sérial de la pression, chaîné au
  grab implicite) — transposé de la référence anvil `shell/xdg.rs`
  (`PointerMoveSurfaceGrab` / `PointerResizeSurfaceGrab`, vérifiées en QA v0.3).
- **policy.rs** — v0.4 : **empilement simple** (stacking) : placement en cascade,
  fenêtre active au sommet (stacking order du `Space`) ; le découpage tiling
  complet relève du Desktop Core v0.5 et des plugins v0.7 (ADR-000 décision 12 :
  le compositor garde le mécanisme, le Desktop Core branche la politique).
- **config.rs** — lecture d'un toml minimal (curseur, placement) ; la
  configuration complète sera branchée par `config-engine` v0.5.

## Gestion des fenêtres (ADR-005 — pas de WM distinct)

- tout mappage/unmapping/close passe par le compositor (pas de processus séparé) ;
- interactifs xdg : move/resize par la voie v0.3 (grabs armés côté serveur) ;
- fenêtre active = focus clavier ; visible = sommet du stacking ;
- close : délégation au client (`xdg_toplevel.close`) puis purge au délai.

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

## QA (TEST-GFX-003, v0.4)

- Harnais : `t-yaor-comp.py`, réutilisation de `t-anvil` (boot live, injections
  QMP, greps invités CNT-, screendumps QMP) et des leçons PS/2 v0.4 (alternance
  de boutons, reprises par round).
- Étape « session » (courante) : binaire/usage/version, seatd actif, session KMS
  tenue 12 s, socket Wayland publiée, client `waytest` connecté et fenêtre mappée
  (screendump : contenu client sur fond du compositor), sortie au SIGTERM.
- Étapes suivantes : fenêtres xdg (focus, move/resize), double fenêtre (disposition),
  fenêtre X11 (XWayland) — complètent TEST-GFX-003.
- Critère ouvert assumé : le canal PS/2 sacrifie occasionnellement un événement
  de bouton (leçon v0.4) ; la QA applique l'alternance + les reprises.

## Hors périmètre v0.4

- Décors de fenêtres (relèvent du Desktop Core et des couches plugins, v0.5+) ; profils
  Caméléon (v0.6) ; tiling complet (v0.5+) ; accessibilité AT-SPI (v0.8, ADR-006) ;
  multi-écran étendu (≥ 2 sorties) reporté.
