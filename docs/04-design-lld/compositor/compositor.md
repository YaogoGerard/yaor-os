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
| `components/compositor` | sources du dépôt, workspace `components/` | toolchain hôte, hors-ligne |
| XWayland | archive épinglée xorg-server (étape 01) | nouveau fournisseur, uniquement ce paquet |
| wayland-protocols | archive déjà épinglée | XML `xdg-shell`, déjà consommé par waytest |

Contrairement à `yaor-common`/`yaor-slot`/`yaor-install` (zéro dépendance externe),
cette crate dépend de Smithay et consomme le vendoring de l'étape 01
(`CARGO_NET_OFFLINE=true`, ADR-018/019) : la phase de build doit donc tourner dans
l'arbre préparé par 01, sur le modèle de `phase_smithay`.

## Modules de la crate

```
components/compositor/src/
├── main.rs        # parsing d'arguments minimal (--tty-udev), signaux, journal
├── session.rs     # init backend : tty::Session (libseat) + udev (drm/gbm/egl)
├── render.rs      # renderer GLES2 sur buffer GBM ; cadence par frame callback
├── input.rs       # événements libinput → input_handlers ; focus clavier/pointeur
├── shell.rs       # desktop::space + window (xdg_shell) ; mappage/configure
├── policy.rs      # politique de fenêtres (layout, placement) — point d'ancrage v1
├── grabs.rs       # move/resize interactifs (référence : anvil shell/xdg.rs)
├── xway.rs        # démarrage/arrêt XWayland + intégration des surfaces X11
└── config.rs      # façade de configuration minimale (toml ; Desktop Core v0.5)
```

Détail par module :

- **session.rs** — l'acquisition du siège est faite par `libseat` en mode **logind**
  (`systemd-logind` du système, ADR-004) avec repli `seatd` (chemin v0.3 documenté).
  La session prend le master KMS, énumère les sorties (`display-info`), construit
  l'`EGLDisplay` (softpipe en QA). Lancement `--tty-udev`, conforme à anvil.
- **render.rs** — boucle : cadencée par les frame callbacks du scanout (pas de
  timer agressif) ; le curseur est rendu par le compositor (pas un client), à
  minima en sprite logiciel au v0.4.
- **input.rs** — flux libinput : clavier, pointeur, axes ; focus par surface sous
  le curseur ; répétition des touches déléguée au client (répéteur xdg).
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

- phase `compositor` après `phase_smithay` (consomme l'arbre vendored smithay) ;
- build : `cargo build --release --locked -p yaor-compositor` (workspace
  `components/`, monté sur `/build/components` comme les autres crates YAOR) ;
- livré `/usr/bin/yaor-compositor` ; lancement en session utilisateur selon
  ADR-014 (launcher de session, au pari des services existants de anvil).

## QA (TEST-GFX-003, v0.4)

- Harnais : `t-yaor-comp.py`, réutilisation de `t-anvil` (boot live, injections
  QMP, greps invités CNT-, screendumps QMP) et des leçons PS/2 v0.4 (alternance
  de boutons, reprises par round).
- Points de contrôle principaux : session tenue 12 s ; fenêtre xdg de test
  (waytest) mappée ; move/resize (requêtes démarrées + loguées) ; double fenêtre
  xdg (disposition) ; fenêtre X11 (XWayland) mappée ; sortie propre au SIGTERM.
- Critère ouvert assumé : le canal PS/2 sacrifie occasionnellement un événement
  de bouton (leçon v0.4) ; la QA applique l'alternance + les reprises.

## Hors périmètre v0.4

- Décors de fenêtres (relèvent du Desktop Core et des couches plugins, v0.5+) ; profils
  Caméléon (v0.6) ; tiling complet (v0.5+) ; accessibilité AT-SPI (v0.8, ADR-006) ;
  multi-écran étendu (≥ 2 sorties) reporté.
