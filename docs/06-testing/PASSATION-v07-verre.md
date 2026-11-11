# Passation v0.7 « Verre » — intégration et validation (11 nov. 2026)

Intégration de la livraison du collaborateur (`yaor-v07-verre/`, cf. son
`LISEZMOI.md` et `docs/PASSATION-v07-verre.md`) dans le dépôt, build ISO,
validation QEMU complète. ADR-025 (dans la livraison) fait foi pour les
choix de conception ; le vrai flou des fenêtres est v0.8.

## Intégration
- Fusion zip : `components/` écrasé par la livraison (delta 15 fichiers +
  Cargo.toml/lock). `cargo check --workspace --offline` : **0 erreur,
  0 warning**. Les 4 risques de compilation prédits par l'auteur se sont
  révélés sans objet (aucune retouche de son code).
- `yaor-settings` ajouté à `scripts/04_chroot_build.sh` (compile, install
  dans /usr/bin, vérification) ; ISO desktop régénérée.
- Tests : 117 verts au total (config 31, ui 31, settings 7, suites v0.6).

## Validation QEMU (ISO desktop)
- 7/7 aperçus `previews/` confirmés visuellement (accents, police corrigée :
  « YAOR » percé, descendantes complètes).
- Sondes pixel par préset : barres de titre SSD en verre, accents suivent
  à chaud (ambre/glacier/améthyste/émeraude), retour plat = rendu v0.6 exact.
- `qa/qa-glass.sh` durci : **26 contrôles PASS** (présets, nom/style relus,
  jetons panel+dock « verre : actif/plat », borne blur 999→48, nom custom,
  refus code 2, retour plat).
- Session desktop réelle : panel journalise « verre : plat → actif », barre
  en verre glacier (pilule YAOR bleue), `yaor-settings` (640×560 mappée) et
  `yaor-dock` (2 tuiles) connectés et rendus.
- « Fond flouté » prouvé (2 lignes au journal) après interaction pointeur.

## Leçons d'environnement (à réutiliser)
1. **Socket wayland** : l'unité `yaor-session` est `Type=simple`, « active »
   ≠ socket prêt. Attendre `-S /run/user/0/wayland-N`, avec le nom résolu
   DANS la boucle (avant l'exec du compositor, `ls` est vide).
2. **SIGHUP aux enfants** : un client lancé hors session (dock de test)
   ne reçoit pas le SIGHUP du compositor → `kill -HUP` explicite.
3. **Repaint idle** : `material()` ne s'exécute que si la barre SSD
   redessine ; une fenêtre idle ne repeint pas au SIGHUP — survol/clic
   déclenche la construction du verre (comportement, pas un bug).
4. **Console série** : les motifs accentués échouent (écho tty) — greps
   ASCII (« fond flout » couvre « flouté »). Injection de fichiers :
   base64 par tranches ~600 chars (une ligne de 3,4 Ko est corrompue).
5. `t-gfx-live.py` : l'ISO par défaut est la **live** ; toujours passer
   `ISO=...desktop` explicitement pour tester l'ISO desktop.

## Écarts et restes
- La peau verre couvre les 3 profils (mêmes chemins panel/compositor) ;
  classic et macos prouvés visuellement, **tiling** non dumpé (héritage
  des mêmes surfaces, sans travail spécifique).
- Pas dans la livraison (brief v0.7 restant) : drag incliné, loupe du
  dock, fermeture animée, transitions VT, éditeur/moniteur.
- La peau ne se voit qu'après `yaor-ctl theme set glass-*` : l'usine
  est `flat` (rendu v0.6 exact) + profil classic, conformément à ADR-025.
