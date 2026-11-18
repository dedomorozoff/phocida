//! The workspace item that hosts the form designer: it resolves which `.lfm`
//! to edit from the active file, renders the canvas with the component palette
//! and an inspector, and saves edits back into the `.lfm`, which stays the
//! source of truth.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use editor::Editor;
use fs::Fs;
use gpui::{
    App, AppContext as _, AsyncWindowContext, Context, Div, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    Stateful, Styled, StatefulInteractiveElement, Subscription, WeakEntity, Window, div, px, rgb,
};
use project::Project;
use workspace::{Toast, Workspace, item::Item, notifications::NotificationId};

use crate::canvas::{FormCanvas, sync_bounds_to_properties};
use crate::inspector::set_property;
use crate::lfm::{LfmComponent, LfmDocument, parse_lfm, serialize_lfm};
use crate::model::{DesignerComponent, components_of};
use crate::palette::{ComponentDescriptor, palette_components, place_component};

pub enum DesignerViewEvent {}

impl EventEmitter<DesignerViewEvent> for DesignerView {}

struct PropertyField {
    key: String,
    editor: Entity<Editor>,
    geometry: bool,
}

/// The designer as a workspace pane item.
pub struct DesignerView {
    form_path: PathBuf,
    document: LfmDocument,
    canvas: Entity<FormCanvas>,
    project: Entity<Project>,
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    _canvas_subscription: Subscription,
    inspector_for: Option<usize>,
    inspector_fields: Vec<PropertyField>,
    new_property_key: Option<Entity<Editor>>,
    new_property_value: Option<Entity<Editor>>,
}

impl DesignerView {
    pub fn new(
        form_path: PathBuf,
        document: LfmDocument,
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let canvas = cx.new(|_| FormCanvas::new(components_of(&document), document.root.bounds));
        canvas.update(cx, |canvas, _| {
            canvas.set_focus_handle(focus_handle.clone());
        });
        let _canvas_subscription = cx.observe(&canvas, |_, _, cx| {
            cx.notify();
        });
        Self {
            form_path,
            document,
            canvas,
            project,
            workspace,
            focus_handle,
            _canvas_subscription,
            inspector_for: None,
            inspector_fields: Vec::new(),
            new_property_key: None,
            new_property_value: None,
        }
    }

    pub fn canvas(&self) -> &Entity<FormCanvas> {
        &self.canvas
    }

    fn selected_component(&self, cx: &App) -> Option<DesignerComponent> {
        let canvas = self.canvas.read(cx);
        let index = canvas.selection.primary()?;
        canvas.components.get(index).cloned()
    }

    fn sync_inspector(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        let selection = self.canvas.read(cx).selection.primary();
        if self.inspector_for == selection && !self.inspector_fields.is_empty() {
            return;
        }
        self.inspector_for = selection;
        self.inspector_fields.clear();
        self.new_property_key = None;
        self.new_property_value = None;
        let Some(window) = window else {
            return;
        };
        let Some(component) = self.selected_component(cx) else {
            return;
        };
        let (left, top, width, height) = component.bounds;
        let geom = [
            ("Left", format_int(left)),
            ("Top", format_int(top)),
            ("Width", format_int(width)),
            ("Height", format_int(height)),
        ];
        for (key, value) in geom {
            let editor = cx.new(|cx| {
                let mut e = Editor::single_line(window, cx);
                e.set_text(value, window, cx);
                e
            });
            self.inspector_fields.push(PropertyField {
                key: key.to_string(),
                editor,
                geometry: true,
            });
        }

        let mut keys: Vec<String> = component
            .properties
            .keys()
            .filter(|k| !is_geometry_key(k))
            .cloned()
            .collect();
        keys.sort();
        for key in keys {
            let value = component.properties.get(&key).cloned().unwrap_or_default();
            let editor = cx.new(|cx| {
                let mut e = Editor::single_line(window, cx);
                e.set_text(value, window, cx);
                e
            });
            self.inspector_fields.push(PropertyField {
                key,
                editor,
                geometry: false,
            });
        }

        self.new_property_key = Some(cx.new(|cx| Editor::single_line(window, cx)));
        self.new_property_value = Some(cx.new(|cx| Editor::single_line(window, cx)));
    }

    fn apply_inspector(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.canvas.read(cx).selection.primary() else {
            return;
        };
        let mut geom: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
        let mut props: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for field in &self.inspector_fields {
            let text = field.editor.read(cx).text(cx);
            if field.geometry {
                if let Ok(number) = text.trim().parse::<f32>() {
                    geom.insert(field.key.clone(), number);
                }
            } else {
                // Allow clearing a property by leaving it empty: skip empty -> removed.
                if !text.trim().is_empty() {
                    props.insert(field.key.clone(), text);
                }
            }
        }
        self.canvas.update(cx, |canvas, _| {
            canvas.checkpoint();
            let Some(component) = canvas.components.get_mut(selection) else {
                return;
            };
            if let Some(v) = geom.get("Left") { component.bounds.0 = *v; }
            if let Some(v) = geom.get("Top") { component.bounds.1 = *v; }
            if let Some(v) = geom.get("Width") { component.bounds.2 = (*v).max(1.0); }
            if let Some(v) = geom.get("Height") { component.bounds.3 = (*v).max(1.0); }

            // Replace non-geometry properties with edited set.
            component.properties.retain(|k, _| is_geometry_key(k));
            for (k, v) in props {
                set_property(component, &k, &v);
            }
            sync_bounds_to_properties(component);
        });
        cx.notify();
    }

    fn add_property(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(key_editor), Some(value_editor)) =
            (self.new_property_key.as_ref(), self.new_property_value.as_ref())
        else {
            return;
        };
        let key = key_editor.read(cx).text(cx).trim().to_string();
        let value = value_editor.read(cx).text(cx);
        if key.is_empty() || is_geometry_key(&key) {
            return;
        }
        if self.inspector_fields.iter().any(|f| f.key.eq_ignore_ascii_case(&key)) {
            return;
        }
        let editor = cx.new(|cx| {
            let mut e = Editor::single_line(window, cx);
            e.set_text(value, window, cx);
            e
        });
        self.inspector_fields.push(PropertyField {
            key,
            editor,
            geometry: false,
        });
        // reset inputs
        if let Some(k) = self.new_property_key.as_ref() {
            k.update(cx, |e, cx| e.set_text(String::new(), window, cx));
        }
        if let Some(v) = self.new_property_value.as_ref() {
            v.update(cx, |e, cx| e.set_text(String::new(), window, cx));
        }
        cx.notify();
    }

    fn remove_property(&mut self, key: &str, cx: &mut Context<Self>) {
        self.inspector_fields.retain(|f| !( !f.geometry && f.key == key ));
        cx.notify();
    }

    fn reset_inspector(&mut self) {
        self.inspector_for = None;
        self.inspector_fields.clear();
        self.new_property_key = None;
        self.new_property_value = None;
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if self.canvas.update(cx, |canvas, _| canvas.undo()) {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if self.canvas.update(cx, |canvas, _| canvas.redo()) {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.canvas.update(cx, |canvas, _| canvas.delete_selected()) {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn duplicate_selected(&mut self, cx: &mut Context<Self>) {
        let new_indices = self
            .canvas
            .update(cx, |canvas, _| canvas.duplicate_selected());
        if !new_indices.is_empty() {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, _| canvas.zoom_in());
        cx.notify();
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, _| canvas.zoom_out());
        cx.notify();
    }

    fn toggle_snap(&mut self, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, _| canvas.toggle_snap_to_grid());
        cx.notify();
    }

    fn raise_selected(&mut self, cx: &mut Context<Self>) {
        if self.canvas.update(cx, |canvas, _| canvas.raise_selected()) {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn lower_selected(&mut self, cx: &mut Context<Self>) {
        if self.canvas.update(cx, |canvas, _| canvas.lower_selected()) {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn bring_selected_to_front(&mut self, cx: &mut Context<Self>) {
        if self
            .canvas
            .update(cx, |canvas, _| canvas.bring_selected_to_front())
        {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn send_selected_to_back(&mut self, cx: &mut Context<Self>) {
        if self
            .canvas
            .update(cx, |canvas, _| canvas.send_selected_to_back())
        {
            self.reset_inspector();
            cx.notify();
        }
    }

    fn fit_to_view(&mut self, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, _| canvas.zoom_to_fit());
        cx.notify();
    }

    fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.canvas.update(cx, |canvas, _| canvas.reset_view());
        cx.notify();
    }

    /// Designer shortcuts only apply while the view itself holds focus, so
    /// `Delete`/`Backspace` typed into an inspector editor stay text edits.
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if window.focused(cx).as_ref() != Some(&self.focus_handle) {
            return;
        }
        let keystroke = &event.keystroke;
        let key = keystroke.key.to_ascii_lowercase();
        let modifiers = keystroke.modifiers;
        if modifiers.secondary(cx) && !modifiers.alt {
            match key.as_str() {
                "z" if modifiers.shift => self.redo(cx),
                "z" => self.undo(cx),
                "y" => self.redo(cx),
                "d" => self.duplicate_selected(cx),
                "=" | "+" => self.zoom_in(cx),
                "-" | "_" => self.zoom_out(cx),
                "0" | ")" if modifiers.shift => self.fit_to_view(cx),
                "0" | ")" => self.reset_view(cx),
                "up" if modifiers.shift => self.bring_selected_to_front(cx),
                "up" => self.raise_selected(cx),
                "down" if modifiers.shift => self.send_selected_to_back(cx),
                "down" => self.lower_selected(cx),
                _ => return,
            }
            cx.stop_propagation();
        } else if !modifiers.modified() && matches!(key.as_str(), "delete" | "backspace") {
            self.delete_selected(cx);
            cx.stop_propagation();
        }
    }

    fn place(&mut self, descriptor: ComponentDescriptor, cx: &mut Context<Self>) {
        let count = self.canvas.read(cx).components.len() as f32;
        let cascade = (count * 8.0) % 96.0;
        let bounds = crate::palette::default_bounds_for(descriptor.class);
        self.canvas.update(cx, |canvas, _| {
            place_component(
                canvas,
                &descriptor,
                (24.0 + cascade, 24.0 + cascade, bounds.2, bounds.3),
            );
        });
        // Force inspector rebuild for new selection.
        self.reset_inspector();
    }

    fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.document.components = self
            .canvas
            .read(cx)
            .components
            .iter()
            .map(|component| LfmComponent {
                name: component.name.clone(),
                class: component.class.clone(),
                bounds: component.bounds,
                properties: component.properties.clone(),
            })
            .collect();
        // Keep root bounds/properties in sync if needed.
        let text = serialize_lfm(&self.document);
        let fs = self.project.read(cx).fs().clone();
        let form_path = self.form_path.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |_, cx| {
            let result = fs.write(&form_path, text.as_bytes()).await;
            workspace
                .update(cx, |workspace, cx| match result {
                    Ok(_) => notify(workspace, "Form saved".to_string(), cx),
                    Err(error) => {
                        notify(workspace, format!("Cannot save form: {error:#}"), cx)
                    }
                })
                .ok();
        })
        .detach();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let fs = self.project.read(cx).fs().clone();
        let form_path = self.form_path.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |this, cx| {
            let loaded = fs.load(&form_path).await;
            match loaded {
                Ok(text) => match parse_lfm(&text) {
                    Ok(document) => {
                        this.update(cx, |this, cx| {
                            this.document = document.clone();
                            this.canvas.update(cx, |canvas, _| {
                                canvas.set_components(components_of(&document));
                            });
                            this.reset_inspector();
                            cx.notify();
                        })
                        .ok();
                        workspace
                            .update(cx, |workspace, cx| {
                                notify(workspace, "Form reloaded".to_string(), cx)
                            })
                            .ok();
                    }
                    Err(error) => {
                        workspace
                            .update(cx, |workspace, cx| {
                                notify(
                                    workspace,
                                    format!("Cannot parse form: {error:#}"),
                                    cx,
                                )
                            })
                            .ok();
                    }
                },
                Err(error) => {
                    workspace
                        .update(cx, |workspace, cx| {
                            notify(workspace, format!("Cannot load form: {error:#}"), cx)
                        })
                        .ok();
                }
            }
        })
        .detach();
    }

    fn render_sidebar(&self, selected: Option<DesignerComponent>, cx: &Context<Self>) -> Stateful<Div> {
        let mut sidebar = div()
            .id("designer-sidebar")
            .w(px(260.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(rgb(0x222227))
            .border_l_1()
            .border_color(rgb(0x333338))
            .overflow_y_scroll();

        sidebar = sidebar.child(muted_label("Palette (drag to canvas or click)"));
        for descriptor in palette_components() {
            let desc = descriptor;
            sidebar = sidebar.child(
                div()
                    .id(SharedString::from(format!(
                        "designer-palette-{}",
                        descriptor.label
                    )))
                    .on_drag(
                        crate::palette::DraggedPaletteComponent { descriptor: desc },
                        |dragged, _, _, cx| cx.new(|_| dragged.clone()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.place(descriptor, cx)))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x2c2c31))
                    .text_size(px(12.0))
                    .text_color(rgb(0xd8d8d8))
                    .cursor_pointer()
                    .child(format!("{} ({})", descriptor.label, descriptor.class)),
            );
        }

        sidebar = sidebar.child(muted_label("Inspector"));
        match selected {
            Some(component) => {
                sidebar = sidebar.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xffffff))
                        .child(format!("{}: {}", component.name, component.class)),
                );
                if self.inspector_fields.is_empty() {
                    sidebar = sidebar.child(muted_label("Select to edit properties"));
                } else {
                    sidebar = sidebar.child(muted_label("Geometry"));
                    for field in self.inspector_fields.iter().filter(|f| f.geometry) {
                        sidebar = sidebar.child(self.render_property_row(field, false, cx));
                    }
                    sidebar = sidebar.child(muted_label("Properties"));
                    let mut any_prop = false;
                    for field in self.inspector_fields.iter().filter(|f| !f.geometry) {
                        any_prop = true;
                        sidebar = sidebar.child(self.render_property_row(field, true, cx));
                    }
                    if !any_prop {
                        sidebar = sidebar.child(muted_label("No custom properties"));
                    }
                    if let (Some(k), Some(v)) = (&self.new_property_key, &self.new_property_value) {
                        sidebar = sidebar.child(muted_label("Add property"));
                        sidebar = sidebar.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .id("designer-new-prop-key")
                                        .px_1()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x1a1a1e))
                                        .border_1()
                                        .border_color(rgb(0x44444a))
                                        .text_size(px(12.0))
                                        .child(k.clone()),
                                )
                                .child(
                                    div()
                                        .id("designer-new-prop-value")
                                        .px_1()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x1a1a1e))
                                        .border_1()
                                        .border_color(rgb(0x44444a))
                                        .text_size(px(12.0))
                                        .child(v.clone()),
                                )
                                .child(
                                    div()
                                        .id("designer-add-prop")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.add_property(window, cx);
                                        }))
                                        .px_2()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(rgb(0x44444a))
                                        .text_size(px(12.0))
                                        .text_color(rgb(0xffffff))
                                        .child("Add property"),
                                ),
                        );
                    }
                    sidebar = sidebar.child(
                        div()
                            .id("designer-apply")
                            .mt_1()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_inspector(window, cx);
                            }))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x2f6feb))
                            .text_size(px(12.0))
                            .text_color(rgb(0xffffff))
                            .child("Apply"),
                    );
                }
            }
            None => sidebar = sidebar.child(muted_label("Nothing selected")),
        }
        sidebar
    }

    fn render_property_row(&self, field: &PropertyField, removable: bool, cx: &Context<Self>) -> Div {
        let mut row = div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(0xa8a8b0))
                            .child(field.key.clone()),
                    ),
            );
        // Add remove button for non-geometry properties.
        if removable {
            let key = field.key.clone();
            row = div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(0xa8a8b0))
                                .child(field.key.clone()),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("designer-remove-{}", field.key)))
                                .px_1()
                                .rounded_sm()
                                .text_size(px(11.0))
                                .text_color(rgb(0xff7777))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_property(&key, cx);
                                }))
                                .child("×"),
                        ),
                );
        }
        row.child(
            div()
                .id(SharedString::from(format!("designer-field-{}", field.key)))
                .px_1()
                .py_0p5()
                .rounded_sm()
                .bg(rgb(0x1a1a1e))
                .border_1()
                .border_color(rgb(0x44444a))
                .text_size(px(12.0))
                .child(field.editor.clone()),
        )
    }
}

fn is_geometry_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "left" | "top" | "width" | "height"
    )
}

fn format_int(value: f32) -> String {
    format!("{}", value.round() as i32)
}

impl Render for DesignerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure inspector reflects current selection.
        if self.inspector_for != self.canvas.read(cx).selection.primary() {
            self.sync_inspector(Some(window), cx);
        }
        let file_name = file_display_name(&self.form_path);
        let selected = self.selected_component(cx);
        let (can_undo, can_redo, zoom, snap) = {
            let canvas = self.canvas.read(cx);
            (
                canvas.can_undo(),
                canvas.can_redo(),
                canvas.zoom,
                canvas.snaps_to_grid(),
            )
        };

        div()
            .id("designer-view")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1a1a1e))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(rgb(0x333338))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0xd8d8d8))
                            .child(format!("{} (design)", file_name)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id("designer-undo")
                                    .on_click(cx.listener(|this, _, _window, cx| this.undo(cx)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(if can_undo { 0x44444a } else { 0x2a2a2f }))
                                    .text_size(px(12.0))
                                    .text_color(rgb(if can_undo { 0xffffff } else { 0x66666c }))
                                    .cursor_pointer()
                                    .child("Undo"),
                            )
                            .child(
                                div()
                                    .id("designer-redo")
                                    .on_click(cx.listener(|this, _, _window, cx| this.redo(cx)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(if can_redo { 0x44444a } else { 0x2a2a2f }))
                                    .text_size(px(12.0))
                                    .text_color(rgb(if can_redo { 0xffffff } else { 0x66666c }))
                                    .cursor_pointer()
                                    .child("Redo"),
                            )
                            .child(
                                div()
                                    .id("designer-duplicate")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.duplicate_selected(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Duplicate"),
                            )
                            .child(
                                div()
                                    .id("designer-delete")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.delete_selected(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Delete"),
                            )
                            .child(
                                div()
                                    .id("designer-snap")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.toggle_snap(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(if snap { 0x2f6feb } else { 0x44444a }))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Snap 8px"),
                            )
                            .child(
                                div()
                                    .id("designer-bring-front")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.bring_selected_to_front(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Front"),
                            )
                            .child(
                                div()
                                    .id("designer-send-back")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.send_selected_to_back(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Back"),
                            )
                            .child(
                                div()
                                    .id("designer-zoom-out")
                                    .on_click(cx.listener(|this, _, _window, cx| this.zoom_out(cx)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("−"),
                            )
                            .child(
                                div()
                                    .id("designer-zoom-reset")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.reset_view(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x2c2c31))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xd8d8d8))
                                    .cursor_pointer()
                                    .child(format!("{}%", (zoom * 100.0).round() as i32)),
                            )
                            .child(
                                div()
                                    .id("designer-zoom-fit")
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.fit_to_view(cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("Fit"),
                            )
                            .child(
                                div()
                                    .id("designer-zoom-in")
                                    .on_click(cx.listener(|this, _, _window, cx| this.zoom_in(cx)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .cursor_pointer()
                                    .child("+"),
                            )
                            .child(
                                div()
                                    .id("designer-reload")
                                    .on_click(cx.listener(|this, _, _window, cx| this.reload(cx)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x44444a))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .child("Reload"),
                            )
                            .child(
                                div()
                                    .id("designer-save")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save(window, cx)
                                    }))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x2f6feb))
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xffffff))
                                    .child("Save form"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(div().flex_1().min_w_0().child(self.canvas.clone()))
                    .child(self.render_sidebar(selected, cx)),
            )
    }
}

impl Item for DesignerView {
    type Event = DesignerViewEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        format!("{} (design)", file_display_name(&self.form_path)).into()
    }
}

impl Focusable for DesignerView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Opens the designer for the form behind the active item: the `.lfm` itself, a
/// form unit's sibling `.lfm`, or the first `.lfm` a `.lpr` references.
pub fn open_designer_for_active_item(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(source_path) = active_item_abs_path(workspace, cx) else {
        notify(
            workspace,
            "Open a .lfm, a form unit, or a .lpr to design its form".to_string(),
            cx,
        );
        return;
    };
    let project = workspace.project().clone();
    let fs = project.read(cx).fs().clone();
    cx.spawn_in(window, async move |workspace, cx| {
        if let Err(error) = open_form(source_path, fs, project, workspace.clone(), cx).await {
            workspace
                .update(cx, |workspace, cx| {
                    notify(workspace, format!("Cannot open designer: {error:#}"), cx)
                })
                .ok();
        }
    })
    .detach();
}

async fn open_form(
    source_path: PathBuf,
    fs: Arc<dyn Fs>,
    project: Entity<Project>,
    workspace: WeakEntity<Workspace>,
    cx: &mut AsyncWindowContext,
) -> anyhow::Result<()> {
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let candidates: Vec<PathBuf> = match extension.as_str() {
        "lfm" | "pas" | "pp" | "lpr" | "dpr" => {
            let source = if matches!(extension.as_str(), "lpr" | "dpr") {
                fs.load(&source_path).await?
            } else {
                String::new()
            };
            form_candidates(&source_path, &source)
        }
        _ => Vec::new(),
    };
    anyhow::ensure!(
        !candidates.is_empty(),
        "no form to design for {}",
        source_path.display()
    );

    for candidate in candidates {
        let text = match fs.load(&candidate).await {
            Ok(text) => text,
            Err(_) => continue,
        };
        let document =
            parse_lfm(&text).with_context(|| format!("cannot parse {}", candidate.display()))?;
        workspace.update_in(cx, |workspace, window, cx| {
            let weak_workspace = cx.weak_entity();
            let view = cx.new(|cx| {
                DesignerView::new(
                    candidate.clone(),
                    document,
                    project.clone(),
                    weak_workspace,
                    cx,
                )
            });
            workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
        })?;
        return Ok(());
    }
    anyhow::bail!("no .lfm found next to {}", source_path.display());
}

fn active_item_abs_path(workspace: &Workspace, cx: &App) -> Option<PathBuf> {
    let item = workspace.active_item(cx)?;
    let project_path = item.project_path(cx)?;
    let project = workspace.project().read(cx);
    let worktree = project.worktree_for_id(project_path.worktree_id, cx)?;
    Some(
        worktree
            .read(cx)
            .abs_path()
            .join(project_path.path.as_std_path()),
    )
}

/// The `.lfm` files to try for an active source file, in order. For a form
/// unit that is its sibling `<stem>.lfm`; for a program file, one candidate
/// per unit named in its first `uses` clause — the first one that loads wins.
pub fn form_candidates(source_path: &Path, source_text: &str) -> Vec<PathBuf> {
    let Some(extension) = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
    else {
        return Vec::new();
    };
    let directory = source_path.parent().unwrap_or(Path::new("."));
    let stem = source_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    match extension.as_str() {
        "lfm" => vec![source_path.to_owned()],
        "pas" | "pp" => vec![directory.join(format!("{stem}.lfm"))],
        "lpr" | "dpr" => uses_units(source_text)
            .iter()
            .map(|unit| directory.join(format!("{unit}.lfm")))
            .collect(),
        _ => Vec::new(),
    }
}

/// The units named in the first `uses` clause of a Pascal source, in order.
pub fn uses_units(source: &str) -> Vec<String> {
    let Some(keyword) = find_word(source, "uses") else {
        return Vec::new();
    };
    let rest = &source[keyword + "uses".len()..];
    let Some(clause_end) = rest.find(';') else {
        return Vec::new();
    };
    strip_comments(&rest[..clause_end])
        .split(',')
        .filter_map(unit_name)
        .collect()
}

fn unit_name(entry: &str) -> Option<String> {
    let name = entry.split_whitespace().next()?;
    let first = name.chars().next()?;
    (first.is_ascii_alphabetic() || first == '_').then(|| name.to_owned())
}

fn strip_comments(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let brace = rest.find('{');
        let line = rest.find("//");
        let (position, to_line_end) = match (brace, line) {
            (Some(brace), Some(line)) if line < brace => (line, true),
            (Some(brace), _) => (brace, false),
            (None, Some(line)) => (line, true),
            (None, None) => break,
        };
        result.push_str(&rest[..position]);
        rest = if to_line_end {
            match rest[position..].find('\n') {
                Some(line_end) => &rest[position + line_end..],
                None => "",
            }
        } else {
            match rest[position + 1..].find('}') {
                Some(brace_end) => &rest[position + 1 + brace_end + 1..],
                None => "",
            }
        };
    }
    result.push_str(rest);
    result
}

fn find_word(source: &str, word: &str) -> Option<usize> {
    let lower = source.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(word) {
        let start = from + offset;
        let end = start + word.len();
        let before_is_word = start > 0 && is_ident_byte(bytes[start - 1]);
        let after_is_word = end < bytes.len() && is_ident_byte(bytes[end]);
        if !before_is_word && !after_is_word {
            return Some(start);
        }
        from = end;
    }
    None
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn file_display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn muted_label(text: &'static str) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(rgb(0x888890))
        .child(text)
}

fn notify(workspace: &mut Workspace, message: String, cx: &mut Context<Workspace>) {
    workspace.show_toast(
        Toast::new(NotificationId::named("lazarus-designer".into()), message),
        cx,
    );
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use fs::{FakeFs, Fs as _};
    use gpui::{AppContext as _, TestAppContext};
    use project::Project;
    use serde_json::json;
    use workspace::{AppState, Workspace};

    use crate::inspector::set_property;
    use crate::lfm::parse_lfm;
    use crate::palette::ComponentDescriptor;
    use crate::{DesignerView, form_candidates, open_designer_for_active_item, uses_units};

    const SAMPLE_LFM: &str = r#"object Form1: TForm1
  Left = 0
  Top = 0
  Width = 320
  Height = 240
  Caption = 'Form1'
  object Button1: TButton
    Left = 24
    Top = 24
    Width = 100
    Height = 30
    Caption = 'OK'
  end
end"#;

    const PROGRAM_LPR: &str = r#"program project1;

{$mode objfpc}{$H+}

uses
  {$IFDEF UNIX}
  cthreads,
  {$ENDIF}
  Interfaces, // this includes the LCL widgetset
  Unit1 in 'Unit1.pas' {Form1};

begin
  Application.Initialize;
end.
"#;

    #[test]
    fn uses_units_reads_entries_in_order() {
        assert_eq!(
            uses_units("uses Unit1, Unit2, Unit3;"),
            vec!["Unit1", "Unit2", "Unit3"]
        );
    }

    #[test]
    fn uses_units_skips_comments_and_directives() {
        assert_eq!(
            uses_units(PROGRAM_LPR),
            vec!["cthreads", "Interfaces", "Unit1"]
        );
    }

    #[test]
    fn uses_units_is_case_insensitive_and_keeps_dotted_units() {
        assert_eq!(uses_units("USES WinCrt;"), vec!["WinCrt"]);
        assert_eq!(
            uses_units("uses System.SysUtils, Forms;"),
            vec!["System.SysUtils", "Forms"]
        );
    }

    #[test]
    fn uses_units_without_a_clause_is_empty() {
        assert!(uses_units("unit Foo; interface implementation end.").is_empty());
    }

    #[test]
    fn form_candidates_for_lfm_is_the_file_itself() {
        assert_eq!(
            form_candidates(Path::new("/proj/Unit1.lfm"), ""),
            vec![PathBuf::from("/proj/Unit1.lfm")]
        );
    }

    #[test]
    fn form_candidates_for_a_form_unit_is_its_sibling_lfm() {
        assert_eq!(
            form_candidates(Path::new("/proj/unit1.pas"), ""),
            vec![PathBuf::from("/proj/unit1.lfm")]
        );
    }

    #[test]
    fn form_candidates_for_a_program_are_its_used_units() {
        assert_eq!(
            form_candidates(Path::new("/proj/project1.lpr"), PROGRAM_LPR),
            vec![
                PathBuf::from("/proj/cthreads.lfm"),
                PathBuf::from("/proj/Interfaces.lfm"),
                PathBuf::from("/proj/Unit1.lfm"),
            ]
        );
    }

    #[test]
    fn form_candidates_for_other_files_is_empty() {
        assert!(form_candidates(Path::new("/proj/readme.txt"), "").is_empty());
    }

    fn init_test(cx: &mut TestAppContext) -> Arc<AppState> {
        cx.update(|cx| {
            let state = AppState::test(cx);
            editor::init(cx);
            state
        })
    }

    #[gpui::test]
    async fn designer_saves_placed_and_edited_components(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        let fs: Arc<FakeFs> = app_state.fs.as_fake();
        fs.insert_tree(Path::new("/proj"), json!({ "Unit1.lfm": SAMPLE_LFM }))
            .await;

        let project = Project::test(fs.clone(), [Path::new("/proj")], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project, window, cx));

        workspace.update_in(cx, |workspace, window, cx| {
            let document = parse_lfm(SAMPLE_LFM).expect("sample lfm parses");
            let weak_workspace = cx.weak_entity();
            let view = cx.new(|cx| {
                DesignerView::new(
                    PathBuf::from("/proj/Unit1.lfm"),
                    document,
                    workspace.project().clone(),
                    weak_workspace,
                    cx,
                )
            });
            workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
        });
        cx.run_until_parked();

        workspace.update_in(cx, |workspace, _window, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item
                .downcast::<DesignerView>()
                .expect("active item is the designer");
            designer.update(cx, |designer, cx| {
                let descriptor = ComponentDescriptor {
                    class: "TButton",
                    label: "Button",
                };
                designer.place(descriptor, cx);
                let selection = designer
                    .canvas()
                    .read(cx)
                    .selection
                    .primary()
                    .expect("placed component is selected");
                designer.canvas().update(cx, |canvas, _| {
                    let component = canvas
                        .components
                        .get_mut(selection)
                        .expect("selection points at a component");
                    set_property(component, "Caption", "'Saved'");
                });
            });
        });

        workspace.update_in(cx, |workspace, window, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item
                .downcast::<DesignerView>()
                .expect("active item is the designer");
            designer.update(cx, |designer, cx| designer.save(window, cx));
        });
        cx.run_until_parked();

        let saved = fs
            .load(Path::new("/proj/Unit1.lfm"))
            .await
            .expect("saved form loads");
        assert!(saved.contains("object Form1: TForm1"), "root preserved");
        assert!(saved.contains("Button2"), "placed component written back");
        assert!(
            saved.contains("Caption = 'Saved'"),
            "inspector edit written back"
        );
        assert!(saved.contains("Button1"), "existing component preserved");
    }

    #[gpui::test]
    async fn design_project_opens_the_designer_for_an_lpr(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        let fs: Arc<FakeFs> = app_state.fs.as_fake();
        fs.insert_tree(
            Path::new("/proj"),
            json!({
                "project1.lpr": PROGRAM_LPR,
                "Unit1.lfm": SAMPLE_LFM,
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [Path::new("/proj")], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project, window, cx));

        workspace
            .update_in(cx, |workspace, window, cx| {
                workspace.open_abs_path(
                    PathBuf::from("/proj/project1.lpr"),
                    Default::default(),
                    window,
                    cx,
                )
            })
            .await
            .expect("the .lpr opens");

        workspace.update_in(cx, |workspace, window, cx| {
            open_designer_for_active_item(workspace, window, cx);
        });
        cx.run_until_parked();

        workspace.read_with(cx, |workspace, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item
                .downcast::<DesignerView>()
                .expect("the designer opens for the .lpr");
            assert_eq!(
                designer.read(cx).canvas().read(cx).components.len(),
                1,
                "the form's components are on the canvas"
            );
            assert_eq!(
                designer.read(cx).canvas().read(cx).components[0].name,
                "Button1"
            );
        });
        assert!(
            cx.debug_bounds("designer-component-0").is_some(),
            "the form component is rendered"
        );
    }

    #[gpui::test]
    async fn designer_shortcuts_duplicate_delete_and_undo(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        let fs: Arc<FakeFs> = app_state.fs.as_fake();
        fs.insert_tree(Path::new("/proj"), json!({ "Unit1.lfm": SAMPLE_LFM }))
            .await;

        let project = Project::test(fs.clone(), [Path::new("/proj")], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project, window, cx));

        workspace.update_in(cx, |workspace, window, cx| {
            let document = parse_lfm(SAMPLE_LFM).expect("sample lfm parses");
            let weak_workspace = cx.weak_entity();
            let view = cx.new(|cx| {
                DesignerView::new(
                    PathBuf::from("/proj/Unit1.lfm"),
                    document,
                    workspace.project().clone(),
                    weak_workspace,
                    cx,
                )
            });
            workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
        });
        cx.run_until_parked();

        workspace.update_in(cx, |workspace, window, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item
                .downcast::<DesignerView>()
                .expect("active item is the designer");
            let focus_handle = designer.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
            designer.update(cx, |designer, cx| {
                designer.canvas.update(cx, |canvas, _| canvas.select(Some(0)));
                cx.notify();
            });
        });

        cx.simulate_keystrokes("ctrl-d");
        cx.run_until_parked();
        workspace.read_with(cx, |workspace, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item.downcast::<DesignerView>().expect("designer item");
            let canvas = designer.read(cx).canvas.read(cx);
            assert_eq!(canvas.components.len(), 2, "duplicate was placed");
            assert_eq!(canvas.components[1].name, "Button2");
        });

        cx.simulate_keystrokes("delete");
        cx.run_until_parked();
        workspace.read_with(cx, |workspace, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item.downcast::<DesignerView>().expect("designer item");
            let canvas = designer.read(cx).canvas.read(cx);
            assert_eq!(canvas.components.len(), 1, "delete removed the selection");
        });

        cx.simulate_keystrokes("ctrl-z");
        cx.run_until_parked();
        workspace.read_with(cx, |workspace, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item.downcast::<DesignerView>().expect("designer item");
            let canvas = designer.read(cx).canvas.read(cx);
            assert_eq!(canvas.components.len(), 2, "undo restored the duplicate");
        });
    }

    #[gpui::test]
    async fn inspector_follows_the_primary_of_a_group_selection(cx: &mut TestAppContext) {
        let app_state = init_test(cx);
        let fs: Arc<FakeFs> = app_state.fs.as_fake();
        fs.insert_tree(Path::new("/proj"), json!({ "Unit1.lfm": SAMPLE_LFM }))
            .await;

        let project = Project::test(fs.clone(), [Path::new("/proj")], cx).await;
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project, window, cx));

        workspace.update_in(cx, |workspace, window, cx| {
            let document = parse_lfm(SAMPLE_LFM).expect("sample lfm parses");
            let weak_workspace = cx.weak_entity();
            let view = cx.new(|cx| {
                DesignerView::new(
                    PathBuf::from("/proj/Unit1.lfm"),
                    document,
                    workspace.project().clone(),
                    weak_workspace,
                    cx,
                )
            });
            workspace.add_item_to_active_pane(Box::new(view), None, true, window, cx);
        });
        cx.run_until_parked();

        workspace.update_in(cx, |workspace, window, cx| {
            let item = workspace.active_item(cx).expect("designer item is active");
            let designer = item
                .downcast::<DesignerView>()
                .expect("active item is the designer");
            designer.update(cx, |designer, cx| {
                designer.place(
                    ComponentDescriptor {
                        class: "TEdit",
                        label: "Edit",
                    },
                    cx,
                );
                // Group: Button1 (index 0) and Edit1 (index 1), primary 0.
                designer.canvas.update(cx, |canvas, _| {
                    canvas.select(Some(1));
                    canvas.toggle_selection(0);
                });
                assert_eq!(designer.canvas.read(cx).selection.indices().count(), 2);

                designer.sync_inspector(Some(window), cx);
                assert_eq!(designer.inspector_for, Some(0));
                assert_eq!(
                    designer.selected_component(cx).expect("primary selected").name,
                    "Button1"
                );

                // A secondary change keeps the inspector; a primary change moves it.
                designer.canvas.update(cx, |canvas, _| {
                    canvas.selection.set_primary(1);
                });
                designer.sync_inspector(Some(window), cx);
                assert_eq!(designer.inspector_for, Some(1));
                assert_eq!(
                    designer.selected_component(cx).expect("primary selected").name,
                    "Edit1"
                );
                assert_eq!(
                    designer.canvas.read(cx).selection.indices().count(),
                    2,
                    "the group stays selected"
                );
            });
        });
    }
}
