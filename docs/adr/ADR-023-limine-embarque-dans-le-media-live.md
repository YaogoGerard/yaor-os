# ADR-023 — Limine embarqué dans le média live

**Statut :** Accepté
**Date :** 2026-10-17
**Décisionnaire(s) :** Équipe YAOR
**Décisions liées :** ADR-000, ADR-001, ADR-003, ADR-010, ADR-011, ADR-015

## Contexte

ADR-015 retient un **installateur** (`yaor-install`) qui écrit le système sur
disque et configure **Limine** sur le disque cible (BIOS et UEFI, REQ-SW-019).
Pour cela, l'installateur — exécuté depuis le **média live** — a besoin de
l'outil Limine et de ses fichiers de démarrage :

- `/usr/bin/limine` (sous-commande `bios-install`) ;
- `/usr/share/limine/` (`BOOTX64.EFI`, `limine-bios.sys`, …).

Or, jusqu'ici, Limine n'existait **que sur la machine de construction** : le média
live ne l'embarquait pas, si bien que `yaor-install` ne pouvait pas écrire le
chargeur sur un disque. Le LLD installateur relevait ce point comme ouvert, en
notant le conflit apparent avec ADR-001 (build depuis les sources).

ADR-010/ADR-011 ont déjà tranché la nature de Limine : c'est un **outil de build
de l'hôte**, présent sur la machine qui assemble l'image, au même titre que
`xorriso` ou `mkfs.erofs` — jamais construit par le pipeline YAOR.

## Options envisagées

- **Construire Limine depuis les sources dans le pipeline** — alourdirait le
  build d'un compilateur et d'une chaîne freestanding ; contredit ADR-010/011 qui
  le rangent parmi les outils d'hôte ; double le travail pour le même résultat.
  Écarté.
- **Copier Limine depuis l'hôte dans l'image live** — cohérent avec ADR-010/011 :
  l'outil existe déjà sur l'hôte ; il suffit de le livrer. **Retenu.**
- **Ne pas embarquer Limine ; l'installateur échouerait** — rend le mode installé
  impossible depuis le live. Écarté.

## Décision

- L'étape **06** (`06_make_iso.sh`) copie, dans le rootfs de l'image live :
  - le **binaire** `limine` de l'hôte vers `/usr/bin/limine` ;
  - l'arborescence `/usr/share/limine/` de l'hôte vers `/usr/share/limine/`.
- Limine reste **absent du manifeste** et n'est **pas construit** par le
  pipeline : il est vérifié comme outil d'hôte (`00_check_host.sh`, `need_cmd
  limine`) et utilisé à la fois pour amorcer l'ISO (étape 06) et pour équiper le
  disque cible (via `yaor-install`).
- Le binaire hôte ne requiert que **glibc** ; l'image embarquant la même famille
  de `glibc`, il s'exécute dans le système live.

## Conséquences

- **Positives :** `yaor-install` dispose de tout ce qu'il lui faut pour écrire
  Limine (UEFI par copie de `BOOTX64.EFI`, BIOS par `limine bios-install`) ; le
  mode installé (ADR-015) devient réalisable depuis le média ; pas de nouvelle
  dépendance de construction ; ADR-001 est respecté (Limine n'est pas un
  composant du système, mais un outil de l'hôte — ADR-010/011).
- **Négatives / risques :** la version de Limine embarquée est **celle de
  l'hôte** — non épinglée par le dépôt ; l'image dépend de la compatibilité
  `glibc` hôte↔cible (Limine n'exige que la `libc`) ; l'espace occupé sur le
  média augmente d'environ 4 Mio.
- **Suivi :** épingler une version de Limine recommandée pour la construction
  (documentation des prérequis, `00_check_host.sh`) ; aligner `installer.md`,
  `deployment-ops.md` et `config/rootfs/README.md`.
