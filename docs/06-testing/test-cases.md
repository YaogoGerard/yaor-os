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
le slot précédent.

## TEST-SEC-001 — Intégrité des sources et des images
**Vérifie :** REQ-SW-022

Altérer une source (empreinte SHA-256) puis une image de mise à jour (signature) et vérifier
que le build et `systemd-sysupdate` **refusent** l'élément non conforme, sans modifier le système.

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

