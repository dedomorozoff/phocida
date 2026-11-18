use std::collections::HashMap;

use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

use crate::canvas::{DragMode, FormCanvas, ResizeEdge, Selection, component_pixel_rect};
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
fn selection_starts_empty() {
    let selection = Selection::new();
    assert!(selection.is_empty());
    assert_eq!(selection.primary(), None);
    assert_eq!(selection.len(), 0);
}

#[test]
fn selecting_replaces_membership_and_sets_primary() {
    let mut selection = Selection::new();
    selection.select(3);
    assert_eq!(selection.primary(), Some(3));
    assert!(selection.contains(3));
    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![3]);

    selection.select(1);
    assert_eq!(selection.primary(), Some(1));
    assert!(
        !selection.contains(3),
        "select replaces the whole membership"
    );
    assert_eq!(selection.len(), 1);
}

#[test]
fn toggling_adds_a_secondary_and_removes_it_again() {
    let mut selection = Selection::new();
    selection.select(0);
    selection.toggle(2);
    assert_eq!(
        selection.primary(),
        Some(2),
        "toggled-in index becomes primary"
    );
    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![0, 2]);

    selection.toggle(2);
    assert!(!selection.contains(2));
    assert_eq!(selection.primary(), Some(0), "primary falls back to a member");
    assert_eq!(selection.len(), 1);

    selection.toggle(0);
    assert!(selection.is_empty());
    assert_eq!(selection.primary(), None);
}

#[test]
fn stale_indices_are_filtered() {
    let mut selection = Selection::new();
    selection.select(1);
    selection.toggle(4);

    selection.retain_within(2);

    assert_eq!(selection.indices().collect::<Vec<_>>(), vec![1]);
    assert_eq!(selection.primary(), Some(1));
    assert!(selection.contains(1));
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
    assert_eq!(canvas.selection.primary(), None);

    canvas.select(Some(0));
    assert_eq!(canvas.selection.primary(), Some(0));

    canvas.delete_selected();
    assert!(canvas.can_undo());

    canvas.set_components(Vec::new());
    assert_eq!(canvas.components.len(), 0);
    assert_eq!(canvas.selection.primary(), None);
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
    assert_eq!(canvas.components[0].bounds, (32.0, 32.0, 100.0, 30.0));

    assert!(canvas.can_undo());
    assert!(canvas.undo());
    assert_eq!(canvas.components[0].bounds, (10.0, 20.0, 100.0, 30.0));
    assert_eq!(canvas.selection.primary(), Some(0));

    assert!(canvas.can_redo());
    assert!(canvas.redo());
    assert_eq!(canvas.components[0].bounds, (32.0, 32.0, 100.0, 30.0));
}

#[test]
fn move_snaps_to_the_grid() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(21.0), px(9.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds, (32.0, 32.0, 100.0, 30.0));
}

#[test]
fn move_without_snapping_keeps_sub_grid_positions() {
    let mut canvas = canvas();
    canvas.toggle_snap_to_grid();
    assert!(!canvas.snaps_to_grid());
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(21.0), px(9.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds, (31.0, 29.0, 100.0, 30.0));
}

#[test]
fn resize_snaps_only_the_dragged_edge() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Resize(ResizeEdge::E), point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(11.0), px(0.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds, (10.0, 20.0, 110.0, 30.0));
}

#[test]
fn resize_from_west_keeps_the_right_edge() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Resize(ResizeEdge::W), point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(-21.0), px(0.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds, (-8.0, 20.0, 118.0, 30.0));
}

#[test]
fn snap_does_not_shrink_a_component_below_the_minimum() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    canvas.start_drag(0, DragMode::Resize(ResizeEdge::E), point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(-500.0), px(0.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds.2, 8.0);
}

#[test]
fn click_without_movement_is_not_undoable() {
    let mut canvas = canvas();

    canvas.start_drag(0, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.end_drag();

    assert_eq!(canvas.selection.primary(), Some(0));
    assert!(!canvas.can_undo());
}

#[test]
fn clicking_an_unselected_component_selects_it_alone() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.press_component(2, false, point(px(0.0), px(0.0)));

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![2]);
    assert_eq!(canvas.selection.primary(), Some(2));
}

#[test]
fn clicking_a_selected_component_makes_it_primary_without_dropping_the_group() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.toggle_selection(2);
    canvas.press_component(0, false, point(px(0.0), px(0.0)));

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(canvas.selection.primary(), Some(0));
}

#[test]
fn shift_click_toggles_membership_and_starts_no_drag() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));

    canvas.press_component(2, true, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 2]);
    assert!(canvas.drag().is_none());

    canvas.press_component(2, true, point(px(0.0), px(0.0)));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);

    canvas.press_component(0, true, point(px(0.0), px(0.0)));
    assert!(canvas.selection.is_empty());
    assert!(canvas.drag().is_none());
}

#[test]
fn delete_removes_the_selection_and_undo_brings_it_back() {
    let mut canvas = canvas();
    canvas.select(Some(0));

    assert!(canvas.delete_selected());
    assert!(canvas.components.is_empty());
    assert_eq!(canvas.selection.primary(), None);

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
    assert_eq!(canvas.selection.primary(), Some(0));

    assert!(canvas.delete_selected());
    assert_eq!(canvas.selection.primary(), None);
}

#[test]
fn duplicate_copies_properties_offsets_and_gets_a_unique_name() {
    let mut canvas = canvas();
    canvas.components[0]
        .properties
        .insert("Caption".to_owned(), "'OK'".to_owned());
    canvas.select(Some(0));

    let indices = canvas.duplicate_selected();
    assert_eq!(indices, vec![1]);
    let copy = &canvas.components[1];
    assert_eq!(copy.name, "Button2");
    assert_eq!(copy.class, "TButton");
    assert_eq!(copy.bounds, (18.0, 28.0, 100.0, 30.0));
    assert_eq!(
        copy.properties.get("Caption").map(String::as_str),
        Some("'OK'")
    );
    assert_eq!(canvas.selection.primary(), Some(1));

    let indices = canvas.duplicate_selected();
    assert_eq!(canvas.components[indices[0]].name, "Button3");
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
    canvas.reset_view();
    assert_eq!(canvas.zoom, 1.0);

    canvas.set_zoom(100.0);
    assert_eq!(canvas.zoom, crate::canvas::MAX_ZOOM);
    canvas.set_zoom(0.01);
    assert_eq!(canvas.zoom, crate::canvas::MIN_ZOOM);
}

#[test]
fn middle_drag_pans_the_view() {
    let mut canvas = canvas();

    canvas.start_pan(point(px(10.0), px(10.0)));
    assert!(canvas.is_panning());
    canvas.update_pan(point(px(30.0), px(25.0)));
    canvas.end_pan();

    assert!(!canvas.is_panning());
    assert_eq!(canvas.offset, (20.0, 15.0));
}

#[test]
fn wheel_scroll_pans_against_the_delta() {
    let mut canvas = canvas();

    canvas.handle_scroll((4.0, 10.0), None, point(px(0.0), px(0.0)));

    assert_eq!(canvas.offset, (-4.0, -10.0));
}

#[test]
fn zoom_at_keeps_the_lfm_point_under_the_cursor() {
    let mut canvas = canvas();
    let cursor = point(px(100.0), px(100.0));

    canvas.zoom_at(cursor, 2.0);

    assert_eq!(canvas.zoom, 2.0);
    let lfm_x = (100.0 - canvas.offset.0) / canvas.zoom;
    let lfm_y = (100.0 - canvas.offset.1) / canvas.zoom;
    assert!((lfm_x - 100.0).abs() < 1e-4);
    assert!((lfm_y - 100.0).abs() < 1e-4);
}

#[test]
fn zoom_at_the_limit_does_not_move_the_view() {
    let mut canvas = canvas();
    canvas.set_zoom(crate::canvas::MAX_ZOOM);
    canvas.offset = (5.0, 7.0);

    canvas.zoom_at(point(px(10.0), px(10.0)), 2.0);

    assert_eq!(canvas.zoom, crate::canvas::MAX_ZOOM);
    assert_eq!(canvas.offset, (5.0, 7.0));
}

#[test]
fn zoom_to_fit_centers_the_form() {
    let mut canvas = canvas();
    canvas.set_viewport(400.0, 300.0);

    canvas.zoom_to_fit();

    let expected_zoom = (352.0f32 / 320.0).min(252.0 / 240.0);
    assert!((canvas.zoom - expected_zoom).abs() < 1e-4);
    assert!((canvas.offset.0 - (400.0 - 320.0 * expected_zoom) / 2.0).abs() < 1e-4);
    assert!((canvas.offset.1 - (300.0 - 240.0 * expected_zoom) / 2.0).abs() < 1e-4);
}

#[test]
fn reset_view_centers_the_form_at_full_size() {
    let mut canvas = canvas();
    canvas.set_viewport(400.0, 300.0);
    canvas.set_zoom(2.0);

    canvas.reset_view();

    assert_eq!(canvas.zoom, 1.0);
    assert_eq!(canvas.offset, (40.0, 30.0));
}

#[gpui::test]
fn canvas_records_its_viewport(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| canvas());
    cx.run_until_parked();

    view.read_with(cx, |canvas, _| {
        assert!(
            canvas.viewport.0 > 0.0 && canvas.viewport.1 > 0.0,
            "viewport measured at prepaint: {:?}",
            canvas.viewport
        );
    });
}

fn stacked_canvas() -> FormCanvas {
    let component = |name: &str, left: f32| DesignerComponent {
        name: name.to_owned(),
        class: "TButton".to_owned(),
        bounds: (left, 10.0, 40.0, 20.0),
        properties: HashMap::default(),
    };
    FormCanvas::new(
        vec![
            component("A", 0.0),
            component("B", 50.0),
            component("C", 100.0),
        ],
        (0.0, 0.0, 320.0, 240.0),
    )
}

fn component_names(canvas: &FormCanvas) -> Vec<&str> {
    canvas
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect()
}

#[test]
fn raise_and_lower_move_one_step() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(1));

    assert!(canvas.raise_selected());
    assert_eq!(component_names(&canvas), vec!["A", "C", "B"]);
    assert_eq!(canvas.selection.primary(), Some(2));

    assert!(canvas.lower_selected());
    assert_eq!(component_names(&canvas), vec!["A", "B", "C"]);
    assert_eq!(canvas.selection.primary(), Some(1));
}

#[test]
fn front_and_back_move_to_the_ends() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(1));

    assert!(canvas.bring_selected_to_front());
    assert_eq!(component_names(&canvas), vec!["A", "C", "B"]);
    assert_eq!(canvas.selection.primary(), Some(2));

    assert!(canvas.send_selected_to_back());
    assert_eq!(component_names(&canvas), vec!["B", "A", "C"]);
    assert_eq!(canvas.selection.primary(), Some(0));
}

#[test]
fn z_order_at_the_edges_is_a_no_op_without_history() {
    let mut canvas = stacked_canvas();

    canvas.select(Some(0));
    assert!(!canvas.lower_selected());
    assert!(!canvas.send_selected_to_back());

    canvas.select(Some(2));
    assert!(!canvas.raise_selected());
    assert!(!canvas.bring_selected_to_front());

    assert!(!canvas.can_undo());
}

#[test]
fn z_order_is_undoable() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));

    assert!(canvas.bring_selected_to_front());
    assert!(canvas.undo());

    assert_eq!(component_names(&canvas), vec!["A", "B", "C"]);
    assert_eq!(canvas.selection.primary(), Some(0));
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

fn two_button_canvas() -> FormCanvas {
    let mut canvas = FormCanvas::new(
        vec![
            button_component(),
            DesignerComponent {
                name: "Button2".to_owned(),
                class: "TButton".to_owned(),
                bounds: (150.0, 60.0, 40.0, 20.0),
                properties: HashMap::default(),
            },
        ],
        (0.0, 0.0, 320.0, 240.0),
    );
    canvas.select(Some(0));
    canvas.toggle_selection(1);
    canvas
}

#[test]
fn group_drag_moves_every_selected_component_by_the_same_delta() {
    let mut canvas = two_button_canvas();
    canvas.toggle_snap_to_grid();

    canvas.start_drag(1, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(21.0), px(9.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[0].bounds, (31.0, 29.0, 100.0, 30.0));
    assert_eq!(canvas.components[1].bounds, (171.0, 69.0, 40.0, 20.0));
}

#[test]
fn group_drag_is_one_undo_step() {
    let mut canvas = two_button_canvas();
    canvas.toggle_snap_to_grid();

    canvas.start_drag(1, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(21.0), px(9.0)));
    canvas.end_drag();

    assert!(canvas.undo());
    assert_eq!(canvas.components[0].bounds, (10.0, 20.0, 100.0, 30.0));
    assert_eq!(canvas.components[1].bounds, (150.0, 60.0, 40.0, 20.0));
    assert!(!canvas.can_undo(), "the group move was one history entry");
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
}

#[test]
fn group_move_snaps_the_primary_and_keeps_relative_offsets() {
    let mut canvas = two_button_canvas();

    canvas.start_drag(1, DragMode::Move, point(px(0.0), px(0.0)));
    canvas.update_drag(point(px(21.0), px(9.0)));
    canvas.end_drag();

    assert_eq!(canvas.components[1].bounds, (168.0, 72.0, 40.0, 20.0));
    assert_eq!(canvas.components[0].bounds, (28.0, 32.0, 100.0, 30.0));
    assert_eq!(
        canvas.components[0].bounds.0 - canvas.components[1].bounds.0,
        -140.0,
        "relative offset is preserved"
    );
}

#[test]
fn group_delete_removes_all_selected_and_one_undo_restores_them() {
    let mut canvas = two_button_canvas();

    assert!(canvas.delete_selected());
    assert!(canvas.components.is_empty());
    assert!(canvas.selection.is_empty());

    assert!(canvas.undo());
    assert_eq!(component_names(&canvas), vec!["Button1", "Button2"]);
    assert!(!canvas.can_undo(), "the group delete was one history entry");
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
}

#[test]
fn group_duplicate_copies_each_selected_with_unique_names() {
    let mut canvas = two_button_canvas();

    let copies = canvas.duplicate_selected();

    assert_eq!(copies, vec![2, 3]);
    assert_eq!(component_names(&canvas), vec![
        "Button1", "Button2", "Button3", "Button4"
    ]);
    assert_eq!(canvas.components[2].bounds, (18.0, 28.0, 100.0, 30.0));
    assert_eq!(canvas.components[3].bounds, (158.0, 68.0, 40.0, 20.0));
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(
        canvas.selection.primary(),
        Some(3),
        "the primary's copy stays primary"
    );
}

#[test]
fn group_duplicate_is_one_undo_step() {
    let mut canvas = two_button_canvas();

    canvas.duplicate_selected();
    assert!(canvas.undo());

    assert_eq!(component_names(&canvas), vec!["Button1", "Button2"]);
    assert!(!canvas.can_undo(), "the group duplicate was one history entry");
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
}


#[test]
fn undo_filters_selection_indices_that_no_longer_exist() {
    let mut canvas = two_button_canvas();
    // A stale index can enter through the public Selection API.
    canvas.selection.toggle(7);
    canvas.checkpoint();

    canvas.select(Some(0));
    assert!(canvas.undo());

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(
        canvas.selection.primary(),
        Some(1),
        "the stale primary falls back to a member"
    );
}

#[test]
fn group_operations_without_a_selection_are_no_ops() {
    let mut canvas = canvas();
    assert!(!canvas.delete_selected());
    assert!(canvas.duplicate_selected().is_empty());
    assert!(!canvas.can_undo(), "no-op operations record no history");
}

#[test]
fn bring_front_moves_every_selected_component() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.toggle_selection(1);

    assert!(canvas.bring_selected_to_front());

    assert_eq!(component_names(&canvas), vec!["C", "A", "B"]);
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(canvas.selection.primary(), Some(2));

    assert!(canvas.send_selected_to_back());
    assert_eq!(component_names(&canvas), vec!["A", "B", "C"]);
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(canvas.selection.primary(), Some(1));
}

#[test]
fn raise_moves_the_group_one_step_and_lower_returns_it() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.toggle_selection(1);

    assert!(canvas.raise_selected());
    assert_eq!(component_names(&canvas), vec!["C", "A", "B"]);
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(canvas.selection.primary(), Some(2));

    assert!(canvas.lower_selected());
    assert_eq!(component_names(&canvas), vec!["A", "B", "C"]);
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(canvas.selection.primary(), Some(1));
}

#[test]
fn group_z_order_at_the_stack_edge_is_a_noop_without_history() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(1));
    canvas.toggle_selection(2);

    assert!(!canvas.raise_selected());
    assert!(!canvas.bring_selected_to_front());
    assert!(!canvas.can_undo());

    canvas.select(Some(0));
    canvas.toggle_selection(1);
    assert!(!canvas.lower_selected());
    assert!(!canvas.send_selected_to_back());
    assert!(!canvas.can_undo());
}

#[test]
fn group_z_order_is_undoable_with_its_selection() {
    let mut canvas = stacked_canvas();
    canvas.select(Some(0));
    canvas.toggle_selection(1);
    assert!(canvas.bring_selected_to_front());

    assert!(canvas.undo());
    assert_eq!(component_names(&canvas), vec!["A", "B", "C"]);
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(canvas.selection.primary(), Some(1));
}

fn unselected_two_button_canvas() -> FormCanvas {
    let mut canvas = two_button_canvas();
    canvas.select(None);
    canvas
}

#[test]
fn marquee_selects_intersecting_components() {
    let mut canvas = unselected_two_button_canvas();

    canvas.start_marquee(point(px(0.0), px(0.0)), false);
    canvas.update_marquee(point(px(120.0), px(60.0)));
    assert!(canvas.finish_marquee());

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);
    assert_eq!(canvas.selection.primary(), Some(0));
    assert!(!canvas.is_marqueeing());
}

#[test]
fn marquee_replaces_the_current_selection() {
    let mut canvas = unselected_two_button_canvas();
    canvas.select(Some(1));

    canvas.start_marquee(point(px(0.0), px(0.0)), false);
    canvas.update_marquee(point(px(120.0), px(60.0)));
    canvas.finish_marquee();

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);
}

#[test]
fn shift_marquee_adds_to_the_selection() {
    let mut canvas = unselected_two_button_canvas();
    canvas.select(Some(1));

    canvas.start_marquee(point(px(0.0), px(0.0)), true);
    canvas.update_marquee(point(px(120.0), px(60.0)));
    canvas.finish_marquee();

    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(canvas.selection.primary(), Some(0));
}

#[test]
fn marquee_click_clears_the_selection_but_shift_click_keeps_it() {
    let mut canvas = unselected_two_button_canvas();
    canvas.select(Some(0));

    canvas.start_marquee(point(px(5.0), px(5.0)), false);
    canvas.finish_marquee();
    assert!(canvas.selection.is_empty());

    canvas.select(Some(0));
    canvas.start_marquee(point(px(5.0), px(5.0)), true);
    canvas.finish_marquee();
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);
}

#[test]
fn marquee_ignores_components_outside_the_rectangle() {
    let mut canvas = unselected_two_button_canvas();
    canvas.select(Some(0));

    canvas.start_marquee(point(px(200.0), px(150.0)), false);
    canvas.update_marquee(point(px(300.0), px(200.0)));
    canvas.finish_marquee();

    assert!(canvas.selection.is_empty());
}

#[test]
fn marquee_hit_tests_in_canvas_pixels_under_zoom_and_pan() {
    let mut canvas = unselected_two_button_canvas();
    canvas.zoom = 2.0;
    canvas.offset = (100.0, 100.0);

    canvas.start_marquee(point(px(0.0), px(0.0)), false);
    canvas.update_marquee(point(px(119.0), px(139.0)));
    canvas.finish_marquee();
    assert!(
        canvas.selection.is_empty(),
        "zoomed-out rectangle stops short of Button1"
    );

    canvas.start_marquee(point(px(0.0), px(0.0)), false);
    canvas.update_marquee(point(px(125.0), px(145.0)));
    canvas.finish_marquee();
    assert_eq!(canvas.selection.indices().collect::<Vec<_>>(), vec![0]);
}

#[gpui::test]
fn marquee_renders_an_overlay_while_dragging(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| canvas());
    cx.run_until_parked();

    view.update(cx, |canvas, cx| {
        canvas.start_marquee(point(px(0.0), px(0.0)), false);
        canvas.update_marquee(point(px(50.0), px(40.0)));
        cx.notify();
    });
    cx.run_until_parked();

    let bounds = cx
        .debug_bounds("designer-marquee")
        .expect("marquee overlay is rendered while dragging");
    assert_eq!(bounds.size.width.to_f64(), 50.0);
    assert_eq!(bounds.size.height.to_f64(), 40.0);

    view.update(cx, |canvas, cx| {
        canvas.finish_marquee();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("designer-marquee").is_none(),
        "the overlay disappears once the drag ends"
    );
}

#[gpui::test]
fn dragging_on_the_background_selects_components(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| unselected_two_button_canvas());
    cx.run_until_parked();

    cx.simulate_mouse_down(
        point(px(0.0), px(0.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(px(120.0), px(60.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(120.0), px(60.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();

    view.read_with(cx, |canvas, _| {
        assert_eq!(
            canvas.selection.indices().collect::<Vec<_>>(),
            vec![0],
            "background drag selected the intersecting component"
        );
    });
}
