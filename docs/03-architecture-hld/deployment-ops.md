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

## Pipeline (scripts)

```text
scripts/00_check_host.sh   -> vérifie les outils hôte
scripts/01_download.sh     -> télécharge et vérifie les sources (SHA-256)
scripts/02_toolchain.sh    -> toolchain temporaire + outils temporaires
scripts/03_chroot_build.sh -> chroot : système de base + systemd + noyau
scripts/04_initramfs.sh    -> initramfs systemd
scripts/05_make_iso.sh     -> EROFS + Limine (BIOS/UEFI) -> ISO signée
```

Chaque étape est idempotente et peut être relancée seule. Les journaux sont écrits dans
`build/logs/<étape>.log`.

## Environnement de développement

- Cible de test initiale : **QEMU** (BIOS et UEFI).
- Le pipeline est d'abord exécutable **en local** (scripts) ; la CI vient ensuite
  (décision 26, ADR-000).

## Reproductibilité

- Les versions et les sources utilisées DOIVENT être explicitement définies.
- Chaque source est vérifiée par empreinte **SHA-256** et les empreintes sont épinglées
  (ADR-007).
- Le fichier `sources/manifest.tsv` liste les sources (colonnes `groupe`, `nom`,
  `version`, `url`, `sha256`). Seul ce fichier est versionné ; les archives elles-mêmes
  sont ignorées par Git.
- Le téléchargement utilise les URL canoniques des amonts (upstream). Un miroir GNU peut être fourni
  pour accélérer sans changer les empreintes :
  `GNU_MIRROR=https://mirrors.kernel.org/gnu ./scripts/01_download.sh`.

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
