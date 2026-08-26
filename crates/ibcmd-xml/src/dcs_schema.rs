//! Evidence-bounded codec for the first typed inner DCS schema cohort.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::dcs::DcsAppearanceColor;
use ibcmd_core::dcs_schema::{
    DcsSchema, DcsSchemaAreaTemplate, DcsSchemaBooleanParameter, DcsSchemaBuildError,
    DcsSchemaCalculatedField, DcsSchemaDataSetField, DcsSchemaDataSetLink, DcsSchemaDataSetObject,
    DcsSchemaDecimalParameter, DcsSchemaDecimalType, DcsSchemaFieldType, DcsSchemaLocalDataSource,
    DcsSchemaLocalString, DcsSchemaParameterDecimalType, DcsSchemaParameterScalarTypes,
    DcsSchemaQueryDataSet, DcsSchemaQueryField, DcsSchemaQueryUnionLink, DcsSchemaReferenceType,
    DcsSchemaSettingsVariantShell, DcsSchemaStandardPeriodParameter,
    DcsSchemaStandardPeriodVariant, DcsSchemaStringParameter, DcsSchemaStringType,
    DcsSchemaTotalFunction, DcsSchemaUngroupedTotalField, DcsSchemaUnionDataSet,
    DcsStyleColorReference,
};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::value::CanonicalText;
use ibcmd_schema::{
    DcsAreaTemplatePolicy, DcsInnerSchemaPolicy, DcsParameterScalarTypesPolicy,
    bundled_dcs_area_template_policy, bundled_dcs_inner_schema_policy,
    bundled_dcs_parameter_scalar_types_policy, bundled_dcs_query_union_link_policy,
};
use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

const MAX_DEPTH: usize = 64;

/// The runaway-loop guard for one XML scan, derived from the document itself.
///
/// Every event and every token a scan yields consumes at least one byte of the
/// document, so a scan that has produced more of them than the document has
/// bytes is not making progress. That is the whole property the guard is there
/// for, and it is the document's own size that states it: a flat constant
/// instead states a maximum document size, which is a claim about the platform
/// nothing measured. UH 3.2.12.6 has data-composition templates whose stored
/// primary schema runs past 32 768 events -- real platform documents the
/// platform itself exports -- and a flat cap refused them for being large.
/// The `+ 1` admits the terminating end-of-input event, which consumes none.
fn scan_bound(bytes: usize) -> usize {
    bytes + 1
}

use crate::{
    analyze_dcs_inline_settings_fragment, validate_dcs_inline_settings_fragment_structure,
};

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

    /// Accepts a fragment transliterated from a standalone `Settings`
    /// document rather than emitted from the typed cohort.
    ///
    /// [`Self::parse`] re-runs the full settings analyzer, which is a cohort
    /// membership test: it asks whether an enumerated shape describes the
    /// fragment. A transliterated fragment reproduces whatever shapes its
    /// source document carried, so that question has already been asked and
    /// answered "no" -- asking it again of our own faithful re-spelling would
    /// reject the exact bytes the platform itself writes. The structural
    /// contract every consumer actually relies on (well-formed, rooted at
    /// `{settings}settings`, declarations closable) is still enforced.
    pub fn parse_transliterated(xml: String) -> Result<Self, DcsInnerSchemaError> {
        let policy = policy()?;
        let closed = close_inline_settings_namespaces(&xml, &policy)?;
        validate_dcs_inline_settings_fragment_structure(&closed).map_err(|error| {
            DcsInnerSchemaError::UnsupportedSource(format!(
                "transliterated inline Settings fragment is not a settings element: {error}"
            ))
        })?;
        Ok(Self(xml))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Byte offset of the `>` that terminates the document's first opening tag.
///
/// `>` is a perfectly legal attribute-value character in XML, so the naive
/// "first `>` in the document" answer can land inside a value and split the
/// tag in the wrong place. This scanner tracks the attribute-value quoting
/// state (both `"` and `'` delimiters, exactly as XML defines them) and only
/// accepts a `>` seen outside a value.
fn opening_tag_end(xml: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, byte) in xml.bytes().enumerate() {
        match (quote, byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'>') => return Some(offset),
            (None, _) => {}
        }
    }
    None
}

/// Splices the missing `xmlns:` declarations into an inline `<dcsset:settings>`
/// fragment so it can be analyzed as a standalone document.
///
/// The declarations must land *inside* the opening tag. For the empty
/// self-closing spelling `<dcsset:settings .../>` the tag's final two bytes are
/// `/` and `>`, so the insertion point is the `/`, not the `>`: splicing at the
/// `>` would produce `<dcsset:settings .../ xmlns:dcsset="..."...>`, which is
/// not well-formed and which our own analyzer then rejects.
fn close_inline_settings_namespaces(
    xml: &str,
    policy: &DcsInnerSchemaPolicy,
) -> Result<String, DcsInnerSchemaError> {
    let opening_end = opening_tag_end(xml).ok_or_else(|| {
        DcsInnerSchemaError::Malformed("inline Settings has no opening tag".into())
    })?;
    let opening = &xml[..opening_end];
    if !opening.trim_start().starts_with("<dcsset:settings") {
        return unsupported("inline Settings root does not use the canonical dcsset spelling");
    }
    let insertion = if opening.ends_with('/') {
        opening_end - 1
    } else {
        opening_end
    };
    let mut closed = String::with_capacity(xml.len() + 384);
    closed.push_str(&xml[..insertion]);
    for (prefix, namespace) in [
        ("dcsset", policy.settings_namespace_uri()),
        ("dcscor", "http://v8.1c.ru/8.1/data-composition-system/core"),
        ("v8", policy.data_core_namespace_uri()),
        ("v8ui", "http://v8.1c.ru/8.1/data/ui"),
        ("xs", policy.xml_schema_namespace_uri()),
        ("xsi", policy.xsi_namespace_uri()),
    ] {
        if !opening.contains(&format!("xmlns:{prefix}=")) {
            closed.push_str(" xmlns:");
            closed.push_str(prefix);
            closed.push_str("=\"");
            closed.push_str(namespace);
            closed.push('"');
        }
    }
    closed.push_str(&xml[insertion..]);
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
    parse_dcs_inner_schema_storage_document_with_references(
        bytes,
        source_profile,
        locator,
        &BTreeMap::new(),
    )
}

/// Parses the bounded schema cohort while resolving configuration-local
/// storage TypeId values to semantic current-configuration qualified names.
pub fn parse_dcs_inner_schema_storage_document_with_references(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
    reference_types: &BTreeMap<String, String>,
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
    let mut cursor = 0usize;
    let data_source = parse_data_source(take(&children, &mut cursor, "dataSource")?, &policy)?;
    let data_set = parse_data_set(
        take(&children, &mut cursor, "dataSet")?,
        &policy,
        reference_types,
    )?;
    let rich = children
        .get(cursor)
        .is_some_and(|child| child.local == "calculatedField");
    let calculated = rich
        .then(|| parse_calculated(take(&children, &mut cursor, "calculatedField")?, &policy))
        .transpose()?;
    let mut totals = Vec::with_capacity(if rich { 2 } else { 0 });
    while children
        .get(cursor)
        .is_some_and(|child| child.local == "totalField")
    {
        totals.push(parse_total(
            take(&children, &mut cursor, "totalField")?,
            &policy,
        )?);
    }
    let parameter = rich
        .then(|| parse_parameter(take(&children, &mut cursor, "parameter")?, &policy))
        .transpose()?;
    let scalar_parameters = if parameter.is_some()
        && children
            .get(cursor)
            .is_some_and(|child| child.local == "parameter")
    {
        Some(parse_parameter_scalar_types(
            &children,
            &mut cursor,
            &policy,
        )?)
    } else {
        None
    };
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
    let schema = match (calculated, parameter) {
        (Some(calculated), Some(parameter)) => DcsSchema::new(
            data_source,
            data_set,
            calculated,
            totals,
            parameter,
            variants,
            provenance,
        ),
        (None, None) if totals.is_empty() && scalar_parameters.is_none() => {
            DcsSchema::new_simple(data_source, data_set, variants, provenance)
        }
        _ => return unsupported("inner schema mixes simple and rich cohort members"),
    }
    .map_err(DcsInnerSchemaError::Build)?;
    match scalar_parameters {
        Some(scalar_parameters) => schema
            .with_scalar_parameters(scalar_parameters)
            .map_err(DcsInnerSchemaError::Build),
        None => Ok(schema),
    }
}

/// Parses the exact one-Query/one-Union/one-link storage cohort.
pub fn parse_dcs_query_union_link_storage_document(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
) -> Result<DcsSchemaQueryUnionLink, DcsInnerSchemaError> {
    parse_dcs_query_union_link_storage_document_with_references(
        bytes,
        source_profile,
        locator,
        &BTreeMap::new(),
    )
}

/// Parses the Query/Union/link storage cohort exactly like
/// [`parse_dcs_query_union_link_storage_document`], but also resolves the
/// second evidenced `DataSetQuery` field's current-config TypeId reference
/// (the `dcs-query-union-link-typeid` cohort's `Owner`/`CatalogRef.FilterProbe`
/// construction, transplanted byte-for-byte from `dcs-typeid-reference`'s own
/// DataSetObject field) via `reference_types` -- the same uuid-to-semantic-name
/// map shape/convention
/// [`parse_dcs_inner_schema_storage_document_with_references`]'s own field
/// resolution already uses. Without a matching entry, that one coordinate
/// fails closed exactly as the plain function does; every other coordinate
/// is unaffected. Only the top-level query may resolve a second field; the
/// `DataSetUnion` item position has no evidence for one and always parses
/// as the original single-field shape.
pub fn parse_dcs_query_union_link_storage_document_with_references(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaQueryUnionLink, DcsInnerSchemaError> {
    let p = policy()?;
    let qp = bundled_dcs_query_union_link_policy()
        .map_err(|e| DcsInnerSchemaError::InvalidEvidence(e.to_string()))?;
    let document = parse_document(bytes)?;
    require_name(&document, None, "SchemaFile")?;
    require_no_attributes(&document)?;
    let wrapper = elements(&document)?;
    if wrapper.len() != 1 {
        return unsupported("SchemaFile must contain exactly one schema root");
    }
    let root = wrapper[0];
    require_name(
        root,
        Some(p.schema_namespace_uri()),
        "dataCompositionSchema",
    )?;
    require_no_attributes(root)?;
    let children = elements(root)?;
    if children.len() != 5 {
        return unsupported("Query/Union/link root must contain exactly five children");
    }
    let data_source = parse_data_source(children[0], &p)?;
    let query = parse_query(children[1], &p, &qp, false, reference_types)?;
    let union = parse_union(children[2], &p, &qp, reference_types)?;
    let link = parse_link(children[3], &p, &qp)?;
    let variant = parse_variant(children[4], &p)?;
    let anchor = CanonicalAnchor::new(
        ObjectPath::new(vec![PathSegment::name("dcs_schema").expect("static")]).expect("static"),
        PropertyPath::root(),
    );
    let provenance = SourceProvenance::with_locator(source_profile, anchor, locator)
        .map_err(|e| DcsInnerSchemaError::Malformed(e.to_string()))?;
    DcsSchemaQueryUnionLink::new(data_source, query, union, link, vec![variant], provenance)
        .map_err(DcsInnerSchemaError::Build)
}

/// Parses the exact style-free trailing AreaTemplate `SchemaFile`.
pub fn parse_dcs_area_template_storage_document(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
) -> Result<DcsSchemaAreaTemplate, DcsInnerSchemaError> {
    parse_dcs_area_template_storage_document_with_references(
        bytes,
        source_profile,
        locator,
        &BTreeMap::new(),
    )
}

/// Parses the bounded AreaTemplate storage cohort while resolving
/// configuration-local storage uuids (the evidenced `0:<uuid>` custom
/// `StyleItem` wire form) to semantic style names. `reference_types` maps
/// lowercased uuid text to semantic name -- the same shape/convention as
/// the TypeId-reference resolver. Building this map from configuration
/// metadata is an adapter-supplied concern; this codec never resolves a
/// uuid by string heuristics, only by exact lookup in the supplied map.
pub fn parse_dcs_area_template_storage_document_with_references(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaAreaTemplate, DcsInnerSchemaError> {
    let p = policy()?;
    let ap = area_policy()?;
    let document = parse_document(bytes)?;
    require_name(&document, None, "SchemaFile")?;
    require_no_attributes(&document)?;
    let wrapper = elements(&document)?;
    if wrapper.is_empty() || wrapper.len() > 2 {
        return unsupported("AreaTemplate SchemaFile has unsupported root cardinality");
    }
    let root = wrapper[0];
    require_name(
        root,
        Some(p.schema_namespace_uri()),
        "dataCompositionSchema",
    )?;
    require_no_attributes(root)?;
    let top = elements(root)?;
    if top.len() != 1 {
        return unsupported("AreaTemplate schema must contain exactly one template");
    }
    let area = parse_area_template_element(top[0], source_profile, locator, &ap)?;
    let area = if wrapper.len() == 2 {
        if area.has_shared_row_appearance() {
            require_storage_shared_row_appearance(wrapper[1], &p, &ap, reference_types)?;
            area
        } else if area.has_parameter_appearance() {
            match require_storage_area_appearance(wrapper[1], &p, &ap, reference_types)? {
                Some(ExtraAppearanceItem::Color(color)) => {
                    area.with_color_and_parameter_appearance(color)
                }
                Some(ExtraAppearanceItem::StyleReference(reference)) => {
                    area.with_style_reference_and_parameter_appearance(reference)
                }
                None => area,
            }
        } else {
            return unsupported("AreaTemplate side table has no matching appIndex");
        }
    } else if area.has_parameter_appearance() || area.has_shared_row_appearance() {
        return unsupported("AreaTemplate appIndex has no appearance side table");
    } else {
        area
    };
    Ok(area)
}

/// Finds and parses the single direct style-free AreaTemplate in a source
/// DataCompositionSchema document. Other root children remain owned by the
/// bounded inner-schema codec.
pub fn parse_dcs_area_template_source_document(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
) -> Result<Option<DcsSchemaAreaTemplate>, DcsInnerSchemaError> {
    let p = policy()?;
    let ap = area_policy()?;
    let document = parse_document(bytes)?;
    require_name(
        &document,
        Some(p.schema_namespace_uri()),
        "DataCompositionSchema",
    )?;
    require_no_attributes(&document)?;
    let templates = elements(&document)?
        .into_iter()
        .filter(|child| {
            child.namespace.as_deref() == Some(p.schema_namespace_uri())
                && child.local == "template"
        })
        .collect::<Vec<_>>();
    match templates.as_slice() {
        [] => Ok(None),
        [template] => parse_area_template_element(template, source_profile, locator, &ap).map(Some),
        _ => unsupported("source schema contains more than one AreaTemplate"),
    }
}

fn parse_area_template_element(
    area_template: &ParsedElement,
    source_profile: ProfileId,
    locator: &str,
    ap: &DcsAreaTemplatePolicy,
) -> Result<DcsSchemaAreaTemplate, DcsInnerSchemaError> {
    let p = policy()?;
    require_name(area_template, Some(p.schema_namespace_uri()), "template")?;
    require_no_attributes(area_template)?;
    let template = elements(area_template)?;
    if template.len() != 3 {
        return unsupported("AreaTemplate must contain name, template and parameter");
    }
    require_name(template[0], Some(p.schema_namespace_uri()), "name")?;
    require_no_attributes(template[0])?;
    require_name(template[1], Some(p.schema_namespace_uri()), "template")?;
    require_type(template[1], &p, &ap.area_template_type_qname())?;
    require_name(template[2], Some(p.schema_namespace_uri()), "parameter")?;
    require_type(template[2], &p, &ap.expression_parameter_type_qname())?;
    let area_body = parse_exact_area_body(template[1], &p, ap)?;
    let parameter = elements(template[2])?;
    if parameter.len() != 2 {
        return unsupported("AreaTemplate parameter must contain name and expression");
    }
    let area_ns = ap.area_namespace_uri();
    require_name(parameter[0], Some(area_ns), "name")?;
    require_name(parameter[1], Some(area_ns), "expression")?;
    require_no_attributes(parameter[0])?;
    require_no_attributes(parameter[1])?;
    let anchor = CanonicalAnchor::new(
        ObjectPath::new(vec![
            PathSegment::name("dcs_area_template").expect("static"),
        ])
        .expect("static"),
        PropertyPath::root(),
    );
    let provenance = SourceProvenance::with_locator(source_profile, anchor, locator)
        .map_err(|e| DcsInnerSchemaError::Malformed(e.to_string()))?;
    let area = DcsSchemaAreaTemplate::new(
        canonical(text(template[0])?)?,
        canonical(text(parameter[0])?)?,
        canonical(text(parameter[1])?)?,
        provenance,
    )
    .map_err(DcsInnerSchemaError::Build)?;
    Ok(match area_body {
        ParsedAreaBody::SingleCell {
            has_appearance: false,
            ..
        } => area,
        ParsedAreaBody::SingleCell {
            has_appearance: true,
            extra: Some(ExtraAppearanceItem::Color(color)),
        } => area.with_color_and_parameter_appearance(color),
        ParsedAreaBody::SingleCell {
            has_appearance: true,
            extra: Some(ExtraAppearanceItem::StyleReference(reference)),
        } => area.with_style_reference_and_parameter_appearance(reference),
        ParsedAreaBody::SingleCell {
            has_appearance: true,
            extra: None,
        } => area.with_parameter_appearance(),
        ParsedAreaBody::SharedRowAppearance => area.with_shared_row_appearance(),
    })
}

/// Which document direction an appearance body was found in. The two
/// directions authenticate different lexical spellings for the same
/// logical parameters once the color item co-occurs (see
/// `DcsAreaTemplatePolicy::storage_appearance_parameter_with_color`) or the
/// side-table entry is shared by more than one cell (see
/// `DcsAreaTemplatePolicy::storage_shared_row_appearance_parameter`).
#[derive(Clone, Copy)]
enum AreaAppearanceDirection {
    Source,
    Storage,
}

/// The outcome of parsing an AreaTemplate's row/cell body: either the
/// original single-row, single-cell shape (with its own optional
/// appearance/extra item), or the evidenced two-row shared-appearance
/// shape.
enum ParsedAreaBody {
    SingleCell {
        has_appearance: bool,
        extra: Option<ExtraAppearanceItem>,
    },
    SharedRowAppearance,
}

/// One authenticated "extra" appearance item preceding `Расшифровка`/
/// `Details`: either the web-color cohort's `ЦветТекста`/`TextColor`, or
/// the style-reference cohort's `ЦветФона`/`BackColor`. Evidence-bound to
/// be mutually exclusive; no cohort proves the two co-occurring.
enum ExtraAppearanceItem {
    Color(DcsAppearanceColor),
    StyleReference(DcsStyleColorReference),
}

/// One `tableCell`'s parsed appearance signal: no second child, an embedded
/// source `dcsat:appearance` (with any extra item found inside it), or a
/// storage `appIndex` (whose raw text the caller must validate -- the
/// side-table wrapper elsewhere is the sole authority for what that
/// index's content actually is).
enum TableCellAppearanceSignal {
    Absent,
    Source(Option<ExtraAppearanceItem>),
    Storage(String),
}

fn parse_exact_area_body(
    area: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<ParsedAreaBody, DcsInnerSchemaError> {
    let area_ns = ap.area_namespace_uri();
    let rows = elements(area)?;
    match rows.len() {
        1 => {
            require_name(rows[0], Some(area_ns), "item")?;
            require_type(rows[0], p, &ap.table_row_type_qname())?;
            let cells = elements(rows[0])?;
            if cells.len() != 1 {
                return unsupported("AreaTemplate row must contain exactly one tableCell");
            }
            let (has_appearance, extra) = match parse_table_cell(cells[0], p, ap)? {
                TableCellAppearanceSignal::Absent => (false, None),
                TableCellAppearanceSignal::Source(extra) => (true, extra),
                TableCellAppearanceSignal::Storage(index) => {
                    if index != "0" {
                        return unsupported(
                            "AreaTemplate appIndex is outside the exact coordinate",
                        );
                    }
                    (true, None)
                }
            };
            Ok(ParsedAreaBody::SingleCell {
                has_appearance,
                extra,
            })
        }
        2 => {
            parse_shared_row_appearance_body(&rows, p, ap)?;
            Ok(ParsedAreaBody::SharedRowAppearance)
        }
        _ => unsupported("AreaTemplate must contain one or two rows"),
    }
}

/// Validates the evidenced two-row shape: row 1 has exactly two `tableCell`s
/// that must both carry the *same* appearance signal (both an embedded
/// source appearance with no color, or both a storage `appIndex` equal to
/// `0`); row 2 has exactly one `tableCell` with no appearance at all. Any
/// divergence between the two row-1 cells, or any appearance on row 2, is
/// outside the evidenced cohort.
fn parse_shared_row_appearance_body(
    rows: &[&ParsedElement],
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<(), DcsInnerSchemaError> {
    let area_ns = ap.area_namespace_uri();
    let (row1, row2) = (rows[0], rows[1]);
    require_name(row1, Some(area_ns), "item")?;
    require_type(row1, p, &ap.table_row_type_qname())?;
    let row1_cells = elements(row1)?;
    if row1_cells.len() != 2 {
        return unsupported(
            "AreaTemplate shared-appearance row 1 must contain exactly two tableCells",
        );
    }
    let first = parse_table_cell(row1_cells[0], p, ap)?;
    let second = parse_table_cell(row1_cells[1], p, ap)?;
    match (first, second) {
        (TableCellAppearanceSignal::Source(None), TableCellAppearanceSignal::Source(None)) => {}
        (
            TableCellAppearanceSignal::Storage(first_index),
            TableCellAppearanceSignal::Storage(second_index),
        ) => {
            if first_index != "0" || second_index != "0" {
                return unsupported(
                    "AreaTemplate shared-appearance appIndex is outside the exact coordinate",
                );
            }
        }
        _ => {
            return unsupported(
                "AreaTemplate shared-appearance row 1 cells diverge from the exact coordinate",
            );
        }
    }

    require_name(row2, Some(area_ns), "item")?;
    require_type(row2, p, &ap.table_row_type_qname())?;
    let row2_cells = elements(row2)?;
    if row2_cells.len() != 1 {
        return unsupported(
            "AreaTemplate shared-appearance row 2 must contain exactly one tableCell",
        );
    }
    match parse_table_cell(row2_cells[0], p, ap)? {
        TableCellAppearanceSignal::Absent => Ok(()),
        _ => unsupported("AreaTemplate shared-appearance row 2 cell must have no appearance"),
    }
}

/// Parses one `tableCell`'s `Field` item plus its optional appearance tail,
/// shared by the single-cell and two-row shared-appearance cohorts. The
/// platform always canonicalizes storage/native output to Field-first,
/// appearance-second regardless of source order (a locally reversed order,
/// as the non-authoritative multi-cell-appearance seed happened to spell
/// it, is rejected here because `cell[0]` will not resolve to the Field
/// type).
fn parse_table_cell(
    cell_element: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<TableCellAppearanceSignal, DcsInnerSchemaError> {
    let area_ns = ap.area_namespace_uri();
    require_name(cell_element, Some(area_ns), "tableCell")?;
    require_no_attributes(cell_element)?;
    let cell = elements(cell_element)?;
    if cell.is_empty() || cell.len() > 2 {
        return unsupported("AreaTemplate cell child cardinality is outside the cohort");
    }
    require_name(cell[0], Some(area_ns), "item")?;
    require_type(cell[0], p, &ap.field_type_qname())?;
    let field = elements(cell[0])?;
    if field.len() != 1 {
        return unsupported("AreaTemplate Field must contain exactly one value");
    }
    require_name(field[0], Some(area_ns), "value")?;
    require_type(field[0], p, &ap.parameter_value_type_qname())?;
    if text_allowing_attributes(field[0])? != ap.parameter_name() {
        return unsupported("AreaTemplate field value is outside the exact coordinate");
    }
    match cell.get(1) {
        None => Ok(TableCellAppearanceSignal::Absent),
        Some(appearance)
            if appearance.namespace.as_deref() == Some(area_ns)
                && appearance.local == "appearance" =>
        {
            let color = require_source_area_appearance(appearance, p, ap)?;
            Ok(TableCellAppearanceSignal::Source(color))
        }
        Some(index) if index.namespace.as_deref() == Some(area_ns) && index.local == "appIndex" => {
            require_no_attributes(index)?;
            Ok(TableCellAppearanceSignal::Storage(text(index)?))
        }
        Some(_) => unsupported("AreaTemplate cell second child is outside the exact coordinate"),
    }
}

fn require_source_area_appearance(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<Option<ExtraAppearanceItem>, DcsInnerSchemaError> {
    require_no_attributes(appearance)?;
    require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Source,
        ap.appearance_parameter(),
        &BTreeMap::new(),
    )
}

fn require_storage_area_appearance(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    reference_types: &BTreeMap<String, String>,
) -> Result<Option<ExtraAppearanceItem>, DcsInnerSchemaError> {
    require_name(appearance, Some(ap.area_namespace_uri()), "appearance")?;
    require_type(appearance, p, &ap.table_cell_appearance_type_qname())?;
    require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Storage,
        ap.appearance_parameter(),
        reference_types,
    )
}

/// Validates the storage side-table entry shared by both row-1 cells of
/// the two-row shared-appearance cohort. Unlike the single-cell storage
/// entry, this one is spelled `Details` (see
/// `DcsAreaTemplatePolicy::storage_shared_row_appearance_parameter`) even
/// though it holds only one item and no extra item -- the evidenced
/// discriminator is the record being referenced by more than one cell, not
/// its own item count. An extra item here would be unevidenced, so it is
/// rejected rather than silently accepted.
fn require_storage_shared_row_appearance(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    reference_types: &BTreeMap<String, String>,
) -> Result<(), DcsInnerSchemaError> {
    require_name(appearance, Some(ap.area_namespace_uri()), "appearance")?;
    require_type(appearance, p, &ap.table_cell_appearance_type_qname())?;
    match require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Storage,
        ap.storage_shared_row_appearance_parameter(),
        reference_types,
    )? {
        None => Ok(()),
        Some(_) => unsupported(
            "AreaTemplate shared-row appearance side table must not contain a color or style-reference item",
        ),
    }
}

/// Validates the shared `dcscor:item`/`item` appearance body shape and
/// returns the extra item, if the evidenced two-item state was found (the
/// web-color cohort's `ЦветТекста`/`ЦветФона` + `Расшифровка`, or the
/// style-reference cohort's `ЦветФона` + `Расшифровка`). Exactly one or two
/// items are admitted; the extra item, when present, must be first.
/// `expected_single_item_parameter` is the literal expected for the lone
/// item in the one-item state, which differs between the plain single-cell
/// storage entry (`Расшифровка`) and the shared-row storage entry
/// (`Details`); both directions' single-cell and two-item cases always
/// expect `Расшифровка`/`storage_appearance_parameter_with_color`.
fn require_parameter_appearance_body(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    direction: AreaAppearanceDirection,
    expected_single_item_parameter: &str,
    reference_types: &BTreeMap<String, String>,
) -> Result<Option<ExtraAppearanceItem>, DcsInnerSchemaError> {
    let items = elements(appearance)?;
    match items.len() {
        1 => {
            require_parameter_item(items[0], p, ap, expected_single_item_parameter)?;
            Ok(None)
        }
        2 => {
            let extra =
                require_color_or_style_reference_item(items[0], p, ap, direction, reference_types)?;
            let expected_parameter = match direction {
                AreaAppearanceDirection::Source => ap.appearance_parameter(),
                AreaAppearanceDirection::Storage => ap.storage_appearance_parameter_with_color(),
            };
            require_parameter_item(items[1], p, ap, expected_parameter)?;
            Ok(Some(extra))
        }
        _ => unsupported("AreaTemplate appearance item cardinality is outside the cohort"),
    }
}

fn require_parameter_item(
    item: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    expected_parameter: &str,
) -> Result<(), DcsInnerSchemaError> {
    require_name(item, Some(ap.core_namespace_uri()), "item")?;
    require_no_attributes(item)?;
    let children = elements(item)?;
    if children.len() != 2 {
        return unsupported("AreaTemplate appearance item must contain parameter and value");
    }
    require_name(children[0], Some(ap.core_namespace_uri()), "parameter")?;
    require_no_attributes(children[0])?;
    require_name(children[1], Some(ap.core_namespace_uri()), "value")?;
    require_type(children[1], p, &ap.parameter_value_type_qname())?;
    let parameter = text(children[0])?;
    let parameter = parameter.trim();
    if parameter != expected_parameter
        || text_allowing_attributes(children[1])? != ap.parameter_name()
    {
        return unsupported("AreaTemplate appearance value is outside the exact coordinate");
    }
    Ok(())
}

/// Dispatches the shared `dcscor:item`/`item` shape to either the
/// web-color cohort (`ЦветТекста`/`TextColor`) or the style-reference
/// cohort (`ЦветФона`/`BackColor`), by the item's own `parameter` text --
/// the two are mutually exclusive by evidence and never ambiguous, since
/// they use disjoint parameter names in both directions.
fn require_color_or_style_reference_item(
    item: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    direction: AreaAppearanceDirection,
    reference_types: &BTreeMap<String, String>,
) -> Result<ExtraAppearanceItem, DcsInnerSchemaError> {
    require_name(item, Some(ap.core_namespace_uri()), "item")?;
    require_no_attributes(item)?;
    let children = elements(item)?;
    if children.len() != 2 {
        return unsupported(
            "AreaTemplate appearance color/style-reference item must contain parameter and value",
        );
    }
    require_name(children[0], Some(ap.core_namespace_uri()), "parameter")?;
    require_no_attributes(children[0])?;
    require_name(children[1], Some(ap.core_namespace_uri()), "value")?;
    let parameter = text(children[0])?;
    let parameter = parameter.trim();
    let color_parameter = match direction {
        AreaAppearanceDirection::Source => ap.text_color_parameter(),
        AreaAppearanceDirection::Storage => ap.storage_text_color_parameter(),
    };
    let style_reference_parameter = match direction {
        AreaAppearanceDirection::Source => ap.back_color_parameter(),
        AreaAppearanceDirection::Storage => ap.storage_back_color_parameter(),
    };
    if parameter == color_parameter {
        require_color_value(children[1], p, ap).map(ExtraAppearanceItem::Color)
    } else if parameter == style_reference_parameter {
        require_style_reference_value(children[1], p, ap, direction, reference_types)
            .map(ExtraAppearanceItem::StyleReference)
    } else {
        unsupported(
            "AreaTemplate appearance color/style-reference parameter is outside the exact coordinate",
        )
    }
}

fn require_color_value(
    value: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<DcsAppearanceColor, DcsInnerSchemaError> {
    require_type(value, p, &ap.color_type_qname())?;
    // Compare only the expanded QName: the platform does not preserve the
    // source prefix spelling, and the evidenced cohort admits any prefix
    // bound to the web-colors namespace (native uses an auto-generated
    // `d8p1`/`d4p2`, the seed uses a locally-declared `web`).
    if resolve_qname_text_allowing_attributes(value)? != ap.web_red_qname() {
        return unsupported("AreaTemplate appearance color value is outside the exact coordinate");
    }
    Ok(DcsAppearanceColor::WebRed)
}

/// Recognizes the evidenced `0:<uuid>` storage wire syntax for a raw
/// custom-`StyleItem` reference: a fixed discriminator prefix (not an XML
/// namespace prefix), followed by the referenced StyleItem's own
/// configuration-local uuid in canonical 8-4-4-4-12 hyphenated hex form.
/// This recognizes the *syntax* only; resolving the uuid to a semantic
/// name is the caller's job via an evidence-backed resolver map, never a
/// string heuristic on the uuid's own digits.
fn parse_style_item_storage_uuid_reference(value: &str) -> Option<&str> {
    let uuid = value.strip_prefix("0:")?;
    let bytes = uuid.as_bytes();
    if bytes.len() != 36 {
        return None;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return None;
            }
        } else if !byte.is_ascii_hexdigit() {
            return None;
        }
    }
    Some(uuid)
}

fn require_style_reference_value(
    value: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    direction: AreaAppearanceDirection,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsStyleColorReference, DcsInnerSchemaError> {
    require_type(value, p, &ap.color_type_qname())?;
    let value_text = text_allowing_attributes(value)?;
    let value_text = value_text.trim();
    if let Some(uuid) = parse_style_item_storage_uuid_reference(value_text) {
        if !matches!(direction, AreaAppearanceDirection::Storage) {
            return unsupported(
                "AreaTemplate appearance style-reference raw uuid form is only evidenced on the storage direction",
            );
        }
        let lowercase_uuid = uuid.to_ascii_lowercase();
        let name = reference_types.get(&lowercase_uuid).ok_or_else(|| {
            DcsInnerSchemaError::UnsupportedSource(
                "style-reference uuid has no evidence-backed semantic resolution".to_string(),
            )
        })?;
        if name != ap.custom_style_item_name() {
            return unsupported(
                "resolved StyleItem name is outside the evidenced style-reference cohort",
            );
        }
        return DcsStyleColorReference::custom_style_item(canonical(name.clone())?)
            .map_err(DcsInnerSchemaError::Build);
    }
    // Not the raw-uuid wire form: must be a QName-resolvable lexical token.
    // The platform does not preserve the source prefix spelling; compare
    // only the expanded QName. Both evidenced forms are lexically
    // *identical* shapes at the source layer -- `style:NegativeTextColor`
    // and `style:CorpusAccent` are indistinguishable except by which name
    // resolves -- so this branch alone can never disambiguate them from
    // syntax; only the literal name (checked against each cohort's exact
    // evidenced value) does. The named custom-StyleItem spelling is only
    // evidenced on the source direction: storage always spells it as the
    // raw uuid form checked above instead.
    let qname = resolve_qname(value, value_text)?;
    if qname == ap.negative_text_color_qname() {
        return DcsStyleColorReference::named(canonical("NegativeTextColor".to_string())?)
            .map_err(DcsInnerSchemaError::Build);
    }
    if qname
        == format!(
            "{{{}}}{}",
            ap.style_namespace_uri(),
            ap.custom_style_item_name()
        )
    {
        if !matches!(direction, AreaAppearanceDirection::Source) {
            return unsupported(
                "AreaTemplate appearance custom-StyleItem named spelling is only evidenced on the source direction",
            );
        }
        return DcsStyleColorReference::custom_style_item(canonical(
            ap.custom_style_item_name().to_string(),
        )?)
        .map_err(DcsInnerSchemaError::Build);
    }
    unsupported("AreaTemplate appearance style-reference value is outside the exact coordinate")
}

fn parse_query(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    qp: &ibcmd_schema::DcsQueryUnionLinkPolicy,
    nested: bool,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaQueryDataSet, DcsInnerSchemaError> {
    require_name(
        e,
        Some(p.schema_namespace_uri()),
        if nested { "item" } else { "dataSet" },
    )?;
    require_type(e, p, qp.query_type_qname())?;
    // Exactly two evidenced shapes, selected by child count (the same
    // "select the evidenced list by count" convention `parse_link` uses):
    // the original single-field cohort, or the `dcs-query-union-link-typeid`
    // cohort's second, typed field. The `DataSetUnion` item position (`nested`)
    // has no evidence for the second form.
    let child_count = element_children(e)?.len();
    let with_typed_field = if child_count == qp.query_children().len() {
        false
    } else if !nested && child_count == qp.query_children_with_typed_field().len() {
        true
    } else {
        return unsupported("dataSet child cardinality is outside the cohort");
    };
    let names = if with_typed_field {
        qp.query_children_with_typed_field()
    } else {
        qp.query_children()
    };
    let c = exact_children(e, names, p.schema_namespace_uri())?;
    let field_children = exact_children(
        c[1],
        &[
            format!("{{{}}}dataPath", p.schema_namespace_uri()),
            format!("{{{}}}field", p.schema_namespace_uri()),
        ],
        p.schema_namespace_uri(),
    )?;
    require_type(c[1], p, qp.field_type_qname())?;
    let field = DcsSchemaQueryField::new(
        canonical(text(field_children[0])?)?,
        canonical(text(field_children[1])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)?;
    let (typed_field, tail_start) = if with_typed_field {
        require_type(c[2], p, qp.field_type_qname())?;
        let typed_field_children = exact_children(
            c[2],
            &[
                format!("{{{}}}dataPath", p.schema_namespace_uri()),
                format!("{{{}}}field", p.schema_namespace_uri()),
                format!("{{{}}}valueType", p.schema_namespace_uri()),
            ],
            p.schema_namespace_uri(),
        )?;
        let data_path = canonical(text(typed_field_children[0])?)?;
        let typed_field_name = canonical(text(typed_field_children[1])?)?;
        if data_path.as_str() != qp.query_typed_field_name()
            || typed_field_name.as_str() != qp.query_typed_field_name()
        {
            return unsupported("query typed field is outside the evidenced cohort");
        }
        let value_type =
            parse_value_type_with_references(typed_field_children[2], p, reference_types)?;
        let typed = DcsSchemaDataSetField::new(data_path, typed_field_name, value_type)
            .map_err(DcsInnerSchemaError::Build)?;
        (Some(typed), 3)
    } else {
        (None, 2)
    };
    let query = DcsSchemaQueryDataSet::new(
        canonical(text(c[0])?)?,
        field,
        typed_field,
        canonical(text(c[tail_start])?)?,
        canonical(text(c[tail_start + 1])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)?;
    require_query_union_link_values(&query, qp)?;
    Ok(query)
}

fn require_query_union_link_values(
    query: &DcsSchemaQueryDataSet,
    policy: &ibcmd_schema::DcsQueryUnionLinkPolicy,
) -> Result<(), DcsInnerSchemaError> {
    if query.field().data_path().as_str() != policy.field()
        || query.field().field().as_str() != policy.field()
        || query.query().as_str() != policy.query_text()
    {
        return unsupported("Query/Union/link field or query text is outside the exact cohort");
    }
    Ok(())
}

fn parse_union(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    qp: &ibcmd_schema::DcsQueryUnionLinkPolicy,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaUnionDataSet, DcsInnerSchemaError> {
    require_name(e, Some(p.schema_namespace_uri()), "dataSet")?;
    require_type(e, p, qp.union_type_qname())?;
    let c = exact_children(e, qp.union_children(), p.schema_namespace_uri())?;
    DcsSchemaUnionDataSet::new(
        canonical(text(c[0])?)?,
        parse_query(c[1], p, qp, true, reference_types)?,
    )
    .map_err(DcsInnerSchemaError::Build)
}

/// Parses `dataSetLink`'s four mandatory children plus its evidenced
/// optional extensions. Exactly three child counts are admitted: the base
/// four alone; the base four plus `parameter`/`parameterListAllowed` (the
/// `dcs-link-parameter` cohort); or all nine, with
/// `linkConditionExpression`/`startExpression`/`required` layered on top
/// (the `dcs-link-expressions` cohort). Any other count, any wrong order
/// within a state, a duplicated field, or an unevidenced literal value all
/// fail closed -- `exact_children` enforces exact name-and-position
/// matching against whichever fixed list this coordinate selects, never a
/// general reordering or subset search.
fn parse_link(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    qp: &ibcmd_schema::DcsQueryUnionLinkPolicy,
) -> Result<DcsSchemaDataSetLink, DcsInnerSchemaError> {
    require_name(e, Some(p.schema_namespace_uri()), "dataSetLink")?;
    require_no_attributes(e)?;
    let base = qp.link_children();
    let optional = qp.link_optional_children_canonical_order();
    let child_count = element_children(e)?.len();
    let names: Vec<String> = if child_count == base.len() {
        base.to_vec()
    } else if child_count == base.len() + 2 {
        base.iter()
            .cloned()
            .chain(optional[..2].iter().cloned())
            .collect()
    } else if child_count == base.len() + 5 {
        base.iter()
            .cloned()
            .chain(optional.iter().cloned())
            .collect()
    } else {
        return unsupported("dataSetLink child cardinality is outside the cohort");
    };
    let c = exact_children(e, &names, p.schema_namespace_uri())?;
    let link = DcsSchemaDataSetLink::new(
        canonical(text(c[0])?)?,
        canonical(text(c[1])?)?,
        canonical(text(c[2])?)?,
        canonical(text(c[3])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)?;
    let link = if c.len() >= 6 {
        let parameter = canonical(text(c[4])?)?;
        let parameter_list_allowed = parse_link_boolean(c[5])?;
        if parameter.as_str() != qp.link_parameter_value()
            || parameter_list_allowed != qp.link_parameter_list_allowed_value()
        {
            return unsupported(
                "dataSetLink parameter/parameterListAllowed is outside the exact coordinate",
            );
        }
        link.with_parameter(parameter, parameter_list_allowed)
            .map_err(DcsInnerSchemaError::Build)?
    } else {
        link
    };
    if c.len() == 9 {
        let link_condition_expression = canonical(text(c[6])?)?;
        let start_expression = canonical(text(c[7])?)?;
        let required = parse_link_boolean(c[8])?;
        if link_condition_expression.as_str() != qp.link_condition_expression_value()
            || start_expression.as_str() != qp.link_start_expression_value()
            || required != qp.link_required_value()
        {
            return unsupported(
                "dataSetLink linkConditionExpression/startExpression/required is outside the exact coordinate",
            );
        }
        link.with_expressions(link_condition_expression, start_expression, required)
            .map_err(DcsInnerSchemaError::Build)
    } else {
        Ok(link)
    }
}

/// Reads a scalar boolean element's text strictly as the XML lexical
/// tokens `true`/`false`; any other text (including `1`/`0`, mixed case,
/// or attributes) fails closed rather than being coerced.
fn parse_link_boolean(e: &ParsedElement) -> Result<bool, DcsInnerSchemaError> {
    match text(e)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => unsupported("dataSetLink boolean field is outside the exact coordinate"),
    }
}

/// The `dataSetLink` boolean-field lexical spelling (`true`/`false`,
/// matching the evidenced XML tokens exactly). Callers only invoke this
/// once the co-occurring group's presence is already established.
fn link_boolean_text(value: Option<bool>) -> &'static str {
    if value == Some(true) { "true" } else { "false" }
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
        emit_value_type(&mut out, 3, field.value_type(), &policy);
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
    if let Some(calculated) = schema.calculated_field() {
        line(&mut out, 1, "<calculatedField>");
        scalar(&mut out, 2, "dataPath", calculated.data_path().as_str());
        scalar(&mut out, 2, "expression", calculated.expression().as_str());
        emit_value_type(
            &mut out,
            2,
            &DcsSchemaFieldType::Decimal(calculated.value_type()),
            &policy,
        );
        line(&mut out, 1, "</calculatedField>");
    }
    for total in schema.total_fields() {
        line(&mut out, 1, "<totalField>");
        scalar(&mut out, 2, "dataPath", total.data_path().as_str());
        let expression = policy
            .sum_total_expression_grammar()
            .replace("{dataPath}", total.data_path().as_str());
        scalar(&mut out, 2, "expression", &expression);
        line(&mut out, 1, "</totalField>");
    }
    if let Some(parameter) = schema.parameter() {
        line(&mut out, 1, "<parameter>");
        scalar(&mut out, 2, "name", parameter.name().as_str());
        emit_local_string(&mut out, 2, "title", parameter.title(), false);
        emit_value_type(
            &mut out,
            2,
            &DcsSchemaFieldType::String(parameter.value_type()),
            &policy,
        );
        let mut value = String::from("<value xsi:type=\"xs:string\">");
        value.push_str(&escape(parameter.value().as_str()));
        value.push_str("</value>");
        line(&mut out, 2, &value);
        scalar(&mut out, 2, "useRestriction", "false");
        line(&mut out, 1, "</parameter>");
    }
    if let Some(scalar_parameters) = schema.scalar_parameters() {
        let flag = scalar_parameters.flag();
        line(&mut out, 1, "<parameter>");
        scalar(&mut out, 2, "name", flag.name().as_str());
        emit_local_string(&mut out, 2, "title", flag.title(), false);
        line(&mut out, 2, "<valueType>");
        scalar(&mut out, 3, "v8:Type", "xs:boolean");
        line(&mut out, 2, "</valueType>");
        line(
            &mut out,
            2,
            &format!(
                "<value xsi:type=\"xs:boolean\">{}</value>",
                if flag.value() { "true" } else { "false" }
            ),
        );
        scalar(&mut out, 2, "useRestriction", "false");
        line(&mut out, 1, "</parameter>");

        let limit = scalar_parameters.limit();
        line(&mut out, 1, "<parameter>");
        scalar(&mut out, 2, "name", limit.name().as_str());
        emit_local_string(&mut out, 2, "title", limit.title(), false);
        line(&mut out, 2, "<valueType>");
        scalar(&mut out, 3, "v8:Type", "xs:decimal");
        line(&mut out, 3, "<v8:NumberQualifiers>");
        scalar(
            &mut out,
            4,
            "v8:Digits",
            &limit.value_type().digits().to_string(),
        );
        scalar(
            &mut out,
            4,
            "v8:FractionDigits",
            &limit.value_type().fraction_digits().to_string(),
        );
        scalar(&mut out, 4, "v8:AllowedSign", "Any");
        line(&mut out, 3, "</v8:NumberQualifiers>");
        line(&mut out, 2, "</valueType>");
        line(
            &mut out,
            2,
            &format!(
                "<value xsi:type=\"xs:decimal\">{}</value>",
                escape(limit.value().as_str())
            ),
        );
        scalar(&mut out, 2, "useRestriction", "false");
        line(&mut out, 1, "</parameter>");

        let period = scalar_parameters.period();
        line(&mut out, 1, "<parameter>");
        scalar(&mut out, 2, "name", period.name().as_str());
        emit_local_string(&mut out, 2, "title", period.title(), false);
        line(&mut out, 2, "<valueType>");
        scalar(&mut out, 3, "v8:Type", "v8:StandardPeriod");
        line(&mut out, 2, "</valueType>");
        line(&mut out, 2, "<value xsi:type=\"v8:StandardPeriod\">");
        let DcsSchemaStandardPeriodVariant::LastMonth = period.variant();
        line(
            &mut out,
            3,
            "<v8:variant xsi:type=\"v8:StandardPeriodVariant\">LastMonth</v8:variant>",
        );
        line(&mut out, 2, "</value>");
        scalar(&mut out, 2, "useRestriction", "false");
        line(&mut out, 1, "</parameter>");
    }
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

pub fn emit_dcs_query_union_link_source_document(
    schema: &DcsSchemaQueryUnionLink,
    settings_blocks: &[DcsInlineSettingsFragment],
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    if settings_blocks.len() != 1 || schema.settings_variants().len() != 1 {
        return unsupported("Query/Union/link requires exactly one Settings block");
    }
    let p = policy()?;
    let qp = bundled_dcs_query_union_link_policy()
        .map_err(|e| DcsInnerSchemaError::InvalidEvidence(e.to_string()))?;
    require_query_union_link_values(schema.query(), &qp)?;
    require_query_union_link_values(schema.union().item(), &qp)?;
    let mut out = source_header(&p);
    line(&mut out, 1, "<dataSource>");
    scalar(&mut out, 2, "name", schema.data_source().name().as_str());
    scalar(&mut out, 2, "dataSourceType", "Local");
    line(&mut out, 1, "</dataSource>");
    emit_query(&mut out, 1, "dataSet", schema.query(), &p);
    line(&mut out, 1, "<dataSet xsi:type=\"DataSetUnion\">");
    scalar(&mut out, 2, "name", schema.union().name().as_str());
    emit_query(&mut out, 2, "item", schema.union().item(), &p);
    line(&mut out, 1, "</dataSet>");
    line(&mut out, 1, "<dataSetLink>");
    for (name, value) in [
        ("sourceDataSet", schema.link().source_data_set()),
        ("destinationDataSet", schema.link().destination_data_set()),
        ("sourceExpression", schema.link().source_expression()),
        (
            "destinationExpression",
            schema.link().destination_expression(),
        ),
    ] {
        scalar(&mut out, 2, name, value.as_str());
    }
    if let Some(parameter) = schema.link().parameter() {
        scalar(&mut out, 2, "parameter", parameter.as_str());
        scalar(
            &mut out,
            2,
            "parameterListAllowed",
            link_boolean_text(schema.link().parameter_list_allowed()),
        );
    }
    if let (Some(link_condition_expression), Some(start_expression)) = (
        schema.link().link_condition_expression(),
        schema.link().start_expression(),
    ) {
        scalar(
            &mut out,
            2,
            "linkConditionExpression",
            link_condition_expression.as_str(),
        );
        scalar(&mut out, 2, "startExpression", start_expression.as_str());
        scalar(
            &mut out,
            2,
            "required",
            link_boolean_text(schema.link().required()),
        );
    }
    line(&mut out, 1, "</dataSetLink>");
    emit_variant(
        &mut out,
        schema.settings_variants().first().expect("one"),
        &settings_blocks[0],
    );
    out.push_str("\r\n</DataCompositionSchema>");
    Ok(out.into_bytes())
}

/// Emits the exact source fragment owned by the style-free AreaTemplate
/// coordinate. The caller owns only its direct root placement.
pub fn emit_dcs_area_template_source_fragment(
    area: &DcsSchemaAreaTemplate,
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    if area.parameter_name().as_str() != "Probe" || area.expression().as_str() != "\"Probe\"" {
        return unsupported("AreaTemplate value is outside the exact coordinate");
    }
    let mut out = String::from("\t<template>");
    scalar(&mut out, 2, "name", area.name().as_str());
    line(
        &mut out,
        2,
        "<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">",
    );
    if area.has_shared_row_appearance() {
        line(&mut out, 3, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        for _ in 0..2 {
            line(&mut out, 4, "<dcsat:tableCell>");
            line(&mut out, 5, "<dcsat:item xsi:type=\"dcsat:Field\">");
            line(
                &mut out,
                6,
                "<dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
            );
            line(&mut out, 5, "</dcsat:item>");
            line(&mut out, 5, "<dcsat:appearance>");
            line(&mut out, 6, "<dcscor:item>");
            scalar(&mut out, 7, "dcscor:parameter", "Расшифровка");
            line(
                &mut out,
                7,
                "<dcscor:value xsi:type=\"dcscor:Parameter\">Probe</dcscor:value>",
            );
            line(&mut out, 6, "</dcscor:item>");
            line(&mut out, 5, "</dcsat:appearance>");
            line(&mut out, 4, "</dcsat:tableCell>");
        }
        line(&mut out, 3, "</dcsat:item>");
        line(&mut out, 3, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        line(&mut out, 4, "<dcsat:tableCell>");
        line(&mut out, 5, "<dcsat:item xsi:type=\"dcsat:Field\">");
        line(
            &mut out,
            6,
            "<dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
        );
        line(&mut out, 5, "</dcsat:item>");
        line(&mut out, 4, "</dcsat:tableCell>");
        line(&mut out, 3, "</dcsat:item>");
    } else {
        line(&mut out, 3, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        line(&mut out, 4, "<dcsat:tableCell>");
        line(&mut out, 5, "<dcsat:item xsi:type=\"dcsat:Field\">");
        line(
            &mut out,
            6,
            "<dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
        );
        line(&mut out, 5, "</dcsat:item>");
        if area.has_parameter_appearance() {
            line(&mut out, 5, "<dcsat:appearance>");
            if let Some(color) = area.text_color_appearance() {
                line(&mut out, 6, "<dcscor:item>");
                scalar(&mut out, 7, "dcscor:parameter", "ЦветТекста");
                line(&mut out, 7, source_color_value_fragment(color));
                line(&mut out, 6, "</dcscor:item>");
            } else if let Some(reference) = area.back_color_style_reference() {
                line(&mut out, 6, "<dcscor:item>");
                scalar(&mut out, 7, "dcscor:parameter", "ЦветФона");
                let value = source_style_reference_value_fragment(reference)?;
                line(&mut out, 7, &value);
                line(&mut out, 6, "</dcscor:item>");
            }
            line(&mut out, 6, "<dcscor:item>");
            scalar(&mut out, 7, "dcscor:parameter", "Расшифровка");
            line(
                &mut out,
                7,
                "<dcscor:value xsi:type=\"dcscor:Parameter\">Probe</dcscor:value>",
            );
            line(&mut out, 6, "</dcscor:item>");
            line(&mut out, 5, "</dcsat:appearance>");
        }
        line(&mut out, 4, "</dcsat:tableCell>");
        line(&mut out, 3, "</dcsat:item>");
    }
    line(&mut out, 2, "</template>");
    line(
        &mut out,
        2,
        "<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">",
    );
    scalar(&mut out, 3, "dcsat:name", area.parameter_name().as_str());
    line(
        &mut out,
        3,
        &format!(
            "<dcsat:expression>{}</dcsat:expression>",
            area.expression().as_str()
        ),
    );
    line(&mut out, 2, "</parameter>");
    line(&mut out, 1, "</template>");
    Ok(out.into_bytes())
}

/// Emits the exact platform-authenticated terminal SchemaFile for the first
/// style-free AreaTemplate coordinate.
pub fn emit_dcs_area_template_storage_document(
    area: &DcsSchemaAreaTemplate,
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    emit_dcs_area_template_storage_document_with_references(area, &BTreeMap::new())
}

/// Emits the exact platform-authenticated terminal SchemaFile, resolving a
/// custom-`StyleItem` style-color reference's semantic name back to its
/// configuration-local storage uuid via `reference_types` (the same
/// uuid-to-name map shape used by the decode-direction resolver -- this
/// direction searches it by value). Without an entry for the wanted name,
/// a `back_color_style_reference` in the evidenced `CustomStyleItem` form
/// fails closed rather than fabricating or guessing a uuid; the standard
/// `Named` form never needs a resolver and always succeeds.
pub fn emit_dcs_area_template_storage_document_with_references(
    area: &DcsSchemaAreaTemplate,
    reference_types: &BTreeMap<String, String>,
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    if area.name().as_str() != "AreaProbe"
        || area.parameter_name().as_str() != "Probe"
        || area.expression().as_str() != "\"Probe\""
    {
        return unsupported("AreaTemplate value is outside the exact coordinate");
    }
    let mut out = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
    );
    line(
        &mut out,
        1,
        "<dataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">",
    );
    line(&mut out, 2, "<template>");
    scalar(&mut out, 3, "name", area.name().as_str());
    line(
        &mut out,
        3,
        "<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">",
    );
    if area.has_shared_row_appearance() {
        line(&mut out, 4, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        for _ in 0..2 {
            line(&mut out, 5, "<dcsat:tableCell>");
            line(&mut out, 6, "<dcsat:item xsi:type=\"dcsat:Field\">");
            line(
                &mut out,
                7,
                "<dcsat:value xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
            );
            line(&mut out, 6, "</dcsat:item>");
            scalar(&mut out, 6, "dcsat:appIndex", "0");
            line(&mut out, 5, "</dcsat:tableCell>");
        }
        line(&mut out, 4, "</dcsat:item>");
        line(&mut out, 4, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        line(&mut out, 5, "<dcsat:tableCell>");
        line(&mut out, 6, "<dcsat:item xsi:type=\"dcsat:Field\">");
        line(
            &mut out,
            7,
            "<dcsat:value xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
        );
        line(&mut out, 6, "</dcsat:item>");
        line(&mut out, 5, "</dcsat:tableCell>");
        line(&mut out, 4, "</dcsat:item>");
    } else {
        line(&mut out, 4, "<dcsat:item xsi:type=\"dcsat:TableRow\">");
        line(&mut out, 5, "<dcsat:tableCell>");
        line(&mut out, 6, "<dcsat:item xsi:type=\"dcsat:Field\">");
        line(
            &mut out,
            7,
            "<dcsat:value xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Parameter\">Probe</dcsat:value>",
        );
        line(&mut out, 6, "</dcsat:item>");
        if area.has_parameter_appearance() {
            scalar(&mut out, 6, "dcsat:appIndex", "0");
        }
        line(&mut out, 5, "</dcsat:tableCell>");
        line(&mut out, 4, "</dcsat:item>");
    }
    line(&mut out, 3, "</template>");
    line(
        &mut out,
        3,
        "<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">",
    );
    scalar(&mut out, 4, "dcsat:name", area.parameter_name().as_str());
    line(
        &mut out,
        4,
        "<dcsat:expression>\"Probe\"</dcsat:expression>",
    );
    line(&mut out, 3, "</parameter>");
    line(&mut out, 2, "</template>");
    line(&mut out, 1, "</dataCompositionSchema>");
    if area.has_shared_row_appearance() {
        line(
            &mut out,
            1,
            "<appearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"TableCellAppearance\">",
        );
        line(
            &mut out,
            2,
            "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
        );
        scalar(&mut out, 3, "parameter", "Details");
        line(&mut out, 3, "<value xsi:type=\"Parameter\">Probe</value>");
        line(&mut out, 2, "</item>");
        line(&mut out, 1, "</appearance>");
    } else if area.has_parameter_appearance() {
        line(
            &mut out,
            1,
            "<appearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"TableCellAppearance\">",
        );
        let color = area.text_color_appearance();
        let style_reference = area.back_color_style_reference();
        if let Some(color) = color {
            line(
                &mut out,
                2,
                "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
            );
            scalar(&mut out, 3, "parameter", "TextColor");
            line(&mut out, 3, storage_color_value_fragment(color));
            line(&mut out, 2, "</item>");
        } else if let Some(reference) = style_reference {
            line(
                &mut out,
                2,
                "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
            );
            scalar(&mut out, 3, "parameter", "BackColor");
            let value = storage_style_reference_value_fragment(reference, reference_types)?;
            line(&mut out, 3, &value);
            line(&mut out, 2, "</item>");
        }
        line(
            &mut out,
            2,
            "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
        );
        let parameter_label = if color.is_some() || style_reference.is_some() {
            "Details"
        } else {
            "Расшифровка"
        };
        scalar(&mut out, 3, "parameter", parameter_label);
        line(&mut out, 3, "<value xsi:type=\"Parameter\">Probe</value>");
        line(&mut out, 2, "</item>");
        line(&mut out, 1, "</appearance>");
    }
    out.push_str("\r\n</SchemaFile>");
    Ok(out.into_bytes())
}

/// Exact `dcscor:value` fragment for the source-direction (embedded
/// `dcsat:appearance`) spelling of the evidenced web color. The `d8p1`
/// prefix is not semantic -- it is the platform's own auto-generated
/// spelling, reproduced verbatim to match `native-template.xml` byte for
/// byte -- but it is only ever compared by expanded QName on parse.
fn source_color_value_fragment(color: DcsAppearanceColor) -> &'static str {
    match color {
        DcsAppearanceColor::WebRed => {
            "<dcscor:value xmlns:d8p1=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xsi:type=\"v8ui:Color\">d8p1:Red</dcscor:value>"
        }
    }
}

/// Exact `value` fragment for the storage-direction (side-table) spelling
/// of the evidenced web color, matching the platform's own `d4p1`/`d4p2`
/// auto-generated prefixes byte for byte.
fn storage_color_value_fragment(color: DcsAppearanceColor) -> &'static str {
    match color {
        DcsAppearanceColor::WebRed => {
            "<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\" xmlns:d4p2=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xsi:type=\"d4p1:Color\">d4p2:Red</value>"
        }
    }
}

/// Exact `dcscor:value` fragment for the source-direction (embedded
/// `dcsat:appearance`) spelling of an evidenced style-color reference.
/// Both proven forms (standard-named and custom StyleItem) are lexically
/// identical here -- only the referenced name differs -- and neither needs
/// a resolver: the semantic name is already the source spelling. The
/// `d8p1` prefix is the platform's own auto-generated spelling, reproduced
/// verbatim to match `native-template.xml` byte for byte, but it is only
/// ever compared by expanded QName on parse.
fn source_style_reference_value_fragment(
    reference: &DcsStyleColorReference,
) -> Result<String, DcsInnerSchemaError> {
    let name = match reference {
        DcsStyleColorReference::Named(name) if name.as_str() == "NegativeTextColor" => {
            name.as_str()
        }
        DcsStyleColorReference::CustomStyleItem(name) if name.as_str() == "CorpusAccent" => {
            name.as_str()
        }
        _ => {
            return unsupported(
                "AreaTemplate style-reference value is outside the exact coordinate",
            );
        }
    };
    Ok(format!(
        "<dcscor:value xmlns:d8p1=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"v8ui:Color\">d8p1:{name}</dcscor:value>"
    ))
}

/// Exact `value` fragment for the storage-direction (side-table) spelling
/// of an evidenced style-color reference, matching the platform's own
/// prefixes byte for byte. The standard-named form retains the same named
/// lexical token (`d4p2:NegativeTextColor`) and needs no resolver; the
/// custom-StyleItem form spells a raw `0:<uuid>` reference instead, so its
/// uuid must be found in `reference_types` by reverse lookup on the
/// semantic name -- an adapter-supplied concern, never guessed here.
fn storage_style_reference_value_fragment(
    reference: &DcsStyleColorReference,
    reference_types: &BTreeMap<String, String>,
) -> Result<String, DcsInnerSchemaError> {
    match reference {
        DcsStyleColorReference::Named(name) if name.as_str() == "NegativeTextColor" => Ok(format!(
            "<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\" xmlns:d4p2=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"d4p1:Color\">d4p2:{}</value>",
            name.as_str()
        )),
        DcsStyleColorReference::CustomStyleItem(name) if name.as_str() == "CorpusAccent" => {
            let uuid = reference_types
                .iter()
                .find(|(_, resolved_name)| resolved_name.as_str() == name.as_str())
                .map(|(uuid, _)| uuid.as_str())
                .ok_or_else(|| {
                    DcsInnerSchemaError::UnsupportedSource(
                        "style-reference custom StyleItem name has no evidence-backed uuid resolution"
                            .to_string(),
                    )
                })?;
            Ok(format!(
                "<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\" xsi:type=\"d4p1:Color\">0:{uuid}</value>"
            ))
        }
        _ => unsupported("AreaTemplate style-reference value is outside the exact coordinate"),
    }
}

fn source_header(p: &DcsInnerSchemaPolicy) -> String {
    format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<DataCompositionSchema xmlns=\"{}\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"{}\" xmlns:v8=\"{}\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"{}\" xmlns:xsi=\"{}\">",
        p.schema_namespace_uri(),
        p.settings_namespace_uri(),
        p.data_core_namespace_uri(),
        p.xml_schema_namespace_uri(),
        p.xsi_namespace_uri()
    )
}

fn emit_variant(
    out: &mut String,
    variant: &DcsSchemaSettingsVariantShell,
    settings: &DcsInlineSettingsFragment,
) {
    line(out, 1, "<settingsVariant>");
    scalar(out, 2, "dcsset:name", variant.name().as_str());
    emit_local_string(out, 2, "dcsset:presentation", variant.presentation(), true);
    append_indented_fragment(out, settings.as_str(), 2);
    line(out, 1, "</settingsVariant>");
}

fn emit_query(
    out: &mut String,
    depth: usize,
    element: &str,
    query: &DcsSchemaQueryDataSet,
    policy: &DcsInnerSchemaPolicy,
) {
    line(
        out,
        depth,
        &format!("<{element} xsi:type=\"DataSetQuery\">"),
    );
    scalar(out, depth + 1, "name", query.name().as_str());
    line(out, depth + 1, "<field xsi:type=\"DataSetFieldField\">");
    scalar(
        out,
        depth + 2,
        "dataPath",
        query.field().data_path().as_str(),
    );
    scalar(out, depth + 2, "field", query.field().field().as_str());
    line(out, depth + 1, "</field>");
    if let Some(typed_field) = query.typed_field() {
        line(out, depth + 1, "<field xsi:type=\"DataSetFieldField\">");
        scalar(out, depth + 2, "dataPath", typed_field.data_path().as_str());
        scalar(out, depth + 2, "field", typed_field.field().as_str());
        emit_value_type(out, depth + 2, typed_field.value_type(), policy);
        line(out, depth + 1, "</field>");
    }
    scalar(out, depth + 1, "dataSource", query.data_source().as_str());
    line(
        out,
        depth + 1,
        &format!(
            "<query>{}</query>",
            escape_query_text(query.query().as_str())
        ),
    );
    line(out, depth, &format!("</{element}>"));
}

fn escape_query_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaDataSetObject, DcsInnerSchemaError> {
    require_schema(e, p, "dataSet")?;
    require_type(e, p, p.data_set_object_type_qname())?;
    let c = element_children(e)?;
    if !(4..=5).contains(&c.len())
        || c[0].local != "name"
        || c[1].local != "field"
        || c[c.len() - 2].local != "dataSource"
        || c[c.len() - 1].local != "objectName"
        || (c.len() == 5 && c[2].local != "field")
    {
        return unsupported("DataSetObject child order/cardinality is outside the cohort");
    }
    for child in &c {
        require_namespace(child, p.schema_namespace_uri())?;
    }
    let fields = c[1..c.len() - 2]
        .iter()
        .map(|field| parse_field(field, p, reference_types))
        .collect::<Result<Vec<_>, _>>()?;
    DcsSchemaDataSetObject::new(
        canonical(text(c[0])?)?,
        fields,
        canonical(text(c[c.len() - 2])?)?,
        canonical(text(c[c.len() - 1])?)?,
    )
    .map_err(DcsInnerSchemaError::Build)
}

fn parse_field(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaDataSetField, DcsInnerSchemaError> {
    require_type(e, p, p.data_set_field_type_qname())?;
    let c = exact_children(e, p.data_set_field_child_order(), p.schema_namespace_uri())?;
    DcsSchemaDataSetField::new(
        canonical(text(c[0])?)?,
        canonical(text(c[1])?)?,
        parse_value_type_with_references(c[2], p, reference_types)?,
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

/// Parses the three additional scalar-typed parameters (`Флаг`, `Лимит`,
/// `Период`) authenticated by the dedicated 2214 parameter-scalar-types
/// cohort. Consumes exactly three `parameter` siblings, in this exact
/// order, starting at `*cursor`.
fn parse_parameter_scalar_types(
    children: &[&ParsedElement],
    cursor: &mut usize,
    p: &DcsInnerSchemaPolicy,
) -> Result<DcsSchemaParameterScalarTypes, DcsInnerSchemaError> {
    let sp = parameter_scalar_types_policy()?;
    let flag = parse_boolean_parameter(take(children, cursor, "parameter")?, p, &sp)?;
    let limit = parse_decimal_parameter(take(children, cursor, "parameter")?, p, &sp)?;
    let period = parse_standard_period_parameter(take(children, cursor, "parameter")?, p, &sp)?;
    DcsSchemaParameterScalarTypes::new(flag, limit, period).map_err(DcsInnerSchemaError::Build)
}

fn parse_boolean_parameter(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    sp: &DcsParameterScalarTypesPolicy,
) -> Result<DcsSchemaBooleanParameter, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(e, p.parameter_child_order(), p.schema_namespace_uri())?;
    let name = canonical(text(c[0])?)?;
    if name.as_str() != sp.flag_parameter_name() {
        return unsupported("scalar-type parameter name is outside the evidenced cohort");
    }
    let title = parse_local_string(c[1], p)?;
    require_name(c[2], Some(p.schema_namespace_uri()), "valueType")?;
    require_no_attributes(c[2])?;
    let vt = element_children(c[2])?;
    if vt.len() != 1 {
        return unsupported("boolean parameter valueType must contain only Type");
    }
    require_name(vt[0], Some(p.data_core_namespace_uri()), "Type")?;
    require_no_attributes(vt[0])?;
    if resolve_qname_text(vt[0])? != sp.boolean_value_type_qname() {
        return unsupported("parameter valueType is not xs:boolean");
    }
    require_type(c[3], p, &sp.boolean_value_type_qname())?;
    let value = match text_allowing_attributes(c[3])?.as_str() {
        "true" => true,
        "false" => false,
        _ => return unsupported("boolean parameter value is not a valid xs:boolean lexical form"),
    };
    if text(c[4])? != "false" {
        return unsupported("parameter useRestriction is not false");
    }
    DcsSchemaBooleanParameter::new(name, title, value).map_err(DcsInnerSchemaError::Build)
}

fn parse_decimal_parameter(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    sp: &DcsParameterScalarTypesPolicy,
) -> Result<DcsSchemaDecimalParameter, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(e, p.parameter_child_order(), p.schema_namespace_uri())?;
    let name = canonical(text(c[0])?)?;
    if name.as_str() != sp.limit_parameter_name() {
        return unsupported("scalar-type parameter name is outside the evidenced cohort");
    }
    let title = parse_local_string(c[1], p)?;
    require_name(c[2], Some(p.schema_namespace_uri()), "valueType")?;
    require_no_attributes(c[2])?;
    let vt = element_children(c[2])?;
    if vt.len() != 2 {
        return unsupported("decimal parameter valueType must contain Type and NumberQualifiers");
    }
    require_name(vt[0], Some(p.data_core_namespace_uri()), "Type")?;
    require_no_attributes(vt[0])?;
    if resolve_qname_text(vt[0])? != p.decimal_value_type_qname() {
        return unsupported("parameter valueType is not xs:decimal");
    }
    require_name(vt[1], Some(p.data_core_namespace_uri()), "NumberQualifiers")?;
    require_no_attributes(vt[1])?;
    let q = exact_children(
        vt[1],
        p.number_qualifiers_child_order(),
        p.data_core_namespace_uri(),
    )?;
    if text(q[2])? != "Any" {
        return unsupported("AllowedSign is outside the cohort");
    }
    let digits = text(q[0])?
        .parse()
        .map_err(|_| DcsInnerSchemaError::Malformed("parameter Digits is not u32".into()))?;
    let fraction = text(q[1])?.parse().map_err(|_| {
        DcsInnerSchemaError::Malformed("parameter FractionDigits is not u32".into())
    })?;
    let value_type =
        DcsSchemaParameterDecimalType::new(digits, fraction).map_err(DcsInnerSchemaError::Build)?;
    require_type(c[3], p, p.decimal_value_type_qname())?;
    let value = canonical(text_allowing_attributes(c[3])?)?;
    if text(c[4])? != "false" {
        return unsupported("parameter useRestriction is not false");
    }
    DcsSchemaDecimalParameter::new(name, title, value_type, value)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_standard_period_parameter(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    sp: &DcsParameterScalarTypesPolicy,
) -> Result<DcsSchemaStandardPeriodParameter, DcsInnerSchemaError> {
    require_no_attributes(e)?;
    let c = exact_children(e, p.parameter_child_order(), p.schema_namespace_uri())?;
    let name = canonical(text(c[0])?)?;
    if name.as_str() != sp.period_parameter_name() {
        return unsupported("scalar-type parameter name is outside the evidenced cohort");
    }
    let title = parse_local_string(c[1], p)?;
    require_name(c[2], Some(p.schema_namespace_uri()), "valueType")?;
    require_no_attributes(c[2])?;
    let vt = element_children(c[2])?;
    if vt.len() != 1 {
        return unsupported("StandardPeriod parameter valueType must contain only Type");
    }
    require_name(vt[0], Some(p.data_core_namespace_uri()), "Type")?;
    require_no_attributes(vt[0])?;
    if resolve_qname_text(vt[0])? != sp.standard_period_value_type_qname() {
        return unsupported("parameter valueType is not v8:StandardPeriod");
    }
    require_name(c[3], Some(p.schema_namespace_uri()), "value")?;
    require_type(c[3], p, &sp.standard_period_value_type_qname())?;
    let value_children = element_children(c[3])?;
    if value_children.len() != 1 {
        return unsupported("StandardPeriod value must contain exactly one variant");
    }
    require_name(
        value_children[0],
        Some(p.data_core_namespace_uri()),
        "variant",
    )?;
    require_type(
        value_children[0],
        p,
        &sp.standard_period_variant_type_qname(),
    )?;
    let variant_token = text_allowing_attributes(value_children[0])?;
    if variant_token.trim() != sp.standard_period_variant_token() {
        return unsupported("StandardPeriodVariant is outside the evidenced cohort");
    }
    if text(c[4])? != "false" {
        return unsupported("parameter useRestriction is not false");
    }
    DcsSchemaStandardPeriodParameter::new(name, title, DcsSchemaStandardPeriodVariant::LastMonth)
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

fn parse_value_type_with_references(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaFieldType, DcsInnerSchemaError> {
    let children = element_children(e)?;
    if children.len() == 1
        && children[0].namespace.as_deref() == Some(p.data_core_namespace_uri())
        && children[0].local == "TypeId"
    {
        require_no_attributes(e)?;
        require_no_attributes(children[0])?;
        let type_id = text(children[0])?.to_ascii_lowercase();
        let qualified_name = reference_types.get(&type_id).ok_or_else(|| {
            DcsInnerSchemaError::UnsupportedSource(format!(
                "TypeId {type_id} has no evidence-backed semantic resolution"
            ))
        })?;
        if type_id != p.reference_storage_type_id()
            || qualified_name != p.reference_source_qualified_name()
        {
            return unsupported("resolved TypeId is outside the evidenced reference cohort");
        }
        return Ok(DcsSchemaFieldType::Reference(
            DcsSchemaReferenceType::new(canonical(qualified_name.clone())?)
                .map_err(DcsInnerSchemaError::Build)?,
        ));
    }
    if children.len() == 1
        && children[0].namespace.as_deref() == Some(p.data_core_namespace_uri())
        && children[0].local == "Type"
    {
        require_no_attributes(e)?;
        require_no_attributes(children[0])?;
        let qname = resolve_qname_text(children[0])?;
        let expected = format!(
            "{{{}}}{}",
            p.current_config_namespace_uri(),
            p.reference_source_qualified_name()
        );
        if qname != expected {
            return unsupported("current-config Type is outside the evidenced reference cohort");
        }
        return Ok(DcsSchemaFieldType::Reference(
            DcsSchemaReferenceType::new(canonical(p.reference_source_qualified_name().to_owned())?)
                .map_err(DcsInnerSchemaError::Build)?,
        ));
    }
    parse_value_type(e, p)
}

fn parse_document(bytes: &[u8]) -> Result<ParsedElement, DcsInnerSchemaError> {
    let mut reader = NsReader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<ParsedElement> = Vec::new();
    let mut root = None;
    let mut events = 0usize;
    let max_events = scan_bound(bytes.len());
    loop {
        events += 1;
        if events > max_events {
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
            // The five predefined XML entities (never a numeric character
            // reference, and never a DTD-defined general entity, neither of
            // which any evidenced cohort exercises) resolve to their plain
            // character and are appended as text -- e.g. `SortKey &gt; 0`
            // (the evidenced `dataSetLink` `linkConditionExpression`
            // literal) is exactly this, not a hidden/rejected construct.
            Event::GeneralRef(bytes_ref) => {
                let name = bytes_ref
                    .decode()
                    .map_err(|x| DcsInnerSchemaError::Malformed(x.to_string()))?;
                let resolved = match name.as_ref() {
                    "lt" => '<',
                    "gt" => '>',
                    "amp" => '&',
                    "apos" => '\'',
                    "quot" => '"',
                    _ => {
                        return unsupported(
                            "only the five predefined XML entity references are admitted",
                        );
                    }
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(ParsedNode::Text(resolved.to_string()));
                } else {
                    return Err(DcsInnerSchemaError::Malformed(
                        "general reference outside root".into(),
                    ));
                }
            }
            Event::Comment(_) | Event::PI(_) | Event::DocType(_) => {
                return unsupported("comments, PI and doctype are outside the cohort");
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

fn area_policy() -> Result<DcsAreaTemplatePolicy, DcsInnerSchemaError> {
    bundled_dcs_area_template_policy()
        .map_err(|error| DcsInnerSchemaError::InvalidEvidence(error.to_string()))
}
fn parameter_scalar_types_policy() -> Result<DcsParameterScalarTypesPolicy, DcsInnerSchemaError> {
    bundled_dcs_parameter_scalar_types_policy()
        .map_err(|error| DcsInnerSchemaError::InvalidEvidence(error.to_string()))
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
fn resolve_qname_text_allowing_attributes(
    e: &ParsedElement,
) -> Result<String, DcsInnerSchemaError> {
    resolve_qname(e, &text_allowing_attributes(e)?)
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
fn emit_value_type(
    out: &mut String,
    depth: usize,
    value: &DcsSchemaFieldType,
    policy: &DcsInnerSchemaPolicy,
) {
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
        DcsSchemaFieldType::Reference(v) => line(
            out,
            depth + 1,
            &format!(
                "<v8:Type xmlns:d5p1=\"{}\">d5p1:{}</v8:Type>",
                policy.current_config_namespace_uri(),
                escape(v.qualified_name().as_str())
            ),
        ),
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
// ---------------------------------------------------------------------------
// Primary `SchemaFile` -> source `DataCompositionSchema` transliteration
// ---------------------------------------------------------------------------
//
// The platform stores and exports the *same* DCS document; the two directions
// differ only in which namespace prefixes are in scope, in one level of
// indentation (storage nests the schema inside a `SchemaFile` wrapper), in
// where the `Settings` documents live, and in how a configuration-local
// `TypeId` is spelled.  Every one of those differences is a mechanical
// rewrite of platform-written bytes, so this codec transliterates them
// instead of round-tripping through a typed IR: the closed typed cohorts
// above can only describe the handful of shapes they enumerate, while real
// configurations use the full schema vocabulary.
//
// Everything this rewriter cannot account for from the document's own bytes
// fails closed: an element or attribute in a namespace the source root does
// not declare, a `TypeId` the configuration type index cannot resolve,
// comments/PIs/doctypes, or a `settingsVariant` count that disagrees with the
// envelope's `Settings` document count.

/// Source prefix for `uri` in a `DataCompositionSchema` source document, or
/// `None` when the source root declares no prefix for it.
///
/// The table is exactly the root declaration
/// [`emit_dcs_inner_schema_source_document`] writes, in the same order.
fn source_namespace_prefix(policy: &DcsInnerSchemaPolicy, uri: &str) -> Option<&'static str> {
    match uri {
        "http://v8.1c.ru/8.1/data-composition-system/common" => Some("dcscom"),
        "http://v8.1c.ru/8.1/data-composition-system/core" => Some("dcscor"),
        "http://v8.1c.ru/8.1/data/ui" => Some("v8ui"),
        _ if uri == policy.schema_namespace_uri() => Some(""),
        _ if uri == policy.settings_namespace_uri() => Some("dcsset"),
        _ if uri == policy.data_core_namespace_uri() => Some("v8"),
        _ if uri == policy.xml_schema_namespace_uri() => Some("xs"),
        _ if uri == policy.xsi_namespace_uri() => Some("xsi"),
        _ => None,
    }
}

/// A raw lexical token: either character data or one complete tag.
///
/// The document is never unescaped, so every entity reference, attribute
/// quoting style and whitespace run reaches the output exactly as the
/// platform wrote it.
enum RawToken<'a> {
    Text(&'a str),
    Tag(&'a str),
}

/// Byte offset of the `>` closing the tag that starts at `start`, ignoring
/// `>` inside single- or double-quoted attribute values.
fn raw_tag_end(body: &str, start: usize) -> Result<usize, DcsInnerSchemaError> {
    let bytes = body.as_bytes();
    let mut quote: Option<u8> = None;
    for (offset, byte) in bytes.iter().enumerate().skip(start) {
        match (quote, *byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(*byte),
            (None, b'>') => return Ok(offset),
            (None, _) => {}
        }
    }
    Err(DcsInnerSchemaError::Malformed(
        "unterminated XML tag".into(),
    ))
}

fn scan_raw_tokens(body: &str) -> Result<Vec<RawToken<'_>>, DcsInnerSchemaError> {
    let mut tokens = Vec::new();
    let mut position = 0usize;
    while position < body.len() {
        match body[position..].find('<') {
            None => {
                tokens.push(RawToken::Text(&body[position..]));
                break;
            }
            Some(relative) => {
                if relative > 0 {
                    tokens.push(RawToken::Text(&body[position..position + relative]));
                }
                let start = position + relative;
                let end = raw_tag_end(body, start)?;
                tokens.push(RawToken::Tag(&body[start..=end]));
                position = end + 1;
            }
        }
    }
    if tokens.len() > scan_bound(body.len()) {
        return Err(DcsInnerSchemaError::Malformed(
            "XML event limit exceeded".into(),
        ));
    }
    Ok(tokens)
}

/// One start (or self-closing) tag split into its name and its attributes,
/// with every value still in its original escaped spelling.
struct RawStartTag<'a> {
    name: &'a str,
    attributes: Vec<(&'a str, &'a str)>,
    self_closing: bool,
}

fn scan_start_tag(tag: &str) -> Result<RawStartTag<'_>, DcsInnerSchemaError> {
    let inner = tag
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .ok_or_else(|| DcsInnerSchemaError::Malformed("malformed XML tag".into()))?;
    let (inner, self_closing) = match inner.strip_suffix('/') {
        Some(value) => (value, true),
        None => (inner, false),
    };
    let name_end = inner
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(inner.len());
    let name = &inner[..name_end];
    if name.is_empty() {
        return Err(DcsInnerSchemaError::Malformed("XML tag has no name".into()));
    }
    let mut attributes = Vec::new();
    let mut rest = &inner[name_end..];
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if rest.is_empty() {
            break;
        }
        let equals = rest
            .find('=')
            .ok_or_else(|| DcsInnerSchemaError::Malformed("attribute has no value".into()))?;
        let key = rest[..equals].trim_end();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(DcsInnerSchemaError::Malformed(
                "malformed attribute name".into(),
            ));
        }
        let after = rest[equals + 1..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        let quote = after
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
            .ok_or_else(|| {
                DcsInnerSchemaError::Malformed("attribute value is not quoted".into())
            })?;
        let value_end = after[1..].find(quote).ok_or_else(|| {
            DcsInnerSchemaError::Malformed("attribute value is not terminated".into())
        })?;
        attributes.push((key, &after[1..1 + value_end]));
        rest = &after[1 + value_end + 1..];
    }
    Ok(RawStartTag {
        name,
        attributes,
        self_closing,
    })
}

/// Whether a namespace prefix is one the platform generated from an element's
/// depth (`d5p1`, `d12p3`, ...) rather than one it spelled itself.
///
/// A generated prefix names the *storage* document's depth, which is one
/// level deeper than the source document's, so it must be reminted against
/// the target depth. Every other prefix carries no depth and is copied.
fn is_generated_depth_prefix(prefix: &str) -> bool {
    let Some(rest) = prefix.strip_prefix('d') else {
        return false;
    };
    let Some((depth, point)) = rest.split_once('p') else {
        return false;
    };
    !depth.is_empty()
        && !point.is_empty()
        && depth.bytes().all(|byte| byte.is_ascii_digit())
        && point.bytes().all(|byte| byte.is_ascii_digit())
}

fn split_prefix(name: &str) -> (&str, &str) {
    match name.split_once(':') {
        Some((prefix, local)) => (prefix, local),
        None => ("", name),
    }
}

/// What the rewriter is currently inside, so a close tag knows what to write.
enum RewriteFrame {
    /// An element the rewriter did not open itself: a placeholder standing in
    /// for the ancestors a seeded sub-rewrite starts underneath. Closing one
    /// means the token range was not balanced.
    Outer,
    /// The dropped `SchemaFile` wrapper.
    Wrapper,
    /// The schema root, re-spelled as `DataCompositionSchema` when a whole
    /// document is being written and dropped when only its children are.
    Root,
    /// A `v8:TypeId` element whose content is replaced wholesale.
    TypeId,
    /// A `dcsat:appIndex` element replaced wholesale by the side-table
    /// `dcsat:appearance` it selects.
    AppIndex,
    /// Any other element, already re-prefixed for the source direction.
    Element(String),
}

/// Which of the two storage documents -- and which part of it -- the token
/// loop is spelling in the source direction.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RewriteMode {
    /// The whole primary `SchemaFile`, becoming the whole source
    /// `DataCompositionSchema` document.
    PrimaryDocument,
    /// The terminal `SchemaFile`'s `dataCompositionSchema`, becoming just the
    /// run of root-level `template`/`fieldTemplate`/`groupTemplate`/
    /// `groupHeaderTemplate` children the source document carries inline.
    TerminalFragment,
    /// The children of one side-table `<appearance>` element, being inlined
    /// at the `dcsat:appIndex` that selects it.
    InlineAppearance,
}

/// The maps every rewrite direction resolves storage identifiers through,
/// plus the terminal envelope's side-table `<appearance>` elements.
struct RewriteContext<'a> {
    policy: &'a DcsInnerSchemaPolicy,
    reference_types: &'a BTreeMap<String, String>,
    type_set_types: &'a BTreeMap<String, String>,
    opaque_type_ids: &'a BTreeSet<String>,
    style_item_names: &'a BTreeMap<String, String>,
    settings_blocks: &'a [DcsInlineSettingsFragment],
    /// Token index ranges (start tag, end tag) of the terminal `SchemaFile`'s
    /// direct `<appearance>` children, in document order -- exactly what a
    /// `dcsat:appIndex` numbers. Empty for the primary document.
    appearances: &'a [(usize, usize)],
    /// Which of them some `appIndex` actually selected. The source document
    /// has no side table, so an appearance nothing selects would simply be
    /// dropped; every one of the 486 in the reference configuration is
    /// selected, so an unselected one is an unevidenced shape and is refused
    /// rather than silently discarded.
    selected: &'a RefCell<BTreeSet<usize>>,
}

/// Everything a rewrite of one token range starts out holding, so a nested
/// range can be spelled as if it stood where it is being inlined.
struct RewriteSeed {
    mode: RewriteMode,
    /// How many elements are already open above the range, which is both the
    /// depth its first element sits at and the `dNpM` number a declaration
    /// minted there takes.
    depth: usize,
    /// Tabs to add to (or, negative, remove from) every layout whitespace run.
    indent_delta: isize,
    /// The storage document's namespace declarations in effect above the
    /// range -- taken from where the range physically sits.
    scopes: NamespaceScopes,
    /// The target document's prefixes in effect above the range -- taken from
    /// where the range is being written, which for an inlined `<appearance>`
    /// is not where it physically sits.
    source_scopes: SourcePrefixScopes,
}

/// How an element's character data must be re-spelled for the source
/// direction.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RewriteTextKind {
    /// Copied through, only re-indented when it is pretty-printing
    /// whitespace.
    Literal,
    /// A `v8:Type`/`v8:TypeSet` QName. An unprefixed value resolves through
    /// the default namespace in scope, exactly as an `xsi:type` attribute
    /// does.
    TypeQName,
    /// A `dcscor:value` whose `xsi:type` is `{data/ui}Color`. Its lexical
    /// space carries two evidenced forms: a QName naming a style colour, and
    /// a `0:<uuid>` reference to a configuration `StyleItem`. Anything else
    /// (a literal colour) is copied through untouched, and an unprefixed
    /// value is *not* resolved through the default namespace -- the default
    /// there is the DCS core namespace, which would spell a colour
    /// `dcscor:...`.
    ColorValue,
}

/// One open element, with everything a later token needs to decide how to
/// write itself.
struct RewriteState {
    frame: RewriteFrame,
    /// Whether an element child has already been written inside this element,
    /// which is what distinguishes pretty-printing whitespace from character
    /// data in a leaf.
    saw_child: bool,
    /// How this element's character data must be spelled.
    text: RewriteTextKind,
    /// Storage prefix -> source prefix for the namespaces this element
    /// declared, so its own QName content resolves the same way its
    /// attributes do.
    renamed: Vec<(String, String)>,
    /// Byte offset in the output at which this element's namespace
    /// declarations were written, so a declaration only its character data
    /// turns out to need can still be spliced into its opening tag.
    declaration_offset: usize,
    /// The `dNpM` point number the next minted declaration on this element
    /// takes.
    next_declaration: usize,
    /// This element's 1-based depth in the target document.
    depth: usize,
    /// The run of direct data-core `Type`/`TypeId` children written so far,
    /// held until the run ends so it can be put into the source document's
    /// order, which storage's schema cannot express.
    type_run: Vec<TypeRunEntry>,
    /// For a data-core `Type` element, where its own output began, so its
    /// closing tag can hand the finished element to the parent's run.
    literal_type_start: Option<usize>,
    /// Output offset right before this element's opening `<` was written.
    start_tag_begin_offset: usize,
    /// Output offset right after this element's opening tag's own closing
    /// `>` was written (i.e., before any content).
    start_tag_end_offset: usize,
    /// Whether a childless, attribute-free instance of this element should
    /// be omitted entirely (both tags, and any whitespace-only interior)
    /// rather than left as an empty open/close pair.
    ///
    /// Evidenced on real ERP УХ 3.2.12.6 bytes: a `DataSetFieldField`'s
    /// `appearance` and a `parameter`'s `inputParameters` are present in
    /// *storage* as empty placeholders even when unset, but the platform's
    /// own decompiled source XML never carries an empty `<appearance/>` or
    /// `<inputParameters/>` -- the element is missing altogether, matching
    /// the same pattern `dcsset:outputParameters` has in the settings
    /// document (see `DcsEmptyElementAction` in `src/mssql_dump/dcs.rs`).
    omit_if_empty: bool,
}

impl RewriteState {
    const fn new(frame: RewriteFrame) -> Self {
        Self {
            frame,
            saw_child: false,
            text: RewriteTextKind::Literal,
            renamed: Vec::new(),
            declaration_offset: 0,
            next_declaration: 1,
            depth: 0,
            type_run: Vec::new(),
            literal_type_start: None,
            start_tag_begin_offset: 0,
            start_tag_end_offset: 0,
            omit_if_empty: false,
        }
    }

    /// End the run of type siblings, writing it in the source document's order.
    fn flush_type_run(&mut self, out: &mut String) -> Result<(), DcsInnerSchemaError> {
        if self.type_run.is_empty() {
            return Ok(());
        }
        let run = std::mem::take(&mut self.type_run);
        let order = evidenced_type_run_order(&run)?;
        reorder_type_run(out, &run, &order);
        Ok(())
    }
}

/// Where a platform builtin type sorts among configuration reference types.
///
/// The platform writes the types of one type list in a single global order,
/// and a configuration reference's place in it is its own
/// `GeneratedType/TypeId` uuid: across the reference configuration's 2 691
/// multi-type lists -- 4 907 metadata objects, 5 201 managed forms and 691
/// data-composition templates -- every list of two or more reference types
/// ascends by that uuid, with no exception. A builtin's key is a uuid the
/// configuration itself never spells, so the evidence pins it only to the open
/// interval between the nearest reference ever observed before it and the
/// nearest ever observed after it. Two independent checks say the key really
/// is a uuid in that same space rather than a rank: for the three builtins
/// whose platform uuid the storage side already carries, that value falls
/// inside the interval measured here -- `v8:ValueListType`
/// 4772b3b4-f4a3-49c0-a1a5-8cb5961511a3 between 2e86fe94 and 55adb97e,
/// `v8:UUID` fc01b5df above b687901c, `v8:StandardPeriod` 2fdc88ec below
/// e63fc7d1.
///
/// `None` on a side means no reference was ever observed there. A comparison
/// landing strictly inside the interval is not evidenced and is refused rather
/// than guessed, so a configuration carrying a type uuid in that gap fails
/// closed instead of being written in a made-up order.
const BUILTIN_TYPE_SORT_BOUNDS: &[(&str, Option<&str>, Option<&str>)] = &[
    (
        "v8:Null",
        Some("1eb045d5-0080-4aae-8c01-7562e94c399a"),
        None,
    ),
    (
        "v8:StandardPeriod",
        None,
        Some("e63fc7d1-3d01-4fbb-8cbe-9d4bf8fe2126"),
    ),
    (
        "v8:TypeDescription",
        Some("5507e9a1-c199-40e0-a820-7436d2faac4b"),
        None,
    ),
    (
        "v8:UUID",
        Some("b687901c-87e7-4f68-b440-5cda82ad3676"),
        None,
    ),
    (
        "v8:ValueListType",
        Some("2e86fe94-4898-4ea5-988a-42122b917bee"),
        Some("55adb97e-a84e-453e-8020-7665bb2abdef"),
    ),
    (
        "xs:boolean",
        Some("56c86461-d1a1-4757-ac34-d36ef2ecf333"),
        Some("604931cd-d4a8-48d0-bc59-2ac90e044abb"),
    ),
    (
        "xs:dateTime",
        Some("a8102d85-e4f4-485c-ba59-8068bb30e9ce"),
        Some("abb75494-7154-4303-b8c6-5840c31ac3ec"),
    ),
    (
        "xs:decimal",
        Some("abbce35f-664f-492a-b8df-0bbf57fcb9bb"),
        Some("b165a6f6-2ba4-4b52-96fc-c0a51f367b7f"),
    ),
    (
        "xs:string",
        Some("9b601b75-6eee-4ccf-9287-877bcbc1645f"),
        Some("9c5f90e5-c89b-418f-8423-7135b80db1aa"),
    ),
];

fn builtin_type_sort_bounds(qname: &str) -> Option<(Option<&'static str>, Option<&'static str>)> {
    BUILTIN_TYPE_SORT_BOUNDS
        .iter()
        .find(|(name, _lower, _upper)| *name == qname)
        .map(|(_name, lower, upper)| (*lower, *upper))
}

/// What orders one member of a type list against its siblings.
#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeSortKey {
    /// A configuration type: its own storage uuid is the key.
    Reference(String),
    /// A platform builtin, whose key is known only within an open interval.
    Builtin {
        lower: Option<&'static str>,
        upper: Option<&'static str>,
    },
    /// A reference family. All 326 observed family-beside-ordered-type pairs
    /// -- 278 against a builtin, 48 against a concrete reference -- write the
    /// family second, and none writes one first, so families sort as a block
    /// behind the ordered members. Their own order is storage's, which is
    /// ascending by the family's protocol uuid in all 35 pairs observed.
    Family,
    /// A member storage wrote as a literal `Type` whose QName the bounds table
    /// does not carry: a configuration type storage spelled by QName rather
    /// than by uuid, an empty `Type`, or a builtin no observed list places.
    /// It has no key of its own, but it does have a group: storage's own
    /// `Type` group, which it shares with every builtin.
    StoredLiteral,
    /// Nothing in the corpus places this member relative to any other: a type
    /// id the configuration resolves to no name at all and the platform echoes
    /// verbatim.
    Unevidenced,
}

/// One direct `Type`/`TypeId` child of an element: the output range it was
/// written into, and the key that orders it.
#[derive(Clone, Debug)]
struct TypeRunEntry {
    start: usize,
    end: usize,
    key: TypeSortKey,
}

/// Whether the builtin is written before the configuration type, or a typed
/// refusal when the type's uuid falls inside the builtin's evidenced interval
/// and so decides nothing.
///
/// This is the only comparison storage cannot answer for itself, so it is the
/// only one asked.
fn type_sorts_before(left: &TypeSortKey, right: &TypeSortKey) -> Result<bool, DcsInnerSchemaError> {
    let (TypeSortKey::Builtin { lower, upper }, TypeSortKey::Reference(uuid)) = (left, right)
    else {
        return unsupported("a valueType member has no evidenced position beside its siblings");
    };
    if upper.is_some_and(|upper| uuid.as_str() >= upper) {
        return Ok(true);
    }
    if lower.is_some_and(|lower| uuid.as_str() <= lower) {
        return Ok(false);
    }
    unsupported(format!(
        "a valueType puts a platform builtin beside configuration type {uuid}, whose uuid falls \
         inside the builtin's evidenced sort interval, so the source order is not derivable"
    ))
}

/// The permutation that writes an already-rendered run of `Type`/`TypeId`
/// siblings in the source document's order.
///
/// Storage loses exactly one thing about that order and nothing else: its
/// schema puts every literal `Type` before every `TypeId`, so a list mixing
/// the two arrives grouped while the platform writes it interleaved. Both
/// groups arrive internally in the platform's own order -- the references
/// ascending by type uuid, the builtins in the order the platform lists them,
/// which is why a list of builtins alone already comes out byte-exact. So the
/// permutation is a stable merge of two ordered runs, and the comparator is
/// only ever asked the one question storage cannot answer: where one builtin
/// sits relative to one reference.
///
/// A list that mixes no builtin with a `TypeId` loses nothing and is left
/// exactly as storage had it, which is also what makes this incapable of
/// changing any list the writer already spelled correctly.
fn evidenced_type_run_order(run: &[TypeRunEntry]) -> Result<Vec<usize>, DcsInnerSchemaError> {
    let keys: Vec<TypeSortKey> = run.iter().map(|entry| entry.key.clone()).collect();
    evidenced_type_sort_order(&keys)
}

/// One member of a run of `Type`/`TypeId` siblings, as a writer that renders
/// such a run outside the storage-document rewrite sees it.
///
/// The same loss the rewrite repairs happens wherever storage hands a type
/// list over: its schema puts every literal `Type` before every `TypeId`, so
/// the run arrives grouped while the platform writes it interleaved. A caller
/// holding its own rendering of the run states each member's kind here and
/// applies the permutation to its own output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRunMember<'a> {
    /// A platform builtin, named by the QName the document writes for it.
    Builtin(&'a str),
    /// A configuration type, named by the storage uuid that orders it.
    Reference(&'a str),
    /// A reference family -- what storage spells as a `TypeSet`.
    Family,
    /// A member nothing observed places: a type id the configuration resolves
    /// to no name and the platform echoes verbatim.
    Unevidenced,
}

/// The permutation that writes `members` in the platform's order, or `None`
/// when nothing observed decides it.
///
/// `None` is a refusal, not an ordering: a caller that gets it must leave its
/// run exactly as it stands rather than pick an order the corpus never shows.
/// This is the same rule [`evidenced_type_run_order`] applies inside the
/// storage-document rewrite, asked without that rewrite's byte ranges.
pub fn evidenced_type_run_permutation(members: &[TypeRunMember<'_>]) -> Option<Vec<usize>> {
    let keys: Vec<TypeSortKey> = members
        .iter()
        .map(|member| match member {
            TypeRunMember::Builtin(qname) => builtin_type_sort_bounds(qname)
                .map_or(TypeSortKey::StoredLiteral, |(lower, upper)| {
                    TypeSortKey::Builtin { lower, upper }
                }),
            TypeRunMember::Reference(uuid) => TypeSortKey::Reference((*uuid).to_ascii_lowercase()),
            TypeRunMember::Family => TypeSortKey::Family,
            TypeRunMember::Unevidenced => TypeSortKey::Unevidenced,
        })
        .collect();
    evidenced_type_sort_order(&keys).ok()
}

/// The permutation for a run, with the members the source spells `TypeId`
/// lifted out of the ordering question and put behind the rest.
///
/// A type id the configuration resolves to no name is spelled `<v8:TypeId>`
/// in the source exactly as in storage, and both put every `TypeId` behind
/// every `Type`. That is what storage's own grouping is, and the platform's
/// source keeps it: across the five stand configurations every one of the
/// three runs that mixes the two spellings --
/// `Reports/КонтрольИсполненияОбязательствСПоставщиком/Templates/ОсновнаяСхемаКомпоновкиДанных`
/// twice and `DataProcessors/СопоставлениеПланФактОперацийМСФО/Forms/Форма`
/// once, both UH 3.2.12.6 -- writes every `v8:Type` before every
/// `v8:TypeId`, and no run anywhere writes one the other way. So such a
/// member needs no key of its own: it stands behind everything the source
/// spells `Type`, in storage's own order, and the remaining members are
/// ordered without it.
///
/// Where it stands relative to a `TypeSet` nothing observed says. That pair
/// is refused -- but only when storage actually puts the family behind it,
/// since otherwise both candidate rules agree and nothing has to be decided.
fn evidenced_type_sort_order(run: &[TypeSortKey]) -> Result<Vec<usize>, DcsInnerSchemaError> {
    let unevidenced: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::Unevidenced));
    if unevidenced.is_empty() {
        return evidenced_named_type_sort_order(run);
    }
    let families: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::Family));
    if let (Some(first), Some(last)) = (unevidenced.first(), families.last())
        && first < last
    {
        return unsupported(
            "a valueType puts a reference family behind a type id the configuration resolves \
             to no name, and nothing observed places either before the other",
        );
    }
    let named: Vec<usize> = (0..run.len())
        .filter(|index| !matches!(run[*index], TypeSortKey::Unevidenced))
        .collect();
    let keys: Vec<TypeSortKey> = named.iter().map(|index| run[*index].clone()).collect();
    let mut ordered: Vec<usize> = evidenced_named_type_sort_order(&keys)?
        .into_iter()
        .map(|slot| named[slot])
        .collect();
    ordered.extend(unevidenced);
    Ok(ordered)
}

/// The permutation for a run none of whose members the source spells
/// `TypeId`.
fn evidenced_named_type_sort_order(run: &[TypeSortKey]) -> Result<Vec<usize>, DcsInnerSchemaError> {
    let builtins: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::Builtin { .. }));
    let literals: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::StoredLiteral));
    let references: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::Reference(_)));
    let families: Vec<usize> = indices_of(run, |key| matches!(key, TypeSortKey::Family));
    let identity = || Ok((0..run.len()).collect());
    // Storage writes a type list in two groups: everything it spells as a
    // literal `Type` first, everything it spells as a `TypeId` after. Only a
    // run holding both groups lost anything -- inside one group storage's own
    // order is already the platform's, which is why a run of builtins alone
    // and a run of references alone both come out byte-exact untouched. A run
    // that mixes a builtin with a configuration type storage itself spelled by
    // QName is still one group, so it is left exactly as storage had it: DO
    // 3.0.21.3 `Reports/ИзменениеУчетныхЗаписей/Templates/Макет` writes
    // `CatalogRef.ВнешниеПользователи`, `xs:string`,
    // `CatalogRef.Пользователи` in that order in storage and in that same
    // order in the platform's own source, and its two other mixed literal runs
    // agree.
    if builtins.is_empty() && literals.is_empty() {
        return identity();
    }
    if references.is_empty() && families.is_empty() {
        return identity();
    }
    if !literals.is_empty() {
        return unsupported(
            "a valueType mixes a type id with a literal type the bounds table does not carry, \
             and nothing observed places either before the other",
        );
    }
    // Stable merge: a builtin goes ahead of the first reference it is
    // evidenced to precede, and the relative order inside each group is the
    // one storage already carries.
    let mut ordered: Vec<usize> = Vec::with_capacity(run.len());
    let mut pending = builtins.as_slice();
    for reference in references {
        while let Some((builtin, rest)) = pending.split_first() {
            if !type_sorts_before(&run[*builtin], &run[reference])? {
                break;
            }
            ordered.push(*builtin);
            pending = rest;
        }
        ordered.push(reference);
    }
    ordered.extend_from_slice(pending);
    ordered.extend(families);
    Ok(ordered)
}

fn indices_of(run: &[TypeSortKey], want: impl Fn(&TypeSortKey) -> bool) -> Vec<usize> {
    run.iter()
        .enumerate()
        .filter(|(_index, key)| want(key))
        .map(|(index, _key)| index)
        .collect()
}

/// Rewrite an already-written run of type siblings into `order`.
///
/// Every sibling in the run sits at one depth, so the layout whitespace
/// between them is interchangeable and stays where it stands while only the
/// elements move. The rewritten region is therefore exactly as long as the
/// original, which is what lets offsets recorded elsewhere in the output
/// survive.
fn reorder_type_run(out: &mut String, run: &[TypeRunEntry], order: &[usize]) {
    if order.iter().copied().eq(0..run.len()) {
        return;
    }
    let elements: Vec<&str> = run
        .iter()
        .map(|entry| &out[entry.start..entry.end])
        .collect();
    let separators: Vec<&str> = run
        .windows(2)
        .map(|pair| &out[pair[0].end..pair[1].start])
        .collect();
    let mut rewritten = String::new();
    for (slot, index) in order.iter().enumerate() {
        rewritten.push_str(elements[*index]);
        if let Some(separator) = separators.get(slot) {
            rewritten.push_str(separator);
        }
    }
    let region = run[0].start..run[run.len() - 1].end;
    debug_assert_eq!(region.len(), rewritten.len());
    out.replace_range(region, &rewritten);
}

/// Namespace declarations in scope, innermost last.
type NamespaceScopes = Vec<Vec<(String, String)>>;

/// The source-direction prefix chosen for every namespace declared in scope,
/// innermost last -- the mirror of [`NamespaceScopes`], which holds the
/// storage document's own spelling.
type SourcePrefixScopes = Vec<Vec<(String, String)>>;

fn resolve_source_prefix(scopes: &SourcePrefixScopes, uri: &str) -> Option<String> {
    scopes.iter().rev().find_map(|scope| {
        scope
            .iter()
            .rev()
            .find(|(declared, _)| declared == uri)
            .map(|(_, prefix)| prefix.clone())
    })
}

const DCS_CORE_NAMESPACE_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
const DCS_AREA_TEMPLATE_NAMESPACE_URI: &str =
    "http://v8.1c.ru/8.1/data-composition-system/area-template";
const TABLE_CELL_APPEARANCE_EXPANDED_NAME: &str =
    "{http://v8.1c.ru/8.1/data-composition-system/area-template}TableCellAppearance";
const DATA_UI_STYLE_NAMESPACE_URI: &str = "http://v8.1c.ru/8.1/data/ui/style";
const XSI_TYPE_EXPANDED_NAME: &str = "{http://www.w3.org/2001/XMLSchema-instance}type";
const DATA_UI_COLOR_EXPANDED_NAME: &str = "{http://v8.1c.ru/8.1/data/ui}Color";
const DATA_CORE_TYPE_EXPANDED_NAME: &str = "{http://v8.1c.ru/8.1/data/core}Type";

/// Expands a prefixed attribute name against the scopes in effect. An
/// unprefixed attribute is in no namespace, which is never what a caller
/// comparing against an expanded name is looking for.
fn expanded_attribute_name(scopes: &NamespaceScopes, key: &str) -> Option<String> {
    let (prefix, local) = split_prefix(key);
    if prefix.is_empty() {
        return None;
    }
    let uri = resolve_prefix(scopes, prefix)?;
    Some(format!("{{{uri}}}{local}"))
}

/// Expands a QName-valued string against the scopes in effect, resolving an
/// unprefixed value through the default namespace exactly as XML does.
fn expanded_qname(scopes: &NamespaceScopes, value: &str) -> Option<String> {
    let (prefix, local) = split_prefix(value);
    let uri = resolve_prefix(scopes, prefix)?;
    Some(format!("{{{uri}}}{local}"))
}

/// A rewritten value plus the namespace declaration its opening tag must gain
/// for the value to be spellable.
struct RewrittenColorValue {
    value: String,
    declaration: Option<String>,
}

/// The uuid a `0:<uuid>` storage colour reference names, canonicalized the way
/// the object-reference index is keyed.
fn style_item_reference_uuid(text: &str) -> Option<&str> {
    let uuid = text.trim().strip_prefix("0:")?;
    let hex = uuid.as_bytes();
    if hex.len() != 36 {
        return None;
    }
    hex.iter()
        .enumerate()
        .all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        .then_some(uuid)
}

/// Re-spells a `0:<uuid>` configuration `StyleItem` reference as the QName the
/// source direction writes: the style item's own name under whatever prefix
/// the style namespace has where the value lands.
///
/// The platform stores the reference by uuid in two places that reach here --
/// the character data of a `{data/ui}Color` value and the `ref` attribute of a
/// `{data/ui}Font` value -- and writes the same QName for both. `declared` is
/// the current element's own namespace declarations, which are in scope for
/// its attributes but not yet pushed onto `source_scopes`; a namespace named
/// by neither gets a declaration minted at `depth`/`point`.
///
/// A uuid the object-reference index cannot resolve fails closed: the platform
/// writes a name there, and we have no name to write.
fn rewrite_style_item_reference(
    declared: &[(String, String)],
    source_scopes: &SourcePrefixScopes,
    depth: usize,
    point: usize,
    style_item_names: &BTreeMap<String, String>,
    value: &str,
) -> Result<RewrittenColorValue, DcsInnerSchemaError> {
    let Some(uuid) = style_item_reference_uuid(value) else {
        return Err(DcsInnerSchemaError::UnsupportedSource(
            "style item reference is not a storage uuid".into(),
        ));
    };
    let lowercase = uuid.to_ascii_lowercase();
    let Some(name) = style_item_names
        .get(&lowercase)
        .or_else(|| style_item_names.get(uuid))
    else {
        return Err(DcsInnerSchemaError::UnsupportedSource(format!(
            "style reference {uuid} has no configuration StyleItem resolution"
        )));
    };
    // The style namespace may already be spelled in scope -- an inline
    // settings element declares it for its whole subtree, and the platform
    // pre-declares it on the very element carrying the reference. Only mint a
    // declaration when nothing names it.
    let in_scope = declared
        .iter()
        .find(|(uri, _)| uri == DATA_UI_STYLE_NAMESPACE_URI)
        .map(|(_, prefix)| prefix.clone())
        .or_else(|| resolve_source_prefix(source_scopes, DATA_UI_STYLE_NAMESPACE_URI));
    if let Some(prefix) = in_scope.filter(|prefix| !prefix.is_empty()) {
        return Ok(RewrittenColorValue {
            value: format!("{prefix}:{}", escape(name)),
            declaration: None,
        });
    }
    let prefix = format!("d{depth}p{point}");
    Ok(RewrittenColorValue {
        value: format!("{prefix}:{}", escape(name)),
        declaration: Some(format!(" xmlns:{prefix}=\"{DATA_UI_STYLE_NAMESPACE_URI}\"")),
    })
}

/// Re-spells the character data of a `dcscor:value` typed `{data/ui}Color`.
///
/// Two evidenced storage forms carry information the source direction spells
/// differently. A `0:<uuid>` names a configuration `StyleItem`, which the
/// source document spells as a QName in the style namespace -- so the style
/// namespace has to be declared at the point of use, exactly like any other
/// namespace the source root does not carry. A value that is already a QName
/// moves onto whichever prefix the source document uses for its namespace.
/// A colour literal is neither and is copied through byte for byte.
///
/// A `0:<uuid>` the object-reference index cannot resolve fails closed: the
/// platform writes a name there, and we have no name to write.
fn rewrite_color_value(
    policy: &DcsInnerSchemaPolicy,
    scopes: &NamespaceScopes,
    source_scopes: &SourcePrefixScopes,
    state: &RewriteState,
    style_item_names: &BTreeMap<String, String>,
    value: &str,
) -> Result<RewrittenColorValue, DcsInnerSchemaError> {
    if style_item_reference_uuid(value).is_some() {
        return rewrite_style_item_reference(
            &[],
            source_scopes,
            state.depth,
            state.next_declaration,
            style_item_names,
            value,
        );
    }
    let (prefix, _) = split_prefix(value);
    if prefix.is_empty() {
        return Ok(RewrittenColorValue {
            value: value.to_owned(),
            declaration: None,
        });
    }
    Ok(RewrittenColorValue {
        // `key` deliberately is not `xsi:type`: an unprefixed colour must not
        // pick up the default namespace, which here is the DCS core one.
        value: rewrite_qname_value(policy, scopes, &state.renamed, "", value)?,
        declaration: None,
    })
}

fn resolve_prefix<'a>(scopes: &'a NamespaceScopes, prefix: &str) -> Option<&'a str> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| {
            scope
                .iter()
                .rev()
                .find(|(declared, _)| declared == prefix)
                .map(|(_, uri)| uri.as_str())
        })
        .or(if prefix.is_empty() { Some("") } else { None })
}

/// Shifts every indentation run after a line break by `delta` tabs.
///
/// Storage nests the schema one level deeper than the source document, so the
/// whole-document directions shift by `-1`. An `<appearance>` the envelope
/// lifted out of a table cell into its side table is shifted the other way,
/// back down to the depth the cell it is inlined at sits at. Only whitespace
/// runs that separate sibling elements are shifted; character data (a query
/// text, an expression) never reaches this function.
fn shift_indent(whitespace: &str, delta: isize) -> String {
    if delta == 0 {
        return whitespace.to_owned();
    }
    let mut out = String::with_capacity(whitespace.len());
    let mut rest = whitespace;
    while let Some(index) = rest.find('\n') {
        out.push_str(&rest[..=index]);
        rest = &rest[index + 1..];
        if delta < 0 {
            for _ in 0..-delta {
                rest = rest.strip_prefix('\t').unwrap_or(rest);
            }
        } else {
            for _ in 0..delta {
                out.push('\t');
            }
        }
    }
    out.push_str(rest);
    out
}

/// Rewrites the platform's primary `SchemaFile` storage document into the
/// source `DataCompositionSchema` document, inlining `settings_blocks` into
/// the `settingsVariant` elements in document order.
///
/// `reference_types` maps a lowercased storage `TypeId` uuid to its semantic
/// current-configuration qualified name (`DocumentRef.X`), exactly as
/// [`parse_dcs_inner_schema_storage_document_with_references`] uses it.
/// `type_set_types` is its `<v8:TypeSet>` twin, holding the uuids that denote
/// a whole reference family (`DocumentRef`, `CatalogRef`, `AnyIBRef`) rather
/// than one configuration type; the type index knows which of the two a uuid
/// is, so the element name is looked up rather than inferred.
/// `opaque_type_ids` holds the uuids the configuration type index resolves to
/// no semantic name at all; the platform writes those back verbatim as
/// `<v8:TypeId>`. A uuid in none of the three fails closed rather than being
/// guessed in any direction.
///
/// `style_item_names` maps a configuration object uuid to the `StyleItem`
/// name it denotes, which is what a `0:<uuid>` colour reference is spelled as
/// in the source direction.
pub fn rewrite_dcs_primary_schema_storage_document(
    bytes: &[u8],
    reference_types: &BTreeMap<String, String>,
    type_set_types: &BTreeMap<String, String>,
    opaque_type_ids: &BTreeSet<String>,
    style_item_names: &BTreeMap<String, String>,
    settings_blocks: &[DcsInlineSettingsFragment],
) -> Result<Vec<u8>, DcsInnerSchemaError> {
    let policy = policy()?;
    let body = storage_document_body(bytes, "primary schema")?;
    let tokens = scan_raw_tokens(body)?;
    let context = RewriteContext {
        policy: &policy,
        reference_types,
        type_set_types,
        opaque_type_ids,
        style_item_names,
        settings_blocks,
        appearances: &[],
        selected: &RefCell::new(BTreeSet::new()),
    };
    let mut out = String::with_capacity(bytes.len() + 1024);
    out.push('\u{feff}');
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str(&rewrite_tokens(
        &context,
        &tokens,
        0..tokens.len(),
        RewriteSeed {
            mode: RewriteMode::PrimaryDocument,
            depth: 0,
            indent_delta: -1,
            scopes: Vec::new(),
            source_scopes: Vec::new(),
        },
    )?);
    Ok(out.into_bytes())
}

/// Rewrites the platform's terminal `SchemaFile` storage document into the run
/// of root-level template elements the source `DataCompositionSchema` document
/// carries between its last `parameter` and its first `settingsVariant`.
///
/// The envelope keeps a report's area templates -- and only those -- in this
/// second document, with every table cell's appearance lifted out into
/// `<appearance>` children of `SchemaFile` itself that the cells select by
/// ordinal. The source document has no such side table: it writes the selected
/// appearance's items inline at the cell. That join, and the fact that the
/// fragment inherits its `dcsat` prefix from the `AreaTemplate` element it
/// sits under rather than from a root declaration, are the only two things
/// this direction does beyond what
/// [`rewrite_dcs_primary_schema_storage_document`] already does.
///
/// Returns `Ok(None)` for the empty terminal document the overwhelming
/// majority of templates carry, which contributes no fragment at all. The
/// resolver maps are the same ones the primary direction takes.
pub fn rewrite_dcs_terminal_area_template_storage_fragment(
    bytes: &[u8],
    reference_types: &BTreeMap<String, String>,
    type_set_types: &BTreeMap<String, String>,
    opaque_type_ids: &BTreeSet<String>,
    style_item_names: &BTreeMap<String, String>,
) -> Result<Option<Vec<u8>>, DcsInnerSchemaError> {
    let policy = policy()?;
    let body = storage_document_body(bytes, "terminal schema")?;
    let tokens = scan_raw_tokens(body)?;
    let (root_declarations, children) = schema_file_children(&tokens)?;
    let Some((schema_start, schema_end)) = children.first().copied() else {
        return unsupported("terminal SchemaFile wraps no dataCompositionSchema");
    };
    // Nothing to inline and nothing to write: the schema element is either
    // self-closing or carries only layout whitespace.
    if schema_end == schema_start
        || tokens[schema_start + 1..schema_end]
            .iter()
            .all(|token| matches!(token, RawToken::Text(value) if value.trim().is_empty()))
    {
        if children.len() != 1 {
            return unsupported(
                "an empty terminal dataCompositionSchema carries a side-table appearance",
            );
        }
        return Ok(None);
    }
    let appearances = &children[1..];
    for (start, _) in appearances {
        let RawToken::Tag(tag) = &tokens[*start] else {
            return Err(DcsInnerSchemaError::Malformed(
                "SchemaFile child range does not start at a tag".into(),
            ));
        };
        let appearance = scan_start_tag(tag)?;
        let (prefix, local) = split_prefix(appearance.name);
        // The side-table element carries its own default declaration, so its
        // name only resolves against the root plus its own scope.
        let mut own = Vec::new();
        for (key, value) in &appearance.attributes {
            if *key == "xmlns" {
                own.push((String::new(), (*value).to_owned()));
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                own.push((prefix.to_owned(), (*value).to_owned()));
            }
        }
        let scopes: NamespaceScopes = vec![root_declarations.clone(), own];
        if local != "appearance"
            || resolve_prefix(&scopes, prefix) != Some(DCS_AREA_TEMPLATE_NAMESPACE_URI)
        {
            return unsupported(
                "a terminal SchemaFile child beside dataCompositionSchema is not an appearance",
            );
        }
    }
    let selected = RefCell::new(BTreeSet::new());
    let context = RewriteContext {
        policy: &policy,
        reference_types,
        type_set_types,
        opaque_type_ids,
        style_item_names,
        settings_blocks: &[],
        appearances,
        selected: &selected,
    };
    let fragment = rewrite_tokens(
        &context,
        &tokens,
        schema_start..schema_end + 1,
        RewriteSeed {
            mode: RewriteMode::TerminalFragment,
            depth: 1,
            indent_delta: -1,
            scopes: vec![root_declarations],
            source_scopes: vec![Vec::new()],
        },
    )?;
    if selected.borrow().len() != appearances.len() {
        return unsupported("the terminal side table holds an appearance no table cell selects");
    }
    // The schema element contributed the line break before its first child and
    // the one before its own closing tag; the caller owns both.
    let fragment = fragment
        .trim_start_matches(['\r', '\n'])
        .trim_end_matches(['\r', '\n', '\t']);
    if fragment.is_empty() {
        return unsupported("terminal dataCompositionSchema rewrote to nothing");
    }
    Ok(Some(fragment.as_bytes().to_vec()))
}

/// A `SchemaFile` root's own namespace declarations paired with the token
/// index range (start tag, end tag) of each of its direct children, in
/// document order.
type SchemaFileChildren = (Vec<(String, String)>, Vec<(usize, usize)>);

/// Splits a scanned terminal document into [`SchemaFileChildren`]. A
/// self-closing child reports the same index twice.
fn schema_file_children(
    tokens: &[RawToken<'_>],
) -> Result<SchemaFileChildren, DcsInnerSchemaError> {
    let mut root_declarations: Option<Vec<(String, String)>> = None;
    let mut children = Vec::new();
    let mut depth = 0usize;
    let mut open: Option<usize> = None;
    for (index, token) in tokens.iter().enumerate() {
        let RawToken::Tag(tag) = token else {
            continue;
        };
        if tag.starts_with("<?") || tag.starts_with("<!") {
            return unsupported("comments, PI and doctype are outside the cohort");
        }
        if tag.starts_with("</") {
            depth = depth.checked_sub(1).ok_or_else(|| {
                DcsInnerSchemaError::Malformed("unexpected closing element".into())
            })?;
            if depth == 1
                && let Some(start) = open.take()
            {
                children.push((start, index));
            }
            continue;
        }
        let start = scan_start_tag(tag)?;
        if depth == 0 {
            let (prefix, local) = split_prefix(start.name);
            if !prefix.is_empty() || local != "SchemaFile" || start.self_closing {
                return unsupported("terminal schema root is not a SchemaFile wrapper");
            }
            let mut declarations = Vec::new();
            for (key, value) in &start.attributes {
                if *key == "xmlns" {
                    declarations.push((String::new(), (*value).to_owned()));
                } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                    declarations.push((prefix.to_owned(), (*value).to_owned()));
                }
            }
            root_declarations = Some(declarations);
        } else if depth == 1 {
            if start.self_closing {
                children.push((index, index));
            } else {
                open = Some(index);
            }
        }
        if !start.self_closing {
            depth += 1;
        }
    }
    if depth != 0 || open.is_some() {
        return Err(DcsInnerSchemaError::Malformed("unclosed element".into()));
    }
    let root_declarations = root_declarations
        .ok_or_else(|| DcsInnerSchemaError::Malformed("terminal schema has no root".into()))?;
    Ok((root_declarations, children))
}

/// Strips the BOM and the canonical XML declaration every platform-written
/// DCS storage document begins with, naming `what` in the failure.
fn storage_document_body<'a>(bytes: &'a [u8], what: &str) -> Result<&'a str, DcsInnerSchemaError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| DcsInnerSchemaError::Malformed(format!("{what} is not UTF-8")))?;
    let text = text
        .strip_prefix('\u{feff}')
        .ok_or_else(|| DcsInnerSchemaError::Malformed(format!("{what} has no UTF-8 BOM")))?;
    text.strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n")
        .ok_or_else(|| {
            DcsInnerSchemaError::Malformed(format!("{what} has no canonical declaration"))
        })
}

/// The source prefix an element or attribute namespace already has in the
/// target document, for a namespace the source root does not declare.
///
/// This only ever answers where the whole-document direction fails closed: the
/// `dcsat` prefix is declared by the `AreaTemplate` element that introduces
/// it, so everything under it is spelled through that declaration rather than
/// through one of its own.
///
/// It is the same rule in every direction, because it is the same document
/// shape. Which `SchemaFile` an area template is stored in is not fixed --
/// UH 3.2.12.6 keeps `Reports/РасшифровкаСтатистики/Templates/ОписаниеНастроек`
/// in the terminal one and
/// `Reports/РасшифровкаФормулыБюджетногоОтчета/Templates/ОсновнаяСхемаКомпоновкиДанных`
/// in the primary one -- and both storages write the very same
/// `<template xmlns:dcsat="..." xsi:type="dcsat:AreaTemplate">` that the
/// platform's own source writes, with the very same `dcsat:` descendants.
/// Answering only in the terminal direction refused the second group for
/// where it was stored rather than for what it said.
fn inherited_source_prefix(source_scopes: &SourcePrefixScopes, uri: &str) -> Option<String> {
    resolve_source_prefix(source_scopes, uri).filter(|prefix| !prefix.is_empty())
}

/// Rewrites `tokens[range]` in the source direction, starting from `seed`.
///
/// The same loop serves all three directions; `seed.mode` decides only what
/// the two outermost storage frames turn into and whether the terminal's
/// `appIndex`/inherited-prefix rules are in play.
fn rewrite_tokens(
    context: &RewriteContext<'_>,
    tokens: &[RawToken<'_>],
    range: std::ops::Range<usize>,
    seed: RewriteSeed,
) -> Result<String, DcsInnerSchemaError> {
    let policy = context.policy;
    let mode = seed.mode;
    let settings_blocks = context.settings_blocks;
    let reference_types = context.reference_types;
    let type_set_types = context.type_set_types;
    let opaque_type_ids = context.opaque_type_ids;
    let style_item_names = context.style_item_names;

    let mut out = String::new();
    let mut scopes: NamespaceScopes = seed.scopes;
    let mut source_scopes: SourcePrefixScopes = seed.source_scopes;
    let mut frames: Vec<RewriteState> = (0..seed.depth)
        .map(|_| RewriteState::new(RewriteFrame::Outer))
        .collect();
    let base_frames = frames.len();
    let mut pending: Option<&str> = None;
    let mut variant = 0usize;

    for index in range.clone() {
        let token = &tokens[index];
        match token {
            RawToken::Text(value) => {
                if pending.is_some() {
                    return Err(DcsInnerSchemaError::Malformed(
                        "adjacent character data runs".into(),
                    ));
                }
                pending = Some(value);
            }
            RawToken::Tag(tag) => {
                if tag.starts_with("<?") || tag.starts_with("<!") {
                    return unsupported("comments, PI and doctype are outside the cohort");
                }
                let closing = tag.starts_with("</");
                let mut resolved_type_qname: Option<String> = None;
                if let Some(value) = pending.take() {
                    let Some(state) = frames.last() else {
                        if !value.trim().is_empty() {
                            return Err(DcsInnerSchemaError::Malformed("text outside root".into()));
                        }
                        continue;
                    };
                    match state.frame {
                        RewriteFrame::Outer => {
                            if !value.trim().is_empty() {
                                return unsupported("rewritten range carries loose character data");
                            }
                            out.push_str(&shift_indent(value, seed.indent_delta));
                        }
                        RewriteFrame::Wrapper => {
                            if !value.trim().is_empty() {
                                return unsupported("SchemaFile wrapper carries character data");
                            }
                        }
                        RewriteFrame::TypeId | RewriteFrame::AppIndex => {}
                        _ if state.text == RewriteTextKind::TypeQName => {
                            let resolved = rewrite_qname_value(
                                policy,
                                &scopes,
                                &state.renamed,
                                "xsi:type",
                                value,
                            )?;
                            out.push_str(&resolved);
                            // The source-direction QName is what the builtin
                            // sort-bounds table is keyed by, so it is kept for
                            // the closing tag to record.
                            resolved_type_qname = Some(resolved);
                        }
                        _ if state.text == RewriteTextKind::ColorValue => {
                            let rendered = rewrite_color_value(
                                policy,
                                &scopes,
                                &source_scopes,
                                state,
                                style_item_names,
                                value,
                            )?;
                            if let Some(declaration) = rendered.declaration {
                                out.insert_str(state.declaration_offset, &declaration);
                            }
                            out.push_str(&rendered.value);
                        }
                        _ => {
                            if value.trim().is_empty() && (!closing || state.saw_child) {
                                out.push_str(&shift_indent(value, seed.indent_delta));
                            } else {
                                out.push_str(value);
                            }
                        }
                    }
                }
                if closing {
                    let name = tag[2..tag.len() - 1].trim();
                    let mut state = frames.pop().ok_or_else(|| {
                        DcsInnerSchemaError::Malformed("unexpected closing element".into())
                    })?;
                    // An element that carried a type run of its own ends it
                    // here rather than at a following sibling.
                    state.flush_type_run(&mut out)?;
                    scopes.pop();
                    source_scopes.pop();
                    match state.frame {
                        RewriteFrame::Outer => {
                            return Err(DcsInnerSchemaError::Malformed(
                                "rewritten token range is not balanced".into(),
                            ));
                        }
                        RewriteFrame::Wrapper => {}
                        RewriteFrame::Root => {
                            if mode == RewriteMode::PrimaryDocument {
                                out.push_str("</DataCompositionSchema>");
                            }
                        }
                        RewriteFrame::TypeId | RewriteFrame::AppIndex => {}
                        RewriteFrame::Element(emitted) => {
                            // Whitespace-only, not strictly zero-length: the
                            // storage document's own pretty-printing writes
                            // indentation text nodes even inside a childless
                            // element, and the platform's own source XML
                            // does not preserve that indentation once there
                            // is nothing left for it to indent.
                            let omit = state.omit_if_empty
                                && !state.saw_child
                                && out[state.start_tag_end_offset..].trim().is_empty();
                            if omit {
                                // The pending text run flushed ahead of this
                                // element's own opening tag is the
                                // indentation that led up to it, trimmed too
                                // so no orphaned blank line is left behind.
                                out.truncate(state.start_tag_begin_offset);
                                let trimmed_len =
                                    out.trim_end_matches(['\r', '\n', '\t', ' ']).len();
                                out.truncate(trimmed_len);
                            } else if out[state.start_tag_end_offset..].is_empty() {
                                // Nothing at all was written between the two
                                // tags -- either storage spelled the element
                                // as an empty open/close pair, or an omitted
                                // child left it that way. The platform never
                                // writes such a pair: across 865 of its own
                                // data-composition templates (DO 3.0.21.3,
                                // BSP demo 3.1.12.297, UT 11.5.27.75) there
                                // is not one, and a childless element is
                                // always self-closed. So the opening tag's
                                // own `>` becomes `/>` and no closing tag
                                // follows. UH 3.2.12.6
                                // `Reports/СправкаРасчетАмортизации/Templates/ОсновнаяСхемаКомпоновкиДанных`
                                // is the case that shows both halves: ten
                                // `dcsat:tableCell`s whose only child is an
                                // empty `dcsat:appearance` are written
                                // `<dcsat:tableCell/>`.
                                out.truncate(state.start_tag_end_offset - 1);
                                out.push_str("/>");
                            } else {
                                if mode == RewriteMode::PrimaryDocument
                                    && name.split_once(':').map_or(name, |(_, local)| local)
                                        == "settingsVariant"
                                    && frames.len() == 2
                                {
                                    let block = settings_blocks.get(variant).ok_or_else(|| {
                                        DcsInnerSchemaError::UnsupportedSource(
                                            "settingsVariant count exceeds the Settings document count"
                                                .into(),
                                        )
                                    })?;
                                    variant += 1;
                                    let tail =
                                        out.len() - out.trim_end_matches(['\r', '\n', '\t']).len();
                                    let held = out.split_off(out.len() - tail);
                                    append_indented_fragment(&mut out, block.as_str(), 2);
                                    out.push_str(&held);
                                }
                                out.push('<');
                                out.push('/');
                                out.push_str(&emitted);
                                out.push('>');
                            }
                        }
                    }
                    if let Some(element_start) = state.literal_type_start {
                        // A literal `Type` the bounds table does not carry has
                        // no key of its own, but it still stands in storage's
                        // `Type` group; only a run that also holds a `TypeId`
                        // needs a key from it, and such a run is refused rather
                        // than ordered on a guess.
                        let key = resolved_type_qname
                            .as_deref()
                            .and_then(builtin_type_sort_bounds)
                            .map_or(TypeSortKey::StoredLiteral, |(lower, upper)| {
                                TypeSortKey::Builtin { lower, upper }
                            });
                        if let Some(parent) = frames.last_mut() {
                            parent.type_run.push(TypeRunEntry {
                                start: element_start,
                                end: out.len(),
                                key,
                            });
                        }
                    }
                    continue;
                }

                let start = scan_start_tag(tag)?;
                let mut declared: Vec<(String, String)> = Vec::new();
                for (key, value) in &start.attributes {
                    if *key == "xmlns" {
                        declared.push((String::new(), (*value).to_owned()));
                    } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                        declared.push((prefix.to_owned(), (*value).to_owned()));
                    }
                }
                scopes.push(declared);
                source_scopes.push(Vec::new());
                let (prefix, local) = split_prefix(start.name);
                let uri = resolve_prefix(&scopes, prefix)
                    .ok_or_else(|| {
                        DcsInnerSchemaError::Malformed(format!("unbound namespace prefix {prefix}"))
                    })?
                    .to_owned();
                let depth = frames.len();
                let literal_type = uri == policy.data_core_namespace_uri() && local == "Type";
                let storage_type_id = uri == policy.data_core_namespace_uri() && local == "TypeId";
                if let Some(state) = frames.last_mut() {
                    state.saw_child = true;
                    if !literal_type && !storage_type_id {
                        // A run of type siblings ends at the first child that
                        // is not one, and its order can be settled then.
                        state.flush_type_run(&mut out)?;
                    }
                }
                if mode == RewriteMode::PrimaryDocument && depth == 0 {
                    if local != "SchemaFile" || !uri.is_empty() || start.self_closing {
                        return unsupported("primary schema root is not a SchemaFile wrapper");
                    }
                    frames.push(RewriteState::new(RewriteFrame::Wrapper));
                    continue;
                }
                if mode != RewriteMode::InlineAppearance && depth == 1 {
                    if local != "dataCompositionSchema"
                        || uri != policy.schema_namespace_uri()
                        || start.self_closing
                    {
                        return unsupported("SchemaFile does not wrap a dataCompositionSchema");
                    }
                    // The fragment direction writes only what the schema root
                    // contains: the source document's own root, and every
                    // namespace it declares, come from the primary document.
                    if mode == RewriteMode::TerminalFragment {
                        frames.push(RewriteState::new(RewriteFrame::Root));
                        continue;
                    }
                    out.push_str("<DataCompositionSchema xmlns=\"");
                    out.push_str(policy.schema_namespace_uri());
                    out.push_str(
                        "\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\"",
                    );
                    out.push_str(
                        " xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\"",
                    );
                    out.push_str(" xmlns:dcsset=\"");
                    out.push_str(policy.settings_namespace_uri());
                    out.push_str("\" xmlns:v8=\"");
                    out.push_str(policy.data_core_namespace_uri());
                    out.push_str("\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"");
                    out.push_str(policy.xml_schema_namespace_uri());
                    out.push_str("\" xmlns:xsi=\"");
                    out.push_str(policy.xsi_namespace_uri());
                    out.push_str("\">");
                    frames.push(RewriteState::new(RewriteFrame::Root));
                    continue;
                }

                if mode == RewriteMode::TerminalFragment
                    && uri == DCS_AREA_TEMPLATE_NAMESPACE_URI
                    && local == "appIndex"
                {
                    // The terminal envelope keeps a table cell's appearance
                    // out of line: the cell carries the ordinal of one of the
                    // `SchemaFile`'s own `<appearance>` children, and the
                    // source document carries that child's items inline in
                    // its place. The join is by position and nothing else.
                    let selected = match tokens.get(index + 1) {
                        Some(RawToken::Text(value)) => value.trim(),
                        _ => return unsupported("appIndex has no storage ordinal"),
                    };
                    let selected: usize = selected.parse().map_err(|_| {
                        DcsInnerSchemaError::UnsupportedSource(format!(
                            "appIndex {selected} is not an ordinal"
                        ))
                    })?;
                    let (appearance_start, appearance_end) =
                        *context.appearances.get(selected).ok_or_else(|| {
                            DcsInnerSchemaError::UnsupportedSource(format!(
                                "appIndex {selected} selects no side-table appearance"
                            ))
                        })?;
                    context.selected.borrow_mut().insert(selected);
                    let RawToken::Tag(appearance_tag) = &tokens[appearance_start] else {
                        return Err(DcsInnerSchemaError::Malformed(
                            "side-table appearance range does not start at a tag".into(),
                        ));
                    };
                    let appearance = scan_start_tag(appearance_tag)?;
                    // The side table sits directly under `SchemaFile`, so its
                    // storage namespaces are the root's declarations plus its
                    // own -- not the ones in scope where it is being inlined.
                    let mut appearance_scopes: NamespaceScopes =
                        vec![scopes.first().cloned().unwrap_or_default(), Vec::new()];
                    for (key, value) in &appearance.attributes {
                        if *key == "xmlns" {
                            appearance_scopes[1].push((String::new(), (*value).to_owned()));
                        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                            appearance_scopes[1].push((prefix.to_owned(), (*value).to_owned()));
                        }
                    }
                    // The side-table element needs its own discriminator
                    // because the slot it sits in is untyped; the inline
                    // element does not, because `dcsat:appearance` already is
                    // that type, so the discriminator is dropped rather than
                    // re-spelled. Any other one is a shape with no evidence
                    // for how it is written inline.
                    for (key, value) in &appearance.attributes {
                        if *key == "xmlns" || key.starts_with("xmlns:") {
                            continue;
                        }
                        if expanded_attribute_name(&appearance_scopes, key).as_deref()
                            != Some(XSI_TYPE_EXPANDED_NAME)
                            || expanded_qname(&appearance_scopes, value).as_deref()
                                != Some(TABLE_CELL_APPEARANCE_EXPANDED_NAME)
                        {
                            return unsupported(
                                "a side-table appearance carries an attribute beyond its \
                                 TableCellAppearance discriminator",
                            );
                        }
                    }
                    let Some(appearance_prefix) =
                        inherited_source_prefix(&source_scopes, DCS_AREA_TEMPLATE_NAMESPACE_URI)
                    else {
                        return unsupported(
                            "no area-template prefix is in scope where an appearance is inlined",
                        );
                    };
                    // The inlined element takes the place of the `appIndex`
                    // it replaces, so its children sit one deeper than it and
                    // its layout moves from the side table's own indentation
                    // to this cell's.
                    let body = rewrite_tokens(
                        context,
                        tokens,
                        appearance_start + 1..appearance_end,
                        RewriteSeed {
                            mode: RewriteMode::InlineAppearance,
                            depth: depth + 1,
                            indent_delta: isize::try_from(depth).map_err(|_| {
                                DcsInnerSchemaError::Malformed("XML depth overflow".into())
                            })? - 2,
                            scopes: appearance_scopes,
                            source_scopes: source_scopes.clone(),
                        },
                    )?;
                    if body.trim().is_empty() {
                        // The side-table entry the ordinal selects carries
                        // nothing, and the platform's own source XML never
                        // writes an empty appearance -- over the
                        // `Templates/*/Ext/Template.xml` trees of ERP УХ
                        // 3.2.12.6, 1С:УТ 11.5.27.75, БСП demo/base and
                        // Документооборот КОРП 3.0.21.3 there are 16 185
                        // `<dcsat:appearance>` and 11 311 `<appearance>` and
                        // not one of either is self-closed or an empty
                        // open/close pair. So nothing is written in the
                        // `appIndex`'s place, and the indentation that led up
                        // to it is trimmed with it, exactly as the bare
                        // self-closed `appearance` above is dropped.
                        let trimmed_len =
                            out.trim_end_matches(['\r', '\n', '\t', ' ']).len();
                        out.truncate(trimmed_len);
                    } else {
                        out.push('<');
                        out.push_str(&appearance_prefix);
                        out.push_str(":appearance>");
                        out.push_str(&body);
                        out.push_str("</");
                        out.push_str(&appearance_prefix);
                        out.push_str(":appearance>");
                    }
                    if start.self_closing {
                        scopes.pop();
                        source_scopes.pop();
                    } else {
                        frames.push(RewriteState::new(RewriteFrame::AppIndex));
                    }
                    continue;
                }

                if storage_type_id {
                    // A `TypeId` becomes a `<v8:Type>` too, so a parent that
                    // carries both spellings ends up with several `v8:Type`
                    // children. Storage cannot say in which order: its schema
                    // puts every literal `Type` before every `TypeId`, while
                    // the platform interleaves them. What it does preserve is
                    // the reference members' own order -- ascending by type
                    // uuid, which is the platform's order -- so the run is
                    // recorded here and put right when it ends.
                    let element_start = out.len();
                    let content = match tokens.get(index + 1) {
                        Some(RawToken::Text(value)) => (*value).trim(),
                        _ if start.self_closing => "",
                        _ => {
                            return unsupported("TypeId has no storage uuid");
                        }
                    };
                    let type_id = content.to_ascii_lowercase();
                    // A uuid resolves either to one configuration type
                    // (`<v8:Type>`) or to a whole reference family such as
                    // `DocumentRef` or `AnyIBRef` (`<v8:TypeSet>`). Which of
                    // the two it is comes from the same type index, never from
                    // the shape of the uuid, so the two maps are disjoint and
                    // the element name is read rather than guessed.
                    let resolved = reference_types
                        .get(&type_id)
                        .map(|qualified| ("Type", qualified))
                        .or_else(|| {
                            type_set_types
                                .get(&type_id)
                                .map(|qualified| ("TypeSet", qualified))
                        });
                    let key = match resolved {
                        Some(("TypeSet", _)) => TypeSortKey::Family,
                        Some(_) => TypeSortKey::Reference(type_id.clone()),
                        None => TypeSortKey::Unevidenced,
                    };
                    match resolved {
                        Some((element, qualified)) => {
                            out.push_str("<v8:");
                            out.push_str(element);
                            out.push_str(" xmlns:d");
                            out.push_str(&depth.to_string());
                            out.push_str("p1=\"");
                            out.push_str(policy.current_config_namespace_uri());
                            out.push_str("\">d");
                            out.push_str(&depth.to_string());
                            out.push_str("p1:");
                            out.push_str(&escape(qualified));
                            out.push_str("</v8:");
                            out.push_str(element);
                            out.push('>');
                        }
                        // Already in its stored lexical form; re-escaping it
                        // would double-encode whatever the platform wrote.
                        None if opaque_type_ids.contains(&type_id) => {
                            out.push_str("<v8:TypeId>");
                            out.push_str(content);
                            out.push_str("</v8:TypeId>");
                        }
                        None => {
                            return unsupported(format!(
                                "TypeId {type_id} has no configuration type-index resolution"
                            ));
                        }
                    }
                    if let Some(parent) = frames.last_mut() {
                        parent.type_run.push(TypeRunEntry {
                            start: element_start,
                            end: out.len(),
                            key,
                        });
                    }
                    if start.self_closing {
                        scopes.pop();
                        source_scopes.pop();
                    } else {
                        frames.push(RewriteState::new(RewriteFrame::TypeId));
                    }
                    continue;
                }

                let element_prefix = match source_namespace_prefix(policy, &uri) {
                    Some(prefix) => prefix.to_owned(),
                    None => match inherited_source_prefix(&source_scopes, &uri) {
                        Some(prefix) => prefix,
                        None => {
                            return unsupported(format!(
                                "element namespace {uri} is outside the source root declaration"
                            ));
                        }
                    },
                };
                let emitted_name = if element_prefix.is_empty() {
                    local.to_owned()
                } else {
                    format!("{element_prefix}:{local}")
                };
                let mut declarations = String::new();
                let mut renamed: Vec<(String, String)> = Vec::new();
                let mut declared_source: Vec<(String, String)> = Vec::new();
                let mut local_declarations = 0usize;
                for (key, value) in &start.attributes {
                    let declared_prefix = if *key == "xmlns" {
                        Some("")
                    } else {
                        key.strip_prefix("xmlns:")
                    };
                    let Some(declared_prefix) = declared_prefix else {
                        continue;
                    };
                    // A default declaration is unusable in the source
                    // direction whenever the namespace already has a prefix
                    // there: the source root binds the default to the schema
                    // namespace, so the spelling that reaches the output is
                    // the prefixed one and the declaration has nothing left
                    // to say. This is how a side-table `<appearance
                    // xmlns="...area-template">` becomes a bare
                    // `<dcsat:appearance>`.
                    let inherited_default = declared_prefix
                        .is_empty()
                        .then(|| inherited_source_prefix(&source_scopes, value))
                        .flatten();
                    match source_namespace_prefix(policy, value) {
                        Some(source) => {
                            declared_source.push(((*value).to_owned(), source.to_owned()));
                            renamed.push((declared_prefix.to_owned(), source.to_owned()));
                        }
                        None if inherited_default.is_some() => {
                            let source = inherited_default.unwrap_or_default();
                            declared_source.push(((*value).to_owned(), source.clone()));
                            renamed.push((declared_prefix.to_owned(), source));
                        }
                        // A namespace the source root does not declare keeps
                        // its own declaration at the point of use. Only a
                        // generated `dNpM` prefix is renumbered -- it names
                        // the storage document's own depth, which the source
                        // document does not share. A prefix the platform
                        // spelled itself (`style`, `sys`, `web`, `win`) is
                        // not depth-derived and travels through verbatim, as
                        // the platform's own export of the same records
                        // shows.
                        None if is_generated_depth_prefix(declared_prefix) => {
                            local_declarations += 1;
                            let replacement = format!("d{depth}p{local_declarations}");
                            declarations.push_str(" xmlns:");
                            declarations.push_str(&replacement);
                            declarations.push_str("=\"");
                            declarations.push_str(value);
                            declarations.push('"');
                            declared_source.push(((*value).to_owned(), replacement.clone()));
                            renamed.push((declared_prefix.to_owned(), replacement));
                        }
                        None => {
                            declarations.push_str(" xmlns:");
                            declarations.push_str(declared_prefix);
                            declarations.push_str("=\"");
                            declarations.push_str(value);
                            declarations.push('"');
                            declared_source.push(((*value).to_owned(), declared_prefix.to_owned()));
                            renamed.push((declared_prefix.to_owned(), declared_prefix.to_owned()));
                        }
                    }
                }
                let mut rendered = String::new();
                for (key, value) in &start.attributes {
                    if *key == "xmlns" || key.starts_with("xmlns:") {
                        continue;
                    }
                    let (attribute_prefix, attribute_local) = split_prefix(key);
                    let emitted_key = if attribute_prefix.is_empty() {
                        attribute_local.to_owned()
                    } else {
                        let attribute_uri =
                            resolve_prefix(&scopes, attribute_prefix).ok_or_else(|| {
                                DcsInnerSchemaError::Malformed(format!(
                                    "unbound namespace prefix {attribute_prefix}"
                                ))
                            })?;
                        let Some(source) = source_namespace_prefix(policy, attribute_uri) else {
                            return unsupported(format!(
                                "attribute namespace {attribute_uri} is outside the source root declaration"
                            ));
                        };
                        if source.is_empty() {
                            return unsupported(
                                "an attribute cannot bind to the source default namespace",
                            );
                        }
                        format!("{source}:{attribute_local}")
                    };
                    // The one attribute whose value is a stored reference
                    // rather than a QName or a literal: a `{data/ui}Font`'s
                    // `ref`, which names a configuration `StyleItem` by uuid
                    // in storage and by name in the source direction.
                    let emitted_value =
                        if emitted_key == "ref" && style_item_reference_uuid(value).is_some() {
                            let rendered = rewrite_style_item_reference(
                                &declared_source,
                                &source_scopes,
                                depth,
                                local_declarations + 1,
                                style_item_names,
                                value,
                            )?;
                            if let Some(declaration) = rendered.declaration {
                                declarations.push_str(&declaration);
                                local_declarations += 1;
                                declared_source.push((
                                    DATA_UI_STYLE_NAMESPACE_URI.to_owned(),
                                    format!("d{depth}p{local_declarations}"),
                                ));
                            }
                            rendered.value
                        } else {
                            rewrite_qname_value(policy, &scopes, &renamed, &emitted_key, value)?
                        };
                    rendered.push(' ');
                    rendered.push_str(&emitted_key);
                    rendered.push_str("=\"");
                    rendered.push_str(&emitted_value);
                    rendered.push('"');
                }
                if let Some(scope) = source_scopes.last_mut() {
                    *scope = declared_source;
                }
                // Evidenced only for the bare (unprefixed, DCS schema
                // default-namespace) `appearance`/`inputParameters` shape
                // with no attributes -- see `RewriteState::omit_if_empty`.
                // The area-template `appearance` is the same placeholder as
                // the schema-namespace one and is dropped by the same rule:
                // storage keeps an empty one, and across UH 3.2.12.6, UT
                // 11.5.27.75, BSP demo 3.1.12.297 and DO 3.0.21.3 the
                // platform's own source never writes a `dcsat:appearance`
                // that is empty -- in either spelling, self-closed or as an
                // open/close pair.
                // A `nestedSchema` carries its own `settingsVariant` inside
                // the primary document, so the settings writer's own
                // `outputParameters` rule (`DcsEmptyElementAction::OmitIfEmpty`
                // in `src/mssql_dump/dcs.rs`) has to hold here as well: it is
                // the same element in the same namespace, kept as an empty
                // placeholder by storage and written by no platform source.
                let omittable_empty = (element_prefix.is_empty()
                    && matches!(local, "appearance" | "inputParameters"))
                    || (uri == DCS_AREA_TEMPLATE_NAMESPACE_URI && local == "appearance")
                    || (uri == policy.settings_namespace_uri() && local == "outputParameters");
                let omit_if_empty = omittable_empty
                    && start
                        .attributes
                        .iter()
                        .all(|(key, _)| *key == "xmlns" || key.starts_with("xmlns:"));
                if start.self_closing && omit_if_empty {
                    // Storage already spells this element self-closed, so
                    // there is no content run to wait on: the platform's own
                    // source XML omits it outright, so nothing is written at
                    // all (not even the opening `<` this branch would
                    // otherwise start). The pending text run just flushed
                    // ahead of this tag is the indentation that led up to
                    // it -- pure whitespace, since anything else would have
                    // already failed the unsupported-shape checks above --
                    // and is trimmed too, so no orphaned blank line is left
                    // behind for a sibling that no longer follows anything.
                    scopes.pop();
                    source_scopes.pop();
                    let trimmed_len = out.trim_end_matches(['\r', '\n', '\t', ' ']).len();
                    out.truncate(trimmed_len);
                    continue;
                }
                let element_start = out.len();
                out.push('<');
                out.push_str(&emitted_name);
                let declaration_offset = out.len();
                out.push_str(&declarations);
                out.push_str(&rendered);
                if start.self_closing {
                    out.push_str("/>");
                    scopes.pop();
                    source_scopes.pop();
                    if literal_type {
                        // A `Type` with no content names no type at all, so
                        // nothing keys it among its siblings -- but storage
                        // still wrote it in its `Type` group.
                        if let Some(parent) = frames.last_mut() {
                            parent.type_run.push(TypeRunEntry {
                                start: element_start,
                                end: out.len(),
                                key: TypeSortKey::StoredLiteral,
                            });
                        }
                    }
                } else {
                    out.push('>');
                    let start_tag_end_offset = out.len();
                    let mut state = RewriteState::new(RewriteFrame::Element(emitted_name));
                    state.start_tag_begin_offset = element_start;
                    state.start_tag_end_offset = start_tag_end_offset;
                    state.omit_if_empty = omit_if_empty;
                    // `v8:Type`/`v8:TypeSet` content is a QName, so it moves
                    // to the source document's prefixes exactly like an
                    // `xsi:type` attribute does: a storage `StandardPeriod`
                    // resolved through the data-core default namespace is
                    // spelled `v8:StandardPeriod` in the source direction.
                    state.text = if uri == policy.data_core_namespace_uri()
                        && (local == "Type" || local == "TypeSet")
                    {
                        RewriteTextKind::TypeQName
                    } else if start
                        .attributes
                        .iter()
                        .filter(|(key, _)| *key != "xmlns" && !key.starts_with("xmlns:"))
                        .any(|(key, value)| {
                            expanded_attribute_name(&scopes, key).as_deref()
                                == Some(XSI_TYPE_EXPANDED_NAME)
                                && expanded_qname(&scopes, value).as_deref()
                                    == Some(DATA_CORE_TYPE_EXPANDED_NAME)
                        })
                    {
                        // An element declared to *be* a `{data/core}Type`
                        // holds a type QName, exactly as a `v8:Type` element
                        // does, and its prefix has to move with the
                        // declaration this rewrite renumbered. Copying the
                        // storage spelling through left the prefix unbound:
                        // UH 3.2.12.6
                        // `Reports/РегистрыНалоговогоУчета/Templates/РегистрНезавершенноеПроизводство`
                        // declared `xmlns:d3p1="http://v8.1c.ru/8.2/data/types"`
                        // on the element and then named `d4p2:Undefined`
                        // inside it. The platform writes `d3p1:Undefined` --
                        // the prefix it just declared.
                        RewriteTextKind::TypeQName
                    } else if uri == DCS_CORE_NAMESPACE_URI
                        && local == "value"
                        && start
                            .attributes
                            .iter()
                            .filter(|(key, _)| *key != "xmlns" && !key.starts_with("xmlns:"))
                            .any(|(key, value)| {
                                expanded_attribute_name(&scopes, key).as_deref()
                                    == Some(XSI_TYPE_EXPANDED_NAME)
                                    && expanded_qname(&scopes, value).as_deref()
                                        == Some(DATA_UI_COLOR_EXPANDED_NAME)
                            })
                    {
                        RewriteTextKind::ColorValue
                    } else {
                        RewriteTextKind::Literal
                    };
                    state.renamed = renamed;
                    state.declaration_offset = declaration_offset;
                    state.next_declaration = local_declarations + 1;
                    state.depth = depth;
                    if literal_type {
                        state.literal_type_start = Some(element_start);
                    }
                    frames.push(state);
                }
            }
        }
    }
    // A range that ends inside its parent -- an inlined `<appearance>`'s
    // children -- ends on the layout whitespace before the closing tag the
    // caller writes, so that run is still held back here.
    if let Some(value) = pending.take() {
        if !value.trim().is_empty() {
            return unsupported("rewritten range ends in character data");
        }
        out.push_str(&shift_indent(value, seed.indent_delta));
    }
    if frames.len() != base_frames {
        return Err(DcsInnerSchemaError::Malformed("unclosed element".into()));
    }
    if mode == RewriteMode::PrimaryDocument && variant != settings_blocks.len() {
        return unsupported("settingsVariant count does not match the Settings document count");
    }
    Ok(out)
}

/// Re-prefixes a QName-valued attribute for the source direction.
///
/// Only a value whose prefix is actually declared in scope is touched: an
/// `xsi:type` naming a storage-local `dNpM` prefix moves to whichever prefix
/// the source document uses for that namespace, and an unprefixed `xsi:type`
/// picks up the prefix of the default namespace it resolved through (this is
/// what turns the storage `xsi:type="StructureItemGroup"` under a default
/// settings namespace into `xsi:type="dcsset:StructureItemGroup"`). Anything
/// else -- a literal like `DataSetQuery`, a boolean, a number, free text --
/// is copied through untouched.
fn rewrite_qname_value(
    policy: &DcsInnerSchemaPolicy,
    scopes: &NamespaceScopes,
    renamed: &[(String, String)],
    key: &str,
    value: &str,
) -> Result<String, DcsInnerSchemaError> {
    let (prefix, local) = split_prefix(value);
    if !prefix.is_empty() {
        if let Some(source) = renamed
            .iter()
            .find(|(declared, _)| declared == prefix)
            .map(|(_, source)| source.clone())
        {
            return Ok(if source.is_empty() {
                local.to_owned()
            } else {
                format!("{source}:{local}")
            });
        }
        if let Some(uri) = resolve_prefix(scopes, prefix)
            && let Some(source) = source_namespace_prefix(policy, uri)
        {
            return Ok(if source.is_empty() {
                local.to_owned()
            } else {
                format!("{source}:{local}")
            });
        }
        return Ok(value.to_owned());
    }
    if key != "xsi:type" {
        return Ok(value.to_owned());
    }
    let Some(default_uri) = resolve_prefix(scopes, "") else {
        return Ok(value.to_owned());
    };
    if default_uri.is_empty() {
        return Ok(value.to_owned());
    }
    let Some(source) = source_namespace_prefix(policy, default_uri) else {
        return unsupported(format!(
            "xsi:type default namespace {default_uri} is outside the source root declaration"
        ));
    };
    Ok(if source.is_empty() {
        value.to_owned()
    } else {
        format!("{source}:{value}")
    })
}

/// Splices a rendered settings fragment in at `depth`, re-indenting its
/// layout and only its layout.
///
/// A line break that falls inside character data -- a multi-line query,
/// expression or presentation string -- belongs to the stored value, not to
/// the document's shape. Re-indenting it, or normalizing its line ending,
/// would change the value; the platform's own export keeps such runs byte for
/// byte, so this does too.
fn append_indented_fragment(out: &mut String, fragment: &str, depth: usize) {
    let literal = character_data_runs(fragment);
    let start = fragment.len() - fragment.trim_start().len();
    let body = &fragment[start..fragment.trim_end().len()];
    // Only the fragment's own layout lines carry indentation to normalize:
    // a line inside a stored multi-line value has whatever indentation the
    // value has, which says nothing about how deep the fragment sits.
    let mut base_indent = usize::MAX;
    let mut scan = start;
    for (index, chunk) in body.split_inclusive('\n').enumerate() {
        let text = chunk.strip_suffix('\n').unwrap_or(chunk);
        let text = text.strip_suffix('\r').unwrap_or(text);
        if index > 0 && !text.trim().is_empty() && !offset_continues_character_data(&literal, scan)
        {
            base_indent = base_indent.min(text.len() - text.trim_start_matches(['\t', ' ']).len());
        }
        scan += chunk.len();
    }
    let base_indent = if base_indent == usize::MAX {
        0
    } else {
        base_indent
    };
    let mut offset = start;
    let mut separator: Option<(usize, usize)> = None;
    for (index, chunk) in body.split_inclusive('\n').enumerate() {
        let text = chunk.strip_suffix('\n').unwrap_or(chunk);
        let text = text.strip_suffix('\r').unwrap_or(text);
        if index > 0 && offset_continues_character_data(&literal, offset) {
            // Verbatim: the line break the stored value itself carries,
            // then the value's own bytes with their own indentation.
            if let Some((from, to)) = separator {
                out.push_str(&fragment[from..to]);
            }
            out.push_str(text);
        } else {
            let relative = if index == 0 {
                text
            } else {
                text.get(base_indent..).unwrap_or(text)
            }
            .trim_end();
            line(out, depth, relative);
        }
        separator = Some((offset + text.len(), offset + chunk.len()));
        offset += chunk.len();
    }
}

/// Byte ranges of the text runs that carry character data rather than
/// pretty-printing whitespace.
///
/// A text run is everything between a `>` that closes a tag and the `<` that
/// opens the next one. The scan is quote-aware so a `>` inside an attribute
/// value cannot end a tag early.
fn character_data_runs(xml: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut quote: Option<u8> = None;
    let mut in_tag = false;
    let mut run_start = 0usize;
    for (offset, byte) in xml.bytes().enumerate() {
        if in_tag {
            match (quote, byte) {
                (Some(open), byte) if byte == open => quote = None,
                (Some(_), _) => {}
                (None, b'"' | b'\'') => quote = Some(byte),
                (None, b'>') => {
                    in_tag = false;
                    run_start = offset + 1;
                }
                (None, _) => {}
            }
        } else if byte == b'<' {
            in_tag = true;
            if run_start < offset && !xml[run_start..offset].trim().is_empty() {
                runs.push((run_start, offset));
            }
        }
    }
    if !in_tag && run_start < xml.len() && !xml[run_start..].trim().is_empty() {
        runs.push((run_start, xml.len()));
    }
    runs
}

/// Whether a line starting at `offset` continues a character-data run rather
/// than starting one. The run's own first line is still preceded by markup,
/// so only offsets strictly inside a run are continuations.
fn offset_continues_character_data(runs: &[(usize, usize)], offset: usize) -> bool {
    runs.iter()
        .any(|(start, end)| offset > *start && offset < *end)
}

#[cfg(test)]
mod type_run_order_tests {
    use super::{
        TypeRunEntry, TypeSortKey, builtin_type_sort_bounds, evidenced_type_run_order,
        reorder_type_run,
    };

    /// Build a run whose rendered elements are single letters, so the
    /// permutation is readable straight off the rewritten output.
    fn run(members: &[(&str, TypeSortKey)]) -> (String, Vec<TypeRunEntry>) {
        let mut out = String::new();
        let mut entries = Vec::new();
        for (index, (rendered, key)) in members.iter().enumerate() {
            if index > 0 {
                out.push('.');
            }
            let start = out.len();
            out.push_str(rendered);
            entries.push(TypeRunEntry {
                start,
                end: out.len(),
                key: key.clone(),
            });
        }
        (out, entries)
    }

    fn builtin(qname: &str) -> TypeSortKey {
        let (lower, upper) =
            builtin_type_sort_bounds(qname).expect("the bounds table carries this builtin");
        TypeSortKey::Builtin { lower, upper }
    }

    fn reference(uuid: &str) -> TypeSortKey {
        TypeSortKey::Reference(uuid.to_owned())
    }

    fn rewritten(members: &[(&str, TypeSortKey)]) -> Result<String, String> {
        let (mut out, entries) = run(members);
        let order = evidenced_type_run_order(&entries).map_err(|error| error.to_string())?;
        reorder_type_run(&mut out, &entries, &order);
        Ok(out)
    }

    /// The reference configuration writes `xs:string` third of five in one
    /// template: two catalog uuids below the builtin's interval, then the
    /// builtin, then the two above it. Storage can only offer the builtin
    /// first, so this is the whole rule in one case.
    #[test]
    fn a_builtin_lands_between_the_references_its_interval_separates() {
        assert_eq!(
            rewritten(&[
                ("s", builtin("xs:string")),
                ("V", reference("3a87ef2a-9de1-4d34-9e5f-3c8cdf53b3ab")),
                ("B", reference("7632c6fe-8cac-4d68-a50a-5714e18b1fec")),
                ("K", reference("c1b798e4-28d2-42ac-b75d-c1521d1d8fff")),
                ("T", reference("f455b6b4-582e-4024-adba-c408ea60e8c6")),
            ])
            .as_deref(),
            Ok("V.B.s.K.T")
        );
    }

    /// A list of builtins alone loses nothing in storage -- the platform's own
    /// order survives the `Type`-before-`TypeId` grouping -- so it is left
    /// exactly as it arrived and no comparison is attempted. The reference
    /// configuration has such lists and they were already byte-exact.
    #[test]
    fn a_builtin_only_run_is_left_exactly_as_storage_had_it() {
        assert_eq!(
            rewritten(&[
                ("b", builtin("xs:boolean")),
                ("d", builtin("xs:dateTime")),
                ("n", builtin("v8:Null")),
            ])
            .as_deref(),
            Ok("b.d.n")
        );
    }

    /// So is a list of references alone, whose storage order is ascending by
    /// type uuid and therefore already the platform's.
    #[test]
    fn a_reference_only_run_is_left_exactly_as_storage_had_it() {
        assert_eq!(
            rewritten(&[
                ("A", reference("c1b798e4-28d2-42ac-b75d-c1521d1d8fff")),
                ("B", reference("3a87ef2a-9de1-4d34-9e5f-3c8cdf53b3ab")),
            ])
            .as_deref(),
            Ok("A.B")
        );
    }

    /// Reference families sort behind every ordered member, so a family whose
    /// uuid would have put it first in storage moves to the back.
    #[test]
    fn a_reference_family_falls_behind_every_ordered_member() {
        assert_eq!(
            rewritten(&[
                ("s", builtin("xs:string")),
                ("F", TypeSortKey::Family),
                ("R", reference("f455b6b4-582e-4024-adba-c408ea60e8c6")),
            ])
            .as_deref(),
            Ok("s.R.F")
        );
    }

    /// Fail-closed floor: a type uuid strictly inside the builtin's evidenced
    /// interval decides nothing, so the run is refused instead of ordered on a
    /// guess. `9bd43cde...` is a real configuration catalog inside the
    /// `xs:string` gap.
    #[test]
    fn a_type_uuid_inside_the_interval_is_refused() {
        let error = rewritten(&[
            ("s", builtin("xs:string")),
            ("R", reference("9bd43cde-a83d-11e7-7088-f45c898df8f7")),
        ])
        .expect_err("a uuid inside the interval has no evidenced position");
        assert!(
            error.contains("inside the builtin's evidenced sort interval"),
            "the refusal must name why the order is not derivable: {error}"
        );
    }

    /// Storage's grouping loses nothing inside one group. A run of literal
    /// `Type` siblings that mixes a builtin with configuration types storage
    /// itself spelled by QName is one group, so its storage order is already
    /// the platform's and nothing moves.
    ///
    /// Evidence: DO 3.0.21.3,
    /// `Reports/ИзменениеУчетныхЗаписей/Templates/Макет`. Storage writes
    /// `CatalogRef.ВнешниеПользователи`, `xs:string`,
    /// `CatalogRef.Пользователи`; the platform's own source writes the same
    /// three in the same order, and the template's two other mixed literal
    /// runs agree.
    #[test]
    fn a_literal_only_run_is_left_exactly_as_storage_had_it() {
        assert_eq!(
            rewritten(&[
                ("E", TypeSortKey::StoredLiteral),
                ("s", builtin("xs:string")),
                ("P", TypeSortKey::StoredLiteral),
            ])
            .as_deref(),
            Ok("E.s.P")
        );
    }

    /// Fail-closed floor: a literal type the bounds table does not carry has
    /// no key, so once a `TypeId` sibling puts it in the group storage did
    /// reorder, the run is refused instead of ordered on a guess.
    #[test]
    fn a_literal_beside_a_type_id_is_refused() {
        let error = rewritten(&[
            ("P", TypeSortKey::StoredLiteral),
            ("R", reference("f455b6b4-582e-4024-adba-c408ea60e8c6")),
        ])
        .expect_err("a literal with no key cannot be merged against a type id");
        assert!(
            error.contains("the bounds table does not carry"),
            "the refusal must say the literal has no key: {error}"
        );
    }

    /// A type id the configuration resolves to no name stays a `TypeId` in
    /// the source, and every `TypeId` stands behind every `Type`, so it needs
    /// no key: it goes to the back and the rest is ordered without it.
    ///
    /// Evidence: all three runs in the corpus that mix the two spellings --
    /// UH 3.2.12.6
    /// `Reports/КонтрольИсполненияОбязательствСПоставщиком/Templates/ОсновнаяСхемаКомпоновкиДанных`
    /// twice and `DataProcessors/СопоставлениеПланФактОперацийМСФО/Forms/Форма`
    /// once -- write every `v8:Type` first, and none writes one the other
    /// way.
    #[test]
    fn an_unresolved_type_id_falls_behind_every_named_member() {
        assert_eq!(
            rewritten(&[
                ("O", TypeSortKey::Unevidenced),
                ("s", builtin("xs:string")),
                ("V", reference("3a87ef2a-9de1-4d34-9e5f-3c8cdf53b3ab")),
            ])
            .as_deref(),
            Ok("V.s.O")
        );
    }

    /// Fail-closed floor: where a reference family stands relative to an
    /// unresolved type id nothing observed says, so a run whose storage order
    /// would have to decide it is refused.
    #[test]
    fn a_family_behind_an_unresolved_type_id_is_refused() {
        let error = rewritten(&[("O", TypeSortKey::Unevidenced), ("F", TypeSortKey::Family)])
            .expect_err("nothing places a family beside an unresolved type id");
        assert!(
            error.contains("reference family behind a type id"),
            "the refusal must name the undecided pair: {error}"
        );
    }

    /// The same pair in the other storage order decides nothing, because both
    /// candidate rules agree there, so it is left as it stands.
    #[test]
    fn a_family_ahead_of_an_unresolved_type_id_is_left_alone() {
        assert_eq!(
            rewritten(&[("F", TypeSortKey::Family), ("O", TypeSortKey::Unevidenced)]).as_deref(),
            Ok("F.O")
        );
    }

    /// An unplaced member on its own is still written: it is only its position
    /// among siblings that is unknown, not its spelling.
    #[test]
    fn a_lone_unevidenced_member_is_still_written() {
        assert_eq!(
            rewritten(&[("O", TypeSortKey::Unevidenced)]).as_deref(),
            Ok("O")
        );
    }

    /// The bounds table must stay a set of non-empty, ordered intervals: an
    /// inverted one would silently make every comparison inside it decide both
    /// ways.
    #[test]
    fn every_evidenced_interval_is_non_empty() {
        for (qname, lower, upper) in super::BUILTIN_TYPE_SORT_BOUNDS {
            if let (Some(lower), Some(upper)) = (lower, upper) {
                assert!(lower < upper, "{qname} has an inverted sort interval");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_base64_fixture(encoded: &str) -> Vec<u8> {
        let mut output = Vec::new();
        let mut quartet = [0u8; 4];
        let mut length = 0usize;
        for byte in encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            quartet[length] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => panic!("invalid base64 fixture"),
            };
            length += 1;
            if length == 4 {
                output.push((quartet[0] << 2) | (quartet[1] >> 4));
                if quartet[2] != 64 {
                    output.push((quartet[1] << 4) | (quartet[2] >> 2));
                }
                if quartet[3] != 64 {
                    output.push((quartet[2] << 6) | quartet[3]);
                }
                length = 0;
            }
        }
        assert_eq!(length, 0);
        output
    }

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

    /// The parameter-scalar-types cohort's manifest, like several other
    /// corpora in this batch, retains only the combined `raw-unpacked`
    /// envelope; slice the primary `SchemaFile` document from its
    /// length-prefixed header the same way `type_id_documents` does above.
    fn parameter_scalar_types_primary() -> Vec<u8> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        body[24..24 + first].to_vec()
    }

    fn parameter_scalar_types_native_source() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/native-template.xml.b64"
        )))
    }

    fn filter_primary() -> Vec<u8> {
        let native = include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-filter/native-template.xml.b64"
        ));
        let source = decode_base64_fixture(native);
        let compiled =
            crate::dcs_template::compile_dcs_schema_template_source_documents(&source).unwrap();
        compiled.primary_schema_file().to_vec()
    }

    fn type_id_documents() -> Vec<Vec<u8>> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        vec![
            body[24..24 + first].to_vec(),
            body[24 + first..24 + first + second].to_vec(),
            body[24 + first + second..].to_vec(),
        ]
    }

    fn query_union_link_documents() -> Vec<Vec<u8>> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        vec![
            body[24..24 + first].to_vec(),
            body[24 + first..24 + first + second].to_vec(),
            body[24 + first + second..].to_vec(),
        ]
    }

    fn link_parameter_documents() -> Vec<Vec<u8>> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        vec![
            body[24..24 + first].to_vec(),
            body[24 + first..24 + first + second].to_vec(),
            body[24 + first + second..].to_vec(),
        ]
    }

    fn link_expressions_documents() -> Vec<Vec<u8>> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        vec![
            body[24..24 + first].to_vec(),
            body[24 + first..24 + first + second].to_vec(),
            body[24 + first + second..].to_vec(),
        ]
    }

    fn query_union_link_typeid_documents() -> Vec<Vec<u8>> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        vec![
            body[24..24 + first].to_vec(),
            body[24 + first..24 + first + second].to_vec(),
            body[24 + first + second..].to_vec(),
        ]
    }

    fn area_template_document() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/area-schema-file.xml.b64"
        )))
    }

    fn area_template_appearance_document() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/area-schema-file.xml.b64"
        )))
    }

    /// The color cohort's manifest retains only the combined `raw-unpacked`
    /// envelope (no standalone `area-schema-file.xml.b64`), so the terminal
    /// side-table document is sliced from the length-prefixed header the
    /// same way `type_id_documents`/`query_union_link_documents` do above.
    fn area_template_web_color_document() -> Vec<u8> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        body[24 + first + second..].to_vec()
    }

    fn area_template_web_color_native_source() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )))
    }

    /// The style-color-reference cohort's manifest, like the web-color
    /// cohort's, retains only the combined `raw-unpacked` envelope; slice
    /// the terminal side-table document the same way.
    fn area_template_style_color_reference_document() -> Vec<u8> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        body[24 + first + second..].to_vec()
    }

    fn area_template_style_color_reference_native_source() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/native-template.xml.b64"
        )))
    }

    /// The custom-StyleItem cohort's manifest, like its siblings, retains
    /// only the combined `raw-unpacked` envelope; slice the terminal
    /// side-table document the same way.
    fn area_template_style_item_uuid_document() -> Vec<u8> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        body[24 + first + second..].to_vec()
    }

    fn area_template_style_item_uuid_native_source() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )))
    }

    /// The evidenced custom-StyleItem cohort's resolver map: the retained
    /// `native-style-item.xml`'s own `uuid="..."` attribute (see
    /// `manifest.json` `cohort.style_item_seed_uuid`), mapped to its
    /// semantic name `CorpusAccent`. A stand-in for what an adapter would
    /// build from live configuration metadata; this test never resolves
    /// the uuid by any means other than this explicit, evidence-derived
    /// map.
    fn style_item_reference_types() -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        map.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "CorpusAccent".to_string(),
        );
        map
    }

    /// Wraps a synthetic `dcsat:appearance` body (source direction, inside
    /// the inline `<template>`) in the minimal document shape
    /// `parse_dcs_area_template_source_document` accepts.
    fn area_template_document_with_appearance(appearance: &str) -> Vec<u8> {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<template><name>AreaProbe</name>\
<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">\
<dcsat:item xsi:type=\"dcsat:TableRow\"><dcsat:tableCell>\
<dcsat:item xsi:type=\"dcsat:Field\"><dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value></dcsat:item>\
<dcsat:appearance>{appearance}</dcsat:appearance>\
</dcsat:tableCell></dcsat:item></template>\
<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">\
<dcsat:name>Probe</dcsat:name><dcsat:expression>\"Probe\"</dcsat:expression></parameter>\
</template></DataCompositionSchema>"
        )
        .into_bytes()
    }

    const COLOR_ITEM_WEB_RED: &str = "<dcscor:item><dcscor:parameter>ЦветТекста</dcscor:parameter><dcscor:value xmlns:d8p1=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xsi:type=\"v8ui:Color\">d8p1:Red</dcscor:value></dcscor:item>";
    const PARAMETER_ITEM_PROBE: &str = "<dcscor:item><dcscor:parameter>Расшифровка</dcscor:parameter><dcscor:value xsi:type=\"dcscor:Parameter\">Probe</dcscor:value></dcscor:item>";

    /// The multi-cell-appearance cohort's manifest, like the color cohort's,
    /// retains only the combined `raw-unpacked` envelope; slice the terminal
    /// side-table document the same way.
    fn area_template_multi_cell_appearance_document() -> Vec<u8> {
        let body = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(body[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(body[16..24].try_into().unwrap()) as usize;
        body[24 + first + second..].to_vec()
    }

    fn area_template_multi_cell_appearance_native_source() -> Vec<u8> {
        decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )))
    }

    /// Wraps a synthetic two-row `<template xsi:type="dcsat:AreaTemplate">`
    /// body (source direction) in the minimal document shape
    /// `parse_dcs_area_template_source_document` accepts. `row1_cells` is
    /// the raw XML for row 1's `tableCell` elements (caller supplies as
    /// many/few/whatever-shaped cells as the test needs); row 2 is always
    /// the fixed one-cell-no-appearance shape.
    fn area_template_document_with_two_rows(row1_cells: &str) -> Vec<u8> {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<template><name>AreaProbe</name>\
<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">\
<dcsat:item xsi:type=\"dcsat:TableRow\">{row1_cells}</dcsat:item>\
<dcsat:item xsi:type=\"dcsat:TableRow\"><dcsat:tableCell>\
<dcsat:item xsi:type=\"dcsat:Field\"><dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value></dcsat:item>\
</dcsat:tableCell></dcsat:item></template>\
<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">\
<dcsat:name>Probe</dcsat:name><dcsat:expression>\"Probe\"</dcsat:expression></parameter>\
</template></DataCompositionSchema>"
        )
        .into_bytes()
    }

    const APPEARANCE_CELL_PROBE: &str = "<dcsat:tableCell><dcsat:item xsi:type=\"dcsat:Field\"><dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value></dcsat:item><dcsat:appearance><dcscor:item><dcscor:parameter>Расшифровка</dcscor:parameter><dcscor:value xsi:type=\"dcscor:Parameter\">Probe</dcscor:value></dcscor:item></dcsat:appearance></dcsat:tableCell>";
    const PLAIN_CELL_PROBE: &str = "<dcsat:tableCell><dcsat:item xsi:type=\"dcsat:Field\"><dcsat:value xsi:type=\"dcscor:Parameter\">Probe</dcsat:value></dcsat:item></dcsat:tableCell>";

    fn inline_settings(source: &str) -> DcsInlineSettingsFragment {
        let start = source.find("<dcsset:settings").unwrap();
        let close = "</dcsset:settings>";
        let end = start + source[start..].find(close).unwrap() + close.len();
        DcsInlineSettingsFragment::parse(source[start..end].to_owned()).unwrap()
    }

    /// The empty settings variant exactly as 1C:Enterprise 8.3.27.2214 writes
    /// it into a `DataCompositionSchema` source document (verified against
    /// `CommonTemplates/ДанныеПечатиРегистрСимволов/Ext/Template.xml` of an
    /// `ibcmd config export` capture of 1C:Trade Management 11.5.27.75).
    const EMPTY_INLINE_SETTINGS: &str = "<dcsset:settings xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"/>";

    #[test]
    fn empty_self_closing_inline_settings_keeps_declarations_inside_the_tag() {
        let policy = policy().unwrap();
        let closed = close_inline_settings_namespaces(EMPTY_INLINE_SETTINGS, &policy).unwrap();
        assert!(
            closed.ends_with("/>"),
            "self-closing marker must survive namespace closing: {closed}"
        );
        assert!(
            !closed.contains("/ xmlns:"),
            "declarations must not be spliced between `/` and `>`: {closed}"
        );
        DcsInlineSettingsFragment::parse(EMPTY_INLINE_SETTINGS.to_owned())
            .expect("the empty native settings variant is a well-formed analyzable fragment");
    }

    #[test]
    fn inline_settings_opening_tag_end_ignores_angle_brackets_inside_attribute_values() {
        let xml = "<dcsset:settings xmlns:probe=\"urn:a>b\" xmlns:other='urn:c>d'/>";
        assert_eq!(opening_tag_end(xml), Some(xml.len() - 1));
        let policy = policy().unwrap();
        let closed = close_inline_settings_namespaces(xml, &policy).unwrap();
        assert!(closed.ends_with("/>"), "{closed}");
        assert!(
            closed.starts_with("<dcsset:settings xmlns:probe="),
            "{closed}"
        );
        assert!(closed.contains("xmlns:dcsset="), "{closed}");
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
    fn platform_parameter_scalar_types_storage_parses_to_typed_ir_and_emits_exact_source() {
        let primary = parameter_scalar_types_primary();
        // document_topology (manifest.json): primary_schema_sha256
        // 66061d435748072b14f3bbc9a55e54d91b1e016831d574fc2732dd2cd53e99f6
        // (verified against this exact slice outside this crate, which has
        // no sha2 dependency; pinned sha256 checks for this corpus live in
        // src/compiler/bodies/dcs.rs and src/mssql_dump/dcs.rs instead).
        assert_eq!(primary.len(), 4743);

        let schema = parse_dcs_inner_schema_storage_document(
            &primary,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-parameter-scalar-types",
        )
        .unwrap();
        assert!(schema.parameter().is_some());
        let scalars = schema.scalar_parameters().unwrap();
        assert!(scalars.flag().value());
        assert_eq!(scalars.limit().value().as_str(), "100.5");
        assert_eq!(scalars.limit().value_type().digits(), 10);
        assert_eq!(scalars.limit().value_type().fraction_digits(), 2);
        assert_eq!(
            scalars.period().variant(),
            DcsSchemaStandardPeriodVariant::LastMonth
        );

        // raw-unpacked (primary slice) -> XML == native-template: the
        // codec-level equivalent of the manifest's own byte-for-byte
        // observation (native re-export equals the submitted seed here).
        // Unlike `core_source()` (a checked-in text file, line-ending
        // normalized by git), this native source is decoded directly from
        // the retained base64 fixture and keeps the platform's own CRLF,
        // matching what the emitter itself produces -- so no `\r\n`->`\n`
        // normalization is applied on either side.
        let source = String::from_utf8(parameter_scalar_types_native_source()).unwrap();
        let emitted = emit_dcs_inner_schema_source_document(
            &schema,
            &[inline_settings(source.trim_start_matches('\u{feff}'))],
        )
        .unwrap();
        let emitted = String::from_utf8(emitted).unwrap();
        assert_eq!(emitted.trim_end(), source.trim_end());
    }

    #[test]
    fn parameter_scalar_types_storage_rejects_unsupported_parameter_type() {
        let primary = parameter_scalar_types_primary();
        let text = String::from_utf8(primary).unwrap();
        let mutated = text.replacen(
            "<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">xs:boolean</Type>",
            "<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">xs:integer</Type>",
            1,
        );
        assert_ne!(mutated, text, "mutation must actually change the fixture");
        let error = parse_dcs_inner_schema_storage_document(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:parameter-scalar-types/unsupported-type",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn parameter_scalar_types_storage_rejects_unknown_period_variant() {
        let primary = parameter_scalar_types_primary();
        let text = String::from_utf8(primary).unwrap();
        let mutated = text.replacen(
            "<d4p1:variant xsi:type=\"d4p1:StandardPeriodVariant\">LastMonth</d4p1:variant>",
            "<d4p1:variant xsi:type=\"d4p1:StandardPeriodVariant\">ThisWeek</d4p1:variant>",
            1,
        );
        assert_ne!(mutated, text, "mutation must actually change the fixture");
        let error = parse_dcs_inner_schema_storage_document(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:parameter-scalar-types/unknown-variant",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn parameter_scalar_types_storage_rejects_value_not_matching_type() {
        let primary = parameter_scalar_types_primary();
        let text = String::from_utf8(primary).unwrap();
        let mutated = text.replacen(
            "<value xsi:type=\"xs:boolean\">true</value>",
            "<value xsi:type=\"xs:boolean\">1</value>",
            1,
        );
        assert_ne!(mutated, text, "mutation must actually change the fixture");
        let error = parse_dcs_inner_schema_storage_document(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:parameter-scalar-types/value-type-mismatch",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn parameter_scalar_types_storage_rejects_corrupted_bytes() {
        let primary = parameter_scalar_types_primary();
        let corrupted = &primary[..primary.len() / 2];
        let error = parse_dcs_inner_schema_storage_document(
            corrupted,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:parameter-scalar-types/corrupted",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_)
        ));
    }

    #[test]
    fn platform_area_appearance_side_table_parses_and_emits_exact_documents() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/native-template.xml.b64"
        )));
        let area = parse_dcs_area_template_storage_document(
            &area_template_appearance_document(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-template-appearance",
        )
        .unwrap();
        assert!(area.has_parameter_appearance());
        assert_eq!(
            emit_dcs_area_template_storage_document(&area).unwrap(),
            area_template_appearance_document()
        );
        let parsed_source = parse_dcs_area_template_source_document(
            &source,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:dcs-area-template-appearance/source",
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed_source.name(), area.name());
        assert_eq!(parsed_source.parameter_name(), area.parameter_name());
        assert_eq!(parsed_source.expression(), area.expression());
        assert_eq!(
            parsed_source.has_parameter_appearance(),
            area.has_parameter_appearance()
        );
    }

    #[test]
    fn platform_area_appearance_web_color_side_table_parses_and_emits_exact_documents() {
        let source = area_template_web_color_native_source();
        let storage = area_template_web_color_document();

        let area = parse_dcs_area_template_storage_document(
            &storage,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-appearance-web-color",
        )
        .unwrap();
        assert!(area.has_parameter_appearance());
        assert_eq!(
            area.text_color_appearance(),
            Some(DcsAppearanceColor::WebRed)
        );
        // Storage direction: raw side-table bytes -> IR -> byte-exact re-emit.
        assert_eq!(
            emit_dcs_area_template_storage_document(&area).unwrap(),
            storage
        );

        let parsed_source = parse_dcs_area_template_source_document(
            &source,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:dcs-area-appearance-web-color/source",
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed_source.name(), area.name());
        assert_eq!(parsed_source.parameter_name(), area.parameter_name());
        assert_eq!(parsed_source.expression(), area.expression());
        assert_eq!(
            parsed_source.text_color_appearance(),
            area.text_color_appearance()
        );
        // Source direction: native XML -> IR -> byte-exact re-emit of the
        // inline `<template>` fragment (the same IR the storage side
        // produced), proving one shared IR drives both directions.
        let fragment = emit_dcs_area_template_source_fragment(&parsed_source).unwrap();
        let fragment = std::str::from_utf8(&fragment).unwrap();
        assert!(fragment.contains("<dcscor:parameter>ЦветТекста</dcscor:parameter>"));
        assert!(fragment.contains("d8p1:Red"));
        let color_at = fragment.find("ЦветТекста").unwrap();
        let details_at = fragment.find("Расшифровка").unwrap();
        assert!(color_at < details_at, "color item must precede Расшифровка");
    }

    #[test]
    fn platform_area_style_color_reference_side_table_parses_and_emits_exact_documents() {
        let source = area_template_style_color_reference_native_source();
        let storage = area_template_style_color_reference_document();

        // Standard/built-in style reference: no resolver needed on either
        // direction, so the plain (non-`_with_references`) entry points
        // must already round-trip byte-exact.
        let area = parse_dcs_area_template_storage_document(
            &storage,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-style-color-reference",
        )
        .unwrap();
        assert!(area.has_parameter_appearance());
        assert_eq!(area.text_color_appearance(), None);
        assert_eq!(
            area.back_color_style_reference(),
            Some(&DcsStyleColorReference::Named(
                CanonicalText::new("NegativeTextColor").unwrap()
            ))
        );
        // Storage direction: raw side-table bytes -> IR -> byte-exact re-emit.
        assert_eq!(
            emit_dcs_area_template_storage_document(&area).unwrap(),
            storage
        );

        let parsed_source = parse_dcs_area_template_source_document(
            &source,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:dcs-area-style-color-reference/source",
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed_source.name(), area.name());
        assert_eq!(
            parsed_source.back_color_style_reference(),
            area.back_color_style_reference()
        );
        // Source direction: native XML -> IR -> byte-exact re-emit of the
        // inline `<template>` fragment (the same IR the storage side
        // produced), proving one shared IR drives both directions.
        let fragment = emit_dcs_area_template_source_fragment(&parsed_source).unwrap();
        let fragment = std::str::from_utf8(&fragment).unwrap();
        assert!(fragment.contains("<dcscor:parameter>ЦветФона</dcscor:parameter>"));
        assert!(fragment.contains("d8p1:NegativeTextColor"));
        let color_at = fragment.find("ЦветФона").unwrap();
        let details_at = fragment.find("Расшифровка").unwrap();
        assert!(
            color_at < details_at,
            "style-reference item must precede Расшифровка"
        );
    }

    #[test]
    fn platform_area_style_item_uuid_side_table_parses_and_emits_exact_documents() {
        let source = area_template_style_item_uuid_native_source();
        let storage = area_template_style_item_uuid_document();
        let reference_types = style_item_reference_types();

        // Custom StyleItem reference: the raw uuid storage form requires a
        // resolver on decode (uuid -> semantic name) and on re-encode
        // (semantic name -> uuid, reverse lookup on the same map).
        let area = parse_dcs_area_template_storage_document_with_references(
            &storage,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-style-item-uuid",
            &reference_types,
        )
        .unwrap();
        assert!(area.has_parameter_appearance());
        assert_eq!(area.text_color_appearance(), None);
        assert_eq!(
            area.back_color_style_reference(),
            Some(&DcsStyleColorReference::CustomStyleItem(
                CanonicalText::new("CorpusAccent").unwrap()
            ))
        );
        // Without a resolver, decode must fail closed rather than silently
        // dropping the uuid form or guessing a name.
        assert!(
            parse_dcs_area_template_storage_document(
                &storage,
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:dcs-area-style-item-uuid/no-resolver",
            )
            .is_err()
        );
        // Storage direction: raw side-table bytes -> IR -> byte-exact
        // re-emit, resolving the semantic name back to the same uuid.
        assert_eq!(
            emit_dcs_area_template_storage_document_with_references(&area, &reference_types)
                .unwrap(),
            storage
        );
        // Without a resolver, re-emitting the custom-StyleItem form must
        // also fail closed rather than fabricating a uuid.
        assert!(emit_dcs_area_template_storage_document(&area).is_err());

        let parsed_source = parse_dcs_area_template_source_document(
            &source,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:dcs-area-style-item-uuid/source",
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed_source.name(), area.name());
        assert_eq!(
            parsed_source.back_color_style_reference(),
            area.back_color_style_reference()
        );
        // Source direction: native XML -> IR -> byte-exact re-emit of the
        // inline `<template>` fragment. No resolver is needed here: both
        // style-reference forms are lexically identical at the source
        // layer, so the plain emitter suffices even for the custom form.
        let fragment = emit_dcs_area_template_source_fragment(&parsed_source).unwrap();
        let fragment = std::str::from_utf8(&fragment).unwrap();
        assert!(fragment.contains("<dcscor:parameter>ЦветФона</dcscor:parameter>"));
        assert!(fragment.contains("d8p1:CorpusAccent"));
        let color_at = fragment.find("ЦветФона").unwrap();
        let details_at = fragment.find("Расшифровка").unwrap();
        assert!(
            color_at < details_at,
            "style-reference item must precede Расшифровка"
        );
    }

    #[test]
    fn area_appearance_source_accepts_any_prefix_bound_to_web_namespace() {
        // The seed used a locally-declared `web` prefix rather than the
        // platform's auto-generated `d8p1`; both must parse identically
        // because only the expanded QName is authenticated, never the
        // lexical prefix spelling.
        let appearance = format!(
            "<dcscor:item><dcscor:parameter>ЦветТекста</dcscor:parameter><dcscor:value xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xsi:type=\"v8ui:Color\">web:Red</dcscor:value></dcscor:item>{PARAMETER_ITEM_PROBE}"
        );
        let document = area_template_document_with_appearance(&appearance);
        let area = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-appearance-web-color/any-prefix",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            area.text_color_appearance(),
            Some(DcsAppearanceColor::WebRed)
        );
    }

    #[test]
    fn area_appearance_source_rejects_color_from_unknown_namespace() {
        let appearance = format!(
            "<dcscor:item><dcscor:parameter>ЦветТекста</dcscor:parameter><dcscor:value xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xsi:type=\"v8ui:Color\">win:Red</dcscor:value></dcscor:item>{PARAMETER_ITEM_PROBE}"
        );
        let document = area_template_document_with_appearance(&appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-appearance-web-color/unknown-namespace",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_appearance_source_rejects_more_than_one_color_item() {
        let appearance = format!("{COLOR_ITEM_WEB_RED}{COLOR_ITEM_WEB_RED}{PARAMETER_ITEM_PROBE}");
        let document = area_template_document_with_appearance(&appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-appearance-web-color/too-many-color-items",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_appearance_source_rejects_color_out_of_position() {
        // The evidenced order is color-then-parameter; the reverse (as the
        // non-authoritative seed happened to spell it) must be rejected,
        // not silently reordered or accepted.
        let appearance = format!("{PARAMETER_ITEM_PROBE}{COLOR_ITEM_WEB_RED}");
        let document = area_template_document_with_appearance(&appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-appearance-web-color/wrong-position",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_appearance_source_rejects_unknown_parameter_name() {
        let appearance = "<dcscor:item><dcscor:parameter>НеизвестныйПараметр</dcscor:parameter><dcscor:value xsi:type=\"dcscor:Parameter\">Probe</dcscor:value></dcscor:item>";
        let document = area_template_document_with_appearance(appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-appearance-web-color/unknown-parameter",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_appearance_storage_rejects_corrupted_side_table_bytes() {
        let storage = area_template_web_color_document();
        // Truncate mid-document: still starts as plausible XML but never
        // closes, so this must fail closed with a typed parse error rather
        // than panicking or silently returning a partial/absent result.
        let corrupted = &storage[..storage.len() / 2];
        let error = parse_dcs_area_template_storage_document(
            corrupted,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-appearance-web-color/corrupted",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_)
        ));
    }

    #[test]
    fn area_style_reference_source_rejects_unknown_style_name() {
        // Neither evidenced literal ("NegativeTextColor"/"CorpusAccent");
        // an unrelated style name in the same namespace is outside the
        // cohort and must not be silently accepted as either form.
        let appearance = format!(
            "<dcscor:item><dcscor:parameter>ЦветФона</dcscor:parameter><dcscor:value xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"v8ui:Color\">style:PositiveTextColor</dcscor:value></dcscor:item>{PARAMETER_ITEM_PROBE}"
        );
        let document = area_template_document_with_appearance(&appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-style-reference/unknown-style-name",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_style_reference_storage_rejects_unknown_uuid() {
        // A uuid absent from the supplied resolver map must fail closed
        // rather than being silently dropped, treated as absent, or
        // guessed at by any means other than the map.
        let storage = area_template_style_item_uuid_document();
        let storage_text = std::str::from_utf8(&storage).unwrap();
        let mutated = storage_text.replacen(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c",
            "00000000-0000-0000-0000-000000000000",
            1,
        );
        assert_ne!(
            mutated, storage_text,
            "mutation must actually change the fixture"
        );
        let error = parse_dcs_area_template_storage_document_with_references(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-style-reference/unknown-uuid",
            &style_item_reference_types(),
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_style_reference_source_rejects_raw_uuid_form() {
        // The raw `0:<uuid>` wire form is only evidenced on the storage
        // direction; the source direction always spells a style reference
        // by name, even for a custom StyleItem.
        let appearance = format!(
            "<dcscor:item><dcscor:parameter>ЦветФона</dcscor:parameter><dcscor:value xsi:type=\"v8ui:Color\">0:4a9d8536-ff59-4a90-a1cf-646d241dc53c</dcscor:value></dcscor:item>{PARAMETER_ITEM_PROBE}"
        );
        let document = area_template_document_with_appearance(&appearance);
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-style-reference/raw-uuid-on-source",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_style_reference_storage_rejects_named_form_for_custom_style_item() {
        // The named `d4p2:CorpusAccent` spelling is the source-direction
        // form; storage always spells a custom StyleItem reference as the
        // raw uuid form instead. Mixing the two forms across directions
        // must fail closed, not be silently accepted as the standard form.
        let storage = area_template_style_item_uuid_document();
        let storage_text = std::str::from_utf8(&storage).unwrap();
        let mutated = storage_text.replacen(
            "<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\" xsi:type=\"d4p1:Color\">0:4a9d8536-ff59-4a90-a1cf-646d241dc53c</value>",
            "<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\" xmlns:d4p2=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"d4p1:Color\">d4p2:CorpusAccent</value>",
            1,
        );
        assert_ne!(
            mutated, storage_text,
            "mutation must actually change the fixture"
        );
        let error = parse_dcs_area_template_storage_document_with_references(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-style-reference/named-form-on-storage",
            &style_item_reference_types(),
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_style_reference_rejects_uuid_resolving_to_unevidenced_name() {
        // The resolver map's own semantics can still drift: a uuid that
        // resolves to some other name (mixing an unrelated configuration
        // object's identity into this cohort) must fail closed rather than
        // being accepted as if it resolved to the evidenced CorpusAccent.
        let storage = area_template_style_item_uuid_document();
        let mut drifted_references = BTreeMap::new();
        drifted_references.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "SomeOtherStyleItem".to_string(),
        );
        let error = parse_dcs_area_template_storage_document_with_references(
            &storage,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-style-reference/drifted-resolution",
            &drifted_references,
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_style_color_reference_storage_rejects_corrupted_side_table_bytes() {
        let storage = area_template_style_color_reference_document();
        let corrupted = &storage[..storage.len() / 2];
        let error = parse_dcs_area_template_storage_document(
            corrupted,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-style-color-reference/corrupted",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_)
        ));
    }

    #[test]
    fn area_style_item_uuid_storage_rejects_corrupted_side_table_bytes() {
        let storage = area_template_style_item_uuid_document();
        let corrupted = &storage[..storage.len() / 2];
        let error = parse_dcs_area_template_storage_document_with_references(
            corrupted,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-style-item-uuid/corrupted",
            &style_item_reference_types(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_)
        ));
    }

    #[test]
    fn platform_multi_cell_appearance_side_table_parses_and_emits_exact_documents() {
        let source = area_template_multi_cell_appearance_native_source();
        let storage = area_template_multi_cell_appearance_document();

        let area = parse_dcs_area_template_storage_document(
            &storage,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-multi-cell-appearance",
        )
        .unwrap();
        // Resolves the manifest's own open question directly from the
        // bytes: both row-1 cells carry `appIndex=0` and there is exactly
        // one side-table `<appearance>` record -- a SHARED index, not two
        // duplicated records.
        assert!(area.has_shared_row_appearance());
        assert!(!area.has_parameter_appearance());
        assert_eq!(area.text_color_appearance(), None);
        assert_eq!(
            emit_dcs_area_template_storage_document(&area).unwrap(),
            storage
        );

        let parsed_source = parse_dcs_area_template_source_document(
            &source,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:dcs-area-multi-cell-appearance/source",
        )
        .unwrap()
        .unwrap();
        assert_eq!(parsed_source.name(), area.name());
        assert_eq!(parsed_source.parameter_name(), area.parameter_name());
        assert_eq!(parsed_source.expression(), area.expression());
        assert!(parsed_source.has_shared_row_appearance());
        let fragment = emit_dcs_area_template_source_fragment(&parsed_source).unwrap();
        let fragment = std::str::from_utf8(&fragment).unwrap();
        assert_eq!(
            fragment
                .matches("<dcsat:item xsi:type=\"dcsat:TableRow\">")
                .count(),
            2
        );
        assert_eq!(fragment.matches("<dcsat:tableCell>").count(), 3);
        assert_eq!(fragment.matches("<dcsat:appearance>").count(), 2);
        assert_eq!(fragment.matches("Расшифровка").count(), 2);
    }

    #[test]
    fn area_multi_cell_appearance_seed_order_is_rejected_fail_closed() {
        // The non-authoritative seed spells each row-1 cell
        // appearance-before-Field (the reverse of what native output and
        // storage always canonicalize to); it must be rejected, not
        // silently reordered or accepted.
        let seed = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/seed/Template.xml"
        ));
        let error = parse_dcs_area_template_source_document(
            seed,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-multi-cell-appearance/seed-order",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_multi_cell_appearance_source_rejects_divergent_row1_cells() {
        // One cell has the appearance, the other doesn't -- the manifest's
        // admitted cohort is exactly two IDENTICAL appearance blocks; any
        // divergence is outside it.
        let document = area_template_document_with_two_rows(&format!(
            "{APPEARANCE_CELL_PROBE}{PLAIN_CELL_PROBE}"
        ));
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-multi-cell-appearance/divergent-cells",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_multi_cell_appearance_source_rejects_unsupported_row1_cell_counts() {
        for row1 in [
            APPEARANCE_CELL_PROBE.to_string(),
            format!("{APPEARANCE_CELL_PROBE}{APPEARANCE_CELL_PROBE}{APPEARANCE_CELL_PROBE}"),
        ] {
            let document = area_template_document_with_two_rows(&row1);
            let error = parse_dcs_area_template_source_document(
                &document,
                ProfileId::parse("source:designer-xml-2.20").unwrap(),
                "fixture:area-multi-cell-appearance/row1-cardinality",
            )
            .unwrap_err();
            assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
        }
    }

    #[test]
    fn area_multi_cell_appearance_source_rejects_more_than_two_rows() {
        let document = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<template><name>AreaProbe</name>\
<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">\
<dcsat:item xsi:type=\"dcsat:TableRow\">{APPEARANCE_CELL_PROBE}{APPEARANCE_CELL_PROBE}</dcsat:item>\
<dcsat:item xsi:type=\"dcsat:TableRow\">{PLAIN_CELL_PROBE}</dcsat:item>\
<dcsat:item xsi:type=\"dcsat:TableRow\">{PLAIN_CELL_PROBE}</dcsat:item>\
</template>\
<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">\
<dcsat:name>Probe</dcsat:name><dcsat:expression>\"Probe\"</dcsat:expression></parameter>\
</template></DataCompositionSchema>"
        )
        .into_bytes();
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-multi-cell-appearance/too-many-rows",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_multi_cell_appearance_source_rejects_appearance_on_row2() {
        let document = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<template><name>AreaProbe</name>\
<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">\
<dcsat:item xsi:type=\"dcsat:TableRow\">{APPEARANCE_CELL_PROBE}{APPEARANCE_CELL_PROBE}</dcsat:item>\
<dcsat:item xsi:type=\"dcsat:TableRow\">{APPEARANCE_CELL_PROBE}</dcsat:item>\
</template>\
<parameter xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:ExpressionAreaTemplateParameter\">\
<dcsat:name>Probe</dcsat:name><dcsat:expression>\"Probe\"</dcsat:expression></parameter>\
</template></DataCompositionSchema>"
        )
        .into_bytes();
        let error = parse_dcs_area_template_source_document(
            &document,
            ProfileId::parse("source:designer-xml-2.20").unwrap(),
            "fixture:area-multi-cell-appearance/row2-has-appearance",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_multi_cell_appearance_storage_rejects_mismatched_app_index() {
        // Both row-1 cells are evidenced to share appIndex 0; a divergent
        // index (or a missing side-table match) is outside the cohort.
        let storage = area_template_multi_cell_appearance_document();
        let storage_text = std::str::from_utf8(&storage).unwrap();
        let mutated = storage_text.replacen(
            "<dcsat:appIndex>0</dcsat:appIndex>\r\n\t\t\t\t\t</dcsat:tableCell>\r\n\t\t\t\t\t<dcsat:tableCell>",
            "<dcsat:appIndex>1</dcsat:appIndex>\r\n\t\t\t\t\t</dcsat:tableCell>\r\n\t\t\t\t\t<dcsat:tableCell>",
            1,
        );
        assert_ne!(
            mutated, storage_text,
            "mutation must actually change the fixture"
        );
        let error = parse_dcs_area_template_storage_document(
            mutated.as_bytes(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-multi-cell-appearance/mismatched-index",
        )
        .unwrap_err();
        assert!(matches!(error, DcsInnerSchemaError::UnsupportedSource(_)));
    }

    #[test]
    fn area_multi_cell_appearance_storage_rejects_corrupted_side_table_bytes() {
        let storage = area_template_multi_cell_appearance_document();
        let corrupted = &storage[..storage.len() / 2];
        let error = parse_dcs_area_template_storage_document(
            corrupted,
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:area-multi-cell-appearance/corrupted",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_)
        ));
    }

    #[test]
    fn platform_filter_simple_schema_parses_and_emits_exact_source() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-filter/native-template.xml.b64"
        )));
        let source_text = String::from_utf8(source.clone()).unwrap();
        let schema = parse_dcs_inner_schema_storage_document(
            &filter_primary(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-filter",
        )
        .unwrap();
        assert_eq!(schema.data_set().fields().len(), 1);
        assert!(schema.calculated_field().is_none());
        assert!(schema.total_fields().is_empty());
        assert!(schema.parameter().is_none());
        let emitted = emit_dcs_inner_schema_source_document(
            &schema,
            &[inline_settings(source_text.trim_start_matches('\u{feff}'))],
        )
        .unwrap();
        assert_eq!(emitted, source);
    }

    #[test]
    fn platform_type_id_reference_resolves_to_semantic_qname_and_emits_exact_source() {
        let documents = type_id_documents();
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/native-template.xml.b64"
        )));
        let mut references = BTreeMap::new();
        references.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            "CatalogRef.FilterProbe".to_owned(),
        );
        let schema = parse_dcs_inner_schema_storage_document_with_references(
            &documents[0],
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-typeid-reference",
            &references,
        )
        .unwrap();
        assert!(matches!(
            schema.data_set().fields()[1].value_type(),
            DcsSchemaFieldType::Reference(reference)
                if reference.qualified_name().as_str() == "CatalogRef.FilterProbe"
        ));
        let expected_text = std::str::from_utf8(&expected).unwrap();
        let emitted = emit_dcs_inner_schema_source_document(
            &schema,
            &[inline_settings(
                expected_text.trim_start_matches('\u{feff}'),
            )],
        )
        .unwrap();
        assert_eq!(emitted, expected);
    }

    #[test]
    fn platform_query_union_link_parses_and_emits_exact_source() {
        let documents = query_union_link_documents();
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let schema = parse_dcs_query_union_link_storage_document(
            &documents[0],
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-query-union-link",
        )
        .unwrap();
        assert_eq!(schema.query().name().as_str(), "QueryRows");
        assert_eq!(schema.union().name().as_str(), "UnionRows");
        let expected_text = std::str::from_utf8(&expected).unwrap();
        let emitted = emit_dcs_query_union_link_source_document(
            &schema,
            &[inline_settings(
                expected_text.trim_start_matches('\u{feff}'),
            )],
        )
        .unwrap();
        assert_eq!(emitted, expected);
    }

    /// The second evidenced `DataSetQuery` shape (`dcs-query-union-link-typeid`):
    /// `QueryRows` carries a second, typed field (`Owner`) transplanting the
    /// exact evidenced current-config TypeId construction
    /// `dcs-typeid-reference`'s DataSetObject field already proved, resolved
    /// through the same `reference_types` mechanism. The `DataSetUnion`
    /// item stays single-field (no evidence for a second field there).
    #[test]
    fn platform_query_union_link_typeid_resolves_second_field_and_emits_exact_source() {
        let documents = query_union_link_typeid_documents();
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/native-template.xml.b64"
        )));
        let mut references = BTreeMap::new();
        references.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            "CatalogRef.FilterProbe".to_owned(),
        );
        let schema = parse_dcs_query_union_link_storage_document_with_references(
            &documents[0],
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-query-union-link-typeid",
            &references,
        )
        .unwrap();
        assert_eq!(schema.query().name().as_str(), "QueryRows");
        assert_eq!(schema.union().name().as_str(), "UnionRows");
        assert!(matches!(
            schema.query().typed_field().unwrap().value_type(),
            DcsSchemaFieldType::Reference(reference)
                if reference.qualified_name().as_str() == "CatalogRef.FilterProbe"
        ));
        assert!(schema.union().item().typed_field().is_none());

        // Without the reference: fails closed, exactly as
        // `parse_dcs_query_union_link_storage_document` (the plain,
        // empty-map wrapper) does.
        assert!(matches!(
            parse_dcs_query_union_link_storage_document(
                &documents[0],
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:dcs-query-union-link-typeid",
            ),
            Err(DcsInnerSchemaError::UnsupportedSource(_))
        ));

        let expected_text = std::str::from_utf8(&expected).unwrap();
        let emitted = emit_dcs_query_union_link_source_document(
            &schema,
            &[inline_settings(
                expected_text.trim_start_matches('\u{feff}'),
            )],
        )
        .unwrap();
        assert_eq!(emitted, expected);
    }

    #[test]
    fn platform_link_parameter_parses_and_emits_exact_source() {
        let documents = link_parameter_documents();
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/native-template.xml.b64"
        )));
        let schema = parse_dcs_query_union_link_storage_document(
            &documents[0],
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-link-parameter",
        )
        .unwrap();
        assert_eq!(schema.link().parameter().unwrap().as_str(), "LinkParam");
        assert_eq!(schema.link().parameter_list_allowed(), Some(true));
        assert_eq!(schema.link().link_condition_expression(), None);
        assert_eq!(schema.link().start_expression(), None);
        assert_eq!(schema.link().required(), None);
        let expected_text = std::str::from_utf8(&expected).unwrap();
        let emitted = emit_dcs_query_union_link_source_document(
            &schema,
            &[inline_settings(
                expected_text.trim_start_matches('\u{feff}'),
            )],
        )
        .unwrap();
        assert_eq!(emitted, expected);
    }

    #[test]
    fn platform_link_expressions_parses_and_emits_exact_source() {
        let documents = link_expressions_documents();
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/native-template.xml.b64"
        )));
        let schema = parse_dcs_query_union_link_storage_document(
            &documents[0],
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-link-expressions",
        )
        .unwrap();
        assert_eq!(schema.link().parameter().unwrap().as_str(), "LinkParam");
        assert_eq!(schema.link().parameter_list_allowed(), Some(true));
        assert_eq!(
            schema.link().link_condition_expression().unwrap().as_str(),
            "SortKey > 0"
        );
        assert_eq!(
            schema.link().start_expression().unwrap().as_str(),
            "SortKey"
        );
        // Non-default (EDT defaultValue "true") retained verbatim.
        assert_eq!(schema.link().required(), Some(false));
        let expected_text = std::str::from_utf8(&expected).unwrap();
        // Confirmed directly against the retained storage bytes (not just
        // the native re-export): the platform already canonicalizes to
        // linkConditionExpression, startExpression, required at the
        // *storage* layer itself, even though the seed submitted
        // required, startExpression, linkConditionExpression -- the
        // typed IR round-trips straight to the exact native byte sequence
        // in this same canonical order both directions.
        let emitted = emit_dcs_query_union_link_source_document(
            &schema,
            &[inline_settings(
                expected_text.trim_start_matches('\u{feff}'),
            )],
        )
        .unwrap();
        assert_eq!(emitted, expected);
    }

    #[test]
    fn query_union_link_rejects_unattested_query_and_field_values() {
        let documents = query_union_link_documents();
        let primary = String::from_utf8(documents[0].clone()).unwrap();
        for drifted in [
            primary.replacen("ВЫБРАТЬ \"A\" КАК SortKey", "ВЫБРАТЬ 1", 1),
            primary.replacen(">SortKey<", ">Other<", 1),
        ] {
            assert!(matches!(
                parse_dcs_query_union_link_storage_document(
                    drifted.as_bytes(),
                    ProfileId::parse("provider:mssql-legacy").unwrap(),
                    "fixture:drift"
                ),
                Err(DcsInnerSchemaError::UnsupportedSource(_))
            ));
        }
    }

    #[test]
    fn data_set_link_rejects_unknown_child() {
        let documents = link_expressions_documents();
        let primary = String::from_utf8(documents[0].clone()).unwrap();
        // Renames one of the nine evidenced children to an unrecognized
        // name, keeping cardinality at 9: `exact_children`'s positional
        // name check must reject it, not silently accept an unknown tag
        // in an otherwise-plausible position.
        let mutated = primary.replacen(
            "<required>false</required>",
            "<requiredFlag>false</requiredFlag>",
            1,
        );
        assert_ne!(
            mutated, primary,
            "mutation must actually change the fixture"
        );
        assert!(matches!(
            parse_dcs_query_union_link_storage_document(
                mutated.as_bytes(),
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:link-unknown-child",
            ),
            Err(DcsInnerSchemaError::UnsupportedSource(_))
        ));
    }

    #[test]
    fn data_set_link_rejects_wrong_order() {
        let documents = link_expressions_documents();
        let primary = String::from_utf8(documents[0].clone()).unwrap();
        // Swaps startExpression and required -- adjacent in the evidenced
        // canonical order -- to the reverse; `exact_children`'s positional
        // check must reject this, not accept any permutation of the same
        // element set.
        let mutated = primary.replacen(
            "<startExpression>SortKey</startExpression>\r\n\t\t\t<required>false</required>",
            "<required>false</required>\r\n\t\t\t<startExpression>SortKey</startExpression>",
            1,
        );
        assert_ne!(
            mutated, primary,
            "mutation must actually change the fixture"
        );
        assert!(matches!(
            parse_dcs_query_union_link_storage_document(
                mutated.as_bytes(),
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:link-wrong-order",
            ),
            Err(DcsInnerSchemaError::UnsupportedSource(_))
        ));
    }

    #[test]
    fn data_set_link_rejects_duplicate_field() {
        let documents = link_expressions_documents();
        let primary = String::from_utf8(documents[0].clone()).unwrap();
        // Replaces linkConditionExpression with a second `required`,
        // duplicating a field while keeping cardinality at 9: the
        // positional name check must reject the duplicate in
        // linkConditionExpression's evidenced slot, not silently accept it
        // as if it were that field.
        let mutated = primary.replacen(
            "<linkConditionExpression>SortKey &gt; 0</linkConditionExpression>",
            "<required>false</required>",
            1,
        );
        assert_ne!(
            mutated, primary,
            "mutation must actually change the fixture"
        );
        assert!(matches!(
            parse_dcs_query_union_link_storage_document(
                mutated.as_bytes(),
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:link-duplicate-field",
            ),
            Err(DcsInnerSchemaError::UnsupportedSource(_))
        ));
    }

    #[test]
    fn data_set_link_rejects_non_boolean_in_boolean_field() {
        let documents = link_expressions_documents();
        let primary = String::from_utf8(documents[0].clone()).unwrap();
        for mutated in [
            primary.replacen("<required>false</required>", "<required>0</required>", 1),
            primary.replacen(
                "<required>false</required>",
                "<required>False</required>",
                1,
            ),
            primary.replacen(
                "<parameterListAllowed>true</parameterListAllowed>",
                "<parameterListAllowed>1</parameterListAllowed>",
                1,
            ),
        ] {
            assert_ne!(
                mutated, primary,
                "mutation must actually change the fixture"
            );
            assert!(matches!(
                parse_dcs_query_union_link_storage_document(
                    mutated.as_bytes(),
                    ProfileId::parse("provider:mssql-legacy").unwrap(),
                    "fixture:link-non-boolean",
                ),
                Err(DcsInnerSchemaError::UnsupportedSource(_))
            ));
        }
    }

    #[test]
    fn data_set_link_storage_rejects_corrupted_bytes() {
        let documents = link_expressions_documents();
        let corrupted = &documents[0][..documents[0].len() / 2];
        assert!(matches!(
            parse_dcs_query_union_link_storage_document(
                corrupted,
                ProfileId::parse("provider:mssql-legacy").unwrap(),
                "fixture:link-corrupted",
            ),
            Err(DcsInnerSchemaError::Malformed(_) | DcsInnerSchemaError::UnsupportedSource(_))
        ));
    }

    #[test]
    fn platform_style_free_area_template_parses_and_emits_exact_fragment() {
        let area = parse_dcs_area_template_storage_document(
            &area_template_document(),
            ProfileId::parse("provider:mssql-legacy").unwrap(),
            "fixture:dcs-area-template",
        )
        .unwrap();
        assert_eq!(area.name().as_str(), "AreaProbe");
        let emitted = emit_dcs_area_template_source_fragment(&area).unwrap();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/native-template.xml.b64"
        )));
        let source = std::str::from_utf8(&source).unwrap();
        let expected_start = source
            .find("\t<template>\r\n\t\t<name>AreaProbe</name>")
            .unwrap();
        let expected_end = source[expected_start..]
            .find("\r\n\t</template>")
            .map(|offset| expected_start + offset + "\r\n\t</template>".len())
            .unwrap();
        assert_eq!(
            std::str::from_utf8(&emitted).unwrap(),
            &source[expected_start..expected_end]
        );
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
