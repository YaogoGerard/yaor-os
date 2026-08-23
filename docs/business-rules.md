# Règles métiers — yaor-os

**Statut :** Brouillon
**Version :** 0.2

Ces règles sont des invariants : elles doivent rester vraies quelle que soit la façon
dont le système est implémenté techniquement.

## RG-001 — Taille maximale de l'image live
L'ISO générée DOIT rester utilisable sur une clé USB de faible capacité (contrainte
courante dans le contexte visé). Cible : moins de 1 Go pour la version CLI minimale.

## RG-002 — Aucune dépendance réseau au démarrage
Le système live DOIT pouvoir démarrer et être pleinement utilisable sans connexion
Internet, une fois l'ISO écrite sur la clé USB.

## RG-003 — Toolchain de construction en GNU classique
La toolchain utilisée pour construire le système (chapitres 5-6 façon LFS) DOIT
rester en GNU classique (GCC, Glibc, Binutils), car un compilateur Rust ne peut pas
s'auto-amorcer sans un système GNU fonctionnel au préalable. Cette règle porte
uniquement sur la construction ; le userland exécuté par le système live est, lui,
en outils Rust (voir RG-007).

## RG-004 — Rien de volumineux n'est versionné dans Git
Le rootfs, le fichier `.squashfs` et l'image `.iso` sont des artefacts générés par le
pipeline de build ; ils ne DOIVENT jamais être commités dans le dépôt Git.

## RG-005 — Double compatibilité de démarrage
L'image ISO DOIT pouvoir démarrer aussi bien en BIOS (legacy) qu'en UEFI.

## RG-006 — Aliases réservés au shell interactif
Les équivalences vers les outils Rust ne s'appliquent qu'au shell interactif
(`/etc/profile.d/`). Les scripts système et les contextes non interactifs utilisent
toujours les binaires réels présents sur le disque.

## RG-007 — Aucun dépaysement utilisateur
Les commandes classiques restent disponibles sous leurs noms habituels :
uutils/coreutils remplace coreutils nativement, et chaque outil Rust complémentaire
est accessible via le nom classique qu'il remplace. Un contournement explicite
(`\outil`, chemin absolu) doit toujours rester possible.
