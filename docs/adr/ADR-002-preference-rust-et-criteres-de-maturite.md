# ADR-002 — Préférence Rust et critères de maturité

**Statut :** Accepté
**Date :** 2026-10-13
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000

## Contexte

`REQ-SW-005` imposait un compositor « développé principalement en Rust », alors que les
règles `RG-002`/`RG-003` et le PRD §5 interdisent de remplacer une solution mature au
seul motif qu'elle n'est pas en Rust. Les termes « mature » et « suffisamment adapté »
n'étaient définis nulle part, rendant la règle indécidable. Le compositor est
précisément le composant le plus risqué à écrire en Rust.

## Options envisagées

- **Option A** — Garder l'obligation Rust et exempter le compositor de la règle de maturité.
- **Option B** — Exigence de résultat, langage indifférent.
- **Option C** — Préférence Rust par défaut, repli documenté si la base Rust n'est pas
  assez mature/stable, avec critères objectifs.

## Décision

**Option C.** YAOR **préfère les solutions Rust** pour tout composant, **sauf** si la
base Rust disponible n'est pas suffisamment mature et stable ; dans ce cas une solution
existante adaptée est utilisée, avec justification dans un ADR.

Un composant candidat est réputé **mature et stable** s'il satisfait, de manière
documentée, l'ensemble de ces critères :

1. au moins une release stable (hors alpha/beta) ;
2. maintenance active (commit ou release de moins de 12 mois) ;
3. licence compatible avec la distribution de YAOR ;
4. couverture de tests significative et CI publique ;
5. documentation d'utilisation et d'intégration suffisante ;
6. adoption réelle (utilisé par des projets tiers identifiables).

`REQ-SW-005` est reformulée en conséquence : le langage devient une préférence, la
fonction (compositor Wayland opérationnel) reste l'exigence.

## Conséquences

- **Positives :** supprime la contradiction avec `RG-003` ; rend la règle opérationnelle
  et vérifiable.
- **Négatives / risques :** les critères doivent être réévalués périodiquement ; un
  composant peut cesser d'être « mature ».
- **Suivi :** appliquer ces critères à chaque `component-selection` et à chaque création
  de composant d'identité YAOR.
