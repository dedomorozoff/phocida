mod predicate;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use gpui::{
    App, AppContext as _, Context, IntoElement, ParentElement, Render, Styled, Window, actions, px,
};
use icons::IconName;
use lazarus_project::LazarusFile;
use project::TaskSourceKind;
use task::{TaskContext, TaskTemplate};
use ui::{IconButton, IconButtonShape, Tooltip, prelude::*};
use workspace::{
    Toast, Workspace, notifications::NotificationId, tasks::ScheduledTaskResult,
};

use predicate::{lazbuild_template, lpi_for_build};

pub use predicate::{default_mode_predicate, exe_name_from_project};

actions!(
    toolbar,
    [
        BuildProject,
        RunProject,
        /// Open the form designer for the active file's form.
        DesignProject
    ]
);

/// Renders the Lazarus-style button row above the workspace docks.
pub struct Toolbar;

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        h_flex()
            .id("toolbar")
            .w_full()
            .h(px(34.))
            .flex_none()
            .px_1()
            .gap_0p5()
            .items_center()
            .bg(colors.title_bar_background)
            .border_b_1()
            .border_color(colors.border)
            .child(
                IconButton::new("toolbar-new", IconName::FileGeneric)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("New file"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(workspace::NewFile), cx)
                    }),
            )
            .child(
                IconButton::new("toolbar-open", IconName::FolderOpen)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Open"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(workspace::Open {
                                create_new_window: None,
                            }),
                            cx,
                        )
                    }),
            )
            .child(
                IconButton::new("toolbar-save", IconName::Save)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Save"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(workspace::Save { save_intent: None }),
                            cx,
                        )
                    }),
            )
            .child(
                IconButton::new("toolbar-save-all", IconName::SaveAll)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Save all"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(workspace::SaveAll { save_intent: None }),
                            cx,
                        )
                    }),
            )
            .child(
                IconButton::new("toolbar-design", IconName::Blocks)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Design form"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(DesignProject), cx)),
            )
            .child(
                IconButton::new("toolbar-build", IconName::ToolHammer)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Build project"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(BuildProject), cx)),
            )
            .child(
                IconButton::new("toolbar-run", IconName::PlayFilled)
                    .shape(IconButtonShape::Square)
                    .tooltip(Tooltip::text("Run project"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(RunProject), cx)),
            )
    }
}

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };
        let item = cx.new(|_cx| Toolbar);
        workspace.set_toolbar_item(item.into(), window, cx);

        workspace.register_action(|workspace, _: &BuildProject, window, cx| {
            build_then(workspace, window, cx, |_, _, _| {});
        });

        workspace.register_action(|workspace, _: &RunProject, window, cx| {
            build_then(workspace, window, cx, |workspace, window, cx| {
                if let Err(error) = launch_exe(workspace, window, cx) {
                    notify(workspace, format!("Cannot run project: {error:#}"), cx);
                }
            });
        });

        workspace.register_action(|workspace, _: &DesignProject, window, cx| {
            lazarus_designer::open_designer_for_active_item(workspace, window, cx);
        });
    })
    .detach();
}

/// Builds the project's default lazbuild task and calls `after_success` once the
/// build finished successfully. The task is constructed directly from the
/// discovered `.lpi` — the task list only exposes templates of the active
/// buffer's language, so with a `.lpr` active the lazbuild templates would never
/// match. Without an `.lpi` the task modal is the fallback.
fn build_then(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
    after_success: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
) {
    let lpi_path = lpi_for_workspace(workspace, cx);
    cx.spawn_in(window, async move |workspace, cx| {
        let Some(lpi_path) = lpi_path else {
            workspace
                .update_in(cx, |_, window, cx| {
                    window.dispatch_action(
                        Box::new(tasks_ui::Spawn::ViaModal {
                            reveal_target: None,
                        }),
                        cx,
                    )
                })
                .ok();
            return;
        };

        let template = match build_template(&lpi_path) {
            Ok(template) => template,
            Err(error) => {
                workspace
                    .update(cx, |workspace, cx| {
                        notify(workspace, format!("Build failed: {error:#}"), cx)
                    })
                    .ok();
                return;
            }
        };

        let task_contexts = match workspace
            .update_in(cx, |workspace, window, cx| {
                tasks_ui::task_contexts(workspace, window, cx)
            }) {
            Ok(contexts) => contexts.await,
            Err(_) => return,
        };

        let Some(resolved) =
            task_contexts.resolve_task(&TaskSourceKind::UserInput, &template)
        else {
            workspace
                .update(cx, |workspace, cx| {
                    notify(
                        workspace,
                        "Build failed: cannot resolve the lazbuild task".to_owned(),
                        cx,
                    )
                })
                .ok();
            return;
        };

        let (tx, rx) = futures::channel::oneshot::channel();
        let scheduled = workspace.update_in(cx, |workspace, window, cx| {
            workspace.schedule_resolved_task_with_completion(
                TaskSourceKind::UserInput,
                resolved,
                false,
                move |result, _cx| {
                    tx.send(result).ok();
                },
                window,
                cx,
            );
        });
        if scheduled.is_err() {
            return;
        }
        let outcome = rx.await.unwrap_or(ScheduledTaskResult::Cancelled);
        workspace
            .update_in(cx, |workspace, window, cx| match outcome {
                ScheduledTaskResult::Success => after_success(workspace, window, cx),
                result => notify(workspace, format!("Build failed: {result:?}"), cx),
            })
            .ok();
    })
    .detach();
}

/// Parses the `.lpi` and builds the default-mode lazbuild task for it.
fn build_template(lpi_path: &Path) -> anyhow::Result<TaskTemplate> {
    let text = std::fs::read_to_string(lpi_path)
        .with_context(|| format!("cannot read {}", lpi_path.display()))?;
    let parsed = match lazarus_project::parse(&text) {
        Ok(LazarusFile::Project(project)) => project,
        Ok(LazarusFile::Package) => bail!("{} is a package, not a project", lpi_path.display()),
        Err(error) => bail!("cannot parse {}: {error}", lpi_path.display()),
    };
    Ok(lazbuild_template(&parsed, lpi_path))
}

fn notify(workspace: &mut Workspace, message: String, cx: &mut Context<Workspace>) {
    workspace.show_toast(Toast::new(NotificationId::named("toolbar".into()), message), cx);
}

/// Starts the executable the build produced as a task in the terminal panel, so
/// it gets a tab, history entry and rerun like any other task.
fn launch_exe(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> anyhow::Result<()> {
    let lpi_path = lpi_for_workspace(workspace, cx).context("open a .lpi project first")?;
    let text = std::fs::read_to_string(&lpi_path)
        .with_context(|| format!("cannot read {}", lpi_path.display()))?;
    let parsed = match lazarus_project::parse(&text) {
        Ok(LazarusFile::Project(project)) => project,
        Ok(LazarusFile::Package) => bail!("{} is a package, not a project", lpi_path.display()),
        Err(error) => bail!("cannot parse {}: {error}", lpi_path.display()),
    };
    let exe = exe_name_from_project(&parsed, &lpi_path.to_string_lossy())
        .context("cannot name the project executable")?;
    let directory = lpi_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let template = TaskTemplate {
        label: format!("run {exe}"),
        command: directory.join(exe).to_string_lossy().into_owned(),
        cwd: Some(directory.to_string_lossy().into_owned()),
        ..TaskTemplate::default()
    };
    workspace.schedule_task(
        TaskSourceKind::UserInput,
        &template,
        &TaskContext::default(),
        false,
        window,
        cx,
    );
    Ok(())
}

/// Finds the `.lpi` project for the toolbar actions: an open `.lpi` item wins,
/// otherwise the `.lpi` next to the active `.lpr`/`.lpp`/`.dpr` buffer.
fn lpi_for_workspace(workspace: &Workspace, cx: &App) -> Option<PathBuf> {
    let project = workspace.project().read(cx);
    let open_lpis: Vec<PathBuf> = workspace
        .panes()
        .iter()
        .flat_map(|pane| pane.read(cx).items())
        .filter_map(|item| item.project_path(cx))
        .filter_map(|path| {
            if !path
                .path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lpi"))
            {
                return None;
            }
            let worktree = project.worktree_for_id(path.worktree_id, cx)?;
            Some(worktree.read(cx).abs_path().join(path.path.as_std_path()))
        })
        .collect();
    let active_path = workspace
        .active_item(cx)
        .and_then(|item| item.project_path(cx))
        .and_then(|path| {
            let worktree = project.worktree_for_id(path.worktree_id, cx)?;
            Some(worktree.read(cx).abs_path().join(path.path.as_std_path()))
        });
    lpi_for_build(&open_lpis, active_path.as_deref())
}

#[cfg(test)]
mod design_tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use fs::FakeFs;
    use gpui::TestAppContext;
    use project::Project;
    use serde_json::json;
    use workspace::{AppState, MultiWorkspace};

    use super::{DesignProject, init};
    use lazarus_designer::DesignerView;

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

    #[gpui::test]
    async fn design_project_opens_the_designer_for_an_lpr(cx: &mut TestAppContext) {
        let app_state = cx.update(|cx| {
            let state = AppState::test(cx);
            init(cx);
            editor::init(cx);
            state
        });
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
        let (multi_workspace, cx) =
            cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));
        let workspace = multi_workspace
            .read_with(cx, |multi_workspace, _| multi_workspace.workspace().clone());

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

        workspace.update_in(cx, |_, window, cx| {
            window.dispatch_action(Box::new(DesignProject), cx);
        });
        cx.run_until_parked();

        workspace.read_with(cx, |workspace, cx| {
            let item = workspace.active_item(cx).expect("an active item");
            let designer = item
                .downcast::<DesignerView>()
                .expect("DesignProject opens the designer for the .lpr");
            let canvas = designer.read(cx).canvas().read(cx);
            assert_eq!(canvas.components.len(), 1);
            assert_eq!(canvas.components[0].name, "Button1");
        });
        assert!(
            cx.debug_bounds("designer-component-0").is_some(),
            "the form component is rendered"
        );
    }
}
