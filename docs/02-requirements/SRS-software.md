# SRS — Software Requirements Specification

**Statut :** Brouillon
**Version :** 0.2
**Dernière mise à jour :** 2026-10-13
**Décisions liées :** ADR-000 à ADR-010

> Les anciennes règles métier (`01-product/rules.md`) sont destinées à être fusionnées
> dans ce document (décision 8 du registre ADR-000).

## REQ-SW-001 — Linux Kernel
Le système DOIT utiliser le noyau Linux comme noyau principal.

## REQ-SW-002 — Réutilisation
Le système DEVRAIT réutiliser des composants open source existants lorsqu'ils répondent au besoin.

## REQ-SW-003 — Préférence Rust
Lorsqu'une implémentation Rust suffisamment adaptée existe, YAOR DEVRAIT la privilégier.

## REQ-SW-004 — Justification des réimplémentations
Tout remplacement par un composant YAOR d'une solution existante DOIT être documenté et justifié.

## REQ-SW-005 — Compositor
YAOR DOIT fournir un compositor Wayland fonctionnel, assurant également la gestion des
fenêtres. Le compositor DEVRAIT être développé en Rust lorsque la base disponible
(par exemple Smithay) satisfait les critères de maturité et de stabilité (ADR-002) ;
sinon un composant existant adapté est utilisé, avec justification dans un ADR.

## REQ-SW-006 — Bureau configurable
Le comportement du bureau DOIT pouvoir être modifié par des fichiers de configuration.

## REQ-SW-007 — Profils
Le bureau DOIT supporter plusieurs profils d'interface.

## REQ-SW-008 — Profil classique
YAOR DOIT pouvoir fournir un profil avec une organisation de bureau classique, de type Windows-like.

## REQ-SW-009 — Profil macOS-like
YAOR DOIT pouvoir fournir un profil avec barre de menus supérieure et dock.

## REQ-SW-010 — Profil tiling
YAOR DOIT pouvoir fournir un profil de gestion des fenêtres en mosaïque.

## REQ-SW-011 — Plugins
Le bureau DEVRAIT fournir une API permettant d'étendre ses fonctionnalités.

## REQ-SW-012 — Accessibilité
YAOR DOIT fournir une architecture d'accessibilité indépendante des profils de bureau.

## REQ-SW-013 — Mooré
Le système DEVRAIT permettre une sortie vocale en Mooré lorsque les composants nécessaires sont disponibles.

## REQ-SW-014 — Remplaçabilité
Les composants externes DEVRAIENT pouvoir être remplacés sans modification majeure des autres couches.

## REQ-SW-015 — Configuration déclarative
Les paramètres de présentation et de comportement exposés par le Desktop Core DEVRAIENT être représentables de manière déclarative.

## REQ-SW-016 — Build reproductible
Le système de construction DEVRAIT être reproductible à partir de sources et versions explicitement définies.

## REQ-SW-017 — BIOS et UEFI
L'image de démarrage DEVRAIT prendre en charge les environnements BIOS et UEFI lorsque techniquement possible.

## REQ-SW-018 — Testabilité
Chaque exigence obligatoire DOIT être associée à au moins un cas de test.

## REQ-SW-019 — Cycle de vie : live et installé
Le système DOIT pouvoir fonctionner en mode live (démarrage depuis une clé USB/CD) ET en
mode installé sur disque (ADR-003).

## REQ-SW-020 — Mises à jour par image immuable
Les mises à jour d'un système installé DOIVENT se faire par remplacement d'image
(mécanisme A/B ou réinstallation), et non par un gestionnaire de paquets classique
(ADR-003).

## REQ-SW-021 — Init et gestion de services
Le système DOIT utiliser systemd comme init et gestionnaire de services (ADR-004).
Une session DOIT s'appuyer sur `logind`.

## REQ-SW-022 — Intégrité des sources et des images
Chaque source DOIT être vérifiée par empreinte SHA-256 avant usage, et l'image ISO
distribuée DOIT être signée (ADR-007).

## REQ-SW-023 — Comptes et sessions
Le système DOIT supporter plusieurs utilisateurs POSIX (comptes, répertoires personnels,
permissions, élévation de privilèges) et des sessions gérées par `logind`.
