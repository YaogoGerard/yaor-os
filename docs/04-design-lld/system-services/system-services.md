# System Services — LLD

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-14
**Décisions liées :** ADR-004, ADR-005, ADR-013, ADR-014

## Init et supervision

YAOR utilise **systemd** comme init et gestionnaire de services (ADR-004).

## Services de base

| Service | Rôle | Décision |
|---|---|---|
| `systemd-logind` | sessions, sièges, cgroups | réactivé (ADR-014, REQ-SW-021) |
| D-Bus (`dbus`) | bus système et utilisateur | réintroduit (ADR-014) |
| `systemd-networkd` | réseau filaire (DHCP) | ADR-013 |
| `systemd-resolved` | résolution de noms (DNS) | ADR-013 |
| `iwd` | Wi-Fi (avec `linux-firmware`) | ADR-013 |
| `udisks2` | montage automatique des volumes amovibles | ADR-014 |
| polkit | autorisation des actions privilégiées | ADR-014 |
| Audio | pile audio | candidat PipeWire, à trancher par ADR (Phase 2) |

## Règles

Chaque service doit préciser sa responsabilité, ses dépendances, son interface, son
démarrage, son arrêt, ses erreurs, ses permissions et sa stratégie de remplacement.

YAOR ne crée pas un service personnalisé lorsqu'un service existant adapté répond au
besoin sans justification contraire (ADR-002).
