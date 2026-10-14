# Component Selection

## Principe

```text
Besoin
  |
Existe-t-il une solution ?
  |
 +-- Non --> Développer, de préférence en Rust
 |
 +-- Oui --> Existe-t-il une solution Rust adaptée ?
                  |
                 Oui --> évaluer maturité, maintenance, licence, performance
                  |
                 Non --> utiliser la solution existante adaptée
```

## Critères
- adéquation fonctionnelle ;
- maturité ;
- maintenance ;
- licence ;
- sécurité ;
- performance ;
- compatibilité Linux ;
- qualité de l'intégration ;
- possibilité de remplacement ;
- disponibilité d'une implémentation Rust.

## Familles évaluées

| Besoin | Candidats | Décision |
|---|---|---|
| Kernel | Linux | **Linux** |
| Core utilities | uutils/coreutils, GNU coreutils | **`uutils/coreutils`** (ADR-012) |
| Init et services | systemd, alternatives | **systemd** (ADR-004) |
| Réseau | systemd-networkd, NetworkManager | **`systemd-networkd` + `systemd-resolved`** (ADR-013) |
| Wi-Fi | iwd, wpa_supplicant | **`iwd`** (ADR-013) |
| Élévation de privilèges | sudo-rs, sudo, doas | **`sudo-rs`** (ADR-012) |
| Montage de volumes | udisks2, systemd-mount | **`udisks2`** (ADR-014) |
| Toolchain | Rust, LLVM, GCC | **GCC** (toolchain) ; **Rust** amorcé par l'hôte, LLVM embarqué (ADR-012) |
| Packaging | dpkg/APT, image immuable | **image immuable** (`systemd-sysupdate`, ADR-015) |
| Compositor | Smithay et autres projets Rust | À évaluer (Phase 2) |
| Graphics | Mesa et alternatives | À évaluer (Phase 2) |
| Audio | PipeWire et éventuelles implémentations Rust | À évaluer (Phase 2) |
| Input | libinput et alternatives | À évaluer (Phase 2) |

Les décisions définitives doivent être enregistrées avec version et justification.
