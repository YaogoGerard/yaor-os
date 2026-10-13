# PRD — YAOR OS

**Statut :** Brouillon  
**Version :** 0.1

## 1. Vision
YAOR est un système d'exploitation basé sur le noyau Linux qui assemble des composants open source existants et privilégie les implémentations Rust lorsqu'elles sont suffisamment adaptées et matures.

YAOR ne cherche pas à réimplémenter inutilement les composants existants. Le projet développe principalement les composants qui définissent son identité : environnement de bureau configurable, compositor, architecture Caméléon, système de plugins et accessibilité multilingue.

## 2. Problème résolu
YAOR cherche à fournir une base Linux cohérente dans laquelle le bureau peut changer de comportement et d'apparence par configuration, tout en réutilisant l'écosystème Linux existant.

## 3. Utilisateurs
- Utilisateurs Linux recherchant une expérience de bureau personnalisable.
- Utilisateurs avancés souhaitant un environnement configurable.
- Développeurs intéressés par Rust et les systèmes Linux.
- Utilisateurs ayant des besoins d'accessibilité multilingue.

## 4. Fonctionnalités principales
- Noyau Linux.
- Assemblage de composants open source.
- Préférence pour les composants Rust lorsqu'ils sont adaptés.
- Compositor développé principalement en Rust.
- Bureau configurable par fichiers.
- Profils Windows-like, macOS-like et tiling.
- Système de plugins.
- Couche d'accessibilité.
- Support vocal du Mooré comme objectif du projet.

## 5. Hors scope
- Réécrire le noyau Linux.
- Réécrire systématiquement tous les composants Linux.
- Remplacer un composant mature uniquement parce qu'il n'est pas écrit en Rust.
- Développer un composant interne lorsqu'une solution existante répond déjà correctement au besoin sans justification technique.

## 6. Principes
1. Réutiliser l'existant.
2. Privilégier Rust lorsque la solution est suffisamment adaptée.
3. Intégrer avant de remplacer.
4. Développer uniquement ce qui apporte une valeur propre à YAOR.
5. Maintenir les composants aussi interchangeables que possible.

## 7. Roadmap indicative
- v0.1 : système de construction et boot Linux minimal.
- v0.2 : userland fonctionnel et intégration des composants sélectionnés.
- v0.3 : base graphique.
- v0.4 : compositor YAOR.
- v0.5 : Desktop Core configurable.
- v0.6 : profils Caméléon.
- v0.7 : système de plugins.
- v0.8 : accessibilité.
- v1.0 : système intégré et documenté.
