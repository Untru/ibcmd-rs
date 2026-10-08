//! Base-free metadata descriptor compiler: a metadata object's XML becomes the
//! complete native text of its Config row without reading the target
//! database, so a configuration can be loaded into an empty infobase.
//!
//! Layout: `brace` (the row as a plain tree, one layout rule for every
//! descriptor), `xml` (a small DOM), `index` (names -> uuids for the whole
//! tree), shared encoders here, and one module per family of kinds.
//! `compile_descriptor` dispatches by the XML's kind; `audit` measures every
//! kind against the rows a platform stored.

pub mod audit;
pub mod brace;
pub mod export;
pub mod index;
pub mod xml;

// Shared encoders owned by the simple-objects track: type descriptions,
// typed values and the attribute body every "attribute-like" object shares.
pub mod attribute;
pub mod types;

// Kind families, one track each.
pub mod common;
pub mod websocket_client;
// Base-free body rows of track D: predefined data, flowcharts, aggregates.
pub mod bodies_aggregates;
pub mod bodies_flowchart;
pub mod bodies_predefined;
pub mod bodies_rows;
pub mod bodies_value_table;
pub(crate) mod external_data_source;
pub mod objects;
pub mod registers;
pub mod root;
pub mod simple;
pub mod standard_pictures;

#[cfg(test)]
mod collection_tests;
#[cfg(test)]
mod slot_evidence_tests;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use self::brace::{Brace, serialize_row};
use self::index::ConfigIndex;
use self::xml::{Element, MetadataXml};
use crate::brace_list;
use crate::module_blob::MetadataSourceContext;

/// What a kind compiler may read besides its own XML.
pub struct DescriptorContext {
    pub root: PathBuf,
    /// Names -> uuids for the whole tree.
    pub index: ConfigIndex,
    /// The older, lazily filled source context the load writers use.
    pub source: MetadataSourceContext,
    /// XML dialect of the tree: `2.20` (8.3.27) or `2.21` (8.5).
    pub version: String,
}

impl DescriptorContext {
    pub fn new(root: &Path, version: &str) -> Result<Self> {
        Ok(Self {
            root: root.to_path_buf(),
            index: ConfigIndex::build(root).context("failed to index the source tree")?,
            source: MetadataSourceContext::new(root.to_path_buf()),
            version: version.to_string(),
        })
    }

    /// The same context from descriptor XMLs the caller already read (`(path,
    /// bytes)` for what `audit::descriptor_xmls(root)` lists): the index is
    /// built from them, and the source context's resolvers read them from
    /// memory instead of from disk.
    pub fn with_files(
        root: &Path,
        version: &str,
        files: &[(PathBuf, std::sync::Arc<Vec<u8>>)],
    ) -> Result<Self> {
        let preloaded = std::sync::Arc::new(
            files
                .iter()
                .cloned()
                .collect::<std::collections::HashMap<_, _>>(),
        );
        Ok(Self {
            root: root.to_path_buf(),
            index: ConfigIndex::build_from_files(root, files)
                .context("failed to index the source tree")?,
            source: MetadataSourceContext::with_preloaded(root.to_path_buf(), preloaded),
            version: version.to_string(),
        })
    }

    /// The tree is written in dialect `2.21` rather than `2.20`.
    pub fn is_xml_2_21(&self) -> bool {
        self.version != "2.20"
    }

    /// The platform the tree is written for, as the platform registry maps
    /// its dialect: `2.20` -> 8.3.27, `2.21` -> 8.5.1.
    pub fn platform(&self) -> crate::platform::PlatformSpec {
        crate::platform::of_xml_dialect(&self.version)
    }
}

/// One metadata object's XML, parsed.
pub struct ObjectXml<'a> {
    /// The object element (`<Catalog uuid=...>`).
    pub element: &'a Element,
    /// `Catalog`, `Form`, ...
    pub kind: &'a str,
    pub uuid: String,
    pub name: String,
    /// The XML file, for bodies next to it.
    pub path: &'a Path,
}

impl<'a> ObjectXml<'a> {
    pub fn properties(&self) -> Result<&'a Element> {
        self.element
            .child("Properties")
            .ok_or_else(|| anyhow!("{} {} has no <Properties>", self.kind, self.name))
    }
    /// A property element, erroring when absent.
    pub fn prop(&self, name: &str) -> Result<&'a Element> {
        self.properties()?
            .child(name)
            .ok_or_else(|| anyhow!("{} {} has no <{name}>", self.kind, self.name))
    }
    pub fn prop_text(&self, name: &str) -> Result<&'a str> {
        Ok(self.prop(name)?.text.as_str())
    }
    pub fn prop_bool(&self, name: &str) -> Result<bool> {
        parse_bool(self.prop_text(name)?)
    }
    pub fn child_objects(&self) -> Option<&'a Element> {
        self.element.child("ChildObjects")
    }
}

pub fn parse_bool(text: &str) -> Result<bool> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("expected true/false, got {other:?}"),
    }
}

/// `{N,"lang","text",...}`, `{0}` when empty: synonyms, tooltips, formats,
/// presentations.
pub fn localized(element: Option<&Element>) -> Brace {
    let pairs = element.map(Element::localized).unwrap_or_default();
    let mut items = vec![Brace::num(pairs.len() as i64)];
    for (lang, content) in pairs {
        items.push(Brace::str(lang));
        items.push(Brace::str(native_text(&content)));
    }
    Brace::List(items)
}

/// XML text -> the stored string: the XML carries a line break as a bare LF,
/// the row as CRLF (every multi-line string of the 4 932 BSP descriptor rows
/// is CRLF-only). Idempotent: some paths apply it twice. A CR LF the string
/// itself holds arrives already spelled `\r\r\n` (`MetadataXml::parse`).
pub fn native_text(text: &str) -> String {
    if !text.contains('\n') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    let mut previous = ' ';
    for ch in text.chars() {
        if ch == '\n' && previous != '\r' {
            out.push('\r');
        }
        out.push(ch);
        previous = ch;
    }
    out
}

/// The block every metadata object and child object starts with:
/// `{3,{1,0,<uuid>},"Name",<synonym>,"Comment",0,0,<nil uuid>,0}`.
pub fn md_base(uuid: &str, properties: &Element) -> Brace {
    brace_list![
        Brace::num(3),
        brace_list![Brace::num(1), Brace::num(0), Brace::uuid(uuid)],
        Brace::str(properties.child_text("Name").unwrap_or_default()),
        localized(properties.child("Synonym")),
        Brace::str(native_text(
            properties.child_text("Comment").unwrap_or_default()
        )),
        Brace::num(0),
        Brace::num(0),
        Brace::nil_uuid(),
        Brace::num(0),
    ]
}

/// Compiles one metadata XML into the inflated text of its Config row
/// (BOM included).
pub fn compile_descriptor(
    kind: &str,
    xml_path: &Path,
    xml: &[u8],
    context: &DescriptorContext,
) -> Result<Vec<u8>> {
    let configuration_document = if kind == "Configuration" {
        let document = ibcmd_xml::XmlReader::from_slice(xml)?;
        ibcmd_xml::metadata::parse_configuration_mobile_functionalities(&document)?;
        Some(document)
    } else {
        None
    };
    if kind == ibcmd_schema::websocket_client::WebSocketClientLayout::KIND {
        let document = ibcmd_xml::XmlReader::from_slice(xml)?;
        ibcmd_xml::metadata::validate_websocket_client_headers_namespaces(&document)?;
    }
    let external_data_source = if kind == "ExternalDataSource" {
        Some(external_data_source::validate_source(
            xml,
            &context.version,
        )?)
    } else {
        None
    };
    let doc = MetadataXml::parse(xml)?;
    let element = doc.object()?;
    if kind == "Configuration" {
        // The declaration is a separate coordinate from the storage context.
        if let Some(properties) = element.child("Properties") {
            root::validate_interface_edition(
                properties,
                doc.root.attr("version").unwrap_or_default(),
            )?;
        }
    }
    let object = ObjectXml {
        element,
        kind,
        uuid: element
            .attr("uuid")
            .unwrap_or_default()
            .to_ascii_lowercase(),
        name: element
            .path(&["Properties", "Name"])
            .map(|name| name.text.clone())
            .unwrap_or_default(),
        path: xml_path,
    };
    if let Some(document) = configuration_document.as_ref()
        && root::configuration_shape(&object, context)? != root::ConfigurationShape::V76
    {
        ibcmd_xml::metadata::validate_older_configuration_v85_defaults(document, &context.version)?;
    }
    let tree = if let Some(canonical) = external_data_source.as_ref() {
        external_data_source::compile_admitted(&object, canonical)?
    } else {
        compile_object(&object, context)?
    };
    Ok(serialize_row(&tree))
}

/// Dispatches one object to its family.
pub fn compile_object(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    match object.kind {
        "Constant"
        | "DefinedType"
        | "SessionParameter"
        | "CommonAttribute"
        | "FunctionalOption"
        | "FunctionalOptionsParameter"
        | "EventSubscription"
        | "ScheduledJob"
        | "SettingsStorage"
        | "FilterCriterion"
        | "Language" => simple::compile(object, context),
        "Catalog"
        | "Document"
        | "ExchangePlan"
        | "ChartOfCharacteristicTypes"
        | "ChartOfAccounts"
        | "ChartOfCalculationTypes"
        | "BusinessProcess"
        | "Task"
        | "Report"
        | "DataProcessor"
        | "Enum" => objects::compile(object, context),
        "InformationRegister"
        | "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister"
        | "Recalculation"
        | "DocumentJournal"
        | "Sequence"
        | "DocumentNumerator" => registers::compile(object, context),
        "CommonModule" | "CommonPicture" | "CommonTemplate" | "CommonCommand" | "CommandGroup"
        | "Role" | "XDTOPackage" | "StyleItem" | "Style" | "WebService" | "HTTPService"
        | "WSReference" | "WebSocketClient" | "IntegrationService" | "Bot"
        | "ExternalDataSource" | "Subsystem" | "Form" | "Template" | "CommonForm" | "Interface"
        | "PaletteColor" => common::compile(object, context),
        "Configuration" => root::compile(object, context),
        other => bail!("unknown metadata kind {other}"),
    }
}

/// The error a family returns for a kind it does not compile yet.
pub fn not_yet(object: &ObjectXml<'_>) -> anyhow::Error {
    anyhow!("no base-free compiler for {} yet", object.kind)
}
