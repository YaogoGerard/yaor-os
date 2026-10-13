# YAOR Compositor — LLD

## Responsabilité
Fournir le compositor graphique propre à YAOR et connecter le Desktop Core à la pile graphique.

## Base candidate
Smithay est une base candidate afin de ne pas réimplémenter les primitives Wayland et compositor.

## Responsabilités YAOR
- intégration Desktop Core ;
- gestion des politiques de fenêtre ;
- intégration des profils ;
- intégration accessibilité ;
- configuration ;
- extensions spécifiques.

YAOR ne doit pas réimplémenter inutilement DRM/KMS, pilotes GPU ou protocoles existants déjà adaptés.
