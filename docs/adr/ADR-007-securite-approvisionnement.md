# ADR-007 — Sécurité de la chaîne d'approvisionnement

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-001, ADR-003

## Contexte

La documentation ne contenait aucune exigence de sécurité, alors que le build part de
sources (ADR-001) et que les mises à jour se font par image (ADR-003). Le risque de
compromission des sources ou de l'image distribuée n'était pas traité.

## Options envisagées

- **Option A — Empreintes des sources + ISO signée.**
- **Option B — Idem + durcissement complet (services minimaux, permissions, A/B).**
- **Option C — Exigence générale, détails en ADR ultérieur.**

## Décision

**Option A**, posée comme socle minimal :

1. Chaque source téléchargée est **vérifiée par empreinte SHA-256** avant usage.
2. Les versions et empreintes sont **épinglées** explicitement (reproductibilité).
3. L'**image ISO finale est signée**.
4. Aucun binaire non vérifié n'entre dans le build.

## Conséquences

- **Positives :** protège l'intégrité des sources et des images distribuées ; base
  compatible avec un build reproductible.
- **Négatives / risques :** ne couvre pas encore le durcissement d'exécution ; à
  compléter.
- **Suivi :** ajouter les exigences correspondantes au SRS ; le durcissement complet et
  la politique de gestion des clés feront l'objet d'ADR ultérieurs.
