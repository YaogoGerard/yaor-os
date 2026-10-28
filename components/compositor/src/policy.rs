// Politique de placement des fenêtres du compositor YAOR (jalon v0.4,
// étendue v0.5 « Desktop Core »).
//
// v0.4 — cascade : chaque nouvelle fenêtre est placée sous la précédente
// en cascade (décalage constant 320x200), repositionnée dans les sorties
// aux connecteurs/déconnexions (modèle anvil shell::fixup, simplifié).
//
// v0.5 — mode **maître + pile** (REQ-SW-006, LLD desktop-core) : la
// première fenêtre d'une sortie occupe la pleine surface, la suivante la
// divise (maître à gauche, pile à droite), la pile se partage
// verticalement pour les suivantes. Le mode est choisi par la
// configuration (défaut : cascade — les QA v0.4 restent exactement
// elles-mêmes) et permutable à chaud au SIGHUP (rechargement sans
// quitter la session, TEST-DESK-001).
//
// Une fenêtre **flottante** (déplacée ou redimensionnée via un grab
// interactif — le scénario QA v0.4 `t-yaor-comp-wm` garde sa géométrie
// après relâchement) sort de la disposition automatique : elle n'occupe
// pas de tuile et ne bouge pas au re-tile.
//
// Les raccourcis clavier (LLD desktop-core, style sway/Hyprland, Mod =
// Super) pilotent le layout : focus, swap, Mod+Tab avec le maître,
// Mod+Q fermeture, Mod+g ratio, Mod+Shift+E arrêt propre. Les
// affectations viennent de `desktop.toml` (rebindables — REQ-SW-015).

use smithay::{
    desktop::Window,
    input::keyboard::{Keysym, ModifiersState},
    output::Output,
    utils::{IsAlive, Logical, Point, Rectangle, Size},
    wayland::seat::WaylandFocus,
};

use yaor_config::LayoutMode;

use crate::state::Yaor;

// ---------------------------------------------------------------------------
// État du layout (jalon v0.5)
// ---------------------------------------------------------------------------

/// Liste des fenêtres tiling (ordre = maître d'abord) et flottantes.
/// Les fenêtres mortes sont purgées à chaque re-tile (SIGHUP, boucle).
#[derive(Default)]
pub struct LayoutState {
    /// Fenêtres engagées dans la disposition automatique : l'index 0
    /// est le maître, les suivantes la pile.
    pub tiled: Vec<Window>,    /// Fenêtres sorties du tiling (grab interactif) : géométrie figée.
    pub floating: Vec<Window>,
}

/// Décalage en cascade d'une nouvelle fenêtre (v0.4).
///
/// Le pas est volontairement grand (320x200, face aux tailles initiales
/// 640x480) pour que les fenêtres empilées restent largement visibles
/// en même temps : chaque fenêtre laisse dépasser une bande franche de
/// la précédente, ce qui rend les preuves QA observables à l'écran
/// (screendump « deux fenêtres distinctes », clic sur une bande
/// visible de la fenêtre du fond).
const CASCADE_OFFSET: (i32, i32) = (320, 200);

const COLLAPSE_MARGIN: i32 = 20;

// ---------------------------------------------------------------------------
// Emplacements d'accueil (next_window_location — API v0.4 conservée)
// ---------------------------------------------------------------------------

/// Emplacement de la prochaine fenêtre : en cascade à droite du haut de
/// la pile (comportement v0.4 exact) ; en maître+pile à l'origine de la
/// sortie sous le pointeur (le re-tile de la boucle affine en ≤ 16 ms).
pub fn next_window_location(state: &mut Yaor) -> (i32, i32) {
    match crate::config::get().layout.mode {
        LayoutMode::MasterStack => master_slot_location(state),
        LayoutMode::Cascade => cascade_location(state),
    }
}

/// Cascade v0.4 — copie exacte (aucune régression permise).
fn cascade_location(state: &Yaor) -> (i32, i32) {
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

/// Maître+pile : origine (premier gap) de la sortie sous le pointeur —
/// les fenêtres naissent sur la sortie active (focus suit le pointeur).
fn master_slot_location(state: &Yaor) -> (i32, i32) {
    let gaps = crate::config::get().layout.gaps as i32;
    if let Some(geo) = output_under(state).and_then(|o| state.space.output_geometry(o)) {
        return (geo.loc.x + gaps, geo.loc.y + gaps);
    }
    (gaps, gaps)
}

/// Sortie sous le pointeur (sortie active), sinon la première connue.
fn output_under(state: &Yaor) -> Option<&Output> {
    let loc = (state.pointer_location.x as i32, state.pointer_location.y as i32);
    let mut fallback = None;
    for output in state.space.outputs() {
        if let Some(geo) = state.space.output_geometry(output) {
            if geo.contains(loc) {
                return Some(output);
            }
            fallback = fallback.or(Some(output));
        }
    }
    fallback
}

// ---------------------------------------------------------------------------
// Recensement (register / mark_floating)
// ---------------------------------------------------------------------------

/// Enregistre une fenêtre nouvelle : rejoint la fin de la pile. En
/// cascade, la liste n'influence pas le placement (v0.4 inchangé).
pub fn register(state: &mut Yaor, window: &Window) {
    state.layout.tiled.push(window.clone());
    tracing::debug!("layout : fenêtre recensée ({} tiling)", state.layout.tiled.len());
}

/// Sort une fenêtre du tiling : déplacée/redimensionnée interactivement
/// (grab move/resize), elle devient flottante et conserve sa géométrie
/// après relâchement (le scénario QA `t-yaor-comp-wm`).
pub fn mark_floating(state: &mut Yaor, window: &Window) {
    state.layout.tiled.retain(|w| !same_window(w, window));
    state.layout.floating.retain(|w| !same_window(w, window));
    state.layout.floating.push(window.clone());
    tracing::debug!("layout : fenêtre flottante ({} tiling)", state.layout.tiled.len());
}

// ---------------------------------------------------------------------------
// Re-tile
// ---------------------------------------------------------------------------

/// Recalcule la disposition conforme à la configuration courante.
///
/// Cascade : no-op strict (v0.4 exact). Maître+pile : purge des fenêtres
/// mortes, adoption des éléments non recensés (mode permuté au
/// rechargement), disposition maître+pile **par sortie**.
pub fn recompute(state: &mut Yaor) {
    if crate::config::get().layout.mode != LayoutMode::MasterStack {
        return;
    }

    // purge des fenêtres mortes (aligné sur Space::refresh)
    state.space.refresh();
    state.layout.tiled.retain(Window::alive);
    state.layout.floating.retain(Window::alive);

    // adoption : toute fenêtre non recensée rejoint le tiling (mode
    // permuté après coup au rechargement de configuration)
    for w in state.space.elements().cloned().collect::<Vec<_>>() {
        if !state.layout.tiled.iter().any(|t| same_window(t, &w))
            && !state.layout.floating.iter().any(|f| same_window(f, &w))
        {
            state.layout.tiled.push(w);
        }
    }

    // groupement par sortie ; ordre de la liste tiling conservé
    let outputs = state
        .space
        .outputs()
        .filter_map(|o| {
            let geo = state.space.output_geometry(o)?;
            Some((o.clone(), geo))
        })
        .collect::<Vec<_>>();

    let tiled = state.layout.tiled.clone();

    for (output, geo) in &outputs {
        let group: Vec<Window> = tiled
            .iter()
            .filter(|w| window_output_of(state, w, &outputs).as_ref() == Some(output))
            .cloned()
            .collect();
        tile_group(state, *geo, &group);
    }
}

/// Sortie à laquelle une fenêtre est rattachée : contient le centre de
/// sa géométrie ; sinon (client pas encore commité) la première sortie.
fn window_output_of(
    state: &Yaor,
    window: &Window,
    outputs: &[(Output, Rectangle<i32, Logical>)],
) -> Option<Output> {
    let geo = state.space.element_geometry(window)?;
    let center = (geo.loc.x + geo.size.w / 2, geo.loc.y + geo.size.h / 2);
    outputs
        .iter()
        .find(|(_, g)| g.contains(center))
        .map(|(o, _)| o.clone())
        .or_else(|| outputs.first().map(|(o, _)| o.clone()))
}

/// Après connecteur/déconnecteur (API v0.4 conservée) : re-tile selon la
/// configuration (cascade : no-op — les grabs gardent la main).
pub fn fixup_positions(state: &mut Yaor) {
    recompute(state);
}

/// Dispose un groupe de fenêtres d'une sortie : maître à gauche, pile
/// divisée verticalement à droite, gaps issus de la configuration.
fn tile_group(state: &mut Yaor, geo: Rectangle<i32, Logical>, windows: &[Window]) {
    let cfg = crate::config::get();
    let gaps = cfg.layout.gaps as i32;
    let n = windows.len();
    if n == 0 {
        return;
    }

    let x0 = geo.loc.x + gaps;
    let y0 = geo.loc.y + gaps;
    let avail_w = geo.size.w - 2 * gaps;
    let avail_h = geo.size.h - 2 * gaps;
    if avail_w <= 0 || avail_h <= 0 {
        return;
    }

    let master_w = if n == 1 {
        avail_w
    } else {
        let w = (avail_w as f64 * cfg.layout.ratio) as i32;
        // la pile doit rester ≥ 1 px chaque côté d'un gap
        w.clamp(1, avail_w.max(2) - gaps - 1)
    };

    // pile : (n-1) fenêtres, (n-2) gaps inter-pile
    let stack_count = n - 1;
    let stack_h_total = avail_h - (stack_count as i32 - 1) * gaps;
    let stack_w = (avail_w - master_w - gaps).max(1);

    let mut slots: Vec<Rectangle<i32, Logical>> = Vec::with_capacity(n);
    slots.push(Rectangle::new(
        Point::from((x0, y0)),
        Size::from((master_w, avail_h)),
    ));
    let mut y = y0;
    for k in 0..stack_count {
        // hauteur équirépartie ; la dernière prend le reste (pas de
        // pixel perdu à l'arrondi)
        let h = if k as i32 == stack_count as i32 - 1 {
            y0 + avail_h - y
        } else {
            stack_h_total / stack_count as i32
        };
        slots.push(Rectangle::new(
            Point::from((x0 + master_w + gaps, y)),
            Size::from((stack_w, h.max(1))),
        ));
        y += h + gaps;
    }

    for (window, slot) in windows.iter().zip(slots) {
        // déplacement si utile : sinon on ne remappe pas (le re-tile
        // périodique ne change ni le z-order ni la géométrie)
        let mapped = state.space.element_geometry(window).map(|g| g.loc) != Some(slot.loc);
        if mapped {
            state.space.map_element(window.clone(), slot.loc, false);
        }

        // taille : configure seulement au changement (le client commit,
        // current_state suit — pas de spam de configure)
        if let Some(toplevel) = window.toplevel() {
            if toplevel.current_state().size != Some(slot.size) {
                toplevel.with_pending_state(|s| s.size = Some(slot.size));
                toplevel.send_configure();
                tracing::info!(
                    "tiling : géométrie {}x{} à ({},{})",
                    slot.size.w,
                    slot.size.h,
                    slot.loc.x,
                    slot.loc.y
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Raccourcis clavier
// ---------------------------------------------------------------------------

/// Actions pilotant le bureau (affectations par défaut « sway-like »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    FocusNext,
    FocusPrev,
    SwapNext,
    SwapPrev,
    SwapMaster,
    Close,
    RatioGrow,
    RatioShrink,
    Quit,
}

/// Identifie l'action déclenchée par la touche pressée (raccourcis
/// rebindables — REQ-SW-015). Aucune correspondance : None.
///
/// Le sym recherché : sym sans modification d'abord (Shift+lettre →
/// lettre, comme sway), sinon sym modifié (en repli).
pub fn find_action(mods: &ModifiersState, raw: Keysym, modified: Keysym) -> Option<Action> {
    let cfg = crate::config::get();
    let candidates: [(&str, Action); 9] = [
        (&cfg.keys.focus_next, Action::FocusNext),
        (&cfg.keys.focus_prev, Action::FocusPrev),
        (&cfg.keys.swap_next, Action::SwapNext),
        (&cfg.keys.swap_prev, Action::SwapPrev),
        (&cfg.keys.master_swap, Action::SwapMaster),
        (&cfg.keys.close, Action::Close),
        (&cfg.keys.ratio_grow, Action::RatioGrow),
        (&cfg.keys.ratio_shrink, Action::RatioShrink),
        (&cfg.keys.quit, Action::Quit),
    ];
    for (spec, action) in candidates {
        let Some((m, k)) = parse_shortcut(spec) else {
            continue;
        };
        if mods_match(m, mods) && (k == raw || k == modified) {
            return Some(action);
        }
    }
    None
}

/// Parse « Mod+Shift+Q » : Mod/Super → logo (Mod = Super, convention
/// sway), Ctrl, Alt, Shift ; puis nom de touche (Right, Tab, …) ou
/// caractère (lettre, chiffre).
pub fn parse_shortcut(spec: &str) -> Option<(ModifiersState, Keysym)> {
    let mut mods = ModifiersState::default();
    let mut key: Option<Keysym> = None;
    for token in spec.split('+') {
        let token = token.trim();
        match token.to_ascii_lowercase().as_str() {
            "mod" | "super" | "logo" | "cmd" => mods.logo = true,
            "ctrl" | "control" => mods.ctrl = true,
            "alt" => mods.alt = true,
            "shift" => mods.shift = true,
            _ => {
                key = Some(if token.chars().count() == 1 {
                    // Lettre insensible à la casse : « Mod+Q » désigne la
                    // TOUCHE q, pas le keysym majuscule (leçon t-diag-key2 :
                    // from_char('Q') = keysym Q — sans Shift, la touche
                    // pressée donne le keysym q minuscule, le raccourci
                    // n'était reconnu que par accident via Shift+E).
                    Keysym::from_char(
                        token.chars().next().unwrap().to_ascii_lowercase(),
                    )
                } else {
                    match token {
                        "Right" | "right" => Keysym::Right,
                        "Left" | "left" => Keysym::Left,
                        "Up" | "up" => Keysym::Up,
                        "Down" | "down" => Keysym::Down,
                        "Tab" | "tab" => Keysym::Tab,
                        "Return" | "Enter" => Keysym::Return,
                        "Escape" | "Esc" => Keysym::Escape,
                        _ => return None,
                    }
                });
            }
        }
    }
    key.map(|k| (mods, k))
}

/// Modificateurs attendus == pressés (les verrous num/CapsLk ignorés :
/// l'état pertinent est ctrl/alt/shift/logo).
fn mods_match(expected: ModifiersState, actual: &ModifiersState) -> bool {
    expected.ctrl == actual.ctrl
        && expected.alt == actual.alt
        && expected.shift == actual.shift
        && expected.logo == actual.logo
}

/// Exécute une action de bureau.
pub fn run_action(state: &mut Yaor, action: Action) {
    if crate::config::get().layout.mode != LayoutMode::MasterStack
        && action != Action::Close
        && action != Action::Quit
    {
        // layout non piloté en cascade (v0.4 inchangé — aucune
        // régression permise sur les QA de cette zone)
        return;
    }

    match action {
        Action::Quit => {
            tracing::info!("arrêt demandé par un raccourci");
            state.running.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        Action::Close => {
            if let Some(window) = focused_window(state) {
                if let Some(toplevel) = window.toplevel() {
                    tracing::info!("raccourci : fermeture de la fenêtre focusée");
                    toplevel.send_close();
                } else if let Some(x11) = window.x11_surface() {
                    tracing::info!("raccourci : fermeture de la fenêtre X11 focusée");
                    let _ = x11.close();
                }
            }
        }
        Action::FocusNext => {
            if state.layout.tiled.is_empty() {
                return;
            }
            let n = state.layout.tiled.len();
            let from = focused_tiled_index(state).unwrap_or(n - 1);
            let idx = (from + 1) % n;
            focus_tiled(state, idx);
            tracing::info!("raccourci : focus suivant (tuile {idx}/{n})");
        }
        Action::FocusPrev => {
            if state.layout.tiled.is_empty() {
                return;
            }
            let n = state.layout.tiled.len();
            let from = focused_tiled_index(state).unwrap_or(0);
            let idx = (from + n - 1) % n;
            focus_tiled(state, idx);
            tracing::info!("raccourci : focus précédent (tuile {idx}/{n})");
        }
        Action::SwapNext => {
            if let Some(idx) = focused_tiled_index(state) {
                let n = state.layout.tiled.len();
                let j = (idx + 1) % n;
                state.layout.tiled.swap(idx, j);
                recompute(state);
                tracing::info!("raccourci : échange tuiles {idx} ↔ {j}");
            }
        }
        Action::SwapPrev => {
            if let Some(idx) = focused_tiled_index(state) {
                let n = state.layout.tiled.len();
                let j = (idx + n - 1) % n;
                state.layout.tiled.swap(idx, j);
                recompute(state);
                tracing::info!("raccourci : échange tuiles {idx} ↔ {j}");
            }
        }
        Action::SwapMaster => {
            if let Some(idx) = focused_tiled_index(state) {
                if idx == 0 {
                    tracing::info!("raccourci : fenêtre déjà au maître");
                } else {
                    state.layout.tiled.swap(0, idx);
                    recompute(state);
                    tracing::info!("raccourci : tuile {idx} ↔ maître");
                }
            }
        }
        Action::RatioGrow | Action::RatioShrink => {
            let delta = if action == Action::RatioGrow { 0.05 } else { -0.05 };
            crate::config::update(|cfg| {
                cfg.layout.ratio = (cfg.layout.ratio + delta).clamp(0.2, 0.8);
            });
            let ratio = crate::config::get().layout.ratio;
            tracing::info!("raccourci : ratio maître {ratio:.2}");
            recompute(state);
        }
    }
}

// ---------------------------------------------------------------------------
// Utilitaires de focus
// ---------------------------------------------------------------------------

/// Fenêtre tuilée à l'index donné : à l'écran (montée d'élément) et au
/// clavier (set_focus — même chemin que un appui du pointeur).
fn focus_tiled(state: &mut Yaor, idx: usize) {
    let Some(window) = state.layout.tiled.get(idx).cloned() else {
        return;
    };
    state.space.raise_element(&window, true);
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    let focus = crate::focus::FocusTarget::Wl(
        window
            .wl_surface()
            .map(|s| s.into_owned())
            .expect("fenêtre tiling wayland sans surface"),
    );
    state.seat.get_keyboard().unwrap().set_focus(state, Some(focus), serial);
}

/// Fenêtre focusée (n'importe laquelle), conversion FocusTarget → Window.
fn focused_window(state: &Yaor) -> Option<Window> {
    let focus = state.seat.get_keyboard()?.current_focus()?;
    for w in state.space.elements().cloned().collect::<Vec<_>>() {
        let matched = if let crate::focus::FocusTarget::Wl(ws) = &focus {
            w.toplevel().map(|t| t.wl_surface() == ws).unwrap_or(false)
        } else if let crate::focus::FocusTarget::X11(xs) = &focus {
            w.x11_surface().map(|s| s == xs).unwrap_or(false)
        } else {
            false
        };
        if matched {
            return Some(w);
        }
    }
    None
}

/// Index, dans la liste tiling, de la fenêtre focusée (None hors tiling).
fn focused_tiled_index(state: &Yaor) -> Option<usize> {
    let focus = focused_window(state)?;
    state.layout.tiled.iter().position(|t| same_window(t, &focus))
}

/// Deux mêmes fenêtres (surface X11 pour X ; wl_surface sinon —
/// comparaison d'identité d'objet wayland).
fn same_window(a: &Window, b: &Window) -> bool {
    if let (Some(x1), Some(x2)) = (a.x11_surface(), b.x11_surface()) {
        return x1 == x2;
    }
    a.wl_surface() == b.wl_surface()
}

#[cfg(test)]
mod tests {
    use super::parse_shortcut;
    use smithay::input::keyboard::Keysym;

    /// Leçon t-diag-key2 (run du 2026-10-28) : « Mod+Q » désigne la
    /// TOUCHE q, pas le keysym majuscule « Q ». from_char('Q') = keysym
    /// 0x51 ; la touche pressée sans Shift donne le keysym q (0x71) —
    /// le raccourci n'était alors reconnu que « par accident » quand
    /// Shift changeait le modified_sym (Mod+Shift+E). L'affectation
    /// ci-dessous est l'usine mêmes (default.toml).
    #[test]
    fn lettre_inscensible_a_la_casse() {
        // Mod+Q (close) : keysym cible = touche q minuscule.
        let (m, k) = parse_shortcut("Mod+Q").expect("parse Mod+Q");
        assert!(m.logo, "Mod = logo seul");
        assert!(!m.shift && !m.ctrl && !m.alt);
        assert_eq!(k, Keysym::from_char('q'));

        // Mod+Shift+E (quit) : la lettre reste minuscule — le match
        // contre le keysym BRUT (raw), sway-like, absorbe le Shift.
        let (m, k) = parse_shortcut("Mod+Shift+E").expect("parse quit");
        assert!(m.logo && m.shift);
        assert_eq!(k, Keysym::from_char('e'));

        // Mod+g (ratio) et Mod+Tab (maître) inchangés.
        assert_eq!(parse_shortcut("Mod+g").unwrap().1, Keysym::from_char('g'));
        assert_eq!(parse_shortcut("Mod+Tab").unwrap().1, Keysym::Tab);
    }
}
