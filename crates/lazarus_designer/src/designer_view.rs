//! The workspace item that hosts the form designer: it resolves which `.lfm`
//! to edit from the active file, renders the canvas with the component palette
//! and an inspector, and saves edits back into the `.lfm`, which stays the
//! source of truth.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use fs::Fs;
use gpui::{
    App, AppContext as _, AsyncWindowContext, Context, Div, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Stateful,
    Styled, StatefulInteractiveElement, Subscription, WeakEntity, Window, div, px, rgb,
};
use project::Project;
use workspace::{Toast, Workspace, item::Item, notifications::NotificationId};

use crate::canvas::FormCanvas;
use crate::lfm::{LfmComponent, LfmDocument, parse_lfm, serialize_lfm};
use crate::model::{DesignerComponent, components_of};
use crate::palette::{ComponentDescriptor, DEFAULT_BOUNDS, palette_components, place_component};

pub enum DesignerViewEvent {}

impl EventEmitter<DesignerViewEvent> for DesignerView {}

/// The designer as a workspace pane item.
pub struct DesignerView {
    form_path: PathBuf,
    document: LfmDocument,
    canvas: Entity<FormCanvas>,
    project: Entity<Project>,
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,
    _canvas_subscription: Subscription,
}

impl DesignerView {
    pub fn new(
        form_path: PathBuf,
        document: LfmDocument,
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let canvas = cx.new(|_| FormCanvas::new(components_of(&document), document.root.bounds));
        let _canvas_subscription = cx.observe(&canvas, |_, _, cx| cx.notify());
        Self {
            form_path,
            document,
            canvas,
            project,
            workspace,
            focus_handle: cx.focus_handle(),
            _canvas_subscription,
        }
    }

    pub fn canvas(&self) -> &Entity<FormCanvas> {
        &self.canvas
    }

    fn selected_component(&self, cx: &App) -> Option<DesignerComponent> {
        let canvas = self.canvas.read(cx);
        let index = canvas.selection?;
        canvas.components.get(index).cloned()
    }

    fn place(&mut self, descriptor: ComponentDescriptor, cx: &mut Context<Self>) {
        let count = self.canvas.read(cx).components.len() as f32;
        let cascade = (count * 8.0) % 96.0;
        self.canvas.update(cx, |canvas, _| {
            place_component(
                canvas,
                &descriptor,
                (
                    24.0 + cascade,
                    24.0 + cascade,
                    DEFAULT_BOUNDS.2,
                    DEFAULT_BOUNDS.3,
                ),
            );
        });
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
        let text = serialize_lfm(&self.document);
        let fs = self.project.read(cx).fs().clone();
        let form_path = self.form_path.clone();
        let workspace = self.workspace.clone();
        cx.spawn(async move |_, cx| {
            if let Err(error) = fs.write(&form_path, text.as_bytes()).await {
                workspace
                    .update(cx, |workspace, cx| {
                        notify(workspace, format!("Cannot save form: {error:#}"), cx)
                    })
                    .ok();
            }
        })
        .detach();
    }

    fn render_sidebar(&self, selected: Option<DesignerComponent>, cx: &Context<Self>) -> Stateful<Div> {
        let mut sidebar = div()
            .id("designer-sidebar")
            .w(px(190.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .bg(rgb(0x222227))
            .border_l_1()
            .border_color(rgb(0x333338))
            .overflow_y_scroll();

        sidebar = sidebar.child(muted_label("Palette"));
        for descriptor in palette_components() {
            sidebar = sidebar.child(
                div()
                    .id(SharedString::from(format!(
                        "designer-palette-{}",
                        descriptor.label
                    )))
                    .on_click(cx.listener(move |this, _, _, cx| this.place(descriptor, cx)))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(0x2c2c31))
                    .text_size(px(12.0))
                    .text_color(rgb(0xd8d8d8))
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
                let mut keys: Vec<&String> = component.properties.keys().collect();
                keys.sort();
                for key in keys {
                    let Some(value) = component.properties.get(key) else {
                        continue;
                    };
                    sidebar = sidebar.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(0xa8a8b0))
                            .child(format!("{key} = {value}")),
                    );
                }
            }
            None => sidebar = sidebar.child(muted_label("Nothing selected")),
        }
        sidebar
    }
}

impl Render for DesignerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let file_name = file_display_name(&self.form_path);
        let selected = self.selected_component(cx);

        div()
            .id("designer-view")
            .track_focus(&self.focus_handle)
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
                            .id("designer-save")
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x2f6feb))
                            .text_size(px(12.0))
                            .text_color(rgb(0xffffff))
                            .child("Save form"),
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
}
