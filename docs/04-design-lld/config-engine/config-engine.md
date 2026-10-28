# Configuration Engine — LLD (jalon v0.5, étape 1)

## Objectif

Permettre de modifier le comportement du bureau sans modifier le code :
lecture d'un `desktop.toml` par le compositor, rechargement à chaud,
journalisation de la configuration effective.

## Réalisation (étape 1 — crate `components/yaor-config`)

```text
components/yaor-config/          # membre du workspace, version produit 0.2.0
├── src/lib.rs                   # DesktopConfig + Default, lecture TOML, tests
└── default.toml                 # valeur d'usine (mode cascade)
```

- Format **TOML** (décision produit, 28 octobre 2026) ; serde + toml
  sont repris dans l'arbre vendu de Smithay 0.7.0 (ADR-017/018) —
  aucune dépendance registre nouvelle, `Cargo.lock` committé
  (ADR-016), build 100 % hors-ligne (ADR-018), toolchain hôte
  (ADR-019), version produit 0.2.0 (ADR-022).
- **Chemins candidats** (premier trouvé gagne, journalisé) :
  1. `YAOR_DESKTOP_CONFIG` (variable d'environnement) ;
  2. `${XDG_CONFIG_HOME:-~/.config}/yaor/desktop.toml` ;
  3. `/etc/yaor/desktop.toml` — valeur d'usine posée par la phase de
     build (`scripts/04_chroot_build.sh`) depuis `default.toml`.
- Fichier **partiel** : toute section/option absente est complétée par
  les défauts compilés (dérivés de serde, déclarés dans lib.rs) ;
- Journal du compositor : la source et les options sont citées
  (« configuration chargée : /root/.config/yaor/desktop.toml »,
  « mode master-stack, ratio 0.500, gaps 0, raccourcis close=… ») —
  preuves QA en jetons (`t-yaor-config.py` 20/20).

## Rechargement à chaud

**SIGHUP** traité par le compositor (`compositor/src/config.rs`) :
relecture du fichier candidat, mise à jour en place (mutateur
`config::update`, aussi utilisé par les raccourcis Mod+g /
Mod+Shift+g) et re-journalisation — la session KMS continue, la
socket Wayland reste publiée. TEST-CONFIG-001/TEST-DESK-001 : ratio
0.7 + gaps 10 puis retour 0.5 + gaps 0, dispositions re-mesurées aux
screendumps ; le rebind close=Mod+X est prouvé à l'usage (Mod+Q
inactif, Mod+X ferme la fenêtre focusée).

## Arborescence (consolidée pour v0.5)

```text
~/.config/yaor/
└── desktop.toml        # v0.5 étape 1 : [layout] + [keys]
```

Les fichiers avancés (theme.toml, windows.toml, panel.toml,
launcher.toml, accessibility.toml) restent v0.6+, rattachés aux
profils Caméléon (REQ-SW-007) ; la seule configuration de v0.5 est
`desktop.toml`. Schéma exact :
[`data-models/config-schema.md`](../data-models/config-schema.md).
