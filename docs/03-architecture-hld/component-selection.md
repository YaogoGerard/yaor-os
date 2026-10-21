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
| Core utilities | uutils/coreutils, GNU coreutils | **`uutils/coreutils`** (remplace GNU, ADR-012) |
| findutils | uutils/findutils, GNU findutils | **`uutils/findutils`** (remplace GNU, ADR-017) |
| Init et services | systemd, alternatives | **systemd** (ADR-004) |
| Réseau | systemd-networkd, NetworkManager | **`systemd-networkd` + `systemd-resolved`** (ADR-013) |
| Wi-Fi | iwd, wpa_supplicant | **`iwd`** (ADR-013) |
| Élévation de privilèges | sudo-rs, sudo, doas | **`sudo-rs`** (ADR-012) |
| Montage de volumes | udisks2, systemd-mount | **`udisks2`** (ADR-014) |
| Installateur | (aucune solution adaptée) | **`yaor-install`** + **`yaor-slot`** (internes, Rust — ADR-015) |
| Toolchain | Rust, LLVM, GCC | **GCC** (dans l'image) ; **Rust = outil de build** (`rustc`/`cargo` de l'hôte, non embarqué — ADR-019) |
| Packaging | dpkg/APT, image immuable | **image immuable** (`systemd-sysupdate`, ADR-015) |
| Compositor | Smithay et autres projets Rust | **Smithay** (ADR-005, Phase 2) |
| Rendu GPU | Mesa et alternatives | **`mesa` + `libdrm`** (ADR-024, Phase 2) |
| Audio | PipeWire, PulseAudio, ALSA pur | **`pipewire` + `wireplumber`** (ADR-024, Phase 2) |
| Protocoles Wayland | libwayland et alternatives | **`libwayland`** (ADR-024, Phase 2) |
| Périphériques d'entrée | libinput et alternatives | **`libinput` + `libevdev`** (ADR-024, Phase 2) |

> **Principe de non-duplication :** un besoin correspond à **un seul fournisseur**. Un
> composant Rust retenu **remplace** son homologue C/GNU, qui n'est pas embarqué
> (ADR-017). Non retenus pour v0.2 (maturité) : `uutils/diffutils`, `grep`, `sed`,
> `util-linux`.
>
> **Doublons écartés à l'installation :** `uutils/coreutils` livre aussi `more` et `kill`,
> déjà assurés par `util-linux` (non remplacé en v0.2) ; ils sont écartés (`SKIP_UTILS`).
> `uutils` installe en revanche `arch` et `[`, dont il devient le seul fournisseur.

Les décisions définitives doivent être enregistrées avec version et justification.
