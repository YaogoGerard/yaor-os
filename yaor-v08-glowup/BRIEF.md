# Demande v0.8 « Glow up interfaces » — brief de travail

## Constat (de l'utilisateur, après test live sur l'ISO desktop)
Les trois profils de base (classic, macos, tiling) rendus par la v0.7 sur
machine réelle sont jugés **très moches** et ne ressemblent pas aux
aperçus `previews/`. L'utilisateur demande un rehaussement sérieux du
rendu des trois profils, calé sur la qualité des aperçus.

## Ce qui est déjà fidèle (ne pas retoucher)
- **Le matériau verre lui-même est conforme** : sonde pixel, même scène,
  même préset (glass-glacier) — aperçu `(63,77,95)` vs réel `(57,74,94)`
  sur la barre (x=600, y=12) : écart de teinte mineur, acceptable.
- Accents, changement de thème et de profil à chaud, rechargement
  SIGHUP, correctif police : validés en QA (26 contrôles PASS, passation
  `docs/06-testing/PASSATION-v07-verre.md`).

## Les écarts mesurés (cibles chiffrées)
1. **Hauteur des barres** : réelle ≈ 24 px à 1280×720, aperçu ≈ 32 px
   (-25 %). Tout paraît tassé (police, boutons, padding). Hypothèse :
   taille logique vs framebuffer, ou hauteur codée en dur basse.
   *Cible : barres à l'échelle de l'aperçu, hauteurs/typo des trois
   profils relus à la loupe.*
2. **Dock macos quasi vide** : 1 tuile (lanceur) en réel contre 8 tuiles
   dans l'aperçu (Fichiers, Terminal, Web, Éditeur, Réglages…).
   *Cible : tuiles épinglées par défaut comme dans l'aperçu, ou au
   minimum intégration des apps installées (yaor-term, yaor-files,
   yaor-settings…).*
3. **Volet de recherche** (« Rechercher une application… ») présent dans
   l'aperçu, jamais vu ouvert en réel : vérifier qu'il s'ouvre bien
   depuis la barre de menus et qu'il rend à l'identique.
4. **Fenêtres** : l'aperçu montre une barre de titre en verre avec
   pastilles macOS ; vérifier le rendu réel sur fenêtre mappée
   (le verre SSD fonctionne, l'esthétique des pastilles à comparer).

## Méthode de vérification imposée (pour éviter un nouvel écart)
Comparer **par sonde pixel** aperçu vs dump, pas à l'œil :
- boot ISO desktop, `yaor-ctl profile set X` + `theme set glass-glacier`,
- screendump QMP → PPM/PNG,
- échantillonner des coordonnées précises (ex. barre : x=600, y=12),
- un écart de teinte > ~15/ canal par pixel = écart à corriger.
Pièges QEMU déjà rencontrés (voir passation) : attendre le socket
wayland dans la boucle, `kill -HUP` au dock externe, une fenêtre idle
ne repeint pas sa SSD au SIGHUP, greps sans accents sur la console
série, `ISO=…desktop` explicite avec `t-gfx-live.py`.

## Preuves jointes
- `preuves/reel-classic.png`, `reel-macos.png`, `reel-tiling.png` :
  dumps réels v0.7 en glass-glacier (1280×720).
- `preuves/bureau-glass-glacier.png` : aperçu v0.7 de référence.
