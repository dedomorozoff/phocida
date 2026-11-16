pub mod canvas;
pub mod designer_view;
pub mod inspector;
pub mod lfm;
pub mod model;
pub mod palette;

#[cfg(test)]
mod canvas_tests;

pub use canvas::FormCanvas;
pub use designer_view::{
    DesignerView, DesignerViewEvent, form_candidates, open_designer_for_active_item, uses_units,
};
pub use inspector::{set_property, unset_property};
pub use lfm::{LfmComponent, LfmDocument, parse_lfm, serialize_lfm};
pub use model::{DesignerComponent, components_of};
pub use palette::{ComponentDescriptor, palette_components};
