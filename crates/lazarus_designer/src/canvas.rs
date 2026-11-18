//! Skeleton of the visual form canvas: renders the form background, its
//! components as rects, and the current selection. State (zoom, offset,
//! selection, drag) is owned here.

use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollDelta, ScrollEvent,
    Styled, Window, div, px,
};

use std::collections::BTreeSet;

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

impl PixelRect {
    /// Whether this rectangle overlaps another, counting shared edges as a hit.
    pub fn intersects(&self, other: PixelRect) -> bool {
        self.left <= other.left + other.width
            && other.left <= self.left + self.width
            && self.top <= other.top + other.height
            && other.top <= self.top + self.height
    }
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

/// An axis-aligned rect spanning two canvas points, in either drag direction.
fn rect_between(start: Point<Pixels>, current: Point<Pixels>) -> PixelRect {
    let start_x = f32::from(start.x);
    let start_y = f32::from(start.y);
    let current_x = f32::from(current.x);
    let current_y = f32::from(current.y);
    PixelRect {
        left: start_x.min(current_x),
        top: start_y.min(current_y),
        width: (current_x - start_x).abs(),
        height: (current_y - start_y).abs(),
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

#[derive(Debug, Clone)]
pub struct DragState {
    pub index: usize,
    pub mode: DragMode,
    pub start_mouse: Point<Pixels>,
    pub start_bounds: (f32, f32, f32, f32),
    /// Starting bounds of every component this drag moves, so a group move
    /// keeps relative offsets.
    pub group: Vec<(usize, (f32, f32, f32, f32))>,
}

/// The components selected on the canvas: a set of indices plus the primary
/// one that drives the inspector and the resize handles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    indices: BTreeSet<usize>,
    primary: Option<usize>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn primary(&self) -> Option<usize> {
        self.primary
    }

    pub fn contains(&self, index: usize) -> bool {
        self.indices.contains(&index)
    }

    pub fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.indices.iter().copied()
    }

    pub fn clear(&mut self) {
        self.indices.clear();
        self.primary = None;
    }

    pub fn select(&mut self, index: usize) {
        self.indices.clear();
        self.indices.insert(index);
        self.primary = Some(index);
    }

    pub fn toggle(&mut self, index: usize) {
        if self.indices.remove(&index) {
            if self.primary == Some(index) {
                self.primary = self.indices.iter().next_back().copied();
            }
        } else {
            self.indices.insert(index);
            self.primary = Some(index);
        }
    }

    pub fn set_primary(&mut self, index: usize) {
        if self.indices.contains(&index) {
            self.primary = Some(index);
        }
    }

    /// Drops indices that no longer address a component.
    pub fn retain_within(&mut self, len: usize) {
        self.indices.retain(|index| *index < len);
        if let Some(primary) = self.primary {
            if primary >= len {
                self.primary = self.indices.iter().next_back().copied();
            }
        } else {
            self.primary = if self.indices.is_empty() {
                None
            } else {
                self.indices.iter().next_back().copied()
            };
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PanState {
    start_mouse: Point<Pixels>,
    start_offset: (f32, f32),
}

/// An in-progress marquee drag. Geometry stays in canvas pixels so it follows
/// zoom and pan without conversion.
#[derive(Debug, Clone, Copy)]
struct MarqueeState {
    start: Point<Pixels>,
    current: Point<Pixels>,
    additive: bool,
}

/// A designer state recorded for undo/redo.
#[derive(Clone, PartialEq)]
struct CanvasSnapshot {
    components: Vec<DesignerComponent>,
    selection: Selection,
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
    /// Components selected on the canvas; the primary drives the inspector.
    pub selection: Selection,
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
    /// Size of the canvas area in pixels, measured at prepaint.
    pub viewport: (f32, f32),
    pan: Option<PanState>,
    /// Active marquee (rubber-band) selection drag, if any.
    marquee: Option<MarqueeState>,
    /// Whether drags snap to [`GRID_SNAP`].
    snap_to_grid: bool,
}

impl FormCanvas {
    pub fn new(components: Vec<DesignerComponent>, root_bounds: (f32, f32, f32, f32)) -> Self {
        Self {
            components,
            root_bounds,
            zoom: 1.0,
            offset: (0.0, 0.0),
            selection: Selection::new(),
            drag: None,
            drag_origin: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
            focus_handle: None,
            viewport: (0.0, 0.0),
            pan: None,
            marquee: None,
            snap_to_grid: true,
        }
    }

    /// The view that hosts this canvas focuses itself on canvas mouse-down so
    /// designer shortcuts work without stealing focus from inspector fields.
    pub fn set_focus_handle(&mut self, focus_handle: FocusHandle) {
        self.focus_handle = Some(focus_handle);
    }

    pub fn set_components(&mut self, components: Vec<DesignerComponent>) {
        self.components = components;
        self.selection.clear();
        self.drag = None;
        self.drag_origin = None;
        self.marquee = None;
        self.history.clear();
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            components: self.components.clone(),
            selection: self.selection.clone(),
        }
    }

    fn restore(&mut self, snapshot: CanvasSnapshot) {
        self.components = snapshot.components;
        self.selection = snapshot.selection;
        self.selection.retain_within(self.components.len());
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

    /// Removes every selected component. Returns whether anything was removed.
    pub fn delete_selected(&mut self) -> bool {
        let indices: Vec<usize> = self
            .selection
            .indices()
            .filter(|index| *index < self.components.len())
            .collect();
        if indices.is_empty() {
            return false;
        }
        let primary = self.selection.primary().unwrap_or(0);
        self.checkpoint();
        for index in indices.iter().rev() {
            self.components.remove(*index);
        }
        if self.components.is_empty() {
            self.selection.clear();
        } else {
            self.selection
                .select(primary.min(self.components.len() - 1));
        }
        true
    }

    /// Copies every selected component, offsets the copies and selects them.
    /// Returns the new indices in the order their sources were selected.
    pub fn duplicate_selected(&mut self) -> Vec<usize> {
        let sources: Vec<usize> = self
            .selection
            .indices()
            .filter(|index| *index < self.components.len())
            .collect();
        if sources.is_empty() {
            return Vec::new();
        }
        let primary = self.selection.primary();
        let mut existing_names: Vec<String> =
            self.components.iter().map(|c| c.name.clone()).collect();
        self.checkpoint();
        let mut new_indices = Vec::with_capacity(sources.len());
        let mut primary_copy = None;
        for source_index in sources {
            let source = self.components[source_index].clone();
            let copy = crate::palette::duplicate_component(&source, |name| {
                existing_names
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(name))
            });
            existing_names.push(copy.name.clone());
            self.components.push(copy);
            let new_index = self.components.len() - 1;
            new_indices.push(new_index);
            if primary == Some(source_index) {
                primary_copy = Some(new_index);
            }
        }
        self.selection.clear();
        for index in &new_indices {
            self.selection.toggle(*index);
        }
        if let Some(primary_copy) = primary_copy {
            self.selection.set_primary(primary_copy);
        }
        new_indices
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            self.viewport = (width, height);
        }
    }

    pub fn snaps_to_grid(&self) -> bool {
        self.snap_to_grid
    }

    pub fn toggle_snap_to_grid(&mut self) {
        self.snap_to_grid = !self.snap_to_grid;
    }

    /// Moves the selected components one step up the stacking order, keeping
    /// their relative order and membership.
    pub fn raise_selected(&mut self) -> bool {
        let selected: BTreeSet<usize> = self.selection.indices().collect();
        let Some(primary) = self.selection.primary() else {
            return false;
        };
        let len = self.components.len();
        if !selected
            .iter()
            .any(|index| index + 1 < len && !selected.contains(&(index + 1)))
        {
            return false;
        }
        self.checkpoint();
        let order: Vec<usize> = selected.iter().rev().copied().collect();
        let mut members = selected;
        let mut new_primary = primary;
        for index in order {
            if index + 1 < self.components.len() && !members.contains(&(index + 1)) {
                self.components.swap(index, index + 1);
                members.remove(&index);
                members.insert(index + 1);
                if new_primary == index {
                    new_primary = index + 1;
                }
            }
        }
        self.replace_selection(members.iter().copied(), Some(new_primary));
        true
    }

    /// Moves the selected components one step down the stacking order, keeping
    /// their relative order and membership.
    pub fn lower_selected(&mut self) -> bool {
        let selected: BTreeSet<usize> = self.selection.indices().collect();
        let Some(primary) = self.selection.primary() else {
            return false;
        };
        if !selected
            .iter()
            .any(|index| *index > 0 && !selected.contains(&(index - 1)))
        {
            return false;
        }
        self.checkpoint();
        let mut members = selected;
        let mut new_primary = primary;
        for index in members.clone() {
            if index > 0 && !members.contains(&(index - 1)) {
                self.components.swap(index, index - 1);
                members.remove(&index);
                members.insert(index - 1);
                if new_primary == index {
                    new_primary = index - 1;
                }
            }
        }
        self.replace_selection(members.iter().copied(), Some(new_primary));
        true
    }

    /// Moves the selected components to the top of the stacking order.
    pub fn bring_selected_to_front(&mut self) -> bool {
        let selected: Vec<usize> = self.selection.indices().collect();
        if selected.is_empty() {
            return false;
        }
        let first = selected[0];
        if selected == (first..self.components.len()).collect::<Vec<_>>() {
            return false;
        }
        let primary = self.selection.primary();
        self.checkpoint();
        let (kept, moved) = self.partition_by_selection(&selected);
        let start = kept.len();
        let mut components = kept;
        components.extend(moved);
        self.components = components;
        let primary_rank = primary.and_then(|primary| selected.iter().position(|i| *i == primary));
        self.replace_selection(
            start..self.components.len(),
            primary_rank.map(|rank| start + rank),
        );
        true
    }

    /// Moves the selected components to the bottom of the stacking order.
    pub fn send_selected_to_back(&mut self) -> bool {
        let selected: Vec<usize> = self.selection.indices().collect();
        if selected.is_empty() {
            return false;
        }
        if selected == (0..selected.len()).collect::<Vec<_>>() {
            return false;
        }
        let primary = self.selection.primary();
        self.checkpoint();
        let (kept, moved) = self.partition_by_selection(&selected);
        let count = moved.len();
        let mut components = moved;
        components.extend(kept);
        self.components = components;
        let primary_rank = primary.and_then(|primary| selected.iter().position(|i| *i == primary));
        self.replace_selection(0..count, primary_rank);
        true
    }

    /// Splits the components into `(unselected, selected)`, each in source
    /// order, for a stable z-order move.
    fn partition_by_selection(
        &self,
        selected: &[usize],
    ) -> (Vec<DesignerComponent>, Vec<DesignerComponent>) {
        let selected: BTreeSet<usize> = selected.iter().copied().collect();
        let mut kept = Vec::new();
        let mut moved = Vec::new();
        for (index, component) in self.components.iter().cloned().enumerate() {
            if selected.contains(&index) {
                moved.push(component);
            } else {
                kept.push(component);
            }
        }
        (kept, moved)
    }

    fn replace_selection(
        &mut self,
        indices: impl IntoIterator<Item = usize>,
        primary: Option<usize>,
    ) {
        self.selection.clear();
        for index in indices {
            self.selection.toggle(index);
        }
        self.selection.retain_within(self.components.len());
        if let Some(primary) = primary {
            self.selection.set_primary(primary);
        }
    }

    pub fn is_panning(&self) -> bool {
        self.pan.is_some()
    }

    pub fn start_pan(&mut self, mouse: Point<Pixels>) {
        self.pan = Some(PanState {
            start_mouse: mouse,
            start_offset: self.offset,
        });
    }

    pub fn update_pan(&mut self, mouse: Point<Pixels>) {
        let Some(pan) = self.pan else {
            return;
        };
        self.offset = (
            pan.start_offset.0 + f32::from(mouse.x - pan.start_mouse.x),
            pan.start_offset.1 + f32::from(mouse.y - pan.start_mouse.y),
        );
    }

    pub fn end_pan(&mut self) {
        self.pan = None;
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.offset.0 += dx;
        self.offset.1 += dy;
    }

    pub fn is_marqueeing(&self) -> bool {
        self.marquee.is_some()
    }

    /// Begins a rubber-band drag on empty canvas. `additive` (shift) unions the
    /// hits with the existing selection instead of replacing it.
    pub fn start_marquee(&mut self, mouse: Point<Pixels>, additive: bool) {
        self.marquee = Some(MarqueeState {
            start: mouse,
            current: mouse,
            additive,
        });
    }

    pub fn update_marquee(&mut self, mouse: Point<Pixels>) {
        if let Some(marquee) = self.marquee.as_mut() {
            marquee.current = mouse;
        }
    }

    /// The marquee rectangle in canvas pixels, while a drag is active.
    pub fn marquee_rect(&self) -> Option<PixelRect> {
        self.marquee
            .map(|marquee| rect_between(marquee.start, marquee.current))
    }

    /// Ends the marquee and applies its selection. Returns whether a real drag
    /// (as opposed to a click) was finished.
    pub fn finish_marquee(&mut self) -> bool {
        let Some(marquee) = self.marquee.take() else {
            return false;
        };
        let rect = rect_between(marquee.start, marquee.current);
        if rect.width < MARQUEE_CLICK_SLOP && rect.height < MARQUEE_CLICK_SLOP {
            if !marquee.additive {
                self.selection.clear();
            }
            return false;
        }
        let hits = self.selection_in_rect(rect);
        if marquee.additive {
            let mut newly_added = None;
            for index in hits {
                if !self.selection.contains(index) {
                    self.selection.toggle(index);
                    newly_added = Some(index);
                }
            }
            if let Some(index) = newly_added {
                self.selection.set_primary(index);
            }
        } else {
            self.selection.clear();
            for index in &hits {
                self.selection.toggle(*index);
            }
            if let Some(index) = hits.last() {
                self.selection.set_primary(*index);
            }
        }
        true
    }

    /// Indices whose pixel rect intersects `rect`, in paint order.
    fn selection_in_rect(&self, rect: PixelRect) -> Vec<usize> {
        self.components
            .iter()
            .enumerate()
            .filter(|(_, component)| {
                component_pixel_rect(component.bounds, self.zoom, self.offset).intersects(rect)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Applies a wheel scroll: plain scroll pans, a provided zoom factor zooms
    /// around the cursor. Deltas are in pixels and follow the input's sign, so
    /// content moves opposite to the scroll direction.
    pub fn handle_scroll(
        &mut self,
        delta: (f32, f32),
        zoom_factor: Option<f32>,
        cursor: Point<Pixels>,
    ) {
        match zoom_factor {
            Some(factor) => self.zoom_at(cursor, factor),
            None => self.pan_by(-delta.0, -delta.1),
        }
    }

    pub fn zoom_in(&mut self) {
        self.zoom_around_center(ZOOM_STEP);
    }

    pub fn zoom_out(&mut self) {
        self.zoom_around_center(1.0 / ZOOM_STEP);
    }

    fn zoom_around_center(&mut self, factor: f32) {
        if self.viewport.0 > 0.0 && self.viewport.1 > 0.0 {
            let center = gpui::point(px(self.viewport.0 / 2.0), px(self.viewport.1 / 2.0));
            self.zoom_at(center, factor);
        } else {
            self.set_zoom(self.zoom * factor);
        }
    }

    /// Zooms while keeping the LFM point under `cursor` in place.
    pub fn zoom_at(&mut self, cursor: Point<Pixels>, factor: f32) {
        let old_zoom = self.zoom;
        let new_zoom = (old_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if new_zoom == old_zoom {
            return;
        }
        let cursor_x = f32::from(cursor.x);
        let cursor_y = f32::from(cursor.y);
        let lfm_x = (cursor_x - self.offset.0) / old_zoom;
        let lfm_y = (cursor_y - self.offset.1) / old_zoom;
        self.zoom = new_zoom;
        self.offset = (
            cursor_x - lfm_x * new_zoom,
            cursor_y - lfm_y * new_zoom,
        );
    }

    /// Fits the form into the viewport with a margin and centers it.
    pub fn zoom_to_fit(&mut self) {
        let (viewport_width, viewport_height) = self.viewport;
        let (_, _, width, height) = self.root_bounds;
        if viewport_width <= 0.0 || viewport_height <= 0.0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let available_width = (viewport_width - FIT_PADDING * 2.0).max(1.0);
        let available_height = (viewport_height - FIT_PADDING * 2.0).max(1.0);
        self.zoom = (available_width / width)
            .min(available_height / height)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.center_form();
    }

    /// Sets 100% zoom and centers the form.
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.center_form();
    }

    pub fn center_form(&mut self) {
        let (viewport_width, viewport_height) = self.viewport;
        if viewport_width <= 0.0 || viewport_height <= 0.0 {
            return;
        }
        let (left, top, width, height) = self.root_bounds;
        self.offset = (
            (viewport_width - width * self.zoom) / 2.0 - left * self.zoom,
            (viewport_height - height * self.zoom) / 2.0 - top * self.zoom,
        );
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    #[cfg(test)]
    pub(crate) fn undo_depth(&self) -> usize {
        self.history.len()
    }

    pub fn select(&mut self, selection: Option<usize>) {
        match selection.filter(|index| *index < self.components.len()) {
            Some(index) => self.selection.select(index),
            None => self.selection.clear(),
        }
    }

    pub fn toggle_selection(&mut self, index: usize) {
        if index < self.components.len() {
            self.selection.toggle(index);
        }
    }

    /// Handles a left mouse-down on a component: shift toggles membership,
    /// a plain click on an unselected component selects it alone, and a plain
    /// click on a selected component makes it primary. Plain clicks also start
    /// a move drag; shift-click never starts one.
    pub fn press_component(&mut self, index: usize, shift: bool, mouse: Point<Pixels>) {
        if index >= self.components.len() {
            return;
        }
        if shift {
            self.selection.toggle(index);
            return;
        }
        if !self.selection.contains(index) {
            self.selection.select(index);
        } else {
            self.selection.set_primary(index);
        }
        self.start_drag(index, DragMode::Move, mouse);
    }

    pub fn drag(&self) -> Option<DragState> {
        self.drag.clone()
    }

    pub fn start_drag(&mut self, index: usize, mode: DragMode, mouse: Point<Pixels>) {
        if index >= self.components.len() {
            return;
        }
        if !self.selection.contains(index) {
            self.selection.select(index);
        } else {
            self.selection.set_primary(index);
        }
        self.drag_origin = Some(self.snapshot());
        let group = if matches!(mode, DragMode::Move) {
            self.selection
                .indices()
                .filter_map(|index| {
                    self.components
                        .get(index)
                        .map(|component| (index, component.bounds))
                })
                .collect()
        } else {
            vec![(index, self.components[index].bounds)]
        };
        self.drag = Some(DragState {
            index,
            mode,
            start_mouse: mouse,
            start_bounds: self.components[index].bounds,
            group,
        });
    }

    pub fn update_drag(&mut self, mouse: Point<Pixels>) {
        let Some(drag) = self.drag.clone() else {
            return;
        };
        let dx = pixels_to_lfm(f32::from(mouse.x - drag.start_mouse.x), self.zoom);
        let dy = pixels_to_lfm(f32::from(mouse.y - drag.start_mouse.y), self.zoom);
        if dx != 0.0 || dy != 0.0 {
            if let Some(origin) = self.drag_origin.take() {
                self.record(origin);
            }
        }
        match drag.mode {
            DragMode::Move => {
                let (start_left, start_top, _, _) = drag.start_bounds;
                let mut target_left = start_left + dx;
                let mut target_top = start_top + dy;
                if self.snap_to_grid {
                    target_left = snap_bound(target_left);
                    target_top = snap_bound(target_top);
                }
                let delta = (target_left - start_left, target_top - start_top);
                for (index, bounds) in &drag.group {
                    if let Some(component) = self.components.get_mut(*index) {
                        component.bounds.0 = bounds.0 + delta.0;
                        component.bounds.1 = bounds.1 + delta.1;
                        sync_bounds_to_properties(component);
                    }
                }
            }
            DragMode::Resize(edge) => {
                let Some(component) = self.components.get_mut(drag.index) else {
                    self.drag = None;
                    return;
                };
                let (mut left, mut top, mut width, mut height) = drag.start_bounds;
                let (new_left, new_top, new_width, new_height) =
                    resize_bounds(left, top, width, height, dx, dy, edge);
                left = new_left;
                top = new_top;
                width = new_width;
                height = new_height;
                if self.snap_to_grid {
                    (left, top, width, height) =
                        snap_bounds(left, top, width, height, drag.mode);
                }
                component.bounds = (left, top, width, height);
                sync_bounds_to_properties(component);
            }
        }
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
    let min_size = MIN_COMPONENT_SIZE;
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

fn snap_bound(value: f32) -> f32 {
    (value / GRID_SNAP).round() * GRID_SNAP
}

/// Snaps the bounds produced by a drag to the grid. Move snaps the origin;
/// resize snaps only the dragged edges so the opposite edge stays fixed.
fn snap_bounds(
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    mode: DragMode,
) -> (f32, f32, f32, f32) {
    match mode {
        DragMode::Move => (snap_bound(left), snap_bound(top), width, height),
        DragMode::Resize(edge) => {
            let mut l = left;
            let mut t = top;
            let mut w = width;
            let mut h = height;
            let right = left + width;
            let bottom = top + height;
            if matches!(
                edge,
                ResizeEdge::E | ResizeEdge::Ne | ResizeEdge::Se
            ) {
                w = (snap_bound(right) - l).max(MIN_COMPONENT_SIZE);
            }
            if matches!(
                edge,
                ResizeEdge::W | ResizeEdge::Nw | ResizeEdge::Sw
            ) {
                l = snap_bound(left).min(right - MIN_COMPONENT_SIZE);
                w = right - l;
            }
            if matches!(
                edge,
                ResizeEdge::S | ResizeEdge::Se | ResizeEdge::Sw
            ) {
                h = (snap_bound(bottom) - t).max(MIN_COMPONENT_SIZE);
            }
            if matches!(
                edge,
                ResizeEdge::N | ResizeEdge::Ne | ResizeEdge::Nw
            ) {
                t = snap_bound(top).min(bottom - MIN_COMPONENT_SIZE);
                h = bottom - t;
            }
            (l, t, w, h)
        }
    }
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
        let canvas_entity = cx.entity().downgrade();

        let canvas = div()
            .on_children_prepainted(move |bounds, _, cx| {
                let Some(probe) = bounds.first() else {
                    return;
                };
                canvas_entity
                    .update(cx, |canvas, _| {
                        canvas.set_viewport(
                            f32::from(probe.size.width),
                            f32::from(probe.size.height),
                        );
                    })
                    .ok();
            })
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
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                let (dx, dy) = match event.delta {
                    ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
                    ScrollDelta::Lines(delta) => (
                        delta.x * WHEEL_LINE_HEIGHT,
                        delta.y * WHEEL_LINE_HEIGHT,
                    ),
                };
                let zoom_factor = event
                    .modifiers
                    .secondary(cx)
                    .then(|| if dy >= 0.0 { WHEEL_ZOOM_STEP } else { 1.0 / WHEEL_ZOOM_STEP });
                this.handle_scroll((dx, dy), zoom_factor, event.position);
                cx.notify();
                cx.stop_propagation();
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.drag.is_some() {
                    this.update_drag(event.position);
                    cx.notify();
                } else if this.is_marqueeing() {
                    this.update_marquee(event.position);
                    cx.notify();
                } else if this.is_panning() {
                    this.update_pan(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_down(MouseButton::Middle, cx.listener(
                |this, event: &MouseDownEvent, window, cx| {
                    if let Some(focus_handle) = this.focus_handle.clone() {
                        window.focus(&focus_handle, cx);
                    }
                    this.start_pan(event.position);
                    cx.stop_propagation();
                    cx.notify();
                },
            ))
            .on_mouse_up(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                if this.is_panning() {
                    this.end_pan();
                    cx.notify();
                }
            }))
            .on_mouse_up_out(MouseButton::Middle, cx.listener(|this, _, _, cx| {
                if this.is_panning() {
                    this.end_pan();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if let Some(focus_handle) = this.focus_handle.clone() {
                        window.focus(&focus_handle, cx);
                    }
                    this.start_marquee(event.position, event.modifiers.shift);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.is_some() {
                        this.end_drag();
                        cx.notify();
                    } else if this.is_marqueeing() {
                        this.finish_marquee();
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
                    } else if this.is_marqueeing() {
                        this.finish_marquee();
                        cx.notify();
                    }
                }),
            );

        // Invisible first child: its bounds are the canvas size for zoom-to-fit.
        let canvas = canvas.child(
            div()
                .id("designer-viewport-probe")
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
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
            let selected = self.selection.contains(index);
            let primary = self.selection.primary() == Some(index);
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
                        this.press_component(index, event.modifiers.shift, event.position);
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

            if primary {
                el = el.child(resize_handle(index, ResizeEdge::Se, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Sw, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Ne, rect, cx));
                el = el.child(resize_handle(index, ResizeEdge::Nw, rect, cx));
            }

            canvas = canvas.child(el);
        }

        if let Some(rect) = self.marquee_rect() {
            canvas = canvas.child(
                div()
                    .id("designer-marquee")
                    .debug_selector(|| "designer-marquee".to_string())
                    .absolute()
                    .left(px(rect.left))
                    .top(px(rect.top))
                    .w(px(rect.width.max(1.0)))
                    .h(px(rect.height.max(1.0)))
                    .bg(gpui::rgba(0x2f6feb33))
                    .border_1()
                    .border_color(gpui::rgb(0x2f6feb)),
            );
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
/// Drag snapping step, same as the visible grid.
const GRID_SNAP: f32 = GRID_SPACING;
const MIN_COMPONENT_SIZE: f32 = 8.0;
/// Upper bound on grid lines drawn per axis per frame.
const MAX_GRID_LINES: usize = 200;
const HANDLE_SIZE: f32 = 8.0;
/// A marquee drag shorter than this on both axes counts as a click.
const MARQUEE_CLICK_SLOP: f32 = 2.0;
/// Undo depth per designer session.
const MAX_HISTORY: usize = 200;
pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 4.0;
const ZOOM_STEP: f32 = 1.25;
const WHEEL_ZOOM_STEP: f32 = 1.1;
/// Scroll `Lines` are converted to pixels with a typical line height.
const WHEEL_LINE_HEIGHT: f32 = 20.0;
/// Margin left around the form by zoom-to-fit.
const FIT_PADDING: f32 = 24.0;

