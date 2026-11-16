//! Skeleton of the visual form canvas: renders the form background, its
//! components as rects, and the current selection. State (zoom, offset,
//! selection) is owned here; later tasks feed components from parsed LFM.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div, px,
};

use crate::model::DesignerComponent;

/// `(left, top, width, height)` of an axis-aligned rectangle in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// Maps an LFM-coordinate rect to canvas pixels for a zoom and offset.
pub fn component_pixel_rect(
    bounds: (f32, f32, f32, f32),
    zoom: f32,
    offset: (f32, f32),
) -> PixelRect {
    PixelRect {
        left: bounds.0 * zoom + offset.0,
        top: bounds.1 * zoom + offset.1,
        width: bounds.2 * zoom,
        height: bounds.3 * zoom,
    }
}

/// Renders a Lazarus form as an interactive canvas.
pub struct FormCanvas {
    /// Components placed on the form, in source order.
    pub components: Vec<DesignerComponent>,
    /// The form's own rect in LFM units: `(left, top, width, height)`.
    pub root_bounds: (f32, f32, f32, f32),
    /// View scale applied to all LFM coordinates.
    pub zoom: f32,
    /// Canvas-space offset in pixels added to scaled coordinates.
    pub offset: (f32, f32),
    /// Index into `components` of the selected component.
    pub selection: Option<usize>,
}

impl FormCanvas {
    pub fn new(components: Vec<DesignerComponent>, root_bounds: (f32, f32, f32, f32)) -> Self {
        Self {
            components,
            root_bounds,
            zoom: 1.0,
            offset: (0.0, 0.0),
            selection: None,
        }
    }

    pub fn set_components(&mut self, components: Vec<DesignerComponent>) {
        self.components = components;
        self.selection = None;
    }

    pub fn select(&mut self, selection: Option<usize>) {
        self.selection = selection.filter(|index| *index < self.components.len());
    }
}

impl Render for FormCanvas {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let root = component_pixel_rect(self.root_bounds, self.zoom, self.offset);
        let spacing = (GRID_SPACING * self.zoom).max(4.0);

        let mut canvas = div()
            .id("form-canvas")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(gpui::rgb(0x1a1a1e));

        // Grid inside the form rect, drawn under it. The line count is capped
        // so an unusually large or malformed form cannot flood the frame.
        let columns = ((root.width / spacing).floor() as usize).min(MAX_GRID_LINES);
        let rows = ((root.height / spacing).floor() as usize).min(MAX_GRID_LINES);
        for column in 1..columns {
            let x = px(root.left + column as f32 * spacing);
            canvas = canvas.child(
                div()
                    .absolute()
                    .left(x)
                    .top(px(root.top))
                    .w(px(1.0))
                    .h(px(root.height))
                    .bg(gpui::rgb(0x26262b)),
            );
        }
        for row in 1..rows {
            let y = px(root.top + row as f32 * spacing);
            canvas = canvas.child(
                div()
                    .absolute()
                    .left(px(root.left))
                    .top(y)
                    .w(px(root.width))
                    .h(px(1.0))
                    .bg(gpui::rgb(0x26262b)),
            );
        }

        canvas = canvas.child(
            div()
                .absolute()
                .left(px(root.left))
                .top(px(root.top))
                .w(px(root.width))
                .h(px(root.height))
                .bg(gpui::rgb(0xf0f0f0))
                .border_1()
                .border_color(gpui::rgb(0x555555)),
        );

        for (index, component) in self.components.iter().enumerate() {
            let rect = component_pixel_rect(component.bounds, self.zoom, self.offset);
            let selected = self.selection == Some(index);
            canvas = canvas.child(
                div()
                    .id(shared_string(format!("designer-component-{index}")))
                    .debug_selector(move || format!("designer-component-{index}"))
                    .absolute()
                    .left(px(rect.left))
                    .top(px(rect.top))
                    .w(px(rect.width))
                    .h(px(rect.height))
                    .bg(gpui::rgb(0xd8d8d8))
                    .border_2()
                    .border_color(if selected {
                        gpui::rgb(0x2f6feb)
                    } else {
                        gpui::rgb(0x888888)
                    })
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(gpui::rgb(0x222222))
                            .child(component.name.clone()),
                    ),
            );
        }

        canvas
    }
}

fn shared_string(text: String) -> gpui::SharedString {
    gpui::SharedString::from(text)
}

const GRID_SPACING: f32 = 8.0;
/// Upper bound on grid lines drawn per axis per frame.
const MAX_GRID_LINES: usize = 200;
