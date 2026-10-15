# System Services — LLD

**Statut :** Accepté
**Version :** 0.3
**Dernière mise à jour :** 2026-10-15
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

## Configuration (v0.2)

Les fichiers de configuration sont fournis dans **`config/rootfs/`** (arborescence
reflétant la racine du système, cf. `deployment-ops.md`) et copiés par la phase de
configuration de l'étape 04.

| Fichier | Service | Rôle |
|---|---|---|
| `/etc/systemd/network/20-wired.network` | `systemd-networkd` | DHCP sur le filaire |
| `/etc/systemd/network/25-wireless.network` | `systemd-networkd` | DHCP sur le Wi-Fi |
| `/etc/systemd/resolved.conf.d/yaor.conf` | `systemd-resolved` | pas de DNS de repli ni de DNSSEC |
| `/etc/iwd/main.conf` | `iwd` | couche 2 seule (`EnableNetworkConfiguration=false`) |
| `/etc/pam.d/*` | `login`, `sudo` | chaînes PAM ; `pam_systemd` dans `system-session` |
| `/etc/sudoers`, `/etc/sudoers.d/00-wheel` | `sudo-rs` | administration par le groupe `wheel` |
| `/etc/polkit-1/rules.d/49-yaor-udisks.rules` | `polkit` | actions `udisks2` pour `wheel` |

Services **activés** au démarrage : `systemd-networkd`, `systemd-resolved`, `iwd`,
`udisks2`, `dbus.socket` et `systemd-logind`. `systemd-logind` conserve les **valeurs par
défaut** (aucun `logind.conf.d` n'est fourni — comportement multi-utilisateur standard).
`/etc/resolv.conf` pointe vers `/run/systemd/resolve/stub-resolv.conf` ; le groupe `wheel`
et `/var/lib/iwd` sont créés à la construction.

## Règles

Chaque service doit préciser sa responsabilité, ses dépendances, son interface, son
démarrage, son arrêt, ses erreurs, ses permissions et sa stratégie de remplacement.

YAOR ne crée pas un service personnalisé lorsqu'un service existant adapté répond au
besoin sans justification contraire (ADR-002).
