//! Component palette: the LCL classes a user can drop onto the form, and the
//! placement logic that turns a dropped descriptor into a component.

use std::collections::HashMap;

use crate::canvas::FormCanvas;
use crate::lfm::Bounds;
use crate::model::DesignerComponent;

/// A palette entry: one placeable LCL class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentDescriptor {
    /// LCL class name, e.g. `TButton`.
    pub class: &'static str,
    /// Human-readable palette label, e.g. `Button`.
    pub label: &'static str,
}

/// The starter palette of LCL classes.
pub fn palette_components() -> Vec<ComponentDescriptor> {
    vec![
        ComponentDescriptor {
            class: "TButton",
            label: "Button",
        },
        ComponentDescriptor {
            class: "TLabel",
            label: "Label",
        },
        ComponentDescriptor {
            class: "TEdit",
            label: "Edit",
        },
        ComponentDescriptor {
            class: "TListBox",
            label: "ListBox",
        },
        ComponentDescriptor {
            class: "TPanel",
            label: "Panel",
        },
    ]
}

/// Default bounds in LFM units for a freshly dropped component.
pub const DEFAULT_BOUNDS: Bounds = (0.0, 0.0, 80.0, 25.0);

/// Builds the component a drop creates: instance name derived from the class
/// (`TButton` -> `Button1`), deduplicated against `existing_names`, at the
/// default bounds with no properties.
pub fn drop_component(
    descriptor: &ComponentDescriptor,
    existing_names: impl Fn(&str) -> bool + Clone,
) -> DesignerComponent {
    let base = class_short_name(descriptor.class);
    let mut counter = 1;
    loop {
        let name = format!("{base}{counter}");
        if !existing_names(&name) {
            return DesignerComponent {
                name,
                class: descriptor.class.to_owned(),
                bounds: DEFAULT_BOUNDS,
                properties: HashMap::default(),
            };
        }
        counter += 1;
    }
}

/// Places a dropped palette entry on the canvas: appends the new component at
/// `bounds`, selects it, and returns its index.
pub fn place_component(
    canvas: &mut FormCanvas,
    descriptor: &ComponentDescriptor,
    bounds: Bounds,
) -> usize {
    let mut component = drop_component(descriptor, |name| {
        canvas.components.iter().any(|c| c.name == name)
    });
    component.bounds = bounds;
    crate::canvas::sync_bounds_to_properties(&mut component);
    canvas.components.push(component);
    let index = canvas.components.len() - 1;
    canvas.select(Some(index));
    index
}

/// `TButton` -> `Button`; a class without the `T` prefix stays as-is.
fn class_short_name(class: &str) -> &str {
    class
        .strip_prefix('T')
        .filter(|rest| !rest.is_empty())
        .unwrap_or(class)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_names(_: &str) -> bool {
        false
    }

    #[test]
    fn palette_offers_the_starter_components() {
        let palette = palette_components();
        let classes: Vec<&str> = palette.iter().map(|d| d.class).collect();
        assert_eq!(
            classes,
            vec!["TButton", "TLabel", "TEdit", "TListBox", "TPanel"]
        );
        assert!(palette.iter().all(|d| !d.label.is_empty()));
    }

    #[test]
    fn drop_derives_a_default_component() {
        let descriptor = ComponentDescriptor {
            class: "TButton",
            label: "Button",
        };
        let component = drop_component(&descriptor, no_names);

        assert_eq!(component.name, "Button1");
        assert_eq!(component.class, "TButton");
        assert_eq!(component.bounds, DEFAULT_BOUNDS);
        assert!(component.properties.is_empty());
    }

    #[test]
    fn drop_deduplicates_against_existing_names() {
        let descriptor = ComponentDescriptor {
            class: "TButton",
            label: "Button",
        };
        let taken = |name: &str| name == "Button1";

        assert_eq!(drop_component(&descriptor, taken).name, "Button2");
    }

    #[test]
    fn place_appends_selects_and_reports_the_index() {
        let descriptor = ComponentDescriptor {
            class: "TButton",
            label: "Button",
        };
        let mut canvas = FormCanvas::new(Vec::new(), (0.0, 0.0, 320.0, 240.0));

        let first = place_component(&mut canvas, &descriptor, (10.0, 20.0, 80.0, 25.0));
        assert_eq!(first, 0);
        assert_eq!(canvas.selection, Some(0));
        assert_eq!(canvas.components[0].bounds, (10.0, 20.0, 80.0, 25.0));

        let second = place_component(&mut canvas, &descriptor, (30.0, 40.0, 80.0, 25.0));
        assert_eq!(second, 1);
        assert_eq!(canvas.selection, Some(1));
        assert_eq!(canvas.components[1].name, "Button2");
    }

    #[gpui::test]
    fn placed_component_renders_on_the_canvas(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;

        let descriptor = ComponentDescriptor {
            class: "TButton",
            label: "Button",
        };
        let (view, cx) = cx.add_window_view(|_, _| {
            let mut canvas = FormCanvas::new(Vec::new(), (0.0, 0.0, 320.0, 240.0));
            place_component(&mut canvas, &descriptor, (10.0, 20.0, 80.0, 25.0));
            canvas
        });
        cx.run_until_parked();

        let bounds = cx
            .debug_bounds("designer-component-0")
            .expect("placed component rendered");
        assert_eq!(bounds.size.width.to_f64(), 80.0);
        assert_eq!(bounds.size.height.to_f64(), 25.0);
        let _ = view;
    }
}
