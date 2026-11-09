use anyhow::{Result, bail};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug)]
pub enum LazarusFile {
    Project(LazarusProject),
    Package,
}

#[derive(Debug)]
pub struct LazarusProject {
    pub title: Option<String>,
    pub main_unit: Option<String>,
    pub build_modes: Vec<String>,
}

#[derive(Default)]
struct ProjectData {
    title: Option<String>,
    main_unit_index: Option<usize>,
    build_modes: Vec<String>,
    unit_files: Vec<(usize, String)>,
    has_project_options: bool,
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.local_name().as_ref() == key.as_bytes())
        .map(|attr| String::from_utf8_lossy(&attr.value).into_owned())
}

fn numeric_suffix(name: &str, prefix: &str) -> Option<usize> {
    name.strip_prefix(prefix)?.parse().ok()
}

/// Records what this element contributes, given the stack of open elements.
/// Returns true when the element marks the file as a package.
fn record(
    element: &BytesStart<'_>,
    name: &str,
    stack: &[String],
    data: &mut ProjectData,
) -> Result<bool> {
    let parent = stack.last().map(String::as_str);
    let grandparent = if stack.len() >= 2 {
        Some(stack[stack.len() - 2].as_str())
    } else {
        None
    };
    match (parent, grandparent) {
        (None, _) => {
            if name != "CONFIG" {
                bail!("expected a CONFIG root element, found {name}");
            }
        }
        (Some("CONFIG"), _) => {
            match name {
                "PackageOptions" => return Ok(true),
                "ProjectOptions" => data.has_project_options = true,
                _ => {}
            }
        }
        (Some("General"), Some("ProjectOptions")) => match name {
            "Title" => data.title = attribute(element, "Value"),
            "MainUnit" => {
                data.main_unit_index =
                    attribute(element, "Value").and_then(|value| value.parse().ok())
            }
            _ => {}
        },
        (Some("BuildModes"), Some("ProjectOptions")) => {
            if numeric_suffix(name, "Item").is_some()
                && let Some(mode) = attribute(element, "Name")
            {
                data.build_modes.push(mode);
            }
        }
        (Some(unit), Some("Units")) => {
            if name == "Filename"
                && let Some(index) = numeric_suffix(unit, "Unit")
                && let Some(file) = attribute(element, "Value")
            {
                data.unit_files.push((index, file));
            }
        }
        _ => {}
    }
    Ok(false)
}

pub fn parse(input: &str) -> Result<LazarusFile> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().check_end_names = true;

    let mut stack: Vec<String> = Vec::new();
    let mut saw_config = false;
    let mut data = ProjectData::default();

    loop {
        let element = match reader.read_event()? {
            Event::Start(element) => {
                let name = String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
                if name == "CONFIG" && stack.is_empty() {
                    saw_config = true;
                }
                if record(&element, &name, &stack, &mut data)? {
                    return Ok(LazarusFile::Package);
                }
                stack.push(name);
                continue;
            }
            Event::Empty(element) => element,
            Event::End(_) => {
                stack.pop();
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let name = String::from_utf8_lossy(element.local_name().as_ref()).into_owned();
        if record(&element, &name, &stack, &mut data)? {
            return Ok(LazarusFile::Package);
        }
    }

    if !saw_config {
        bail!("not a Lazarus project file");
    }
    if !data.has_project_options {
        bail!("CONFIG has neither ProjectOptions nor PackageOptions");
    }

    let main_unit_index = data.main_unit_index.unwrap_or(0);
    let main_unit = data
        .unit_files
        .iter()
        .find(|(index, _)| *index == main_unit_index)
        .map(|(_, file)| file.clone());
    if data.build_modes.is_empty() {
        data.build_modes.push("Default".to_owned());
    }

    Ok(LazarusFile::Project(LazarusProject {
        title: data.title,
        main_unit,
        build_modes: data.build_modes,
    }))
}

#[cfg(test)]
use crate::LazarusFile::{Package, Project};

#[cfg(test)]
const FIXTURE: &str = r#"<?xml version="1.0"?><CONFIG>
  <ProjectOptions>
    <Version Value="12"/>
    <General>
      <MainUnit Value="0"/>
      <Title Value="app1"/>
      <ProjectType Value="Application"/>
    </General>
    <BuildModes Count="2" Active="0">
      <Item1 Name="Debug" Default="True"/>
      <Item2 Name="Release"/>
    </BuildModes>
    <Units Count="1">
      <Unit0>
        <Filename Value="project1.lpr"/>
        <IsPartOfProject Value="True"/>
      </Unit0>
    </Units>
  </ProjectOptions>
</CONFIG>"#;

#[test]
fn parses_real_project() {
    let parsed = parse(FIXTURE).unwrap();
    let project = match parsed {
        Project(project) => project,
        Package => panic!("expected a project"),
    };
    assert_eq!(project.title.as_deref(), Some("app1"));
    assert_eq!(project.main_unit.as_deref(), Some("project1.lpr"));
    assert_eq!(project.build_modes, ["Debug", "Release"]);
}

#[test]
fn default_mode_without_build_modes() {
    let without_modes = FIXTURE.replacen("<BuildModes", "<!-- Modes Off", 1);
    let without_modes = without_modes
        .replacen("</BuildModes>", "</Modes Off--> ", 1)
        .replace("<!-- Modes Off", "")
        .replace("</Modes Off--> ", "");
    let project = match parse(&without_modes).unwrap() {
        Project(project) => project,
        Package => panic!("expected a project"),
    };
    assert_eq!(project.build_modes, ["Default"]);
    assert_eq!(project.main_unit.as_deref(), Some("project1.lpr"));
}

#[test]
fn main_unit_falls_back_to_zero() {
    let without_index = FIXTURE.replace("<MainUnit Value=\"0\"/>\n      ", "");
    let project = match parse(&without_index).unwrap() {
        Project(project) => project,
        Package => panic!("expected a project"),
    };
    assert_eq!(project.main_unit.as_deref(), Some("project1.lpr"));
}

#[test]
fn lpk_is_package() {
    assert!(matches!(
        parse(r#"<CONFIG><PackageOptions><Version Value="11"/></PackageOptions></CONFIG>"#),
        Ok(Package)
    ));
}

#[test]
fn malformed_is_an_error() {
    assert!(parse("not xml at all").is_err());
}

#[test]
fn malformed_config_without_options_is_not_a_project() {
    // A CONFIG file with neither ProjectOptions nor PackageOptions is not
    // something we can honestly offer lazbuild tasks for.
    assert!(parse(r#"<CONFIG><Version Value="12"/></CONFIG>"#).is_err());
}
