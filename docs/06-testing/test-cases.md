# Test Cases

## TEST-SYS-001 — Linux Kernel
**Vérifie :** REQ-SW-001

Construire l'image, démarrer dans QEMU et vérifier que le noyau démarre.

## TEST-ARCH-001 — Réutilisation
**Vérifie :** REQ-SW-002

Examiner les composants sélectionnés et vérifier qu'une solution existante adaptée est utilisée lorsqu'elle est disponible.

## TEST-ARCH-002 — Préférence Rust
**Vérifie :** REQ-SW-003

Recenser les composants retenus et vérifier que, pour chaque besoin où une implémentation
Rust mature existe, celle-ci est privilégiée ou le choix contraire est justifié par un ADR.

## TEST-ARCH-003 — Justification des réimplémentations
**Vérifie :** REQ-SW-004

Recenser les composants développés par YAOR et vérifier que chacun remplace une solution
existante de façon documentée et justifiée (ADR, `component-selection`).

## TEST-ARCH-004 — Remplaçabilité
**Vérifie :** REQ-SW-014

Remplacer un composant externe (par exemple le fournisseur DNS) et vérifier qu'aucune autre
couche ne nécessite de modification majeure.

## TEST-GFX-001 — Compositor
**Vérifie :** REQ-SW-005

Démarrer l'environnement graphique, ouvrir une fenêtre, puis la déplacer et la redimensionner.

Statut v0.3 (QA `t-anvil.py`, build ISO courante) : fenêtre ouverte ✓ (client
`waytest` mappé, screendump avec contenu client, focus clavier au clic) ;
entrées livrées ✓ (injection QMP PS/2 + touches) ;
déplacement/redimensionnement ✓ — anvil arme ses grabs serveur sur les
requêtes xdg interactives du client (alternance : 1re pression interactive
livrée = `move`, 2e = `resize` ; run vert virtio : 507 px de déplacement
mesuré au scanout + 8 buffers recréés ; repli simpledrm : 13/13).

## TEST-GFX-002 — Pile graphique et session de référence (v0.3)
**Vérifie :** ADR-024, REQ-SW-005 (partie pile)

Sous QEMU : le KMS prend l'affichage au boot (fbcon) ; la session de référence
(compositor d'exemple Smithay) s'ouvre sur KMS, initialise une surface EGL/GLES2 via
mesa (softpipe), reçoit les événements libinput (déplacement d'un objet au
pointeur, frappe clavier) et redonne proprement la main à la console (bascule VT).
La validation couvre virtio-gpu et le repli `simpledrm`.

Statut v0.3 : **vert** — `t-gfx-live.py` 8/8, `t-anvil.py` 12/12 (virtio),
repli `t-anvil-simpledrm.py` 13/13 (le rendu est softpipe, cf.
`graphics-tests.md`) ; les événements livrés au client de validation
`waytest` font foi pour le chemin libinput → compositor → client.

## TEST-GFX-003 — Compositor YAOR (v0.4)
**Vérifie :** ADR-005, REQ-SW-005 (partie compositor), ADR-022

Le compositor propre à YAOR (`yaor-compositor`) est livré dans le live et tient
une session KMS réelle : siège obtenu via seatd/libseat, maître DRM, socket
Wayland publiée, client `waytest` connecté et fenêtre mappée au scanout, sortie
propre sur SIGTERM avec restauration de l'état CRTC et remise du maître.

Statut v0.4 (étape 2, session) : **vert** — `t-yaor-comp.py` 16/16 (virtio-gpu)
et `t-yaor-comp-simpledrm.py` 18/18 (repli `-vga std`), RC=0 : session KMS,
socket Wayland, client mappé au scanout, console restituée après SIGTERM
(screendump fbcon) et maître DRM rendu sans erreur de permission.

Statut v0.4 (étape 3, gestion des fenêtres) : **vert** — harnais dédié
`t-yaor-comp-wm.py` **23/23** (virtio) et repli simpledrm **23/23** :
deux fenêtres `waytest --two` (W1 vert / W2 orange) à la taille
initiale 640x480 posée par le compositor, focus clavier au clic
(touches livrées à la fenêtre cliquée, jamais aux autres), raise au
clic (pixels de la fenêtre du fond dans la zone de chevauchement),
`xdg_toplevel_move` + glisser (bbox déplacée de 644 px au screendump)
et `xdg_toplevel_resize` + glisser (buffer client recréé à la taille
mesurée 794x589). Détails et triage dans `graphics-tests.md`.

## TEST-GFX-004 — Compatibilité X11 via XWayland (v0.4)
**Vérifie :** ADR-005, REQ-SW-005 (partie compositor), ADR-024

Chaîne X épinglée (14 archives, cf. `sources/manifest.tsv`) et phases chroot
`ch7.*` sans réseau ; le compositor démarre Xwayland, joue le gestionnaire
X11 (`XwmHandler`) et une fenêtre X11 de test (`xwtest`, libX11 seule) est
rendue au scanout, cliquable (ButtonPress livré au client) et livrée clavier
(focus par clic → `SetInputFocus`/`WM_TAKE_FOCUS`).

Statut v0.4 (étape 4) : **vert** — `t-yaor-comp-xwl.py` **11/11, RC=0**
(virtio-gpu) : `WM X11 attaché (display :0)`, socket `X0` + verrou publié,
`xwtest: MAP id=0x400001`, px bleus + carré blanc au screendump, `BTN n=1`,
`KEY keysym=0x61|0x62|0x63`, sortie propre sur la touche q. Détails et triage
dans `graphics-tests.md`.

## TEST-AUD-001 — Audio (PipeWire + WirePlumber)
**Vérifie :** ADR-024

`pipewire` assure le serveur, `wireplumber` la politique : un son synthétique joué
via `pw-cat` sort sur la carte HDA générique (QEMU `intel-hda`) et `wpctl` liste la
sortie active (routage ALSA par PipeWire).

## TEST-DESK-001 — Configuration
**Vérifie :** REQ-SW-006

Modifier `desktop.toml`, recharger le bureau et vérifier que le comportement change.

## TEST-DESK-002 — Profils
**Vérifie :** REQ-SW-007

Sélectionner chaque profil et vérifier les politiques d'interface correspondantes.

## TEST-PLUGIN-001 — Plugin
**Vérifie :** REQ-SW-011

Installer un plugin valide, charger le Desktop Core et vérifier qu'il est actif.

## TEST-A11Y-001 — Accessibility
**Vérifie :** REQ-SW-012

Activer l'accessibilité, cibler un élément accessible et vérifier que ses informations sont transmises au moteur.

## TEST-A11Y-002 — Mooré
**Vérifie :** REQ-SW-013

Fournir un texte compatible, activer la sortie Mooré et vérifier la génération vocale lorsque le moteur TTS est disponible.

## TEST-NET-001 — Réseau filaire (DHCP)
**Vérifie :** REQ-SW-024

Démarrer le système avec une interface filaire (virtio-net ou carte réelle) reliée à un
réseau DHCP : `networkctl status` signale l'interface `routable`, l'adresse obtenue est
joignable et la passerelle répond.

## TEST-NET-002 — Résolution de noms
**Vérifie :** REQ-SW-024

Avec `systemd-resolved` actif, résoudre un nom d'hôte (`resolvectl query <nom>`) et
vérifier que l'adresse retournée est correcte.

## TEST-NET-003 — Wi-Fi (iwd)
**Vérifie :** REQ-SW-024

Sur du matériel Wi-Fi dont le microprogramme est présent, connecter une interface via
`iwctl` (`station <iface> connect <SSID>`) puis vérifier l'obtention d'une adresse et la
connectivité.

## TEST-SESS-001 — Bus système et logind
**Vérifie :** REQ-SW-021

Démarrer le système et vérifier que `dbus` et `systemd-logind` sont `active (running)` ;
après une connexion, `loginctl` liste la session correspondante.

## TEST-SESS-002 — Multi-utilisateur
**Vérifie :** REQ-SW-023

Créer un compte (`useradd -m`), ouvrir une session via `getty`/`login` sur une console,
puis vérifier le répertoire personnel, les permissions du compte et l'enregistrement de la
session par `loginctl`.

## TEST-SESS-003 — Élévation de privilèges
**Vérifie :** REQ-SW-023

Depuis un compte non privilégié, exécuter une commande via `sudo` (sudo-rs) et vérifier
que l'élévation est effective après authentification.

## TEST-STOR-001 — Montage automatique
**Vérifie :** REQ-SW-025

Attacher un volume amovible (clé USB ou périphérique virtuel) formaté : `udisks2` le monte
automatiquement sous `/run/media/<utilisateur>/<étiquette>` et son contenu est accessible
en lecture/écriture.

## TEST-STOR-002 — Systèmes de fichiers
**Vérifie :** REQ-SW-025

Formater puis monter en lecture/écriture des volumes **ext4, btrfs, vfat, exFAT et NTFS**,
et vérifier la création puis la relecture d'un fichier sur chacun.

## TEST-INST-001 — Démarrage installé (BIOS et UEFI)
**Vérifie :** REQ-SW-019

Installer le système sur un disque (virtuel) via `yaor-install`, retirer le média live et
démarrer le disque en **BIOS** puis en **UEFI** : le système installé atteint `multi-user.target`.

## TEST-INST-002 — Persistance des données
**Vérifie :** REQ-SW-026

Créer des données dans `/var` (et `/home`), appliquer une mise à jour par image, puis
vérifier que les données sont **intactes** et que la nouvelle version est active.

## TEST-UPD-001 — Mise à jour A/B et retour arrière
**Vérifie :** REQ-SW-020

Appliquer une mise à jour (slot inactif), démarrer le nouveau slot et vérifier la promotion ;
provoquer ensuite un échec de confirmation et vérifier le **retour arrière automatique** vers
le slot précédent. Vérifier aussi que le **noyau** de chaque slot (`yaor/vmlinuz-<v>` sur
l'ESP) a été mis à jour avec son image et que l'entrée Limine repointe à la fois `path:` et
`cmdline: root=` vers la version du slot démarré.

## TEST-SEC-001 — Intégrité des sources et des images
**Vérifie :** REQ-SW-022

Altérer une source (empreinte SHA-256) puis une image de mise à jour (somme SHA-256) et
vérifier que le build et `systemd-sysupdate` **refusent** l'élément non conforme. La
vérification porte sur la somme du manifeste `SHA256SUMS` (`Verify=no` explicite : le
digest de l'artefact reste inconditionnel, sa signature GPG est un point ouvert
ADR-007). Le slot visé reste à l'état « partial » de sysupdate (étiquette cible posée,
type GPT dérivé) — la version altérée n'est **pas** installée ; le système actif est
inchangé.

## TEST-CONFIG-001 — Configuration déclarative
**Vérifie :** REQ-SW-015

Modifier un fichier de configuration déclarative, recharger et vérifier que le changement
prend effet sans recompilation.

## TEST-BUILD-001 — Reproductibilité fonctionnelle
**Vérifie :** REQ-SW-016

Vérifier que chaque source est épinglée (version et empreinte SHA-256) au manifeste et que,
pour une même révision, le build reproduit un système fonctionnellement identique.

## TEST-BOOT-001 — Démarrage de l'ISO (BIOS et UEFI)
**Vérifie :** REQ-SW-017

Démarrer l'ISO live dans QEMU en **BIOS** puis en **UEFI** et vérifier que le système atteint
`multi-user.target`.

## TEST-TRACE-001 — Traçabilité
**Vérifie :** REQ-SW-018

Vérifier que chaque exigence de la matrice pointe un cas de test défini et que chaque cas de
test référence une exigence (analyse croisée des deux documents).

## TEST-UI-001 — Profil classique
**Vérifie :** REQ-SW-008

Activer le profil classique et vérifier l'organisation de bureau attendue (barre des tâches,
menu de démarrage).

## TEST-UI-002 — Profil macOS-like
**Vérifie :** REQ-SW-009

Activer le profil « barre de menus supérieure + dock » et vérifier la disposition.

## TEST-UI-003 — Profil tiling
**Vérifie :** REQ-SW-010

Activer le profil en mosaïque, ouvrir plusieurs fenêtres et vérifier leur disposition.

## Harnais d'exécution QA

Les cas exécutables en machine virtuelle sont automatisés par le harnais
`/home/gerard/yaor-qa` (hors dépôt) : `yaexpect.py` pilote la console série de
QEMU (`-serial stdio`, autologin `root` sur `ttyS0`, marqueurs `QAEND-N`) et
`t-*.py` enchaînent les scénarios (`t-boot` : TEST-BOOT-001 ; `t-net`/`t-sess`/
`t-stor` : réseau, sessions, stockage ; `t-install` : TEST-INST-001/002 ;
`t-update` : TEST-UPD-001 ; `t-sec-img` : TEST-SEC-001 partie image). Les
artefacts de mise à jour sont servis par un serveur HTTP de l'hôte
(`qa_http.py`, écarts QA documentés dans `../04-design-lld/installer/installer.md`,
« Points ouverts »).

Règles d'exécution établies :

- la console série est lente (~1 o/s) : les commandes invité produisent des
  **sorties compactes** (un chiffre, une ligne) et le parse des chiffres
  tolère les séquences ANSI/CR du tty ;
- après un reformattage GPT massif (écriture de 2 Gio par sysupdate), les
  outils invités s'appuyant sur udev (`lsblk`) peuvent suspendre : le slot
  écrit est déduit **côté hôte** du stdout de sysupdate
  (« as /proc/self/fd/3pN », N = index GPT : 3 = A, 4 = B) ;
- après un reboot invité suivi en session, l'attente porte sur le marqueur
  **unique par démarrage** (ligne d'autologin agetty), pas sur une invite
  déjà présente dans le tampon ;
- un update interrompu laisse un slot « partial » compté comme instance
  disponible (« already acquired ») : `yaor-slot update` ordonne
  `systemd-sysupdate vacuum` avant chaque mise à jour (les versions
  installées sont conservées, `transfer_vacuum` 262).

