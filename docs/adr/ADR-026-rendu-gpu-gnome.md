# ADR-026 — Rendu GPU du bureau « à la GNOME »

Statut : proposé — date : 11 nov. 2026

## Contexte
v0.7 rend les widgets et effets en CPU (`yaor-ui`, rastériseur logiciel) ;
seul le compositing des fenêtres passe par le GLES du compositor (Smithay).
Mesures RAM : bureau complet ≈ 40 Mo (plat), ≈ 150 Mo (verre, flou CPU +
89 Mo de travail au compositor). L'utilisateur vise une consommation RAM
minimale **et** demande un rendu « comme GNOME » (GPU intégral) pour
 supporter le lot v0.8 (flou des fenêtres animé, transitions).

## Ce que fait GNOME (référence)
- mutter : compositing GL intégral (Cogl/Clutter).
- GNOME Shell (panel, overview) : **client** rendu en GL, widgets CSS/St.
- Effets (flou) : shaders GL (expérimental 46+).
- Repli : rendu logiciel (llvmpipe) quand pas d'accélération.
- Coût RAM constaté du shell complet : 300–500 Mo (bien plus que YAOR).

## Décision
Adopter la **cible GNOME avec le chemin de migration Hyprland/wlroots** :
1. **Phase 0 (spike)** : valider GL sous QEMU (virtio-gpu + VirGL/Venus)
   et un chemin de capture GL hors-écran (FBO → PNG) pour conserver la
   QA pixel et les previews. Livrable : chiffres bench flou shader vs CPU.
2. **Phase 1** : flou (3 passes boîte) et effets en **shaders GL** dans le
   contexte GLES existant du compositor ; repli CPU conservé, choisi au
   boot. Objectif : RSS compositor < 40 Mo en verre, flou animé temps réel.
3. **Phase 2** : `yaor-ui` migre son painting vers un **renderer GL**
   (texture atlas pour le texte, rounded-corners/shadows en shader), les
   surfaces panel/dock/settings devenant des buffers GL ; renderer CPU
   conservé en repli déterministe (tests, machines sans accel).
4. **Invariants non négociables** :
   - **SSD conservé** (le compositor décore) : c'est un écart assumé avec
     GNOME (CSD), requis par le thème verre transversal.
   - **QA pixel conservée** : tout rendu GL doit être capturable et
     comparable au canal près avec l'aperçu CPU de référence.
   - **Repli CPU permanent** : jamais de « GPU carrément » sans chemin
     logiciel testable (leçon wlroots/GNOME/KDE).

## Alternatives écartées
- Tout-CPU à vie (Xfce) : incompatible avec le lot v0.8 animé.
- Réécriture GL monolithique immédiate : casse QA/previews, risque majeur.
- GPU optionnel uniquement sur carte dédiée : ne répond pas à la demande.

## Conséquences
- RAM : le flou quitte le RSS système vers VRAM (identique physiquement
  sur iGPU ; dégagement réel sur GPU dédié) ; les buffers de travail CPU
  disparaissent (points 1–2 de l'audit RAM v0.7 absorbés par la phase 1).
- RAM shell : la phase 2 montera légèrement (contextes GL, atlas) ;
  cible volontairement *sous* le shell GNOME (pas de CSS/St, widgets
  maison), budget < 250 Mo bureau complet en verre animé.
- Effort : phase 1 ~1 semaine ; phase 2 est un chantier majeur (le plus
  gros du portage), à séquencer par brique (texte → kit verre → widgets).
