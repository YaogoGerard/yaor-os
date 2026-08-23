# Conception détaillée des composants (LLD) — yaor-os

**Statut :** Brouillon
**Version :** 0.2

## Séquence — `01_download.sh`

```mermaid
sequenceDiagram
    participant CI as GitHub Actions
    participant DL as 01_download.sh
    participant Mirror as Miroir de sources

    CI->>DL: Exécute le script
    loop Pour chaque source (GCC, Glibc, Linux...)
        DL->>Mirror: Télécharge l'archive
        Mirror-->>DL: Archive .tar.xz
        DL->>DL: Vérifie le SHA-256 (REQ-SW-002)
        alt Somme invalide
            DL-->>CI: Échec du job
        end
    end
    DL-->>CI: Archives vérifiées prêtes
```

## Séquence — `03_chroot_build.sh`

```mermaid
sequenceDiagram
    participant CI as GitHub Actions
    participant Build as 03_chroot_build.sh
    participant Chroot as Environnement chroot

    CI->>Build: Exécute le script (toolchain prête)
    Build->>Chroot: Monte /proc, /dev, /sys
    Build->>Chroot: Entre en chroot (REQ-SW-004)
    Chroot->>Chroot: Compile et installe le système final
    Chroot-->>Build: rootfs/ complet
    Build->>Chroot: Démonte /proc, /dev, /sys
    Build-->>CI: rootfs/ prêt pour l'étape suivante
```

## États — Cycle de vie d'un build

```mermaid
stateDiagram-v2
    [*] --> Téléchargement
    Téléchargement --> Toolchain: sources vérifiées
    Toolchain --> ConstructionChroot: toolchain compilée
    ConstructionChroot --> Initramfs: rootfs prêt
    Initramfs --> GénérationISO: initramfs prêt
    GénérationISO --> [*]: ISO publiée
    Téléchargement --> Échec: somme invalide
    Toolchain --> Échec: erreur de compilation
    ConstructionChroot --> Échec: erreur de compilation
    Échec --> [*]
```

## Séquence — Installation du userland Rust (dans `03_chroot_build.sh`)

```mermaid
sequenceDiagram
    participant Build as 03_chroot_build.sh
    participant Chroot as Environnement chroot
    participant Cargo as cargo (rust stage0)

    Build->>Chroot: Installe rust stage0 (tarball vérifié)
    Chroot->>Cargo: Compile uutils/coreutils (--locked)
    Chroot-->>Chroot: Installe les commandes de base sous noms classiques
    Chroot->>Cargo: Compile bat, ripgrep, fd, dust, procs, hexyl
    Chroot-->>Chroot: Installe les binaires + /etc/profile.d/10-oxidized.sh
    Chroot-->>Build: rootfs avec userland oxidized prêt
```

## Couche aliases — `/etc/profile.d/10-oxidized.sh`

Définis pour le shell interactif uniquement (RG-006) :

| Alias | Cible | Notes |
|---|---|---|
| `cat` | `bat` | pagination désactivée pour rester fidèle à `cat` |
| `grep` | `rg` (ripgrep) | options courantes compatibles |
| `find` | `fd` | syntaxe simplifiée, différence documentée dans GUIDE |
| `du` | `dust` | |
| `ps` | `procs` | |
| `hexdump` | `hexyl` | |

Contournement : préfixe `\outil` ou chemin absolu (RG-007). Le socle GNU minimal
(grep, findutils, sed, gawk, tar...) reste installé sous ses vrais noms pour les
scripts et comme repli.
