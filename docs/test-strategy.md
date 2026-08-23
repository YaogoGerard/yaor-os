# Stratégie de test — yaor-os

**Statut :** Brouillon
**Version :** 0.1

## 1. Types de tests

| Type | Objectif | Outil |
|---|---|---|
| Vérification d'intégrité | Confirmer que chaque source téléchargée n'est pas corrompue | `sha256sum` |
| Test de build | Confirmer que le pipeline complet s'exécute sans erreur | GitHub Actions (à chaque push) |
| Test de démarrage (boot) | Confirmer que l'ISO démarre réellement | QEMU (BIOS et UEFI) |
| Test de taille | Confirmer que l'ISO reste dans la limite fixée (RG-001) | Vérification automatique post-build |

## 2. Fréquence

- **À chaque push sur la branche principale** : build complet + test de démarrage QEMU
- **À chaque tag de version** : build complet + publication de l'ISO comme release

## 3. Environnement de test

Les tests de démarrage sont réalisés via **QEMU**, en émulant à la fois un démarrage
BIOS (legacy) et un démarrage UEFI (avec OVMF), pour valider REQ-SW-001 sans matériel
physique.
