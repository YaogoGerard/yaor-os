# Business Rules — YAOR OS

## RG-001 — Réutilisation prioritaire
YAOR DEVRAIT utiliser un composant existant lorsqu'il satisfait le besoin fonctionnel et technique.

## RG-002 — Préférence Rust
Lorsqu'une implémentation Rust suffisamment adaptée existe, YAOR DEVRAIT la privilégier.

## RG-003 — Maturité
Une implémentation Rust expérimentale NE DOIT PAS remplacer automatiquement une solution mature uniquement parce qu'elle est écrite en Rust.

## RG-004 — Pas de réécriture inutile
YAOR NE DOIT PAS réimplémenter un composant existant sans justification technique documentée.

## RG-005 — Composants interchangeables
Les composants externes DEVRAIENT être intégrés de manière à pouvoir être remplacés ultérieurement.

## RG-006 — Configuration externe
Le comportement configurable du bureau DEVRAIT être défini par des fichiers de configuration lorsque cela est raisonnablement possible.

## RG-007 — Identité YAOR
Les composants développés spécifiquement pour YAOR DOIVENT se concentrer sur les fonctionnalités qui constituent l'identité du système.
