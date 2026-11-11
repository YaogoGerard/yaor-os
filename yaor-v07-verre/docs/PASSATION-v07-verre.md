# Passation — v0.7 « Verre » (10 nov. 2026)

Livré : thème configurable (script + app visuelle) avec matériau « liquid
glass », 5 présets (flat + 4 verres), compositor (barres de titre en verre,
bordures adoucies, animation d'ouverture), CLI `yaor-ctl theme`, app Réglages.
Décision d'architecture : **ADR-025**. Aperçus : `previews/`.

## ⚠ Statut de vérification — à lire d'abord
Je n'avais ni Smithay vendu ni sctk dans mon environnement. Donc :

| Zone | Statut |
|---|---|
| `yaor-config` (clés, présets, éditeur toml, `tune`, `notify_compositor`) | **compilé + 31 tests verts** (15 existants + 16 nouveaux) |
| `yaor-ui` (flou, matériau, `GlassKit`, `GlassState`) + **correctif police** | **compilé + 31 tests verts** ; rendu visuel vérifié (`previews/`) |
| `yaor-settings/src/ui.rs` (modèle, hit-test, dessin) | **compilé + 7 tests**, rendu vérifié |
| logique `yaor-ctl theme` | **extraite, compilée et exécutée** de bout en bout (list/get/set/tune/refus) |
| `compositor/src/glass.rs::open_progress` | test isolé vert |
| compositor (`ssd.rs`, `element.rs`, `render.rs`, `config.rs`), `yaor-panel`, `yaor-dock`, `yaor-settings/main.rs`, `yaor-ctl` entier | **NON compilé** — syntaxe contrôlée (rustfmt), types non vérifiés |

**Première action : `cd components && cargo check --workspace --offline`**
(`PKG_CONFIG_PATH=/tmp/opencode/pc` sur l'hôte). Envoie-moi la sortie : les
écarts attendus sont de l'ordre de la signature (cf. risques ci-dessous), pas
de la conception. `Cargo.lock` a été complété à la main (`yaor-settings`).

### Risques de compilation identifiés (par probabilité)
1. `MemoryRenderBufferRenderElement::from_buffer(.., Some(alpha), ..)` : le
   1er `Option` après le buffer est supposé être l'alpha (déduit des appels
   existants) — `element.rs`, `ssd.rs` (HeaderBar).
2. `OutputInfo::logical_size` / `modes[].current` (sctk 0.19) — panel, dock.
3. `SlotPool::len()` / `resize()` — `yaor-settings/main.rs`.
4. Emprunts `RefCell` dans `element.rs` (bloc d'animation avant le test SSD :
   le `RefMut` est refermé avant l'appel suivant, à confirmer).

## Ce qui a changé
- **yaor-config** : `[theme]` + `name, style, glass_blur, glass_opacity,
  glass_saturation, glass_tint, glass_border, glass_highlight, glass_grain,
  anim_ms` ; `presets` (flat, glass-ambre, -glacier, -amethyste, -emeraude) ;
  `edit` (édition ligne à ligne, commentaires conservés, écriture atomique) ;
  `hup::notify_compositor`. `default.toml` documenté.
- **yaor-ui** : `glass` (flou boîte 3 passes, saturation, teinte adaptative,
  rim light, grain, `GlassKit`, `GlassState`), exemple `glass_preview`.
- **yaor-assets** : `wallpaper_argb` (fond plein écran opaque pour le flou).
- **yaor-panel / yaor-dock** : fonds (barre, taskbar, menu, dialog, dock) en
  verre si `style = "glass"` ; taille d'écran lue sur la sortie ; tuile
  « Réglages » lance `yaor-settings` (style de tuile inchangé : PNG).
- **compositor** : `glass.rs` (fond flouté en cache), barre de titre en verre
  alignée sur la position de la fenêtre, bordures discrètes en verre,
  **animation d'ouverture** (fondu + glissement 10 px, ease-out, `anim_ms`).
- **yaor-ctl** : `theme list | get [clé] | set <préset> | tune clé=valeur…`
  (0 ok, 1 écriture, 2 refus — jamais de repli silencieux).
- **yaor-settings** (nouveau) : cartes de présets, bascule Plat/Verre, 8
  accents, 6 curseurs (flou, opacité, saturation, rayon, ombre, animations).
  Écrit au relâchement (un SIGHUP par geste).

## ⚠ Correctif hors périmètre mais impactant : police (yaor-ui/text.rs)
Deux bugs du rastériseur : (1) `do_quad` repartait toujours du point de départ
de la courbe (éventail de traits parasites) → **les contre-formes de O, o, 0,
B, 8, @ étaient rayées** (le « VAOR os » du rapport v0.6) ; (2) `maxy` utilisait
`min` au lieu de `max` → bas des glyphes tronqué (descendantes g, j, p, q, y).
Test de non-régression ajouté. **Conséquence QA** : les sondes qui mesurent des
étendues de texte peuvent bouger de 1–2 px → ré-étalonner `probe-visuals` pour
les libellés.

## Conformité aux PNG / QA existantes
- `style = "flat"` (défaut) : tous les chemins v0.6 sont conservés.
- **Exception en régime transitoire** : l'animation d'ouverture (`anim_ms =
  150` par défaut) s'applique aussi en plat pendant 150 ms. Pour les sondes
  pixel et screendumps : mettre `anim_ms = 0` dans le desktop.toml de QA.
- Sondes dock/ssd/launcher à rejouer en plat ; ajouter `qa/qa-glass.sh`
  (présets, bornes, refus, jetons « verre : actif/plat », fond flouté).
- Captures avant/après par préset à produire sur l'ISO desktop (probe-visuals).

## Intégration build (non faite : `scripts/` absent du zip)
Ajouter `yaor-settings` à la liste des binaires installés par la phase
`ch7.yaor-components` (même endroit que `yaor-files`/`yaor-term`) :
`grep -rn yaor-files scripts/ build/`. Puis `stamp-reset.sh ch7.yaor-components
ch8.initramfs ch9.iso`.

## Non fait (hors demande « glass », restent au brief v0.7)
Inclinaison au drag, animation de **fermeture**, loupe/rebond/badge du dock,
émulateur VT, transitions d'espaces, `yaor-editeur`/`yaor-moniteur`. Et le vrai
flou « backdrop » des fenêtres derrière une surface (ADR-025, v0.8). Le menu
n'a pas de recherche fonctionnelle ni les autres tuiles (inchangé).

## Commits suggérés (un par étape, push à chaque étape)
1. `fix(ui): contre-formes et descendantes du rastériseur TrueType`
2. `feat(config): theme verre, presets, edition toml, notify_compositor`
3. `feat(ui): materiau verre (flou, teinte, rim light) + GlassState`
4. `feat(panel): fonds en verre et tuile Reglages`
5. `feat(dock): barre en verre`
6. `feat(compositor): barre de titre en verre, animation d'ouverture`
7. `feat(ctl): theme list|get|set|tune`
8. `feat(settings): app Reglages (presets, accent, matiere)`
Trailers `Jalon: v0.7`, `Feature: verre`, `Refs: ADR-025`. Scan U+FFFD+CRLF fait
(62 fichiers, 0 problème).
