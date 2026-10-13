# Package Architecture

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-13
**Décisions liées :** ADR-001, ADR-003

## Principe

YAOR est construit **depuis les sources** (ADR-001) et mis à jour sous forme d'**image
immuable** (ADR-003). Il n'y a donc **pas de gestionnaire de paquets classique** au cœur
du système pour la v1 : APT/dpkg ne constituent pas le socle de construction.

```text
Build (sources)  ->  Image immuable  ->  Déploiement (live / installé)
```

## Ce que recouvre le « packaging » dans YAOR

- l'intégration reproductible des composants dans l'image ;
- les métadonnées et la configuration associées ;
- la préparation éventuelle de paquets applicatifs tiers.

## Évolution

Une couche d'intégration YAOR (Rust) pourra être étudiée plus tard pour des applications
externes, sans remettre en cause le build depuis les sources. Ce point fera l'objet d'un
ADR dédié.
