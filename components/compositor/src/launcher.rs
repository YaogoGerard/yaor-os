// launcher — lancement des surfaces du bureau par le profil (jalon v0.6).
//
// Le compositor lance lui-même les clients du shell selon le profil
// élu (desktop.toml [profile] name) : le panel (barre supérieure —
// menu + taskbar/indicateurs selon le profil) et le dock (profil
// macOS-like seul). Le changement de profil (SIGHUP) réconcilie :
// dock lancé en entrant dans macOS-like, terminé en sortant ; le
// panel reste voulu des trois profils.
//
// Porte QA : `YAOR_SHELL_AUTOSTART=0` désactive tout — les harnais
// gardent la main sur leurs propres instances (une barre en double
// ferait deux surfaces layer-shell, deux zones exclusives et des
// mesures de pixels fausses). Sans la variable (image réelle), les
// surfaces du profil sont lancées à l'ouverture de session.
//
// Les enfants héritent du stdio du compositor (leurs jetons
// « yaor-panel: » / « yaor-dock: » tombent donc dans le même journal)
// et reçoivent WAYLAND_DISPLAY/XDG_RUNTIME_DIR posés depuis la socket
// du compositor — le même export explicite que les harnais.
// Terminaison : SIGTERM au démontage, avant le retrait de la socket
// (les clients voient leur siège s'éteindre, pas une socket morte).

use std::ffi::OsString;
use std::process::Child;
use std::process::Command;

use yaor_config::Profile;

/// État du lanceur : les clients du shell en cours.
#[derive(Default)]
pub struct Launcher {
    panel: Option<Child>,
    dock: Option<Child>,
}

/// Autostart actif ? Défaut ON (l'image réelle lance le shell du
/// profil) ; `YAOR_SHELL_AUTOSTART=0` le coupe — autre chose que « 0 »
/// ou l'absence de la variable laissent le lancement actif.
fn enabled() -> bool {
    match std::env::var("YAOR_SHELL_AUTOSTART") {
        Ok(v) => v != "0",
        Err(_) => true,
    }
}

impl Launcher {
    /// Premier lancement (à l'ouverture de session — sortie primaire
    /// déjà configurée : les surfaces layer-shell trouvent une cible).
    pub fn start(&mut self, socket_name: &Option<OsString>) {
        if !enabled() {
            tracing::info!("launcher : autostart désactivé (YAOR_SHELL_AUTOSTART=0)");
            return;
        }
        self.spawn_step(socket_name, None);
    }

    /// Réconciliation au changement de profil (SIGHUP) — l'ancien
    /// profil sert à décider l'arrêt du dock (exit macOS-like).
    pub fn reconcile(&mut self, socket_name: &Option<OsString>, avant: Option<Profile>) {
        if !enabled() {
            return;
        }
        self.spawn_step(socket_name, avant);
    }

    /// Transmission du SIGHUP aux enfants (v0.6, leçon QA t-yaor-
    /// launcher : le compositor recharge sa configuration au SIGHUP,
    /// mais le panel et le dock ne le sauraient pas tous seuls — ils
    /// gardaient leur config d'origine : taskbar figée dans le profil
    /// macos, indicateurs d'espaces absents en tiling). Cascade :
    /// d'abord le panel (les deux profils), puis le dock — chacun
    /// relit desktop.toml et se remasque/remasque selon le profil
    /// élu ; les surfaces layer-shell masquées reviennnent de la
    /// même façon qu'au SIGHUP direct des harnais.
    pub fn sighup_children(&mut self) {
        for (name, child) in [("panel", self.panel.as_mut()), ("dock", self.dock.as_mut())] {
            if let Some(child) = child {
                // SIGHUP = rechargement, pas terminaison.
                let r = unsafe { libc::kill(child.id() as libc::c_int, libc::SIGHUP) };
                if r == 0 {
                    tracing::info!("launcher : SIGHUP transmis au {name}");
                } else {
                    tracing::warn!("launcher : SIGHUP au {name} : errno {r}");
                }
            }
        }
    }

    fn spawn_step(&mut self, socket_name: &Option<OsString>, avant: Option<Profile>) {
        let profile = crate::config::get().profile.name;
        let label = profile.map_or("aucun", Profile::as_str);

        // Panel : voulu des trois profils (barre supérieure : menu +
        // taskbar classique/tiling, indicateurs d'espaces tiling).
        if self.panel.is_none() {
            self.panel = spawn_client("yaor-panel", socket_name);
            match self.panel.is_some() {
                true => tracing::info!("launcher : panel lancé (profil {label})"),
                false => tracing::warn!("launcher : lancement du panel impossible"),
            }
        }

        // Dock : profil macOS-like seul — « avant » (profil du
        // rechargement précédent) décide l'arrêt : entrée en macos →
        // lancé ; sortie → terminé.
        match (avant, profile) {
            (Some(Profile::Macos), Some(Profile::Macos)) => {}
            (Some(Profile::Macos), _) => {
                if let Some(child) = self.dock.as_mut() {
                    terminate_child(child);
                    tracing::info!("launcher : dock arrêté (profil {label})");
                }
                self.dock = None;
            }
            (_, Some(Profile::Macos)) => {
                if self.dock.is_none() {
                    self.dock = spawn_client("yaor-dock", socket_name);
                    match self.dock.is_some() {
                        true => tracing::info!("launcher : dock lancé (profil {label})"),
                        false => tracing::warn!("launcher : lancement du dock impossible"),
                    }
                }
            }
            _ => {}
        }
    }

    /// Terminaison des enfants (démontage — avant le retrait de la
    /// socket : les clients voient leur siège s'éteindre, pas une socket
    /// morte).
    pub fn shutdown(&mut self) {
        if let Some(child) = self.panel.as_mut() {
            terminate_child(child);
            tracing::info!("launcher : panel arrêté (démontage)");
        }
        self.panel = None;
        if let Some(child) = self.dock.as_mut() {
            terminate_child(child);
            tracing::info!("launcher : dock arrêté (démontage)");
        }
        self.dock = None;
    }
}

fn terminate_child(child: &mut Child) {
    // Arrêt gracieux par SIGTERM (permet le retrait des surfaces et le flush).
    unsafe {
        libc::kill(child.id() as libc::c_int, libc::SIGTERM);
    }
    let _ = child.wait();
}

fn spawn_client(program: &str, socket_name: &Option<OsString>) -> Option<Child> {
    let Some(socket_name) = socket_name else {
        // Session sans socket (mode headless) : rien à lancer.
        return None;
    };
    // Répertoire runtime : celui du compositor s'il est posé, sinon le
    // repli smithay de ListeningSocket::new_auto (XDG_RUNTIME_DIR
    // absent) — /tmp.
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    Command::new(format!("/usr/bin/{program}"))
        .env("WAYLAND_DISPLAY", socket_name)
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .spawn()
        .map_err(|err| tracing::warn!("lancement {program} : {err}"))
        .ok()
}
