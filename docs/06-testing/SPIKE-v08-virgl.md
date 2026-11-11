# Spike phase 0 ADR-026 — VirGL dans QEMU : résultats (11 nov. 2026)

## Questions du spike et réponses

1. **QEMU hôte sait-il faire du GL ?** OUI : `virtio-vga-gl`/`virtio-gpu-gl`
   présents dans `-device help`, `libvirglrenderer.so.1` installée.
   Piège : `-device virtio-vga-gl` exige `-display egl-headless,gl=on`
   (sinon : « The display backend does not have OpenGL support enabled »).
2. **L'invité voit-il le GPU GL ?** OUI : boot OK de l'ISO desktop avec
   `virtio-vga-gl` + `egl-headless,gl=on` ; kernel `[drm] features:
   +virgl +edid` ; `/dev/dri/card0` + `/dev/dri/renderD128` présents ;
   session yaor-session active, panel rendu (verre appliqué via
   `yaor-ctl theme set glass-glacier` accepté).
3. **Coût du flou CPU (référence)** : bureau complet 1280×720 (flou
   compris) rendu par le vrai code en **~0,18 s/préset** (release, hôte :
   `glass_preview`, 5 présets en 0,92 s). Suffisant pour un rendu statique
   mis en cache ; insuffisant pour un flou **animé** à 60 fps (5–6 fps
   pire cas si reconstruit par frame) → le shader est justifié pour v0.8.
4. **RAM** : le poste à éliminer est mesuré : **+89 Mo de RSS au
   compositor** à l'activation du verre (ensemble de travail CPU du flou,
   audit v0.7). La phase 1 shader doit ramener le compositor < 40 Mo.
5. **Observabilité manquante** : le compositor ne journalise pas sa
   renderer GL (aucune ligne renderer/EGL au journal). **Ajouter un log
   INFO de la chaîne renderer au boot** (1 ligne dans session.rs) —
   prérequis de vérification de la phase 1.

## Verdict
**Feu vert phase 1** : flou en shaders GLSL dans le contexte GLES
existant du compositor (`glass_gl.rs`), repli CPU conservé, sélection au
boot. La chaîne de test QEMU-VirGL est opérationnelle (`t-gfx-live.py` +
`probe-virgl.py` : `-device virtio-vga-gl -display egl-headless,gl=on`).

## Références
- ADR-026 (`docs/adr/ADR-026-rendu-gpu-gnome.md`).
- Sondes : `yaor-qa/probe-virgl.py`, `probe-virgl3.py` ;
  transcrits `virgl.log`, `virgl3.log`.
- Audit RAM v0.7 : `MemAv 1919→1803 Mo` à l'activation du verre,
  compositor 15,4 → 104 Mo RSS (détail par processus).
