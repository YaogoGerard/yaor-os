# Test Cases

## TEST-SYS-001 — Linux Kernel
**Vérifie :** REQ-SW-001

Construire l'image, démarrer dans QEMU et vérifier que le noyau démarre.

## TEST-ARCH-001 — Réutilisation
**Vérifie :** REQ-SW-002

Examiner les composants sélectionnés et vérifier qu'une solution existante adaptée est utilisée lorsqu'elle est disponible.

## TEST-GFX-001 — Compositor
**Vérifie :** REQ-SW-005

Démarrer l'environnement graphique, ouvrir une fenêtre, puis la déplacer et la redimensionner.

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

