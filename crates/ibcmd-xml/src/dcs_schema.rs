//! Evidence-bounded codec for the first typed inner DCS schema cohort.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::dcs_schema::{
    DcsSchema, DcsSchemaBuildError, DcsSchemaCalculatedField, DcsSchemaDataSetField,
    DcsSchemaDataSetObject, DcsSchemaDecimalType, DcsSchemaFieldType, DcsSchemaLocalDataSource,
    DcsSchemaLocalString, DcsSchemaSettingsVariantShell, DcsSchemaStringParameter,
    DcsSchemaStringType, DcsSchemaTotalFunction, DcsSchemaUngroupedTotalField,
};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::value::CanonicalText;
use ibcmd_schema::{DcsInnerSchemaPolicy, bundled_dcs_inner_schema_policy};
use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

const MAX_DEPTH: usize = 64;
const MAX_EVENTS: usize = 32_768;

use crate::analyze_dcs_inline_settings_fragment;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsInnerSchemaError {
    InvalidEvidence(String),
    Malformed(String),
    UnsupportedSource(String),
    Build(DcsSchemaBuildError),
}

impl Display for DcsInnerSchemaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEvidence(reason) => {
                write!(f, "invalid inner DCS schema evidence: {reason}")
            }
            Self::Malformed(reason) => write!(f, "malformed inner DCS schema XML: {reason}"),
            Self::UnsupportedSource(reason) => write!(f, "unsupported inner DCS schema: {reason}"),
            Self::Build(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for DcsInnerSchemaError {}

/// An inline settings block that passed the common DCS Settings analyzer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsInlineSettingsFragment(String);

impl DcsInlineSettingsFragment {
    pub fn parse(xml: String) -> Result<Self, DcsInnerSchemaError> {
        let policy = policy()?;
        let closed = close_inline_settings_namespaces(&xml, &policy)?;
        analyze_dcs_inline_settings_fragment(&closed).map_err(|error| {
            DcsInnerSchemaError::UnsupportedSource(format!(
                "inline Settings fragment is outside the common contract: {error}"
            ))
        })?;
        Ok(Self(xml))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn close_inline_settings_namespaces(
    xml: &str,
    policy: &DcsInnerSchemaPolicy,
) -> Result<String, DcsInnerSchemaError> {
    let opening_end = xml.find('>').ok_or_else(|| {
        DcsInnerSchemaError::Malformed("inline Settings has no opening tag".into())
    })?;
    if !xml[..opening_end]
        .trim_start()
        .starts_with("<dcsset:settings")
    {
        return unsupported("inline Settings root does not use the canonical dcsset spelling");
    }
    let mut closed = String::with_capacity(xml.len() + 384);
    closed.push_str(&xml[..opening_end]);
    for (prefix, namespace) in [
        ("dcsset", policy.settings_namespace_uri()),
        ("dcscor", "http://v8.1c.ru/8.1/data-composition-system/core"),
        ("v8", policy.data_core_namespace_uri()),
        ("v8ui", "http://v8.1c.ru/8.1/data/ui"),
        ("xs", policy.xml_schema_namespace_uri()),
        ("xsi", policy.xsi_namespace_uri()),
    ] {
        if !xml[..opening_end].contains(&format!("xmlns:{prefix}=")) {
            closed.push_str(" xmlns:");
            closed.push_str(prefix);
            closed.push_str("=\"");
            closed.push_str(namespace);
            closed.push('"');
        }
    }
    closed.push_str(&xml[opening_end..]);
    Ok(closed)
}

#[derive(Clone, Debug)]
struct ExpandedAttribute {
    namespace: Option<String>,
    local: String,
    value: String,
}

#[derive(Clone, Debug)]
enum ParsedNode {
    Element(ParsedElement),
    Text(String),
}

#[derive(Clone, Debug)]
struct ParsedElement {
    namespace: Option<String>,
    local: String,
    attributes: Vec<ExpandedAttribute>,
    namespaces: BTreeMap<Option<String>, String>,
    children: Vec<ParsedNode>,
}

/// Parses the platform-authenticated primary `SchemaFile` document into the
/// closed canonical cohort. External Settings documents are deliberately not
/// accepted here; their positional binding remains in `dcs_template`.
pub fn parse_dcs_inner_schema_storage_document(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
) -> Result<DcsSchema, DcsInnerSchemaError> {
    let policy = policy()?;
    let document = parse_document(bytes)?;
    require_name(&document, None, "SchemaFile")?;
    require_no_attributes(&document)?;
    let wrapper_children = elements(&document)?;
    if wrapper_children.len() != 1 {
        return unsupported("SchemaFile must contain exactly one schema root");
    }
    let root = wrapper_children[0];
    require_name(
        root,
        Some(policy.schema_namespace_uri()),
        "dataCompositionSchema",
    )?;
    require_no_attributes(root)?;

    let children = elements(root)?;
    let expected_counts = [1usize, 1, 1, 2, 1];
    let mut cursor = 0usize;
    let data_source = parse_data_source(take(&children, &mut cursor, "dataSource")?, &policy)?;
    let data_set = parse_data_set(take(&children, &mut cursor, "dataSet")?, &policy)?;
    let calculated = parse_calculated(take(&children, &mut cursor, "calculatedField")?, &policy)?;
    let mut totals = Vec::with_capacity(expected_counts[3]);
    for _ in 0..expected_counts[3] {
        totals.push(parse_total(
            take(&children, &mut cursor, "totalField")?,
            &policy,
        )?);
    }
    let parameter = parse_parameter(take(&children, &mut cursor, "parameter")?, &policy)?;
    let mut variants = Vec::new();
    while cursor < children.len() {
        variants.push(parse_variant(
            take(&children, &mut cursor, "settingsVariant")?,
            &policy,
        )?);
    }
    if !policy.supports_settings_variant_count(variants.len()) {
        return unsupported("settingsVariant count is outside the evidenced range");
    }

    let anchor = CanonicalAnchor::new(
        ObjectPath::new(vec![PathSegment::name("dcs_schema").expect("static path")])
            .expect("bounded static path"),
        PropertyPath::root(),
    );
    let provenance = SourceProvenance::with_locator(source_profile, anchor, locator)
        .map_err(|error| DcsInnerSchemaError::Malformed(error.to_string()))?;
    DcsSchema::new(
        data_source,
        data_set,
        calculated,
        totals,
        parameter,
        variants,
        provenance,
    )
    .map_err(DcsInnerSchemaError::Build)
}

/// Emits the exact canonical XML 2.20 source spelling for the bounded cohort.
/// Each supplied block must be an already evidence-gated inline
/// `<dcsset:settings...>` fragment owned by the common Settings codec.
pub fn emit_dcs_inner_schema_source_document(
    schema: &DcsSchema,
    settings_blocks: &[DcsInlineSettingsFragment],
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    let policy = policy()?;
    if schema.settings_variants().len() != settings_blocks.len()
        || !policy.supports_settings_variant_count(settings_blocks.len())
    {
        return unsupported("Settings block count does not match settingsVariant shells");
    }
    let mut out = String::from("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str("<DataCompositionSchema xmlns=\"");
    out.push_str(policy.schema_namespace_uri());
    out.push_str("\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\"");
    out.push_str(" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\"");
    out.push_str(" xmlns:dcsset=\"");
    out.push_str(policy.settings_namespace_uri());
    out.push_str("\" xmlns:v8=\"");
    out.push_str(policy.data_core_namespace_uri());
    out.push_str("\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"");
    out.push_str(policy.xml_schema_namespace_uri());
    out.push_str("\" xmlns:xsi=\"");
    out.push_str(policy.xsi_namespace_uri());
    out.push_str("\">");

    line(&mut out, 1, "<dataSource>");
    scalar(&mut out, 2, "name", schema.data_source().name().as_str());
    scalar(&mut out, 2, "dataSourceType", "Local");
    line(&mut out, 1, "</dataSource>");
    line(&mut out, 1, "<dataSet xsi:type=\"DataSetObject\">");
    scalar(&mut out, 2, "name", schema.data_set().name().as_str());
    for field in schema.data_set().fields() {
        line(&mut out, 2, "<field xsi:type=\"DataSetFieldField\">");
        scalar(&mut out, 3, "dataPath", field.data_path().as_str());
        scalar(&mut out, 3, "field", field.field().as_str());
        emit_value_type(&mut out, 3, field.value_type());
        line(&mut out, 2, "</field>");
    }
    scalar(
        &mut out,
        2,
        "dataSource",
        schema.data_set().data_source().as_str(),
    );
    scalar(
        &mut out,
        2,
        "objectName",
        schema.data_set().object_name().as_str(),
    );
    line(&mut out, 1, "</dataSet>");
    line(&mut out, 1, "<calculatedField>");
    scalar(
        &mut out,
        2,
        "dataPath",
        schema.calculated_field().data_path().as_str(),
    );
    scalar(
        &mut out,
        2,
        "expression",
        schema.calculated_field().expression().as_str(),
    );
    emit_value_type(
        &mut out,
        2,
        DcsSchemaFieldType::Decimal(schema.calculated_field().value_type()),
    );
    line(&mut out, 1, "</calculatedField>");
    for total in schema.total_fields() {
        line(&mut out, 1, "<totalField>");
        scalar(&mut out, 2, "dataPath", total.data_path().as_str());
        let expression = policy
            .sum_total_expression_grammar()
            .replace("{dataPath}", total.data_path().as_str());
        scalar(&mut out, 2, "expression", &expression);
        line(&mut out, 1, "</totalField>");
    }
    line(&mut out, 1, "<parameter>");
    scalar(&mut out, 2, "name", schema.parameter().name().as_str());
    emit_local_string(&mut out, 2, "title", schema.parameter().title(), false);
    emit_value_type(
        &mut out,
        2,
        DcsSchemaFieldType::String(schema.parameter().value_type()),
    );
    let mut value = String::from("<value xsi:type=\"xs:string\">");
    value.push_str(&escape(schema.parameter().value().as_str()));
    value.push_str("</value>");
    line(&mut out, 2, &value);
    scalar(&mut out, 2, "useRestriction", "false");
    line(&mut out, 1, "</parameter>");
    for (variant, settings) in schema.settings_variants().iter().zip(settings_blocks) {
        line(&mut out, 1, "<settingsVariant>");
        scalar(&mut out, 2, "dcsset:name", variant.name().as_str());
        emit_local_string(
            &mut out,
            2,
            "dcsset:presentation",
            variant.presentation(),
            true,
        );
        append_indented_fragment(&mut out, settings.as_str(), 2);
        line(&mut out, 1, "</settingsVariant>");
    }
    out.push_str("\r\n</DataCompositionSchema>");
    Ok(out.into_bytes())
}

fn parse_data_source(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaLocalDataSource, DcsInnerSchemaError> {
    require_schema(e, p, "dataSource")?;
    require_no_attributes(e)?;
    let c = exact_children(e, p.data_source_child_order(), p.schema_namespace_uri())?;
    if text(c[1])? != "Local" {
        return unsupported("dataSourceType is not Local");
    }
    DcsSchemaLocalDataSource::new(canonical(text(c[0])?)?).map_err(DcsInnerSchemaError::Build)
}

fn parse_data_set(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaDataSetObject, DcsInnerSchemaError> {
    require_schema(e, p, "dataSet")?;
    require_type(e, p, p.data_set_object_type_qname())?;
    let c = element_children(e)?;
    if c.len() != 5
        || c[0].local != "name"
        || c[1].local != "field"
        || c[2].local != "field"
        || c[3].local != "dataSource"
        || c[4].local != "objectName"
    {
        return unsupported("DataSetObject child order/cardinality is outside the cohort");
    }
    for child in &c {
        require_namespace(child, p.schema_namespace_uri())?;
    }
    let fields = vec![parse_field(c[1], p)?, parse_field(c[2], p)?];
    DcsSchemaDataSetObject::new(
        canonical(text(c[0])?)?,
        fields,
        canonical(text(c[3])?)?,
        canonical(text(c[4])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)
}

fn parse_field(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaDataSetField, DcsInnerSchemaError> {
    require_type(e, p, p.data_set_field_type_qname())?;
    let c = exact_children(e, p.data_set_field_child_order(), p.schema_namespace_uri())?;
    DcsSchemaDataSetField::new(
        canonical(text(c[0])?)?,
        canonical(text(c[1])?)?,
        parse_value_type(c[2], p)?,
    )
    .map_err(DcsInnerSchemaError::Build)
}

fn parse_calculated(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaCalculatedField, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(
        e,
        p.calculated_field_child_order(),
        p.schema_namespace_uri(),
    )?;
    let DcsSchemaFieldType::Decimal(value_type) = parse_value_type(c[2], p)? else {
        return unsupported("calculatedField value type is not decimal");
    };
    DcsSchemaCalculatedField::new(canonical(text(c[0])?)?, canonical(text(c[1])?)?, value_type)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_total(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaUngroupedTotalField, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(e, p.total_field_child_order(), p.schema_namespace_uri())?;
    let path = text(c[0])?;
    let expected = p
        .sum_total_expression_grammar()
        .replace("{dataPath}", &path);
    if text(c[1])? != expected {
        return unsupported("totalField expression is outside the Sum(dataPath) grammar");
    }
    DcsSchemaUngroupedTotalField::new(canonical(path)?, DcsSchemaTotalFunction::Sum)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_parameter(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaStringParameter, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(e, p.parameter_child_order(), p.schema_namespace_uri())?;
    let title = parse_local_string(c[1], p)?;
    let DcsSchemaFieldType::String(value_type) = parse_value_type(c[2], p)? else {
        return unsupported("parameter valueType is not string");
    };
    require_type(c[3], p, p.string_value_type_qname())?;
    if text(c[4])? != "false" {
        return unsupported("parameter useRestriction is not false");
    }
    DcsSchemaStringParameter::new(
        canonical(text(c[0])?)?,
        title,
        value_type,
        canonical(text_allowing_attributes(c[3])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)
}

fn parse_variant(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaSettingsVariantShell, DcsInnerSchemaError> {
    require_schema(e, p, "settingsVariant")?;
    require_no_attributes(e)?;
    let order = p.settings_variant_child_order();
    let c = exact_children(e, &order[..2], p.settings_namespace_uri())?;
    DcsSchemaSettingsVariantShell::new(canonical(text(c[0])?)?, parse_local_string(c[1], p)?)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_local_string(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaLocalString, DcsInnerSchemaError> {
    require_type(e, p, p.local_string_type_qname())?;
    let c = element_children(e)?;
    if c.len() != 1 {
        return unsupported("LocalStringType must contain one item");
    }
    require_name(c[0], Some(p.data_core_namespace_uri()), "item")?;
    require_no_attributes(c[0])?;
    let item = exact_children(
        c[0],
        p.localized_item_child_order(),
        p.data_core_namespace_uri(),
    )?;
    DcsSchemaLocalString::new(canonical(text(item[0])?)?, canonical(text(item[1])?)?)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_value_type(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaFieldType, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = element_children(e)?;
    if c.len() != 2 {
        return unsupported("valueType must contain Type and one qualifier block");
    }
    require_name(c[0], Some(p.data_core_namespace_uri()), "Type")?;
    require_no_attributes(c[0])?;
    let qname = resolve_qname_text(c[0])?;
    if qname == p.string_value_type_qname() {
        require_name(c[1], Some(p.data_core_namespace_uri()), "StringQualifiers")?;
        require_no_attributes(c[1])?;
        let q = exact_children(
            c[1],
            p.string_qualifiers_child_order(),
            p.data_core_namespace_uri(),
        )?;
        if text(q[1])? != "Variable" {
            return unsupported("AllowedLength is outside the cohort");
        }
        let length = text(q[0])?
            .parse()
            .map_err(|_| DcsInnerSchemaError::Malformed("string Length is not u32".into()))?;
        Ok(DcsSchemaFieldType::String(
            DcsSchemaStringType::new(length).map_err(DcsInnerSchemaError::Build)?,
        ))
    } else if qname == p.decimal_value_type_qname() {
        require_name(c[1], Some(p.data_core_namespace_uri()), "NumberQualifiers")?;
        require_no_attributes(c[1])?;
        let q = exact_children(
            c[1],
            p.number_qualifiers_child_order(),
            p.data_core_namespace_uri(),
        )?;
        if text(q[2])? != "Any" {
            return unsupported("AllowedSign is outside the cohort");
        }
        let digits = text(q[0])?
            .parse()
            .map_err(|_| DcsInnerSchemaError::Malformed("Digits is not u32".into()))?;
        let fraction = text(q[1])?
            .parse()
            .map_err(|_| DcsInnerSchemaError::Malformed("FractionDigits is not u32".into()))?;
        Ok(DcsSchemaFieldType::Decimal(
            DcsSchemaDecimalType::new(digits, fraction).map_err(DcsInnerSchemaError::Build)?,
        ))
    } else {
        unsupported("value Type QName is outside the cohort")
    }
}

fn parse_document(bytes: &[u8]) -> Result<ParsedElement, DcsInnerSchemaError> {
    let mut reader = NsReader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<ParsedElement> = Vec::new();
    let mut root = None;
    let mut events = 0usize;
    loop {
        events += 1;
        if events > MAX_EVENTS {
            return Err(DcsInnerSchemaError::Malformed(
                "XML event limit exceeded".into(),
            ));
        }
        match reader
            .read_event()
            .map_err(|e| DcsInnerSchemaError::Malformed(e.to_string()))?
        {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(DcsInnerSchemaError::Malformed(
                        "XML depth limit exceeded".into(),
                    ));
                }
                stack.push(parsed_start(
                    &reader,
                    &e,
                    stack.last().map(|v| &v.namespaces),
                )?);
            }
            Event::Empty(e) => {
                let element = parsed_start(&reader, &e, stack.last().map(|v| &v.namespaces))?;
                push_parsed(element, &mut stack, &mut root)?;
            }
            Event::End(_) => {
                let element = stack.pop().ok_or_else(|| {
                    DcsInnerSchemaError::Malformed("unexpected closing element".into())
                })?;
                push_parsed(element, &mut stack, &mut root)?;
            }
            Event::Text(e) => {
                let value = e
                    .xml_content()
                    .map_err(|x| DcsInnerSchemaError::Malformed(x.to_string()))?
                    .into_owned();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(ParsedNode::Text(value));
                } else if !value.trim().is_empty() {
                    return Err(DcsInnerSchemaError::Malformed("text outside root".into()));
                }
            }
            Event::CData(e) => {
                let value = String::from_utf8(e.into_inner().into_owned())
                    .map_err(|_| DcsInnerSchemaError::Malformed("CDATA is not UTF-8".into()))?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(ParsedNode::Text(value));
                } else {
                    return Err(DcsInnerSchemaError::Malformed("CDATA outside root".into()));
                }
            }
            Event::Decl(_) => {}
            Event::Eof => break,
            Event::Comment(_) | Event::PI(_) | Event::DocType(_) | Event::GeneralRef(_) => {
                return unsupported(
                    "comments, PI, doctype and general references are outside the cohort",
                );
            }
        }
    }
    if !stack.is_empty() {
        return Err(DcsInnerSchemaError::Malformed("unclosed element".into()));
    }
    root.ok_or_else(|| DcsInnerSchemaError::Malformed("missing XML root".into()))
}

fn parsed_start(
    reader: &NsReader<&[u8]>,
    e: &BytesStart<'_>,
    inherited: Option<&BTreeMap<Option<String>, String>>,
) -> Result<ParsedElement, DcsInnerSchemaError> {
    let (resolved, local) = reader.resolve_element(e.name());
    let namespace = match resolved {
        ResolveResult::Bound(value) => Some(String::from_utf8_lossy(value.as_ref()).into_owned()),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(prefix) => {
            return Err(DcsInnerSchemaError::Malformed(format!(
                "unbound namespace prefix {}",
                String::from_utf8_lossy(prefix.as_ref())
            )));
        }
    };
    let mut namespaces = inherited.cloned().unwrap_or_default();
    let mut attributes = Vec::new();
    for attr in e.attributes().with_checks(true) {
        let attr = attr.map_err(|x| DcsInnerSchemaError::Malformed(x.to_string()))?;
        let raw = attr.key.as_ref();
        let value = attr
            .decode_and_unescape_value(reader.decoder())
            .map_err(|x| DcsInnerSchemaError::Malformed(x.to_string()))?
            .into_owned();
        if raw == b"xmlns" {
            namespaces.insert(None, value);
            continue;
        }
        if let Some(prefix) = raw.strip_prefix(b"xmlns:") {
            namespaces.insert(Some(String::from_utf8_lossy(prefix).into_owned()), value);
            continue;
        }
        let (resolved, local) = reader.resolve_attribute(attr.key);
        let namespace = match resolved {
            ResolveResult::Bound(value) => {
                Some(String::from_utf8_lossy(value.as_ref()).into_owned())
            }
            ResolveResult::Unbound => None,
            ResolveResult::Unknown(prefix) => {
                return Err(DcsInnerSchemaError::Malformed(format!(
                    "unbound attribute prefix {}",
                    String::from_utf8_lossy(prefix.as_ref())
                )));
            }
        };
        attributes.push(ExpandedAttribute {
            namespace,
            local: String::from_utf8_lossy(local.as_ref()).into_owned(),
            value,
        });
    }
    Ok(ParsedElement {
        namespace,
        local: String::from_utf8_lossy(local.as_ref()).into_owned(),
        attributes,
        namespaces,
        children: Vec::new(),
    })
}

fn push_parsed(
    element: ParsedElement,
    stack: &mut [ParsedElement],
    root: &mut Option<ParsedElement>,
) -> Result<(), DcsInnerSchemaError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(ParsedNode::Element(element));
        Ok(())
    } else if root.replace(element).is_none() {
        Ok(())
    } else {
        Err(DcsInnerSchemaError::Malformed("multiple XML roots".into()))
    }
}

fn policy() -> Result<DcsInnerSchemaPolicy, DcsInnerSchemaError> {
    bundled_dcs_inner_schema_policy()
        .map_err(|e| DcsInnerSchemaError::InvalidEvidence(e.to_string()))
}
fn unsupported<T>(reason: impl Into<String>) -> Result<T, DcsInnerSchemaError> {
    Err(DcsInnerSchemaError::UnsupportedSource(reason.into()))
}
fn require_namespace(e: &ParsedElement, ns: &str) -> Result<(), DcsInnerSchemaError> {
    if e.namespace.as_deref() == Some(ns) {
        Ok(())
    } else {
        unsupported(format!("{} has the wrong namespace", e.local))
    }
}
fn require_name(
    e: &ParsedElement,
    ns: Option<&str>,
    local: &str,
) -> Result<(), DcsInnerSchemaError> {
    if e.namespace.as_deref() == ns && e.local == local {
        Ok(())
    } else {
        unsupported(format!(
            "expected {{{}}}{local}, found {{{}}}{}",
            ns.unwrap_or(""),
            e.namespace.as_deref().unwrap_or(""),
            e.local
        ))
    }
}
fn require_schema(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    local: &str,
) -> Result<(), DcsInnerSchemaError> {
    require_name(e, Some(p.schema_namespace_uri()), local)
}
fn require_no_attributes(e: &ParsedElement) -> Result<(), DcsInnerSchemaError> {
    if e.attributes.is_empty() {
        Ok(())
    } else {
        unsupported(format!("{} has unsupported attributes", e.local))
    }
}
fn elements(e: &ParsedElement) -> Result<Vec<&ParsedElement>, DcsInnerSchemaError> {
    element_children(e)
}
fn element_children(e: &ParsedElement) -> Result<Vec<&ParsedElement>, DcsInnerSchemaError> {
    let mut out = Vec::new();
    for n in &e.children {
        match n {
            ParsedNode::Element(v) => out.push(v),
            ParsedNode::Text(v) if v.trim().is_empty() => {}
            _ => return unsupported(format!("{} has mixed content", e.local)),
        }
    }
    Ok(out)
}
fn exact_children<'a>(
    e: &'a ParsedElement,
    names: &[String],
    ns: &str,
) -> Result<Vec<&'a ParsedElement>, DcsInnerSchemaError> {
    let c = element_children(e)?;
    if c.len() != names.len() {
        return unsupported(format!(
            "{} child cardinality is outside the cohort",
            e.local
        ));
    }
    for (child, expected) in c.iter().zip(names) {
        let local = expected.rsplit('}').next().unwrap_or(expected);
        require_name(child, Some(ns), local)?;
    }
    Ok(c)
}
fn take<'a>(
    children: &[&'a ParsedElement],
    cursor: &mut usize,
    local: &str,
) -> Result<&'a ParsedElement, DcsInnerSchemaError> {
    let e = *children
        .get(*cursor)
        .ok_or_else(|| DcsInnerSchemaError::UnsupportedSource(format!("missing {local}")))?;
    if e.local != local {
        return unsupported(format!("expected {local} at root ordinal {}", *cursor));
    }
    *cursor += 1;
    Ok(e)
}
fn text(e: &ParsedElement) -> Result<String, DcsInnerSchemaError> {
    if !e.attributes.is_empty() {
        return unsupported(format!("{} scalar has attributes", e.local));
    }
    let mut out = String::new();
    for n in &e.children {
        match n {
            ParsedNode::Text(v) => out.push_str(v),
            ParsedNode::Element(_) => {
                return unsupported(format!("{} scalar has element content", e.local));
            }
        }
    }
    if out.is_empty() {
        unsupported(format!("{} scalar is empty", e.local))
    } else {
        Ok(out)
    }
}
fn text_allowing_attributes(e: &ParsedElement) -> Result<String, DcsInnerSchemaError> {
    let mut out = String::new();
    for node in &e.children {
        match node {
            ParsedNode::Text(value) => out.push_str(value),
            ParsedNode::Element(_) => {
                return unsupported(format!("{} scalar has element content", e.local));
            }
        }
    }
    if out.is_empty() {
        unsupported(format!("{} scalar is empty", e.local))
    } else {
        Ok(out)
    }
}
fn canonical(value: String) -> Result<CanonicalText, DcsInnerSchemaError> {
    CanonicalText::new(&value).map_err(|e| DcsInnerSchemaError::Malformed(e.to_string()))
}
fn require_type(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    expected: &str,
) -> Result<(), DcsInnerSchemaError> {
    if e.attributes.len() != 1 {
        return unsupported(format!("{} must have exactly xsi:type", e.local));
    }
    let a = &e.attributes[0];
    if a.namespace.as_deref() != Some(p.xsi_namespace_uri()) || a.local != "type" {
        return unsupported(format!("{} attribute is not xsi:type", e.local));
    }
    if resolve_qname(e, &a.value)?.as_str() == expected {
        Ok(())
    } else {
        unsupported(format!("{} xsi:type is outside the cohort", e.local))
    }
}
fn resolve_qname_text(e: &ParsedElement) -> Result<String, DcsInnerSchemaError> {
    resolve_qname(e, &text(e)?)
}
fn resolve_qname(e: &ParsedElement, value: &str) -> Result<String, DcsInnerSchemaError> {
    let (prefix, local) = value
        .split_once(':')
        .map_or((None, value), |(p, l)| (Some(p), l));
    if local.is_empty() {
        return Err(DcsInnerSchemaError::Malformed("empty QName local".into()));
    }
    let ns = e
        .namespaces
        .get(&prefix.map(str::to_owned))
        .ok_or_else(|| DcsInnerSchemaError::Malformed(format!("unbound QName `{value}`")))?;
    Ok(format!("{{{ns}}}{local}"))
}

fn line(out: &mut String, depth: usize, value: &str) {
    out.push_str("\r\n");
    for _ in 0..depth {
        out.push('\t');
    }
    out.push_str(value)
}
fn scalar(out: &mut String, depth: usize, name: &str, value: &str) {
    line(out, depth, &format!("<{name}>{}</{name}>", escape(value)))
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn emit_value_type(out: &mut String, depth: usize, value: DcsSchemaFieldType) {
    line(out, depth, "<valueType>");
    match value {
        DcsSchemaFieldType::String(v) => {
            scalar(out, depth + 1, "v8:Type", "xs:string");
            line(out, depth + 1, "<v8:StringQualifiers>");
            scalar(out, depth + 2, "v8:Length", &v.length().to_string());
            scalar(out, depth + 2, "v8:AllowedLength", "Variable");
            line(out, depth + 1, "</v8:StringQualifiers>");
        }
        DcsSchemaFieldType::Decimal(v) => {
            scalar(out, depth + 1, "v8:Type", "xs:decimal");
            line(out, depth + 1, "<v8:NumberQualifiers>");
            scalar(out, depth + 2, "v8:Digits", &v.digits().to_string());
            scalar(
                out,
                depth + 2,
                "v8:FractionDigits",
                &v.fraction_digits().to_string(),
            );
            scalar(out, depth + 2, "v8:AllowedSign", "Any");
            line(out, depth + 1, "</v8:NumberQualifiers>");
        }
    }
    line(out, depth, "</valueType>")
}
fn emit_local_string(
    out: &mut String,
    depth: usize,
    name: &str,
    value: &DcsSchemaLocalString,
    _settings: bool,
) {
    line(
        out,
        depth,
        &format!("<{name} xsi:type=\"v8:LocalStringType\">"),
    );
    line(out, depth + 1, "<v8:item>");
    scalar(out, depth + 2, "v8:lang", value.language().as_str());
    scalar(out, depth + 2, "v8:content", value.content().as_str());
    line(out, depth + 1, "</v8:item>");
    line(out, depth, &format!("</{name}>"))
}
fn append_indented_fragment(out: &mut String, fragment: &str, depth: usize) {
    let fragment = fragment.trim();
    let base_indent = fragment
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(['\t', ' ']).len())
        .min()
        .unwrap_or(0);
    for (index, raw) in fragment.lines().enumerate() {
        let relative = if index == 0 {
            raw
        } else {
            raw.get(base_indent..).unwrap_or(raw)
        }
        .trim_end();
        line(out, depth, relative);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_source() -> Vec<u8> {
        include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-core/seed/unica-main-template.xml"
        ))
        .to_vec()
    }

    fn core_primary() -> Vec<u8> {
        let body = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-core/raw/f4db0f6c-34f4-4449-995d-6265516e5fa8.0.bin"
        ));
        body[24..24 + 3029].to_vec()
    }

    fn inline_settings(source: &str) -> DcsInlineSettingsFragment {
        let start = source.find("<dcsset:settings").unwrap();
        let close = "</dcsset:settings>";
        let end = start + source[start..].find(close).unwrap() + close.len();
        DcsInlineSettingsFragment::parse(source[start..end].to_owned()).unwrap()
    }

    #[test]
    fn platform_core_storage_parses_to_typed_ir_and_emits_exact_source() {
        let schema = parse_dcs_inner_schema_storage_document(
            &core_primary(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-core/raw",
        )
        .unwrap();
        assert_eq!(schema.data_set().name().as_str(), "Rows");
        assert_eq!(schema.total_fields().len(), 2);
        assert_eq!(schema.settings_variants().len(), 1);

        let source = String::from_utf8(core_source()).unwrap();
        let emitted = emit_dcs_inner_schema_source_document(
            &schema,
            &[inline_settings(source.trim_start_matches('\u{feff}'))],
        )
        .unwrap();
        let emitted = String::from_utf8(emitted).unwrap().replace("\r\n", "\n");
        assert_eq!(emitted.trim_end(), source.trim_end());
    }

    #[test]
    fn wrong_namespace_type_and_total_expression_fail_closed() {
        let primary = String::from_utf8(core_primary()).unwrap();
        for mutated in [
            primary.replace("xsi:type=\"DataSetObject\"", "xsi:type=\"xs:string\""),
            primary.replace("Sum(Amount)", "SUM(Amount)"),
            primary.replace("<dataSource>", "<x:dataSource xmlns:x=\"urn:future\">"),
        ] {
            assert!(matches!(
                parse_dcs_inner_schema_storage_document(
                    mutated.as_bytes(),
                    ProfileId::parse("provider:mssql-legacy").unwrap(),
                    "fixture:negative",
                ),
                Err(DcsInnerSchemaError::UnsupportedSource(_))
                    | Err(DcsInnerSchemaError::Malformed(_))
            ));
        }
    }
}
