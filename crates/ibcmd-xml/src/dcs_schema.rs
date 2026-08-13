//! Evidence-bounded codec for the first typed inner DCS schema cohort.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::dcs::DcsAppearanceColor;
use ibcmd_core::dcs_schema::{
    DcsSchema, DcsSchemaAreaTemplate, DcsSchemaBuildError, DcsSchemaCalculatedField,
    DcsSchemaDataSetField, DcsSchemaDataSetLink, DcsSchemaDataSetObject, DcsSchemaDecimalType,
    DcsSchemaFieldType, DcsSchemaLocalDataSource, DcsSchemaLocalString, DcsSchemaQueryDataSet,
    DcsSchemaQueryField, DcsSchemaQueryUnionLink, DcsSchemaReferenceType,
    DcsSchemaSettingsVariantShell, DcsSchemaStringParameter, DcsSchemaStringType,
    DcsSchemaTotalFunction, DcsSchemaUngroupedTotalField, DcsSchemaUnionDataSet,
};
use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::value::CanonicalText;
use ibcmd_schema::{
    DcsAreaTemplatePolicy, DcsInnerSchemaPolicy, bundled_dcs_area_template_policy,
    bundled_dcs_inner_schema_policy, bundled_dcs_query_union_link_policy,
};
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
    match (calculated, parameter) {
        (Some(calculated), Some(parameter)) => DcsSchema::new(
            data_source,
            data_set,
            calculated,
            totals,
            parameter,
            variants,
            provenance,
        ),
        (None, None) if totals.is_empty() => {
            DcsSchema::new_simple(data_source, data_set, variants, provenance)
        }
        _ => return unsupported("inner schema mixes simple and rich cohort members"),
    }
    .map_err(DcsInnerSchemaError::Build)
}

/// Parses the exact one-Query/one-Union/one-link storage cohort.
pub fn parse_dcs_query_union_link_storage_document(
    bytes: &[u8],
    source_profile: ProfileId,
    locator: &str,
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
    let query = parse_query(children[1], &p, &qp, false)?;
    let union = parse_union(children[2], &p, &qp)?;
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
            require_storage_shared_row_appearance(wrapper[1], &p, &ap)?;
            area
        } else if area.has_parameter_appearance() {
            match require_storage_area_appearance(wrapper[1], &p, &ap)? {
                Some(color) => area.with_color_and_parameter_appearance(color),
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
            color: Some(color),
        } => area.with_color_and_parameter_appearance(color),
        ParsedAreaBody::SingleCell {
            has_appearance: true,
            color: None,
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
/// appearance/color), or the evidenced two-row shared-appearance shape.
enum ParsedAreaBody {
    SingleCell {
        has_appearance: bool,
        color: Option<DcsAppearanceColor>,
    },
    SharedRowAppearance,
}

/// One `tableCell`'s parsed appearance signal: no second child, an embedded
/// source `dcsat:appearance` (with any color found inside it), or a storage
/// `appIndex` (whose raw text the caller must validate -- the side-table
/// wrapper elsewhere is the sole authority for what that index's content
/// actually is).
enum TableCellAppearanceSignal {
    Absent,
    Source(Option<DcsAppearanceColor>),
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
            let (has_appearance, color) = match parse_table_cell(cells[0], p, ap)? {
                TableCellAppearanceSignal::Absent => (false, None),
                TableCellAppearanceSignal::Source(color) => (true, color),
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
                color,
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
) -> Result<Option<DcsAppearanceColor>, DcsInnerSchemaError> {
    require_no_attributes(appearance)?;
    require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Source,
        ap.appearance_parameter(),
    )
}

fn require_storage_area_appearance(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<Option<DcsAppearanceColor>, DcsInnerSchemaError> {
    require_name(appearance, Some(ap.area_namespace_uri()), "appearance")?;
    require_type(appearance, p, &ap.table_cell_appearance_type_qname())?;
    require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Storage,
        ap.appearance_parameter(),
    )
}

/// Validates the storage side-table entry shared by both row-1 cells of
/// the two-row shared-appearance cohort. Unlike the single-cell storage
/// entry, this one is spelled `Details` (see
/// `DcsAreaTemplatePolicy::storage_shared_row_appearance_parameter`) even
/// though it holds only one item and no color -- the evidenced
/// discriminator is the record being referenced by more than one cell, not
/// its own item count. A color item here would be unevidenced, so it is
/// rejected rather than silently accepted.
fn require_storage_shared_row_appearance(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
) -> Result<(), DcsInnerSchemaError> {
    require_name(appearance, Some(ap.area_namespace_uri()), "appearance")?;
    require_type(appearance, p, &ap.table_cell_appearance_type_qname())?;
    match require_parameter_appearance_body(
        appearance,
        p,
        ap,
        AreaAppearanceDirection::Storage,
        ap.storage_shared_row_appearance_parameter(),
    )? {
        None => Ok(()),
        Some(_) => unsupported(
            "AreaTemplate shared-row appearance side table must not contain a color item",
        ),
    }
}

/// Validates the shared `dcscor:item`/`item` appearance body shape and
/// returns the color, if the evidenced two-item `ЦветТекста` + `Расшифровка`
/// state was found. Exactly one or two items are admitted; the color item,
/// when present, must be first. `expected_single_item_parameter` is the
/// literal expected for the lone item in the one-item state, which differs
/// between the plain single-cell storage entry (`Расшифровка`) and the
/// shared-row storage entry (`Details`); both directions' single-cell and
/// two-item-with-color cases always expect `Расшифровка`/`storage_appearance_parameter_with_color`.
fn require_parameter_appearance_body(
    appearance: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    direction: AreaAppearanceDirection,
    expected_single_item_parameter: &str,
) -> Result<Option<DcsAppearanceColor>, DcsInnerSchemaError> {
    let items = elements(appearance)?;
    match items.len() {
        1 => {
            require_parameter_item(items[0], p, ap, expected_single_item_parameter)?;
            Ok(None)
        }
        2 => {
            let color = require_color_item(items[0], p, ap, direction)?;
            let expected_parameter = match direction {
                AreaAppearanceDirection::Source => ap.appearance_parameter(),
                AreaAppearanceDirection::Storage => ap.storage_appearance_parameter_with_color(),
            };
            require_parameter_item(items[1], p, ap, expected_parameter)?;
            Ok(Some(color))
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

fn require_color_item(
    item: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    ap: &DcsAreaTemplatePolicy,
    direction: AreaAppearanceDirection,
) -> Result<DcsAppearanceColor, DcsInnerSchemaError> {
    require_name(item, Some(ap.core_namespace_uri()), "item")?;
    require_no_attributes(item)?;
    let children = elements(item)?;
    if children.len() != 2 {
        return unsupported("AreaTemplate appearance color item must contain parameter and value");
    }
    require_name(children[0], Some(ap.core_namespace_uri()), "parameter")?;
    require_no_attributes(children[0])?;
    require_name(children[1], Some(ap.core_namespace_uri()), "value")?;
    require_type(children[1], p, &ap.color_type_qname())?;
    let parameter = text(children[0])?;
    let expected_parameter = match direction {
        AreaAppearanceDirection::Source => ap.text_color_parameter(),
        AreaAppearanceDirection::Storage => ap.storage_text_color_parameter(),
    };
    if parameter.trim() != expected_parameter {
        return unsupported(
            "AreaTemplate appearance color parameter is outside the exact coordinate",
        );
    }
    // Compare only the expanded QName: the platform does not preserve the
    // source prefix spelling, and the evidenced cohort admits any prefix
    // bound to the web-colors namespace (native uses an auto-generated
    // `d8p1`/`d4p2`, the seed uses a locally-declared `web`).
    if resolve_qname_text_allowing_attributes(children[1])? != ap.web_red_qname() {
        return unsupported("AreaTemplate appearance color value is outside the exact coordinate");
    }
    Ok(DcsAppearanceColor::WebRed)
}

fn parse_query(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    qp: &ibcmd_schema::DcsQueryUnionLinkPolicy,
    nested: bool,
) -> Result<DcsSchemaQueryDataSet, DcsInnerSchemaError> {
    require_name(
        e,
        Some(p.schema_namespace_uri()),
        if nested { "item" } else { "dataSet" },
    )?;
    require_type(e, p, qp.query_type_qname())?;
    let c = exact_children(e, qp.query_children(), p.schema_namespace_uri())?;
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
    let query = DcsSchemaQueryDataSet::new(
        canonical(text(c[0])?)?,
        field,
        canonical(text(c[2])?)?,
        canonical(text(c[3])?)?,
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
) -> Result<DcsSchemaUnionDataSet, DcsInnerSchemaError> {
    require_name(e, Some(p.schema_namespace_uri()), "dataSet")?;
    require_type(e, p, qp.union_type_qname())?;
    let c = exact_children(e, qp.union_children(), p.schema_namespace_uri())?;
    DcsSchemaUnionDataSet::new(canonical(text(c[0])?)?, parse_query(c[1], p, qp, true)?)
        .map_err(DcsInnerSchemaError::Build)
}

fn parse_link(
    e: &ParsedElement,
    p: &DcsInnerSchemaPolicy,
    qp: &ibcmd_schema::DcsQueryUnionLinkPolicy,
) -> Result<DcsSchemaDataSetLink, DcsInnerSchemaError> {
    require_name(e, Some(p.schema_namespace_uri()), "dataSetLink")?;
    require_no_attributes(e)?;
    let c = exact_children(e, qp.link_children(), p.schema_namespace_uri())?;
    DcsSchemaDataSetLink::new(
        canonical(text(c[0])?)?,
        canonical(text(c[1])?)?,
        canonical(text(c[2])?)?,
        canonical(text(c[3])?)?,
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
    emit_query(&mut out, 1, "dataSet", schema.query());
    line(&mut out, 1, "<dataSet xsi:type=\"DataSetUnion\">");
    scalar(&mut out, 2, "name", schema.union().name().as_str());
    emit_query(&mut out, 2, "item", schema.union().item());
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
        if let Some(color) = color {
            line(
                &mut out,
                2,
                "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
            );
            scalar(&mut out, 3, "parameter", "TextColor");
            line(&mut out, 3, storage_color_value_fragment(color));
            line(&mut out, 2, "</item>");
        }
        line(
            &mut out,
            2,
            "<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
        );
        let parameter_label = if color.is_some() {
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

fn emit_query(out: &mut String, depth: usize, element: &str, query: &DcsSchemaQueryDataSet) {
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

fn area_policy() -> Result<DcsAreaTemplatePolicy, DcsInnerSchemaError> {
    bundled_dcs_area_template_policy()
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
