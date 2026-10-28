# Configuration Data Model (desktop.toml, v0.5 étape 1)

Schéma effectif (serde `DesktopConfig`, crate `components/yaor-config`,
Rust 0.2.0) — toute clé absente prend la valeur compilée par défaut ;
les valeurs d'usine du live (`/etc/yaor/desktop.toml`) sont identiques.

```toml
[layout]
mode   = "cascade"          # "cascade" | "master-stack" (défaut : cascade)
ratio  = 0.5                # largeur relative du maître, 0.0..1.0
                            # (limité à 0.2..0.8 par les raccourcis Mod+g)
gaps   = 0                  # espacement en pixels entre tuiles

[keys]
focus_next    = "Mod+Right"        # focus suivant (tuile)
focus_prev    = "Mod+Left"         # focus précédent (tuile)
swap_next     = "Mod+Shift+Right"  # échange deux fenêtres du tiling
swap_prev     = "Mod+Shift+Left"   # (l'ordre de la pile)
master_swap   = "Mod+Tab"          # fenêtre focusée ↔ maître
close         = "Mod+Q"            # xdg_toplevel.close fenêtre focusée
ratio_grow    = "Mod+g"            # maître +5 % (borne 0.8)
ratio_shrink  = "Mod+Shift+g"      # maître −5 % (borne 0.2)
quit          = "Mod+Shift+E"      # arrêt propre (équivalent SIGTERM)
```

## Conventions

- `Mod` = touche Super (logo) — convention sway/Hyprland ;
  combinateurs admis : `Mod`, `Ctrl`, `Alt`, `Shift`.
- Touche désignée par son **caractère** (Q, X, g, chiffre) — casse
  indifférente (parsé minuscule) — ou par son **nom** (`Right`, `Left`,
  `Up`, `Down`, `Tab`, `Return`, `Escape`).
- Les 9 raccourcis sont rebinables (REQ-SW-015) ; rebind vérifié à
  l'usage (t-yaor-config : close=Mod+X).

## Hors v0.5 (rattaché à v0.6)

[desktop] (profils, REQ-SW-007), [panel], [windows], [theme],
[accessibility] — les fichiers séparés de la vue préliminaire
(theme.toml, windows.toml…) ne sont pas admis en v0.5 : tout le
bureau configurable se trouve dans ce seul `desktop.toml`, les
sections inconnues sont ignorées (extension future).
