# Themes

**Statut :** prévu — *mise à jour : 29 octobre 2026*

Les thèmes sont des ensembles de configuration visuelle. Un thème ne
doit pas modifier la logique du Desktop Core (REQ-SW-015 :
représentation déclarative — un thème est une section de la
configuration `desktop.toml`, rechargée par SIGHUP, voir
[`../user-flows.md`](../user-flows.md)).

Contenu prévu d'un thème (limité aux primitives du
[`../design-system.md`](../design-system.md)) :

- palette (rôles de `branding/` : fonds, accent, bordure, texte) ;
- présentation optionnelle de surface (dans les bornes du design
  system) — pas de police vectorielle à ce stade (bitmap 5×7).

Exemples futurs : YAOR Default (ambre sur noir), Dark, Light,
High Contrast (accentué pour l'accessibilité, voir
[`../accessibility.md`](../accessibility.md)) et thèmes
communautaires.

État : aucun thème livré — la palette « ambre sur noir » est
actuellement la constante des composants, pas encore paramétrable.
