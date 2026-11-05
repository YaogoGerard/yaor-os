// Éléments de rendu et curseur du compositor YAOR (jalon v0.4, étape 2).
//
// Adapté de anvil/src/drawing.rs et anvil/src/render.rs (licence MIT ©
// Smithay contributors) : PointerElement local (pas de xcursor — un seul
// curseur mémoire statique), pas de preview plein-écran, pas de DnD, pas
// d'élément FPS. Le rendu multi-GPU reste possible (MultiRenderer), mais
// le chemin nominal est un seul GPU (QEMU virtio-gpu).

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                surface::render_elements_from_surface_tree,
                AsRenderElements, Kind, Wrap,
            },
            gles::GlesRenderer,
            multigpu::{gbm::GbmGlesBackend, MultiRenderer},
            ImportAll, ImportMem, Renderer, Texture,
        },
    },
    desktop::space::{Space, SpaceRenderElements},
    output::Output,
    utils::{Buffer, Point, Scale, Size, Transform},
};

// Ré-exportations utilisées par les modules voisins (state, session).
pub use smithay::{backend::drm::DrmDeviceFd, backend::renderer::element::surface::WaylandSurfaceRenderElement, input::pointer::CursorImageStatus};

pub const CLEAR_COLOR: [f32; 4] = [0.8, 0.8, 0.9, 1.0];

/// Rendu de l'arbre smithay udev : MultiRenderer (anvil UdevRenderer).
/// Le backend cible et de rendu sont le même GbmGlesBackend — un seul
/// GPU en QA (QEMU), le multi-GPU reste possible par le même type.
pub type UdevRenderer<'a> = MultiRenderer<
    'a,
    'a,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
>;

/// Type concret d'élément rendu d'une sortie udev (assemblage du
/// generic OutputRenderElements avec le rendu udev et l'élément
/// fenêtre ENVELOPPÉ — surface + décorations serveur, jalon 1b.b).
pub type UdevOutputElements<'a> = OutputRenderElements<
    UdevRenderer<'a>,
    crate::element::WindowRenderElement<UdevRenderer<'a>>,
>;

// Élément de rendu du pointeur (mémoire ou surface cliente — modélisé
// comme anvil PointerRenderElement).
smithay::backend::renderer::element::render_elements! {
    pub PointerRenderElement<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
}

// Élément de sortie : curseur + surfaces clientes.
smithay::backend::renderer::element::render_elements! {
    pub CustomRenderElements<R> where R: ImportAll + ImportMem;
    Pointer=PointerRenderElement<R>,
    Surface=WaylandSurfaceRenderElement<R>,
}

smithay::backend::renderer::element::render_elements! {
    pub OutputRenderElements<R, E> where R: ImportAll + ImportMem;
    Space=SpaceRenderElements<R, E>,
    Window=Wrap<E>,
    Custom=CustomRenderElements<R>,
}

/// Curseur : buffer mémoire (statique, « flèche » minimal via
/// MemoryRenderBuffer — pas de xcursor).
pub struct CursorImage;

impl CursorImage {
    /// Curseur « flèche » dessiné en mémoire (pas de xcursor) :
    /// silhouette classique complète (pointe en haut-gauche = hotspot,
    /// encoche de queue), remplissage blanc, contour noir, bords
    /// anti-aliasés (supersampling 3x3) — lisible sur fond clair du
    /// bureau comme sur les fenêtres sombres. 24x24, détectable sur
    /// screendump (QA).
    pub fn arrow_static() -> MemoryRenderBuffer {
        const W: i32 = 24;
        const H: i32 = 24;
        /// Silhouette (sens horaire depuis la pointe) — échelle 24 px.
        const POLY: [(f64, f64); 7] = [
            (0.0, 0.0),   // pointe (hotspot)
            (0.0, 17.0),  // bord gauche descendant
            (4.5, 13.0),  // encoche : remontée
            (7.5, 20.5),  // queue : descend
            (10.5, 19.0), // queue : base
            (7.5, 12.0),  // queue : remonte
            (13.0, 12.0), // vers la diagonale
        ];

        // Point dans le polygone (ray casting) — pointe en (0,0) :
        // les échantillons exactement sur les arêtes gauche/haut
        // comptent dedans (le contour noir les couvre).
        fn inside(px: f64, py: f64) -> bool {
            let mut hit = false;
            let mut i = 0;
            let mut j = POLY.len() - 1;
            while i < POLY.len() {
                let (xi, yi) = POLY[i];
                let (xj, yj) = POLY[j];
                if ((yi > py) != (yj > py))
                    && (px < (xj - xi) * (py - yi) / (yj - yi) + xi)
                {
                    hit = !hit;
                }
                j = i;
                i += 1;
            }
            hit
        }

        // Argb8888 : remplissage blanc opaque, contour noir opaque,
        // bords adoucis par moyenne de sous-échantillons.
        let mut px = vec![0u8; (4 * (W * H)) as usize];
        const SS: i32 = 3;
        for y in 0..H {
            for x in 0..W {
                let i = (4 * (y * W + x)) as usize;
                let mut inside_n = 0;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let sxp = x as f64 + (sx as f64 + 0.5) / SS as f64;
                        let syp = y as f64 + (sy as f64 + 0.5) / SS as f64;
                        if inside(sxp, syp) {
                            inside_n += 1;
                        }
                    }
                }
                let total = SS * SS;
                if inside_n == 0 || total == 0 {
                    continue; // transparent
                }
                // Bord = au moins un sous-échantillon dehors (à
                // l'intérieur du polygone strict, élargi d'1 px pour
                // tracer le contour tout autour).
                let mut edge_n = 0;
                'outer: for dy in -1..=1 {
                    for dx in -1..=1 {
                        let sxp = x as f64 + 0.5 + dx as f64;
                        let syp = y as f64 + 0.5 + dy as f64;
                        if !inside(sxp, syp) {
                            edge_n += 1;
                            break 'outer;
                        }
                    }
                }
                let (r, g, b, a) = if edge_n > 0 {
                    (0x00, 0x00, 0x00, 0xFF) // contour noir
                } else {
                    (0xFF, 0xFF, 0xFF, 0xFF) // corps blanc
                };
                px[i] = b;
                px[i + 1] = g;
                px[i + 2] = r;
                px[i + 3] = a;
            }
        }
        MemoryRenderBuffer::from_slice(
            &px,
            Fourcc::Argb8888,
            (W, H),
            1,
            Transform::Normal,
            None,
        )
    }
}

/// Porteur de l'état de rendu du pointeur (modèle anvil PointerElement).
pub struct PointerElement {
    buffer: Option<MemoryRenderBuffer>,
    status: CursorImageStatus,
}

impl Default for PointerElement {
    fn default() -> Self {
        PointerElement {
            buffer: Default::default(),
            status: CursorImageStatus::default_named(),
        }
    }
}

impl PointerElement {
    pub fn set_status(&mut self, status: CursorImageStatus) {
        self.status = status;
    }

    pub fn set_buffer(&mut self, buffer: MemoryRenderBuffer) {
        self.buffer = Some(buffer);
    }
}

impl<R> AsRenderElements<R> for PointerElement
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Texture + Clone + Send + 'static,
{
    type RenderElement = PointerRenderElement<R>;

    fn render_elements<E>(
        &self,
        renderer: &mut R,
        location: Point<i32, smithay::utils::Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<E>
    where
        E: From<PointerRenderElement<R>>,
    {
        match &self.status {
            CursorImageStatus::Hidden => vec![],
            CursorImageStatus::Named(_) => {
                if let Some(buffer) = self.buffer.as_ref() {
                    vec![PointerRenderElement::<R>::from(
                        MemoryRenderBufferRenderElement::from_buffer(
                            renderer,
                            location.to_f64(),
                            buffer,
                            None,
                            None,
                            None,
                            Kind::Cursor,
                        )
                        .expect("perte du buffer curseur système"),
                    )
                    .into()]
                } else {
                    vec![]
                }
            }
            CursorImageStatus::Surface(surface) => {
                let elements: Vec<PointerRenderElement<R>> =
                    render_elements_from_surface_tree(
                        renderer,
                        surface,
                        location,
                        scale,
                        alpha,
                        Kind::Cursor,
                    );
                elements.into_iter().map(E::from).collect()
            }
        }
    }
}

// Hotspot d'un curseur surface (lecture des attributs, comme anvil).
/// Fond d'écran (1d.fond) : rendu procédural en mémoire — dégradé
/// sombre chaud + lueur emblème ambre à droite avec silhouette de
/// flamme (façon `design/Classique.png`). Aucun asset (ADR-018) :
/// tout est calculé, buffer réutilisé tant que la taille ne change pas.
fn wallpaper_buffer(size: Size<i32, Buffer>) -> MemoryRenderBuffer {
    static CACHE: std::sync::Mutex<Option<(Size<i32, Buffer>, MemoryRenderBuffer)>> =
        std::sync::Mutex::new(None);
    if let Some((s, buf)) = CACHE.lock().unwrap().as_ref() {
        if *s == size {
            return buf.clone();
        }
    }

    let (w, h) = (size.w, size.h);
    let mut px = vec![0u8; (4 * (w * h)) as usize];

    // Dégradé vertical : gris bleuté sombre -> brun chaud.
    let top = [0.055, 0.055, 0.075_f64];
    let bot = [0.098, 0.082, 0.063_f64];

    // Lueur emblème : centre à droite (78 % largeur, 42 % hauteur),
    // rayon 45 % de la largeur, ambre atténué quadratiquement.
    let (cx, cy) = (0.78 * w as f64, 0.42 * h as f64);
    let radius = 0.45 * w as f64;
    let amber = [0.9098, 0.5255, 0.1804_f64];

    // Silhouette de flamme : triangle doux (pointe haute, base large),
    //Distance minimale aux segments pour la plume du bord.
    let tri = [
        (cx, 0.16 * h as f64),
        (cx - 0.16 * w as f64, 0.88 * h as f64),
        (cx + 0.16 * w as f64, 0.88 * h as f64),
    ];
    fn seg_dist(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
        let (ax, ay) = a;
        let (bx, by) = b;
        let (vx, vy) = (bx - ax, by - ay);
        let t = (((px - ax) * vx + (py - ay) * vy)
            / (vx * vx + vy * vy))
            .clamp(0.0, 1.0);
        let (dx, dy) = (px - (ax + t * vx), py - (ay + t * vy));
        (dx * dx + dy * dy).sqrt()
    }

    for y in 0..h {
        let ty = y as f64 / (h - 1).max(1) as f64;
        for x in 0..w {
            let i = (4 * (y * w + x)) as usize;
            let mut r = top[0] + (bot[0] - top[0]) * ty;
            let mut g = top[1] + (bot[1] - top[1]) * ty;
            let mut b = top[2] + (bot[2] - top[2]) * ty;

            // Lueur ambre.
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt() / radius;
            if d < 1.0 {
                let a = 0.26 * (1.0 - d) * (1.0 - d);
                r += amber[0] * a;
                g += amber[1] * a;
                b += amber[2] * a;
            }

            // Flamme : silhouette SOMBRE sur la lueur (façon mockup),
            // bord adouci sur 18 px — assombrit jusqu'à 55 %.
            let fx = x as f64;
            let fy = y as f64 + 0.5;
            let d_edge = seg_dist(fx, fy, tri[0], tri[1])
                .min(seg_dist(fx, fy, tri[1], tri[2]))
                .min(seg_dist(fx, fy, tri[2], tri[0]));
            // Dedans si le point est du même côté des trois arêtes.
            let mut inside = true;
            for k in 0..3 {
                let (ax, ay) = tri[k];
                let (bx, by) = tri[(k + 1) % 3];
                let (cxx, cyy) = tri[(k + 2) % 3];
                let side_p = (by - ay) * (fx - ax) - (bx - ax) * (fy - ay);
                let side_c = (by - ay) * (cxx - ax) - (bx - ax) * (cyy - ay);
                if side_p * side_c <= 0.0 {
                    inside = false;
                    break;
                }
            }
            let s = if !inside {
                0.0
            } else if d_edge < 18.0 {
                d_edge / 18.0
            } else {
                1.0
            };
            let dark = 0.55 * s;
            r *= 1.0 - dark;
            g *= 1.0 - dark;
            b *= 1.0 - dark;

            px[i] = (b * 255.0).round() as u8;
            px[i + 1] = (g * 255.0).round() as u8;
            px[i + 2] = (r * 255.0).round() as u8;
            px[i + 3] = 0xFF;
        }
    }

    let buf = MemoryRenderBuffer::from_slice(
        &px,
        Fourcc::Argb8888,
        size,
        1,
        Transform::Normal,
        None,
    );
    *CACHE.lock().unwrap() = Some((size, buf.clone()));
    buf
}

/// Assemble les éléments de rendu d'une sortie : éléments personnalisés
/// (curseur), puis layers Background/Bottom, fenêtres de l'espace, puis
/// layers Top/Overlay (panneaux au-dessus — modèle anvil output_elements,
/// sans fullscreen ni preview). v0.6 étape 1 : rendu layer-shell.
/// 1d.fond : le fond d'écran est poussé EN DERNIER (l'ordre du vec est
/// « premier = dessus » — anvil rend son curseur en tête).
pub fn output_elements<R>(
    output: &Output,
    space: &Space<crate::element::WindowElement>,
    custom_elements: Vec<CustomRenderElements<R>>,
    renderer: &mut R,
) -> (
    Vec<OutputRenderElements<R, crate::element::WindowRenderElement<R>>>,
    [f32; 4],
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Texture + Clone + Send + 'static,
{
    let mut out: Vec<OutputRenderElements<R, crate::element::WindowRenderElement<R>>> =
        custom_elements
            .into_iter()
            .map(OutputRenderElements::Custom)
            .collect();

    let scale = Scale::from(output.current_scale().fractional_scale());
    let scale_i = Scale::from(scale.x.round() as i32);

    // couches basses (Background/Bottom) : sous les fenêtres
    {
        let map = smithay::desktop::layer_map_for_output(output);
        for kind in [
            smithay::wayland::shell::wlr_layer::Layer::Background,
            smithay::wayland::shell::wlr_layer::Layer::Bottom,
        ] {
            for layer in map.layers_on(kind) {
                let loc = map
                    .layer_geometry(layer)
                    .map(|g| g.loc)
                    .unwrap_or_default()
                    .to_physical(scale_i);
                out.extend(
                    layer
                        .render_elements::<crate::element::WindowRenderElement<R>>(
                            renderer, loc, scale, 1.0,
                        )
                        .into_iter()
                        .map(|e| OutputRenderElements::Window(Wrap::from(e))),
                );
            }
        }
    }

    if let Ok(space_elements) =
        smithay::desktop::space::space_render_elements(renderer, [space], output, 1.0)
    {
        out.extend(space_elements.into_iter().map(OutputRenderElements::Space));
    }

    // couches hautes (Top/Overlay) : panneaux au-dessus des fenêtres
    {
        let map = smithay::desktop::layer_map_for_output(output);
        for kind in [
            smithay::wayland::shell::wlr_layer::Layer::Top,
            smithay::wayland::shell::wlr_layer::Layer::Overlay,
        ] {
            for layer in map.layers_on(kind) {
                let loc = map
                    .layer_geometry(layer)
                    .map(|g| g.loc)
                    .unwrap_or_default()
                    .to_physical(scale_i);
                out.extend(
                    layer
                        .render_elements::<crate::element::WindowRenderElement<R>>(
                            renderer, loc, scale, 1.0,
                        )
                        .into_iter()
                        .map(|e| OutputRenderElements::Window(Wrap::from(e))),
                );
            }
        }
    }

    // fond d'écran (1d.fond) : DERNIER élément = dessous de tout.
    {
        let phys = output
            .current_mode()
            .map(|m| smithay::output::Mode { size: m.size, refresh: m.refresh })
            .unwrap_or_else(|| smithay::output::Mode { size: (1280, 800).into(), refresh: 60_000 });
        let buf = wallpaper_buffer(Size::<i32, Buffer>::from((phys.size.w, phys.size.h)));
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::<f64, smithay::utils::Physical>::from((0.0, 0.0)),
            &buf,
            None,
            None,
            None,
            smithay::backend::renderer::element::Kind::Unspecified,
        ) {
            let elem: crate::element::WindowRenderElement<R> = elem.into();
            out.push(OutputRenderElements::Window(Wrap::from(elem)));
        }
    }

    (out, CLEAR_COLOR)
}
