// yaor-compositor — compositor graphique YAOR (ADR-005, jalon v0.4).
//
// Binaire intégré au live (étape 1) et session udev/KMS réelle
// (étape 2) : session libseat, backend udev, rendu GLES via Smithay.
// Les modules portent la structure du LLD
// (docs/04-design-lld/compositor/compositor.md).

mod config;
mod grabs;
mod input;
mod policy;
mod render;
mod session;
mod shell;
mod state;
mod xway;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROG: &str = "yaor-compositor";

fn usage(out: &mut dyn std::io::Write) {
    let _ = writeln!(out, "Usage: {PROG} [--tty-udev] [--help] [--version]");
    let _ = writeln!(out, "Compositor graphique YAOR (Smithay {V})", V = smithay_version());
    let _ = writeln!(out, "  --tty-udev    démarre une session udev/KMS (libseat)");
    let _ = writeln!(out, "  --help        affiche cette aide");
    let _ = writeln!(out, "  --version     affiche la version du produit");
}

fn smithay_version() -> &'static str {
    // Version de la bibliothèque : épinglée par le build (ADR-016) et
    // vérifiée par la phase de build. La chaîne est déclarée ici pour la
    // lisibilité de --version (la crate n'expose pas d'accesseur stable).
    "0.7.0"
}

/// Journal de la session (même convention qu'anvil) : `RUST_LOG` filtre,
/// sortie compacte par défaut vers stderr — redirigée par le lanceur.
fn init_logging() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }
}

fn main() {
    init_logging();
    let mut args = std::env::args().skip(1);
    let mut tty_udev = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tty-udev" => tty_udev = true,
            "--help" | "-h" => {
                usage(&mut std::io::stdout());
                return;
            }
            "--version" | "-V" => {
                println!("{PROG} {VERSION}");
                return;
            }
            other => {
                eprintln!("{PROG} : argument inconnu : {other}");
                usage(&mut std::io::stderr());
                std::process::exit(2);
            }
        }
    }
    if tty_udev {
        session::run_udev();
        return;
    }
    // Sans session : l'usage seul n'est pas une session de test.
    eprintln!(
        "{PROG} {VERSION} : aucune session demandée — utiliser --tty-udev"
    );
    usage(&mut std::io::stderr());
    std::process::exit(1);
}
