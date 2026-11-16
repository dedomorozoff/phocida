//! Skeleton of the visual form canvas: renders the form background, its
//! components as rects, and the current selection. State (zoom, offset,
//! selection, drag) is owned here.

use gpui::{
    Bounds, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Point, Render, StatefulInteractiveElement, Styled, Window,
    div, px,
};

use crate::inspector::set_property;
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

/// Maps a canvas pixel delta to LFM units.
pub fn pixels_to_lfm(delta: f32, zoom: f32) -> f32 {
    if zoom.abs() < f32::EPSILON {
        0.0
    } else {
        delta / zoom
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeEdge {
    N,
    S,
    E,
    W,
    Ne,
    Nw,
    Se,
    Sw,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragMode {
    Move,
    Resize(ResizeEdge),
}

#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub index: usize,
    pub mode: DragMode,
    pub start_mouse: Point<Pixels>,
    pub start_bounds: (f32, f32, f32, f32),
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
    /// Active drag operation, if any.
    drag: Option<DragState>,
}

impl FormCanvas {
    pub fn new(components: Vec<DesignerComponent>, root_bounds: (f32, f32, f32, f32)) -> Self {
        Self {
            components,
            root_bounds,
            zoom: 1.0,
            offset: (0.0, 0.0),
            selection: None,
            drag: None,
        }
    }

    pub fn set_components(&mut self, components: Vec<DesignerComponent>) {
        self.components = components;
        self.selection = None;
        self.drag = None;
    }

    pub fn select(&mut self, selection: Option<usize>) {
        self.selection = selection.filter(|index| *index < self.components.len());
    }

    pub fn drag(&self) -> Option<DragState> {
        self.drag
    }

    pub fn start_drag(&mut self, index: usize, mode: DragMode, mouse: Point<Pixels>) {
        if index >= self.components.len() {
            return;
        }
        self.selection = Some(index);
        self.drag = Some(DragState {
            index,
            mode,
            start_mouse: mouse,
            start_bounds: self.components[index].bounds,
        });
    }

    pub fn update_drag(&mut self, mouse: Point<Pixels>) {
        let Some(drag) = self.drag else {
            return;
        };
        let Some(component) = self.components.get_mut(drag.index) else {
            self.drag = None;
            return;
        };
        let dx = pixels_to_lfm(f32::from(mouse.x - drag.start_mouse.x), self.zoom);
        let dy = pixels_to_lfm(f32::from(mouse.y - drag.start_mouse.y), self.zoom);
        let (mut left, mut top, mut width, mut height) = drag.start_bounds;
        match drag.mode {
            DragMode::Move => {
                left += dx;
                top += dy;
            }
            DragMode::Resize(edge) => {
                let (new_left, new_top, new_width, new_height) =
                    resize_bounds(left, top, width, height, dx, dy, edge);
                left = new_left;
                top = new_top;
                width = new_width;
                height = new_height;
            }
        }
        component.bounds = (left, top, width, height);
        sync_bounds_to_properties(component);
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }
}

fn resize_bounds(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    dx: f32,
    dy: f32,
    edge: ResizeEdge,
) -> (f32, f32, f32, f32) {
    let min_size = 8.0;
    let mut l = left;
    let mut t = top;
    let mut w = width;
    let mut h = height;
    match edge {
        ResizeEdge::E | ResizeEdge::Ne | ResizeEdge::Se => {
            w = (width + dx).max(min_size);
        }
        _ => {}
    }
    match edge {
        ResizeEdge::S | ResizeEdge::Se | ResizeEdge::Sw => {
            h = (height + dy).max(min_size);
        }
        _ => {}
    }
    match edge {
        ResizeEdge::W | ResizeEdge::Nw | ResizeEdge::Sw => {
            let new_w = (width - dx).max(min_size);
            l = left + (width - new_w);
            w = new_w;
        }
        _ => {}
    }
    match edge {
        ResizeEdge::N | ResizeEdge::Ne | ResizeEdge::Nw => {
            let new_h = (height - dy).max(min_size);
            t = top + (height - new_h);
            h = new_h;
        }
        _ => {}
    }
    (l, t, w, h)
}

pub fn sync_bounds_to_properties(component: &mut DesignerComponent) {
    let (left, top, width, height) = component.bounds;
    set_property(component, "Left", &format_int(left));
    set_property(component, "Top", &format_int(top));
    set_property(component, "Width", &format_int(width));
    set_property(component, "Height", &format_int(height));
}

fn format_int(value: f32) -> String {
    format!("{}", value.round() as i32)
}

impl Render for FormCanvas {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = component_pixel_rect(self.root_bounds, self.zoom, self.offset);
        let spacing = (GRID_SPACING * self.zoom).max(4.0);

        let mut canvas = div()
            .id("form-canvas")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(gpui::rgb(0x1a1a1e))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.update_drag(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.is_some() {
                        this.end_drag();
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.is_some() {
                        this.end_drag();
                        cx.notify();
                    }
                }),
            );

        // Grid inside the form rect, drawn under it.
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
            let mut el = div()
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
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.start_drag(index, DragMode::Move, event.position);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::rgb(0x222222))
                        .child(component.name.clone()),
                );

            if selected {
                // Resize handle at bottom-right corner.
                el = el.child(resize_handle(index, ResizeEdge::Se, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Sw, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Ne, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Nw, rect, cx));
            }

            canvas = canvas.child(el);
        }

        canvas
    }
}

fn resize_handle(
    index: usize,
    edge: ResizeEdge,
    rect: PixelRect,
    cx: &mut Context<FormCanvas>,
) -> impl IntoElement {
    let size = HANDLE_SIZE;
    let (left, top) = match edge {
        ResizeEdge::Se => (rect.width - size / 2.0, rect.height - size / 2.0),
        ResizeEdge::Sw => (-size / 2.0, rect.height - size / 2.0),
        ResizeEdge::Ne => (rect.width - size / 2.0, -size / 2.0),
        ResizeEdge::Nw => (-size / 2.0, -size / 2.0),
        ResizeEdge::N => (rect.width / 2.0 - size / 2.0, -size / 2.0),
        ResizeEdge::S => (rect.width / 2.0 - size / 2.0, rect.height - size / 2.0),
        ResizeEdge::E => (rect.width - size / 2.0, rect.height / 2.0 - size / 2.0),
        ResizeEdge::W => (-size / 2.0, rect.height / 2.0 - size / 2.0),
    };
    div()
        .id(shared_string(format!("designer-handle-{index}-{edge:?}")))
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(size))
        .h(px(size))
        .bg(gpui::rgb(0x2f6feb))
        .border_1()
        .border_color(gpui::rgb(0xffffff))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.start_drag(index, DragMode::Resize(edge), event.position);
                cx.stop_propagation();
                cx.notify();
            }),
        )
}

fn shared_string(text: String) -> gpui::SharedString {
    gpui::SharedString::from(text)
}

const GRID_SPACING: f32 = 8.0;
/// Upper bound on grid lines drawn per axis per frame.
const MAX_GRID_LINES: usize = 200;
const HANDLE_SIZE: f32 = 8.0;

