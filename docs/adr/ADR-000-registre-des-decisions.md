# ADR-000 — Registre des décisions fondatrices

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-001 à ADR-016

## Contexte

La documentation initiale (v0.1) décrivait une vision cohérente dans son esprit mais
contenait plusieurs contradictions et zones non décidées (modèle de construction,
compositor Rust, cycle de vie, init, sécurité). Ce registre fige les décisions prises
pour lever ces ambiguïtés et servir de référence à l'implémentation.

## Décision

Les décisions suivantes sont retenues. Elles font autorité jusqu'à remplacement par un
ADR dédié.

| # | Sujet | Décision |
|---|---|---|
| 1 | Modèle de construction | **Depuis les sources (LFS)** — voir ADR-001 |
| 2 | Compositor | **Rust par défaut, repli si pas assez mature/stable** — voir ADR-002 |
| 3 | Cycle de vie | **Live + installé dès la v1** — voir ADR-003 |
| 4 | Init / services | **systemd** — voir ADR-004 |
| 5 | Mises à jour | **Image immuable (A/B ou réinstallation)** — voir ADR-003 |
| 6 | Périmètre v1 | **Tout le périmètre** (bureau, profils, plugins, accessibilité, Mooré) |
| 7 | Exigences non fonctionnelles | **Principes seulement pour l'instant**, chiffrage ultérieur |
| 8 | Règles métier vs exigences | **Fusion en exigences** ; `rules.md` sera absorbé dans le SRS |
| 9 | Traçabilité | **Compléter les 10 cas de test manquants** |
| 10 | Maturité | **Critères objectifs et documentés** — voir ADR-002 |
| 11 | Registre de décisions | **Dossier `adr/` + modèle + index** (ce dossier) |
| 12 | Window Manager / compositor | **Fusion : le compositor assure la gestion des fenêtres** |
| 13 | Conventions documentaires | **En-tête standard + glossaire + convention d'IDs** |
| 14 | Pile graphique | **Smithay + XWayland + logind** — voir ADR-005 |
| 15 | Utilisateurs | **Multi-utilisateur POSIX** |
| 16 | Réseau & stockage | **Exigences documentées, outils non figés** |
| 17 | Sécurité des plugins | **Sandbox + permissions déclarées** |
| 18 | Accessibilité | **Réutiliser AT-SPI (D-Bus)** — voir ADR-006 |
| 19 | Internationalisation | **i18n/l10n devient une exigence** |
| 20 | Mooré | **Exigence au niveau résultat ; moteurs en ADR** |
| 21 | Progression | **Jalons numérotés + critères de sortie** |
| 22 | Langue de la doc | **Corps en français, titres/sections en anglais** |
| 23 | Exigences matérielles | **Durcir x86_64, GPU, input en DOIT** |
| 24 | Sélection des composants | **Figer seulement le déjà décidé ; le reste en ADR** |
| 25 | Gouvernance | **Risques/hypothèses/contraintes : ajout ultérieur** |
| 26 | CI/CD | **Pipeline local d'abord, CI ensuite** |
| 27 | Composants d'identité | **Au cas par cas, avec justification (ADR)** |
| 28 | Niveaux de test | **Conserver les 8 niveaux, à remplir plus tard** |
| 29 | Sécurité | **Empreintes sources (SHA-256) + ISO signée** — voir ADR-007 |
| 30 | Licence | **À décider avant la première release** |
| 31 | Convention de commit | **Conventional Commits + trailers `Jalon`/`Feature`** — voir ADR-008 |
| 32 | Sélection par défaut des composants | **Modernes, maintenus et performants** (« mature » ≠ « ancien ») — voir ADR-009 |
| 33 | Démarrage live | **Limine (BIOS+UEFI) + initramfs shell + image EROFS (zstd)** — voir ADR-011 |
| 34 | Amorce et intégration de Rust | **`rustc` de l'hôte comme amorçage ; LLVM embarqué ; `uutils/coreutils` ; `sudo-rs`** — voir ADR-012 |
| 35 | Réseau | **`systemd-networkd` + `systemd-resolved` ; Wi-Fi par `iwd`** — voir ADR-013 |
| 36 | Sessions et comptes | **D-Bus et `systemd-logind` réactivés ; PAM ; polkit ; `udisks2`** — voir ADR-014 |
| 37 | Cycle de vie installé | **installateur + Limine sur disque ; mises à jour `systemd-sysupdate` (A/B)** — voir ADR-015 |
| 38 | Systèmes de fichiers | **`ext4`, `btrfs`, `vfat`/`exFAT`, `NTFS` (rw)** — voir ADR-015 |
| 39 | Reproductibilité | **fonctionnelle (versions + empreintes épinglées)** — voir ADR-016 |

## Conséquences

- **Positives :** les contradictions bloquantes sont levées ; les décisions sont
  traçables et datées.
- **Négatives / risques :** le périmètre v1 est très large ; les décisions 6, 7 et 25
  devront être réévaluées au fil des jalons.
- **Suivi :** la décision 21 est matérialisée dans [`milestones.md`](../01-product/milestones.md) ;
  les décisions 8, 9, 13 et 23 restent à matérialiser dans les documents concernés lors
  des jalons suivants.
