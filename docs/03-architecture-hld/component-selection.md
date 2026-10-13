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

## Familles à évaluer

| Besoin | Candidats à évaluer | Décision |
|---|---|---|
| Kernel | Linux | Linux |
| Core utilities | uutils/coreutils, alternatives | À évaluer |
| Compositor | Smithay et autres projets Rust | À évaluer |
| Graphics | Mesa et alternatives | À évaluer |
| Audio | PipeWire et éventuelles implémentations Rust | À évaluer |
| Input | libinput et alternatives | À évaluer |
| Toolchain | Rust, LLVM, GCC | À évaluer |
| Packaging | dpkg/APT et alternatives | À évaluer |

Les décisions définitives doivent être enregistrées avec version et justification.
