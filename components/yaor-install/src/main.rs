//! `yaor-install` — installe YAOR-OS sur un disque (mode installé, ADR-015).
//!
//! Enchaîne, sur un disque cible :
//!
//! 1. création idempotente du schéma GPT par `systemd-repart` (ESP, « BIOS
//!    boot », deux slots racine A/B, `/var`) ;
//! 2. écriture de l'image racine dans un slot par `systemd-sysupdate`
//!    (`--image=<disque>`) ;
//! 3. copie du noyau et écriture de `limine.conf` sur l'ESP ;
//! 4. installation de Limine (UEFI et BIOS) ;
//! 5. écriture de l'état des slots (`yaor-slot`).
//!
//! Aucune dépendance externe (bibliothèque standard seule).

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use yaor_common::state::SlotState;
use yaor_common::{limine_kernel_path, Slot, ESP_YAOR_DIR, ROOT_LABEL_PREFIX};

/// Outil Limine (outil de build de l'hôte embarqué dans le média, ADR-010).
const LIMINE: &str = "/usr/bin/limine";
/// Fichiers Limine à copier sur l'ESP.
const LIMINE_SHARE: &str = "/usr/share/limine";
/// Point de montage temporaire de l'ESP pendant l'installation.
const ESP_TMP: &str = "/run/yaor-install/esp";
/// Point de montage temporaire de la racine installée (lecture/écriture).
const ROOT_TMP: &str = "/run/yaor-install/root";
/// Point de montage temporaire de la partition /var pendant l'installation.
const VAR_TMP: &str = "/run/yaor-install/var";
/// Étiquette de l'ESP (voir 10-esp.conf).
const ESP_LABEL: &str = "yaor-esp";
/// Étiquette de la partition de données (voir 50-var.conf).
const VAR_LABEL: &str = "yaor-var";

const USAGE: &str = "\
yaor-install — installe YAOR-OS sur un disque (mode installé)

USAGE :
    yaor-install --disk <DEV> --version <VERSION> [options]

OPTIONS :
    --disk <DEV>            disque cible (obligatoire), ex. /dev/sdb
    --version <VERSION>     version de l'image à installer (obligatoire)
    --source <CHEMIN|URL>   source des transferts (systemd-sysupdate)
    --yes                   ne pas demander de confirmation
    --dry-run               n'écrit rien ; affiche le plan
    -h, --help              affiche cette aide

ATTENTION : l'installation EFFACE le contenu du disque cible.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("yaor-install : erreur : {e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    disk: String,
    version: String,
    source: Option<String>,
    yes: bool,
    dry_run: bool,
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut disk = None;
    let mut version = None;
    let mut source = None;
    let mut yes = false;
    let mut dry_run = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--disk" => disk = Some(take(&args, &mut i, "--disk")?),
            "--version" => version = Some(take(&args, &mut i, "--version")?),
            "--source" => source = Some(take(&args, &mut i, "--source")?),
            "--yes" | "-y" => {
                yes = true;
                i += 1;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            a => return Err(format!("option inconnue : « {a} » (voir --help)")),
        }
    }

    if args.is_empty() {
        print!("{USAGE}");
        return Ok(());
    }

    let opts = Opts {
        disk: disk.ok_or("« --disk » est obligatoire")?,
        version: version.ok_or("« --version » est obligatoire")?,
        source,
        yes,
        dry_run,
    };
    install(&opts)
}

fn take(args: &[String], i: &mut usize, name: &str) -> Result<String, String> {
    if *i + 1 >= args.len() {
        return Err(format!("« {name} » attend une valeur"));
    }
    let v = args[*i + 1].clone();
    *i += 2;
    Ok(v)
}

// --- Exécution de commandes -------------------------------------------------

fn exec(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("lancement de {program} impossible : {e}"))?;
    if !status.success() {
        return Err(format!("{program} a échoué ({status})"));
    }
    Ok(())
}

/// Exécute une commande et renvoie sa sortie standard (texte).
fn capture(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("lancement de {program} impossible : {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} a échoué ({}) : {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn est_root() -> bool {
    capture("id", &["-u"]).map(|s| s.trim() == "0").unwrap_or(false)
}

// --- Découverte du disque ---------------------------------------------------

/// Interroge `lsblk` et renvoie les couples (chemin de partition, étiquette)
/// d'un disque.
fn partitions_du_disque(disk: &str) -> Result<Vec<(String, String)>, String> {
    let out = capture("lsblk", &["-Pno", "PATH,PARTLABEL", disk])?;
    let mut v = Vec::new();
    for line in out.lines() {
        let path = champ(line, "PATH").unwrap_or_default();
        let label = champ(line, "PARTLABEL").unwrap_or_default();
        if !path.is_empty() && !label.is_empty() {
            v.push((path, label));
        }
    }
    Ok(v)
}

/// Extrait `clé="valeur"` d'une ligne `lsblk -P`.
fn champ(line: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Partition du disque portant une étiquette donnée.
fn partition_par_etiquette(
    disk: &str,
    label: &str,
) -> Result<String, String> {
    let parts = partitions_du_disque(disk)?;
    parts
        .iter()
        .find(|(_, l)| l == label)
        .map(|(p, _)| p.clone())
        .ok_or_else(|| format!("partition « {label} » introuvable sur {disk}"))
}

/// Version des slots racine écrits sur le disque (doit être unique).
fn version_installee(disk: &str) -> Result<String, String> {
    let parts = partitions_du_disque(disk)?;
    let versions: Vec<String> = parts
        .iter()
        .filter_map(|(_, l)| l.strip_prefix(ROOT_LABEL_PREFIX).map(str::to_string))
        .filter(|v| !v.is_empty())
        .collect();
    match versions.as_slice() {
        [v] => Ok(v.clone()),
        _ => Err(format!(
            "attendu exactement un slot racine installé, trouvé : {versions:?}"
        )),
    }
}

// --- Étapes de l'installation ----------------------------------------------

fn install(opts: &Opts) -> Result<(), String> {
    if !est_root() {
        return Err("l'installation exige les privilèges root".into());
    }
    let disk = Path::new(&opts.disk);
    if !disk.is_file() && !disk.exists() {
        return Err(format!("disque introuvable : {}", opts.disk));
    }

    verifier_disque_cible(&opts.disk)?;

    eprintln!("== Plan de partitionnement (systemd-repart --dry-run) ==");
    exec(
        "systemd-repart",
        &["--dry-run=yes", "--empty=allow", "--pretty=yes", &opts.disk],
    )?;

    if !opts.yes && !opts.dry_run {
        eprintln!(
            "\nATTENTION : cette opération EFFACE le contenu de {}.\n\
             Relancez avec --yes pour confirmer.",
            opts.disk
        );
        return Err("confirmation requise (--yes)".into());
    }

    if opts.dry_run {
        eprintln!("[dry-run] aucune écriture effectuée");
        return Ok(());
    }

    // 1. Schéma GPT (repart écrit avec --dry-run=no ; systemd 262 fait
    //    --dry-run=yes par défaut).
    section("Création du schéma GPT");
    exec(
        "systemd-repart",
        &["--dry-run=no", "--empty=allow", "--pretty=yes", &opts.disk],
    )?;

    // 2. Image racine dans un slot.
    section("Écriture de l'image racine (systemd-sysupdate)");
    let image_arg = format!("--image={}", opts.disk);
    let mut args = vec!["update", image_arg.as_str()];
    let src;
    if let Some(s) = &opts.source {
        src = format!("--transfer-source={s}");
        args.push(&src);
    }
    exec("systemd-sysupdate", &args)?;

    // 3. Retrouver le slot écrit.
    let version = version_installee(&opts.disk)?;
    if version != opts.version {
        eprintln!(
            "note : version installée « {version} » différente de --version « {} »",
            opts.version
        );
    }

    // 4. ESP et /var : noyau, fstab, limine.conf, état.
    section("Préparation de l'ESP et de /var");
    let esp_dev = partition_par_etiquette(&opts.disk, ESP_LABEL)?;
    let var_dev = partition_par_etiquette(&opts.disk, VAR_LABEL)?;
    let root_dev = partition_par_etiquette(&opts.disk, &format!("{ROOT_LABEL_PREFIX}{version}"))?;
    let esp = PathBuf::from(ESP_TMP);
    let root = PathBuf::from(ROOT_TMP);
    let var = PathBuf::from(VAR_TMP);
    creer_dossier(&esp)?;
    creer_dossier(&root)?;
    creer_dossier(&var)?;

    exec("mount", &[&esp_dev, ESP_TMP])?;
    let resultat = (|| -> Result<(), String> {
        exec("mount", &[&root_dev, ROOT_TMP])?;
        exec("mount", &[&var_dev, VAR_TMP])?;
        copier_noyau(&root, &esp, &version)?;
        populer_var(&root, &var)?;
        ecrire_fstab(&root)?;
        let _ = exec("umount", &[VAR_TMP]);
        let _ = exec("umount", &[ROOT_TMP]);
        ecrire_limine_conf(&esp, &version)?;
        installer_limine(&opts.disk, &esp)?;
        ecrire_etat(&esp, &version)?;
        Ok(())
    })();
    let _ = exec("sync", &[]);
    let _ = exec("umount", &[VAR_TMP]);
    let _ = exec("umount", &[ROOT_TMP]);
    let _ = exec("umount", &[ESP_TMP]);
    resultat?;

    section("Terminé");
    eprintln!(
        "YAOR-OS {version} installé sur {}. Retirez le média et redémarrez.",
        opts.disk
    );
    Ok(())
}

fn section(titre: &str) {
    eprintln!("\n== {titre} ==");
}

fn creer_dossier(p: &Path) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(|e| format!("création de {} impossible : {e}", p.display()))
}

/// Refuse d'installer sur le disque qui porte le système en cours.
fn verifier_disque_cible(disk: &str) -> Result<(), String> {
    let out = capture("findmnt", &["-no", "SOURCE", "/"])?;
    let src = out.trim();
    if src.starts_with(disk) {
        return Err(format!(
            "refus d'installer sur {disk} : il porte le système en cours ({src})"
        ));
    }
    Ok(())
}

/// Copie le noyau de l'image installée vers l'ESP sous `yaor/vmlinuz-<version>`.
/// Le noyau porte la version du slot ; `yaor-slot` repointe l'entrée Limine
/// correspondante à chaque changement de slot (voir le LLD installateur).
fn copier_noyau(root: &Path, esp: &Path, version: &str) -> Result<(), String> {
    let boot = root.join("boot");
    let mut noyau: Option<PathBuf> = None;
    let entries =
        std::fs::read_dir(&boot).map_err(|e| format!("lecture de {} : {e}", boot.display()))?;
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("vmlinuz") {
            noyau = Some(e.path());
            break;
        }
    }
    let noyau = noyau.ok_or_else(|| format!("aucun noyau dans {}", boot.display()))?;
    let dir = esp.join(ESP_YAOR_DIR);
    creer_dossier(&dir)?;
    let dest = dir.join(format!("vmlinuz-{version}"));
    std::fs::copy(&noyau, &dest)
        .map_err(|e| format!("copie du noyau vers {} : {e}", dest.display()))?;
    eprintln!("noyau installé : {}", dest.display());
    // L'initramfs live (EROFS+overlay) n'est PAS copié : en mode installé, la
    // racine ext4 est montée directement par le noyau (CONFIG_EXT4_FS=y).
    Ok(())
}

/// Peuple la partition /var avec le contenu initial de `/var` de l'image, au
/// premier remplissage (REQ-SW-026). Au démarrage, la partition monte par-dessus
/// le `/var` de la racine ; elle doit donc déjà contenir les répertoires dont le
/// système a besoin (/var/lib, /var/log, …).
fn populer_var(root: &Path, var: &Path) -> Result<(), String> {
    // Ne peuple qu'une partition vierge (réinstallation : on préserve /var).
    if var.join("lib").exists() {
        eprintln!("/var déjà peuplé : conservé");
        return Ok(());
    }
    let src = root.join("var");
    exec(
        "cp",
        &["-a", &format!("{}/.", src.display()), &format!("{}/", var.display())],
    )?;
    eprintln!("/var initialisé depuis l'image");
    Ok(())
}

/// Écrit `/etc/fstab` dans la racine installée.
///
/// `systemd-gpt-auto-generator` ne monte PAS les partitions dont le point de
/// montage contient déjà des fichiers — c'est le cas de `/boot` (noyau) et de
/// `/var`. Un `fstab` explicite est donc nécessaire pour que l'ESP et `/var`
/// persistent (REQ-SW-026).
fn ecrire_fstab(root: &Path) -> Result<(), String> {
    let path = root.join("etc/fstab");
    let contenu = "\
# YAOR — partitions retrouvées par étiquette de partition (schéma GPT créé par
# systemd-repart). /boot (ESP) et /var (données persistantes) survivent aux mises
# à jour et aux retours arrière, qui ne touchent que les slots racine (REQ-SW-026).
PARTLABEL=yaor-esp /boot vfat umask=0077,shortname=winnt 0 2
PARTLABEL=yaor-var /var  ext4 defaults                    0 2
";
    if let Ok(existant) = std::fs::read_to_string(&path) {
        if existant.contains("yaor-esp") {
            eprintln!("fstab déjà configuré : conservé");
            return Ok(());
        }
    }
    std::fs::write(&path, contenu).map_err(|e| format!("écriture de {} : {e}", path.display()))?;
    eprintln!("fstab écrit : {}", path.display());
    Ok(())
}

/// Écrit `limine.conf` sur l'ESP : deux entrées (slot A puis slot B). Le slot B
/// n'a pas encore d'image ; son entrée reprend le noyau et la racine du slot A
/// (repli) et sera repointée par `yaor-slot` à la première mise à jour.
fn ecrire_limine_conf(esp: &Path, version: &str) -> Result<(), String> {
    let label = format!("{ROOT_LABEL_PREFIX}{version}");
    let cmdline = format!("root=PARTLABEL={label} ro rootfstype=ext4");
    let noyau = limine_kernel_path(version);
    let contenu = format!(
        "timeout: 3\ndefault_entry: 1\n\n\
         /YAOR-OS (slot A)\n    protocol: linux\n    path: {noyau}\n    cmdline: {cmdline}\n\n\
         /YAOR-OS (slot B)\n    protocol: linux\n    path: {noyau}\n    cmdline: {cmdline}\n"
    );
    let path = esp.join("limine.conf");
    std::fs::write(&path, contenu)
        .map_err(|e| format!("écriture de {} : {e}", path.display()))?;
    eprintln!("configuration Limine écrite : {}", path.display());
    Ok(())
}

/// Installe Limine : UEFI (copie de BOOTX64.EFI) et BIOS (`limine bios-install`).
fn installer_limine(disk: &str, esp: &Path) -> Result<(), String> {
    let efi_dir = esp.join("EFI/BOOT");
    creer_dossier(&efi_dir)?;
    let src = Path::new(LIMINE_SHARE).join("BOOTX64.EFI");
    let dst = efi_dir.join("BOOTX64.EFI");
    std::fs::copy(&src, &dst)
        .map_err(|e| format!("copie de {} vers {} : {e}", src.display(), dst.display()))?;
    eprintln!("démarrage UEFI installé : {}", dst.display());
    exec(LIMINE, &["bios-install", disk])?;
    eprintln!("démarrage BIOS installé sur {disk}");
    Ok(())
}

/// Écrit l'état des slots : slot A actif et confirmé, version installée.
fn ecrire_etat(esp: &Path, version: &str) -> Result<(), String> {
    let mut state = SlotState::new(Slot::A);
    state.set_version(Slot::A, version);
    state
        .save(esp)
        .map_err(|e| format!("écriture de l'état des slots : {e}"))?;
    eprintln!("état des slots écrit : slot A = {version}");
    Ok(())
}
