# Cahier de recette — yaor-os

**Statut :** Brouillon
**Version :** 0.2

## TEST-001 (vérifie REQ-SW-001)
**Étapes :**
1. Démarrer l'ISO générée dans QEMU en mode BIOS
2. Démarrer l'ISO générée dans QEMU en mode UEFI (OVMF)

**Résultat attendu :** le système démarre avec succès dans les deux cas, jusqu'à
l'invite de commande.

## TEST-002 (vérifie REQ-SW-002)
**Étapes :**
1. Modifier volontairement une archive source pour corrompre son contenu
2. Lancer `01_download.sh`

**Résultat attendu :** le script détecte la somme de contrôle invalide et arrête le
pipeline avec une erreur claire.

## TEST-003 (vérifie REQ-SW-006)
**Étapes :**
1. Lancer un build complet jusqu'à `05_make_iso.sh`
2. Mesurer la taille du fichier `.iso` généré

**Résultat attendu :** la taille est inférieure à 1 Go.

## TEST-004 (vérifie REQ-SW-007)
**Étapes :**
1. Démarrer l'ISO dans QEMU
2. Observer l'écran après le démarrage

**Résultat attendu :** le système affiche directement une invite de commande, sans
menu d'installation ni interface graphique.

## TEST-005 (vérifie REQ-SW-005)
**Étapes :**
1. Lister le contenu de l'ISO générée (`xorriso -indev … -find /`)
2. Chercher le fichier `filesystem.squashfs`

**Résultat attendu :** le rootfs est bien présent sous forme de SquashFS dans l'image.

## TEST-006 (vérifie REQ-SW-003, REQ-SW-004, REQ-SW-008)
**Étapes :**
1. Pousser sur la branche principale
2. Observer l'exécution du pipeline GitHub Actions

**Résultat attendu :** le pipeline s'exécute de bout en bout sans étape manuelle ;
la toolchain temporaire est compilée avant la construction chroot (ordre des jobs),
ce qui vérifie REQ-SW-003 et REQ-SW-004 par construction.

## TEST-007 (vérifie REQ-SW-009)
**Étapes :**
1. Démarrer l'ISO dans QEMU
2. Vérifier la présence et le fonctionnement de `ls` (uutils), puis de `bat`,
   `rg`, `fd`, `dust`, `procs` et `hexyl`

**Résultat attendu :** chaque outil répond correctement à sa commande de test
(`--version` ou usage basique).

## TEST-008 (vérifie REQ-SW-010, REQ-SW-011)
**Étapes :**
1. Dans le shell interactif : exécuter `type cat`, `type grep`, `type find`
2. Exécuter un script non interactif contenant `command -v cat` et `command -v grep`
3. Dans le shell interactif : exécuter `\grep --version`

**Résultat attendu :** en interactif, les noms classiques pointent vers les outils
Rust (bat, ripgrep, fd) ; dans le script non interactif, ce sont les binaires réels
qui sont résolus ; `\grep` invoque bien le binaire GNU.

## Couverture REQ → TEST

| Exigence | Test |
|---|---|
| REQ-SW-001 | TEST-001 |
| REQ-SW-002 | TEST-002 |
| REQ-SW-003 | TEST-006 |
| REQ-SW-004 | TEST-006 |
| REQ-SW-005 | TEST-005 |
| REQ-SW-006 | TEST-003 |
| REQ-SW-007 | TEST-004 |
| REQ-SW-008 | TEST-006 |
| REQ-SW-009 | TEST-007 |
| REQ-SW-010 | TEST-008 |
| REQ-SW-011 | TEST-008 |
