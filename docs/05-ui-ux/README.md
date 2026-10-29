# YAOR OS — UI/UX

Interface et expérience utilisateur : identité visuelle, profils de
bureau, design system, maquettes et flux utilisateurs.

## Contenu

| Dossier / fichier | Rôle |
|---|---|
| [`branding/`](branding/) | Emblème produit et palette « ambre sur noir » (référence canonique des couleurs) |
| [`design/`](design/) | Rendus haute-fidélité des trois bureaux (cibles design) |
| [`mockups/`](mockups/) | Maquettes SVG schématiques liées aux cas de test TEST-UI-001/002/003 |
| [`themes/`](themes/) | Thèmes (ensembles de configuration visuelle, sans logique) |
| [`desktop-modes.md`](desktop-modes.md) | Les trois modes de bureau et leur état d'implémentation |
| [`design-system.md`](design-system.md) | Primitives visuelles communes (couleurs, typographie, composants) |
| [`user-flows.md`](user-flows.md) | Flux utilisateurs (profils, menu, fenêtres, espaces) |
| [`accessibility.md`](accessibility.md) | Accessibilité (REQ-SW-012, REQ-SW-013) |

## Hiérarchie des références visuelles

1. **`branding/`** — l'emblème et la palette produit font foi (constantes
   des composants) ;
2. **`design/`** — les rendus haute-fidélité montrent l'expérience
   cible de chaque profil ;
3. **`mockups/`** — les SVG schématiques servent aux cas de test
   (zones de clic QA, géométries mesurables).

Un élément d'interface livré se juge contre le rendu de son profil
(`design/`), avec les couleurs canoniques de `branding/`.

## Exigences liées

- REQ-SW-008 — Profil classique (`TEST-UI-001`)
- REQ-SW-009 — Profil macOS-like (`TEST-UI-002`)
- REQ-SW-010 — Profil tiling (`TEST-UI-003`)
- REQ-SW-012 / REQ-SW-013 — Accessibilité, sortie vocale Mooré
- REQ-SW-015 — Configuration déclarative (thèmes et profils)
