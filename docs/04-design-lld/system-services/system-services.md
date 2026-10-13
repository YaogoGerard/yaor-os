# System Services — LLD

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-13
**Décisions liées :** ADR-004, ADR-005

## Init et supervision

YAOR utilise **systemd** comme init et gestionnaire de services (ADR-004).

## Services de base attendus (à préciser par jalon)

| Service | Rôle |
|---|---|
| `systemd-logind` | sessions, sièges, cgroups (ADR-005) |
| Réseau | configuration réseau (outil non figé, décision 16) |
| Stockage | montage des volumes (outil non figé, décision 16) |
| Audio | pile audio (candidat : PipeWire, à trancher par ADR) |

## Règles

Chaque service doit préciser sa responsabilité, ses dépendances, son interface, son
démarrage, son arrêt, ses erreurs, ses permissions et sa stratégie de remplacement.

YAOR ne crée pas un service personnalisé lorsqu'un service existant adapté répond au
besoin sans justification contraire (ADR-002).
