# ADR-025 — Thème « Verre » : jetons `[theme]` + matériau CPU partagé

**Statut** : proposé (v0.7) — **Contexte** : le brief v0.7 demande un bureau
« liquid glass » configurable par script ET par une app visuelle, sans casser
la conformité aux PNG du design (Classique / macOS-like / Tiling).

## Décision
1. **Un seul format de thème** : la section `[theme]` de `desktop.toml`
   (déjà lue par compositor, panel, dock, rechargée au SIGHUP). On l'étend de
   clés optionnelles (`style`, `glass_*`, `anim_ms`, `name`). Aucun second
   fichier `theme.conf`.
2. **`style = "flat"` reste le défaut** et redonne le rendu v0.6 : le verre
   est un thème de plus, pas un remplacement des PNG. Un `[theme]` v0.6 reste
   valable (tous les champs ont un défaut).
3. **Un seul fournisseur de matériau** : `yaor_ui::glass` (CPU, pur, sans
   dépendance). Fond d'écran flouté UNE fois (réduit ÷2/÷4, 3 passes de flou
   boîte, saturation), puis découpé à la position écran de chaque surface +
   teinte adaptative (monte sur fond clair pour le contraste) + reflet de haut
   + liseré spéculaire éclairé d'en haut + grain. Panel, dock, Réglages et
   barres de titre SSD l'appellent à l'identique.
4. **Un seul chemin d'écriture** : `yaor_config::edit` (édition ligne à ligne,
   commentaires conservés) + `presets` (présets, validation, bornes). La CLI
   `yaor-ctl theme` et l'app `yaor-settings` n'ont aucune logique propre. Le
   fichier écrit est celui que `load()` lira (premier candidat existant, comme
   `profile set`), pour ne jamais masquer `/etc/yaor/desktop.toml`.

## Alternatives écartées
- **Flou shader du framebuffer (backdrop réel)** : seul moyen de flouter les
  fenêtres qui passent derrière une surface ; exige un passage GLES sur
  Smithay 0.7 et un repli Pixman (QEMU/llvmpipe). Reporté (v0.8) — le flou du
  fond d'écran couvre la barre, le dock, le menu et les barres de titre.
- **Protocole de flou côté client (ext-background-effect)** : second
  fournisseur et surface de protocole en plus ; non nécessaire ici.
- **theme.conf `clé = valeur`** : doublon du `[theme]` existant.

## Conséquences
- Les surfaces restent opaques côté Wayland (le verre est « cuit » dans le
  buffer) : aucune translucidité réelle, donc pas de régression de damage.
- Coût : un flou par (thème, taille de sortie), au démarrage / SIGHUP / changement
  de sortie — jamais par image. Un buffer de barre de titre se reconstruit
  pendant le drag d'une fenêtre (28 × largeur px : négligeable).
- Limite assumée : le flou porte sur le fond d'écran, pas sur les fenêtres.
