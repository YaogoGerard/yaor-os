# ADR-006 — Accessibilité par réutilisation d'AT-SPI

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-005, ADR-000

## Contexte

`accessibility-architecture.md` suppose de « recevoir les événements et demander les
informations accessibles de l'élément ciblé », mais aucun standard d'extraction n'était
cité. L'accessibilité est une composante d'identité du projet (voir PRD).

## Options envisagées

- **Option A — Réutiliser AT-SPI (D-Bus).**
- **Option B — Protocole d'accessibilité propre à YAOR.**
- **Option C — Exigence posée, standard tranché plus tard.**

## Décision

**Option A : réutilisation d'AT-SPI (over D-Bus).** L'extraction des informations
accessibles s'appuie sur le standard AT-SPI, plutôt que d'inventer un protocole
(`RG-001`, ADR-002). L'`Accessibility Engine` YAOR se branche dessus.

## Conséquences

- **Positives :** compatibilité avec l'écosystème d'applications accessibles ; effort
  réduit.
- **Négatives / risques :** AT-SPI sous Wayland nécessite une attention particulière
  (bus d'accessibilité, pont). À valider en phase graphique.
- **Suivi :** mettre à jour `accessibility-architecture.md` et `accessibility.md` (LLD) ;
  préciser les moteurs de traduction et de TTS (dont le Mooré) par ADR distincts.
