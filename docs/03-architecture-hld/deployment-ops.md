# Deployment & Operations

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-13
**Décisions liées :** ADR-001, ADR-003, ADR-004, ADR-007

## Modèle de construction

YAOR est construit **depuis les sources** (ADR-001) : aucun paquet binaire d'une
distribution tierce n'est utilisé pour le système de base.

```text
Sources (versions + empreintes épinglées)
  -> Toolchain
  -> Root filesystem
  -> Linux kernel (systemd, ADR-004)
  -> Initramfs
  -> Filesystem image (immuable, ADR-003)
  -> ISO (signée, ADR-007)
```

## Environnement de développement

- Cible de test initiale : **QEMU** (BIOS et UEFI).
- Le pipeline est d'abord exécutable **en local** (scripts) ; la CI vient ensuite
  (décision 26, ADR-000).

## Reproductibilité

- Les versions et les sources utilisées DOIVENT être explicitement définies.
- Chaque source est vérifiée par empreinte **SHA-256** et les empreintes sont épinglées
  (ADR-007).

## Sécurité de l'approvisionnement

- Vérification par empreinte de toutes les sources avant usage.
- Aucun binaire non vérifié n'entre dans le build.
- L'image ISO finale est **signée** (ADR-007).

## Mises à jour

- Modèle **image immuable** : mise à jour par remplacement d'image (A/B ou
  réinstallation), pas de gestionnaire de paquets classique (ADR-003).

## CI (ultérieure)

Une fois le pipeline local stabilisé, la CI devrait vérifier le formatage, exécuter les
tests, construire l'image, tester le boot (BIOS + UEFI) et conserver les artefacts.
