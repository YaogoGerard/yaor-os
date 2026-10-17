# Configuration système (rootfs)

Cette arborescence **reflète la racine du système** : chaque fichier placé sous
`config/rootfs/` est installé au même chemin dans l'image. Par exemple,
`config/rootfs/etc/iwd/main.conf` devient `/etc/iwd/main.conf`.

Elle est copiée par la **phase de configuration système** de
`scripts/04_chroot_build.sh` — `cp -a config/rootfs/. "$LFS/"` — **après**
l'installation des paquets et **avant** la construction de l'image, afin que la
configuration fournie par YAOR prime sur les fichiers par défaut des paquets.

## Contenu

| Fichier | Rôle | Référence |
|---|---|---|
| `etc/systemd/network/20-wired.network` | DHCP sur le filaire (`systemd-networkd`) | ADR-013 |
| `etc/systemd/network/25-wireless.network` | DHCP sur le Wi-Fi (`systemd-networkd`) | ADR-013 |
| `etc/systemd/resolved.conf.d/yaor.conf` | pas de DNS de repli ni de DNSSEC | ADR-013 |
| `etc/iwd/main.conf` | `iwd` limité à la couche 2 | ADR-013 |
| `etc/pam.d/*` | chaînes PAM, dont `pam_systemd` | ADR-014 |
| `etc/sudoers`, `etc/sudoers.d/00-wheel` | administration par le groupe `wheel` | ADR-012, ADR-014 |
| `etc/polkit-1/rules.d/49-yaor-udisks.rules` | actions `udisks2` pour `wheel` | ADR-014 |
| `etc/repart.d/*.conf` | schéma GPT (ESP, BIOS boot, slots A/B, `/var`) | ADR-015, REQ-SW-026 |
| `etc/sysupdate.d/*.transfer` | mises à jour par image, slots A/B | ADR-015, REQ-SW-020 |
| `etc/systemd/system/yaor-slot.service` | essai du slot / retour arrière au démarrage | ADR-015 |
| `etc/systemd/system/yaor-slot-confirm.service` | confirmation du slot démarré | ADR-015 |

## Compléments de la phase de configuration

La phase de configuration système assure aussi ce qui n'est pas un simple
fichier :

- **activer les services** : `systemctl enable systemd-networkd
  systemd-resolved iwd yaor-slot.service yaor-slot-confirm.service` ;
  `dbus.socket` et `systemd-logind.service` n'ont pas de section `[Install]` :
  leurs paquets les activent par des liens symboliques posés à l'installation ;
- faire pointer **`/etc/resolv.conf`** vers
  `/run/systemd/resolve/stub-resolv.conf` ;
- créer le groupe **`wheel`** et le dossier **`/var/lib/iwd`** (profils Wi-Fi).

## Règles PAM

`pam_systemd.so` (fourni par systemd) est inclus dans `system-session` : sans
lui, `logind` ne suit pas les sessions, ce que `REQ-SW-021` impose. Le
changement de mot de passe utilise **yescrypt**, l'algorithme par défaut de
`libxcrypt` (présent au lot).
