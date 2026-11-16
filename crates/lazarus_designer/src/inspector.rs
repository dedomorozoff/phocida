//! Inspector: edits the properties of a placed component. Edits go straight
//! into the component's property map, which the LFM serializer writes back out,
//! keeping the `.lfm` the source of truth.

use crate::model::DesignerComponent;

/// Sets or replaces a property on a component. Pascal property names are
/// case-insensitive, so a set on `"caption"` updates an existing `"Caption"`
/// entry instead of adding a duplicate; the canonical first-seen casing is kept.
pub fn set_property(component: &mut DesignerComponent, key: &str, value: &str) {
    let canonical = component
        .properties
        .keys()
        .find(|existing| existing.eq_ignore_ascii_case(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned());
    component.properties.insert(canonical, value.to_owned());
}

/// Removes a property, matching case-insensitively.
pub fn unset_property(component: &mut DesignerComponent, key: &str) {
    let canonical = component
        .properties
        .keys()
        .find(|existing| existing.eq_ignore_ascii_case(key))
        .cloned();
    if let Some(canonical) = canonical {
        component.properties.remove(&canonical);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfm::{parse_lfm, serialize_lfm};
    use crate::model::components_of;

    fn sample_component() -> DesignerComponent {
        let doc =
            parse_lfm("object Form1: TForm1\n  Caption = 'Form1'\n  object Button1: TButton\n    Left = 10\n    Top = 20\n    Width = 100\n    Height = 30\n    Caption = 'Button1'\n  end\nend")
                .expect("sample lfm parses");
        components_of(&doc).remove(0)
    }

    #[test]
    fn set_property_adds_a_new_entry() {
        let mut component = sample_component();
        set_property(&mut component, "Enabled", "True");

        assert_eq!(
            component.properties.get("Enabled").map(String::as_str),
            Some("True")
        );
    }

    #[test]
    fn set_property_updates_case_insensitively() {
        let mut component = sample_component();
        set_property(&mut component, "caption", "'OK'");

        assert_eq!(
            component.properties.get("Caption").map(String::as_str),
            Some("'OK'")
        );
        assert!(!component.properties.keys().any(|key| key == "caption"));
    }

    #[test]
    fn unset_property_removes_case_insensitively() {
        let mut component = sample_component();
        unset_property(&mut component, "caption");

        assert!(!component.properties.contains_key("Caption"));
    }

    #[test]
    fn inspector_edit_round_trips_through_lfm() {
        let mut doc =
            parse_lfm("object Form1: TForm1\n  Caption = 'Form1'\n  object Button1: TButton\n    Left = 10\n    Top = 20\n    Width = 100\n    Height = 30\n    Caption = 'Button1'\n  end\nend")
                .expect("sample lfm parses");
        let mut component = components_of(&doc).remove(0);
        set_property(&mut component, "Caption", "'OK'");
        set_property(&mut component, "Enabled", "True");
        // The canvas keeps components as `DesignerComponent`s; serialize the
        // edited properties back by rebuilding the document root components.
        doc.components = vec![crate::lfm::LfmComponent {
            name: component.name.clone(),
            class: component.class.clone(),
            bounds: component.bounds,
            properties: component.properties.clone(),
        }];

        let text = serialize_lfm(&doc);
        let reparsed = parse_lfm(&text).expect("edited lfm parses");
        let button = &reparsed.components[0];
        assert_eq!(
            button.properties.get("Caption").map(String::as_str),
            Some("'OK'")
        );
        assert_eq!(
            button.properties.get("Enabled").map(String::as_str),
            Some("True")
        );
    }
}
