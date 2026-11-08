# ADR-025 — Durcissement de la session graphique

**Statut :** Proposé
**Date :** 2026-11-08
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-007, ADR-014, ADR-024

## Contexte

Le jalon v0.6 « Caméléon » livre la session graphique réelle : compositor Smithay
(`--tty-udev`, libseat), panel, dock, taskbar et profils de bureau. La posture
sécurité **actuelle** est explicitement root de bout en bout :

- le compositor tourne en root (accès direct DRM/evdev via libseat) ;
- les clients de session (`yaor-panel`, `yaor-dock`) tournent en root ;
- la QA QEMU s'exécute en root et `XDG_RUNTIME_DIR=/run/user/0` ;
- seatd reste le repli documenté (chaînage `YAOR_LIBSEAT_BACKEND=seatd`).

Cet état est un **expédient de jalon**, pas une décision : ADR-007 a posé le socle
approvisionnement (empreintes SHA-256, épinglage, ISO signée) et renvoie
explicitement le « durcissement d'exécution » à un ADR ultérieur. ADR-014 a
rétabli `logind` + PAM + polkit et le multi-utilisateur POSIX (REQ-SW-021,
REQ-SW-023) — mais la session graphique ne les exploite pas encore.

L'écart à trancher : un compositeur Wayland en root **contredit REQ-SW-023**
(moindre privilège) et annule le bénéfice de sécurité principal du modèle Wayland —
l'isolation entre clients par sockets nommés par utilisateur, qui n'a de sens que
si les clients ne partagent pas l'identité du compositeur.

## Options envisagées

- **Option A — Statu quo root, documenté comme tel.** Rien ne change ; le
  durcissement est reporté sine die. Simple, mais l'écart REQ-SW-023 devient
  permanent et chaque client ajouté (v0.7+) élargit la surface root.
- **Option B — Session non privilégiée.** Le compositeur est démarré dans une
  session `logind` d'un utilisateur non privilégié (backend `logind` de libseat,
  déjà prioritaire dans `session.rs`) ; seatd demeure le repli avec appartenance
  aux groupes `video`/`input` ; `XDG_RUNTIME_DIR=/run/user/UID` réel.
- **Option C — Option B + durcissements complémentaires.** Permissions des
  sockets Wayland, durcissement systemd des unités de session
  (`ProtectSystem=strict`, `NoNewPrivileges`, liste blanche de devices), revue
  des setuid résiduels, politique de gestion des clés de signature ISO
  (initialement prévue par ADR-007).

## Décision

**Option B retenue comme cible du prochain jalon de sécurité (v0.7), proposée
aujourd'hui :**

1. Le compositor et les clients de session tournent sous un **compte utilisateur
   non privilégié**, dans une session `logind` (ADR-014) — le mode installé ne
   démarre plus de session graphique en root.
2. libseat conserve la priorité **logind** ; seatd reste repli pour les
   environnements sans D-Bus, moyennant l'appartenance aux groupes
   `video` et `input`.
3. **root reste acceptable uniquement** dans le banc QA QEMU actuel (ISO live de
   développement, aucun compte réel) et tant que le mode installé ne crée pas
   encore d'utilisateur final.
4. Les points de l'option C ne sont **pas** tranchés ici : chacun fera l'objet
   d'une décision séparée, au fur et à mesure que les surfaces concernées
   apparaissent (sockets multi-utilisateurs, unités systemd, clés de signature).

## Conséquences

- **Positives :** REQ-SW-021/023 effectifs pour la session graphique ; le modèle
  de sécurité Wayland (isolation par socket utilisateur) devient réel ; le mode
  installé (ADR-015) part sur des bases moindre-privilège.
- **Négatives / risques :** les harnais QA (`t-yaor-*.py`) devront rejouer en
  non-root ; le live et l'installateur doivent créer un compte de session et ses
  groupes ; le repli seatd suppose l'appartenance `video`/`input` (sinon échec
  d'ouverture DRM).
- **Suivi :** ajouter les exigences de durcissement au SRS ; ajuster
  `scripts/` (création du compte, groupes) ; rejouer la QA QEMU en non-root ;
  politique de clés toujours pendante côté ADR-007 ; réévaluer à l'ouverture de
  v0.7.
