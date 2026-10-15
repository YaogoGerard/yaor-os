# Installer & Updates — LLD

**Statut :** Accepté
**Version :** 0.1
**Dernière mise à jour :** 2026-10-15
**Décisions liées :** ADR-002, ADR-003, ADR-007, ADR-011, ADR-015, ADR-016

Ce document précise la **forme de l'installateur**, le **schéma de partitionnement** et le
**mécanisme de retour arrière** exigés par le suivi d'ADR-015.

## Responsabilités

Deux composants internes, en complément de `systemd-sysupdate` :

| Composant | Langage | Rôle |
|---|---|---|
| `yaor-install` | Rust | Installer le système sur disque (mode installé). |
| `yaor-slot` | Rust | Sélection du slot A/B et retour arrière avec Limine. |
| `systemd-sysupdate` | C (systemd) | Écriture/versionnage des images dans les slots. |
| `systemd-repart` | C (systemd) | Création idempotente du schéma GPT. |

`yaor-install` et `yaor-slot` sont développés en Rust (ADR-002) : aucune solution
existante adaptée ne couvre ce besoin propre à YAOR.

## Schéma de partitionnement (GPT)

| # | Partition | Type GPT (UUID) | FS | Taille | Rôle |
|---|---|---|---|---|---|
| 1 | ESP | `c12a7328-f81f-11d2-ba4b-00a0c93ec93b` | vfat | 512 Mio | Limine (UEFI + BIOS), noyaux, initramfs, `limine.cfg`, état des slots |
| 2 | BIOS boot | `21686148-6449-6e6f-744e-656564454649` | — | 1 Mio | seconde étape de Limine en BIOS sur GPT |
| 3 | Root A | `4f68bce3-e8cd-4db1-96e7-fbcaf984b709` | ext4 / btrfs | 2 Gio | racine, slot A |
| 4 | Root B | `4f68bce3-e8cd-4db1-96e7-fbcaf984b709` | ext4 / btrfs | 2 Gio | racine, slot B |
| 5 | Data | `0fc63daf-8483-4772-8e79-3d69d8477de4` | ext4 / btrfs | reste | `/var` persistant (`/home` en dessous) |

Règles :

- le partitionnement est **délégué à `systemd-repart`** (`/etc/repart.d/*.conf`), idempotent
  et sans outil de partitionnement supplémentaire ;
- chaque **slot racine** porte une image de système de fichiers **complète**, écrite d'un
  bloc par `systemd-sysupdate`, **montée en lecture seule** au démarrage (modèle immuable,
  ADR-003) ; la couche inscriptible est `/var` ;
- le **noyau et l'initramfs vivent sur l'ESP** : une mise à jour peut remplacer le noyau
  sans réécrire une racine ;
- `/var` n'est **jamais** dans un slot : il **survit** aux mises à jour et aux retours
  arrière ;
- `ext4` est le système de fichiers **par défaut** ; `btrfs` est une alternative (ADR-015).

## Installateur `yaor-install`

- **Forme :** binaire en ligne de commande. Sans argument, mode **guidé** (invites sur la
  console) ; avec arguments, mode **non interactif** (automatisation et tests). Aucune
  bibliothèque d'interface graphique n'est introduite.
- **Interface :**

  ```text
  yaor-install [--disk <périphérique>] [--fs ext4|btrfs] [--source <dir|url>]
               [--yes] [--dry-run]
  ```

  Codes de sortie : `0` succès ; `1` paramètre invalide ; `2` disque trop petit ;
  `3` échec de partitionnement ; `4` échec de mise en place du système ; `5` échec
  d'installation de l'amorçage.

- **Séquence :**

  1. choisir le disque cible et le système de fichiers ;
  2. **`systemd-repart`** crée le schéma GPT (aperçu avec `--dry-run`) ;
  3. création des systèmes de fichiers (`mkfs.ext4`/`mkfs.btrfs`, `mkfs.vfat`) ;
  4. écriture de l'image du système dans le **slot A** (`systemd-sysupdate`, source
     `--source`) ;
  5. installation de **Limine** (UEFI : `EFI/BOOT/BOOTX64.EFI` ; BIOS : `limine bios-install`) ;
  6. écriture de `limine.cfg`, des transferts `/etc/sysupdate.d/*.transfer` et de l'**état
     des slots** ;
  7. marquage du slot A **actif et confirmé** ; première entrée par défaut.

- **Invariant :** l'installation n'écrit **jamais** dans le disque avant l'accord de
  l'utilisateur (mode guidé) ou la présence de `--yes`.

## Mises à jour `systemd-sysupdate`

Définitions dans `/etc/sysupdate.d/` (forme visée) :

```ini
# /etc/sysupdate.d/10-root.transfer
[Transfer]
Verify=signature
Source=https://updates.yaor.example/

[Source]
Type=url-file
Path=@v
MatchPattern=yaor-root-x86-64-*.raw.xz

[Target]
Type=partition
MatchPartitionType=yaor-root
PartitionFlags=0
ReadOnly=1
Path=/dev/disk/by-partlabel/yaor-root-@v
```

- `Verify=signature` : l'image doit être **signée** (ADR-007) ; une vérification qui échoue
  **n'écrit aucun slot** ;
- le transfert **racine** cible une **partition** ; le transfert **noyau/initramfs** cible
  l'**ESP** (`Type=regular-file`) ;
- `systemd-sysupdate update` écrit dans le slot **inactif** puis le marque « à essayer ».

`systemd-sysupdate` et `systemd-repart` sont des **fonctions optionnelles** de systemd :
systemd est bâti `-Dauto_features=disabled`, il faut donc ajouter
**`-Drepart=true -Dsysupdate=true`** (la signature des images ramène OpenSSL/ADR-020,
cf. ADR-007).

## Sélection A/B et retour arrière (`yaor-slot`)

`systemd-sysupdate` ne connaît pas Limine : `yaor-slot` assure la **sélection** et le
**retour arrière**.

**État** (sur l'ESP, `/boot/yaor/slots.state`) :

```ini
active=A
trying=
attempts_left=0
confirmed=A
```

**Machine à états :**

| État | `trying` | `attempts_left` | Signification |
|---|---|---|---|
| `confirmed` | vide | 0 | le slot `active` est sain |
| `trying` | `A`/`B` | > 0 | un nouveau slot tente de démarrer |
| `reverting` | transitoire | 0 | retour au slot confirmé en cours |

**Séquence de mise à jour :**

1. après `systemd-sysupdate update`, `yaor-slot try <slot>` : `trying=<slot>`,
   `attempts_left=N`, le `default_entry` de `limine.cfg` pointe le nouveau slot ;
2. à chaque démarrage, `yaor-slot.service` (avant `sysinit`) décrémente `attempts_left` ;
3. si le système atteint `yaor-slot-confirm.service` (après `multi-user.target`, test de
   santé : cible `sysupdate` active, réseau, montage de `/var`), `yaor-slot ok` promeut le
   slot (`confirmed=<slot>`, `active=<slot>`, `trying=` vide) ;
4. si `attempts_left` atteint 0 sans confirmation, `yaor-slot revert` réécrit
   `default_entry` vers `confirmed` et redémarre.

**Invariants :**

- hors mise à jour, **exactement un** slot est `confirmed` ;
- `default_entry` de `limine.cfg` désigne toujours, au repos, le slot `confirmed` ;
- un retour arrière **ne touche pas** `/var`.

## Erreurs et défaillances

| Cas | Comportement |
|---|---|
| Disque plus petit que le schéma | refus **avant** toute écriture (code `2`) |
| Signature de mise à jour invalide | abandon, **aucun slot** modifié |
| Slot essayé non confirmé après `N` démarrages | retour automatique au slot `confirmed` |
| `limine.cfg` illisible | `yaor-slot` s'arrête en erreur ; Limine retombe sur son entrée par défaut |
| Échec du test de santé | le slot n'est **pas** promu (retour arrière au terme des essais) |

## Tests

| Cas | Vérifie |
|---|---|
| [`TEST-INST-001`](../../06-testing/test-cases.md) | démarrage installé sans média live, BIOS **et** UEFI (REQ-SW-019) |
| [`TEST-INST-002`](../../06-testing/test-cases.md) | persistance de `/var` à travers une mise à jour (REQ-SW-026) |
| [`TEST-UPD-001`](../../06-testing/test-cases.md) | mise à jour A/B et retour arrière automatique (REQ-SW-020) |
| [`TEST-SEC-001`](../../06-testing/test-cases.md) | rejet d'une source ou d'une image non conforme (REQ-SW-022) |
