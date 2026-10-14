# ADR-014 — Sessions, comptes et services de session

**Statut :** Accepté
**Date :** 2026-10-14
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-002, ADR-004, ADR-006, ADR-012

## Contexte

Le jalon 0.1 ne construit **ni D-Bus, ni `agetty`/`login`**, et `systemd-logind` y est
**masqué** (`systemd-logind.service` → `/dev/null`) précisément parce que D-Bus est absent
(voir `deployment-ops.md`). Or :

- `REQ-SW-021` impose que toute **session** s'appuie sur `logind` ;
- `REQ-SW-023` et la décision #15 imposent un système **multi-utilisateur POSIX** ;
- l'accessibilité (`AT-SPI`, ADR-006) reposera sur D-Bus ;
- les volumes amovibles doivent être montés automatiquement.

Le jalon v0.2 doit donc réintroduire la **couche de session** proprement.

## Options envisagées

- **Rester sans D-Bus ni `logind`** — impossible : viole `REQ-SW-021` et bloque
  l'accessibilité.
- **Pile Linux standard : D-Bus + `logind` + PAM + polkit** — cohérente avec systemd
  (ADR-004), nécessaire aux sessions, à l'élévation de privilèges et à AT-SPI.
- **Réimplémentation maison** — disproportionnée par rapport au besoin (ADR-002).

## Décision

- **D-Bus** (bus système **et** bus utilisateur) est réintroduit.
- **`systemd-logind`** est **réactivé** : les sessions sont suivies par `logind`
  (`REQ-SW-021`).
- **PAM** assure l'authentification ; **polkit** autorise les actions privilégiées.
- Le système est **multi-utilisateur POSIX** (`REQ-SW-015`, `REQ-SW-023`) :
  `getty`/`login` (util-linux) sont activés et les comptes vivent dans `/home`.
- L'**élévation de privilèges** se fait par **`sudo-rs`** (Rust, voir ADR-012).
- Le **montage automatique** des volumes amovibles est assuré par **`udisks2`**
  (déclenché par udev, via D-Bus) ; il n'existe pas d'équivalent Rust mature.

## Conséquences

- **Positives :** sessions conformes à `REQ-SW-021` ; base nécessaire à l'accessibilité
  (ADR-006) ; système réellement multi-utilisateur.
- **Négatives / risques :** la couche de session élargit le lot et la surface d'attaque ;
  elle doit être configurée et limitée avec soin (services actifs, règles polkit).
- **Suivi :** réactiver `systemd-logind` ; activer `getty`/`login` (util-linux) ;
  documenter les services dans `system-services.md` ; préciser les règles polkit.
