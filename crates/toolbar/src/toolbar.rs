mod predicate;

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use gpui::{
    App, AppContext as _, Context, IntoElement, ParentElement, Render, Styled, Window, actions,
};
use icons::IconName;
use lazarus_project::LazarusFile;
use project::TaskSourceKind;
use task::{TaskContext, TaskTemplate};
use ui::{IconButton, Tooltip, prelude::*};
use workspace::{
    Toast, Workspace, notifications::NotificationId, tasks::ScheduledTaskResult,
};

pub use predicate::{default_mode_predicate, exe_name_from_project};

actions!(toolbar, [BuildProject, RunProject]);

/// Renders the Lazarus-style button row above the workspace docks.
pub struct Toolbar;

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("toolbar")
            .w_full()
            .px_2()
            .py_1()
            .gap_1()
            .items_center()
            .child(
                IconButton::new("toolbar-new", IconName::FileGeneric)
                    .tooltip(Tooltip::text("New file"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(workspace::NewFile), cx)
                    }),
            )
            .child(
                IconButton::new("toolbar-open", IconName::FolderOpen)
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
                IconButton::new("toolbar-save", IconName::FileGeneric)
                    .tooltip(Tooltip::text("Save"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(workspace::Save { save_intent: None }),
                            cx,
                        )
                    }),
            )
            .child(
                IconButton::new("toolbar-save-all", IconName::FileGeneric)
                    .tooltip(Tooltip::text("Save all"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(
                            Box::new(workspace::SaveAll { save_intent: None }),
                            cx,
                        )
                    }),
            )
            .child(
                IconButton::new("toolbar-build", IconName::ToolHammer)
                    .tooltip(Tooltip::text("Build project"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(BuildProject), cx)),
            )
            .child(
                IconButton::new("toolbar-run", IconName::PlayFilled)
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

        workspace.register_action(|_workspace, _: &BuildProject, window, cx| {
            build_then(window, cx, |_, _, _| {});
        });

        workspace.register_action(|_workspace, _: &RunProject, window, cx| {
            build_then(window, cx, |workspace, window, cx| {
                if let Err(error) = launch_exe(workspace, window, cx) {
                    notify(workspace, format!("Cannot run project: {error:#}"), cx);
                }
            });
        });
    })
    .detach();
}

/// Builds the project's default lazbuild task and calls `after_success` once the
/// build finished successfully.
fn build_then(
    window: &mut Window,
    cx: &mut Context<Workspace>,
    after_success: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
) {
    let build = tasks_ui::spawn_task_awaited(
        |(_, template)| default_mode_predicate(template),
        window,
        cx,
    );
    cx.spawn_in(window, async move |workspace, cx| {
        let outcome = build.await;
        workspace
            .update_in(cx, |workspace, window, cx| match outcome {
                Ok(Some(ScheduledTaskResult::Success)) => after_success(workspace, window, cx),
                Ok(Some(result)) => notify(workspace, format!("Build failed: {result:?}"), cx),
                Ok(None) => window.dispatch_action(
                    Box::new(tasks_ui::Spawn::ViaModal {
                        reveal_target: None,
                    }),
                    cx,
                ),
                Err(error) => notify(workspace, format!("Build failed: {error:#}"), cx),
            })
            .ok();
    })
    .detach();
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
    let lpi_path = find_lpi_path(workspace, cx).context("open a .lpi project first")?;
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

/// Finds the `.lpi` project among the open items.
fn find_lpi_path(workspace: &Workspace, cx: &App) -> Option<PathBuf> {
    let project = workspace.project().read(cx);
    workspace
        .panes()
        .iter()
        .flat_map(|pane| pane.read(cx).items())
        .filter_map(|item| item.project_path(cx))
        .find_map(|path| {
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
}
