//! Managed-form code that depends on the platform build or on the XML format.
//!
//! Modules here are named after the FULL version in which their subject first
//! appeared, never after a family nickname such as "85":
//!
//! - `layout_<platform>`: a stored form-body layout that platform introduced
//!   (`layout_8_5_1`: root revision 59 and the members 8.5.1 appends to the
//!   8.3 records), read by down-converting it to the previous layout (now
//!   the `ibcmd-form-body` crate's, re-exported here);
//! - `load_<platform>`: compiling a `Form.xml` into that layout;
//! - `xml_<format>_*`: code of one XML dialect, named by the XML format
//!   version (`xml_2_21_writer`, `xml_2_21_order`: what the 2.21 `Form.xml`
//!   adds to the 2.20 one).
//!
//! The `load_*` and `xml_*` modules are the form writer's, in the
//! `ibcmd-form-writer` crate (`ibcmd_form_writer::form`), and are re-exported
//! here; what reads the platform registry stays here.
//!
//! Which layout and which XML format a build uses is the platform registry's
//! answer (`crates/ibcmd-platform-spec`), not something to infer from a version number.
//! When a new build arrives (say 8.5.4):
//!
//! - nothing changed in its forms: one registry entry, no code here;
//! - its stored layout changed: a delta module `layout_8_5_4.rs` (and
//!   `load_8_5_4.rs`) converting between it and the 8.5.1 layout, chosen by
//!   the registry's `form_layout()`;
//! - its XML format changed (say to 2.22): `xml_2_22_*` modules beside the
//!   2.21 ones, chosen by the registry's `xml_version()`.

pub(super) use ibcmd_form_body::layout_8_5_1;
pub(super) use ibcmd_form_writer::form::{load_8_5_1, xml_2_21_writer};
// Only a test of the exporter reads the 2.21 child order.
#[cfg(test)]
pub(super) use ibcmd_form_writer::form::xml_2_21_order;

/// [`xml_2_21_writer::apply_form_facts_8_5_1`] for the platform the export
/// runs for: whether the build writes `ButtonImportance` `Main` on a default
/// button is the process-wide answer `platform::note_export_platform`
/// recorded at the command's start, read here and handed to the writer.
pub(super) fn apply_form_facts_8_5_1_for_export(
    xml: String,
    facts: &layout_8_5_1::FormFactsV8_5_1,
    object_refs: &std::collections::BTreeMap<String, String>,
    adoption: Option<&ibcmd_form_body::FormAdoption>,
) -> anyhow::Result<(String, Vec<ibcmd_form_body::FormItemAsset>)> {
    xml_2_21_writer::apply_form_facts_8_5_1(
        xml,
        facts,
        object_refs,
        adoption,
        crate::platform::export_writes_default_button_importance(),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ibcmd_form_body::layout_8_5_1::{FormFactsV8_5_1, FormItemFactsV8_5_1, Node};

    /// The export hands the writer the default-button spelling the process
    /// noted for its platform (fixed on this thread here): the very body the
    /// writer gives for that value, and the two values give two bodies.
    #[test]
    fn the_export_hands_the_writer_the_noted_platforms_default_button_spelling() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" version=\"2.21\">\r\n\
\t<ChildItems>\r\n\
\t\t<Button name=\"Default\" id=\"30\">\r\n\
\t\t\t<Type>UsualButton</Type>\r\n\
\t\t\t<DefaultButton>true</DefaultButton>\r\n\
\t\t\t<CommandName>Form.Command.Default</CommandName>\r\n\
\t\t</Button>\r\n\
\t</ChildItems>\r\n</Form>"
            .to_owned();
        // A button record of revision 34 whose appended importance member
        // (5) holds code 1, no importance of its own.
        let facts = FormFactsV8_5_1 {
            items: BTreeMap::from([(
                "30".to_owned(),
                FormItemFactsV8_5_1 {
                    revision: "34".to_owned(),
                    tail: (0..6)
                        .map(|index| Node::Leaf(if index == 5 { "1" } else { "0" }.to_owned()))
                        .collect(),
                    record: vec![Node::Leaf("0".to_owned()); 60],
                    ..FormItemFactsV8_5_1::default()
                },
            )]),
            ..FormFactsV8_5_1::default()
        };
        let mut bodies = Vec::new();
        for writes in [false, true] {
            let exported = crate::platform::with_export_default_button_importance(writes, || {
                super::apply_form_facts_8_5_1_for_export(
                    xml.clone(),
                    &facts,
                    &BTreeMap::new(),
                    None,
                )
            })
            .unwrap();
            let written = super::xml_2_21_writer::apply_form_facts_8_5_1(
                xml.clone(),
                &facts,
                &BTreeMap::new(),
                None,
                writes,
            )
            .unwrap();
            assert_eq!(exported.0, written.0);
            assert!(exported.1.is_empty() && written.1.is_empty());
            bodies.push(exported.0);
        }
        assert_eq!(bodies[0], xml);
        assert_ne!(bodies[0], bodies[1]);
    }
}
