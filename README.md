# yaor-os

Mini-distribution GNU/Linux **live** construite intégralement depuis les sources
(façon Linux From Scratch) : CLI minimaliste, bootable depuis une clé USB
(BIOS + UEFI), utilisable sans installation ni connexion Internet.

Son userland est « oxidized » : les outils y sont écrits en Rust mais restent
accessibles sous les noms classiques — uutils/coreutils en natif (`ls`, `cp`,
`mv`...), et bat, ripgrep, fd, dust, procs, hexyl via aliases interactifs
(`cat`, `grep`, `find`...). Voir le [guide](docs/GUIDE.md).

## Construire

```bash
./scripts/01_download.sh            # télécharge et vérifie les sources (SHA-256)
./scripts/02_toolchain.sh           # compile la toolchain croisée
sudo ./scripts/03_chroot_build.sh   # compile noyau, socle GNU et userland Rust
./scripts/04_initramfs.sh           # fabrique le mini-système de boot
./scripts/05_make_iso.sh            # assemble l'ISO finale dans out/
```

Prérequis et mode d'emploi : [docs/GUIDE.md](docs/GUIDE.md).

## Documentation

| Document | Contenu |
|---|---|
| [PRD](docs/PRD.md) | Vision, problème résolu, scope, roadmap |
| [Vue d'ensemble](docs/system-overview.md) | Architecture C4 (contexte, conteneurs) |
| [SRS](docs/SRS-software.md) | Exigences logicielles |
| [SAD](docs/SAD-software.md) | Architecture logicielle |
| [LLD](docs/components-lld.md) | Conception détaillée des composants |
| [Règles métiers](docs/business-rules.md) | Invariants du projet |
| [Stratégie de test](docs/test-strategy.md) / [Cas de test](docs/test-cases.md) | Recette |
| [Déploiement](docs/deployment-ops.md) | CI/CD GitHub Actions |
| [Glossaire](docs/glossary.md) | Vocabulaire du projet |

## Statut

- [x] Étape 1 — sources téléchargées et vérifiées (manifeste : 12 paquets épinglés SHA-256)
- [ ] Étape 2 — toolchain croisée (`02_toolchain.sh`), en conception
- [ ] Étapes 3–5 — chroot, initramfs, ISO
