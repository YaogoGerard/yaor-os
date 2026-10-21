# ADR-024 — Base graphique v0.3 : mesa, libinput, Wayland, PipeWire

**Statut :** Accepté
**Date :** 2026-10-21
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-002, ADR-004, ADR-005, ADR-009, ADR-012, ADR-014

## Contexte

L'ouverture de v0.3 (« base graphique ») doit fixer les fournisseurs de la pile
graphique sous le compositor (v0.4, Smithay — ADR-005). Le noyau du jalon v0.2 porte
déjà le nécessaire côté noyau : KMS (`CONFIG_DRM_*`), `virtio-gpu` (QEMU),
`simpledrm` (GPU réel non piloté), émulation fbdev, socle ALSA (`CONFIG_SND_*`) et
HDA générique (audio QEMU `intel-hda`). Restent les composants userland, conformément
aux exigences : « un besoin = un fournisseur » (ADR-009 / ADR-017), préférence Rust
lorsque la maturité est au rendez-vous (ADR-002), sinon un composant existant (REQ-SW-005).

Aucun serveur audio Rust mature n'existe : la base audio ne peut pas être Rust.

## Options envisagées

- **Option A (audio) — PipeWire + WirePlumber.** Standard actuel, API client stable
  (le futur compositor l'utilisera), serveur unique qui mixe ALSA.
- **Option B (audio) — PulseAudio.** Typique des bureaux traditionnels ; changement
  coûteux vers PipeWire ensuite.
- **Option C (audio) — ALSA pur, sans serveur.** Minimal mais pas de mixage ni de
  session audio de bureau.
- **Option (validation) — compositor de référence tiers (weston…).** Pile éprouvée
  mais fournisseur non réutilisé après v0.3.
- **Option (validation) — client Wayland minimal Rust seul.** Ne valide pas le rôle
  compositor (rendu KMS complet, entrée).

## Décision

**Option A pour l'audio.** Composants de la base graphique v0.3 :

| Besoin | Fournisseur | Rôle |
|---|---|---|
| Rendu GPU / EGL-GLES | **mesa** (+ `libdrm`) | render soft (llvmpipe) pour QEMU (virtio-gpu, pas d'accélération 3D hôte) |
| Protocoles Wayland | **libwayland** | bibliothèque client et serveur |
| Périphériques d'entrée | **libinput** (+ `libevdev`) | clavier, pointeur (événements via udev/logind, ADR-014) |
| Audio (serveur) | **pipewire** | serveur mixant captures et sorties ALSA |
| Audio (session) | **wireplumber** | gestionnaire de session audio (politique, routage) |

La **session Wayland de validation** est le **compositor d'exemple de Smithay** :
aucun composant tiers supplémentaire (un seul fournisseur, ADR-005), et il amorce
directement le travail de v0.4. Il n'est pas livré comme composant système — véhicule
de validation du jalon uniquement.

**XWayland** (ADR-005) est intégré avec le compositor en **v0.4** : sans compositor
YAOR il n'a pas d'homologue ; il n'est pas dans le périmètre v0.3.

Les sources sont épinglées dans `sources/manifest.tsv` (groupe, version, URL,
SHA-256) et intégrées par les chapitres de build (compilation hors-ligne, ADR-016).

## Conséquences

- **Positives :** base standard et pérenne sous le compositor Smithay ; préparation
  directe de v0.4 ; un seul fournisseur par besoin ; validation 100 % sous QEMU
  (virtio-gpu + simpledrm, intel-hda).
- **Négatives / risques :** wireplumber est en C (aucun équivalent Rust mature) —
  écart assumé à la préférence Rust, justifié comme REQ-SW-004 l'exige ; la surface
  mesa est large — EGL/GLES2 + llvmpipe suffisent à v0.3 (pas de Vulkan dans le
  périmètre).
- **Suivi :** LLD des services (mode graphique vs console — `graphics.target`,
  terminaux tty conservés) ; mise à jour de `component-selection.md` ; critères
  détaillés dans `milestones.md` (v0.3) ; KMS+audio seront retentés quand la radio
  et le GPU matériel réels pourront l'être.
