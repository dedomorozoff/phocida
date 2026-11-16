use std::collections::HashMap;

use gpui::TestAppContext;

use crate::canvas::{FormCanvas, component_pixel_rect};
use crate::model::DesignerComponent;

fn button_component() -> DesignerComponent {
    DesignerComponent {
        name: "Button1".to_owned(),
        class: "TButton".to_owned(),
        bounds: (10.0, 20.0, 100.0, 30.0),
        properties: HashMap::default(),
    }
}

fn canvas() -> FormCanvas {
    FormCanvas::new(vec![button_component()], (0.0, 0.0, 320.0, 240.0))
}

#[test]
fn component_rect_maps_lfm_bounds_to_pixels() {
    let rect = component_pixel_rect((10.0, 20.0, 100.0, 30.0), 1.0, (0.0, 0.0));
    assert_eq!(rect.left, 10.0);
    assert_eq!(rect.top, 20.0);
    assert_eq!(rect.width, 100.0);
    assert_eq!(rect.height, 30.0);

    let zoomed = component_pixel_rect((10.0, 20.0, 100.0, 30.0), 2.0, (5.0, 7.0));
    assert_eq!(zoomed.left, 25.0);
    assert_eq!(zoomed.top, 47.0);
    assert_eq!(zoomed.width, 200.0);
    assert_eq!(zoomed.height, 60.0);
}

#[test]
fn canvas_owns_components_and_selection() {
    let mut canvas = canvas();
    assert_eq!(canvas.components.len(), 1);
    assert_eq!(canvas.selection, None);

    canvas.select(Some(0));
    assert_eq!(canvas.selection, Some(0));

    canvas.set_components(Vec::new());
    assert_eq!(canvas.components.len(), 0);
    assert_eq!(canvas.selection, None);
}

#[gpui::test]
fn canvas_renders_component_rects(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| canvas());
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("designer-component-0")
        .expect("component rect rendered");
    assert_eq!(bounds.size.width.to_f64(), 100.0);
    assert_eq!(bounds.size.height.to_f64(), 30.0);

    view.update(cx, |canvas, cx| {
        canvas.select(Some(0));
        cx.notify();
    });
    cx.run_until_parked();
    // Selected canvas still lays the component out at the same place.
    assert!(cx.debug_bounds("designer-component-0").is_some());
}
