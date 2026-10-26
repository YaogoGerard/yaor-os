// Politique de placement des fenêtres du compositor YAOR (jalon v0.4).
//
// Empilement simple : chaque nouvelle fenêtre est placée sous la
// précédente en cascade (décalage constant), repositionnées dans les
// sorties aux connecteurs/déconnexions (modèle anvil shell::fixup,
// simplifié pour un layout horizontal unique).

use crate::state::Yaor;

/// Décalage en cascade d'une nouvelle fenêtre.
const CASCADE_OFFSET: (i32, i32) = (20, 20);

/// Emplacement de la prochaine fenêtre : à droite du haut de la pile,
/// borné aux sorties (simple en v0.4 — la gestion de layout viendra
/// avec le service de session, hors jalon v0.4).
pub fn next_window_location(state: &mut Yaor) -> (i32, i32) {
    let count = state.space.elements().count() as i32;
    let mut x = COLLAPSE_MARGIN + count * CASCADE_OFFSET.0;
    let mut y = COLLAPSE_MARGIN + count * CASCADE_OFFSET.1;

    // bornage aux sorties connues (sinon origine)
    if let Some(output) = state.space.outputs().next() {
        if let Some(geo) = state.space.output_geometry(output) {
            if x >= geo.size.w {
                x = COLLAPSE_MARGIN;
            }
            if y >= geo.size.h {
                y = COLLAPSE_MARGIN;
            }
        }
    }

    (x, y)
}

const COLLAPSE_MARGIN: i32 = 20;

/// Après connecteur/déconnecteur : on repositionne les fenêtres hors
/// limites dans la première sortie (fixupPositions-like d'anvil,
/// simplifié : pas de décoration ni layout dynamique en v0.4).
pub fn fixup_positions(state: &mut Yaor) {
    // v0.4 : les fenêtres gèrent leur position via leurs grabs ; le
    // repositionnement au changement de sorties reste local aux éléments
    // my simple. À l'étape de window decorations cette fonction prend
    // son extension.
    let _ = state;
}
