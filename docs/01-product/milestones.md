# Jalons — YAOR OS

**Statut :** Accepté
**Version :** 0.1
**Dernière mise à jour :** 2026-10-23
**Décisions liées :** ADR-000 (#21), ADR-003, ADR-011, ADR-015, ADR-022

## Objet

Ce document matérialise la décision **ADR-000 #21** (« Jalons numérotés + critères de
sortie »). Il fixe, pour chaque jalon, un **périmètre** et des **critères de sortie
vérifiables** : un jalon n'est déclaré atteint que lorsque **tous** ses critères sont
satisfaits et vérifiés.

Les jalons sont numérotés `v0.x` (alignés sur la roadmap indicative du PRD §7) ; chaque
jalon correspond à une ou plusieurs phases de [`roadmap.md`](roadmap.md).

Un critère de sortie est **vérifiable** s'il peut être constaté par un test, une commande
ou un artefact (image, journal). Les critères d'un jalon lointain ne sont détaillés qu'à
l'approche de ce jalon : seuls `v0.1` et `v0.2` (atteints) sont figés ici.

## Vue d'ensemble

| Jalon | Objet | Phase roadmap | État |
|---|---|---|---|
| v0.1 | Système de construction + boot Linux minimal | Phase 0 — Foundation | Atteint |
| v0.2 | Userland fonctionnel et composants sélectionnés | Phase 1 — System | Atteint |
| v0.3 | Base graphique | Phase 2 — Graphics | Atteint |
| v0.4 | Compositor YAOR | Phase 2 — Graphics | Atteint |
| v0.5 | Desktop Core configurable | Phase 3 — YAOR Desktop | Atteint |
| v0.6 | Profils Caméléon | Phase 4 — Caméléon | En cours |
| v0.7 | Système de plugins | Phase 5 — Extensibilité | À venir |
| v0.8 | Accessibilité | Phase 6 — Accessibility | À venir |
| v1.0 | Système intégré et documenté | Phase 7 — Stabilisation | À venir |

## v0.1 — Système de construction + boot Linux minimal (atteint)

**Périmètre :** build depuis les sources (ADR-001), système de base minimal, noyau Linux,
ISO live amorçable en BIOS et UEFI (ADR-011).

**Critères de sortie (vérifiés) :**

- [x] La toolchain et le système de base se construisent depuis les sources vérifiées
  (SHA-256).
- [x] L'ISO live démarre en BIOS **et** en UEFI sous QEMU jusqu'à un shell.
- [x] `systemd` est le PID 1 du système réel (ADR-004) ; la racine est un `overlayfs` sur
  une EROFS compressée en zstd.
- [x] `systemctl is-system-running` retourne `running` ; `poweroff` est propre.
- [x] L'orchestrateur `scripts/build.sh` enchaîne les étapes 00 → 06 et s'arrête à la
  première erreur.
- [x] Une empreinte SHA-256 de l'ISO est produite (la signature GPG reste à activer,
  ADR-007).

## v0.2 — Userland fonctionnel et composants sélectionnés (atteint)

**Périmètre :** rendre le système utilisable au quotidien en mode **live** et **installé** :
réseau, sessions et comptes, firmware, stockage amovible, toolchain Rust, cycle de vie
installé et mises à jour par image. Correspond à la Phase 1 « System ».

**Critères de sortie :**

### Construction et toolchain

- [x] La toolchain Rust (`rustc`/`cargo` de l'hôte) est un **prérequis de build** vérifié à
  l'étape 00 ; aucun `rustc`/LLVM n'est embarqué (ADR-019, remplace en partie ADR-012).
- [x] `uutils/coreutils` remplace GNU coreutils dans l'image finale (ADR-012) : aucun
  binaire portant la chaîne « GNU coreutils » ne subsiste dans `/usr/bin` (contrôlé aux
  étapes 04 et 06).
- [x] `uutils/findutils` remplace GNU findutils (ADR-017) : aucun binaire portant
  « GNU findutils » ne subsiste ; un besoin n'a **qu'un seul fournisseur**.
- [x] Les dépendances cargo sont **vendues au téléchargement** (`cargo vendor`, étape 01)
  et les composants Rust se compilent **hors-ligne** (`CARGO_NET_OFFLINE=true`,
  ADR-018/ADR-019).
- [x] Le build est **reproductible au sens fonctionnel** : versions et empreintes épinglées,
  même résultat pour une même révision (ADR-016, REQ-SW-016).

### Réseau (ADR-013)

- [x] Le réseau filaire est opérationnel via `systemd-networkd` (DHCP), la résolution via
  `systemd-resolved`.
- [x] Le Wi-Fi via `iwd` et `linux-firmware` est embarqué (ADR-013, ADR-020) ; la
  validation sur matériel réel reste à planifier (la QA sous QEMU n'expose pas de
  radio Wi-Fi).
- [x] Aucun démon réseau secondaire (NetworkManager) n'est embarqué.

### Sessions, comptes et services (ADR-014)

- [x] D-Bus (bus système et utilisateur) est présent ; `systemd-logind` est réactivé et
  opérationnel (plus masqué).
- [x] Le système est multi-utilisateur POSIX (REQ-SW-015, REQ-SW-023) : création de
  comptes, `/home`, `getty`/`login`, sessions suivies par `logind` (REQ-SW-021).
- [x] L'élévation de privilèges fonctionne via `sudo-rs` (ADR-012).

### Stockage (ADR-015)

- [x] Le montage automatique des volumes amovibles fonctionne via `udisks2`.
- [x] Les systèmes de fichiers `ext4`, `btrfs`, `vfat`, `exFAT` et `NTFS`
  (lecture/écriture) sont supportés.
- [x] Le chiffrement LUKS reste **hors** périmètre de v0.2 (reporté).

### Mode installé et mises à jour (ADR-015)

- [x] Un système installé sur disque démarre sans le média live, en BIOS **et** UEFI
  (REQ-SW-019).
- [x] Les mises à jour se font par image, avec deux slots A/B et retour arrière, via
  `systemd-sysupdate` (REQ-SW-020).

### Tests et traçabilité (REQ-SW-018)

- [x] Les cas de test manquants sont rédigés et la matrice de traçabilité
  (`requirements-traceability.md`) couvre les exigences du jalon.
- [x] Les tests réseau, sessions, stockage, installation et mise à jour sont exécutés et
  passent.

### Documentation

- [x] Les ADR, la sélection des composants, l'architecture de déploiement et les services
  système reflètent l'état atteint.

## v0.3 — Base graphique

**Périmètre :** la pile sous le compositor — rendu GPU (mesa + libdrm), protocoles
Wayland (libwayland), périphériques d'entrée (libinput + libevdev) et audio
(pipewire + wireplumber) — validée sous QEMU par la session Wayland de référence
(ADR-024). Le compositor YAOR, les profils et XWayland relèvent de v0.4 (ADR-005).

**Critères de sortie :**

### Kernel et affichage

- [x] Le KMS prend l'affichage nativement dès le boot (fbcon sur `simpledrm` /
  `virtio-gpu`) ; les consoles tty restent accessibles (bascule VT propre).

### Pile userland (ADR-024)

- [x] `mesa` (EGL/GLES2, softpipe — llvmpipe écarté du live sans LLVM) et
  `libdrm` épinglés dans `sources/manifest.tsv` (SHA-256), construits
  hors-ligne ; une surface EGL s'initialise et rend (GLES2) — QA anvil :
  EGL 1.5 initialisé sur `PLATFORM_GBM_KHR`.
- [x] `libwayland` (client + serveur) épinglée et construite ; un client de test
  dialogue avec le compositor de validation.
  Preuves : `libwayland` et `wayland-protocols` bâties depuis des archives
  épinglées (un fournisseur, étape 04) ; le client de validation `waytest`
  (`components/waytest/`) se connecte à la socket du compositor, échange
  globals/bindings et reçoit les événements (runs verts `t-anvil`).
- [x] `libinput` (+ `libevdev`) livre les événements clavier/pointeur à la session
  graphique sous QEMU.
  Preuve : injection QMP `input-send-event` → evdev → libinput → anvil →
  waytest (compteurs K/P/B du client + journal `/tmp/wt.log` : 2 clics et
  3 touches livrés dans les runs verts).

### Audio

- [x] `pipewire` daemon + `wireplumber` politique : un son synthétique est audible
  via la carte HDA générique (`pw-cat` → `intel-hda`, QEMU).

### Session graphique de référence

- [x] Le compositor d'exemple Smithay tient une session sur KMS dans le système
  installé : rendu (rectangle + horloge), entrée (déplacement d'un objet), sortie
  propre vers la console ; la validation couvre virtio-gpu (KMS) et le repli
  `simpledrm`.

  Preuves (session anvil) (QA `t-anvil` 12/12 virtio, `t-anvil-simpledrm` 13/13 —
  RC=0) : anvil 0.7.0 tient la session (KMS master via seatd), socket
  `wayland-1` publiée, screendump QMP avec contenu client (client
  `waytest` : rectangle + horloge digitale + compteurs, carré qui suit le
  pointeur) ; entrées réelles injectées par QEMU (QMP `input-send-event`,
  pointeur PS/2) traversant evdev → libinput → anvil → waytest (2 clics,
  3 touches livrés) ; **fenêtre déplacée (507 px, bbox du scanout) et
  redimensionnée (8 buffers recréés) via les requêtes xdg interactives
  déclenchées par waytest** — anvil arme ses grabs serveur côté
  `shell/xdg.rs` ; sortie propre au SIGTERM. Repli `simpledrm` couvert
  (`-vga std`, dmesg + connecteur `Unknown-1`, move/resize aussi verts :
  722 px + buffers). Détails : `docs/06-testing/graphics-tests.md`.

### Tests et traçabilité (REQ-SW-018)

- [x] Les cas TEST-GFX-001/002 et les cas audio sont exécutés et passent ; la
  matrice de traçabilité couvre les exigences graphiques du jalon.

  Preuves (session anvil) : TEST-GFX-002 **vert** (session de référence complète :
  KMS boot, mesa softpipe, session anvil, client Wayland, entrées libinput,
  audio, repli simpledrm) ; TEST-AUD-001 vert ; TEST-GFX-001 **vert**
  (fenêtre ouverte ✓, entrées ✓, déplacer/redimensionner ✓ via les requêtes
  xdg interactives — grabs d'anvil armés côté serveur) ; audio vert. Matrice
  de traçabilité : REQ-SW-005 → TEST-GFX-001 + TEST-GFX-002 (ADR-024).

## v0.4 — Compositor YAOR (atteint)

**Périmètre :** remplacer la session de référence (anvil 0.7) par le compositor YAOR
propre, bâti sur **Smithay** (ADR-005) : gestion des fenêtres intégrée (pas de «
Window Manager » distinct), session via **logind** (ADR-004/005), **XWayland** pour la
compatibilité X11. Rendu logiciel (mesa softpipe) ; les fondamentaux KMS/entrées/mesa
sont établis par v0.3 et réutilisés tels quels.

**Critères de sortie :**

### Crate compositor YAOR

- [x] La crate `components/compositor` (Rust + Smithay, dépendances vendues) est
  intégrée à la chaîne de build (étape 04 ; hors-ligne, toolchain hôte, un besoin un
  fournisseur) et livrée comme binaire du live.

  Preuves : membre du workspace `components/`, bâti par la phase
  `yaor-components` (toolchain Rust hôte, `CARGO_NET_OFFLINE=true`, sources
  vendues de Smithay 0.7.0, `Cargo.lock` committé) ; binaire
  `/usr/bin/yaor-compositor` présent dans le live, `--version` → `0.2.0`
  (ADR-022), `--help` rc=0, argument inconnu rc=2 (QA `t-yaor-comp.py`,
  phase 1 verte).
- [x] Le compositor tient une session KMS (virtio-gpu, repli simpledrm) : socket
  Wayland publiée, screendump avec contenu client, sortie propre au SIGTERM.

  Preuves : `t-yaor-comp.py` **16/16** (virtio-gpu) et
  `t-yaor-comp-simpledrm.py` **18/18** (`-vga std`), RC=0. Session libseat
  sur seatd, maître DRM, EGL `PLATFORM_GBM_KHR`/softpipe, connecteur
  `Virtual-1` (repli `Unknown-1`) ; socket `wayland-1` publiée, client
  `waytest` connecté et fenêtre mappée, screendump 55,4 % clairs /
  44,6 % sombres ; SIGTERM : processus arrêté, socket retirée, console
  restituée (screendump post-sortie 0,2 % clairs / 99,8 % sombres) et maître
  DRM rendu sans erreur de permission (`Dropping device: /dev/dri/card0`).
  Limite connue : Smithay ne peut pas rejouer l'instantané fbcon capturé à
  l'ouverture (EINVAL au drop) — sans effet sur la remise du maître ni sur la
  console. Détails : `docs/06-testing/graphics-tests.md` (TEST-GFX-003,
  étape 2).

### Gestion des fenêtres (ADR-005 — pas de WM distinct)

- [x] Fenêtres xdg-shell : mappage, focus clavier/pointeur, déplacement et
  redimensionnement — vérifiés par une QA dédiée (TEST-GFX-003, sur le modèle de
  `t-anvil`, harnais réutilisant les leçons PS/2 v0.3).

  Preuves : `t-yaor-comp-wm.py` **23/23, RC=0** (virtio-gpu) et repli
  simpledrm **23/23** — deux fenêtres `waytest --two` (W1 vert / W2
  orange) à la taille 640x480 posée par le compositor ; focus au clic
  (`keyboard enter` de la fenêtre cliquée, touches livrées 3+2,
  aucune fuite vers l'autre fenêtre), `xdg_toplevel_move` + glisser
  (bbox 20,20,659,499 → 221,141,860,620, 644 px) et
  `xdg_toplevel_resize` + glisser (buffers client 709x529 → 794x589,
  bbox 221,141,1014,729). Détails : `docs/06-testing/graphics-tests.md`
  (TEST-GFX-003, étape 3).
- [x] Disposition multi-fenêtres minimale : au moins deux fenêtres xdg simultanées
  avec la fenêtre active visible, disposition (découpage ou empilement) documentée
  au LLD `compositor.md`.

  Preuves : les deux fenêtres du run ci-dessus rendues **simultanément**
  au scanout (11 924 px verts / 16 352 px oranges ; bboxes W1
  (20,20,659,499) et W2 (340,220,979,699)) ; fenêtre active visible
  après raise (4 428 px verts, 0 orange dans la zone de chevauchement).
  Empilement en cascade au pas 320x200 documenté au LLD
  `docs/04-design-lld/compositor/compositor.md` (policy.rs, mécanisme et
  politique ; tiling complet reporté au Desktop Core v0.5).

### Compatibilité X11 (ADR-005)

- [x] XWayland intégré (épinglé, construit hors-ligne) ; une fenêtre X11 de test est
  rendue dans la session, cliquable et livrée clavier.

  Preuves (`t-yaor-comp-xwl.py`, vert — 11/11, RC=0, ISO 0.2.0) : le
  compositor publie `WM X11 attaché (display :0)` et la socket
  `/tmp/.X11-unix/X0` (verrou `.X0-lock`) ; la fenêtre X11 de `xwtest`
  (bleu 0x2A6ADB, carré blanc) est mappée côté X (`xwtest: MAP
  id=0x400001`) et rendue au scanout (px bleus > 8 000, carré blanc >
  5 000 px au screendump QMP) ; le clic est livré (`xwtest: BTN n=1`) et
  les touches a/b/c atteignent la fenêtre après focus par clic
  (`xwtest: KEY keysym=0x61|0x62|0x63` — focus posé via la cible
  `FocusTarget::X11`, smithay pose `SetInputFocus`/`WM_TAKE_FOCUS` côté X) ;
  la touche q ferme la fenêtre et xwtest sort proprement. Chaîne X épinglée
  (14 archives SHA-256, `sources/manifest.tsv`), lectures détaillées :
  `docs/06-testing/graphics-tests.md` (TEST-GFX-004), LLD
  `docs/04-design-lld/compositor/compositor.md`.

  Régression au passage : le démontage du compositor retire maintenant
  explicitement la socket Wayland (`t-yaor-comp.py` 16/16).

### Sessions (ADR-004, ADR-005)

- [x] La session graphique acquiert le siège via libseat en mode **logind**
  (systemd-logind actif ; journal + `loginctl` en preuve), en live et installé ;
  le chemin seatd de v0.3 reste documenté comme repli.

  Preuves (QA `t-yaor-sess` **6/6**, RC=0, harnais dédié) : `systemd-logind`,
  `dbus.socket` et `seatd` actifs (`ACT-0`) ; lancement **sans variable** — le
  compositor journalise « backend de siège : logind », résout la session
  logind attachée au siège et reprend son contrôle (socket Wayland publiée,
  aucune session logind de plus, `LB+1/SL-1/ERR-0/N1-3/WSOK+1`) ; SIGTERM
  propre (`AG+1/SG+1/WS0-0`) ; repli explicite `YAOR_LIBSEAT_BACKEND=seatd`
  (journal « backend de siège : seatd », toujours aucune session logind, le
  chemin v0.3 fonctionne, `SV+1/SD+1/NOLEAD-0/N2-3`). Détails : LLD
  `docs/04-design-lld/compositor/compositor.md` (§ session.rs), cas
  TEST-GFX-003 dans `docs/06-testing/graphics-tests.md`.
  **Variante installée** : même rootfs EROFS que le live (yaor-install en
  recopie le contenu) et même binaire/paquet environment — `systemd-logind`
  et `dbus.socket` y sont activés par les mêmes liens posés à l'installation
  (phase 04) ; la couverture TEST-INST-001 (démarrage installé) du jalon v0.3
  s'applique telle quelle, le comportement de session est identique.
  Régression au passage (siège logind par défaut sur toutes les QA) : les
  harnais v0.4 restent verts — `t-yaor-comp` 16/16, `t-yaor-comp-wm` 23/23
  (virtio **et** simpledrm), `t-yaor-comp-xwl` 11/11.

### Tests et traçabilité

- [x] Les cas TEST-GFX-003/004 (+ actualisation TEST-GFX-001/002 si périmètre commun) et
  les critères du compositor sont exécutés et passent ; la matrice de traçabilité
  couvre les exigences compositor du jalon (REQ-SW-005 partie compositor, ADR-005).

  Preuves : TEST-GFX-003 **vert** (session 16/16, gestion des fenêtres
  23/23 virtio **et** simpledrm, sessions logind `t-yaor-sess` 6/6) et
  TEST-GFX-004 **vert** (XWayland 11/11, statuts dans
  `docs/06-testing/test-cases.md`) ; la matrice
  `docs/02-requirements/requirements-traceability.md` couvre REQ-SW-005 par
  parties (compositor → TEST-GFX-003, compatibilité X11 → TEST-GFX-004, en
  plus du socle v0.3 → TEST-GFX-001/002). Périmètre commun de
  TEST-GFX-001/002 assumé par le compositor : la ligne « Statut v0.4 » de
  TEST-GFX-001 (test-cases.md) reporte fenêtre/move-resize/entrées sur la
  QA du compositor ; TEST-GFX-002 (session de référence) ne change pas —
  anvil reste le repli de validation dans le live (ADR-005).

## v0.5 — Desktop Core (ouvert)

**Périmètre (roadmap Phase 3 — YAOR Desktop) :** le bureau lui-même devient
vivable — **tiling maître+pile** dans le compositor, **moteur de
configuration déclaratif** (TOML, `desktop.toml`), **multi-écrans (2
sorties)**. Décisions d'ouverture (faites avec le produit, 28 octobre 2026) :
format **TOML** (`~/.config/yaor/desktop.toml`), disposition **maître + pile**
(fenêtre active principale, pile à droite), **2 sorties** virtio-gpu en QA
(QEMU `-device virtio-gpu` avec 2 têtes), raccourcis clavier du type
**sway/Hyprland** adaptés à l'image v0.4 (pas de terminal applicatif : focus,
swap, close, ratio, quitter). Hors périmètre v0.5 — v0.6 et au-delà :
layer-shell/barre, launcher, espaces de travail, DnD, profils Caméléon,
plugins. La traçabilité cible REQ-SW-006 (bureau configurable,
TEST-DESK-001) et REQ-SW-015 (configuration déclarative, TEST-CONFIG-001) ;
REQ-SW-007 (profils) reste v0.6.

### Tiling (ADR-005 — desktop-core LLD)

- [x] Le layout **maître + pile** dispose automatiquement les fenêtres
  xdg-shell : 2 fenêtres → moitié/moitié ; 3 fenêtres → maître (gauche)
  + pile divisée à droite ; la 4ᵉ s'empile, le maître garde son ratio.
  Preuves : largeurs mesurées au screendump QMP (±4 px) + journalisation
  du compositor (« tiling : … », jetons QA) ;
- [x] Les raccourcis par défaut (Mod =SUPER) pilotent le layout :
  Mod+←/→ focus suivant/précédent, Mod+Shift+←/→ swap de deux fenêtres,
  Mod+Tab échange la fenêtre focusée avec le maître, Mod+Q ferme la
  fenêtre focusée (`xdg_toplevel.close`, client → DestroyWindow
  journalisé), Mod+g / Mod+Shift+g grossissent/rétrécissent la zone
  maître, Mod+Shift+E arrêt propre du compositor (équivalent SIGTERM).
  Preuves : injections clavier QMP, jetons/effets observables (l'image
  n'a pas de terminal applicatif : la preuve passe par les clients
  `waytest`) ;
- [x] La régression v0.4 reste verte sur toute la pile :
  `t-yaor-comp` 16/16, `t-yaor-comp-wm` 23/23 (virtio et simpledrm),
  `t-yaor-comp-xwl` 11/11, `t-yaor-sess` 6/6.

### Configuration (REQ-SW-006, REQ-SW-015 ; LLD config-engine)

- [x] crate `components/config` (membre du workspace, version produit
  0.2.0, ADR-022) lit `desktop.toml` au démarrage du compositor, valeurs
  par défaut appliquées dans l'image si le fichier utilisateur est absent ;
  les dépendances ajoutées (`toml`…) sont vendues (ADR-017/018),
  `--locked` et build hors-ligne maintenus ;
- [x] SIGHUP recharge la configuration **sans quitter la session** :
  comportement modifié à chaud — preuve : changement de ratio/gaps suivi
  d'une disposition re-mesurée au screendump (TEST-DESK-001,
  TEST-CONFIG-001) ;
- [x] Options exposées en v0.5 et vérifiées par changement de valeur :
  ratio maître, espacement (gaps), raccourcis clavier rebindables
  (rebind d'un raccourci reconnu à l'usage) ; (le chemin de la socket,
  non exposé en étape 1, reste un candidat de l'étape multi-écrans si
  utile) ;
- [x] Journal du compositor affiche la configuration chargée (source du
  fichier, options) — preuve QA en jetons.

### Multi-écrans (v0.5 : 2 sorties)

- [x] Lancement du compositor avec 2 sorties virtio-gpu (`-device` avec
  2 têtes) : 2 globales `wl_output` énumérées (connecteurs distincts) ;
  preuves : connecteurs KMS `Virtual-1`/`Virtual-2` « connected »
  (jetons CC), « Creating new wl_output » ×2 au journal, mode
  1280x800@75 par tête. **Recette QEMU 11.1.2 headless** (leçon du
  diagnostic, voir `graphics-tests.md`) : `-vga none` — sinon QEMU
  ajoute un VGA std par défaut, deux cartes DRM coexistent et Smithay
  retient la mauvaise — et `-device` **JSON** déclarant chaque tête
  (`outputs[i].xres/yres`) : une tête n'est « enabled » dans
  GET_DISPLAY_INFO que si elle est déclarée, sinon le connecteur noyau
  reste « disconnected » ;
- [x] Le tiling est indépendant **par sortie** ; les fenêtres naissent
  sur la sortie active (le focus suit le pointeur d'une sortie à
  l'autre) ; preuves : sonde `t-probe-ptr.py` — pointeur tête 1 →
  fenêtres tuilées à (1280,0)/(1920,0), tête 1 verte+orange mesurée,
  tête 0 vide ; pointeur tête 0 → tête 0 tuilée, tête 1 INTACTE
  (pas de mirroring) ; harnais 2 têtes : têtes mesurées au pixel
  (vert/orange ~20 000 px par demi-tête) ;
- [x] **Swap trans-sortie** (Mod+Tab sur une fenêtre de l'autre
  sortie) : les fenêtres maître/focusée échangent de sortie — tête 0
  TOUT orange, tête 1 TOUT vert mesurés au screendump, journal
  « raccourci : maître trans-sortie — les fenêtres échangent de
  sortie » (commit `fdd002a`) ;
- [x] Nouveau harnais QA multi-sorties — `t-yaor-out2.py`
  16/16 mesures + 6/6 sortie (RC=0), sondes `t-probe-ptr.py`
  (adoption par sortie) et `t-probe-click.py` (clic→focus trans-sortie
  prouvé ; leçon PS/2 : deltas rel en petits pas uniquement).

### Tests et traçabilité

- [x] Nouveaux harnais : `t-yaor-tiling.py` (dispositions mesurées),
  `t-yaor-config.py` (rechargement + rebind), `t-yaor-out2.py`
  (multi-sorties, 2 têtes) ; sortie **verte RC=0** pour chacun —
  `t-yaor-tiling.py` 26/26 + 6/6 et `t-yaor-config.py` 20/20
  (QEMU virtio), `t-yaor-out2.py` 16/16 + 6/6 (2 têtes) ;
- [x] La matrice de traçabilité couvre REQ-SW-006 → TEST-DESK-001 et
  REQ-SW-015 → TEST-CONFIG-001 (nouvelles parties REQ notées dans
  `requirements-traceability.md`) ; REQ-SW-007 (profils) explicitement
  repoussé v0.6 ;
- [x] Docs : LLD `desktop-core.md` (layout/raccourcis/config) et
  `config-engine.md` développés à hauteur de l'implémentation,
  `compositor.md` étayé au fil des étapes ; scans Unicode verts.

### Clôture de l'étape 1 (28 octobre 2026)

Étape 1 « tiling maître+pile + moteur de config TOML + raccourcis »
livrée : commits `afbd82e` (crate yaor-config, layout, SIGHUP,
`/etc/yaor/desktop.toml` d'usine), `68238f3` (correctif clavier :
casse du keysym des raccourcis — leçon diag live). QA complète
(ISO `out/yaor-live-x86_64-0.2.0.iso` reconstruite, SHA-256 vérifiée) :
`t-yaor-tiling` 26/26 + 6/6, `t-yaor-config` 20/20, régressions v0.4
obligatoires vertes (16/16, 23/23virtio+simpledrm, 11/11, 6/6).
Reste à v0.5 : multi-écrans (étape 2 — harnais 2 têtes virtio-gpu).

### Clôture de l'étape 2 et de v0.5 (28 octobre 2026)

Étape 2 « multi-écrans (2 sorties) » livrée : commit `fdd002a`
(swap trans-sortie au Mod+Tab, facteur `outputs_of`, adoption des
fenêtres neuves par la sortie sous le pointeur). QA complète (ISO
reconstruite du 28 oct. 04:05, SHA-256 vérifiée) : harnais 2 têtes
`t-yaor-out2` 16/16 + 6/6 — 2 `wl_output`, tiling indépendant par
tête mesuré au screendump QMP, swap trans-sortie avec inversion de
couleurs (tête 0 tout orange / tête 1 tout vert), sortie propre —,
régression single-sortie `t-yaor-tiling` 26/26 + 6/6 sur le même
binaire, régressions v0.4 obligatoires vertes (16/16, 23/23virtio+
simpledrm, 11/11, 6/6). Leçons de diagnostic documentées dans
`graphics-tests.md` (recette QEMU multi-têtes headless, canal PS/2).

**Tous les critères de sortie de v0.5 sont cochés avec preuves**
(tiling maître+pile, raccourcis, configuration déclarative TOML +
SIGHUP, multi-écrans 2 sorties, traçabilité REQ-SW-006/015) :
jalon **v0.5 — Desktop Core** atteint. Enchaînement : v0.6 profils
Caméléon.

## v0.6 → v1.0

Les périmètres sont fixés par [`roadmap.md`](roadmap.md) ; les critères de sortie sont à
détailler à l'ouverture de chaque jalon (profils Caméléon, plugins,
accessibilité, stabilisation).

## v0.6 — Caméléon (ouvert)

**Périmètre (roadmap Phase 4 — Caméléon)** : les trois profils
d'interface — **classique** (panneau principal type barre des tâches +
menu d'applications, fenêtres flottantes), **macOS-like** (barre
supérieure + dock), **tiling** (le socle maître+pile de v0.5, espaces
de travail) — et le **changement de profil**. Conformément à l'HLD
(`desktop-architecture.md`) : les profils sont des **configurations de
l'expérience de bureau, pas trois systèmes indépendants** — un seul
Desktop Core, coordonnant WM et UI Engine, pilote les trois.

**Décisions d'ouverture (faites avec le produit, 28 octobre 2026)** :

1. **Barre/dock/panneau = clients layer-shell dédiés** (ex. `yaor-panel`,
   `yaor-dock`) : binaires Wayland parlant `wlr_layer_shell` au
   compositor — conforme à l'HLD (UI Engine distinct du compositor),
   les thèmes et variantes de panneaux évoluent sans recompiler le
   compositor. Faisabilité vérifiée : le Smithay vendu 0.7 expose
   `WlrLayerShellState`/`layer_map_for_output` (gabarit anvil),
   aucune extension du vendoring (ADR-018) ;
2. **Trois étapes progressives** : étape 1 — socle layer-shell dans le
   compositor (mapping par sortie, zones exclusives soustraites de la
   zone utile du tiling) + profil classique (barre des tâches, menu
   d'applications) ; étape 2 — profil macOS-like (barre supérieure +
   dock) ; étape 3 — changement de profil + espaces de travail du
   profil tiling ;
3. **Menu d'applications / dock peuplés d'apps YAOR** (clients internes
   de démonstration + waytest) — pas de scan `.desktop` en v0.6
   (l'image live n'a pas d'applications tierces à lister) ;
4. **Changement de profil à chaud** via SIGHUP (même mécanisme que le
   rechargement de configuration v0.5, prouvé en QA).

Traçabilité : REQ-SW-007 → TEST-DESK-002, REQ-SW-008 → TEST-UI-001,
REQ-SW-009 → TEST-UI-002, REQ-SW-010 → TEST-UI-003 (parties requises
déjà dans la matrice). Maquettes des profils et identité visuelle
(logo) : à produire en début d'étape 1, dans `docs/05-ui-ux/`
(`mockups/`, `branding/`) — les critères TEST-UI-001/002/003 y
référeront.

### Clôture de v0.4 (28 octobre 2026)

Tous les critères de sortie du jalon sont cochés avec preuves (session KMS,
gestion des fenêtres, XWayland, sessions logind, traçabilité REQ-SW-005).
Étapes de réalisation : étape 1 « crate compositor » (`deb28c3`), étape 2
« session udev/KMS, rendu et entrées » (`46ffefe`), étape 3 « gestion des
fenêtres » (`7c4edd7`), étape 4 « XWayland » (`4ee641b`), étape 5
« sessions logind » (`a3a5501`), étape 6 « traçabilité » (`2c367ce`). ISO de référence :
`yaor-live-x86_64-0.2.0.iso` (0.2.0, ADR-022), empreinte SHA-256 publiée à
côté. Enchaînement : v0.5 Desktop Core (tiling, moteur de configuration,
multi-écrans).
