# YAOR v0.7 « Verre » — livraison

Structure (même découpage que ton envoi) :

- `components/`  workspace complet mis à jour — à copier par-dessus `components/` du dépôt
- `docs/PASSATION-v07-verre.md`  **à lire en premier** (statut de vérification, risques, commits)
- `docs/ADR-025-theme-verre.md`  décision d'architecture
- `qa/qa-glass.sh`  contrôles à lancer dans l'invité
- `previews/`  rendus produits par le vrai code (`bureau-*.png` : 5 thèmes ; `reglages-*.png`)

Première commande : `cd components && cargo check --workspace --offline`
(le compositor, le panel, le dock et `yaor-settings/main.rs` n'ont pas pu être compilés ici).

Utilisation une fois installé :
    yaor-ctl theme list
    yaor-ctl theme set glass-glacier
    yaor-ctl theme tune glass_blur=30 accent=#5AB8FF
    yaor-settings          # ou tuile « Réglages » du menu
