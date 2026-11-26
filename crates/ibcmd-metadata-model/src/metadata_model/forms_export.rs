//! The export direction for subsystems and forms: Subsystem, an object's Form
//! and Template, CommonForm. A stored row -> the object's XML DOM, and what
//! the row contributes to a name index.
//!
//! Each decoder walks the layout of `common.rs` backwards and reads its code
//! tables (a child module sees the private ones).
//!
//! Names: a top-level subsystem and a common form name themselves
//! (`Subsystem.X`, `CommonForm.X`). A form, a template and a nested subsystem
//! are owned: their full names start with their owner's, which their rows do
//! not hold (the owner's row lists them by uuid), so the name index names
//! them through their owner (`model_export`'s owner pass).

use anyhow::{Result, anyhow, bail};

use super::{
    FORM_TYPES, INTERFACE_COMPATIBILITY_MODES, STD_PICTURE_CODES, SUBSYSTEM_CHILDREN,
    TEMPLATE_TYPES, USE_PURPOSE_CLASS, USE_PURPOSES,
};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{
    Header, bool_text, code_text, header, header_elements, localized_element, metadata_ref_text,
};
use crate::metadata_model::export::{
    Build, ExportContext, NameIndex, ObjectNames, atom, el, item, leaf, list, number, short, string,
};
use crate::metadata_model::xml::Element;

/// Decodes one stored row of these kinds into the object's element.
pub(crate) fn decode(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    let root = list(row)?;
    if atom(item(root, 0)?)? != "1" {
        bail!("not a descriptor row: {}", short(row));
    }
    let payload = item(root, 1)?;
    match kind {
        "Subsystem" => subsystem(root, &context.names),
        "Form" => form(payload, context),
        "CommonForm" => common_form(payload, context),
        "Template" => template(payload),
        other => bail!("{other} is not a subsystem or form kind"),
    }
}

/// What a row contributes to a name index: a top-level subsystem or a
/// common form. A nested subsystem's name starts with its owner's, which only
/// the owner pass knows; so do a form's and a template's, which this refuses.
pub(crate) fn names(kind: &str, row: &Brace) -> Result<ObjectNames> {
    let root = list(row)?;
    let payload = item(root, 1)?;
    let head = match kind {
        "Subsystem" => header(item(record(payload, "22", kind)?, 1)?)?,
        "CommonForm" => {
            let fields = record(payload, "4", kind)?;
            header(item(list(item(fields, 1)?)?, 1)?)?
        }
        "Form" | "Template" => bail!(
            "an owned {kind}'s full name starts with its owner's, which its row does not hold"
        ),
        other => bail!("{other} is not a subsystem or form kind"),
    };
    Ok(ObjectNames {
        uuid: head.uuid.clone(),
        full_name: format!("{kind}.{}", head.name),
        children: Vec::new(),
        types: Vec::new(),
    })
}

/// The payload's fields, its version checked.
fn record<'a>(payload: &'a Brace, version: &str, kind: &str) -> Result<&'a [Brace]> {
    let fields = list(payload)?;
    if atom(item(fields, 0)?)? != version {
        bail!("not a {kind} record: {}", short(payload));
    }
    Ok(fields)
}

fn flag(name: &str, node: &Brace) -> Result<Element> {
    Ok(leaf(name, bool_text(node)?))
}

fn coded(name: &str, node: &Brace, table: &[(&'static str, i64)]) -> Result<Element> {
    Ok(leaf(name, code_text(node, table)?))
}

/// `<Properties>` starting with the header's `Name`, `Synonym`, `Comment`.
fn header_properties(head: &Header) -> Result<Element> {
    let [name, synonym, comment] = header_elements(head)?;
    Ok(el("Properties").child(name).child(synonym).child(comment))
}

// ---------------------------------------------------------------------------
// Subsystem

/// `{1,{22,<header>,help,{0,0},in interface,<picture>,<explanation>,
/// {0,N,<content refs>},one command},1,{<children class>,K,<uuids>}}`
fn subsystem(root: &[Brace], names: &NameIndex) -> Result<Element> {
    let fields = record(item(root, 1)?, "22", "Subsystem")?;
    let head = header(item(fields, 1)?)?;
    let stored = list(item(fields, 7)?)?;
    if atom(item(stored, 0)?)? != "0" {
        bail!("bad subsystem content {}", short(item(fields, 7)?));
    }
    let count = number(item(stored, 1)?)? as usize;
    let mut content = el("Content");
    for reference in stored
        .get(2..2 + count)
        .ok_or_else(|| anyhow!("short subsystem content"))?
    {
        content.children.push(
            leaf("xr:Item", metadata_ref_text(reference, names)?).attr("type", "xr:MDObjectRef"),
        );
    }
    let properties = header_properties(&head)?
        .child(flag("IncludeHelpInContents", item(fields, 2)?)?)
        .child(flag("IncludeInCommandInterface", item(fields, 4)?)?)
        .child(flag("UseOneCommand", item(fields, 8)?)?)
        .child(localized_element("Explanation", item(fields, 6)?)?)
        .child(picture(item(fields, 5)?, names)?)
        .child(content);

    // Nested subsystems by their short names: the last part of the full name
    // the index gives them (`Subsystem.A.Subsystem.B` -> `B`).
    if number(item(root, 2)?)? != 1 {
        bail!("expected one child collection");
    }
    let nested = list(item(root, 3)?)?;
    if atom(item(nested, 0)?)? != SUBSYSTEM_CHILDREN {
        bail!("expected nested subsystems: {}", short(item(root, 3)?));
    }
    let count = number(item(nested, 1)?)? as usize;
    let mut children = el("ChildObjects");
    for stored in nested
        .get(2..2 + count)
        .ok_or_else(|| anyhow!("short nested subsystems"))?
    {
        let uuid = atom(stored)?;
        let full = names
            .name(uuid)
            .ok_or_else(|| anyhow!("no name for subsystem {uuid}"))?;
        let (owner, name) = full
            .rsplit_once(".Subsystem.")
            .ok_or_else(|| anyhow!("{full} is not a nested subsystem"))?;
        if let Some(own) = names.name(&head.uuid)
            && own != owner
        {
            bail!("subsystem {full} is not owned by {own}");
        }
        children.children.push(leaf("Subsystem", name));
    }
    Ok(el("Subsystem")
        .attr("uuid", head.uuid)
        .child(properties)
        .child(children))
}

/// The nine-member `{4,present,<ref>,"",x,y,load transparent,0,""}` picture
/// reference -> `<Picture>`: the inverse of `common::Picture::to_brace`.
fn picture(node: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = list(node)?;
    if atom(item(fields, 0)?)? != "4" {
        bail!("not a picture: {}", short(node));
    }
    if atom(item(fields, 1)?)? == "0" {
        return Ok(el("Picture"));
    }
    let reference = match list(item(fields, 2)?)? {
        [code] => {
            let code = number(code)?;
            let name = STD_PICTURE_CODES
                .iter()
                .find_map(|(name, candidate)| (*candidate == code).then_some(*name))
                .ok_or_else(|| anyhow!("unknown picture code {code}"))?;
            format!("StdPicture.{name}")
        }
        [zero, uuid] if atom(zero)? == "0" => {
            let uuid = atom(uuid)?;
            match ibcmd_1c_vocab::standard_picture_name(uuid) {
                Some(name) => name.to_string(),
                None => match names.name(uuid) {
                    Some(name) => name.to_string(),
                    // A common picture that no longer exists.
                    None => format!("0:{uuid}"),
                },
            }
        }
        _ => bail!("unsupported picture {}", short(node)),
    };
    let mut element = el("Picture")
        .child(leaf("xr:Ref", reference))
        .child(leaf("xr:LoadTransparent", bool_text(item(fields, 6)?)?));
    let (x, y) = (atom(item(fields, 4)?)?, atom(item(fields, 5)?)?);
    if x != "-1" || y != "-1" {
        element
            .children
            .push(el("xr:TransparentPixel").attr("x", x).attr("y", y));
    }
    Ok(element)
}

// ---------------------------------------------------------------------------
// Forms

/// The record every form descriptor holds,
/// `{13,<header>,help,type,{N,{"#",<class>,purpose}...}}` or on 8.5
/// `{14,...,compatibility mode}` -> the header and `<Properties>` up to
/// `UseInInterfaceCompatibilityMode`.
///
/// A 2.21 file always writes `UseInInterfaceCompatibilityMode`; a row of a
/// configuration in an 8.3 compatibility mode keeps record 13, which holds
/// none: `Any`, the only value.
fn form_record(node: &Brace, context: &ExportContext) -> Result<(Header, Element)> {
    let fields = list(node)?;
    let version = atom(item(fields, 0)?)?;
    if version != "13" && version != "14" {
        bail!("not a form record: {}", short(node));
    }
    let head = header(item(fields, 1)?)?;
    let stored = list(item(fields, 4)?)?;
    let count = number(item(stored, 0)?)? as usize;
    let mut purposes = el("UsePurposes");
    for purpose in stored
        .get(1..1 + count)
        .ok_or_else(|| anyhow!("short use purposes"))?
    {
        let parts = list(purpose)?;
        if string(item(parts, 0)?)? != "#" || atom(item(parts, 1)?)? != USE_PURPOSE_CLASS {
            bail!("not a use purpose: {}", short(purpose));
        }
        purposes.children.push(
            leaf("v8:Value", code_text(item(parts, 2)?, USE_PURPOSES)?)
                .attr("type", "app:ApplicationUsePurpose"),
        );
    }
    let mut properties = header_properties(&head)?
        .child(coded("FormType", item(fields, 3)?, FORM_TYPES)?)
        .child(flag("IncludeHelpInContents", item(fields, 2)?)?)
        .child(purposes);
    if context.is_xml_2_21() {
        let mode = match (version, fields.get(5)) {
            ("14", Some(mode)) => code_text(mode, INTERFACE_COMPATIBILITY_MODES)?,
            _ => "Any",
        };
        properties
            .children
            .push(leaf("UseInInterfaceCompatibilityMode", mode));
    }
    Ok((head, properties))
}

/// An object's form. Its owner's kind decides the wrapper, which the row
/// shows: `{13|14,...}` bare (business processes, tasks, charts of accounts
/// and characteristic types, accounting registers), `{0,<record>}` (catalogs,
/// documents, registers, ...), `{1,{0,<record>,<extended presentation>}}`
/// (data processors and reports, the only ones with `ExtendedPresentation`).
fn form(payload: &Brace, context: &ExportContext) -> Result<Element> {
    let fields = list(payload)?;
    let (stored_record, presentation) = match atom(item(fields, 0)?)? {
        "13" | "14" => (payload, None),
        "0" => (item(fields, 1)?, None),
        "1" => {
            let inner = list(item(fields, 1)?)?;
            if atom(item(inner, 0)?)? != "0" {
                bail!("bad form wrapper {}", short(payload));
            }
            (item(inner, 1)?, Some(item(inner, 2)?))
        }
        _ => bail!("bad form wrapper {}", short(payload)),
    };
    let (head, mut properties) = form_record(stored_record, context)?;
    if let Some(presentation) = presentation {
        properties
            .children
            .push(localized_element("ExtendedPresentation", presentation)?);
    }
    Ok(el("Form").attr("uuid", head.uuid).child(properties))
}

/// `{4,<record>,<extended presentation>,<explanation>,standard commands}`
fn common_form(payload: &Brace, context: &ExportContext) -> Result<Element> {
    let fields = record(payload, "4", "CommonForm")?;
    let (head, properties) = form_record(item(fields, 1)?, context)?;
    let properties = properties
        .child(flag("UseStandardCommands", item(fields, 4)?)?)
        .child(localized_element("ExtendedPresentation", item(fields, 2)?)?)
        .child(localized_element("Explanation", item(fields, 3)?)?);
    Ok(el("CommonForm").attr("uuid", head.uuid).child(properties))
}

/// An object's template: `{2,type,<header>}`.
fn template(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "2", "Template")?;
    let head = header(item(fields, 2)?)?;
    let properties =
        header_properties(&head)?.child(coded("TemplateType", item(fields, 1)?, TEMPLATE_TYPES)?);
    Ok(el("Template").attr("uuid", head.uuid).child(properties))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::metadata_model::ObjectXml;
    use crate::metadata_model::export::write_document;
    use crate::metadata_model::index::{ConfigIndex, ObjectEntry};
    use crate::metadata_model::objects::parts::Compat;
    use crate::metadata_model::types::tests::{context, element};

    const SUBSYSTEM: &str = "5f0bd9b1-0000-4000-8000-000000000001";
    const NESTED: &str = "5f0bd9b1-0000-4000-8000-000000000002";
    const CATALOG: &str = "5f0bd9b1-0000-4000-8000-000000000003";
    const GONE: &str = "5f0bd9b1-0000-4000-8000-0000000000ff";

    fn add_object(index: &mut ConfigIndex, full_name: &str, uuid: &str) {
        let (kind, name) = full_name.rsplit_once('.').unwrap();
        let kind = kind.rsplit('.').next().unwrap();
        index.objects.insert(
            full_name.to_string(),
            ObjectEntry {
                kind: kind.to_string(),
                name: name.to_string(),
                uuid: uuid.to_string(),
                full_name: full_name.to_string(),
                path: PathBuf::new(),
            },
        );
        index
            .objects_by_uuid
            .insert(uuid.to_string(), full_name.to_string());
    }

    /// XML -> row (the load direction) -> model -> XML: the same text.
    fn round_trip(kind: &str, path: &str, version: &str, xml: &str) -> Brace {
        let mut context = context();
        context.version = version.to_string();
        add_object(&mut context.index, "Subsystem.Продажи", SUBSYSTEM);
        add_object(
            &mut context.index,
            "Subsystem.Продажи.Subsystem.Опт",
            NESTED,
        );
        add_object(&mut context.index, "Catalog.Валюты", CATALOG);
        let original = element(xml);
        let object = ObjectXml {
            element: &original,
            kind,
            uuid: Element::attr(&original, "uuid")
                .unwrap_or_default()
                .to_string(),
            name: original
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default(),
            path: Path::new(path),
        };
        let row = super::super::compile(&object, &context).unwrap();
        let export = ExportContext {
            names: NameIndex::from_config_index(&context.index),
            version: version.to_string(),
            compat: Compat(8, 3, 27),
        };
        let decoded = decode(kind, &row, &export).unwrap();
        assert_eq!(
            write_document(&decoded, version),
            write_document(&original, version)
        );
        row
    }

    #[test]
    fn a_subsystem_round_trips_with_content_picture_and_nested_subsystems() {
        let xml = format!(
            r##"<Subsystem uuid="{SUBSYSTEM}"><Properties><Name>Продажи</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Продажи</v8:content></v8:item></Synonym><Comment/><IncludeHelpInContents>true</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Explanation><v8:item><v8:lang>ru</v8:lang><v8:content>Две
строки</v8:content></v8:item></Explanation><Picture><xr:Ref>StdPicture.Print</xr:Ref><xr:LoadTransparent>true</xr:LoadTransparent></Picture><Content><xr:Item xsi:type="xr:MDObjectRef">Catalog.Валюты</xr:Item><xr:Item xsi:type="xr:MDObjectRef">{GONE}</xr:Item></Content></Properties><ChildObjects><Subsystem>Опт</Subsystem></ChildObjects></Subsystem>"##
        );
        let row = round_trip("Subsystem", "Subsystems/Продажи.xml", "2.20", &xml);
        let found = names("Subsystem", &row).unwrap();
        assert_eq!(found.full_name, "Subsystem.Продажи");
        assert_eq!(found.uuid, SUBSYSTEM);
        let nested = format!(
            r##"<Subsystem uuid="{NESTED}"><Properties><Name>Опт</Name><Synonym/><Comment/><IncludeHelpInContents>false</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Explanation/><Picture/><Content/></Properties><ChildObjects/></Subsystem>"##
        );
        round_trip(
            "Subsystem",
            "Subsystems/Продажи/Subsystems/Опт.xml",
            "2.20",
            &nested,
        );
    }

    #[test]
    fn forms_round_trip_in_every_wrapper_and_both_dialects() {
        for (folder, extended) in [
            ("BusinessProcesses", ""),
            ("Catalogs", ""),
            (
                "DataProcessors",
                "<ExtendedPresentation><v8:item><v8:lang>ru</v8:lang><v8:content>Форма</v8:content></v8:item></ExtendedPresentation>",
            ),
        ] {
            for version in ["2.20", "2.21"] {
                let mode = if version == "2.20" {
                    ""
                } else {
                    "<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>"
                };
                let xml = format!(
                    r##"<Form uuid="5f0bd9b1-0000-4000-8000-000000000010"><Properties><Name>ФормаЭлемента</Name><Synonym/><Comment>Комментарий</Comment><FormType>Managed</FormType><IncludeHelpInContents>false</IncludeHelpInContents><UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">PlatformApplication</v8:Value><v8:Value xsi:type="app:ApplicationUsePurpose">MobilePlatformApplication</v8:Value></UsePurposes>{mode}{extended}</Properties></Form>"##
                );
                let row = round_trip(
                    "Form",
                    &format!("{folder}/X/Forms/ФормаЭлемента.xml"),
                    version,
                    &xml,
                );
                assert!(names("Form", &row).is_err());
            }
        }
        // An ordinary form without use purposes.
        round_trip(
            "Form",
            "Documents/X/Forms/Старая.xml",
            "2.20",
            r##"<Form uuid="5f0bd9b1-0000-4000-8000-000000000011"><Properties><Name>Старая</Name><Synonym/><Comment/><FormType>Ordinary</FormType><IncludeHelpInContents>true</IncludeHelpInContents><UsePurposes/></Properties></Form>"##,
        );
    }

    #[test]
    fn common_forms_round_trip_in_both_dialects() {
        for (version, mode) in [
            ("2.20", ""),
            (
                "2.21",
                "<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>",
            ),
        ] {
            let xml = format!(
                r##"<CommonForm uuid="5f0bd9b1-0000-4000-8000-000000000020"><Properties><Name>Настройки</Name><Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Мои настройки</v8:content></v8:item></Synonym><Comment/><FormType>Managed</FormType><IncludeHelpInContents>false</IncludeHelpInContents><UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">PlatformApplication</v8:Value></UsePurposes>{mode}<UseStandardCommands>true</UseStandardCommands><ExtendedPresentation/><Explanation><v8:item><v8:lang>ru</v8:lang><v8:content>Пояснение</v8:content></v8:item></Explanation></Properties></CommonForm>"##
            );
            let row = round_trip("CommonForm", "CommonForms/Настройки.xml", version, &xml);
            assert_eq!(
                names("CommonForm", &row).unwrap().full_name,
                "CommonForm.Настройки"
            );
        }
    }

    #[test]
    fn a_template_round_trips() {
        let row = round_trip(
            "Template",
            "Catalogs/X/Templates/Макет.xml",
            "2.20",
            r##"<Template uuid="5f0bd9b1-0000-4000-8000-000000000030"><Properties><Name>Макет</Name><Synonym/><Comment/><TemplateType>DataCompositionSchema</TemplateType></Properties></Template>"##,
        );
        assert!(names("Template", &row).is_err());
    }

    #[test]
    fn an_8_3_form_record_exports_to_2_21_with_the_default_mode() {
        // ERP УХ 8.5 (compatibility 8.3.27) keeps record 13 under a 2.21 file.
        let row = crate::metadata_model::brace::parse_row(
            "{1,{0,{13,{3,{1,0,5f0bd9b1-0000-4000-8000-000000000040},\"Ф\",{0},\"\",0,0,00000000-0000-0000-0000-000000000000,0},0,1,{0}}},0}"
                .as_bytes(),
        )
        .unwrap();
        let export = ExportContext {
            names: NameIndex::default(),
            version: "2.21".to_string(),
            compat: Compat(8, 3, 27),
        };
        let decoded = decode("Form", &row, &export).unwrap();
        let properties = Element::child(&decoded, "Properties").unwrap();
        assert_eq!(
            properties.child_text("UseInInterfaceCompatibilityMode"),
            Some("Any")
        );
        assert!(properties.child("ExtendedPresentation").is_none());
    }
}
