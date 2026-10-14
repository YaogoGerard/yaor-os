# ADR-016 — Reproductibilité : niveau fonctionnel

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-007

## Contexte

`REQ-SW-016` demande un build **reproductible à partir de sources et versions
explicitement définies**. Le projet épingle déjà les versions et vérifie les empreintes
SHA-256 des sources (ADR-007). Reste à fixer le **niveau** de reproductibilité visé : un
build **bit-à-bit identique** impose de maîtriser horodatages, ordre des fichiers, locale,
chemins et métadonnées — un effort important pour un lot construit depuis les sources
(noyau, Rust, systemd…).

## Options envisagées

- **Bit-à-bit** — garantie maximale (attestation forte), coût et complexité élevés.
- **Fonctionnelle** — versions et empreintes épinglées ; à révision donnée, le build
  produit le même système, sans exiger l'identité binaire octet pour octet.
- **Aucune** — écarté : contredit `REQ-SW-016`.

## Décision

**Reproductibilité fonctionnelle.** Pour une révision donnée, les **sources et versions
sont épinglées et vérifiées** (SHA-256) et le résultat est **fonctionnellement identique**.
La reproductibilité **bit-à-bit** n'est **pas** un objectif de v0.2.

Là où c'est simple, les pratiques favorables (ex. `SOURCE_DATE_EPOCH`) sont appliquées,
sans être une condition de sortie.

## Conséquences

- **Positives :** objectif atteignable et vérifiable ; cohérent avec `REQ-SW-016` ; ne
  contraint pas la construction depuis les sources.
- **Négatives / risques :** pas d'attestation binaire ; une dérive d'outillage hôte
  (compilateur, horodatage) peut produire des artefacts binairement différents.
- **Suivi :** durcir vers le bit-à-bit ultérieurement si un besoin d'attestation apparaît ;
  un ADR le trancherait alors.
