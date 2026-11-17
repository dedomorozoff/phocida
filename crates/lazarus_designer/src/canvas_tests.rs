use std::collections::HashMap;

use gpui::{TestAppContext, point, px};

use crate::canvas::{DragMode, FormCanvas, component_pixel_rect};
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

    canvas.delete_selected();
    assert!(canvas.can_undo());

    canvas.set_components(Vec::new());
    assert_eq!(canvas.components.len(), 0);
    assert_eq!(canvas.selection, None);
    assert!(!canvas.can_undo(), "reload clears the undo history");
    assert!(!canvas.can_redo());
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

#[test]
fn undo_restores_a_drag_and_redo_reapplies_it() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(20.0), px(10.0)));
    canvas.end_drag();
    assert_eq!(canvas.components[0].bounds, (30.0, 30.0, 100.0, 30.0));

    assert!(canvas.can_undo());
    assert!(canvas.undo());
    assert_eq!(canvas.components[0].bounds, (10.0, 20.0, 100.0, 30.0));
    assert_eq!(canvas.selection, Some(0));

    assert!(canvas.can_redo());
    assert!(canvas.redo());
    assert_eq!(canvas.components[0].bounds, (30.0, 30.0, 100.0, 30.0));
}

#[test]
fn click_without_movement_is_not_undoable() {
    let mut canvas = canvas();

    canvas.start_drag(0, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.end_drag();

    assert_eq!(canvas.selection, Some(0));
    assert!(!canvas.can_undo());
}

#[test]
fn delete_removes_the_selection_and_undo_brings_it_back() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    assert!(canvas.delete_selected());
    assert!(canvas.components.is_empty());
    assert_eq!(canvas.selection, None);

    assert!(canvas.undo());
    assert_eq!(canvas.components.len(), 1);
    assert_eq!(canvas.components[0].name, "Button1");
}

#[test]
fn delete_clamps_the_selection_to_the_remaining_components() {
    let mut canvas = canvas();
    canvas
        .components
        .push(DesignerComponent {
            name: "Edit1".to_owned(),
            class: "TEdit".to_owned(),
            bounds: (150.0, 20.0, 120.0, 28.0),
            properties: HashMap::default(),
        });
    canvas.select(Some(1));

    assert!(canvas.delete_selected());
    assert_eq!(canvas.selection, Some(0));

    assert!(canvas.delete_selected());
    assert_eq!(canvas.selection, None);
}

#[test]
fn duplicate_copies_properties_offsets_and_gets_a_unique_name() {
    let mut canvas = canvas();
    canvas.components[0]
        .properties
        .insert("Caption".to_owned(), "'OK'".to_owned());
    canvas.select(Some(0));

    let index = canvas.duplicate_selected().expect("duplicate is placed");
    assert_eq!(index, 1);
    let copy = &canvas.components[1];
    assert_eq!(copy.name, "Button2");
    assert_eq!(copy.class, "TButton");
    assert_eq!(copy.bounds, (18.0, 28.0, 100.0, 30.0));
    assert_eq!(
        copy.properties.get("Caption").map(String::as_str),
        Some("'OK'")
    );
    assert_eq!(canvas.selection, Some(1));

    let index = canvas.duplicate_selected().expect("second duplicate");
    assert_eq!(canvas.components[index].name, "Button3");
}

#[test]
fn history_is_bounded() {
    let mut canvas = canvas();
    for step in 0..250 {
        canvas.components[0].bounds.0 = step as f32;
        canvas.checkpoint();
    }

    assert!(canvas.undo_depth() <= 200);
}

#[test]
fn zoom_is_clamped_and_resettable() {
    let mut canvas = canvas();
    assert_eq!(canvas.zoom, 1.0);

    canvas.zoom_in();
    assert!((canvas.zoom - 1.25).abs() < f32::EPSILON);
    canvas.reset_zoom();
    assert_eq!(canvas.zoom, 1.0);

    canvas.set_zoom(100.0);
    assert_eq!(canvas.zoom, crate::canvas::MAX_ZOOM);
    canvas.set_zoom(0.01);
    assert_eq!(canvas.zoom, crate::canvas::MIN_ZOOM);
}

#[test]
fn undo_does_not_reset_the_zoom() {
    let mut canvas = canvas();
    canvas.zoom_in();
    canvas.select(Some(0));
    canvas.delete_selected();

    assert!(canvas.undo());
    assert!((canvas.zoom - 1.25).abs() < f32::EPSILON);
}
