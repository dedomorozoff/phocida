use std::path::{Path, PathBuf};

use lazarus_project::LazarusProject;
use task::TaskTemplate;

/// Tag carried by the lazbuild task templates that the `.lpi` language provider
/// contributes.
pub const LAZBUILD_TAG: &str = "lazbuild";

/// Selects the lazbuild task for the project's default build mode: it must come
/// from the `.lpi` provider and must not pin a `--build-mode`.
pub fn default_mode_predicate(template: &TaskTemplate) -> bool {
    template.tags.iter().any(|tag| tag == LAZBUILD_TAG)
        && !template
            .args
            .iter()
            .any(|arg| arg.starts_with("--build-mode"))
}

/// Names the executable lazbuild produces for this project: the main unit's
/// stem when the project declares one, otherwise the `.lpi` file's stem.
pub fn exe_name_from_project(parsed: &LazarusProject, lpi_path: &str) -> Option<String> {
    let stem = parsed
        .main_unit
        .as_deref()
        .and_then(|main_unit| Path::new(main_unit).file_stem())
        .or_else(|| Path::new(lpi_path).file_stem())?;
    Some(format!("{}.exe", stem.to_string_lossy()))
}

/// Builds the default-mode lazbuild task for the parsed project. The first
/// build mode is the project default, so it never gets a `--build-mode` flag.
pub fn lazbuild_template(project: &LazarusProject, lpi_path: &Path) -> TaskTemplate {
    let title = project
        .title
        .as_deref()
        .map(str::to_owned)
        .or(project
            .main_unit
            .as_deref()
            .and_then(|unit| Path::new(unit).file_stem())
            .map(|stem| stem.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_owned());
    let mut args = Vec::with_capacity(2);
    if let Some(mode) = project.build_modes.get(1) {
        args.push(format!("--build-mode={mode}"));
    }
    args.push(lpi_path.to_string_lossy().into_owned());
    TaskTemplate {
        label: format!("build project '{title}' with lazbuild"),
        command: "lazbuild".to_owned(),
        args,
        tags: vec![LAZBUILD_TAG.to_owned()],
        ..TaskTemplate::default()
    }
}

/// Finds the project file for the toolbar build: an open `.lpi` item wins,
/// otherwise the `.lpi` next to the active `.lpr`/`.lpp`/`.dpr` buffer.
pub fn lpi_for_build(open_lpis: &[PathBuf], active_buffer: Option<&Path>) -> Option<PathBuf> {
    open_lpis
        .first()
        .cloned()
        .or_else(|| active_buffer.and_then(lpi_sibling))
}

fn lpi_sibling(buffer_path: &Path) -> Option<PathBuf> {
    let extension = buffer_path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(extension.as_str(), "lpr" | "lpp" | "dpr") {
        return None;
    }
    let mut sibling = buffer_path.to_path_buf();
    sibling.set_extension("lpi");
    Some(sibling)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(tags: &[&str], args: &[&str]) -> TaskTemplate {
        TaskTemplate {
            label: "build".to_owned(),
            command: "lazbuild".to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            ..TaskTemplate::default()
        }
    }

    fn project(main_unit: Option<&str>) -> LazarusProject {
        LazarusProject {
            title: Some("app1".to_owned()),
            main_unit: main_unit.map(str::to_owned),
            build_modes: vec!["Debug".to_owned()],
        }
    }

    #[test]
    fn default_mode_predicate_skips_flagged() {
        assert!(!default_mode_predicate(&template(
            &["lazbuild"],
            &["--build-mode=Release", "src/project1.lpi"]
        )));
        assert!(default_mode_predicate(&template(
            &["lazbuild"],
            &["src/project1.lpi"]
        )));
        assert!(!default_mode_predicate(&template(
            &[],
            &["src/project1.lpi"]
        )));
    }

    #[test]
    fn exe_name_uses_main_unit() {
        assert_eq!(
            exe_name_from_project(&project(Some("project1.lpr")), "x/app.lpi").as_deref(),
            Some("project1.exe")
        );
    }

    #[test]
    fn exe_name_falls_back_to_lpi_stem() {
        assert_eq!(
            exe_name_from_project(&project(None), "src/project1.lpi").as_deref(),
            Some("project1.exe")
        );
    }

    #[test]
    fn run_without_lpi_is_an_error() {
        assert_eq!(exe_name_from_project(&project(None), ""), None);
    }

    #[test]
    fn lazbuild_template_builds_the_default_mode() {
        let parsed = project(Some("project1.lpr"));
        let template = lazbuild_template(&parsed, Path::new("src/project1.lpi"));

        assert_eq!(template.command, "lazbuild");
        assert_eq!(template.args, vec!["src/project1.lpi".to_owned()]);
        assert!(template.tags.iter().any(|tag| tag == LAZBUILD_TAG));
        assert_eq!(template.label, "build project 'app1' with lazbuild");
    }

    #[test]
    fn lazbuild_template_flags_non_default_mode() {
        // The project() fixture declares a single build mode; a second mode
        // must make the template carry a --build-mode flag.
        let mut modes = LazarusProject {
            title: None,
            main_unit: Some("project1.lpr".to_owned()),
            build_modes: vec!["Debug".to_owned()],
        };
        modes.build_modes.push("Release".to_owned());
        let template = lazbuild_template(&modes, Path::new("src/project1.lpi"));

        assert_eq!(
            template.args,
            vec!["--build-mode=Release".to_owned(), "src/project1.lpi".to_owned()]
        );
    }

    #[test]
    fn lpi_sibling_is_found_for_the_active_program_file() {
        assert_eq!(
            lpi_for_build(&[], Some(Path::new("C:/proj/laz/project1.lpr"))),
            Some(PathBuf::from("C:/proj/laz/project1.lpi"))
        );
        assert_eq!(lpi_for_build(&[], Some(Path::new("C:/proj/unit1.pas"))), None);
        assert_eq!(lpi_for_build(&[], None), None);
    }

    #[test]
    fn open_lpi_wins_over_the_sibling() {
        let open = vec![PathBuf::from("C:/proj/other.lpi")];
        assert_eq!(
            lpi_for_build(&open, Some(Path::new("C:/proj/laz/project1.lpr"))),
            Some(PathBuf::from("C:/proj/other.lpi"))
        );
    }
}
