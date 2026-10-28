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
    utils::{Point, Scale, Transform},
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
/// generic OutputRenderElements avec le rendu udev et la surface
/// fenêtre nue).
pub type UdevOutputElements<'a> =
    OutputRenderElements<UdevRenderer<'a>, WaylandSurfaceRenderElement<UdevRenderer<'a>>>;

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
    /// Curseur « flèche » minimal dessiné en mémoire (pas de xcursor) :
    /// détectable sur screendump malgré son minimalisme.
    pub fn arrow_static() -> MemoryRenderBuffer {
        const W: i32 = 16;
        const H: i32 = 16;
        // Argb8888 : contorno noir opaque sur fond transparent.
        let mut px = vec![0u8; (4 * (W * H)) as usize];
        for y in 0..H {
            for x in 0..W {
                let i = (4 * (y * W + x)) as usize;
                let inside = x < (2 + y / 2) && y < (H - 2);
                let border = x == 0 || x == (1 + y / 2) || y == 0;
                if inside && border {
                    px[i] = 0x00;
                    px[i + 1] = 0x00;
                    px[i + 2] = 0x00;
                    px[i + 3] = 0xFF;
                }
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
/// Assemble les éléments de rendu d'une sortie : éléments personnalisés
/// (curseur), puis layers Background/Bottom, fenêtres de l'espace, puis
/// layers Top/Overlay (panneaux au-dessus — modèle anvil output_elements,
/// sans fullscreen ni preview). v0.6 étape 1 : rendu layer-shell.
pub fn output_elements<R>(
    output: &Output,
    space: &Space<smithay::desktop::Window>,
    custom_elements: Vec<CustomRenderElements<R>>,
    renderer: &mut R,
) -> (
    Vec<OutputRenderElements<R, WaylandSurfaceRenderElement<R>>>,
    [f32; 4],
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Texture + Clone + 'static,
{
    let mut out: Vec<OutputRenderElements<R, WaylandSurfaceRenderElement<R>>> =
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
                        .render_elements(renderer, loc, scale, 1.0)
                        .into_iter()
                        .map(OutputRenderElements::Window),
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
                        .render_elements(renderer, loc, scale, 1.0)
                        .into_iter()
                        .map(OutputRenderElements::Window),
                );
            }
        }
    }

    (out, CLEAR_COLOR)
}
