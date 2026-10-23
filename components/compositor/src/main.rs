// yaor-compositor — compositor graphique YAOR (ADR-005, jalon v0.4).
//
// Squelette v0.4 étape 1 : binaire installé dans le live, gestion des
// arguments et héritage des conventions de session de anvil (--tty-udev).
// Les modules ci-dessous portent la structure du LLD
// (docs/04-design-lld/compositor/compositor.md) ; chaque étape du jalon
// les remplit : session → rendu → entrées → shell → policy → grabs →
// XWayland → config.
mod session;
mod render;
mod input;
mod shell;
mod policy;
mod grabs;
mod xway;
mod config;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROG: &str = "yaor-compositor";

fn usage(out: &mut dyn std::io::Write) {
    let _ = writeln!(out, "Usage: {PROG} [--tty-udev] [--help] [--version]");
    let _ = writeln!(out, "Compositor graphique YAOR (Smithay {V})", V = smithay_version());
    let _ = writeln!(out, "  --tty-udev    démarre sur tty créé via drv udev (session de test)");
    let _ = writeln!(out, "  --help        affiche cette aide");
    let _ = writeln!(out, "  --version     affiche la version du produit");
}

fn smithay_version() -> &'static str {
    // Version de la bibliothèque : elle est épinglée par le build (ADR-016)
    // et vérifiée par la phase de build. La chaîne est déclarée ici pour la
    // lisibilité de --version (la crate ne fournit pas d'accesseur stable).
    "0.7.0"
}

fn main() {
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
    // Session réelle : étape suivante du jalon v0.4 (session.rs). En
    // attendant, l'invocation sans session est un usage explicite.
    if tty_udev {
        eprintln!("{PROG} {VERSION} : session non implémentée (v0.4 étape 2)");
        std::process::exit(1);
    }
    usage(&mut std::io::stdout());
}
