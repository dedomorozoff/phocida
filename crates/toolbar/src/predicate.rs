use std::path::Path;

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
}
