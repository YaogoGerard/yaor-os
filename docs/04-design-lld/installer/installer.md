# Installer & Updates — LLD

**Statut :** Accepté
**Version :** 0.2
**Dernière mise à jour :** 2026-10-17
**Décisions liées :** ADR-002, ADR-003, ADR-007, ADR-011, ADR-015, ADR-016, ADR-020, ADR-021, ADR-022, ADR-023

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
- le **noyau vit sur l'ESP**, **versionné** (`yaor/vmlinuz-<version>`) : chaque version de
  racine a son noyau, écrit à part, ce qui permet de remplacer l'un sans réécrire l'autre
  (ADR-022, voir « transfert noyau » plus bas) ;
- `/var` n'est **jamais** dans un slot : il **survit** aux mises à jour et aux retours
  arrière ;
- `ext4` est le système de fichiers **par défaut** ; `btrfs` est une alternative (ADR-015).

## Installateur `yaor-install`

- **Forme :** binaire en ligne de commande. Il exige ses paramètres (automatisation et
  tests) ; un mode **guidé** (invites sur la console) reste un suivi possible. Aucune
  bibliothèque d'interface graphique n'est introduite.
- **Interface :**

  ```text
  yaor-install --disk <périphérique> --version <version> [--source <dir|url>]
               [--yes] [--dry-run]
  ```

  `--version` (obligatoire) est la version de l'image à installer ; elle devient
  l'étiquette du slot et l'identifiant de l'état des slots. Le système de fichiers est
  fixé par `/etc/repart.d/*.conf` (`ext4`), ce qui évite une option et un second
  fournisseur. Codes de sortie : `0` succès ; `1` échec (paramètre, disque ou étape).

- **Séquence :**

  1. contrôles préalables : privilèges root, disque cible présent et **distinct** du
     disque qui porte le système en cours (`findmnt -no SOURCE /`) ;
  2. **`systemd-repart --dry-run=yes --empty=allow`** pour l'aperçu, puis
     **`systemd-repart --dry-run=no --empty=allow`** crée le schéma GPT d'après
     `/etc/repart.d/*.conf` (idempotent) et formate l'ESP (`vfat`) et `/var` (`ext4`) ;
     les deux slots racine restent **vides** (étiquette `_empty`, sans système de
     fichiers) ;
  3. écriture de l'image racine dans un slot **libre** via **`systemd-sysupdate
     --image=<disque> update`** : `--image=` fait opérer l'outil **à l'intérieur** du
     disque cible. La version effectivement écrite est relue sur les étiquettes de
     partition ;
  4. montage de l'ESP, de la racine et de `/var` du disque cible ; copie du **noyau**
     sous `yaor/vmlinuz-<version>` sur l'ESP ; **peuplement de `/var`** depuis l'image ;
     écriture d'**`/etc/fstab`** ;
  5. installation de **Limine** : `EFI/BOOT/BOOTX64.EFI` et `limine.conf` sur l'ESP,
     `limine bios-install` sur la partition « BIOS boot » ;
  6. écriture de l'**état des slots** (`/boot/yaor/slots.state`) : le slot écrit est
     **actif et confirmé** ; son entrée est celle par défaut de `limine.conf` ;
  7. démontage et `sync`.

- **Limine :** c'est un **outil de build de l'hôte** (ADR-010, ADR-011), **embarqué dans le
  média live** (ADR-023) : `06_make_iso.sh` copie `/usr/bin/limine` et
  `/usr/share/limine/` depuis l'hôte, ce qui permet à `yaor-install` d'écrire le chargeur
  sur le disque cible.

- **`/etc/fstab` et `/var` :** `systemd-gpt-auto-generator` ne monte **pas** une
  partition dont le point de montage contient déjà des fichiers — c'est le cas de
  `/boot` (noyau) et de `/var`. `yaor-install` écrit donc un `fstab` explicite
  (`PARTLABEL=yaor-esp`, `PARTLABEL=yaor-var`) et **peuple `/var`** avec le contenu
  initial de l'image : sans cela, `/var` serait vide et la racine ne monterait pas sa
  partition de données (REQ-SW-026).

- **Invariant :** l'installation n'écrit **jamais** dans le disque avant la présence de
  `--yes` (hors `--dry-run`).

## Mises à jour `systemd-sysupdate`

Définitions dans `/etc/sysupdate.d/` (fournies par le système, voir
`config/rootfs/etc/sysupdate.d/`) : `10-root.transfer` pour l'image racine et
`20-boot.transfer` pour le noyau.

**Racine** (`10-root.transfer`) :

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

**Noyau** (`20-boot.transfer`) : écrit le noyau d'une version sur l'ESP, à côté de
l'état des slots.

```ini
[Transfer]
Verify=signature
ProtectVersion=%A

[Source]
Type=url-file
Path=https://updates.yaor.example/boot/
MatchPattern=vmlinuz-@v

[Target]
Type=regular-file
Path=/yaor
PathRelativeTo=boot
MatchPattern=vmlinuz-@v
Mode=0644
InstancesMax=2
```

- la cible **racine** est une **partition** : `Path=auto` désigne le disque qui porte
  la racine en cours ; `MatchPartitionType=root-x86-64` sélectionne les slots A/B ;
  `MatchPattern=yaor-root_@v` apparie leur **étiquette**, qui porte la version `@v` ;
  `InstancesMax=2` borne à deux versions simultanées ;
- la cible **noyau** est un **fichier de l'ESP** (`Type=regular-file`,
  `PathRelativeTo=boot` = `$BOOT`, l'ESP) : le noyau est écrit sous `yaor/vmlinuz-<v>`,
  avec la **même** version `@v` que la racine. `20-` est traité **après** `10-` : le point
  d'entrée (la racine que Limine démarre) est écrit avant le noyau qui l'accompagne ;
- `systemd-sysupdate` ne **crée ni ne supprime** de partition : il écrit dans un slot
  **libre** — étiquette `_empty`, posée par `systemd-repart` — puis le réétiquette
  selon la version ; le slot rempli n'est donc pas repéré par une lettre fixe mais par
  sa **version**, et le suivi « actif / à essayer » incombe à `yaor-slot` (voir plus
  bas) ;
- `ProtectVersion=%A` empêche d'écraser la version en cours (celle qui tourne) : `%A`
  lit `IMAGE_VERSION=` d'`/etc/os-release` (ADR-022) ;
- `Verify=signature` : l'image, le noyau et leurs manifestes doivent être **signés**
  (ADR-007) ; une vérification qui échoue **n'écrit aucun slot**.

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
   `attempts_left=N` et repointe l'entrée `limine.conf` du nouveau slot : `path:` vers le
   noyau de sa version (`boot():/yaor/vmlinuz-<v>`) et `cmdline: root=` vers sa partition
   (`yaor-root_<v>`), puis en fait l'entrée par défaut ;
2. à chaque démarrage, `yaor-slot.service` (avant `sysinit`) décrémente `attempts_left` ;
3. si le système atteint `yaor-slot-confirm.service` (après `multi-user.target`, test de
   santé : cible `sysupdate` active, réseau, montage de `/var`), `yaor-slot ok` promeut le
   slot (`confirmed=<slot>`, `active=<slot>`, `trying=` vide) ;
4. si `attempts_left` atteint 0 sans confirmation, `yaor-slot revert` réécrit
   `default_entry` vers `confirmed` et redémarre.

**Invariants :**

- hors mise à jour, **exactement un** slot est `confirmed` ;
- l'entrée Limine d'un slot pointe **toujours** le noyau et la partition de la version
  enregistrée pour ce slot ; au repos, `default_entry` désigne le slot `confirmed` ;
- un retour arrière **ne touche pas** `/var`.

**Implémentation.** `yaor-install` et `yaor-slot` sont des binaires Rust **sans
dépendance externe** (bibliothèque standard seule : pas de vendoring, ADR-018),
compilés par la toolchain de l'hôte (ADR-019). Leurs sources vivent dans
`components/` du dépôt (`components/yaor-common` pour l'état des slots, `limine.conf`
et la découverte des partitions ; `components/yaor-install` et
`components/yaor-slot` pour les binaires). La phase `yaor-components` de
`scripts/04_chroot_build.sh` les compile (`cargo build --release --locked`, cible et
`CARGO_HOME` sous `/build/work`) et les installe dans `/usr/bin`.

Sous-commandes de `yaor-slot` : `status` (état courant), `update` (mise à jour puis
essai du nouveau slot), `try <A|B>` (essai d'un slot), `startup` (décrément au
démarrage, `yaor-slot.service`), `ok` (confirmation, `yaor-slot-confirm.service`),
`revert` (retour au slot confirmé puis redémarrage).

## Publication des images

L'étape **07** (`scripts/07_publish.sh`, hors pipeline par défaut 00→06) assemble le jeu
d'artefacts servi par le dépôt de mises à jour (ADR-007) :

| Artefact | URL (transfert) |
|---|---|
| `out/update/root/yaor-root_x86-64_<v>.raw.xz` | `https://updates.yaor.example/root/` (`10-root`) |
| `out/update/boot/vmlinuz-<v>` | `https://updates.yaor.example/boot/` (`20-boot`) |
| `SHA256SUMS`, `SHA256SUMS.gpg` (par répertoire) | manifeste et signature |

- l'**image racine** est un système de fichiers ext4 peuplé depuis le rootfs livré
  (`mkfs.ext4 -d`, mêmes exclusions que l'image live, `/boot` conservé), dimensionné pour
  tenir exactement dans un slot de 2 Gio ;
- l'image installable porte un **`/etc/fstab`** (ESP + `/var`) : une racine écrite par une
  mise à jour doit déjà monter ces partitions (REQ-SW-026), au même titre qu'après
  l'installation ;
- `<v>` est la **version produit** (`VERSION`, ADR-022), qui **apparie** l'image racine et
  son noyau ; une publication doit **incrémenter** cette version (sinon `systemd-sysupdate`
  considère l'image comme déjà installée) ;
- les manifestes sont produits en `sha256sum --binary` et **signés** lorsque
  `YAOR_GPG_KEY` est défini.

## Points ouverts

Ces points ne sont **pas** encore tranchés et devront l'être pour clore le mode installé :

- **démarrage installé sur matériel réel** : en QEMU, `ext4` (`CONFIG_EXT4_FS=y`) et le
  disque (`CONFIG_VIRTIO_BLK=y`) sont intégrés, donc `root=PARTLABEL=yaor-root_<v>
  rootfstype=ext4` suffit **sans initramfs**. Sur matériel réel (NVMe, AHCI/SATA), il faut
  un **initramfs dédié** — l'initramfs de l'étape 05 est **live** (EROFS + overlay) et ne
  convient pas tel quel ; l'installateur ne le copie donc **pas** sur l'ESP aujourd'hui.
  Le transfert du noyau (ESP) **n'embarque pas** non plus d'initramfs pour cette raison :
  il faudra l'étendre (`initramfs-<v>`) le jour où l'initramfs installé existera ;
- **installation hors-ligne** : le transfert `10-root` lit une **URL** ; installer sans
  réseau suppose de prévoir une source locale (`Type=regular-file`) — non retenu pour
  l'instant, la distribution live restant légère (≤ ~150 Mio) ;
- **politique de clés** (ADR-007) : la clé publique de vérification des mises à jour doit
  être provisionnée dans le système.

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
