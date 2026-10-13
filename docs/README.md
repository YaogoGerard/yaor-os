# YAOR OS — Documentation

Documentation industrielle du projet YAOR OS.

## Structure
- `adr/` — Décisions d'architecture (ADR)
- `01-product/` — Pourquoi le projet existe
- `02-requirements/` — Ce que le système doit faire
- `03-architecture-hld/` — Architecture haut niveau
- `04-design-lld/` — Design détaillé
- `05-ui-ux/` — Interface et expérience utilisateur
- `06-testing/` — Vérification et validation
- `07-contributing/` — Conventions de contribution (commits…)

## Conventions
- Corps des documents en français, titres et identifiants en anglais.
- Identifiants : `REQ-SW-`, `HRS-HW-`, `TEST-`, `ADR-`.
- En-tête standard : statut, version, dernière mise à jour, décisions liées.

## Principe
> **Reuse what exists. Prefer Rust when suitable. Build what is missing.**

YAOR utilise le noyau Linux comme fondation, réutilise les composants open source appropriés et privilégie les implémentations Rust lorsqu'elles sont suffisamment adaptées et matures.
