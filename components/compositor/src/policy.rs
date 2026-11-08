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
    input::keyboard::{Keysym, ModifiersState},
    output::Output,
    utils::{IsAlive, Logical, Point, Rectangle, Size},
};

use yaor_config::LayoutMode;

use crate::{element::WindowElement, state::Yaor};

// ---------------------------------------------------------------------------
// État du layout (jalon v0.5)
// ---------------------------------------------------------------------------

/// Listes de fenêtrage : tiling (ordre = maître d'abord), flottantes
/// (géométrie libre), minimisées (sorties du rendu, restaurables —
/// jalon v0.6 étape 1b, bureau flottant du profil classique).
///
/// Default manuel : les slots d'espaces de travail doivent exister
/// dès l'origine (WORKSPACE_COUNT vecteurs vides) — un Default dérivé
/// laisserait `workspaces` vide et la première bascule indexerait
/// hors bornes.
pub struct LayoutState {
    /// Fenêtres engagées dans la disposition automatique : l'index 0
    /// est le maître, les suivantes la pile.
    pub tiled: Vec<WindowElement>,
    /// Fenêtres sorties du tiling (grab interactif) : géométrie figée.
    pub floating: Vec<WindowElement>,
    /// Fenêtres minimisées : (fenêtre, position de restauration).
    /// Sorties du Space (unmap) — invisibles et intouchables, la
    /// taskbar (étape 1b.c) ou Mod+m les restaurent.
    pub minimized: Vec<(WindowElement, i32, i32)>,
    /// Dernier mode de disposition vu par `recompute` : la permutation
    /// (rechargement de configuration) déclenche les migrations —
    /// libération des tuilées en flottant, adoption des flottantes en
    /// maître+pile. `None` avant le premier passage.
    pub last_mode: Option<crate::config::LayoutMode>,
    /// Espaces de travail HORS espace courant (v0.6 étape 3 — profil
    /// tiling, maquette « 1-5 en barre ») : fenêtres tuilées de l'espace
    /// N avec leur dernière position de tuile, SORTIES du Space (unmap —
    /// même modèle que `minimized`). L'espace courant vit dans `tiled` :
    /// tout le code v0.5 continue d'opérer sur l'espace affiché. Les
    /// slots sont fusionnés au départ du maître+pile (changement de
    /// profil — aucune fenêtre perdue).
    pub workspaces: Vec<Vec<(WindowElement, i32, i32)>>,
    /// Indice de l'espace courant (base 0 — affiché 1-5).
    pub current_workspace: usize,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            tiled: Vec::new(),
            floating: Vec::new(),
            minimized: Vec::new(),
            last_mode: None,
            workspaces: vec![Vec::new(); WORKSPACE_COUNT],
            current_workspace: 0,
        }
    }
}

/// Nombre d'espaces de travail (maquette profil tiling : « 1-5 en
/// barre »). Fixe : les indicateurs du panel et les raccourcis
/// Mod+1..5 sont taillés pour cinq.
pub const WORKSPACE_COUNT: usize = 5;

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
    match crate::config::layout_mode() {
        LayoutMode::MasterStack => master_slot_location(state),
        LayoutMode::Cascade => cascade_location(state),
        LayoutMode::Floating => floating_location(state),
    }
}

/// Flottant (v0.6 étape 1b) : la fenêtre naît **centrée** dans la zone
/// utile de la sortie sous le pointeur, décalée de 24 px par fenêtre
/// déjà présente (lisible quand plusieurs naissent d'affilée), bornée
/// à l'intérieur de la zone utile. Le grab move/resize la place
/// ensuite librement — sa position n'est plus jamais réécrite.
fn floating_location(state: &Yaor) -> (i32, i32) {
    const FLOATING_STEP: i32 = 24;
    let count = state.space.elements().count() as i32;
    if let Some(o) = output_under(state) {
        if let Some(geo) = state.space.output_geometry(o) {
            let map = smithay::desktop::layer_map_for_output(o);
            let zone = map.non_exclusive_zone();
            let usable = Rectangle::new(geo.loc + zone.loc, zone.size);
            let size = state
                .space
                .elements()
                .filter_map(|w| state.space.element_geometry(w))
                .map(|g| g.size)
                // la première fenêtre peut ne pas être committée
                // encore (géométrie 0x0) : ne PAS centrer sur elle —
                // repli 640x480 (leçon run 1 : W2 éjectée au
                // bord bas-droit sinon).
                .find(|s| s.w > 0 && s.h > 0)
                .unwrap_or(smithay::utils::Size::from((640, 480)));
            let mut x = usable.loc.x + (usable.size.w - size.w).max(0) / 2
                + count * FLOATING_STEP;
            let mut y = usable.loc.y + (usable.size.h - size.h).max(0) / 2
                + count * FLOATING_STEP;
            // bornage : la fenêtre ne dépasse jamais la zone utile (à
            // une marge de 8 px près pour rester agrippable).
            let max_x = usable.loc.x + usable.size.w - size.w - 8;
            let max_y = usable.loc.y + usable.size.h - size.h - 8;
            if max_x > usable.loc.x {
                x = x.min(max_x);
            }
            if max_y > usable.loc.y {
                y = y.min(max_y);
            }
            return (x, y);
        }
    }
    let count_off = count * FLOATING_STEP;
    (COLLAPSE_MARGIN + count_off, COLLAPSE_MARGIN + count_off)
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
/// v0.6 étape 1 : dans la zone utile (bandes exclusives soustraites).
fn master_slot_location(state: &Yaor) -> (i32, i32) {
    let gaps = crate::config::get().layout.gaps as i32;
    if let Some(o) = output_under(state) {
        if let Some(geo) = state.space.output_geometry(o) {
            let map = smithay::desktop::layer_map_for_output(o);
            let zone = map.non_exclusive_zone();
            let usable = Rectangle::new(geo.loc + zone.loc, zone.size);
            return (usable.loc.x + gaps, usable.loc.y + gaps);
        }
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
/// En flottant (v0.6 étape 1b), aucune disposition automatique : la
/// fenêtre rejoint la liste flottante directement.
pub fn register(state: &mut Yaor, window: &WindowElement) {
    if crate::config::layout_mode() == LayoutMode::Floating {
        state.layout.floating.push(window.clone());
        tracing::debug!("layout : fenêtre flottante recensée");
        return;
    }
    state.layout.tiled.push(window.clone());
    tracing::debug!("layout : fenêtre recensée ({} tiling)", state.layout.tiled.len());
}

/// Sort une fenêtre du tiling : déplacée/redimensionnée interactivement
/// (grab move/resize), elle devient flottante et conserve sa géométrie
/// après relâchement (le scénario QA `t-yaor-comp-wm`).
pub fn mark_floating(state: &mut Yaor, window: &WindowElement) {
    state.layout.tiled.retain(|w| !same_window(w, window));
    state.layout.floating.retain(|w| !same_window(w, window));
    state.layout.floating.push(window.clone());
    tracing::debug!("layout : fenêtre flottante ({} tiling)", state.layout.tiled.len());
}

/// Minimise/restaure (v0.6 étape 1b — bureau flottant du profil
/// classique).
///
/// Sémantique : fenêtre focusée mappée → minimiser ; sinon (focus
/// libéré, bureau vide cliqué) → restaurer la plus récemment
/// minimisée. Le Mod+m sert donc de bascule minimiser/afficher en
/// pratique, et la taskbar (1b.c) aura la restauration explicite.
///
/// Minimiser : la fenêtre SORT du Space (`unmap_elem`) — plus rendue,
/// plus touchable (hors hit-test), focus transféré au sommet de pile
/// restant (ou libéré). La position est mémorisée pour la
/// restauration. Restaurer : re-map à la position mémorisée, à la
/// tête de pile.
///
/// X11 : la surface Xwayland sort du Space de YAOR (le serveur X la
/// croit toujours mappée — invisible côté YAOR, rendue au restaurer ;
/// c'est le même modèle que le v0.4 pour les grabs X11).
fn minimize_toggle(state: &mut Yaor) {
    let focused = focused_window(state);
    let focused_mapped = focused
        .as_ref()
        .map(|w| state.space.element_geometry(w).is_some())
        .unwrap_or(false);

    // 1) fenêtre focusée minimisée (défensif) → la restaurer.
    if let Some(window) = focused.as_ref() {
        if let Some(idx) = state
            .layout
            .minimized
            .iter()
            .position(|(w, _, _)| same_window(w, window))
        {
            restore_minimized(state, idx);
            return;
        }
    }

    // 2) fenêtre focusée mappée → la minimiser.
    if let (true, Some(window)) = (focused_mapped, focused) {
        minimize_window(state, &window);
        return;
    }

    // 3) pas de focus mappé → restaurer la plus récente minimisée.
    if let Some(idx) = state.layout.minimized.len().checked_sub(1) {
        restore_minimized(state, idx);
    }
}

/// Minimise une fenêtre : sortie du Space, position mémorisée,
/// focus au sommet de pile flottante restant (ou libéré).
/// Public : aussi appelée par le bouton minimiser des décorations
/// serveur (jalon 1b.b).
pub fn minimize_window(state: &mut Yaor, window: &WindowElement) {
    let loc = state
        .space
        .element_geometry(window)
        .map(|g| g.loc)
        .unwrap_or_else(|| Point::from((COLLAPSE_MARGIN, COLLAPSE_MARGIN)));
    state.space.unmap_elem(window);
    state.layout.tiled.retain(|w| !same_window(w, window));
    state.layout.floating.retain(|w| !same_window(w, window));
    state
        .layout
        .minimized
        .push((window.clone(), loc.x, loc.y));
    tracing::info!(
        "raccourci : minimisation ({} minimisée(s))",
        state.layout.minimized.len()
    );

    // focus : au sommet de pile flottante restant (le dernier élément
    // mappé), sinon libéré.
    let next = state
        .layout
        .floating
        .iter()
        .rev()
        .find(|w| state.space.element_geometry(*w).is_some())
        .cloned();
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    let keyboard = state.seat.get_keyboard().unwrap();
    match next {
        Some(w) => {
            let focus = crate::focus::FocusTarget::Wl(
                w.wl_surface()
                    .map(|s| s.into_owned())
                    .expect("fenêtre flottante wayland sans surface"),
            );
            keyboard.set_focus(state, Some(focus), serial);
            crate::element::update_focused_decoration(state, Some(&w));
        }
        None => {
            keyboard.set_focus(state, None::<crate::focus::FocusTarget>, serial);
            crate::element::update_focused_decoration(state, None);
        }
    }
    // publication foreign-toplevel : état mappée → minimized (1b.c).
    crate::toplevels::publish_all(state);
}

/// Restaure la fenêtre minimisée d'indice `idx` : re-map à la
/// position mémorisée, à la tête de pile, focus rendu.
/// Publique (crate) : aussi appelée par la taskbar via le protocole
/// foreign-toplevel (jalon 1b.c).
pub(crate) fn restore_minimized(state: &mut Yaor, idx: usize) {
    let (window, x, y) = state.layout.minimized.remove(idx);
    state.space.map_element(window.clone(), (x, y), true);
    state.layout.floating.push(window.clone());
    state.space.raise_element(&window, true);
    tracing::info!("raccourci : restauration de la fenêtre minimisée");
    if let Some(surface) = window.wl_surface() {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let focus = crate::focus::FocusTarget::Wl(surface.into_owned());
        state
            .seat
            .get_keyboard()
            .unwrap()
            .set_focus(state, Some(focus), serial);
    }
    crate::element::update_focused_decoration(state, Some(&window));
    // publication foreign-toplevel : état minimized → mappée (1b.c).
    crate::toplevels::publish_all(state);
}

// ---------------------------------------------------------------------------
// Espaces de travail (v0.6 étape 3 — profil tiling, maquette 1-5)
// ---------------------------------------------------------------------------

/// Bascule vers l'espace `idx` (base 0). Les tuilées de l'espace quitté
/// sortent du Space (unmap, positions mémorisées — modèle `minimized`),
/// celles de l'espace entrant reviennent et sont re-tuilées par
/// `recompute` (tile_group re-mappe les non-mappées). Focus : maître de
/// l'espace entrant, sinon libéré. Hors bornes ou espace déjà courant :
/// no-op. Flottantes et minimisées : inchangées (hors espaces).
pub fn switch_workspace(state: &mut Yaor, idx: usize) {
    if idx >= WORKSPACE_COUNT || idx == state.layout.current_workspace {
        tracing::info!(
            "espaces : bascule ignorée (espace courant {} ou hors bornes)",
            state.layout.current_workspace + 1
        );
        return;
    }
    let from_ws = state.layout.current_workspace;
    // Sauvegarde de l'espace quitté (positions de tuile pour le retour).
    let mut leaving: Vec<(WindowElement, i32, i32)> = Vec::new();
    let old = std::mem::take(&mut state.layout.tiled);
    for w in old {
        let loc = state
            .space
            .element_geometry(&w)
            .map(|g| (g.loc.x, g.loc.y))
            .unwrap_or((0, 0));
        state.space.unmap_elem(&w);
        leaving.push((w, loc.0, loc.1));
    }
    state.layout.workspaces[from_ws] = leaving;

    // Reprise de l'espace cible : dé-mémorisé vers `tiled` (recompute
    // re-map et re-tuile).
    let from = std::mem::take(&mut state.layout.workspaces[idx]);
    state.layout.current_workspace = idx;
    state.layout.tiled = from.into_iter().map(|(w, _, _)| w).collect();

    tracing::info!(
        "espaces : bascule {} -> {} ({} fenêtre(s) à l'arrivée)",
        from_ws + 1,
        idx + 1,
        state.layout.tiled.len()
    );
    recompute(state);
    // focus APRÈS recompute : les fenêtres entrantes viennent d'être
    // (re)mappées — le modèle éprouvé de restore_minimized (map puis
    // focus). Poser le focus AVANT le re-map le perdait : les requêtes
    // d'envoi partaient en « aucune fenêtre focusée » (leçon QA run 3 :
    // quatre essais Mod+Shift+2 muets, focus non retrouvé par
    // focused_window qui ne parcourt que les fenêtres mappées).
    if !state.layout.tiled.is_empty() {
        focus_tiled(state, 0);
    } else {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        state
            .seat
            .get_keyboard()
            .unwrap()
            .set_focus(state, None::<crate::focus::FocusTarget>, serial);
        crate::element::update_focused_decoration(state, None);
    }
    // publications : fenêtres mappées changées (taskbar) + espace
    // courant (indicateurs panel / yaor-ctl).
    crate::toplevels::publish_all(state);
    crate::workspaces::publish_current(state);
}

/// Envoie la fenêtre focusée à l'espace `idx` puis bascule avec elle
/// (convention sway). Seules les tuilées participent aux espaces — une
/// fenêtre flottante ou minimisée focusée : no-op. Hors bornes ou
/// espace courant : no-op.
pub fn send_focused_to_workspace(state: &mut Yaor, idx: usize) {
    if idx >= WORKSPACE_COUNT || idx == state.layout.current_workspace {
        tracing::info!("espaces : envoi ignoré (espace courant ou hors bornes)");
        return;
    }
    let Some(window) = focused_window(state) else {
        tracing::info!("espaces : envoi ignoré (aucune fenêtre focusée)");
        return;
    };
    if !state.layout.tiled.iter().any(|t| same_window(t, &window)) {
        tracing::info!("espaces : envoi ignoré (fenêtre focusée hors tiling)");
        return;
    }
    let loc = state
        .space
        .element_geometry(&window)
        .map(|g| (g.loc.x, g.loc.y))
        .unwrap_or((0, 0));
    state.space.unmap_elem(&window);
    state.layout.tiled.retain(|w| !same_window(w, &window));
    state.layout.workspaces[idx].push((window, loc.0, loc.1));
    tracing::info!("espaces : envoi de la fenêtre focusée à l'espace {} + bascule", idx + 1);
    recompute(state);
    // focus : maître restant, sinon libéré (avant la bascule qui
    // refocus le maître de l'espace entrant — la fenêtre envoyée).
    if !state.layout.tiled.is_empty() {
        focus_tiled(state, 0);
    } else {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        state
            .seat
            .get_keyboard()
            .unwrap()
            .set_focus(state, None::<crate::focus::FocusTarget>, serial);
        crate::element::update_focused_decoration(state, None);
    }
    crate::toplevels::publish_all(state);
    switch_workspace(state, idx);
}

/// Fusionne tous les espaces dans l'espace courant (départ du
/// maître+pile — changement de profil) : re-map à la dernière position
/// de tuile mémorisée, retour dans `tiled`. Le mode cible (cascade ou
/// flottant) tranche ensuite leur sort (sur place, no-op…). Aucune
/// fenêtre perdue ; les slots redeviennent vides.
fn merge_workspaces(state: &mut Yaor) {
    let mut merged = 0usize;
    for slot in state.layout.workspaces.drain(..) {
        for (w, x, y) in slot {
            state.space.map_element(w.clone(), (x, y), true);
            state.layout.tiled.push(w);
            merged += 1;
        }
    }
    state.layout.workspaces = vec![Vec::new(); WORKSPACE_COUNT];
    if merged > 0 {
        tracing::info!("profil : {merged} fenêtre(s) d'autres espaces revenue(s)");
    }
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
    let mode = crate::config::layout_mode();
    // Permutation de mode (rechargement de configuration) : détectée
    // ICI, avant les branchements, pour que `last_mode` progresse dans
    // TOUS les modes (aller-retour tiling -> flottant -> tiling compris).
    let switched = state.layout.last_mode != Some(mode);
    state.layout.last_mode = Some(mode);
    if mode == LayoutMode::Cascade {
        // v0.4 exact : no-op strict (aucune régression permise).
        // Purge des décorations si le mode vient du flottant
        // (rechargement) : set_ssd(false) est un no-op sinon.
        // Permutation DEPUIS le maître+pile : fusion des espaces de
        // travail (aucune fenêtre ne reste cachée — re-map à la
        // dernière position de tuile mémorisée).
        if switched {
            merge_workspaces(state);
        }
        state.space.elements().for_each(|w| w.set_ssd(false));
        return;
    }
    if mode == LayoutMode::Floating {
        // Flottant (v0.6 étape 1b) : aucune disposition automatique —
        // les positions appartiennent aux grabs et à l'utilisateur.
        // Purge des morts ; si le mode vient d'être permuté depuis le
        // tiling (rechargement), les fenêtres tuilées sont libérées
        // SUR PLACE (géométrie conservée). Jalon 1b.b : tout élément
        // flottant porte ses décorations serveur.
        state.space.refresh();
        state.layout.tiled.retain(IsAlive::alive);
        state.layout.floating.retain(IsAlive::alive);
        state.layout.minimized.retain(|(w, _, _)| w.alive());
        // Permutation depuis le tiling : les fenêtres des autres
        // espaces reviennent AVANT la libération (elles sont re-mappées
        // à leur position mémorisée, puis libérées sur place comme les
        // autres tuilées).
        if switched {
            merge_workspaces(state);
        }
        let freed = state.layout.tiled.drain(..).collect::<Vec<_>>();
        if !freed.is_empty() {
            tracing::info!(
                "layout flottant : {} fenêtre(s) libérée(s) du tiling sur place",
                freed.len()
            );
            state.layout.floating.extend(freed);
        }
        state.space.elements().for_each(|w| w.set_ssd(true));
        return;
    }

    // purge des fenêtres mortes (aligné sur Space::refresh)
    state.space.refresh();
    state.layout.tiled.retain(IsAlive::alive);
    state.layout.floating.retain(IsAlive::alive);
    // les slots d'espaces peuvent contenir des fenêtres mortes (client
    // fermé pendant qu'il vivait hors espace courant).
    for slot in &mut state.layout.workspaces {
        slot.retain(|(w, _, _)| w.alive());
    }
    state.space.elements().for_each(|w| w.set_ssd(false));

    // À l'entrée en maître+pile (rechargement) : les flottantes
    // rejoignent le tiling — symétrique du branchement flottant
    // ci-dessus (qui libère les tuilées sur place). Les sorties de
    // tiling interactives (grab, v0.6 1b) ne sont pas touchées : elles
    // vivent SANS permutation, `switched` reste faux.
    if mode == LayoutMode::MasterStack && switched && !state.layout.floating.is_empty() {
        tracing::info!(
            "profil maître+pile : {} fenêtre(s) flottante(s) adoptée(s) au tiling",
            state.layout.floating.len()
        );
        state.layout.tiled.append(&mut state.layout.floating);
    }

    // adoption : toute fenêtre non recensée rejoint le tiling (mode
    // permuté après coup au rechargement de configuration)
    for w in state.space.elements().cloned().collect::<Vec<_>>() {
        if !state.layout.tiled.iter().any(|t| same_window(t, &w))
            && !state.layout.floating.iter().any(|f| same_window(f, &w))
        {
            state.layout.tiled.push(w);
        }
    }

    let tiled = state.layout.tiled.clone();
    let outputs = outputs_of(state);

    for (output, geo) in &outputs {
        let group: Vec<WindowElement> = tiled
            .iter()
            .filter(|w| window_output_of(state, w, &outputs).as_ref() == Some(output))
            .cloned()
            .collect();
        tile_group(state, *geo, &group);
    }
}

/// Sorties connues (géométrie globale) : côte à côte dans l'espace.
/// v0.6 étape 1 : la géométrie est la **zone utile** de la sortie —
/// `layer_map_for_output(o).non_exclusive_zone()` — c.-à-d. la sortie
/// entière moins les bandes exclusives des panneaux layer-shell
/// (barre des tâches…). Sans layer mappé, c'est la sortie entière
/// (comportement v0.4/v0.5 inchangé, garde de régression).
fn outputs_of(state: &mut Yaor) -> Vec<(Output, Rectangle<i32, Logical>)> {
    state
        .space
        .outputs()
        .filter_map(|o| {
            let geo = state.space.output_geometry(o)?;
            let map = smithay::desktop::layer_map_for_output(o);
            let zone = map.non_exclusive_zone();
            Some((o.clone(), Rectangle::new(geo.loc + zone.loc, zone.size)))
        })
        .collect()
}

/// Sortie à laquelle une fenêtre est rattachée : contient le centre de
/// sa géométrie ; sinon (client pas encore commité) la première sortie.
fn window_output_of(
    state: &Yaor,
    window: &WindowElement,
    outputs: &[(Output, Rectangle<i32, Logical>)],
) -> Option<Output> {
    // Fenêtre hors Space (fenêtre entrante d'un autre espace de
    // travail, pas encore remappée par tile_group) : première sortie —
    // le `?` direct rendait ce repli INATTEIGNABLE (leçon QA
    // t-yaor-espaces run 3/4 : fenêtres « arrivées » jamais re-mappées,
    // invisibles et introuvables pour le focus).
    let geo = match state.space.element_geometry(window) {
        Some(geo) => geo,
        None => return outputs.first().map(|(o, _)| o.clone()),
    };
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
fn tile_group(state: &mut Yaor, geo: Rectangle<i32, Logical>, windows: &[WindowElement]) {
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
    /// Minimiser/restaurer la fenêtre focusée (v0.6 étape 1b).
    Minimize,
    /// Bascule vers l'espace de travail N (v0.6 étape 3 — tiling ;
    /// base 0, affiché N+1).
    Workspace(usize),
    /// Envoie la fenêtre focusée à l'espace N puis bascule avec elle
    /// (convention sway — v0.6 étape 3, tiling).
    WorkspaceSend(usize),
}

/// Identifie l'action déclenchée par la touche pressée (raccourcis
/// rebindables — REQ-SW-015). Aucune correspondance : None.
///
/// Le sym recherché : sym sans modification d'abord (Shift+lettre →
/// lettre, comme sway), sinon sym modifié (en repli).
pub fn find_action(mods: &ModifiersState, raw: Keysym, modified: Keysym) -> Option<Action> {
    let cfg = crate::config::get();
    let mut candidates: Vec<(&str, Action)> = vec![
        (&cfg.keys.focus_next, Action::FocusNext),
        (&cfg.keys.focus_prev, Action::FocusPrev),
        (&cfg.keys.swap_next, Action::SwapNext),
        (&cfg.keys.swap_prev, Action::SwapPrev),
        (&cfg.keys.master_swap, Action::SwapMaster),
        (&cfg.keys.close, Action::Close),
        (&cfg.keys.ratio_grow, Action::RatioGrow),
        (&cfg.keys.ratio_shrink, Action::RatioShrink),
        (&cfg.keys.quit, Action::Quit),
        (&cfg.keys.minimize, Action::Minimize),
    ];
    // Espaces de travail (étape 3) : bascule Mod+N, envoi Mod+Shift+N.
    // Indice hors bornes (config plus longue que WORKSPACE_COUNT) :
    // l'action est un no-op côté run_action.
    for (i, spec) in cfg.keys.workspace.iter().enumerate() {
        candidates.push((spec, Action::Workspace(i)));
    }
    for (i, spec) in cfg.keys.workspace_send.iter().enumerate() {
        candidates.push((spec, Action::WorkspaceSend(i)));
    }
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
    let mode = crate::config::layout_mode();
    let tiling_only = matches!(
        action,
        Action::FocusNext
            | Action::FocusPrev
            | Action::SwapNext
            | Action::SwapPrev
            | Action::SwapMaster
            | Action::RatioGrow
            | Action::RatioShrink
            | Action::Workspace(_)
            | Action::WorkspaceSend(_)
    );
    if mode != LayoutMode::MasterStack && tiling_only {
        // actions de tiling : sans objet hors maître+pile (v0.4
        // inchangé — aucune régression permise sur les QA de cette
        // zone). Close/Quit/Minimize sont des actions de bureau
        // générales.
        return;
    }
    if mode == LayoutMode::MasterStack && action == Action::Minimize {
        // minimiser une tuile fausserait le tiling (elle serait
        // ré-adoptée au prochain re-tile) — sans objet en v0.6 ; le
        // minimiser appartient au bureau flottant (étape 1b).
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
        Action::Minimize => minimize_toggle(state),
        Action::Workspace(idx) => switch_workspace(state, idx),
        Action::WorkspaceSend(idx) => send_focused_to_workspace(state, idx),
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
                    let outputs = outputs_of(state);
                    let master = state.layout.tiled[0].clone();
                    let focused = state.layout.tiled[idx].clone();
                    let master_out = window_output_of(state, &master, &outputs);
                    let focused_out = window_output_of(state, &focused, &outputs);
                    if master_out == focused_out {
                        // même sortie : l'échange d'ordre suffit (le re-tile
                        // regroupe par sortie et le maître change de fenêtre)
                        state.layout.tiled.swap(0, idx);
                        recompute(state);
                        tracing::info!("raccourci : tuile {idx} ↔ maître");
                    } else {
                        // trans-sortie (2 têtes, v0.5 étape 2) : les fenêtres
                        // échangent leurs positions — la fenêtre focusée
                        // rejoint la sortie du maître (et le maître
                        // réciproquement la sienne) ; le re-tile regroupe
                        // par centre et dispose chaque sortie ; le maître
                        // de CHAQUE sortie suit l'ordre de la liste échangée.
                        let gm = state.space.element_geometry(&master);
                        let gf = state.space.element_geometry(&focused);
                        if let (Some(gm), Some(gf)) = (gm, gf) {
                            state.space.map_element(master.clone(), gf.loc, false);
                            state.space.map_element(focused.clone(), gm.loc, false);
                            state.layout.tiled.swap(0, idx);
                            recompute(state);
                            tracing::info!(
                                "raccourci : maître trans-sortie — les fenêtres échangent de sortie"
                            );
                        }
                    }
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
    crate::element::update_focused_decoration(state, Some(&window));
}

/// Fenêtre focusée (n'importe laquelle), conversion FocusTarget → élément.
fn focused_window(state: &Yaor) -> Option<WindowElement> {
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
pub(crate) fn same_window(a: &WindowElement, b: &WindowElement) -> bool {
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
