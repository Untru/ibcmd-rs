//! The body rows track D writes for a base-free stage: predefined data
//! (Catalog `.1c`, ChartOfCharacteristicTypes `.7`, ChartOfAccounts `.9`,
//! ChartOfCalculationTypes `.2`), business process flowcharts (`.7`),
//! accumulation register aggregates (`.3`) and GraphicalSchema templates
//! (`.0`, with their inline pictures). Each is compiled from the object's
//! XML and its `Ext` files alone; an object without them has no row.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use super::DescriptorContext;
use super::bodies_aggregates::{aggregates_path, aggregates_row, emptied_aggregates_row};
use super::bodies_flowchart::{
    flowchart_path, flowchart_row, graphical_schema_row, is_graphical_schema_template,
    template_body_path,
};
use super::bodies_predefined::{
    emptied_predefined_row, predefined_path, predefined_row, predefined_suffix,
};
use super::brace::serialize_row;
use super::common::crlf_strings;
use super::xml::MetadataXml;

/// One body row: its Config file name, the file it was compiled from and
/// the stored text (BOM included).
#[derive(Clone, Debug)]
pub struct BodyRow {
    pub file_name: String,
    pub source: PathBuf,
    pub text: Vec<u8>,
}

/// Whether these writers replace the loader's own kind-body writer for an
/// object (whose writer only patches a base row): predefined data,
/// flowcharts and graphical-schema templates.
pub fn owns_kind_body(kind: &str, xml: &[u8]) -> bool {
    match kind {
        "Catalog" | "ChartOfCharacteristicTypes" | "BusinessProcess" => true,
        "Template" | "CommonTemplate" => MetadataXml::parse(xml)
            .ok()
            .as_ref()
            .and_then(|doc| doc.object().ok())
            .is_some_and(is_graphical_schema_template),
        _ => false,
    }
}

/// The suffix of the row these writers give a kind, if any.
pub fn body_row_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "BusinessProcess" => Some("7"),
        "AccumulationRegister" => Some("3"),
        "Template" | "CommonTemplate" => Some("0"),
        other => predefined_suffix(other),
    }
}

/// The rows of one object (none, or one).
pub fn compile_body_rows(
    kind: &str,
    xml_path: &Path,
    xml: &[u8],
    context: &DescriptorContext,
) -> Result<Vec<BodyRow>> {
    let Some(suffix) = body_row_suffix(kind) else {
        return Ok(Vec::new());
    };
    let doc = MetadataXml::parse(xml)?;
    let object = doc.object()?;
    let uuid = object
        .attr("uuid")
        .ok_or_else(|| anyhow!("{kind} has no uuid"))?
        .to_ascii_lowercase();
    let (tree, source) = match kind {
        "BusinessProcess" => (flowchart_row(xml_path, context)?, flowchart_path(xml_path)),
        "Template" | "CommonTemplate" => {
            if !is_graphical_schema_template(object) {
                return Ok(Vec::new());
            }
            (
                graphical_schema_row(xml_path, context)?,
                template_body_path(xml_path),
            )
        }
        "AccumulationRegister" => (
            aggregates_row(object, xml_path, context)?,
            aggregates_path(xml_path),
        ),
        _ => (
            predefined_row(kind, object, xml_path, context)?,
            predefined_path(xml_path),
        ),
    };
    let Some(mut tree) = tree else {
        return Ok(Vec::new());
    };
    crlf_strings(&mut tree);
    Ok(vec![BodyRow {
        file_name: format!("{uuid}.{suffix}"),
        source,
        text: serialize_row(&tree),
    }])
}

/// A part a stored configuration keeps as a row after its content was
/// deleted in the Designer: the export writes no file for it, and only the
/// tree's `ConfigDumpInfo.xml` still lists it (ERP УХ keeps 145: 132 helps,
/// 11 predefined data, 1 aggregates, 1 command interface).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StubPart {
    Help,
    CommandInterface,
    Predefined,
    Aggregates,
}

impl StubPart {
    /// From the last segment of a `ConfigDumpInfo.xml` name
    /// (`Catalog.X.Form.Y.Help`, `Subsystem.S.CommandInterface`, ...).
    pub fn of_dump_info_name(name: &str) -> Option<Self> {
        Some(match name.rsplit('.').next()? {
            "Help" => Self::Help,
            "CommandInterface" => Self::CommandInterface,
            "Predefined" => Self::Predefined,
            "Aggregates" => Self::Aggregates,
            _ => return None,
        })
    }
}

/// The stored text (BOM included) of a content-free row of `part`, for an
/// owner of `owner_kind` whose descriptor XML is `owner_xml`; `None` when
/// the row `suffix` is not the one that part takes for that kind.
///
/// - help: `{5,0,0}` (10 bytes, every such row of ERP УХ);
/// - command interface: `{7,0,0,0,0,0,0}` (18 bytes);
/// - predefined data: the tree with its root alone
///   (`bodies_predefined::emptied_predefined_row`);
/// - aggregates: the table's columns and no row
///   (`bodies_aggregates::emptied_aggregates_row`).
pub fn stub_row_text(
    part: StubPart,
    owner_kind: &str,
    suffix: &str,
    owner_xml: &[u8],
    context: &DescriptorContext,
) -> Result<Option<Vec<u8>>> {
    let tree = match part {
        StubPart::Help => return Ok(Some("\u{feff}{5,0,0}".as_bytes().to_vec())),
        StubPart::CommandInterface => {
            return Ok(Some("\u{feff}{7,0,0,0,0,0,0}".as_bytes().to_vec()));
        }
        StubPart::Predefined => {
            if predefined_suffix(owner_kind) != Some(suffix) {
                return Ok(None);
            }
            let doc = MetadataXml::parse(owner_xml)?;
            emptied_predefined_row(owner_kind, doc.object()?, context)?
        }
        StubPart::Aggregates => {
            if owner_kind != "AccumulationRegister" || suffix != "3" {
                return Ok(None);
            }
            let doc = MetadataXml::parse(owner_xml)?;
            Some(emptied_aggregates_row(doc.object()?)?)
        }
    };
    Ok(tree.map(|mut tree| {
        crlf_strings(&mut tree);
        serialize_row(&tree)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, relative: &str, body: &str) -> Vec<u8> {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MetaDataObject \
             xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.20\">{body}</MetaDataObject>"
        );
        fs::write(&path, &xml).unwrap();
        xml.into_bytes()
    }

    #[test]
    fn stub_rows_are_modelled_on_the_stored_ones() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-stub-rows-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        write(
            &root,
            "Configuration.xml",
            "<Configuration uuid=\"cccccccc-0000-4000-8000-000000000001\"><Properties>\
             <Name>C</Name></Properties></Configuration>",
        );
        let catalog = write(
            &root,
            "Catalogs/A.xml",
            "<Catalog uuid=\"aaaaaaaa-0000-4000-8000-000000000001\"><Properties><Name>A</Name>\
             <CodeLength>9</CodeLength><CodeType>String</CodeType>\
             <CodeAllowedLength>Variable</CodeAllowedLength>\
             <DescriptionLength>25</DescriptionLength></Properties></Catalog>",
        );
        let register = write(
            &root,
            "AccumulationRegisters/R.xml",
            "<AccumulationRegister uuid=\"bbbbbbbb-0000-4000-8000-000000000001\"><Properties>\
             <Name>R</Name></Properties><ChildObjects>\
             <Dimension uuid=\"dddddddd-0000-4000-8000-000000000001\"><Properties><Name>D</Name>\
             </Properties></Dimension></ChildObjects></AccumulationRegister>",
        );
        let context = DescriptorContext::new(&root, "2.20").unwrap();
        let text = |part, kind: &str, suffix: &str, xml: &[u8]| {
            stub_row_text(part, kind, suffix, xml, &context)
                .unwrap()
                .map(|text| String::from_utf8(text).unwrap().replace("\r\n", ""))
        };

        // the 10- and 18-byte rows every emptied help and command interface keeps
        assert_eq!(
            text(StubPart::Help, "Catalog", "0", &catalog).as_deref(),
            Some("\u{feff}{5,0,0}")
        );
        assert_eq!(
            text(StubPart::CommandInterface, "Subsystem", "1", &catalog).as_deref(),
            Some("\u{feff}{7,0,0,0,0,0,0}")
        );
        // predefined data: the root alone, with the six values of an edited tree
        let predefined = text(StubPart::Predefined, "Catalog", "1c", &catalog).unwrap();
        assert!(predefined.starts_with("\u{feff}{0,{1,{7,"), "{predefined}");
        assert!(
            predefined.ends_with(r#"{"S","Элементы"},{"S",""},{"S",""},0}},-1,0}}}"#),
            "{predefined}"
        );
        assert!(predefined.contains(r#"{"S",9,1}"#), "{predefined}");
        assert!(predefined.contains(r#"{"S",25,1}"#), "{predefined}");
        // aggregates: the register's dimension columns and no row
        let aggregates =
            text(StubPart::Aggregates, "AccumulationRegister", "3", &register).unwrap();
        assert!(aggregates.starts_with("\u{feff}{0,{9,{4,"), "{aggregates}");
        assert!(aggregates.ends_with("{1,0},3,-1},{0,0}}}"), "{aggregates}");
        // a row the part does not take for that kind is no stub
        assert_eq!(text(StubPart::Predefined, "Catalog", "7", &catalog), None);
        assert_eq!(text(StubPart::Predefined, "Document", "1c", &catalog), None);
        assert_eq!(
            text(StubPart::Aggregates, "InformationRegister", "3", &register),
            None
        );
        let _ = fs::remove_dir_all(&root);

        assert_eq!(
            StubPart::of_dump_info_name("Catalog.A.Form.F.Help"),
            Some(StubPart::Help)
        );
        assert_eq!(
            StubPart::of_dump_info_name("Subsystem.S.Subsystem.T.CommandInterface"),
            Some(StubPart::CommandInterface)
        );
        assert_eq!(StubPart::of_dump_info_name("Catalog.A.Form.F"), None);
    }
}
