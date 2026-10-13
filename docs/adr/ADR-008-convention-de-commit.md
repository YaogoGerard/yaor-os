# ADR-008 — Convention de commit

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000

## Contexte

Les commits doivent permettre d'identifier **l'étape (jalon)** et **la fonctionnalité**
en cours, et de comprendre **l'objet exact** du changement. Sans convention, l'historique
devient inexploitable pour suivre l'avancement par jalon.

## Options envisagées

- **Option A — Conventional Commits + trailers `Jalon`/`Feature`/`Refs`.**
- **Option B — Jalon et fonctionnalité dans le scope du type.**
- **Option C — Format libre structuré (sections Contexte/Contenu/Impact).**

## Décision

**Option A.** Les messages suivent le format :

```text
<type>(<zone>): <sujet à l'impératif>

<Corps : pourquoi, quoi, impact. Obligatoire si non trivial.>

Jalon: <...>
Feature: <slug>
Refs: <...>
```

- Sujet et corps en **français** ; types, zones et trailers en anglais.
- Les trailers **`Jalon:`** et **`Feature:`** sont **obligatoires**.
- La convention est **documentée sans outillage** : aucun hook ni modèle imposé pour
  l'instant (voir Conséquences).

## Conséquences

- **Positives :** historique lisible, suivi par jalon et par fonctionnalité, traçabilité
  vers les ADR/REQ/TEST.
- **Négatives / risques :** sans hook, le respect repose sur la discipline ; à outiller
  plus tard si nécessaire.
- **Suivi :** la convention complète vit dans `docs/07-contributing/commits.md`.
