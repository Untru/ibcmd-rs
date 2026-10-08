//! The export direction of the descriptor model: a stored Config row ->
//! the object's model -> the exact XML file the native exporter writes.
//!
//! The model is the object's XML DOM (`xml::Element`): the same tree the
//! load direction compiles from. A kind's `decode` walks its row layout
//! backwards into that tree, `write_document` prints it with the platform's
//! layout (BOM, CRLF, tabs, self-closed empty elements), and compiling the
//! decoded tree again must give the stored row back -- the lossless check.
//!
//! References are resolved while decoding, through a [`NameIndex`] (uuid ->
//! full name, type id -> generated type name, predefined item -> name). For
//! now it is the reverse of the load direction's `ConfigIndex`, built from an
//! XML tree; each kind's [`object_names`] says what the object contributes to
//! such an index from its row alone, so the index can later be built from
//! the rows.

#[path = "export_audit.rs"]
pub mod audit;
#[path = "export_names.rs"]
pub mod names;
#[path = "export_values.rs"]
pub mod values;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow, bail};

use super::brace::{Brace, NIL_UUID, parse_row};
use super::index::ConfigIndex;
use super::objects::parts::Compat;
use super::xml::{Element, parse_element_tree};

/// Names by uuid: what a decoder needs to spell references.
#[derive(Debug, Default, Clone)]
pub struct NameIndex {
    /// Objects and child objects: uuid -> full name (`Catalog.X`,
    /// `Catalog.X.Form.F`, `Catalog.X.TabularSection.T.Attribute.A`).
    names: HashMap<String, String>,
    /// Generated types: type id -> name (`CatalogRef.X`, `DefinedType.Y`).
    types: HashMap<String, String>,
    /// Predefined items of catalogs and charts: item uuid -> (owner full
    /// name, item name). A copied object keeps its items' ids, so one id can
    /// name items of several owners.
    predefined: HashMap<String, Vec<(String, String)>>,
}

impl NameIndex {
    /// The reverse of the load direction's index, plus the predefined items
    /// of every `Ext/Predefined.xml` of the tree.
    pub fn from_config_index(index: &ConfigIndex) -> Self {
        let mut names = HashMap::with_capacity(index.objects.len() + index.children.len());
        for (full_name, entry) in &index.objects {
            names.insert(entry.uuid.clone(), full_name.clone());
        }
        for (full_name, uuid) in &index.children {
            names
                .entry(uuid.clone())
                .or_insert_with(|| full_name.clone());
        }
        let mut types = HashMap::with_capacity(index.generated_types.len());
        for generated in index.generated_types.values() {
            types.insert(generated.type_id.clone(), generated.name.clone());
        }
        let mut predefined = HashMap::<String, Vec<(String, String)>>::new();
        for entry in index.objects.values() {
            if !matches!(
                entry.kind.as_str(),
                "Catalog"
                    | "ChartOfCharacteristicTypes"
                    | "ChartOfAccounts"
                    | "ChartOfCalculationTypes"
            ) {
                continue;
            }
            let path = entry
                .path
                .with_extension("")
                .join("Ext")
                .join("Predefined.xml");
            if let Ok(bytes) = fs::read(&path)
                && let Ok(root) = parse_element_tree(&bytes)
            {
                let mut items = Vec::new();
                collect_predefined(&root, &mut items);
                for (uuid, name) in items {
                    predefined
                        .entry(uuid)
                        .or_default()
                        .push((entry.full_name.clone(), name));
                }
            }
        }
        Self {
            names,
            types,
            predefined,
        }
    }

    /// Full name of an object or child object.
    pub fn name(&self, uuid: &str) -> Option<&str> {
        self.names.get(uuid).map(String::as_str)
    }

    /// Name of a generated type (`CatalogRef.X`) by its type id.
    pub fn type_name(&self, type_id: &str) -> Option<&str> {
        self.types.get(type_id).map(String::as_str)
    }

    /// Name of a predefined item by its uuid; `None` when the uuid names
    /// items of several owners under different names (use
    /// [`Self::predefined_in`]).
    pub fn predefined(&self, uuid: &str) -> Option<&str> {
        let items = self.predefined.get(uuid)?;
        let (_, first) = items.first()?;
        items
            .iter()
            .all(|(_, name)| name == first)
            .then_some(first.as_str())
    }

    /// Name of a predefined item of `owner` (`Catalog.X`) by its uuid.
    pub fn predefined_in(&self, owner: &str, uuid: &str) -> Option<&str> {
        self.predefined
            .get(uuid)?
            .iter()
            .find(|(item_owner, _)| item_owner == owner)
            .map(|(_, name)| name.as_str())
    }

    /// Adds what one object contributes (for an index built from rows).
    pub fn add(&mut self, names: &ObjectNames) {
        self.names
            .insert(names.uuid.clone(), names.full_name.clone());
        for (full_name, uuid) in &names.children {
            self.names
                .entry(uuid.clone())
                .or_insert_with(|| full_name.clone());
        }
        for generated in &names.types {
            self.types
                .insert(generated.type_id.clone(), generated.name.clone());
        }
    }
}

fn collect_predefined(element: &Element, out: &mut Vec<(String, String)>) {
    for item in &element.children {
        if item.name == "Item"
            && let (Some(id), Some(name)) = (item.attr("id"), item.child_text("Name"))
        {
            out.push((id.to_ascii_lowercase(), name.to_string()));
        }
        collect_predefined(item, out);
    }
}

/// What a decoder may read besides the row.
pub struct ExportContext {
    pub names: NameIndex,
    /// XML dialect to write: `2.20` (8.3.27) or `2.21` (8.5).
    pub version: String,
    /// The configuration's compatibility mode: it decides the record
    /// versions of the rows.
    pub(crate) compat: Compat,
}

impl ExportContext {
    /// The XML is written in dialect `2.21` rather than `2.20`.
    pub fn is_xml_2_21(&self) -> bool {
        self.version != "2.20"
    }

    /// The platform the XML is written for, as the platform registry maps
    /// its dialect: `2.20` -> 8.3.27, `2.21` -> 8.5.1.
    pub fn platform(&self) -> crate::platform::PlatformSpec {
        crate::platform::of_xml_dialect(&self.version)
    }
}

/// A generated type an object declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedTypeName {
    pub name: String,
    pub category: String,
    pub type_id: String,
    pub value_id: String,
}

/// What one object contributes to a name index, read from its row alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObjectNames {
    pub uuid: String,
    pub full_name: String,
    /// Child objects inside the row (attributes, tabular sections and their
    /// attributes, commands, enum values...): (full name, uuid).
    pub children: Vec<(String, String)>,
    pub types: Vec<GeneratedTypeName>,
}

/// Decodes one stored row into the object's model (its XML element).
pub fn decode_object(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    match kind {
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
        | "Enum" => super::objects::export::decode(kind, row, context),
        "InformationRegister"
        | "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister"
        | "Recalculation"
        | "DocumentJournal"
        | "Sequence"
        | "DocumentNumerator" => super::registers::export::decode(kind, row, context),
        "Configuration" => super::root::export::decode(row, context),
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
        | "Language" => super::simple::export::decode(kind, row, context),
        "CommonModule" | "CommonPicture" | "CommonTemplate" | "CommonCommand" | "CommandGroup"
        | "Role" | "XDTOPackage" | "StyleItem" | "Style" | "PaletteColor" | "WebService"
        | "HTTPService" | "WSReference" | "WebSocketClient" | "IntegrationService" | "Bot" => {
            super::common::export::decode(kind, row, context)
        }
        "Subsystem" | "Form" | "Template" | "CommonForm" => {
            super::common::forms_export::decode(kind, row, context)
        }
        other => bail!("not yet: no row decoder for {other}"),
    }
}

/// What one stored row contributes to a name index.
pub fn object_names(kind: &str, row: &Brace) -> Result<ObjectNames> {
    match kind {
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
        | "Enum" => super::objects::export::names(kind, row),
        "InformationRegister"
        | "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister"
        | "Recalculation"
        | "DocumentJournal"
        | "Sequence"
        | "DocumentNumerator" => super::registers::export::names(kind, row),
        "Configuration" => super::root::export::names(row),
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
        | "Language" => super::simple::export::names(kind, row),
        "CommonModule" | "CommonPicture" | "CommonTemplate" | "CommonCommand" | "CommandGroup"
        | "Role" | "XDTOPackage" | "StyleItem" | "Style" | "PaletteColor" | "WebService"
        | "HTTPService" | "WSReference" | "WebSocketClient" | "IntegrationService" | "Bot" => {
            super::common::export::names(kind, row)
        }
        "Subsystem" | "Form" | "Template" | "CommonForm" => {
            super::common::forms_export::names(kind, row)
        }
        other => bail!("not yet: no row decoder for {other}"),
    }
}

/// (kind, uuid) of every object the configuration lists, read from its
/// `Configuration` row (the row `root` names): the rows a name index built
/// from rows reads next.
pub fn configuration_objects(row: &Brace) -> Result<Vec<(String, String)>> {
    super::root::export::top_level_objects(row)
}

/// What the row of an owned object (a recalculation, a nested subsystem)
/// contributes to a name index. Its names start with its owner's full name
/// (`owner`, `CalculationRegister.X`), which its own row does not hold: the
/// owner's row lists it by uuid, and the index names the owners first.
/// Owned forms and templates need no arm: their full name is the owner's
/// and their own name, and they carry no child objects or generated types.
pub fn owned_object_names(kind: &str, row: &Brace, owner: &str) -> Result<ObjectNames> {
    match kind {
        "Recalculation" => super::registers::export::owned_names(kind, row, owner),
        // A nested subsystem names itself like a top-level one; the owner
        // goes in front.
        "Subsystem" => {
            let mut names = super::common::forms_export::names(kind, row)?;
            let short = names
                .full_name
                .strip_prefix("Subsystem.")
                .ok_or_else(|| anyhow!("{} is not a subsystem", names.full_name))?
                .to_string();
            names.full_name = format!("{owner}.Subsystem.{short}");
            Ok(names)
        }
        // An owned form or template: its own header, under its owner.
        "Form" | "Template" => {
            let head =
                names::own_header(row).ok_or_else(|| anyhow!("no header in the {kind} row"))?;
            Ok(ObjectNames {
                full_name: format!("{owner}.{kind}.{}", head.1),
                uuid: head.0,
                children: Vec::new(),
                types: Vec::new(),
            })
        }
        other => bail!("not yet: no owned-row names for {other}"),
    }
}

/// A stored row (inflated, BOM optional) -> the XML file text.
pub fn export_descriptor(kind: &str, row: &[u8], context: &ExportContext) -> Result<String> {
    let tree = parse_row(row)?;
    let object = decode_object(kind, &tree, context)?;
    Ok(write_document(&object, &context.version))
}

// ---------------------------------------------------------------------------
// The DOM builder the decoders use.

/// An empty element by qualified name (`Properties`, `xr:Field`).
pub(crate) fn el(qname: &str) -> Element {
    let (prefix, name) = match qname.split_once(':') {
        Some((prefix, name)) => (prefix, name),
        None => ("", qname),
    };
    Element {
        name: name.to_string(),
        prefix: prefix.to_string(),
        ..Element::default()
    }
}

/// An element holding text.
pub(crate) fn leaf(qname: &str, text: impl Into<String>) -> Element {
    let mut element = el(qname);
    element.text = text.into();
    element
}

/// Builder methods on the DOM element.
pub(crate) trait Build: Sized {
    fn attr(self, key: &str, value: impl Into<String>) -> Self;
    fn child(self, child: Element) -> Self;
    fn children(self, children: impl IntoIterator<Item = Element>) -> Self;
}

impl Build for Element {
    fn attr(mut self, key: &str, value: impl Into<String>) -> Self {
        self.attrs.push((key.to_string(), value.into()));
        self
    }
    fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }
    fn children(mut self, children: impl IntoIterator<Item = Element>) -> Self {
        self.children.extend(children);
        self
    }
}

// ---------------------------------------------------------------------------
// Row access with errors instead of panics.

pub(crate) fn list(node: &Brace) -> Result<&[Brace]> {
    node.as_list()
        .ok_or_else(|| anyhow!("expected a list, got {}", short(node)))
}

pub(crate) fn atom(node: &Brace) -> Result<&str> {
    node.as_atom()
        .ok_or_else(|| anyhow!("expected a value, got {}", short(node)))
}

pub(crate) fn string(node: &Brace) -> Result<&str> {
    node.as_str()
        .ok_or_else(|| anyhow!("expected a string, got {}", short(node)))
}

pub(crate) fn number(node: &Brace) -> Result<i64> {
    let text = atom(node)?;
    text.parse()
        .map_err(|_| anyhow!("expected a number, got {text}"))
}

pub(crate) fn item(items: &[Brace], index: usize) -> Result<&Brace> {
    items
        .get(index)
        .ok_or_else(|| anyhow!("list too short: no member {index} of {}", items.len()))
}

pub(crate) fn is_nil(uuid: &str) -> bool {
    uuid == NIL_UUID
}

/// A short printable excerpt of a node, for errors.
pub(crate) fn short(node: &Brace) -> String {
    let text = super::brace::serialize(node).replace("\r\n", "");
    if text.chars().count() > 80 {
        format!("{}...", text.chars().take(80).collect::<String>())
    } else {
        text
    }
}

/// Stored text -> XML text: the rows keep CRLF, the XML writes LF.
pub(crate) fn xml_text(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n")
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// The writer.

/// Namespaces the root element declares, in the order the platform writes
/// them; 2.21 adds the palette.
fn root_namespaces(version: &str) -> &'static str {
    if version == "2.20" {
        r#"xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" xmlns:cmi="http://v8.1c.ru/8.2/managed-application/cmi" xmlns:ent="http://v8.1c.ru/8.1/data/enterprise" xmlns:lf="http://v8.1c.ru/8.2/managed-application/logform" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:sys="http://v8.1c.ru/8.1/data/ui/fonts/system" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" xmlns:win="http://v8.1c.ru/8.1/data/ui/colors/windows" xmlns:xen="http://v8.1c.ru/8.3/xcf/enums" xmlns:xpr="http://v8.1c.ru/8.3/xcf/predef" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    } else {
        r#"xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:app="http://v8.1c.ru/8.2/managed-application/core" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" xmlns:cmi="http://v8.1c.ru/8.2/managed-application/cmi" xmlns:ent="http://v8.1c.ru/8.1/data/enterprise" xmlns:lf="http://v8.1c.ru/8.2/managed-application/logform" xmlns:pal="http://v8.1c.ru/8.1/data/ui/colors/palette" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:sys="http://v8.1c.ru/8.1/data/ui/fonts/system" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" xmlns:win="http://v8.1c.ru/8.1/data/ui/colors/windows" xmlns:xen="http://v8.1c.ru/8.3/xcf/enums" xmlns:xpr="http://v8.1c.ru/8.3/xcf/predef" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    }
}

const ROOT_PREFIXES: &[&str] = &[
    "app", "cfg", "cmi", "ent", "lf", "pal", "style", "sys", "v8", "v8ui", "web", "win", "xen",
    "xpr", "xr", "xs", "xsi",
];

/// The namespace a type value's prefix stands for when the root does not
/// declare it: the platform declares it on the element itself.
fn value_namespace(prefix: &str, local: &str) -> Option<&'static str> {
    Some(match prefix {
        "mxl" => "http://v8.1c.ru/8.2/data/spreadsheet",
        "dcsset" => "http://v8.1c.ru/8.1/data-composition-system/settings",
        "dcscor" => "http://v8.1c.ru/8.1/data-composition-system/core",
        "dcssch" => "http://v8.1c.ru/8.1/data-composition-system/schema",
        "fd" => "http://v8.1c.ru/8.2/data/formatted-document",
        "pdfdoc" => "http://v8.1c.ru/8.3/data/pdf",
        "pl" => "http://v8.1c.ru/8.3/data/planner",
        // Generated prefixes (`d<depth>p1`) name the namespace of the type.
        _ if prefix.starts_with('d') && prefix.ends_with("p1") => match local {
            "Chart" | "GanttChart" => "http://v8.1c.ru/8.2/data/chart",
            "TextDocument" => "http://v8.1c.ru/8.1/data/txtedt",
            "GeographicalSchema" => "http://v8.1c.ru/8.2/data/geo",
            "FlowchartContextType" => "http://v8.1c.ru/8.2/data/graphscheme",
            "Filter" => "http://v8.1c.ru/8.2/misc",
            "DataAnalysisTimeIntervalUnitType" => "http://v8.1c.ru/8.2/data/data-analysis",
            "ConditionalAppearance" => "http://v8.1c.ru/8.3/data/entext",
            _ => "http://v8.1c.ru/8.1/data/enterprise/current-config",
        },
        _ => return None,
    })
}

/// The whole file: BOM, declaration, root element, the object, no newline
/// after the root's end tag.
pub fn write_document(object: &Element, version: &str) -> String {
    let mut out = String::with_capacity(16 * 1024);
    out.push('\u{feff}');
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject ");
    out.push_str(root_namespaces(version));
    out.push_str(" version=\"");
    out.push_str(version);
    out.push_str("\">\r\n");
    write_element(&mut out, object, 1);
    out.push_str("</MetaDataObject>");
    out
}

fn push_qname(out: &mut String, element: &Element) {
    if !element.prefix.is_empty() {
        out.push_str(&element.prefix);
        out.push(':');
    }
    out.push_str(&element.name);
}

fn write_element(out: &mut String, element: &Element, depth: usize) {
    for _ in 0..depth {
        out.push('\t');
    }
    out.push('<');
    push_qname(out, element);
    for (key, value) in &element.attrs {
        out.push(' ');
        match key.as_str() {
            "type" | "nil" => out.push_str("xsi:"),
            _ => {}
        }
        out.push_str(key);
        out.push_str("=\"");
        escape(out, value, true);
        out.push('"');
    }
    for (prefix, uri) in &element.namespaces {
        out.push_str(" xmlns");
        if !prefix.is_empty() {
            out.push(':');
            out.push_str(prefix);
        }
        out.push_str("=\"");
        escape(out, uri, true);
        out.push('"');
    }
    let leaf = element.children.is_empty();
    // A decoded type value whose prefix the root does not declare
    // (`mxl:...`, `d0p1:Chart`) gets its own declaration; a generated prefix
    // is the element's depth.
    let mut text: &str = &element.text;
    let rewritten;
    if leaf
        && element.namespaces.is_empty()
        && element.prefix == "v8"
        && matches!(element.name.as_str(), "Type" | "TypeSet")
        && let Some((prefix, local)) = text.split_once(':')
        && !ROOT_PREFIXES.contains(&prefix)
        && let Some(uri) = value_namespace(prefix, local)
    {
        let prefix = if prefix.starts_with('d') && prefix.ends_with("p1") {
            format!("d{}p1", depth + 1)
        } else {
            prefix.to_string()
        };
        out.push_str(" xmlns:");
        out.push_str(&prefix);
        out.push_str("=\"");
        out.push_str(uri);
        out.push('"');
        rewritten = format!("{prefix}:{local}");
        text = &rewritten;
    }
    if leaf {
        if text.is_empty() {
            out.push_str("/>\r\n");
        } else {
            out.push('>');
            escape(out, text, false);
            out.push_str("</");
            push_qname(out, element);
            out.push_str(">\r\n");
        }
        return;
    }
    out.push_str(">\r\n");
    for child in &element.children {
        write_element(out, child, depth + 1);
    }
    for _ in 0..depth {
        out.push('\t');
    }
    out.push_str("</");
    push_qname(out, element);
    out.push_str(">\r\n");
}

/// `&`, `<`, `>` everywhere; `"` in attribute values.
fn escape(out: &mut String, text: &str, attribute: bool) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            '\r' => out.push_str("&#xD;"),
            _ => out.push(ch),
        }
    }
}

/// A parsed file's object element written back: the writer's own check.
pub fn rewrite_file(bytes: &[u8]) -> Result<String> {
    let doc = super::xml::MetadataXml::parse(bytes)?;
    let version = doc.version().unwrap_or("2.20").to_string();
    Ok(write_document(doc.object()?, &version))
}

/// The version a tree's files are written in, from its `Configuration.xml`.
pub fn tree_version(root: &Path) -> Option<String> {
    let bytes = fs::read(root.join("Configuration.xml")).ok()?;
    let doc = super::xml::MetadataXml::parse(&bytes).ok()?;
    doc.version().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::super::brace::{Brace, parse_row};
    use super::super::objects::parts::compatibility;
    use super::super::{DescriptorContext, compile_descriptor};
    use super::values::{Owner, data_path_text};
    use super::*;
    use crate::brace_list;

    /// Lines as the platform writes them: tabs, CRLF, no final newline.
    fn native(version: &str, lines: &[&str]) -> String {
        let mut text = format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject {} version=\"{version}\">\r\n",
            root_namespaces(version)
        );
        for line in lines {
            text.push_str(line);
            text.push_str("\r\n");
        }
        text.push_str("</MetaDataObject>");
        text
    }

    #[test]
    fn the_writer_self_closes_escapes_and_declares_type_namespaces() {
        let object = el("Enum")
            .attr("uuid", "u")
            .child(
                el("Properties")
                    .child(leaf("Comment", "a < b & \"c\""))
                    .child(el("Explanation")),
            )
            .child(
                el("Type")
                    .child(leaf("v8:Type", "mxl:SpreadsheetDocument"))
                    .child(leaf("v8:Type", "d0p1:Chart")),
            )
            .child(el("MinValue").attr("nil", "true"));
        let written = write_document(&object, "2.20");
        let expected = native(
            "2.20",
            &[
                "\t<Enum uuid=\"u\">",
                "\t\t<Properties>",
                "\t\t\t<Comment>a &lt; b &amp; \"c\"</Comment>",
                "\t\t\t<Explanation/>",
                "\t\t</Properties>",
                "\t\t<Type>",
                "\t\t\t<v8:Type xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\">mxl:SpreadsheetDocument</v8:Type>",
                "\t\t\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.2/data/chart\">d4p1:Chart</v8:Type>",
                "\t\t</Type>",
                "\t\t<MinValue xsi:nil=\"true\"/>",
                "\t</Enum>",
            ],
        );
        assert_eq!(written, expected);
        // A parsed file writes back unchanged, 2.21 root included.
        let file = native(
            "2.21",
            &["\t<Enum uuid=\"u\">", "\t\t<Properties/>", "\t</Enum>"],
        );
        assert_eq!(rewrite_file(file.as_bytes()).unwrap(), file);
    }

    #[test]
    fn a_data_path_outside_its_owner_is_written_raw() {
        let mut names = NameIndex::default();
        names
            .names
            .insert("a".repeat(36), "Catalog.X.Attribute.A".to_string());
        names
            .names
            .insert("b".repeat(36), "Document.Y.Attribute.B".to_string());
        let owner = Owner {
            kind: "Catalog",
            full_name: "Catalog.X",
        };
        let segment = |uuid: String| brace_list![Brace::num(0), Brace::atom(uuid)];
        let inside = data_path_text(&[segment("a".repeat(36))], owner, &names).unwrap();
        assert_eq!(inside, "Catalog.X.Attribute.A");
        let outside = data_path_text(&[segment("b".repeat(36))], owner, &names).unwrap();
        assert_eq!(outside, format!("0:{}", "b".repeat(36)));
        let standard = data_path_text(&[brace_list![Brace::num(-5)]], owner, &names).unwrap();
        assert_eq!(standard, "Catalog.X.StandardAttribute.Owner");
        let foreign = data_path_text(&[brace_list![Brace::num(-99)]], owner, &names).unwrap();
        assert_eq!(foreign, "-99");
    }

    /// An enum goes XML -> row -> XML and comes back byte for byte.
    #[test]
    fn an_enum_round_trips_through_its_row() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-export-enum-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(root.join("Enums")).unwrap();
        fs::write(
            root.join("Configuration.xml"),
            native(
                "2.20",
                &[
                    "\t<Configuration uuid=\"11111111-1111-1111-1111-111111111111\">",
                    "\t\t<Properties>",
                    "\t\t\t<Name>C</Name>",
                    "\t\t\t<CompatibilityMode>Version8_3_27</CompatibilityMode>",
                    "\t\t</Properties>",
                    "\t</Configuration>",
                ],
            ),
        )
        .unwrap();
        let xml = native(
            "2.20",
            &[
                "\t<Enum uuid=\"0d2189f1-306c-4236-836a-1a4bfc0cff13\">",
                "\t\t<InternalInfo>",
                "\t\t\t<xr:GeneratedType name=\"EnumRef.E\" category=\"Ref\">",
                "\t\t\t\t<xr:TypeId>e4253022-3bfb-430b-bd60-e507622d28c9</xr:TypeId>",
                "\t\t\t\t<xr:ValueId>2b90a755-c5f0-4075-b543-5eaf9c0f9b54</xr:ValueId>",
                "\t\t\t</xr:GeneratedType>",
                "\t\t\t<xr:GeneratedType name=\"EnumManager.E\" category=\"Manager\">",
                "\t\t\t\t<xr:TypeId>6a120ac5-26bf-4c49-b7e5-37917a0c787e</xr:TypeId>",
                "\t\t\t\t<xr:ValueId>e8eb2b7a-fbde-45c9-9e8a-1a2a0a0832bf</xr:ValueId>",
                "\t\t\t</xr:GeneratedType>",
                "\t\t\t<xr:GeneratedType name=\"EnumList.E\" category=\"List\">",
                "\t\t\t\t<xr:TypeId>3a7ddce0-9165-4ff2-ab59-f91506e04201</xr:TypeId>",
                "\t\t\t\t<xr:ValueId>6d76ebe3-a08e-4c5e-ad5d-c9f739008a98</xr:ValueId>",
                "\t\t\t</xr:GeneratedType>",
                "\t\t</InternalInfo>",
                "\t\t<Properties>",
                "\t\t\t<Name>E</Name>",
                "\t\t\t<Synonym>",
                "\t\t\t\t<v8:item>",
                "\t\t\t\t\t<v8:lang>ru</v8:lang>",
                "\t\t\t\t\t<v8:content>Две\nстроки &amp; знак</v8:content>",
                "\t\t\t\t</v8:item>",
                "\t\t\t</Synonym>",
                "\t\t\t<Comment/>",
                "\t\t\t<UseStandardCommands>false</UseStandardCommands>",
                "\t\t\t<Characteristics/>",
                "\t\t\t<QuickChoice>true</QuickChoice>",
                "\t\t\t<ChoiceMode>BothWays</ChoiceMode>",
                "\t\t\t<DefaultListForm/>",
                "\t\t\t<DefaultChoiceForm/>",
                "\t\t\t<AuxiliaryListForm/>",
                "\t\t\t<AuxiliaryChoiceForm/>",
                "\t\t\t<ListPresentation/>",
                "\t\t\t<ExtendedListPresentation/>",
                "\t\t\t<Explanation/>",
                "\t\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>",
                "\t\t</Properties>",
                "\t\t<ChildObjects>",
                "\t\t\t<EnumValue uuid=\"a03d3535-090a-46f5-a411-3567c8c12bc1\">",
                "\t\t\t\t<Properties>",
                "\t\t\t\t\t<Name>V</Name>",
                "\t\t\t\t\t<Synonym/>",
                "\t\t\t\t\t<Comment>Одна</Comment>",
                "\t\t\t\t</Properties>",
                "\t\t\t</EnumValue>",
                "\t\t</ChildObjects>",
                "\t</Enum>",
            ],
        );
        let path = root.join("Enums").join("E.xml");
        fs::write(&path, &xml).unwrap();
        let descriptor_context = DescriptorContext::new(&root, "2.20").unwrap();
        let row = compile_descriptor("Enum", &path, xml.as_bytes(), &descriptor_context).unwrap();
        let context = ExportContext {
            names: NameIndex::from_config_index(&descriptor_context.index),
            version: "2.20".to_string(),
            compat: compatibility(&descriptor_context),
        };
        assert_eq!(export_descriptor("Enum", &row, &context).unwrap(), xml);
        // The names a row gives match the tree's.
        let names = object_names("Enum", &parse_row(&row).unwrap()).unwrap();
        assert_eq!(names.full_name, "Enum.E");
        assert_eq!(
            names.children,
            vec![(
                "Enum.E.EnumValue.V".to_string(),
                "a03d3535-090a-46f5-a411-3567c8c12bc1".to_string()
            )]
        );
        assert_eq!(names.types.len(), 3);
        assert_eq!(names.types[2].name, "EnumList.E");
        fs::remove_dir_all(&root).ok();
    }
}
