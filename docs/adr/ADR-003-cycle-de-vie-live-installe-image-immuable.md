# ADR-003 — Cycle de vie : live + installé, image immuable

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-001, ADR-004, ADR-007

## Contexte

Aucune documentation ne précisait si YAOR est « live », « installé » ou les deux, ni
comment corriger un système après déploiement. Cela conditionne le partitionnement, le
système de fichiers (lecture seule, overlayfs, persistance) et le modèle de mise à jour.

## Options envisagées

- **Option A — Live uniquement.**
- **Option B — Live puis installable ultérieurement.**
- **Option C — Live + installé dès la v1, avec mises à jour par image.**

## Décision

**Option C.** YAOR est livré à la fois comme **système live** (démarrage depuis clé
USB/CD) et comme **système installable** dès la v1.

Les mises à jour se font par **image immuable** : le système est remplacé dans son
ensemble (mécanisme A/B ou réinstallation), et non corrigé par un gestionnaire de
paquets classique. Les images distribuées sont signées (ADR-007).

## Conséquences

- **Positives :** cohérent avec un OS construit depuis les sources ; mises à jour
  atomiques et robustes.
- **Négatives / risques :** nécessite un schéma de partitionnement et un mécanisme A/B ;
  la persistance des données utilisateur devient un point de conception majeur.
- **Suivi :** ajouter des exigences de cycle de vie, de partitionnement et de persistance
  (SRS) ; préciser le rôle du packaging (`package-architecture.md`).
