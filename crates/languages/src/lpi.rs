use gpui::{App, Entity, Task};
use language::{Buffer, ContextProvider, File as _};
use lazarus_project::{LazarusFile, LazarusProject, parse};
use std::path::Path;
use task::{TaskTemplate, TaskTemplates};

pub(super) fn lazbuild_task_templates(
    parsed: &LazarusProject,
    relative_path: &str,
) -> TaskTemplates {
    let title = match parsed.title.as_deref() {
        Some(title) => title.to_owned(),
        None => parsed
            .main_unit
            .as_deref()
            .and_then(|unit| Path::new(unit).file_stem())
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_owned()),
    };

    let mut templates = TaskTemplates(Vec::new());
    for (index, mode) in parsed.build_modes.iter().enumerate() {
        let default_mode = index == 0;
        let mut args = Vec::with_capacity(2);
        if !default_mode {
            args.push(format!("--build-mode={mode}"));
        }
        args.push(relative_path.to_owned());
        let label = if default_mode {
            format!("build project '{title}' with lazbuild")
        } else {
            format!("build project '{title}' with lazbuild ({mode})")
        };
        templates.0.push(TaskTemplate {
            label,
            command: "lazbuild".to_owned(),
            args,
            ..TaskTemplate::default()
        });
    }
    templates
}

pub(super) struct LpiContextProvider;

impl ContextProvider for LpiContextProvider {
    fn associated_tasks(
        &self,
        buffer: Option<Entity<Buffer>>,
        cx: &App,
    ) -> Task<Option<TaskTemplates>> {
        let Some(buffer) = buffer else {
            return Task::ready(None);
        };
        let text = buffer.read(cx).text().to_string();
        let Some(file) = project::File::from_dyn(buffer.read(cx).file()).cloned() else {
            return Task::ready(None);
        };
        let relative_path = format!("{}", file.path());
        match parse(&text) {
            Ok(LazarusFile::Project(project)) => {
                Task::ready(Some(lazbuild_task_templates(&project, &relative_path)))
            }
            Ok(LazarusFile::Package) | Err(_) => Task::ready(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(title: Option<&str>, main_unit: Option<&str>, modes: &[&str]) -> LazarusProject {
        LazarusProject {
            title: title.map(str::to_owned),
            main_unit: main_unit.map(str::to_owned),
            build_modes: modes.iter().map(|mode| mode.to_string()).collect(),
        }
    }

    #[test]
    fn task_labels_match_spec() {
        let parsed = project(Some("app1"), Some("project1.lpr"), &["Debug", "Release"]);
        let templates = lazbuild_task_templates(&parsed, "src/project1.lpi");
        assert_eq!(templates.0.len(), 2);
        assert_eq!(
            templates.0[0].label,
            "build project 'app1' with lazbuild"
        );
        assert_eq!(templates.0[0].args, ["src/project1.lpi"]);
        assert_eq!(
            templates.0[1].label,
            "build project 'app1' with lazbuild (Release)"
        );
        assert_eq!(templates.0[1].args, ["--build-mode=Release", "src/project1.lpi"]);
    }

    #[test]
    fn title_falls_back_to_main_unit() {
        let parsed = project(None, Some("project1.lpr"), &["Debug", "Release"]);
        let templates = lazbuild_task_templates(&parsed, "src/project1.lpi");
        assert_eq!(
            templates.0[0].label,
            "build project 'project1' with lazbuild"
        );
        assert_eq!(
            templates.0[1].label,
            "build project 'project1' with lazbuild (Release)"
        );
    }

    #[test]
    fn default_mode_yields_one_task() {
        let parsed = project(Some("app1"), None, &["Default"]);
        let templates = lazbuild_task_templates(&parsed, "project1.lpi");
        assert_eq!(templates.0.len(), 1);
        assert!(
            templates
                .0
                .iter()
                .all(|task| !task.args.iter().any(|arg| arg.starts_with("--build-mode")))
        );
    }

    #[test]
    fn three_modes_each_get_a_task() {
        let parsed = project(
            Some("app1"),
            Some("project1.lpr"),
            &["Default", "Nightly", "Release"],
        );
        let templates = lazbuild_task_templates(&parsed, "project1.lpi");
        assert_eq!(templates.0.len(), 3);
        assert_eq!(templates.0[0].args, ["project1.lpi"]);
        assert_eq!(
            templates.0[1].args,
            ["--build-mode=Nightly", "project1.lpi"]
        );
        assert_eq!(
            templates.0[2].args,
            ["--build-mode=Release", "project1.lpi"]
        );
    }
}
