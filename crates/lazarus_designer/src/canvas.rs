//! Skeleton of the visual form canvas: renders the form background, its
//! components as rects, and the current selection. State (zoom, offset,
//! selection, drag) is owned here.

use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, Styled, Window, div, px,
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

/// A designer state recorded for undo/redo.
#[derive(Clone, PartialEq)]
struct CanvasSnapshot {
    components: Vec<DesignerComponent>,
    selection: Option<usize>,
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
    /// State before the active drag moved anything; becomes an undo entry on
    /// the first actual movement so a plain click is not undoable.
    drag_origin: Option<CanvasSnapshot>,
    /// Undo entries, oldest first.
    history: Vec<CanvasSnapshot>,
    /// Undone entries, most recently undone last.
    redo_stack: Vec<CanvasSnapshot>,
    focus_handle: Option<FocusHandle>,
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
            drag_origin: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
            focus_handle: None,
        }
    }

    /// The view that hosts this canvas focuses itself on canvas mouse-down so
    /// designer shortcuts work without stealing focus from inspector fields.
    pub fn set_focus_handle(&mut self, focus_handle: FocusHandle) {
        self.focus_handle = Some(focus_handle);
    }

    pub fn set_components(&mut self, components: Vec<DesignerComponent>) {
        self.components = components;
        self.selection = None;
        self.drag = None;
        self.drag_origin = None;
        self.history.clear();
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            components: self.components.clone(),
            selection: self.selection,
        }
    }

    fn restore(&mut self, snapshot: CanvasSnapshot) {
        self.components = snapshot.components;
        self.selection = snapshot
            .selection
            .filter(|index| *index < self.components.len());
        self.drag = None;
        self.drag_origin = None;
    }

    /// Records the current state for undo if it differs from the last entry.
    /// Call before mutating the canvas.
    pub fn checkpoint(&mut self) {
        self.record(self.snapshot());
    }

    fn record(&mut self, snapshot: CanvasSnapshot) {
        if self.history.last() == Some(&snapshot) {
            return;
        }
        self.history.push(snapshot);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.history.pop() else {
            return false;
        };
        self.redo_stack.push(self.snapshot());
        if self.redo_stack.len() > MAX_HISTORY {
            self.redo_stack.remove(0);
        }
        self.restore(previous);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let snapshot = self.snapshot();
        self.history.push(snapshot);
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        self.restore(next);
        true
    }

    /// Removes the selected component. Returns whether anything was removed.
    pub fn delete_selected(&mut self) -> bool {
        let Some(index) = self.selection else {
            return false;
        };
        if index >= self.components.len() {
            return false;
        }
        self.checkpoint();
        self.components.remove(index);
        self.selection = if self.components.is_empty() {
            None
        } else {
            Some(index.min(self.components.len() - 1))
        };
        true
    }

    /// Copies the selected component, offsets the copy and selects it.
    pub fn duplicate_selected(&mut self) -> Option<usize> {
        let index = self.selection?;
        let source = self.components.get(index)?.clone();
        let existing_names: Vec<String> =
            self.components.iter().map(|c| c.name.clone()).collect();
        self.checkpoint();
        let copy = crate::palette::duplicate_component(&source, |name| {
            existing_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(name))
        });
        self.components.push(copy);
        let new_index = self.components.len() - 1;
        self.selection = Some(new_index);
        Some(new_index)
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom * ZOOM_STEP);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom / ZOOM_STEP);
    }

    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    #[cfg(test)]
    pub(crate) fn undo_depth(&self) -> usize {
        self.history.len()
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
        self.drag_origin = Some(self.snapshot());
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
        let dx = pixels_to_lfm(f32::from(mouse.x - drag.start_mouse.x), self.zoom);
        let dy = pixels_to_lfm(f32::from(mouse.y - drag.start_mouse.y), self.zoom);
        if dx != 0.0 || dy != 0.0 {
            if let Some(origin) = self.drag_origin.take() {
                self.record(origin);
            }
        }
        let Some(component) = self.components.get_mut(drag.index) else {
            self.drag = None;
            return;
        };
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
        self.drag_origin = None;
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

        let canvas = div()
            .id("form-canvas")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(gpui::rgb(0x2b2b30))
            .on_drop::<crate::palette::DraggedPaletteComponent>(cx.listener(
                |this, dragged: &crate::palette::DraggedPaletteComponent, _window, cx| {
                    let count = this.components.len() as f32;
                    let cascade = (count * 8.0) % 96.0;
                    let bounds = crate::palette::default_bounds_for(dragged.descriptor.class);
                    crate::palette::place_component(
                        this,
                        &dragged.descriptor,
                        (24.0 + cascade, 24.0 + cascade, bounds.2, bounds.3),
                    );
                    cx.notify();
                },
            ))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.update_drag(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if let Some(focus_handle) = this.focus_handle.clone() {
                        window.focus(&focus_handle, cx);
                    }
                }),
            )
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

        // Form background + title bar.
        let mut canvas = canvas.child(
            div()
                .absolute()
                .left(px(root.left))
                .top(px(root.top))
                .w(px(root.width))
                .h(px(root.height))
                .bg(gpui::rgb(0xf0f0f0))
                .border_1()
                .border_color(gpui::rgb(0x7a7a7a))
                .child(
                    div()
                        .w_full()
                        .h(px(22.0))
                        .bg(gpui::rgb(0x3a6ea5))
                        .flex()
                        .items_center()
                        .px_2()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(gpui::rgb(0xffffff))
                                .child(form_caption(self)),
                        ),
                ),
        );

        // Grid inside the form rect.
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
                    .bg(gpui::rgb(0xe0e0e0)),
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
                    .bg(gpui::rgb(0xe0e0e0)),
            );
        }

        for (index, component) in self.components.iter().enumerate() {
            let rect = component_pixel_rect(component.bounds, self.zoom, self.offset);
            let selected = self.selection == Some(index);
            let (bg, border, text_color) = component_style(&component.class, selected);
            let label = component_label(component);

            let mut el = div()
                .id(shared_string(format!("designer-component-{index}")))
                .debug_selector(move || format!("designer-component-{index}"))
                .absolute()
                .left(px(rect.left))
                .top(px(rect.top))
                .w(px(rect.width))
                .h(px(rect.height))
                .bg(gpui::rgb(bg))
                .border_1()
                .border_color(gpui::rgb(if selected { 0x2f6feb } else { border }))
                .overflow_hidden()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if let Some(focus_handle) = this.focus_handle.clone() {
                            window.focus(&focus_handle, cx);
                        }
                        this.start_drag(index, DragMode::Move, event.position);
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .px_1()
                        .py_0p5()
                        .text_size(px(11.0))
                        .text_color(gpui::rgb(text_color))
                        .child(label),
                );

            if selected {
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
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                if let Some(focus_handle) = this.focus_handle.clone() {
                    window.focus(&focus_handle, cx);
                }
                this.start_drag(index, DragMode::Resize(edge), event.position);
                cx.stop_propagation();
                cx.notify();
            }),
        )
}

fn component_style(class: &str, selected: bool) -> (u32, u32, u32) {
    // (background, border, text)
    let (bg, border, text) = match class {
        "TButton" => (0xe1e1e1, 0x8c8c8c, 0x111111),
        "TLabel" => (0x00000000, 0x00000000, 0x111111),
        "TEdit" => (0xffffff, 0x8c8c8c, 0x111111),
        "TMemo" => (0xffffff, 0x8c8c8c, 0x111111),
        "TPanel" => (0xdcdcdc, 0x9a9a9a, 0x111111),
        "TListBox" | "TComboBox" => (0xffffff, 0x8c8c8c, 0x111111),
        "TCheckBox" | "TRadioButton" => (0xf0f0f0, 0xf0f0f0, 0x111111),
        _ => (0xd8d8d8, 0x888888, 0x222222),
    };
    if selected {
        (if bg == 0 { 0xf0f0f0 } else { bg }, 0x2f6feb, text)
    } else {
        (bg, border, text)
    }
}

fn component_label(component: &DesignerComponent) -> String {
    if let Some(caption) = component
        .properties
        .get("Caption")
        .or_else(|| component.properties.get("Text"))
    {
        let caption = caption.trim();
        let caption = caption
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .unwrap_or(caption);
        if !caption.is_empty() {
            return format!("{}: {}", component.name, caption);
        }
    }
    format!("{} ({})", component.name, component.class)
}

fn form_caption(_canvas: &FormCanvas) -> String {
    "Form".to_string()
}

fn shared_string(text: String) -> gpui::SharedString {
    gpui::SharedString::from(text)
}

const GRID_SPACING: f32 = 8.0;
/// Upper bound on grid lines drawn per axis per frame.
const MAX_GRID_LINES: usize = 200;
const HANDLE_SIZE: f32 = 8.0;
/// Undo depth per designer session.
const MAX_HISTORY: usize = 200;
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;
const ZOOM_STEP: f32 = 1.25;

