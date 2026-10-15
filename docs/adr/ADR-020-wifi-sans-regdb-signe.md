# ADR-020 — Wi-Fi : cfg80211 sans registre réglementaire signé

**Statut :** Accepté
**Date :** 2026-10-15
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-002, ADR-004, ADR-007, ADR-009, ADR-013

## Contexte

ADR-013 retient **`iwd`** (avec `linux-firmware`) pour le Wi-Fi. Or le noyau fait
dépendre **tout le Wi-Fi** d'**OpenSSL** : `CONFIG_CFG80211` sélectionne
`CONFIG_CFG80211_REQUIRE_SIGNED_REGDB` (valeur par défaut `y`, **sans invite de
configuration**), qui sélectionne `SYSTEM_DATA_VERIFICATION`, lequel sélectionne
`SYSTEM_TRUSTED_KEYRING`. La construction des certificats du noyau
(`scripts/extract-cert.c`) exige alors **OpenSSL**.

Au jalon 0.1, la contrainte de légèreté a conduit à **désactiver tout le Wi-Fi**
(`# CONFIG_CFG80211 is not set`, voir `config/kernel-live.fragment` et
`deployment-ops.md`). Le jalon v0.2 doit rendre le Wi-Fi **fonctionnel**. ADR-013 a
explicitement laissé ce point « à trancher lors de l'implémentation ».

## Options envisagées

- **Ajouter OpenSSL au lot** — permet le registre réglementaire signé **et** la
  signature de modules ; deviendra nécessaire pour la signature des images
  (`ADR-007`). Mais alourdit le socle minimal dès v0.2, pour un besoin non démontré.
- **Désactiver `CONFIG_CFG80211_REQUIRE_SIGNED_REGDB`** — garde le socle minimal ;
  renonce à la vérification **par signature** de la base réglementaire.
- **Reporter le Wi-Fi hors de v0.2** — contraire à ADR-013 ; écarté.

## Décision

- **`CONFIG_CFG80211_REQUIRE_SIGNED_REGDB=n`** : le Wi-Fi (`CFG80211`, `MAC80211` et
  les pilotes nécessaires) est **activé** sans introduire OpenSSL.
- **OpenSSL reste hors du lot v0.2.** Il sera introduit lorsqu'un besoin avéré le
  justifiera — au premier chef la **signature des images / `systemd-sysupdate`**
  (`ADR-007`, `REQ-SW-022`).
- `CONFIG_MODULE_SIG` reste **`n`** (cohérent : aucune infrastructure de clés/signature
  n'est embarquée à ce stade).

## Conséquences

- **Positives :** Wi-Fi opérationnel sans alourdir le socle ; aucune dépendance OpenSSL
  pour construire le noyau ; cohérent avec la contrainte de légèreté et ADR-009.
- **Négatives / risques :** la base réglementaire n'est pas vérifiée par signature
  (impact faible en live) ; pas de signature de modules ; le Wi-Fi réel dépend en plus
  de `linux-firmware` et du chargement de modules par `kmod`.
- **Suivi :** réaligner `config/kernel-live.fragment` (activer `CFG80211`/`MAC80211`,
  les pilotes et `FW_LOADER` ; retirer la mise à l'écart du jalon 0.1) et
  `deployment-ops.md` ; prévoir l'**ajout d'OpenSSL** avec la mise en œuvre
  d'ADR-007 (signature ISO et mises à jour).
