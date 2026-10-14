# ADR-013 — Réseau : systemd-networkd, systemd-resolved, iwd

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-004, ADR-009, ADR-011

## Contexte

La décision fondatrice #16 laissait les **outils** réseau « non figés ». Le jalon 0.1 a
volontairement désactivé tout le Wi-Fi (`CFG80211`) faute d'OpenSSL (voir
`deployment-ops.md`) ; seul le réseau filaire virtuel (virtio-net) était disponible. Le
jalon v0.2 doit rendre le réseau **fonctionnel**, filaire **et** Wi-Fi, sans alourdir le
système.

Le système utilise déjà systemd comme init et gestionnaire de services (ADR-004) et
retient par défaut des composants modernes, maintenus et performants (ADR-009, ADR-002).

## Options envisagées

- **NetworkManager** — riche (Wi-Fi, VPN, GUI) mais lourd, à nombreuses dépendances et
  redondant avec systemd.
- **systemd-networkd (+ systemd-resolved)** — natif systemd, léger, cohérent avec
  l'écosystème retenu ; la gestion Wi-Fi est déléguée à un démon dédié.
- **Outils manuels** (`ip`, `dhcpcd`) — minimal mais peu intégré, sans connexion
  persistante ni intégration de session.

Pour le Wi-Fi :

- **`iwd`** — moderne, léger, s'intègre à networkd et à la résolution de noms ;
- **`wpa_supplicant`** — référence historique, plus lourd.

Aucun démon Wi-Fi **mature écrit en Rust** n'existe ; ADR-002 prévoit explicitement le
repli vers une solution existante adaptée.

## Décision

- **Réseau filaire : `systemd-networkd`** (client DHCP intégré).
- **Résolution de noms : `systemd-resolved`**.
- **Wi-Fi : `iwd`** (repli C assumé, faute d'alternative Rust mature) avec
  **`linux-firmware`**.
- Aucun démon réseau secondaire (NetworkManager) n'est embarqué.

## Conséquences

- **Positives :** pile réseau légère et cohérente avec systemd ; pas de double modèle de
  gestion réseau ; Wi-Fi moderne.
- **Négatives / risques :** la pile requiert une configuration explicite (pas d'interface
  graphique) ; l'activation du Wi-Fi lève la contrainte OpenSSL du jalon 0.1.
- **Suivi :** lors de l'implémentation, trancher entre ajouter **OpenSSL** au lot ou
  désactiver `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` ; documenter la configuration réseau
  par défaut ; tester le Wi-Fi sur du matériel réel.
