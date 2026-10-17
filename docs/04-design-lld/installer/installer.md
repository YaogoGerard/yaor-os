# Installer & Updates — LLD

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-17
**Décisions liées :** ADR-002, ADR-003, ADR-007, ADR-011, ADR-015, ADR-016, ADR-020, ADR-021

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

| # | Partition | Type GPT (identifiant) | FS | Taille | Rôle |
|---|---|---|---|---|---|
| 1 | ESP | `esp` (`c12a7328-f81f-11d2-ba4b-00a0c93ec93b`) | vfat | 512 Mio | Limine (UEFI), noyaux, initramfs, `limine.conf`, état des slots |
| 2 | BIOS boot | `21686148-6449-6e6f-744e-656564454649` | — | 1 Mio | seconde étape de Limine en BIOS sur GPT |
| 3 | Root A | `root-x86-64` (`4f68bce3-e8cd-4db1-96e7-fbcaf984b709`) | ext4 / btrfs | 2 Gio | racine, slot A |
| 4 | Root B | `root-x86-64` (`4f68bce3-e8cd-4db1-96e7-fbcaf984b709`) | ext4 / btrfs | 2 Gio | racine, slot B |
| 5 | Var | `var` (`4d21b016-b534-45c2-a9fb-5c16e091fd2d`) | ext4 / btrfs | reste | `/var` persistant (`/home` en dessous) |

Les identifiants (`esp`, `root-x86-64`, `var`) sont ceux de `systemd-repart`
(`Type=`) et de `systemd-sysupdate` (`MatchPartitionType=`) ; « BIOS boot », sans
identifiant symbolique, est donnée par son UUID littéral.

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
  2. **`systemd-repart --dry-run`** pour l'aperçu, puis **`systemd-repart`** crée le
     schéma GPT d'après `/etc/repart.d/*.conf` (idempotent) ;
  3. formatage de l'ESP (`mkfs.vfat`) et de `/var` (`mkfs.ext4`/`mkfs.btrfs`) ; les
     deux slots racine restent **vides** (étiquette `_empty`) ;
  4. écriture de l'image du système dans un slot **libre** via **`systemd-sysupdate`**
     (source `--source`) ;
  5. installation de **Limine** : `EFI/BOOT/BOOTX64.EFI` et `limine.conf` sur l'ESP,
     `limine bios-install` sur la partition « BIOS boot » ;
  6. écriture de l'**état des slots** (`/boot/yaor/slots.state`) ; les transferts
     `/etc/sysupdate.d/*.transfer`, déjà présents dans le système, sont utilisés tels
     quels ;
  7. marquage du slot écrit comme **actif et confirmé** ; son entrée devient celle par
     défaut de `limine.conf`.

- **Limine :** c'est un **outil de build de l'hôte** (ADR-010, ADR-011) : le média live
  embarque `/usr/bin/limine` et `/usr/share/limine/` pour que `yaor-install` écrive le
  chargeur sur le disque cible.

- **Invariant :** l'installation n'écrit **jamais** dans le disque avant l'accord de
  l'utilisateur (mode guidé) ou la présence de `--yes`.

## Mises à jour `systemd-sysupdate`

Définitions dans `/etc/sysupdate.d/` (fournies par le système, voir
`config/rootfs/etc/sysupdate.d/10-root.transfer`) :

```ini
[Transfer]
Verify=signature
ProtectVersion=%A

[Source]
Type=url-file
Path=https://updates.yaor.example/root/
MatchPattern=yaor-root_x86-64_@v.raw.xz

[Target]
Type=partition
Path=auto
MatchPartitionType=root-x86-64
MatchPattern=yaor-root_@v
PartitionNoAuto=1
ReadOnly=1
InstancesMax=2
```

- la cible **racine** est une **partition** : `Path=auto` désigne le disque qui porte
  la racine en cours ; `MatchPartitionType=root-x86-64` sélectionne les slots A/B ;
  `MatchPattern=yaor-root_@v` apparie leur **étiquette**, qui porte la version `@v` ;
  `InstancesMax=2` borne à deux versions simultanées ;
- `systemd-sysupdate` ne **crée ni ne supprime** de partition : il écrit dans un slot
  **libre** — étiquette `_empty`, posée par `systemd-repart` — puis le réétiquette
  selon la version ; le slot rempli n'est donc pas repéré par une lettre fixe mais par
  sa **version**, et le suivi « actif / à essayer » incombe à `yaor-slot` (voir plus
  bas) ;
- `ProtectVersion=%A` empêche d'écraser la version en cours (celle qui tourne) : `%A`
  lit `IMAGE_VERSION=` d'`/etc/os-release` (voir « points ouverts ») ;
- `Verify=signature` : l'image et son manifeste doivent être **signés** (ADR-007) ; une
  vérification qui échoue **n'écrit aucun slot**.

`systemd-sysupdate` et `systemd-repart` sont des **fonctions optionnelles** de systemd.
systemd étant bâti `-Dauto_features=disabled`, elles exigent d'être activées :
**`-Dimportd=enabled -Dsysupdate=enabled -Drepart=enabled`**, avec leurs dépendances
`-Dlibcurl=enabled -Dopenssl=enabled -Dzlib=enabled -Dxz=enabled -Dfdisk=enabled`
(openssl et curl sont entrés au socle v0.2 pour cela, ADR-021 ; voir
`deployment-ops.md`).

## Sélection A/B et retour arrière (`yaor-slot`)

`systemd-sysupdate` ne connaît pas Limine : `yaor-slot` assure la **sélection** et le
**retour arrière**.

**Slots et versions :** les deux slots sont **physiquement** les deux partitions
`root-x86-64` du schéma GPT ; ils sont repérés par leur **ordre sur le disque**
(A = première, B = seconde). Comme leur étiquette porte la **version** (posée par
`systemd-sysupdate`, voir plus haut), `yaor-slot` enregistre la version hébergée par
chaque slot, afin de savoir lequel démarre et à quelle image revenir.

**État** (sur l'ESP, `/boot/yaor/slots.state`) :

```ini
active=A
trying=
attempts_left=0
confirmed=A
version_a=2026.10.17
version_b=2026.10.12
```

**Machine à états :**

| État | `trying` | `attempts_left` | Signification |
|---|---|---|---|
| `confirmed` | vide | 0 | le slot `active` est sain |
| `trying` | `A`/`B` | > 0 | un nouveau slot tente de démarrer |
| `reverting` | transitoire | 0 | retour au slot confirmé en cours |

**Séquence de mise à jour :**

1. `yaor-slot update` ordonne `systemd-sysupdate update`, puis identifie le slot écrit
   (par différence d'étiquettes de partition : celle qui n'est plus `_empty` et porte une
   nouvelle version) et enregistre `version_<slot>` ; il pose `trying=<slot>`,
   `attempts_left=N` et fait pointer le `default_entry` de `limine.conf` sur le nouveau
   slot ;
2. à chaque démarrage, `yaor-slot.service` (avant `sysinit`) décrémente `attempts_left` ;
3. si le système atteint `yaor-slot-confirm.service` (après `multi-user.target`, test de
   santé : cible `sysupdate` active, réseau, montage de `/var`), `yaor-slot ok` promeut le
   slot (`confirmed=<slot>`, `active=<slot>`, `trying=` vide) ;
4. si `attempts_left` atteint 0 sans confirmation, `yaor-slot revert` réécrit
   `default_entry` vers `confirmed` et redémarre.

**Invariants :**

- hors mise à jour, **exactement un** slot est `confirmed` ;
- `default_entry` de `limine.conf` désigne toujours, au repos, le slot `confirmed` ;
- un retour arrière **ne touche pas** `/var`.

## Points ouverts

Ces points ne sont **pas** encore tranchés et devront l'être pour clore le mode installé :

- **démarrage installé sans initramfs ?** En QEMU, `ext4` (`CONFIG_EXT4_FS=y`) et le
  disque (`CONFIG_VIRTIO_BLK=y`) sont intégrés : `root=PARTLABEL=yaor-root_<v>
  rootfstype=ext4` suffit donc sans initramfs. Sur matériel réel (NVMe, AHCI/SATA), il
  faudra soit intégrer ces pilotes, soit un **initramfs** dédié — l'initramfs de
  l'étape 05 est **live** (EROFS + overlay) et ne convient pas tel quel ;
- **version d'image produit** : `ProtectVersion=%A` et l'appariement des versions
  reposent sur `IMAGE_VERSION=` dans `/etc/os-release`, aujourd'hui absent (`VERSION_ID`
  porte la version du noyau). Un schéma de version produit (et sa correspondance avec
  `@v`) reste à définir ;
- **transfert noyau/initramfs** (cible **ESP**, `Type=regular-file`,
  `PathRelativeTo=boot`) : à écrire, pour que les mises à jour remplacent aussi le noyau
  sans réécrire la racine ;
- **Limine embarqué** : le média live doit contenir `/usr/bin/limine` et
  `/usr/share/limine/` (outil d'hôte, ADR-010/ADR-011) ; la copie dans l'image reste à
  faire à l'étape 04/06 ;
- **publication des images** : dépôt (URL), génération de `SHA256SUMS` et signature
  (`SHA256SUMS.gpg`, ADR-007).

## Erreurs et défaillances

| Cas | Comportement |
|---|---|
| Disque plus petit que le schéma | refus **avant** toute écriture (code `2`) |
| Signature de mise à jour invalide | abandon, **aucun slot** modifié |
| Slot essayé non confirmé après `N` démarrages | retour automatique au slot `confirmed` |
| `limine.conf` illisible | `yaor-slot` s'arrête en erreur ; Limine retombe sur son entrée par défaut |
| Échec du test de santé | le slot n'est **pas** promu (retour arrière au terme des essais) |

## Tests

| Cas | Vérifie |
|---|---|
| [`TEST-INST-001`](../../06-testing/test-cases.md) | démarrage installé sans média live, BIOS **et** UEFI (REQ-SW-019) |
| [`TEST-INST-002`](../../06-testing/test-cases.md) | persistance de `/var` à travers une mise à jour (REQ-SW-026) |
| [`TEST-UPD-001`](../../06-testing/test-cases.md) | mise à jour A/B et retour arrière automatique (REQ-SW-020) |
| [`TEST-SEC-001`](../../06-testing/test-cases.md) | rejet d'une source ou d'une image non conforme (REQ-SW-022) |
