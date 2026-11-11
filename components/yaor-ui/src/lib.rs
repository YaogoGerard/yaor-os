//! Primitives de rendu des clients du bureau YAOR (jalon v0.6 « Caméléon »,
//! étape 2).
//!
//! Extrait de `yaor-panel` au moment où un second client (`yaor-dock`) a eu
//! besoin des mêmes formes : un seul fournisseur pour le canvas Argb8888, les
//! rectangles et pilules anti-aliasés, la police bitmap 5x7 et la palette
//! `[theme]` de `desktop.toml`.
//!
//! Rien ici ne touche à Wayland : ce sont des fonctions pures sur des
//! tranches d'octets, donc testables sans session ni compositor. Les clients
//! gardent leur propre boucle d'événements sctk et ne partagent que le
//! dessin — c'est ce qui permet d'ajouter une surface (dock, panneau,
//! widget) sans recompiler le compositor (ADR-018, décision v0.6).

pub mod font;
pub mod glass;
pub mod raster;
pub mod text;
pub mod theme;

pub use font::{draw_text, draw_text_scaled, glyph, text_width, GLYPHS};
pub use text::{
    draw_text as draw_text_true, draw_text_baseline, text_ascent as text_ascent_true,
    text_height as text_height_true, text_width as text_width_true, Weight as TextWeight,
};
pub use raster::{
    blend, blit, fill_circle, fill_pill, fill_rect, fill_rrect, fill_rrect_premul, fill_ring,
    pill_cov, put, rrect_cov,
};
pub use glass::{Backdrop, GlassKit, GlassParams, GlassState, Rect, WallLoader};
pub use theme::{dim, Theme};
