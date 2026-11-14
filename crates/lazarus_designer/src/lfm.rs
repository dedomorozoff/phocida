use anyhow::{Result, bail};
use std::collections::HashMap;

/// Position and size of a component in LFM coordinates: `(left, top, width, height)`.
pub type Bounds = (f32, f32, f32, f32);

/// A component declared in an LFM form.
#[derive(Debug, Clone, PartialEq)]
pub struct LfmComponent {
    /// Instance name, e.g. `Button1` in `object Button1: TButton`.
    pub name: String,
    /// LCL class name, e.g. `TButton`.
    pub class: String,
    /// `(left, top, width, height)`.
    pub bounds: Bounds,
    /// Declared properties keyed by Pascal property name.
    pub properties: HashMap<String, String>,
}

/// The parsed content of an `.lfm` form module.
#[derive(Debug, Clone, PartialEq)]
pub struct LfmDocument {
    /// The root form object, e.g. `object Form1: TForm1`.
    pub root: LfmComponent,
    /// Components declared inside the root form, in source order.
    pub components: Vec<LfmComponent>,
}

/// Parse an `.lfm` form module into a [`LfmDocument`].
pub fn parse_lfm(text: &str) -> Result<LfmDocument> {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let mut stack: Vec<Block> = Vec::new();
    let mut root_block: Option<Block> = None;

    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].as_str();
        let first_word = first_word(line);
        if first_word.eq_ignore_ascii_case("object") {
            let block = parse_object_line(line)?;
            stack.push(block);
        } else if first_word.eq_ignore_ascii_case("end") {
            let closed = stack.pop().ok_or_else(|| {
                anyhow::anyhow!("unexpected 'end' without a matching 'object' in lfm")
            })?;
            if stack.is_empty() {
                root_block = Some(closed);
            } else if let Some(parent) = stack.last_mut() {
                parent.children.push(closed);
            }
        } else {
            let (key, mut value) = split_property(line)?;
            while paren_depth(&value) > 0 && index + 1 < lines.len() {
                index += 1;
                value.push(' ');
                value.push_str(&lines[index]);
            }
            let block = stack
                .last_mut()
                .ok_or_else(|| anyhow::anyhow!("lfm property outside of an object: {line}"))?;
            block.properties.insert(key, value);
        }
        index += 1;
    }

    if !stack.is_empty() {
        bail!("unterminated object block in lfm");
    }
    let root_block = root_block.ok_or_else(|| anyhow::anyhow!("lfm has no object declaration"))?;
    let mut flat = Vec::new();
    collect_components(&root_block, &mut flat);

    Ok(LfmDocument {
        root: to_component(&root_block),
        components: flat.into_iter().map(|block| to_component(&block)).collect(),
    })
}

/// Serialize a [`LfmDocument`] back into an `.lfm` form module.
pub fn serialize_lfm(doc: &LfmDocument) -> String {
    let mut out = format!("object {}: {}\n", doc.root.name, doc.root.class);
    write_properties(&mut out, &doc.root.properties, 1);
    for component in &doc.components {
        out.push_str(&format!(
            "\nobject {}: {}\n",
            component.name, component.class
        ));
        write_properties(&mut out, &component.properties, 2);
        out.push_str("  end\n");
    }
    out.push_str("end\n");
    out
}

#[derive(Debug, Clone)]
struct Block {
    name: String,
    class: String,
    properties: HashMap<String, String>,
    children: Vec<Block>,
}

fn first_word(line: &str) -> &str {
    let end = line.find(|c: char| c.is_whitespace()).unwrap_or(line.len());
    &line[..end]
}

fn parse_object_line(line: &str) -> Result<Block> {
    let rest = line[first_word(line).len()..].trim();
    let (name, class) = match rest.split_once(':') {
        Some((name, class)) => (
            name.trim().to_string(),
            class
                .trim()
                .split('[')
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
        ),
        None => (rest.trim().to_string(), String::new()),
    };
    if name.is_empty() {
        bail!("lfm object without a name: {line}");
    }
    Ok(Block {
        name,
        class,
        properties: HashMap::new(),
        children: Vec::new(),
    })
}

fn split_property(line: &str) -> Result<(String, String)> {
    let Some((key, value)) = line.split_once('=') else {
        bail!("invalid lfm property line: {line}");
    };
    let key = key.trim().to_string();
    if key.is_empty() {
        bail!("lfm property without a name: {line}");
    }
    Ok((key, value.trim().to_string()))
}

fn paren_depth(value: &str) -> usize {
    let opens = value.chars().filter(|c| *c == '(').count();
    let closes = value.chars().filter(|c| *c == ')').count();
    opens.saturating_sub(closes)
}

fn collect_components(block: &Block, out: &mut Vec<Block>) {
    for child in &block.children {
        out.push(child.clone());
        collect_components(child, out);
    }
}

fn to_component(block: &Block) -> LfmComponent {
    LfmComponent {
        name: block.name.clone(),
        class: block.class.clone(),
        bounds: component_bounds(&block.properties),
        properties: block.properties.clone(),
    }
}

fn component_bounds(properties: &HashMap<String, String>) -> Bounds {
    let left = property_number(properties, "Left");
    let top = property_number(properties, "Top");
    let width = property_number(properties, "Width");
    let height = property_number(properties, "Height");
    (left, top, width, height)
}

fn property_number(properties: &HashMap<String, String>, key: &str) -> f32 {
    let Some(value) = properties.get(key) else {
        return 0.0;
    };
    let cleaned = value
        .trim()
        .trim_matches(|c: char| c.is_whitespace() || c == '\'');
    cleaned.parse::<f32>().unwrap_or(0.0)
}

fn write_properties(out: &mut String, properties: &HashMap<String, String>, indent: usize) {
    let mut keys: Vec<&String> = properties.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = properties.get(key) {
            out.push_str(&format!("{}{} = {}\n", "  ".repeat(indent), key, value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"object Form1: TForm1
  Left = 0
  Top = 0
  Width = 320
  Height = 240
  Caption = 'Form1'
  object Button1: TButton
    Left = 10
    Top = 20
    Width = 100
    Height = 30
    Caption = 'Button1'
  end
end"#;

    #[test]
    fn parses_component_bounds() {
        let doc = parse_lfm(SAMPLE).expect("sample lfm parses");
        assert_eq!(doc.root.name, "Form1");
        assert_eq!(doc.root.class, "TForm1");
        assert_eq!(doc.components.len(), 1);
        let button = &doc.components[0];
        assert_eq!(button.name, "Button1");
        assert_eq!(button.class, "TButton");
        assert_eq!(button.bounds, (10.0, 20.0, 100.0, 30.0));
    }

    #[test]
    fn malformed_lfm_is_error_not_panic() {
        assert!(parse_lfm("object Form1: TForm1\n  Left = 0\n").is_err());
        assert!(parse_lfm("not an lfm at all").is_err());
        assert!(parse_lfm("").is_err());
    }

    #[test]
    fn serialize_then_parse_round_trips() {
        let doc = parse_lfm(SAMPLE).expect("sample lfm parses");
        let text = serialize_lfm(&doc);
        let reparsed = parse_lfm(&text).expect("serialized lfm parses");
        assert_eq!(reparsed, doc);
    }
}
