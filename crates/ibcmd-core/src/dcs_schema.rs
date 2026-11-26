//! Bounded canonical IR for the platform-attested inner DCS schema cohort.
//!
//! This module owns semantic structure only. XML QNames, namespace prefixes,
//! `xsi:type` spellings, document framing, and writer order remain schema/XML
//! policy. The admitted shape is deliberately narrow: one local data source,
//! one object data set with either the attested single string field or the
//! richer string/decimal pair, optional richer calculated/totals/parameter
//! members, and one or two positional settings-variant shells.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::dcs::DcsAppearanceColor;
use crate::provenance::SourceProvenance;
use crate::value::CanonicalText;

/// Exact field count in the first attested object-data-set cohort.
pub const DCS_SCHEMA_DATA_SET_FIELD_COUNT: usize = 2;
/// Exact total count in the first attested inner-schema cohort.
pub const DCS_SCHEMA_TOTAL_FIELD_COUNT: usize = 2;
/// Maximum settings variants admitted by the attested positional envelope.
pub const MAX_DCS_SCHEMA_SETTINGS_VARIANTS: usize = 2;
/// Aggregate variable-sized byte budget for one bounded inner schema.
pub const MAX_DCS_SCHEMA_RETAINED_BYTES: usize = 16_777_216;

/// Exact Query/Union/link cardinality authenticated by the dedicated cohort.
pub const DCS_SCHEMA_QUERY_UNION_LINK_COUNT: usize = 1;

/// One evidenced DCS area-template appearance style-color reference, in one
/// of the two forms proven by the dedicated 2214 style-reference cohort:
///
/// - A standard, built-in platform style, referenced by its bare lexical
///   name (e.g. `NegativeTextColor`). No configuration object backs this
///   reference, and the storage side retains the same named lexical form.
/// - A custom `StyleItem` configuration object, referenced by its semantic
///   name (e.g. `CorpusAccent`). At the *source* XML layer this is
///   lexically indistinguishable from the standard-named form above; only
///   the storage side reveals the difference, spelling it as a raw
///   `0:<uuid>` reference to the StyleItem's own configuration-local uuid.
///
/// This IR never carries that uuid: resolving it to (or from) this
/// semantic name is reference resolution, an adapter-supplied concern (see
/// the evidenced TypeId-reference precedent), not something this XML-layer
/// value type performs itself.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsStyleColorReference {
    Named(CanonicalText),
    CustomStyleItem(CanonicalText),
}

impl DcsStyleColorReference {
    pub fn named(name: CanonicalText) -> Result<Self, DcsSchemaBuildError> {
        require_text("style-color reference name", &name)?;
        Ok(Self::Named(name))
    }

    pub fn custom_style_item(name: CanonicalText) -> Result<Self, DcsSchemaBuildError> {
        require_text("style-color reference custom StyleItem name", &name)?;
        Ok(Self::CustomStyleItem(name))
    }

    /// The referenced style's semantic name, common to both forms.
    pub const fn name(&self) -> &CanonicalText {
        match self {
            Self::Named(name) | Self::CustomStyleItem(name) => name,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum DcsStyleColorReferenceWire {
    Named(CanonicalText),
    CustomStyleItem(CanonicalText),
}

impl<'de> Deserialize<'de> for DcsStyleColorReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsStyleColorReferenceWire::deserialize(deserializer)?;
        match wire {
            DcsStyleColorReferenceWire::Named(name) => Self::named(name),
            DcsStyleColorReferenceWire::CustomStyleItem(name) => Self::custom_style_item(name),
        }
        .map_err(de::Error::custom)
    }
}

/// Exact style-free AreaTemplate authenticated by the dedicated 2214 cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaAreaTemplate {
    name: CanonicalText,
    parameter_name: CanonicalText,
    expression: CanonicalText,
    parameter_appearance: bool,
    /// Web-cohort text color authenticated by the dedicated 2214
    /// appearance-color side-table cohort. Always co-occurs with
    /// `parameter_appearance`; the platform never emits color alone.
    text_color_appearance: Option<DcsAppearanceColor>,
    /// `ЦветФона` (`BackColor` in storage) style-color reference
    /// authenticated by the dedicated 2214 style-reference cohort. Always
    /// co-occurs with `parameter_appearance`, exactly like
    /// `text_color_appearance`; mutually exclusive with it by evidence (no
    /// cohort proves combining the two color parameters).
    back_color_style_reference: Option<DcsStyleColorReference>,
    /// Replaces the whole area body with the exact two-row shape
    /// authenticated by the dedicated 2214 multi-cell-appearance cohort:
    /// row 1 has two `tableCell`s sharing one identical `Расшифровка =
    /// Parameter(Probe)` appearance record (the storage side table has
    /// exactly one entry, referenced by both cells via the same
    /// `appIndex`); row 2 has one `tableCell` with no appearance. Mutually
    /// exclusive with `parameter_appearance`/`text_color_appearance`, which
    /// describe the single-cell area body instead.
    shared_row_appearance: bool,
    provenance: SourceProvenance,
}

impl DcsSchemaAreaTemplate {
    pub fn new(
        name: CanonicalText,
        parameter_name: CanonicalText,
        expression: CanonicalText,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("AreaTemplate name", &name)?;
        require_text("AreaTemplate parameter name", &parameter_name)?;
        require_text("AreaTemplate expression", &expression)?;
        if name.as_str() != "AreaProbe"
            || parameter_name.as_str() != "Probe"
            || expression.as_str() != "\"Probe\""
        {
            return Err(DcsSchemaBuildError::AreaTemplateMismatch);
        }
        Ok(Self {
            name,
            parameter_name,
            expression,
            parameter_appearance: false,
            text_color_appearance: None,
            back_color_style_reference: None,
            shared_row_appearance: false,
            provenance,
        })
    }

    /// Enables the exact `Расшифровка = Parameter(Probe)` table-cell
    /// appearance authenticated by the dedicated 2214 side-table cohort.
    pub fn with_parameter_appearance(mut self) -> Self {
        self.parameter_appearance = true;
        self
    }

    /// Enables the exact `ЦветТекста = web:Red` then `Расшифровка =
    /// Parameter(Probe)` table-cell appearance pair authenticated by the
    /// dedicated 2214 appearance-color side-table cohort. The color item is
    /// always ordered before the parameter item; there is no evidenced
    /// color-only state.
    pub fn with_color_and_parameter_appearance(mut self, color: DcsAppearanceColor) -> Self {
        self.parameter_appearance = true;
        self.text_color_appearance = Some(color);
        self
    }

    /// Enables the exact `ЦветФона = <style reference>` then `Расшифровка =
    /// Parameter(Probe)` table-cell appearance pair authenticated by the
    /// dedicated 2214 style-reference cohort. The style-color item is
    /// always ordered before the parameter item, exactly like
    /// `with_color_and_parameter_appearance`; callers must not also call
    /// `with_color_and_parameter_appearance` on the same value (no cohort
    /// proves combining the two color parameters).
    pub fn with_style_reference_and_parameter_appearance(
        mut self,
        reference: DcsStyleColorReference,
    ) -> Self {
        self.parameter_appearance = true;
        self.back_color_style_reference = Some(reference);
        self
    }

    /// Enables the exact two-row shared-appearance area body authenticated
    /// by the dedicated 2214 multi-cell-appearance cohort. See the field
    /// doc comment for the exact shape. Replaces the single-cell area body
    /// entirely; callers must not also call `with_parameter_appearance` or
    /// `with_color_and_parameter_appearance` on the same value.
    pub fn with_shared_row_appearance(mut self) -> Self {
        self.shared_row_appearance = true;
        self
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }
    pub const fn parameter_name(&self) -> &CanonicalText {
        &self.parameter_name
    }
    pub const fn expression(&self) -> &CanonicalText {
        &self.expression
    }
    pub const fn has_parameter_appearance(&self) -> bool {
        self.parameter_appearance
    }
    pub const fn text_color_appearance(&self) -> Option<DcsAppearanceColor> {
        self.text_color_appearance
    }
    pub const fn back_color_style_reference(&self) -> Option<&DcsStyleColorReference> {
        self.back_color_style_reference.as_ref()
    }
    pub const fn has_shared_row_appearance(&self) -> bool {
        self.shared_row_appearance
    }
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaAreaTemplateWire {
    name: CanonicalText,
    parameter_name: CanonicalText,
    expression: CanonicalText,
    #[serde(default)]
    parameter_appearance: bool,
    #[serde(default)]
    text_color_appearance: Option<DcsAppearanceColor>,
    #[serde(default)]
    back_color_style_reference: Option<DcsStyleColorReference>,
    #[serde(default)]
    shared_row_appearance: bool,
    provenance: SourceProvenance,
}

impl<'de> Deserialize<'de> for DcsSchemaAreaTemplate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaAreaTemplateWire::deserialize(deserializer)?;
        let value = Self::new(
            wire.name,
            wire.parameter_name,
            wire.expression,
            wire.provenance,
        )
        .map_err(de::Error::custom)?;
        let value = match (
            wire.parameter_appearance,
            wire.text_color_appearance,
            wire.back_color_style_reference,
        ) {
            (_, Some(color), None) => value.with_color_and_parameter_appearance(color),
            (_, None, Some(reference)) => {
                value.with_style_reference_and_parameter_appearance(reference)
            }
            (_, Some(_), Some(_)) => {
                return Err(de::Error::custom(
                    "DCS AreaTemplate cannot combine text_color_appearance and back_color_style_reference",
                ));
            }
            (true, None, None) => value.with_parameter_appearance(),
            (false, None, None) => value,
        };
        Ok(if wire.shared_row_appearance {
            value.with_shared_row_appearance()
        } else {
            value
        })
    }
}

/// Failure to construct or revalidate the bounded inner DCS schema IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsSchemaBuildError {
    /// A required semantic text was empty.
    EmptyText { field: &'static str },
    /// String length must be positive for the attested qualified string type.
    ZeroStringLength,
    /// Only the two string lengths present in the attested cohort are admitted.
    UnsupportedStringLength { length: u32 },
    /// The data-set string field has the attested length 20.
    UnexpectedDataSetStringLength { length: u32 },
    /// The scalar string parameter has the attested length 40.
    UnexpectedParameterStringLength { length: u32 },
    /// Decimal precision must be positive.
    ZeroDecimalDigits,
    /// Decimal scale exceeded its precision.
    DecimalFractionExceedsDigits { digits: u32, fraction_digits: u32 },
    /// Only decimal(15,2) is present in the attested cohort.
    UnsupportedDecimalQualifiers { digits: u32, fraction_digits: u32 },
    /// The bounded localized string admits only the attested language token.
    UnsupportedLanguage { language: String },
    /// The object data set did not contain exactly the attested two fields.
    UnexpectedDataSetFieldCount { expected: usize, actual: usize },
    /// The attested field order is string followed by decimal or one
    /// generated reference.
    UnexpectedDataSetFieldTypeOrder,
    /// Two data-set fields used the same output data path.
    DuplicateDataSetFieldPath { path: String },
    /// The data set referenced a different data source.
    DataSourceReferenceMismatch,
    /// The calculated field reused a data-set output path.
    DuplicateCalculatedFieldPath { path: String },
    /// The schema did not contain exactly the attested two total fields.
    UnexpectedTotalFieldCount { expected: usize, actual: usize },
    /// Total fields did not target the data-set decimal field followed by the
    /// decimal calculated field.
    UnexpectedTotalFieldOrder,
    /// No settings variant was supplied.
    EmptySettingsVariants,
    /// More settings variants were supplied than the envelope evidence admits.
    TooManySettingsVariants { maximum: usize, actual: usize },
    /// Two settings-variant shells used the same exact name.
    DuplicateSettingsVariantName { name: String },
    /// Aggregate variable-sized content exceeded the core IR budget.
    RetainedBytesExceeded { maximum: usize, actual: usize },
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
    /// The bounded Query/Union/link graph has inconsistent names or fields.
    QueryUnionLinkMismatch,
    /// The style-free AreaTemplate differs from the exact admitted coordinate.
    AreaTemplateMismatch,
    /// Only decimal(10,2) is present in the attested scalar-parameter cohort.
    UnsupportedParameterDecimalQualifiers { digits: u32, fraction_digits: u32 },
    /// Only the attested `100.5` decimal literal is present in the cohort.
    UnsupportedParameterDecimalValue { value: String },
    /// The three additional scalar-typed parameters require the existing
    /// string `Caption` parameter to already be present.
    ScalarParametersRequireStringParameter,
    /// The evidenced `dataSetLink` `linkConditionExpression`/`startExpression`/
    /// `required` triple only co-occurs with `parameter`/`parameterListAllowed`
    /// already present; no cohort proves the triple alone.
    LinkExpressionsRequireParameter,
}

impl Display for DcsSchemaBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { field } => write!(formatter, "DCS schema {field} is empty"),
            Self::ZeroStringLength => {
                formatter.write_str("DCS schema string length must be positive")
            }
            Self::UnsupportedStringLength { length } => write!(
                formatter,
                "DCS schema string length {length} is outside the attested 20/40 cohort"
            ),
            Self::UnexpectedDataSetStringLength { length } => write!(
                formatter,
                "DCS schema data-set string length must be 20 (actual {length})"
            ),
            Self::UnexpectedParameterStringLength { length } => write!(
                formatter,
                "DCS schema parameter string length must be 40 (actual {length})"
            ),
            Self::ZeroDecimalDigits => {
                formatter.write_str("DCS schema decimal digits must be positive")
            }
            Self::DecimalFractionExceedsDigits {
                digits,
                fraction_digits,
            } => write!(
                formatter,
                "DCS schema decimal fraction digits {fraction_digits} exceed digits {digits}"
            ),
            Self::UnsupportedDecimalQualifiers {
                digits,
                fraction_digits,
            } => write!(
                formatter,
                "DCS schema decimal qualifiers ({digits},{fraction_digits}) are outside the attested (15,2) cohort"
            ),
            Self::UnsupportedLanguage { language } => write!(
                formatter,
                "DCS schema localized language `{language}` is outside the attested `ru` cohort"
            ),
            Self::UnexpectedDataSetFieldCount { expected, actual } => write!(
                formatter,
                "DCS schema object data set requires exactly {expected} fields (actual {actual})"
            ),
            Self::UnexpectedDataSetFieldTypeOrder => formatter.write_str(
                "DCS schema object data-set fields must be string followed by decimal or one generated reference",
            ),
            Self::DuplicateDataSetFieldPath { path } => {
                write!(formatter, "DCS schema data-set field path `{path}` is duplicated")
            }
            Self::DataSourceReferenceMismatch => formatter.write_str(
                "DCS schema object data set references a different local data source",
            ),
            Self::DuplicateCalculatedFieldPath { path } => write!(
                formatter,
                "DCS schema calculated field path `{path}` duplicates a data-set field path"
            ),
            Self::UnexpectedTotalFieldCount { expected, actual } => write!(
                formatter,
                "DCS schema requires exactly {expected} ungrouped totals (actual {actual})"
            ),
            Self::UnexpectedTotalFieldOrder => formatter.write_str(
                "DCS schema totals must target the data-set decimal field and then the calculated decimal field",
            ),
            Self::EmptySettingsVariants => {
                formatter.write_str("DCS schema requires at least one settings-variant shell")
            }
            Self::TooManySettingsVariants { maximum, actual } => write!(
                formatter,
                "DCS schema exceeds {maximum} settings-variant shells (actual {actual})"
            ),
            Self::DuplicateSettingsVariantName { name } => write!(
                formatter,
                "DCS schema settings-variant name `{name}` is duplicated"
            ),
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "DCS schema exceeds retained-byte budget {maximum} (actual {actual})"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("DCS schema retained-byte count overflowed")
            }
            Self::QueryUnionLinkMismatch => formatter.write_str(
                "DCS schema Query/Union/link references do not match the attested graph",
            ),
            Self::AreaTemplateMismatch => formatter.write_str(
                "DCS schema AreaTemplate differs from the attested style-free coordinate",
            ),
            Self::UnsupportedParameterDecimalQualifiers {
                digits,
                fraction_digits,
            } => write!(
                formatter,
                "DCS schema parameter decimal qualifiers ({digits},{fraction_digits}) are outside the attested (10,2) cohort"
            ),
            Self::UnsupportedParameterDecimalValue { value } => write!(
                formatter,
                "DCS schema parameter decimal value `{value}` is outside the attested `100.5` cohort"
            ),
            Self::ScalarParametersRequireStringParameter => formatter.write_str(
                "DCS schema scalar-typed parameters require the existing string Caption parameter",
            ),
            Self::LinkExpressionsRequireParameter => formatter.write_str(
                "DCS schema dataSetLink linkConditionExpression/startExpression/required require parameter/parameterListAllowed",
            ),
        }
    }
}

impl Error for DcsSchemaBuildError {}

/// One untyped field in the exact Query cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaQueryField {
    data_path: CanonicalText,
    field: CanonicalText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaQueryFieldWire {
    data_path: CanonicalText,
    field: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaQueryField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaQueryFieldWire::deserialize(deserializer)?;
        Self::new(wire.data_path, wire.field).map_err(de::Error::custom)
    }
}

impl DcsSchemaQueryField {
    pub fn new(
        data_path: CanonicalText,
        field: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("query field data path", &data_path)?;
        require_text("query source field", &field)?;
        Ok(Self { data_path, field })
    }
    pub const fn data_path(&self) -> &CanonicalText {
        &self.data_path
    }
    pub const fn field(&self) -> &CanonicalText {
        &self.field
    }
}

/// One exact `DataSetQuery` node. `typed_field` is the second, evidence-bound
/// evidenced extension: exactly one additional field carrying a
/// `DcsSchemaFieldType::Reference` value type (the `dcs-query-union-link-typeid`
/// cohort's `Owner`/`CatalogRef.FilterProbe` construction, reusing the same
/// `DcsSchemaDataSetField` shape and the same evidenced reference resolution
/// the `dcs-typeid-reference` DataSetObject cohort already proved). `None`
/// keeps the original single-field `dcs-query-union-link` cohort; the
/// `DataSetUnion` item position has no evidence for a second field and is
/// never constructed with one.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaQueryDataSet {
    name: CanonicalText,
    field: DcsSchemaQueryField,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typed_field: Option<DcsSchemaDataSetField>,
    data_source: CanonicalText,
    query: CanonicalText,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaQueryDataSetWire {
    name: CanonicalText,
    field: DcsSchemaQueryField,
    #[serde(default)]
    typed_field: Option<DcsSchemaDataSetField>,
    data_source: CanonicalText,
    query: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaQueryDataSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaQueryDataSetWire::deserialize(deserializer)?;
        Self::new(
            wire.name,
            wire.field,
            wire.typed_field,
            wire.data_source,
            wire.query,
        )
        .map_err(de::Error::custom)
    }
}

impl DcsSchemaQueryDataSet {
    pub fn new(
        name: CanonicalText,
        field: DcsSchemaQueryField,
        typed_field: Option<DcsSchemaDataSetField>,
        data_source: CanonicalText,
        query: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("query data-set name", &name)?;
        require_text("query data-source reference", &data_source)?;
        require_text("query text", &query)?;
        Ok(Self {
            name,
            field,
            typed_field,
            data_source,
            query,
        })
    }
    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }
    pub const fn field(&self) -> &DcsSchemaQueryField {
        &self.field
    }
    pub const fn typed_field(&self) -> Option<&DcsSchemaDataSetField> {
        self.typed_field.as_ref()
    }
    pub const fn data_source(&self) -> &CanonicalText {
        &self.data_source
    }
    pub const fn query(&self) -> &CanonicalText {
        &self.query
    }
}

/// One exact `DataSetUnion` containing one Query item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaUnionDataSet {
    name: CanonicalText,
    item: DcsSchemaQueryDataSet,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaUnionDataSetWire {
    name: CanonicalText,
    item: DcsSchemaQueryDataSet,
}

impl<'de> Deserialize<'de> for DcsSchemaUnionDataSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaUnionDataSetWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.item).map_err(de::Error::custom)
    }
}

impl DcsSchemaUnionDataSet {
    pub fn new(
        name: CanonicalText,
        item: DcsSchemaQueryDataSet,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("union data-set name", &name)?;
        Ok(Self { name, item })
    }
    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }
    pub const fn item(&self) -> &DcsSchemaQueryDataSet {
        &self.item
    }
}

/// One exact direct link from the Query to the Union. Four fields
/// (`source_data_set`/`destination_data_set`/`source_expression`/
/// `destination_expression`) are always present, authenticated by the base
/// `dcs-query-union-link` cohort. Two evidenced optional extensions layer
/// on top, each only proven together as a whole group, never partially:
///
/// - `parameter`/`parameter_list_allowed` (the `dcs-link-parameter`
///   cohort's `LinkParam`/`true` pair).
/// - `link_condition_expression`/`start_expression`/`required` (the
///   `dcs-link-expressions` cohort's triple), which itself only co-occurs
///   with `parameter`/`parameter_list_allowed` already present -- no
///   cohort proves the triple alone.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaDataSetLink {
    source_data_set: CanonicalText,
    destination_data_set: CanonicalText,
    source_expression: CanonicalText,
    destination_expression: CanonicalText,
    parameter: Option<CanonicalText>,
    parameter_list_allowed: Option<bool>,
    link_condition_expression: Option<CanonicalText>,
    start_expression: Option<CanonicalText>,
    required: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaDataSetLinkWire {
    source_data_set: CanonicalText,
    destination_data_set: CanonicalText,
    source_expression: CanonicalText,
    destination_expression: CanonicalText,
    #[serde(default)]
    parameter: Option<CanonicalText>,
    #[serde(default)]
    parameter_list_allowed: Option<bool>,
    #[serde(default)]
    link_condition_expression: Option<CanonicalText>,
    #[serde(default)]
    start_expression: Option<CanonicalText>,
    #[serde(default)]
    required: Option<bool>,
}

impl<'de> Deserialize<'de> for DcsSchemaDataSetLink {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaDataSetLinkWire::deserialize(deserializer)?;
        let value = Self::new(
            wire.source_data_set,
            wire.destination_data_set,
            wire.source_expression,
            wire.destination_expression,
        )
        .map_err(de::Error::custom)?;
        let value = match (wire.parameter, wire.parameter_list_allowed) {
            (Some(parameter), Some(parameter_list_allowed)) => value
                .with_parameter(parameter, parameter_list_allowed)
                .map_err(de::Error::custom)?,
            (None, None) => value,
            _ => {
                return Err(de::Error::custom(
                    "DCS dataSetLink parameter and parameterListAllowed must both be present or both absent",
                ));
            }
        };
        match (
            wire.link_condition_expression,
            wire.start_expression,
            wire.required,
        ) {
            (Some(link_condition_expression), Some(start_expression), Some(required)) => value
                .with_expressions(link_condition_expression, start_expression, required)
                .map_err(de::Error::custom),
            (None, None, None) => Ok(value),
            _ => Err(de::Error::custom(
                "DCS dataSetLink linkConditionExpression/startExpression/required must all be present or all absent",
            )),
        }
    }
}

/// Complete bounded semantic value for the exact Query/Union/link cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaQueryUnionLink {
    data_source: DcsSchemaLocalDataSource,
    query: DcsSchemaQueryDataSet,
    union: DcsSchemaUnionDataSet,
    link: DcsSchemaDataSetLink,
    settings_variants: Vec<DcsSchemaSettingsVariantShell>,
    provenance: SourceProvenance,
}

impl DcsSchemaQueryUnionLink {
    pub fn new(
        data_source: DcsSchemaLocalDataSource,
        query: DcsSchemaQueryDataSet,
        union: DcsSchemaUnionDataSet,
        link: DcsSchemaDataSetLink,
        settings_variants: Vec<DcsSchemaSettingsVariantShell>,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsSchemaBuildError> {
        let field = query.field().data_path();
        if query.data_source() != data_source.name()
            || union.item().data_source() != data_source.name()
            || query.field() != union.item().field()
            || query.query() != union.item().query()
            || link.source_data_set() != query.name()
            || link.destination_data_set() != union.name()
            || link.source_expression() != field
            || link.destination_expression() != field
            || settings_variants.len() != DCS_SCHEMA_QUERY_UNION_LINK_COUNT
        {
            return Err(DcsSchemaBuildError::QueryUnionLinkMismatch);
        }
        let model = Self {
            data_source,
            query,
            union,
            link,
            settings_variants,
            provenance,
        };
        validate_query_union_link_retained_bytes(&model)?;
        Ok(model)
    }
    pub const fn data_source(&self) -> &DcsSchemaLocalDataSource {
        &self.data_source
    }
    pub const fn query(&self) -> &DcsSchemaQueryDataSet {
        &self.query
    }
    pub const fn union(&self) -> &DcsSchemaUnionDataSet {
        &self.union
    }
    pub const fn link(&self) -> &DcsSchemaDataSetLink {
        &self.link
    }
    pub fn settings_variants(&self) -> &[DcsSchemaSettingsVariantShell] {
        &self.settings_variants
    }
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaQueryUnionLinkWire {
    data_source: DcsSchemaLocalDataSource,
    query: DcsSchemaQueryDataSet,
    union: DcsSchemaUnionDataSet,
    link: DcsSchemaDataSetLink,
    settings_variants:
        BoundedDcsSchemaVec<DcsSchemaSettingsVariantShell, DCS_SCHEMA_QUERY_UNION_LINK_COUNT>,
    provenance: SourceProvenance,
}

impl<'de> Deserialize<'de> for DcsSchemaQueryUnionLink {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DcsSchemaQueryUnionLinkWire::deserialize(deserializer)?;
        Self::new(
            wire.data_source,
            wire.query,
            wire.union,
            wire.link,
            wire.settings_variants.values,
            wire.provenance,
        )
        .map_err(de::Error::custom)
    }
}

impl DcsSchemaDataSetLink {
    pub fn new(
        source_data_set: CanonicalText,
        destination_data_set: CanonicalText,
        source_expression: CanonicalText,
        destination_expression: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        for (field, value) in [
            ("link source data set", &source_data_set),
            ("link destination data set", &destination_data_set),
            ("link source expression", &source_expression),
            ("link destination expression", &destination_expression),
        ] {
            require_text(field, value)?;
        }
        Ok(Self {
            source_data_set,
            destination_data_set,
            source_expression,
            destination_expression,
            parameter: None,
            parameter_list_allowed: None,
            link_condition_expression: None,
            start_expression: None,
            required: None,
        })
    }

    /// Enables the exact `parameter`/`parameterListAllowed` pair
    /// authenticated by the dedicated 2214 `dcs-link-parameter` cohort.
    /// The two always co-occur; there is no evidenced state with only one
    /// of them present.
    pub fn with_parameter(
        mut self,
        parameter: CanonicalText,
        parameter_list_allowed: bool,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("link parameter", &parameter)?;
        self.parameter = Some(parameter);
        self.parameter_list_allowed = Some(parameter_list_allowed);
        Ok(self)
    }

    /// Enables the exact `linkConditionExpression`/`startExpression`/
    /// `required` triple authenticated by the dedicated 2214
    /// `dcs-link-expressions` cohort. Requires [`Self::with_parameter`] to
    /// have already been applied: no cohort proves this triple without the
    /// `parameter`/`parameterListAllowed` pair also present.
    pub fn with_expressions(
        mut self,
        link_condition_expression: CanonicalText,
        start_expression: CanonicalText,
        required: bool,
    ) -> Result<Self, DcsSchemaBuildError> {
        if self.parameter.is_none() {
            return Err(DcsSchemaBuildError::LinkExpressionsRequireParameter);
        }
        require_text("link condition expression", &link_condition_expression)?;
        require_text("link start expression", &start_expression)?;
        self.link_condition_expression = Some(link_condition_expression);
        self.start_expression = Some(start_expression);
        self.required = Some(required);
        Ok(self)
    }

    pub const fn source_data_set(&self) -> &CanonicalText {
        &self.source_data_set
    }
    pub const fn destination_data_set(&self) -> &CanonicalText {
        &self.destination_data_set
    }
    pub const fn source_expression(&self) -> &CanonicalText {
        &self.source_expression
    }
    pub const fn destination_expression(&self) -> &CanonicalText {
        &self.destination_expression
    }
    pub const fn parameter(&self) -> Option<&CanonicalText> {
        self.parameter.as_ref()
    }
    pub const fn parameter_list_allowed(&self) -> Option<bool> {
        self.parameter_list_allowed
    }
    pub const fn link_condition_expression(&self) -> Option<&CanonicalText> {
        self.link_condition_expression.as_ref()
    }
    pub const fn start_expression(&self) -> Option<&CanonicalText> {
        self.start_expression.as_ref()
    }
    pub const fn required(&self) -> Option<bool> {
        self.required
    }
}

/// Variable-length string qualifiers admitted by the attested cohort.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaStringType {
    length: u32,
}

impl DcsSchemaStringType {
    pub fn new(length: u32) -> Result<Self, DcsSchemaBuildError> {
        if length == 0 {
            return Err(DcsSchemaBuildError::ZeroStringLength);
        }
        if !matches!(length, 20 | 40) {
            return Err(DcsSchemaBuildError::UnsupportedStringLength { length });
        }
        Ok(Self { length })
    }

    pub const fn length(&self) -> u32 {
        self.length
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaStringTypeWire {
    length: u32,
}

impl<'de> Deserialize<'de> for DcsSchemaStringType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaStringTypeWire::deserialize(deserializer)?;
        Self::new(wire.length).map_err(de::Error::custom)
    }
}

/// Decimal qualifiers with unrestricted sign, as observed in the cohort.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaDecimalType {
    digits: u32,
    fraction_digits: u32,
}

impl DcsSchemaDecimalType {
    pub fn new(digits: u32, fraction_digits: u32) -> Result<Self, DcsSchemaBuildError> {
        if digits == 0 {
            return Err(DcsSchemaBuildError::ZeroDecimalDigits);
        }
        if fraction_digits > digits {
            return Err(DcsSchemaBuildError::DecimalFractionExceedsDigits {
                digits,
                fraction_digits,
            });
        }
        if digits != 15 || fraction_digits != 2 {
            return Err(DcsSchemaBuildError::UnsupportedDecimalQualifiers {
                digits,
                fraction_digits,
            });
        }
        Ok(Self {
            digits,
            fraction_digits,
        })
    }

    pub const fn digits(&self) -> u32 {
        self.digits
    }

    pub const fn fraction_digits(&self) -> u32 {
        self.fraction_digits
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaDecimalTypeWire {
    digits: u32,
    fraction_digits: u32,
}

impl<'de> Deserialize<'de> for DcsSchemaDecimalType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaDecimalTypeWire::deserialize(deserializer)?;
        Self::new(wire.digits, wire.fraction_digits).map_err(de::Error::custom)
    }
}

/// Closed field value-type variants admitted by the first schema cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsSchemaFieldType {
    String(DcsSchemaStringType),
    Decimal(DcsSchemaDecimalType),
    Reference(DcsSchemaReferenceType),
}

/// One generated current-configuration reference, represented semantically
/// rather than by the configuration-local storage UUID.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaReferenceType {
    qualified_name: CanonicalText,
}

impl DcsSchemaReferenceType {
    pub fn new(qualified_name: CanonicalText) -> Result<Self, DcsSchemaBuildError> {
        require_text("reference type qualified name", &qualified_name)?;
        Ok(Self { qualified_name })
    }

    pub const fn qualified_name(&self) -> &CanonicalText {
        &self.qualified_name
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaReferenceTypeWire {
    qualified_name: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaReferenceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaReferenceTypeWire::deserialize(deserializer)?;
        Self::new(wire.qualified_name).map_err(de::Error::custom)
    }
}

/// One local DCS data source. Its type is fixed by this semantic type and is
/// not retained as an open token.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaLocalDataSource {
    name: CanonicalText,
}

impl DcsSchemaLocalDataSource {
    pub fn new(name: CanonicalText) -> Result<Self, DcsSchemaBuildError> {
        require_text("local data-source name", &name)?;
        Ok(Self { name })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaLocalDataSourceWire {
    name: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaLocalDataSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaLocalDataSourceWire::deserialize(deserializer)?;
        Self::new(wire.name).map_err(de::Error::custom)
    }
}

/// One direct `DataSetFieldField` semantic entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaDataSetField {
    data_path: CanonicalText,
    field: CanonicalText,
    value_type: DcsSchemaFieldType,
}

impl DcsSchemaDataSetField {
    pub fn new(
        data_path: CanonicalText,
        field: CanonicalText,
        value_type: DcsSchemaFieldType,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("data-set field data path", &data_path)?;
        require_text("data-set source field", &field)?;
        Ok(Self {
            data_path,
            field,
            value_type,
        })
    }

    pub const fn data_path(&self) -> &CanonicalText {
        &self.data_path
    }

    pub const fn field(&self) -> &CanonicalText {
        &self.field
    }

    pub const fn value_type(&self) -> &DcsSchemaFieldType {
        &self.value_type
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaDataSetFieldWire {
    data_path: CanonicalText,
    field: CanonicalText,
    value_type: DcsSchemaFieldType,
}

impl<'de> Deserialize<'de> for DcsSchemaDataSetField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaDataSetFieldWire::deserialize(deserializer)?;
        Self::new(wire.data_path, wire.field, wire.value_type).map_err(de::Error::custom)
    }
}

/// One bounded object data set. Field order is semantic and retained exactly.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaDataSetObject {
    name: CanonicalText,
    fields: Vec<DcsSchemaDataSetField>,
    data_source: CanonicalText,
    object_name: CanonicalText,
}

impl DcsSchemaDataSetObject {
    pub fn new(
        name: CanonicalText,
        fields: Vec<DcsSchemaDataSetField>,
        data_source: CanonicalText,
        object_name: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("object data-set name", &name)?;
        require_text("object data-set source reference", &data_source)?;
        require_text("object data-set object name", &object_name)?;
        if !(1..=DCS_SCHEMA_DATA_SET_FIELD_COUNT).contains(&fields.len()) {
            return Err(DcsSchemaBuildError::UnexpectedDataSetFieldCount {
                expected: DCS_SCHEMA_DATA_SET_FIELD_COUNT,
                actual: fields.len(),
            });
        }
        if !matches!(fields[0].value_type(), DcsSchemaFieldType::String(_))
            || (fields.len() == 2
                && !matches!(
                    fields[1].value_type(),
                    DcsSchemaFieldType::Decimal(_) | DcsSchemaFieldType::Reference(_)
                ))
        {
            return Err(DcsSchemaBuildError::UnexpectedDataSetFieldTypeOrder);
        }
        let DcsSchemaFieldType::String(string_type) = fields[0].value_type() else {
            unreachable!("field type order checked above")
        };
        if string_type.length() != 20 {
            return Err(DcsSchemaBuildError::UnexpectedDataSetStringLength {
                length: string_type.length(),
            });
        }
        let mut paths = BTreeSet::new();
        for field in &fields {
            if !paths.insert(field.data_path().as_str()) {
                return Err(DcsSchemaBuildError::DuplicateDataSetFieldPath {
                    path: field.data_path().as_str().to_owned(),
                });
            }
        }
        Ok(Self {
            name,
            fields,
            data_source,
            object_name,
        })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub fn fields(&self) -> &[DcsSchemaDataSetField] {
        &self.fields
    }

    pub const fn data_source(&self) -> &CanonicalText {
        &self.data_source
    }

    pub const fn object_name(&self) -> &CanonicalText {
        &self.object_name
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaDataSetObjectWire {
    name: CanonicalText,
    fields: BoundedDcsSchemaVec<DcsSchemaDataSetField, DCS_SCHEMA_DATA_SET_FIELD_COUNT>,
    data_source: CanonicalText,
    object_name: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaDataSetObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaDataSetObjectWire::deserialize(deserializer)?;
        Self::new(
            wire.name,
            wire.fields.values,
            wire.data_source,
            wire.object_name,
        )
        .map_err(de::Error::custom)
    }
}

/// The single direct calculated field admitted by the cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaCalculatedField {
    data_path: CanonicalText,
    expression: CanonicalText,
    value_type: DcsSchemaDecimalType,
}

impl DcsSchemaCalculatedField {
    pub fn new(
        data_path: CanonicalText,
        expression: CanonicalText,
        value_type: DcsSchemaDecimalType,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("calculated-field data path", &data_path)?;
        require_text("calculated-field expression", &expression)?;
        Ok(Self {
            data_path,
            expression,
            value_type,
        })
    }

    pub const fn data_path(&self) -> &CanonicalText {
        &self.data_path
    }

    pub const fn expression(&self) -> &CanonicalText {
        &self.expression
    }

    pub const fn value_type(&self) -> DcsSchemaDecimalType {
        self.value_type
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaCalculatedFieldWire {
    data_path: CanonicalText,
    expression: CanonicalText,
    value_type: DcsSchemaDecimalType,
}

impl<'de> Deserialize<'de> for DcsSchemaCalculatedField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaCalculatedFieldWire::deserialize(deserializer)?;
        Self::new(wire.data_path, wire.expression, wire.value_type).map_err(de::Error::custom)
    }
}

/// Closed aggregate-function set for the first ungrouped-total cohort.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsSchemaTotalFunction {
    Sum,
}

/// One direct, ungrouped total. Grouping collections are intentionally absent
/// from this type and therefore cannot be inferred by a writer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaUngroupedTotalField {
    data_path: CanonicalText,
    function: DcsSchemaTotalFunction,
}

impl DcsSchemaUngroupedTotalField {
    pub fn new(
        data_path: CanonicalText,
        function: DcsSchemaTotalFunction,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("total-field data path", &data_path)?;
        Ok(Self {
            data_path,
            function,
        })
    }

    pub const fn data_path(&self) -> &CanonicalText {
        &self.data_path
    }

    pub const fn function(&self) -> DcsSchemaTotalFunction {
        self.function
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaUngroupedTotalFieldWire {
    data_path: CanonicalText,
    function: DcsSchemaTotalFunction,
}

impl<'de> Deserialize<'de> for DcsSchemaUngroupedTotalField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaUngroupedTotalFieldWire::deserialize(deserializer)?;
        Self::new(wire.data_path, wire.function).map_err(de::Error::custom)
    }
}

/// One-entry localized string used by the attested parameter and variant
/// shells. Multi-language collections are outside this bounded cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaLocalString {
    language: CanonicalText,
    content: CanonicalText,
}

impl DcsSchemaLocalString {
    pub fn new(
        language: CanonicalText,
        content: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("localized-string language", &language)?;
        require_text("localized-string content", &content)?;
        if language.as_str() != "ru" {
            return Err(DcsSchemaBuildError::UnsupportedLanguage {
                language: language.as_str().to_owned(),
            });
        }
        Ok(Self { language, content })
    }

    pub const fn language(&self) -> &CanonicalText {
        &self.language
    }

    pub const fn content(&self) -> &CanonicalText {
        &self.content
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaLocalStringWire {
    language: CanonicalText,
    content: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaLocalString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaLocalStringWire::deserialize(deserializer)?;
        Self::new(wire.language, wire.content).map_err(de::Error::custom)
    }
}

/// One scalar string parameter. Its `useRestriction` semantic is fixed to the
/// attested `false`; collections of values and restriction modes are absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaStringParameter {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value_type: DcsSchemaStringType,
    value: CanonicalText,
}

impl DcsSchemaStringParameter {
    pub fn new(
        name: CanonicalText,
        title: DcsSchemaLocalString,
        value_type: DcsSchemaStringType,
        value: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("parameter name", &name)?;
        require_text("parameter scalar string value", &value)?;
        if value_type.length() != 40 {
            return Err(DcsSchemaBuildError::UnexpectedParameterStringLength {
                length: value_type.length(),
            });
        }
        Ok(Self {
            name,
            title,
            value_type,
            value,
        })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub const fn title(&self) -> &DcsSchemaLocalString {
        &self.title
    }

    pub const fn value_type(&self) -> DcsSchemaStringType {
        self.value_type
    }

    pub const fn value(&self) -> &CanonicalText {
        &self.value
    }

    pub const fn use_restriction(&self) -> bool {
        false
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaStringParameterWire {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value_type: DcsSchemaStringType,
    value: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaStringParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaStringParameterWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.title, wire.value_type, wire.value).map_err(de::Error::custom)
    }
}

/// `v8:StandardPeriodVariant` value authenticated for the initial 2214
/// parameter-scalar-types cohort. XML lexical spelling and namespace
/// prefixes remain schema/XML policy. Only `LastMonth` is evidenced; other
/// variants must fail closed at the schema/XML layer, not be represented
/// here.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsSchemaStandardPeriodVariant {
    LastMonth,
}

/// Decimal qualifiers for the `Лимит` scalar-parameter cohort. Kept
/// separate from [`DcsSchemaDecimalType`] (the Amount/DoubleAmount
/// data-set/calculated-field cohort's own, independently evidenced 15/2
/// pair): each digit/fraction pair is its own bounded, evidence-enumerated
/// coordinate, not a general decimal-qualifiers facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaParameterDecimalType {
    digits: u32,
    fraction_digits: u32,
}

impl DcsSchemaParameterDecimalType {
    pub fn new(digits: u32, fraction_digits: u32) -> Result<Self, DcsSchemaBuildError> {
        if digits == 0 {
            return Err(DcsSchemaBuildError::ZeroDecimalDigits);
        }
        if fraction_digits > digits {
            return Err(DcsSchemaBuildError::DecimalFractionExceedsDigits {
                digits,
                fraction_digits,
            });
        }
        if digits != 10 || fraction_digits != 2 {
            return Err(DcsSchemaBuildError::UnsupportedParameterDecimalQualifiers {
                digits,
                fraction_digits,
            });
        }
        Ok(Self {
            digits,
            fraction_digits,
        })
    }

    pub const fn digits(&self) -> u32 {
        self.digits
    }

    pub const fn fraction_digits(&self) -> u32 {
        self.fraction_digits
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaParameterDecimalTypeWire {
    digits: u32,
    fraction_digits: u32,
}

impl<'de> Deserialize<'de> for DcsSchemaParameterDecimalType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaParameterDecimalTypeWire::deserialize(deserializer)?;
        Self::new(wire.digits, wire.fraction_digits).map_err(de::Error::custom)
    }
}

/// The `Флаг` scalar boolean parameter authenticated by the dedicated 2214
/// parameter-scalar-types cohort. Its `useRestriction` semantic is fixed to
/// the attested `false`, matching [`DcsSchemaStringParameter`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaBooleanParameter {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value: bool,
}

impl DcsSchemaBooleanParameter {
    pub fn new(
        name: CanonicalText,
        title: DcsSchemaLocalString,
        value: bool,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("boolean parameter name", &name)?;
        Ok(Self { name, title, value })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub const fn title(&self) -> &DcsSchemaLocalString {
        &self.title
    }

    pub const fn value(&self) -> bool {
        self.value
    }

    pub const fn use_restriction(&self) -> bool {
        false
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaBooleanParameterWire {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value: bool,
}

impl<'de> Deserialize<'de> for DcsSchemaBooleanParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaBooleanParameterWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.title, wire.value).map_err(de::Error::custom)
    }
}

/// The `Лимит` scalar decimal parameter authenticated by the dedicated
/// 2214 parameter-scalar-types cohort. Its `useRestriction` semantic is
/// fixed to the attested `false`, matching [`DcsSchemaStringParameter`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaDecimalParameter {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value_type: DcsSchemaParameterDecimalType,
    value: CanonicalText,
}

impl DcsSchemaDecimalParameter {
    pub fn new(
        name: CanonicalText,
        title: DcsSchemaLocalString,
        value_type: DcsSchemaParameterDecimalType,
        value: CanonicalText,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("decimal parameter name", &name)?;
        require_text("decimal parameter value", &value)?;
        if value.as_str() != "100.5" {
            return Err(DcsSchemaBuildError::UnsupportedParameterDecimalValue {
                value: value.as_str().to_owned(),
            });
        }
        Ok(Self {
            name,
            title,
            value_type,
            value,
        })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub const fn title(&self) -> &DcsSchemaLocalString {
        &self.title
    }

    pub const fn value_type(&self) -> DcsSchemaParameterDecimalType {
        self.value_type
    }

    pub const fn value(&self) -> &CanonicalText {
        &self.value
    }

    pub const fn use_restriction(&self) -> bool {
        false
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaDecimalParameterWire {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    value_type: DcsSchemaParameterDecimalType,
    value: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSchemaDecimalParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaDecimalParameterWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.title, wire.value_type, wire.value).map_err(de::Error::custom)
    }
}

/// The `Период` scalar `v8:StandardPeriod` parameter authenticated by the
/// dedicated 2214 parameter-scalar-types cohort. Its `useRestriction`
/// semantic is fixed to the attested `false`, matching
/// [`DcsSchemaStringParameter`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaStandardPeriodParameter {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    variant: DcsSchemaStandardPeriodVariant,
}

impl DcsSchemaStandardPeriodParameter {
    pub fn new(
        name: CanonicalText,
        title: DcsSchemaLocalString,
        variant: DcsSchemaStandardPeriodVariant,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("StandardPeriod parameter name", &name)?;
        Ok(Self {
            name,
            title,
            variant,
        })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub const fn title(&self) -> &DcsSchemaLocalString {
        &self.title
    }

    pub const fn variant(&self) -> DcsSchemaStandardPeriodVariant {
        self.variant
    }

    pub const fn use_restriction(&self) -> bool {
        false
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaStandardPeriodParameterWire {
    name: CanonicalText,
    title: DcsSchemaLocalString,
    variant: DcsSchemaStandardPeriodVariant,
}

impl<'de> Deserialize<'de> for DcsSchemaStandardPeriodParameter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaStandardPeriodParameterWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.title, wire.variant).map_err(de::Error::custom)
    }
}

/// The three additional scalar-typed parameters (`Флаг`, `Лимит`,
/// `Период`) authenticated by the dedicated 2214 parameter-scalar-types
/// cohort. Always present as a group immediately after the base schema's
/// existing string `Caption` parameter, in this exact order (enforced by
/// [`DcsSchema::with_scalar_parameters`], not by this type itself).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaParameterScalarTypes {
    flag: DcsSchemaBooleanParameter,
    limit: DcsSchemaDecimalParameter,
    period: DcsSchemaStandardPeriodParameter,
}

impl DcsSchemaParameterScalarTypes {
    pub fn new(
        flag: DcsSchemaBooleanParameter,
        limit: DcsSchemaDecimalParameter,
        period: DcsSchemaStandardPeriodParameter,
    ) -> Result<Self, DcsSchemaBuildError> {
        Ok(Self {
            flag,
            limit,
            period,
        })
    }

    pub const fn flag(&self) -> &DcsSchemaBooleanParameter {
        &self.flag
    }

    pub const fn limit(&self) -> &DcsSchemaDecimalParameter {
        &self.limit
    }

    pub const fn period(&self) -> &DcsSchemaStandardPeriodParameter {
        &self.period
    }
}

/// Positional metadata shell for one externally delegated Settings document.
/// The Settings payload itself remains in the common settings/envelope layer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchemaSettingsVariantShell {
    name: CanonicalText,
    presentation: DcsSchemaLocalString,
}

impl DcsSchemaSettingsVariantShell {
    pub fn new(
        name: CanonicalText,
        presentation: DcsSchemaLocalString,
    ) -> Result<Self, DcsSchemaBuildError> {
        require_text("settings-variant name", &name)?;
        Ok(Self { name, presentation })
    }

    pub const fn name(&self) -> &CanonicalText {
        &self.name
    }

    pub const fn presentation(&self) -> &DcsSchemaLocalString {
        &self.presentation
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaSettingsVariantShellWire {
    name: CanonicalText,
    presentation: DcsSchemaLocalString,
}

impl<'de> Deserialize<'de> for DcsSchemaSettingsVariantShell {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaSettingsVariantShellWire::deserialize(deserializer)?;
        Self::new(wire.name, wire.presentation).map_err(de::Error::custom)
    }
}

/// Complete bounded semantic value for the first inner DCS schema cohort.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSchema {
    data_source: DcsSchemaLocalDataSource,
    data_set: DcsSchemaDataSetObject,
    calculated_field: Option<DcsSchemaCalculatedField>,
    total_fields: Vec<DcsSchemaUngroupedTotalField>,
    parameter: Option<DcsSchemaStringParameter>,
    /// The three additional scalar-typed parameters authenticated by the
    /// dedicated 2214 parameter-scalar-types cohort. Requires `parameter`
    /// (the existing `Caption` parameter) to already be present; see
    /// `with_scalar_parameters`.
    scalar_parameters: Option<DcsSchemaParameterScalarTypes>,
    settings_variants: Vec<DcsSchemaSettingsVariantShell>,
    provenance: SourceProvenance,
}

impl DcsSchema {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data_source: DcsSchemaLocalDataSource,
        data_set: DcsSchemaDataSetObject,
        calculated_field: DcsSchemaCalculatedField,
        total_fields: Vec<DcsSchemaUngroupedTotalField>,
        parameter: DcsSchemaStringParameter,
        settings_variants: Vec<DcsSchemaSettingsVariantShell>,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsSchemaBuildError> {
        Self::new_parts(
            data_source,
            data_set,
            Some(calculated_field),
            total_fields,
            Some(parameter),
            settings_variants,
            provenance,
        )
    }

    pub fn new_simple(
        data_source: DcsSchemaLocalDataSource,
        data_set: DcsSchemaDataSetObject,
        settings_variants: Vec<DcsSchemaSettingsVariantShell>,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsSchemaBuildError> {
        Self::new_parts(
            data_source,
            data_set,
            None,
            Vec::new(),
            None,
            settings_variants,
            provenance,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_parts(
        data_source: DcsSchemaLocalDataSource,
        data_set: DcsSchemaDataSetObject,
        calculated_field: Option<DcsSchemaCalculatedField>,
        total_fields: Vec<DcsSchemaUngroupedTotalField>,
        parameter: Option<DcsSchemaStringParameter>,
        settings_variants: Vec<DcsSchemaSettingsVariantShell>,
        provenance: SourceProvenance,
    ) -> Result<Self, DcsSchemaBuildError> {
        if data_set.data_source() != data_source.name() {
            return Err(DcsSchemaBuildError::DataSourceReferenceMismatch);
        }
        if calculated_field.as_ref().is_some_and(|calculated| {
            data_set
                .fields()
                .iter()
                .any(|field| field.data_path() == calculated.data_path())
        }) {
            return Err(DcsSchemaBuildError::DuplicateCalculatedFieldPath {
                path: calculated_field
                    .as_ref()
                    .expect("checked as present")
                    .data_path()
                    .as_str()
                    .to_owned(),
            });
        }
        let rich = data_set
            .fields()
            .get(1)
            .is_some_and(|field| matches!(field.value_type(), DcsSchemaFieldType::Decimal(_)));
        if rich != calculated_field.is_some() || rich != parameter.is_some() {
            return Err(DcsSchemaBuildError::UnexpectedDataSetFieldTypeOrder);
        }
        let expected_totals = if rich {
            DCS_SCHEMA_TOTAL_FIELD_COUNT
        } else {
            0
        };
        if total_fields.len() != expected_totals {
            return Err(DcsSchemaBuildError::UnexpectedTotalFieldCount {
                expected: expected_totals,
                actual: total_fields.len(),
            });
        }
        if rich {
            let decimal_path = data_set.fields()[1].data_path();
            if total_fields[0].data_path() != decimal_path
                || total_fields[1].data_path()
                    != calculated_field.as_ref().expect("rich cohort").data_path()
            {
                return Err(DcsSchemaBuildError::UnexpectedTotalFieldOrder);
            }
        }
        if settings_variants.is_empty() {
            return Err(DcsSchemaBuildError::EmptySettingsVariants);
        }
        if settings_variants.len() > MAX_DCS_SCHEMA_SETTINGS_VARIANTS {
            return Err(DcsSchemaBuildError::TooManySettingsVariants {
                maximum: MAX_DCS_SCHEMA_SETTINGS_VARIANTS,
                actual: settings_variants.len(),
            });
        }
        let mut variant_names = BTreeSet::new();
        for variant in &settings_variants {
            if !variant_names.insert(variant.name().as_str()) {
                return Err(DcsSchemaBuildError::DuplicateSettingsVariantName {
                    name: variant.name().as_str().to_owned(),
                });
            }
        }

        let schema = Self {
            data_source,
            data_set,
            calculated_field,
            total_fields,
            parameter,
            scalar_parameters: None,
            settings_variants,
            provenance,
        };
        validate_retained_bytes(&schema)?;
        Ok(schema)
    }

    /// Enables the exact three-parameter `Флаг`/`Лимит`/`Период` group
    /// authenticated by the dedicated 2214 parameter-scalar-types cohort.
    /// Requires the existing string `Caption` parameter to already be
    /// present (the evidenced insertion point is immediately after it).
    pub fn with_scalar_parameters(
        mut self,
        scalar_parameters: DcsSchemaParameterScalarTypes,
    ) -> Result<Self, DcsSchemaBuildError> {
        if self.parameter.is_none() {
            return Err(DcsSchemaBuildError::ScalarParametersRequireStringParameter);
        }
        self.scalar_parameters = Some(scalar_parameters);
        validate_retained_bytes(&self)?;
        Ok(self)
    }

    pub const fn data_source(&self) -> &DcsSchemaLocalDataSource {
        &self.data_source
    }

    pub const fn data_set(&self) -> &DcsSchemaDataSetObject {
        &self.data_set
    }

    pub const fn calculated_field(&self) -> Option<&DcsSchemaCalculatedField> {
        self.calculated_field.as_ref()
    }

    pub fn total_fields(&self) -> &[DcsSchemaUngroupedTotalField] {
        &self.total_fields
    }

    pub const fn parameter(&self) -> Option<&DcsSchemaStringParameter> {
        self.parameter.as_ref()
    }

    pub const fn scalar_parameters(&self) -> Option<&DcsSchemaParameterScalarTypes> {
        self.scalar_parameters.as_ref()
    }

    pub fn settings_variants(&self) -> &[DcsSchemaSettingsVariantShell] {
        &self.settings_variants
    }

    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSchemaWire {
    data_source: DcsSchemaLocalDataSource,
    data_set: DcsSchemaDataSetObject,
    calculated_field: Option<DcsSchemaCalculatedField>,
    total_fields: BoundedDcsSchemaVec<DcsSchemaUngroupedTotalField, DCS_SCHEMA_TOTAL_FIELD_COUNT>,
    parameter: Option<DcsSchemaStringParameter>,
    #[serde(default)]
    scalar_parameters: Option<DcsSchemaParameterScalarTypes>,
    settings_variants:
        BoundedDcsSchemaVec<DcsSchemaSettingsVariantShell, MAX_DCS_SCHEMA_SETTINGS_VARIANTS>,
    provenance: SourceProvenance,
}

impl<'de> Deserialize<'de> for DcsSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSchemaWire::deserialize(deserializer)?;
        let schema = Self::new_parts(
            wire.data_source,
            wire.data_set,
            wire.calculated_field,
            wire.total_fields.values,
            wire.parameter,
            wire.settings_variants.values,
            wire.provenance,
        )
        .map_err(de::Error::custom)?;
        match wire.scalar_parameters {
            Some(scalar_parameters) => schema
                .with_scalar_parameters(scalar_parameters)
                .map_err(de::Error::custom),
            None => Ok(schema),
        }
    }
}

struct BoundedDcsSchemaVec<T, const MAXIMUM: usize> {
    values: Vec<T>,
}

struct BoundedDcsSchemaVecVisitor<T, const MAXIMUM: usize>(PhantomData<fn() -> T>);

impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedDcsSchemaVecVisitor<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    type Value = BoundedDcsSchemaVec<T, MAXIMUM>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a bounded DCS schema collection of at most {MAXIMUM} items"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(MAXIMUM));
        while values.len() < MAXIMUM {
            let Some(value) = sequence.next_element::<T>()? else {
                return Ok(BoundedDcsSchemaVec { values });
            };
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS schema collection exceeds {MAXIMUM} items"
            )));
        }
        Ok(BoundedDcsSchemaVec { values })
    }
}

impl<'de, T, const MAXIMUM: usize> Deserialize<'de> for BoundedDcsSchemaVec<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedDcsSchemaVecVisitor(PhantomData))
    }
}

fn require_text(field: &'static str, value: &CanonicalText) -> Result<(), DcsSchemaBuildError> {
    if value.as_str().is_empty() {
        return Err(DcsSchemaBuildError::EmptyText { field });
    }
    Ok(())
}

fn validate_retained_bytes(schema: &DcsSchema) -> Result<(), DcsSchemaBuildError> {
    let mut retained = schema.provenance.retained_byte_len();
    retained = add_retained(retained, schema.data_source.name().as_str().len())?;
    retained = add_retained(retained, schema.data_set.name().as_str().len())?;
    retained = add_retained(retained, schema.data_set.data_source().as_str().len())?;
    retained = add_retained(retained, schema.data_set.object_name().as_str().len())?;
    for field in schema.data_set.fields() {
        retained = add_retained(retained, field.data_path().as_str().len())?;
        retained = add_retained(retained, field.field().as_str().len())?;
        if let DcsSchemaFieldType::Reference(reference) = field.value_type() {
            retained = add_retained(retained, reference.qualified_name().as_str().len())?;
        }
    }
    if let Some(calculated) = &schema.calculated_field {
        retained = add_retained(retained, calculated.data_path().as_str().len())?;
        retained = add_retained(retained, calculated.expression().as_str().len())?;
    }
    for total in &schema.total_fields {
        retained = add_retained(retained, total.data_path().as_str().len())?;
    }
    if let Some(parameter) = &schema.parameter {
        retained = add_retained(retained, parameter.name().as_str().len())?;
        retained = add_retained(retained, parameter.title().language().as_str().len())?;
        retained = add_retained(retained, parameter.title().content().as_str().len())?;
        retained = add_retained(retained, parameter.value().as_str().len())?;
    }
    if let Some(scalar_parameters) = &schema.scalar_parameters {
        let flag = scalar_parameters.flag();
        retained = add_retained(retained, flag.name().as_str().len())?;
        retained = add_retained(retained, flag.title().language().as_str().len())?;
        retained = add_retained(retained, flag.title().content().as_str().len())?;
        let limit = scalar_parameters.limit();
        retained = add_retained(retained, limit.name().as_str().len())?;
        retained = add_retained(retained, limit.title().language().as_str().len())?;
        retained = add_retained(retained, limit.title().content().as_str().len())?;
        retained = add_retained(retained, limit.value().as_str().len())?;
        let period = scalar_parameters.period();
        retained = add_retained(retained, period.name().as_str().len())?;
        retained = add_retained(retained, period.title().language().as_str().len())?;
        retained = add_retained(retained, period.title().content().as_str().len())?;
    }
    for variant in &schema.settings_variants {
        retained = add_retained(retained, variant.name().as_str().len())?;
        retained = add_retained(retained, variant.presentation().language().as_str().len())?;
        retained = add_retained(retained, variant.presentation().content().as_str().len())?;
    }
    Ok(())
}

fn validate_query_union_link_retained_bytes(
    model: &DcsSchemaQueryUnionLink,
) -> Result<(), DcsSchemaBuildError> {
    let mut retained = model.provenance.retained_byte_len();
    for value in [
        model.data_source.name(),
        model.query.name(),
        model.query.field.data_path(),
        model.query.field.field(),
        model.query.data_source(),
        model.query.query(),
        model.union.name(),
        model.union.item.name(),
        model.link.source_data_set(),
        model.link.destination_data_set(),
        model.link.source_expression(),
        model.link.destination_expression(),
    ] {
        retained = add_retained(retained, value.as_str().len())?;
    }
    for variant in &model.settings_variants {
        retained = add_retained(retained, variant.name().as_str().len())?;
        retained = add_retained(retained, variant.presentation().language().as_str().len())?;
        retained = add_retained(retained, variant.presentation().content().as_str().len())?;
    }
    if retained > MAX_DCS_SCHEMA_RETAINED_BYTES {
        return Err(DcsSchemaBuildError::RetainedBytesExceeded {
            maximum: MAX_DCS_SCHEMA_RETAINED_BYTES,
            actual: retained,
        });
    }
    Ok(())
}

fn add_retained(current: usize, additional: usize) -> Result<usize, DcsSchemaBuildError> {
    let actual = current
        .checked_add(additional)
        .ok_or(DcsSchemaBuildError::RetainedByteCountOverflow)?;
    if actual > MAX_DCS_SCHEMA_RETAINED_BYTES {
        return Err(DcsSchemaBuildError::RetainedBytesExceeded {
            maximum: MAX_DCS_SCHEMA_RETAINED_BYTES,
            actual,
        });
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ProfileId;
    use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use crate::provenance::CanonicalAnchor;

    fn text(value: &str) -> CanonicalText {
        CanonicalText::new(value).unwrap()
    }

    fn provenance() -> SourceProvenance {
        SourceProvenance::with_locator(
            ProfileId::parse("platform:8.3.27").unwrap(),
            CanonicalAnchor::new(
                ObjectPath::new(vec![PathSegment::name("dcs_schema").unwrap()]).unwrap(),
                PropertyPath::root(),
            ),
            "fixture:dcs-core/Template.xml",
        )
        .unwrap()
    }

    fn string_field() -> DcsSchemaDataSetField {
        DcsSchemaDataSetField::new(
            text("Name"),
            text("Name"),
            DcsSchemaFieldType::String(DcsSchemaStringType::new(20).unwrap()),
        )
        .unwrap()
    }

    fn decimal_field() -> DcsSchemaDataSetField {
        DcsSchemaDataSetField::new(
            text("Amount"),
            text("Amount"),
            DcsSchemaFieldType::Decimal(DcsSchemaDecimalType::new(15, 2).unwrap()),
        )
        .unwrap()
    }

    fn reference_field() -> DcsSchemaDataSetField {
        DcsSchemaDataSetField::new(
            text("Owner"),
            text("Owner"),
            DcsSchemaFieldType::Reference(
                DcsSchemaReferenceType::new(text("CatalogRef.FilterProbe")).unwrap(),
            ),
        )
        .unwrap()
    }

    #[test]
    fn query_union_link_graph_is_bounded_cross_checked_and_serde_stable() {
        let field = DcsSchemaQueryField::new(text("SortKey"), text("SortKey")).unwrap();
        let query = DcsSchemaQueryDataSet::new(
            text("QueryRows"),
            field.clone(),
            None,
            text("ИсточникДанных1"),
            text("ВЫБРАТЬ \"A\" КАК SortKey"),
        )
        .unwrap();
        let item = DcsSchemaQueryDataSet::new(
            text("UnionPart"),
            field,
            None,
            text("ИсточникДанных1"),
            text("ВЫБРАТЬ \"A\" КАК SortKey"),
        )
        .unwrap();
        let union = DcsSchemaUnionDataSet::new(text("UnionRows"), item).unwrap();
        let link = DcsSchemaDataSetLink::new(
            text("QueryRows"),
            text("UnionRows"),
            text("SortKey"),
            text("SortKey"),
        )
        .unwrap();
        let model = DcsSchemaQueryUnionLink::new(
            DcsSchemaLocalDataSource::new(text("ИсточникДанных1")).unwrap(),
            query,
            union,
            link,
            vec![variant("Main")],
            provenance(),
        )
        .unwrap();
        let json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaQueryUnionLink>(&json).unwrap(),
            model
        );
        let mut drift = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        drift["link"]["destination_expression"] = serde_json::json!("Other");
        assert!(serde_json::from_value::<DcsSchemaQueryUnionLink>(drift).is_err());
    }

    /// `dcs-query-union-link-typeid` cohort: the query data set's second,
    /// evidenced `typed_field` (reusing the same `DcsSchemaDataSetField`
    /// shape and reference resolution the `dcs-typeid-reference`
    /// DataSetObject cohort already proved). `None` (the base
    /// `dcs-query-union-link` cohort) and `Some` (this cohort) must both
    /// stay serde-stable and distinguishable; the `DataSetUnion` item
    /// position has no evidence for a typed field and keeps `None`.
    #[test]
    fn query_data_set_typed_field_is_bounded_and_serde_stable() {
        let field = DcsSchemaQueryField::new(text("SortKey"), text("SortKey")).unwrap();
        let query = DcsSchemaQueryDataSet::new(
            text("QueryRows"),
            field.clone(),
            Some(reference_field()),
            text("ИсточникДанных1"),
            text("ВЫБРАТЬ \"A\" КАК SortKey"),
        )
        .unwrap();
        assert_eq!(
            query.typed_field().map(DcsSchemaDataSetField::field),
            Some(&text("Owner"))
        );
        let item = DcsSchemaQueryDataSet::new(
            text("UnionPart"),
            field,
            None,
            text("ИсточникДанных1"),
            text("ВЫБРАТЬ \"A\" КАК SortKey"),
        )
        .unwrap();
        assert!(item.typed_field().is_none());
        let union = DcsSchemaUnionDataSet::new(text("UnionRows"), item).unwrap();
        let link = DcsSchemaDataSetLink::new(
            text("QueryRows"),
            text("UnionRows"),
            text("SortKey"),
            text("SortKey"),
        )
        .unwrap();
        let model = DcsSchemaQueryUnionLink::new(
            DcsSchemaLocalDataSource::new(text("ИсточникДанных1")).unwrap(),
            query,
            union,
            link,
            vec![variant("Main")],
            provenance(),
        )
        .unwrap();
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("\"typed_field\":{"));
        assert_eq!(
            serde_json::from_str::<DcsSchemaQueryUnionLink>(&json).unwrap(),
            model
        );
        let mut drift = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        drift["query"]["typed_field"]
            .as_object_mut()
            .unwrap()
            .remove("value_type");
        assert!(serde_json::from_value::<DcsSchemaQueryUnionLink>(drift).is_err());
    }

    #[test]
    fn data_set_link_optional_fields_are_grouped_bounded_and_serde_stable() {
        let base = DcsSchemaDataSetLink::new(
            text("QueryRows"),
            text("UnionRows"),
            text("SortKey"),
            text("SortKey"),
        )
        .unwrap();
        assert_eq!(base.parameter(), None);
        assert_eq!(base.parameter_list_allowed(), None);
        assert_eq!(base.link_condition_expression(), None);
        assert_eq!(base.start_expression(), None);
        assert_eq!(base.required(), None);

        // dcs-link-parameter cohort: parameter + parameterListAllowed only.
        let with_parameter = base
            .clone()
            .with_parameter(text("LinkParam"), true)
            .unwrap();
        assert_eq!(with_parameter.parameter(), Some(&text("LinkParam")));
        assert_eq!(with_parameter.parameter_list_allowed(), Some(true));
        assert_eq!(with_parameter.link_condition_expression(), None);
        let json = serde_json::to_string(&with_parameter).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaDataSetLink>(&json).unwrap(),
            with_parameter
        );

        // dcs-link-expressions cohort: the full 6-field state.
        let with_expressions = with_parameter
            .clone()
            .with_expressions(text("SortKey > 0"), text("SortKey"), false)
            .unwrap();
        assert_eq!(
            with_expressions.link_condition_expression(),
            Some(&text("SortKey > 0"))
        );
        assert_eq!(with_expressions.start_expression(), Some(&text("SortKey")));
        assert_eq!(with_expressions.required(), Some(false));
        let json = serde_json::to_string(&with_expressions).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaDataSetLink>(&json).unwrap(),
            with_expressions
        );
        assert_ne!(with_expressions, with_parameter);

        // The expressions triple has no evidenced state without the
        // parameter pair already present.
        assert!(matches!(
            base.clone()
                .with_expressions(text("SortKey > 0"), text("SortKey"), false),
            Err(DcsSchemaBuildError::LinkExpressionsRequireParameter)
        ));

        // Wire-level partial presence (one of a co-occurring pair/triple
        // present without the rest) must fail closed, not silently default.
        let mut partial_pair: serde_json::Value = serde_json::from_str(&json).unwrap();
        partial_pair["parameter_list_allowed"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<DcsSchemaDataSetLink>(partial_pair).is_err());
        let mut partial_triple: serde_json::Value = serde_json::from_str(&json).unwrap();
        partial_triple["required"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<DcsSchemaDataSetLink>(partial_triple).is_err());
    }

    #[test]
    fn style_free_area_template_is_bounded_and_serde_stable() {
        let value = DcsSchemaAreaTemplate::new(
            text("AreaProbe"),
            text("Probe"),
            text("\"Probe\""),
            provenance(),
        )
        .unwrap();
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            value
        );
        let mut drift: serde_json::Value = serde_json::from_str(&json).unwrap();
        drift["expression"] = serde_json::json!("Other");
        assert!(serde_json::from_value::<DcsSchemaAreaTemplate>(drift).is_err());

        let styled = value.with_parameter_appearance();
        assert!(styled.has_parameter_appearance());
        let json = serde_json::to_string(&styled).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            styled
        );
    }

    #[test]
    fn style_free_area_template_color_appearance_is_bounded_and_serde_stable() {
        let value = DcsSchemaAreaTemplate::new(
            text("AreaProbe"),
            text("Probe"),
            text("\"Probe\""),
            provenance(),
        )
        .unwrap();
        let colored = value
            .clone()
            .with_color_and_parameter_appearance(DcsAppearanceColor::WebRed);
        assert!(colored.has_parameter_appearance());
        assert_eq!(
            colored.text_color_appearance(),
            Some(DcsAppearanceColor::WebRed)
        );
        let json = serde_json::to_string(&colored).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            colored
        );

        let parameter_only = value.with_parameter_appearance();
        assert_eq!(parameter_only.text_color_appearance(), None);
        assert_ne!(parameter_only, colored);
    }

    #[test]
    fn style_free_area_template_style_reference_appearance_is_bounded_and_serde_stable() {
        let value = DcsSchemaAreaTemplate::new(
            text("AreaProbe"),
            text("Probe"),
            text("\"Probe\""),
            provenance(),
        )
        .unwrap();
        let named = value.clone().with_style_reference_and_parameter_appearance(
            DcsStyleColorReference::named(text("NegativeTextColor")).unwrap(),
        );
        assert!(named.has_parameter_appearance());
        assert_eq!(
            named.back_color_style_reference(),
            Some(&DcsStyleColorReference::Named(text("NegativeTextColor")))
        );
        assert_eq!(named.text_color_appearance(), None);
        let json = serde_json::to_string(&named).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            named
        );

        let custom = value.clone().with_style_reference_and_parameter_appearance(
            DcsStyleColorReference::custom_style_item(text("CorpusAccent")).unwrap(),
        );
        assert_eq!(
            custom.back_color_style_reference(),
            Some(&DcsStyleColorReference::CustomStyleItem(text(
                "CorpusAccent"
            )))
        );
        assert_ne!(custom, named);
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            custom
        );

        // Combining text_color_appearance and back_color_style_reference on
        // the wire is rejected: no cohort proves the two color parameters
        // co-occurring.
        let mut combined: serde_json::Value = serde_json::to_value(&named).unwrap();
        combined["text_color_appearance"] = serde_json::json!("web_red");
        assert!(serde_json::from_value::<DcsSchemaAreaTemplate>(combined).is_err());

        // Empty style-reference names are rejected.
        assert!(DcsStyleColorReference::named(text("")).is_err());
        assert!(DcsStyleColorReference::custom_style_item(text("")).is_err());

        let parameter_only = value.with_parameter_appearance();
        assert_eq!(parameter_only.back_color_style_reference(), None);
        assert_ne!(parameter_only, named);
    }

    #[test]
    fn style_free_area_template_shared_row_appearance_is_bounded_and_serde_stable() {
        let value = DcsSchemaAreaTemplate::new(
            text("AreaProbe"),
            text("Probe"),
            text("\"Probe\""),
            provenance(),
        )
        .unwrap();
        let shared = value.clone().with_shared_row_appearance();
        assert!(shared.has_shared_row_appearance());
        // The shared-row body replaces the single-cell appearance state
        // entirely; it does not also flip the single-cell flags.
        assert!(!shared.has_parameter_appearance());
        assert_eq!(shared.text_color_appearance(), None);
        let json = serde_json::to_string(&shared).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchemaAreaTemplate>(&json).unwrap(),
            shared
        );
        assert_ne!(shared, value);
    }

    fn variant(name: &str) -> DcsSchemaSettingsVariantShell {
        DcsSchemaSettingsVariantShell::new(
            text(name),
            DcsSchemaLocalString::new(text("ru"), text(name)).unwrap(),
        )
        .unwrap()
    }

    fn schema(variants: Vec<DcsSchemaSettingsVariantShell>) -> DcsSchema {
        DcsSchema::new(
            DcsSchemaLocalDataSource::new(text("ИсточникДанных1")).unwrap(),
            DcsSchemaDataSetObject::new(
                text("Rows"),
                vec![string_field(), decimal_field()],
                text("ИсточникДанных1"),
                text("Rows"),
            )
            .unwrap(),
            DcsSchemaCalculatedField::new(
                text("DoubleAmount"),
                text("Amount * 2"),
                DcsSchemaDecimalType::new(15, 2).unwrap(),
            )
            .unwrap(),
            vec![
                DcsSchemaUngroupedTotalField::new(text("Amount"), DcsSchemaTotalFunction::Sum)
                    .unwrap(),
                DcsSchemaUngroupedTotalField::new(
                    text("DoubleAmount"),
                    DcsSchemaTotalFunction::Sum,
                )
                .unwrap(),
            ],
            DcsSchemaStringParameter::new(
                text("Caption"),
                DcsSchemaLocalString::new(text("ru"), text("Caption")).unwrap(),
                DcsSchemaStringType::new(40).unwrap(),
                text("DCS corpus"),
            )
            .unwrap(),
            variants,
            provenance(),
        )
        .unwrap()
    }

    #[test]
    fn attested_one_variant_cohort_is_typed_and_serde_stable() {
        let schema = schema(vec![variant("Main")]);
        assert_eq!(schema.data_source().name().as_str(), "ИсточникДанных1");
        assert!(matches!(
            schema.data_set().fields()[0].value_type(),
            DcsSchemaFieldType::String(value) if value.length() == 20
        ));
        assert!(matches!(
            schema.data_set().fields()[1].value_type(),
            DcsSchemaFieldType::Decimal(value)
                if value.digits() == 15 && value.fraction_digits() == 2
        ));
        assert_eq!(schema.total_fields().len(), 2);
        assert!(!schema.parameter().unwrap().use_restriction());
        assert_eq!(schema.settings_variants()[0].name().as_str(), "Main");

        let json = serde_json::to_string(&schema).unwrap();
        assert_eq!(serde_json::from_str::<DcsSchema>(&json).unwrap(), schema);
        assert_eq!(serde_json::to_string(&schema).unwrap(), json);
    }

    #[test]
    fn reference_field_is_semantic_bounded_and_serde_stable() {
        let schema = DcsSchema::new_simple(
            DcsSchemaLocalDataSource::new(text("ИсточникДанных1")).unwrap(),
            DcsSchemaDataSetObject::new(
                text("ProbeData"),
                vec![string_field(), reference_field()],
                text("ИсточникДанных1"),
                text("ProbeData"),
            )
            .unwrap(),
            vec![variant("Main")],
            provenance(),
        )
        .unwrap();
        assert!(matches!(
            schema.data_set().fields()[1].value_type(),
            DcsSchemaFieldType::Reference(reference)
                if reference.qualified_name().as_str() == "CatalogRef.FilterProbe"
        ));
        let json = serde_json::to_string(&schema).unwrap();
        assert_eq!(serde_json::from_str::<DcsSchema>(&json).unwrap(), schema);
    }

    #[test]
    fn two_variant_shells_preserve_positional_order() {
        let schema = schema(vec![variant("Main"), variant("Secondary Secondary")]);
        assert_eq!(
            schema
                .settings_variants()
                .iter()
                .map(|variant| variant.name().as_str())
                .collect::<Vec<_>>(),
            ["Main", "Secondary Secondary"]
        );
        assert_eq!(
            serde_json::from_str::<DcsSchema>(&serde_json::to_string(&schema).unwrap()).unwrap(),
            schema
        );
    }

    #[test]
    fn type_qualifiers_and_required_text_fail_closed() {
        assert_eq!(
            DcsSchemaStringType::new(0),
            Err(DcsSchemaBuildError::ZeroStringLength)
        );
        assert_eq!(
            DcsSchemaDecimalType::new(0, 0),
            Err(DcsSchemaBuildError::ZeroDecimalDigits)
        );
        assert_eq!(
            DcsSchemaDecimalType::new(2, 3),
            Err(DcsSchemaBuildError::DecimalFractionExceedsDigits {
                digits: 2,
                fraction_digits: 3,
            })
        );
        assert_eq!(
            DcsSchemaStringType::new(21),
            Err(DcsSchemaBuildError::UnsupportedStringLength { length: 21 })
        );
        assert_eq!(
            DcsSchemaDecimalType::new(10, 2),
            Err(DcsSchemaBuildError::UnsupportedDecimalQualifiers {
                digits: 10,
                fraction_digits: 2,
            })
        );
        assert!(matches!(
            DcsSchemaLocalString::new(text("en"), text("Caption")),
            Err(DcsSchemaBuildError::UnsupportedLanguage { .. })
        ));
        assert!(matches!(
            DcsSchemaLocalDataSource::new(text("")),
            Err(DcsSchemaBuildError::EmptyText { .. })
        ));
    }

    #[test]
    fn object_data_set_accepts_attested_simple_or_rich_shape_and_unique_paths() {
        let one = DcsSchemaDataSetObject::new(
            text("Rows"),
            vec![string_field()],
            text("Source"),
            text("Rows"),
        );
        assert_eq!(one.unwrap().fields().len(), 1);

        let reversed = DcsSchemaDataSetObject::new(
            text("Rows"),
            vec![decimal_field(), string_field()],
            text("Source"),
            text("Rows"),
        );
        assert_eq!(
            reversed,
            Err(DcsSchemaBuildError::UnexpectedDataSetFieldTypeOrder)
        );

        let string_40 = DcsSchemaDataSetField::new(
            text("Name"),
            text("Name"),
            DcsSchemaFieldType::String(DcsSchemaStringType::new(40).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            DcsSchemaDataSetObject::new(
                text("Rows"),
                vec![string_40, decimal_field()],
                text("Source"),
                text("Rows"),
            ),
            Err(DcsSchemaBuildError::UnexpectedDataSetStringLength { length: 40 })
        ));

        let duplicate_decimal = DcsSchemaDataSetField::new(
            text("Name"),
            text("Amount"),
            DcsSchemaFieldType::Decimal(DcsSchemaDecimalType::new(15, 2).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            DcsSchemaDataSetObject::new(
                text("Rows"),
                vec![string_field(), duplicate_decimal],
                text("Source"),
                text("Rows"),
            ),
            Err(DcsSchemaBuildError::DuplicateDataSetFieldPath { .. })
        ));
    }

    #[test]
    fn schema_cross_references_totals_and_variants_are_strict() {
        let valid = schema(vec![variant("Main")]);
        let mut json = serde_json::to_value(&valid).unwrap();
        json["data_set"]["data_source"] = serde_json::json!("Other");
        assert!(
            serde_json::from_value::<DcsSchema>(json)
                .unwrap_err()
                .to_string()
                .contains("different local data source")
        );

        let mut json = serde_json::to_value(&valid).unwrap();
        json["calculated_field"]["data_path"] = serde_json::json!("Amount");
        assert!(
            serde_json::from_value::<DcsSchema>(json)
                .unwrap_err()
                .to_string()
                .contains("duplicates a data-set field")
        );

        let mut json = serde_json::to_value(&valid).unwrap();
        json["total_fields"].as_array_mut().unwrap().reverse();
        assert!(
            serde_json::from_value::<DcsSchema>(json)
                .unwrap_err()
                .to_string()
                .contains("totals must target")
        );

        assert!(matches!(
            DcsSchema::new(
                valid.data_source.clone(),
                valid.data_set.clone(),
                valid.calculated_field.clone().unwrap(),
                valid.total_fields.clone(),
                valid.parameter.clone().unwrap(),
                Vec::new(),
                valid.provenance.clone(),
            ),
            Err(DcsSchemaBuildError::EmptySettingsVariants)
        ));
        assert!(matches!(
            DcsSchema::new(
                valid.data_source.clone(),
                valid.data_set.clone(),
                valid.calculated_field.clone().unwrap(),
                valid.total_fields.clone(),
                valid.parameter.clone().unwrap(),
                vec![variant("Main"), variant("Main")],
                valid.provenance.clone(),
            ),
            Err(DcsSchemaBuildError::DuplicateSettingsVariantName { .. })
        ));
    }

    fn scalar_parameters() -> DcsSchemaParameterScalarTypes {
        DcsSchemaParameterScalarTypes::new(
            DcsSchemaBooleanParameter::new(
                text("Флаг"),
                DcsSchemaLocalString::new(text("ru"), text("Флаг")).unwrap(),
                true,
            )
            .unwrap(),
            DcsSchemaDecimalParameter::new(
                text("Лимит"),
                DcsSchemaLocalString::new(text("ru"), text("Лимит")).unwrap(),
                DcsSchemaParameterDecimalType::new(10, 2).unwrap(),
                text("100.5"),
            )
            .unwrap(),
            DcsSchemaStandardPeriodParameter::new(
                text("Период"),
                DcsSchemaLocalString::new(text("ru"), text("Период")).unwrap(),
                DcsSchemaStandardPeriodVariant::LastMonth,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn scalar_parameters_are_bounded_and_serde_stable() {
        let with_scalars = schema(vec![variant("Main")])
            .with_scalar_parameters(scalar_parameters())
            .unwrap();
        let scalars = with_scalars.scalar_parameters().unwrap();
        assert!(scalars.flag().value());
        assert_eq!(scalars.limit().value().as_str(), "100.5");
        assert_eq!(scalars.limit().value_type().digits(), 10);
        assert_eq!(scalars.limit().value_type().fraction_digits(), 2);
        assert_eq!(
            scalars.period().variant(),
            DcsSchemaStandardPeriodVariant::LastMonth
        );

        let json = serde_json::to_string(&with_scalars).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchema>(&json).unwrap(),
            with_scalars
        );

        // Without the group, the schema round-trips as before (backward
        // compatible: `scalar_parameters` defaults to absent on wire).
        let without_scalars = schema(vec![variant("Main")]);
        assert!(without_scalars.scalar_parameters().is_none());
        let json = serde_json::to_string(&without_scalars).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSchema>(&json).unwrap(),
            without_scalars
        );
    }

    #[test]
    fn scalar_parameters_require_existing_string_parameter() {
        let simple = DcsSchema::new_simple(
            DcsSchemaLocalDataSource::new(text("ИсточникДанных1")).unwrap(),
            DcsSchemaDataSetObject::new(
                text("Rows"),
                vec![string_field()],
                text("ИсточникДанных1"),
                text("Rows"),
            )
            .unwrap(),
            vec![variant("Main")],
            provenance(),
        )
        .unwrap();
        assert!(matches!(
            simple.with_scalar_parameters(scalar_parameters()),
            Err(DcsSchemaBuildError::ScalarParametersRequireStringParameter)
        ));
    }

    #[test]
    fn parameter_decimal_type_rejects_unevidenced_qualifiers() {
        assert!(matches!(
            DcsSchemaParameterDecimalType::new(15, 2),
            Err(DcsSchemaBuildError::UnsupportedParameterDecimalQualifiers { .. })
        ));
        assert!(matches!(
            DcsSchemaParameterDecimalType::new(0, 0),
            Err(DcsSchemaBuildError::ZeroDecimalDigits)
        ));
        assert!(matches!(
            DcsSchemaParameterDecimalType::new(1, 5),
            Err(DcsSchemaBuildError::DecimalFractionExceedsDigits { .. })
        ));
    }

    #[test]
    fn decimal_parameter_rejects_unevidenced_value() {
        assert!(matches!(
            DcsSchemaDecimalParameter::new(
                text("Лимит"),
                DcsSchemaLocalString::new(text("ru"), text("Лимит")).unwrap(),
                DcsSchemaParameterDecimalType::new(10, 2).unwrap(),
                text("7"),
            ),
            Err(DcsSchemaBuildError::UnsupportedParameterDecimalValue { .. })
        ));
    }

    #[test]
    fn public_serde_is_bounded_revalidating_and_denies_unknown_fields() {
        let valid = schema(vec![variant("Main")]);
        let mut unknown = serde_json::to_value(&valid).unwrap();
        unknown["guessed_qname"] = serde_json::json!("DataCompositionSchema");
        assert!(serde_json::from_value::<DcsSchema>(unknown).is_err());

        let mut too_many_variants = serde_json::to_value(&valid).unwrap();
        too_many_variants["settings_variants"] =
            serde_json::json!([variant("A"), variant("B"), variant("C")]);
        assert!(
            serde_json::from_value::<DcsSchema>(too_many_variants)
                .unwrap_err()
                .to_string()
                .contains("exceeds 2 items")
        );

        let mut too_many_fields = serde_json::to_value(&valid).unwrap();
        too_many_fields["data_set"]["fields"] =
            serde_json::json!([string_field(), decimal_field(), decimal_field()]);
        assert!(
            serde_json::from_value::<DcsSchema>(too_many_fields)
                .unwrap_err()
                .to_string()
                .contains("exceeds 2 items")
        );

        let mut invalid_qualifier = serde_json::to_value(&valid).unwrap();
        invalid_qualifier["data_set"]["fields"][0]["value_type"]["string"]["length"] =
            serde_json::json!(0);
        assert!(
            serde_json::from_value::<DcsSchema>(invalid_qualifier)
                .unwrap_err()
                .to_string()
                .contains("string length must be positive")
        );
    }

    #[test]
    fn retained_byte_arithmetic_is_fail_closed() {
        assert_eq!(
            add_retained(MAX_DCS_SCHEMA_RETAINED_BYTES, 1),
            Err(DcsSchemaBuildError::RetainedBytesExceeded {
                maximum: MAX_DCS_SCHEMA_RETAINED_BYTES,
                actual: MAX_DCS_SCHEMA_RETAINED_BYTES + 1,
            })
        );
        assert_eq!(
            add_retained(usize::MAX, 1),
            Err(DcsSchemaBuildError::RetainedByteCountOverflow)
        );
    }
}
