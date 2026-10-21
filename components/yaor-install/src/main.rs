//! `yaor-install` — installe YAOR-OS sur un disque (mode installé, ADR-015).
//!
//! Enchaîne, sur un disque cible :
//!
//! 1. création idempotente du schéma GPT par `systemd-repart` (ESP, « BIOS
//!    boot », deux slots racine A/B, `/var`) ; la définition du premier slot
//!    racine est réécrite dans `/run` avec l'étiquette `yaor-root_<version>` :
//!    `repart` la pose à la création de la partition, dispensant de retoucher
//!    la table GPT après coup ;
//! 2. écriture de l'image racine (téléchargée en flux depuis la source des
//!    transferts, décompressée et copiée dans le slot A) ;
//! 3. copie du noyau et écriture de `limine.conf` sur l'ESP ;
//! 4. installation de Limine (UEFI et BIOS) ;
//! 5. écriture de l'état des slots (`yaor-slot`).
//!
//! `systemd-sysupdate` n'est pas utilisé à l'installation : son mode
//! `--image=<disque>` attache le disque comme image OS et exige une racine
//! montable (`DISSECT_IMAGE_REQUIRE_ROOT`), alors que les slots vides portent
//! justement l'étiquette « _empty » et l'attribut « NoAuto » que le dissect
//! ignore — il échoue donc sur un disque neuf (« No suitable root partition
//! found in image »). Les mises à jour A/B, elles, restent gérées par
//! `systemd-sysupdate` depuis le système installé (pas de dissect : la racine
//! du système en cours sert de contexte, et `yaor-slot` pilote le slot libre).
//!
//! Aucune dépendance externe (bibliothèque standard seule) ; les outils
//! appelés (`systemd-repart`, `curl`, `xz`, `sha256sum`, Limine) font partie
//! du système live.
//!
//! Aucune dépendance externe (bibliothèque standard seule).

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use yaor_common::state::SlotState;
use yaor_common::{limine_kernel_path, Slot, ESP_YAOR_DIR, ROOT_LABEL_PREFIX};

/// Outil Limine (outil de build de l'hôte embarqué dans le média, ADR-010).
const LIMINE: &str = "/usr/bin/limine";
/// Définitions repart du système (schéma GPT).
const REPART_DISTRI_DIR: &str = "/etc/repart.d";
/// Définitions de transferts sysupdate (source et vérification de l'image).
const SYSUPDATE_DISTRI_DIR: &str = "/etc/sysupdate.d";
/// Définitions repart régénérées pour l'installation (tmpfs, volatiles).
const REPART_INSTALL_DIR: &str = "/run/yaor-install/repart";
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
    --source <CHEMIN|URL>   surcharge la base des artefacts racine
                            (défaut : Path= du transfert racine)
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

    // 1. Schéma GPT : définitions régénérées pour poser l'étiquette du slot A
    //    à la création (la préversion avec --dry-run montre le plan).
    section("Création du schéma GPT");
    let defs = preparer_definitions(&opts.version)?;
    exec(
        "systemd-repart",
        &[
            &format!("--definitions={defs}"),
            "--dry-run=no",
            "--empty=allow",
            "--pretty=yes",
            &opts.disk,
        ],
    )?;

    // 2. Image racine écrite en flux dans le slot A (téléchargement →
    //    vérification éventuelle de la somme → décompression à l'écriture).
    section("Écriture de l'image racine");
    ecrire_image_racine(&opts.disk, &opts.version, &opts.source)?;

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

// --- Définitions repart et transferts ----------------------------------------

/// Prépare les définitions repart de l'installation dans `/run/yaor-install/repart`
/// : copies de `/etc/repart.d`, où la **première** définition de slot racine
/// (« root-a ») reçoit l'étiquette `yaor-root_<version>` au lieu de « _empty ».
/// `repart` pose l'étiquette GPT à la création de la partition — inutile de la
/// retoucher après l'écriture de l'image. Les définitions ultérieures de slot
/// racine (slot B) restent « _empty » : ce sont elles que `systemd-sysupdate`
/// ciblera lors des mises à jour A/B.
fn preparer_definitions(version: &str) -> Result<String, String> {
    let dir = Path::new(REPART_INSTALL_DIR);
    let _ = std::fs::remove_dir_all(dir);
    creer_dossier(dir)?;
    let src_dir = Path::new(REPART_DISTRI_DIR);
    let mut defs: Vec<PathBuf> = std::fs::read_dir(src_dir)
        .map_err(|e| format!("lecture de {} : {e}", src_dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "conf").unwrap_or(false))
        .collect();
    defs.sort();
    let mut slot_a_traite = false;
    for src in defs {
        let nom = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut texte = std::fs::read_to_string(&src)
            .map_err(|e| format!("lecture de {} : {e}", src.display()))?;
        if !slot_a_traite && texte.contains("Type=root-x86-64") {
            // Slot A : l'étiquette porte la version dès la création de la
            // partition (repart écrit la table GPT une fois pour toutes).
            texte = texte.replace(
                "Label=_empty",
                &format!("Label={ROOT_LABEL_PREFIX}{version}"),
            );
            slot_a_traite = true;
        }
        let dest = dir.join(&nom);
        std::fs::write(&dest, texte)
            .map_err(|e| format!("écriture de {} : {e}", dest.display()))?;
    }
    if !slot_a_traite {
        return Err(format!(
            "aucune définition de slot racine (Type=root-x86-64) dans {REPART_DISTRI_DIR}"
        ));
    }
    Ok(REPART_INSTALL_DIR.to_string())
}

/// Description du transfert racine, extraite de `/etc/sysupdate.d/*.transfer`
/// (celui qui écrit une **partition** — le transfert du noyau écrit un fichier).
struct TransfertRacine {
    /// `Path=` de la section Source : base des artefacts (URL ou chemin).
    base: String,
    /// `MatchPattern=` : motif du nom d'artefact, `@v` remplacé par la version.
    motif: String,
    /// `Verify=` : « checksum » (défaut), « signature » ou « no ».
    verify: String,
}

/// Découpe un fichier de définition en sections (`[Source]`, `[Transfer]`, …).
fn sections(texte: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in texte.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            out.push((l[1..l.len() - 1].to_string(), Vec::new()));
        } else if !l.is_empty() {
            if let Some((_, v)) = out.last_mut() {
                v.push(l.to_string());
            }
        }
    }
    out
}

/// Valeur d'une clé dans une section donnée.
fn champ_de(sections: &[(String, Vec<String>)], titre: &str, cle: &str) -> Option<String> {
    sections
        .iter()
        .find(|(t, _)| t == titre)
        .and_then(|(_, lignes)| {
            let prefixe = format!("{cle}=");
            lignes.iter().find_map(|l| {
                l.strip_prefix(&prefixe)
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            })
        })
}

fn transfert_racine() -> Result<TransfertRacine, String> {
    let dir = Path::new(SYSUPDATE_DISTRI_DIR);
    let mut fichiers: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("lecture de {} : {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "transfer").unwrap_or(false))
        .collect();
    fichiers.sort();
    for p in fichiers {
        let texte = std::fs::read_to_string(&p)
            .map_err(|e| format!("lecture de {} : {e}", p.display()))?;
        // Le transfert racine est celui qui écrit une partition GPT (le
        // transfert du noyau écrit un fichier régulier).
        if champ_de(&sections(&texte), "Target", "Type").as_deref() != Some("partition") {
            continue;
        }
        let s = sections(&texte);
        return Ok(TransfertRacine {
            base: champ_de(&s, "Source", "Path")
                .ok_or_else(|| format!("Path= absent de [Source] dans {}", p.display()))?,
            motif: champ_de(&s, "Source", "MatchPattern")
                .ok_or_else(|| format!("MatchPattern= absent de [Source] dans {}", p.display()))?,
            // Défaut « checksum » : aligné sur sysupdate 262 — la directive
            // Verify (tri-state) ne pilote que la signature du manifeste, la
            // somme de l'artefact reste toujours vérifiée ; la signature GPG
            // reste un point ouvert (ADR-007), jamais un défaut.
            verify: champ_de(&s, "Transfer", "Verify").unwrap_or_else(|| "checksum".into()),
        });
    }
    Err(format!(
        "aucun transfert racine (cible Type=partition) dans {SYSUPDATE_DISTRI_DIR}"
    ))
}

/// URL d'un artefact à partir d'une base `http(s)://` ou d'un répertoire local
/// (servi alors par curl via `file://`, même chemin de code).
fn url_de(base: &str, nom: &str) -> Result<String, String> {
    if base.starts_with("http://") || base.starts_with("https://") {
        return Ok(format!("{}/{}", base.trim_end_matches('/'), nom));
    }
    let local = Path::new(base).join(nom);
    let abs = local
        .canonicalize()
        .map_err(|e| format!("artefact {} introuvable : {e}", local.display()))?;
    Ok(format!("file://{}", abs.display()))
}

/// Vérifie la somme SHA-256 de l'artefact (mode `Verify=checksum`) : le
/// manifeste `SHA256SUMS` de la base est comparé à un calcul en flux
/// (téléchargement sans écriture disque). Écart v0.2 : `Verify=signature`
/// exige un fournisseur GPG absent (ADR-007, point ouvert) — refus explicite.
fn verifier_somme(url_art: &str, base: &str, nom: &str) -> Result<(), String> {
    let url_sommes = url_de(base, "SHA256SUMS")?;
    let sommes = capture("curl", &["-sSL", "--fail", &url_sommes])?;
    let attendu = sommes
        .lines()
        .filter(|l| l.contains(nom))
        .next()
        .and_then(|l| l.split_whitespace().next())
        .ok_or_else(|| format!("somme de {nom} absente du manifeste SHA256SUMS"))?
        .to_string();
    eprintln!("vérification de la somme SHA-256 de {nom}…");
    let calcul = capture(
        "bash",
        &["-c", &format!("curl -sSL --fail '{url_art}' | sha256sum")],
    )?;
    let calcule = calcul.split_whitespace().next().unwrap_or_default();
    if attendu != calcule {
        return Err(format!(
            "somme SHA-256 inattendue pour {nom} : calculé {calcule}, attendu {attendu}"
        ));
    }
    Ok(())
}

/// Télécharge l'image racine et l'écrit dans le slot A, en flux (aucun
/// fichier temporaire : `curl | xz -d > partition`), après vérification
/// éventuelle de sa somme.
fn ecrire_image_racine(
    disk: &str,
    version: &str,
    source: &Option<String>,
) -> Result<(), String> {
    let t = transfert_racine()?;
    let nom = t.motif.replace("@v", version);
    let base = source.clone().unwrap_or(t.base);
    let url_art = url_de(&base, &nom)?;
    match t.verify.as_str() {
        "signature" => {
            return Err(
                "Verify=signature non pris en charge en v0.2 (aucun fournisseur GPG,\
 ADR-007 point ouvert) : servez les artefacts avec Verify=checksum ou Verify=no"
                    .into(),
            )
        }
        "checksum" => verifier_somme(&url_art, &base, &nom)?,
        "no" => eprintln!("vérification de l'artefact écartée (Verify=no)"),
        autre => return Err(format!("Verify={autre} inconnu")),
    }
    let part = partition_par_etiquette(disk, &format!("{ROOT_LABEL_PREFIX}{version}"))?;
    eprintln!("écriture de {nom} dans {part}…");
    exec(
        "bash",
        &["-c", &format!("curl -sSL --fail '{url_art}' | xz -d -c > '{part}'")],
    )?;
    exec("sync", &[])?;
    Ok(())
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
/// En BIOS, le stage 2 (écrit au début de la partition « BIOS boot ») cherche
/// `limine-bios.sys` à la racine d'une partition du disque (« root, /boot,
/// /limine ou /boot/limine », cf. avertissement de `limine bios-install`) :
/// il est copié à la racine de l'ESP, sans quoi le démarrage QEMU/BIOS reste
/// muet (aucun message, date d'attente indéfinie).
fn installer_limine(disk: &str, esp: &Path) -> Result<(), String> {
    let efi_dir = esp.join("EFI/BOOT");
    creer_dossier(&efi_dir)?;
    let src = Path::new(LIMINE_SHARE).join("BOOTX64.EFI");
    let dst = efi_dir.join("BOOTX64.EFI");
    std::fs::copy(&src, &dst)
        .map_err(|e| format!("copie de {} vers {} : {e}", src.display(), dst.display()))?;
    eprintln!("démarrage UEFI installé : {}", dst.display());
    let bios_sys = Path::new(LIMINE_SHARE).join("limine-bios.sys");
    let dst_sys = esp.join("limine-bios.sys");
    std::fs::copy(&bios_sys, &dst_sys).map_err(|e| {
        format!("copie de {} vers {} : {e}", bios_sys.display(), dst_sys.display())
    })?;
    eprintln!("stage 2 BIOS installé : {}", dst_sys.display());
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
