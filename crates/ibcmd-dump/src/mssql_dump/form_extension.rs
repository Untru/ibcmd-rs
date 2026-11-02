//! A form an extension adopts: its event interceptors (`callType`) and the
//! base form it adopted (`<BaseForm>`).
//!
//! An event block stores every handler of every event:
//! `{N,(event,"first handler")xN,1,0,(event,code,n,("handler",code)x(n-1))xN}`;
//! code `0` is `Before`, `1` `After`, `2` `Override`. A configuration's form
//! stores the same block with one handler per event and writes no call type;
//! an adopted form (and the base form inside it) writes one on every event
//! (fixture `adopted/form_events`: `Before`/`After`/`Override` interceptors,
//! the base form's own events `Before`; ИТК: an interceptor with an empty
//! first handler and one `After` handler).
//!
//! The adopted form's body ends `…,{0,0}x4,1,<base form body>,0` where a
//! form body ends `…,{0,0}x4,0,0`; the base form body is a whole form body.
//!
//! Both are facts the form writer spells on its own pass ([`FormAdoption`]):
//! the call type travels with every event and command of the form's model
//! ([`FormAdoption::mark`]), and the base form, written as a document of its
//! own by the same writer ([`form_adoption`]), closes the adopted form's
//! document as `<BaseForm>`. The model is the `ibcmd-form-body` crate's, with
//! the form reader and the constructor of an adopted form that keeps no base
//! form record (`adoption_without_base_form`); the constructors that read
//! `module_blob` and the 2.21 writer are here.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

use super::*;

// The base form record's slot, the adoption model the form writer spells
// and its constructor for a form without a base form record are the
// `ibcmd-form-body` crate's; the extension's form check reaches them here,
// as before. The constructors below read the root (`module_blob`, the 2.21
// writer).
use ibcmd_form_body::{CALL_TYPES, adopted_base_record_slot};
pub(super) use ibcmd_form_body::{
    FormAdoption, FormBaseForm, adoption_without_base_form, form_event_call_types,
};

/// `FormBaseForm::from_document`, the root's: the base form's document is
/// read with the 2.21 writer's `XmlEdits`.
trait FormBaseFormExt: Sized {
    /// The written document `xml` of the base form, in the dialect
    /// `source_version`.
    fn from_document(source_version: InfobaseConfigSourceVersion, xml: &str) -> Result<Self>;
}

impl FormBaseFormExt for FormBaseForm {
    fn from_document(source_version: InfobaseConfigSourceVersion, xml: &str) -> Result<Self> {
        let document = super::form::xml_2_21_writer::XmlEdits::new(xml)
            .context("the adopted form's base form document")?;
        let root = document.root()?;
        Ok(Self {
            version: source_version.as_str(),
            children: document.inner_lines(root).to_owned(),
        })
    }
}

/// The base form body an adopted form carries, when it is one.
pub(super) fn adopted_base_form(body: &ParsedFormBodyBlob) -> Option<ParsedFormBodyBlob> {
    let base = body.trailing.get(adopted_base_record_slot(body)?)?.trim();
    ibcmd_form_body::parse_form_body_plain(base).ok()
}

/// The adoption facts of the form `body`; `None` for a form without a base
/// form record. The base form is written by the same writer as the form, in
/// the same dialect: `object_refs` is passed for dialect 2.21, whose base
/// form goes through the same 8.5 pass as the form itself, by its own facts
/// (8.5.1.1529 writes `WindowOpeningMode`/`Group` inside `<BaseForm>` too;
/// fixture `v85_extension/adopted_form_events`).
pub(super) fn form_adoption(
    body: &ParsedFormBodyBlob,
    context: &FormParseContext<'_>,
    source_version: InfobaseConfigSourceVersion,
    object_refs: &BTreeMap<String, String>,
    adopted: bool,
) -> Result<Option<FormAdoption>> {
    let Some(base) = adopted_base_form(body) else {
        return if adopted {
            adoption_without_base_form(&body.layout).map(Some)
        } else {
            Ok(None)
        };
    };
    let base_call_types = form_event_call_types(&base.layout);
    let v85 = source_version == InfobaseConfigSourceVersion::V2_21;
    let (base, facts) = if v85 && super::form::layout_8_5_1::is_form_body_8_5_1(&base) {
        let (converted, facts) = super::form::layout_8_5_1::down_convert_form_body_8_5_1(&base)
            .context("the adopted form's 8.5 base form")?;
        (converted, Some(facts))
    } else {
        (base, None)
    };
    // The base form's own handlers are interceptors too (fixture
    // `adopted/form_events`: every event of the base form says `Before`).
    let base_adoption = FormAdoption {
        call_types: base_call_types,
        command_call_type: None,
        base_form: None,
    };
    let base_xml = match extract_form_body_xml_from_body_detailed_timed(
        &base,
        context,
        Some(&base_adoption),
        None,
    ) {
        Some(DetailedFormBodyExtraction::Emitted { xml, .. }) => xml,
        _ => anyhow::bail!("the adopted form's base form is not readable"),
    };
    let base_xml = match (v85, &facts) {
        (false, _) => base_xml,
        (true, Some(facts)) => {
            super::form::apply_form_facts_8_5_1_for_export(
                base_xml,
                facts,
                object_refs,
                Some(&base_adoption),
            )?
            .0
        }
        (true, None) => super::form::xml_2_21_writer::apply_xml_2_21_upgrade_defaults(base_xml)?,
    };
    Ok(Some(FormAdoption {
        call_types: form_event_call_types(&body.layout),
        // `Before`.
        command_call_type: Some(CALL_TYPES[0]),
        base_form: Some(FormBaseForm::from_document(source_version, &base_xml)?),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(name: &str, handler: &str) -> FormBodyEvent {
        FormBodyEvent {
            name: name.to_owned(),
            handler: handler.to_owned(),
            call_type: None,
        }
    }

    #[test]
    fn an_adopted_form_without_a_base_record_carries_the_stored_call_types() {
        // Native 8.3.27.2214 blocks of an adopted common form without
        // BaseForm (`extension-form-interceptors`).
        for (call_type, block) in [
            (
                "Before",
                include_str!(
                    "../../../../tests/fixtures/native-evidence/extension-form-interceptors/before.block.txt"
                ),
            ),
            (
                "After",
                include_str!(
                    "../../../../tests/fixtures/native-evidence/extension-form-interceptors/after.block.txt"
                ),
            ),
            (
                "Override",
                include_str!(
                    "../../../../tests/fixtures/native-evidence/extension-form-interceptors/override.block.txt"
                ),
            ),
        ] {
            let adoption = adoption_without_base_form(block.trim()).unwrap();
            let mut events = vec![event("OnCreateAtServer", "ПриСозданииНаСервере")];
            let mut commands = Vec::new();
            adoption.mark(&mut events, None, &mut [], &mut commands);
            assert_eq!(events[0].call_type, Some(call_type), "{block}");
            assert_eq!(adoption.command_call_type, Some("Before"));
            assert!(adoption.base_form.is_none());
        }
    }

    #[test]
    fn a_malformed_interceptor_block_without_a_base_record_is_refused() {
        const EVENT: &str = "9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b";
        for tail in ["3,1", "0,2,\"Handler\",3", "0,0", "0,2", "0,1,extra"] {
            let block = format!("{{1,{EVENT},\"Handler\",1,0,{EVENT},{tail}}}");
            assert!(adoption_without_base_form(&block).is_err(), "{block}");
        }
    }

    #[test]
    fn the_base_form_is_the_children_of_its_root() {
        let document = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
                        <Form xmlns=\"x\" version=\"2.20\">\r\n\
                        \t<Attributes/>\r\n\
                        </Form>";
        let base =
            FormBaseForm::from_document(InfobaseConfigSourceVersion::V2_20, document).unwrap();
        assert_eq!(base.version, "2.20");
        assert_eq!(base.children, "\t<Attributes/>\r\n");
        let empty =
            FormBaseForm::from_document(InfobaseConfigSourceVersion::V2_21, "<Form>\r\n</Form>")
                .unwrap();
        assert_eq!((empty.version, empty.children.as_str()), ("2.21", ""));
        assert!(
            FormBaseForm::from_document(InfobaseConfigSourceVersion::V2_20, "<Other/>").is_err()
        );
    }
}
