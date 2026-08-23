# PRD — yaor-os (Live OS)

**Statut :** Brouillon
**Version :** 0.2
**Dernière mise à jour :** 2026-08-23

---

## 1. Vision

Construire, entièrement à partir des sources (façon Linux From Scratch), un système
d'exploitation Linux live minimal en ligne de commande, qui démarre depuis une clé USB
sans nécessiter d'installation ni de connexion Internet. Ce live OS sert de fondation
technique à une future distribution communautaire destinée au Burkina Faso et à
l'Afrique.

## 2. Problème résolu

Les distributions Linux généralistes embarquent souvent un grand nombre de paquets et
de services non nécessaires pour un contexte à faible bande passante, ce qui alourdit
le téléchargement, l'installation et la maintenance. Un système construit sur mesure,
maîtrisé de bout en bout, permet de ne garder que l'essentiel et d'adapter chaque choix
au contexte réel d'usage.

## 3. Personas (utilisateurs types)

- **Développeur curieux** : veut tester un environnement Linux minimal sans rien
  installer sur son disque, en bootant simplement une clé USB.
- **Contributeur du projet (Tos-Team / Burkina Open Lab)** : veut comprendre et
  modifier le processus de construction du système pour proposer des améliorations.
- **Futur utilisateur final de la distribution communautaire** : bénéficiera plus tard
  d'une version installable, construite sur cette même fondation.

## 4. Fonctionnalités principales (scope de cette version)

- Démarrage depuis une clé USB, en mode BIOS et en mode UEFI
- Système en ligne de commande, fonctionnel sans interface graphique
- Userland « oxidized » : outils Rust accessibles sous les noms classiques
  (uutils/coreutils en natif ; bat, ripgrep, fd, dust, procs, hexyl via aliases
  interactifs)
- Socle GNU minimal conservé pour la fiabilité des scripts (bash, grep, sed,
  gawk, tar...)
- Toolchain et système construits à partir des sources (GNU : GCC, Glibc, Binutils)
- Pipeline de construction automatisé et reproductible (GitHub Actions)
- Image ISO comme livrable final

## 5. Hors scope (ce qu'on NE fait PAS dans cette version)

- Pas d'installateur (ni graphique type Calamares, ni en mode texte) — le système
  reste "live only" pour l'instant
- Pas d'interface graphique (Sway/Wayland reste un objectif futur, non démarré)
- Pas d'extension du périmètre Rust au-delà du userland : noyau, toolchain de
  construction et initramfs restent respectivement en Linux / GNU / BusyBox

## 6. Roadmap

- **v1** : pipeline de build fonctionnel, ISO live CLI bootable (BIOS + UEFI),
  avec userland oxidized (outils Rust sous noms classiques)
- **v2** : installateur en ligne de commande (sans interface graphique)
- **v3** : première interface graphique minimale (Sway/Wayland)
- **Au-delà** : élargissement progressif du périmètre Rust et des services selon
  les besoins de la communauté
