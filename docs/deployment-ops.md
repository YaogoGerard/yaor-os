# Déploiement et opérations — yaor-os

**Statut :** Brouillon
**Version :** 0.2

## 1. Pipeline CI/CD

- **Plateforme :** GitHub Actions
- **Type de runner :** `ubuntu-latest` (VM complète — nécessaire pour `chroot`,
  `mount`, `losetup`, utilisés lors de la construction du système)
- **Limite de temps par job :** jusqu'à 6h (suffisant pour toolchain + build + ISO)

## 2. Étapes du pipeline (stages)

```
download → toolchain → chroot-build → initramfs → make-iso
```

Chaque étape est un job distinct ; les artefacts intermédiaires (sources, toolchain,
rootfs) sont transmis entre jobs via `actions/upload-artifact` /
`actions/download-artifact`.

## 3. Cache

Les sources téléchargées (`01_download.sh`) sont mises en cache via `actions/cache`,
avec une clé basée sur la liste de versions, pour éviter de re-télécharger à chaque
run. Les dépendances Rust (registre crates.io, utilisées lors de la construction du
userland) suivent la même stratégie : cache clé sur les versions épinglées des
outils.

## 4. Livrable final

L'image ISO est publiée comme artefact GitHub Actions téléchargeable, jamais commitée
dans le dépôt Git (voir RG-004).
