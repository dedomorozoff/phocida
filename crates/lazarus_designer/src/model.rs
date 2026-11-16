use crate::lfm::{LfmComponent, LfmDocument};
use std::collections::HashMap;

/// A component in the designer, as displayed and edited on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerComponent {
    /// Instance name, e.g. `Button1` in `object Button1: TButton`.
    pub name: String,
    /// LCL class name, e.g. `TButton`.
    pub class: String,
    /// `(left, top, width, height)`.
    pub bounds: (f32, f32, f32, f32),
    /// Declared properties keyed by Pascal property name.
    pub properties: HashMap<String, String>,
}

impl From<&LfmComponent> for DesignerComponent {
    fn from(source: &LfmComponent) -> Self {
        Self {
            name: source.name.clone(),
            class: source.class.clone(),
            bounds: source.bounds,
            properties: source.properties.clone(),
        }
    }
}

impl From<LfmComponent> for DesignerComponent {
    fn from(source: LfmComponent) -> Self {
        Self::from(&source)
    }
}

/// Extract the components of a parsed [LfmDocument] for the designer.
///
/// Returns the components declared inside the root form, in source order,
/// excluding the root form itself.
pub fn components_of(doc: &LfmDocument) -> Vec<DesignerComponent> {
    doc.components.iter().map(DesignerComponent::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfm::parse_lfm;

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
  object Edit1: TEdit
    Left = 200
    Top = 40
    Width = 120
    Height = 30
    Text = 'Name'
  end
end"#;

    #[test]
    fn extracts_components_with_order() {
        let doc = parse_lfm(SAMPLE).expect("sample lfm parses");
        let components = components_of(&doc);
        assert_eq!(components.len(), 2);
        assert_eq!(components[0].name, "Button1");
        assert_eq!(components[0].class, "TButton");
        assert_eq!(components[0].bounds, (10.0, 20.0, 100.0, 30.0));
        assert_eq!(
            components[0].properties.get("Caption").map(String::as_str),
            Some("'Button1'")
        );
        assert_eq!(components[1].name, "Edit1");
        assert_eq!(components[1].class, "TEdit");
        assert_eq!(components[1].bounds, (200.0, 40.0, 120.0, 30.0));
        assert_eq!(
            components[1].properties.get("Text").map(String::as_str),
            Some("'Name'")
        );
    }
}
