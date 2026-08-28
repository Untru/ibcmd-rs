//! Bounded canonical data-composition settings.
//!
//! This module deliberately models only the verified minimum shared by a
//! standalone settings document and Form `ListSettings`. XML names, type IDs,
//! and writer order do not belong to this IR. A decoder must retain every
//! unsupported extension as an exact [`OpaqueFacet`] instead of guessing a
//! typed interpretation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::opaque::{OpaqueFacet, OpaqueFacets};
use crate::provenance::SourceProvenance;
use crate::value::{CanonicalText, EnumToken};

/// Maximum opaque XML extensions retained by one DCS settings value.
pub const MAX_DCS_OPAQUE_EXTENSIONS: usize = 4_096;
/// Maximum selected items retained by one DCS settings selection.
pub const MAX_DCS_SELECTION_ITEMS: usize = 16_384;
/// Maximum order items retained by one DCS settings order.
pub const MAX_DCS_ORDER_ITEMS: usize = 16_384;
/// Maximum filter items retained by one DCS settings filter.
pub const MAX_DCS_FILTER_ITEMS: usize = 16_384;
/// Maximum conditional-appearance items admitted by the first authenticated
/// standalone/Form cohort.
pub const MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS: usize = 1;
/// Maximum aggregate variable-sized data retained by one DCS settings value.
pub const MAX_DCS_RETAINED_BYTES: usize = 67_108_864;

/// Failure to construct or revalidate bounded canonical DCS settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsBuildError {
    /// A DCS selection was present without any selected items.
    EmptySelection,
    /// A selected field did not contain a field path.
    EmptySelectionField,
    /// A DCS selection exceeded its direct item bound.
    TooManySelectionItems {
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items.
        actual: usize,
    },
    /// A DCS order was present without items or supported container metadata.
    EmptyOrder,
    /// An order field did not contain a field path.
    EmptyOrderField,
    /// A DCS order exceeded its direct item bound.
    TooManyOrderItems {
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items.
        actual: usize,
    },
    /// The only observed explicit `use` value is `false`; `true` must not be
    /// normalized into an omission without default evidence.
    UnsupportedOrderUseTrue,
    /// A DCS filter was present without items or the complete evidenced
    /// metadata-only pair.
    EmptyFilter,
    /// A comparison left operand did not contain a field path.
    EmptyFilterField,
    /// The first evidenced right operand is a non-empty string.
    EmptyFilterStringValue,
    /// A DCS filter exceeded its direct item bound.
    TooManyFilterItems {
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items.
        actual: usize,
    },
    /// No explicit `use` value is authenticated for the first filter cohort.
    UnsupportedFilterUse,
    /// A conditional-appearance container had neither the one supported rule
    /// nor the complete platform-generated metadata pair.
    EmptyConditionalAppearance,
    /// The authenticated first cohort contains exactly one rule.
    TooManyConditionalAppearanceItems {
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items.
        actual: usize,
    },
    /// The selected field of a conditional-appearance rule was empty.
    EmptyConditionalAppearanceField,
    /// The nested comparison must address the same field as the selection.
    ConditionalAppearanceFieldMismatch,
    /// The opaque extension collection exceeded its DCS-specific item bound.
    TooManyOpaqueExtensions {
        /// Maximum accepted extensions.
        maximum: usize,
        /// Actual extensions.
        actual: usize,
    },
    /// An opaque extension was not anchored to the source profile of settings.
    OpaqueSourceProfileMismatch {
        /// Zero-based extension index.
        index: usize,
    },
    /// An opaque extension did not declare an XML placement kind.
    NonXmlPlacement {
        /// Zero-based extension index.
        index: usize,
        /// Exact rejected placement token.
        placement: String,
    },
    /// An opaque extension did not declare a canonical XML media kind.
    NonXmlMediaKind {
        /// Zero-based extension index.
        index: usize,
        /// Exact rejected media-kind token.
        media_kind: String,
    },
    /// Two opaque extensions claimed the same anchor and placement.
    DuplicateOpaquePlacement {
        /// Index of the second extension.
        index: usize,
    },
    /// Aggregate variable-sized data exceeded the DCS budget.
    RetainedBytesExceeded {
        /// Maximum accepted retained bytes.
        maximum: usize,
        /// Actual retained bytes.
        actual: usize,
    },
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
    /// A DCS output-parameter item's name was empty.
    EmptyOutputParameterName,
}

impl Display for DcsBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySelection => formatter.write_str(
                "DCS selection is empty; empty-selection emission is not evidence-backed",
            ),
            Self::EmptySelectionField => formatter.write_str("DCS selected field path is empty"),
            Self::TooManySelectionItems { maximum, actual } => write!(
                formatter,
                "DCS selection exceeds {maximum} items (actual {actual})"
            ),
            Self::EmptyOrder => formatter.write_str(
                "DCS order has neither items nor supported metadata; propertyless empty-order emission is not evidence-backed",
            ),
            Self::EmptyOrderField => formatter.write_str("DCS order field path is empty"),
            Self::TooManyOrderItems { maximum, actual } => write!(
                formatter,
                "DCS order exceeds {maximum} items (actual {actual})"
            ),
            Self::UnsupportedOrderUseTrue => formatter.write_str(
                "DCS order use=true is not evidence-backed; preserve it at the owning boundary",
            ),
            Self::EmptyFilter => formatter.write_str(
                "DCS filter has neither items nor the complete evidenced metadata-only pair",
            ),
            Self::EmptyFilterField => formatter.write_str("DCS filter field path is empty"),
            Self::EmptyFilterStringValue => {
                formatter.write_str("DCS filter string comparison value is empty")
            }
            Self::TooManyFilterItems { maximum, actual } => write!(
                formatter,
                "DCS filter exceeds {maximum} items (actual {actual})"
            ),
            Self::UnsupportedFilterUse => formatter.write_str(
                "DCS filter explicit use is not evidence-backed; preserve it at the owning boundary",
            ),
            Self::EmptyConditionalAppearance => formatter.write_str(
                "DCS conditional appearance has neither the evidenced rule nor the complete metadata-only pair",
            ),
            Self::TooManyConditionalAppearanceItems { maximum, actual } => write!(
                formatter,
                "DCS conditional appearance exceeds {maximum} items (actual {actual})"
            ),
            Self::EmptyConditionalAppearanceField => {
                formatter.write_str("DCS conditional-appearance selected field path is empty")
            }
            Self::ConditionalAppearanceFieldMismatch => formatter.write_str(
                "DCS conditional-appearance selection and nested filter address different fields",
            ),
            Self::TooManyOpaqueExtensions { maximum, actual } => write!(
                formatter,
                "DCS settings exceed {maximum} opaque extensions (actual {actual})"
            ),
            Self::OpaqueSourceProfileMismatch { index } => write!(
                formatter,
                "DCS opaque extension {index} belongs to a different source profile"
            ),
            Self::NonXmlPlacement { index, placement } => write!(
                formatter,
                "DCS opaque extension {index} has non-XML placement `{placement}`"
            ),
            Self::NonXmlMediaKind { index, media_kind } => write!(
                formatter,
                "DCS opaque extension {index} has non-XML media kind `{media_kind}`"
            ),
            Self::DuplicateOpaquePlacement { index } => write!(
                formatter,
                "DCS opaque extension {index} duplicates an anchor and placement"
            ),
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "DCS settings exceed retained-byte budget {maximum} (actual {actual})"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("DCS retained-byte count overflowed")
            }
            Self::EmptyOutputParameterName => {
                formatter.write_str("DCS output-parameter name is empty")
            }
        }
    }
}

impl Error for DcsBuildError {}

/// Platform-evidenced selected field with an exact data path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSelectedField {
    field: CanonicalText,
}

impl DcsSelectedField {
    /// Builds a selected field without interpreting or normalizing its path.
    pub fn new(field: CanonicalText) -> Result<Self, DcsBuildError> {
        if field.as_str().is_empty() {
            return Err(DcsBuildError::EmptySelectionField);
        }
        Ok(Self { field })
    }

    /// Returns the exact selected field path.
    pub const fn field(&self) -> &CanonicalText {
        &self.field
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSelectedFieldWire {
    field: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsSelectedField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSelectedFieldWire::deserialize(deserializer)?;
        Self::new(wire.field).map_err(de::Error::custom)
    }
}

/// Verified selected-item variants. Unsupported item properties and variants
/// must remain opaque at the source boundary instead of entering this enum.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsSelectedItem {
    /// An explicit field selected in source order.
    Field(DcsSelectedField),
    /// The platform's property-free automatic selection item.
    Auto,
}

/// Non-empty, bounded root settings selection in exact source order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSelection {
    items: Vec<DcsSelectedItem>,
}

impl DcsSelection {
    /// Builds a selection while preserving duplicates and source order.
    pub fn new(items: Vec<DcsSelectedItem>) -> Result<Self, DcsBuildError> {
        if items.is_empty() {
            return Err(DcsBuildError::EmptySelection);
        }
        if items.len() > MAX_DCS_SELECTION_ITEMS {
            return Err(DcsBuildError::TooManySelectionItems {
                maximum: MAX_DCS_SELECTION_ITEMS,
                actual: items.len(),
            });
        }
        Ok(Self { items })
    }

    /// Returns selected items in exact source order.
    pub fn items(&self) -> &[DcsSelectedItem] {
        &self.items
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSelectionWire {
    items: DcsSelectionItemsWire,
}

struct DcsSelectionItemsWire(Vec<DcsSelectedItem>);

struct DcsSelectionItemsVisitor;

impl<'de> Visitor<'de> for DcsSelectionItemsVisitor {
    type Value = DcsSelectionItemsWire;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "between 1 and {MAX_DCS_SELECTION_ITEMS} DCS selected items"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_DCS_SELECTION_ITEMS),
        );
        while items.len() < MAX_DCS_SELECTION_ITEMS {
            let Some(item) = sequence.next_element::<DcsSelectedItem>()? else {
                return Ok(DcsSelectionItemsWire(items));
            };
            items.push(item);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS selection exceeds {MAX_DCS_SELECTION_ITEMS} items"
            )));
        }
        Ok(DcsSelectionItemsWire(items))
    }
}

impl<'de> Deserialize<'de> for DcsSelectionItemsWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DcsSelectionItemsVisitor)
    }
}

impl<'de> Deserialize<'de> for DcsSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSelectionWire::deserialize(deserializer)?;
        Self::new(wire.items.0).map_err(de::Error::custom)
    }
}

/// Platform-evidenced direction token for a DCS field order.
///
/// The enum is intentionally closed. A new direction enters the canonical
/// writer only together with profile evidence instead of through an open
/// string fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsOrderType {
    Asc,
    Desc,
}

/// A supported field order with presence-aware `use` semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsOrderField {
    use_value: Option<bool>,
    field: CanonicalText,
    order_type: DcsOrderType,
}

impl DcsOrderField {
    /// Builds a field order without inferring omitted values.
    pub fn new(
        use_value: Option<bool>,
        field: CanonicalText,
        order_type: DcsOrderType,
    ) -> Result<Self, DcsBuildError> {
        if field.as_str().is_empty() {
            return Err(DcsBuildError::EmptyOrderField);
        }
        if use_value == Some(true) {
            return Err(DcsBuildError::UnsupportedOrderUseTrue);
        }
        Ok(Self {
            use_value,
            field,
            order_type,
        })
    }

    /// Returns the exact source presence/value of the `use` child.
    pub const fn use_value(&self) -> Option<bool> {
        self.use_value
    }

    /// Returns the exact ordered field path.
    pub const fn field(&self) -> &CanonicalText {
        &self.field
    }

    /// Returns the explicit order direction.
    pub const fn order_type(&self) -> DcsOrderType {
        self.order_type
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsOrderFieldWire {
    use_value: Option<bool>,
    field: CanonicalText,
    order_type: DcsOrderType,
}

impl<'de> Deserialize<'de> for DcsOrderField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsOrderFieldWire::deserialize(deserializer)?;
        Self::new(wire.use_value, wire.field, wire.order_type).map_err(de::Error::custom)
    }
}

/// Verified order item variants. `Auto` is retained as a canonical semantic
/// variant because the 8.3.27 corpus proves its property-free shape in nested
/// orders; individual output contexts still gate whether it may be emitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsOrderItem {
    Field(DcsOrderField),
    Auto,
}

/// Bounded DCS order in exact source order. A metadata-only order is valid;
/// a propertyless empty container is not.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsOrder {
    items: Vec<DcsOrderItem>,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

impl DcsOrder {
    /// Builds an order while preserving duplicates and source order.
    pub fn new(
        items: Vec<DcsOrderItem>,
        view_mode: Option<EnumToken>,
        user_setting_id: Option<CanonicalText>,
    ) -> Result<Self, DcsBuildError> {
        if items.is_empty() && (view_mode.is_none() || user_setting_id.is_none()) {
            return Err(DcsBuildError::EmptyOrder);
        }
        if items.len() > MAX_DCS_ORDER_ITEMS {
            return Err(DcsBuildError::TooManyOrderItems {
                maximum: MAX_DCS_ORDER_ITEMS,
                actual: items.len(),
            });
        }
        Ok(Self {
            items,
            view_mode,
            user_setting_id,
        })
    }

    pub fn items(&self) -> &[DcsOrderItem] {
        &self.items
    }

    pub const fn view_mode(&self) -> Option<&EnumToken> {
        self.view_mode.as_ref()
    }

    pub const fn user_setting_id(&self) -> Option<&CanonicalText> {
        self.user_setting_id.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsOrderWire {
    items: DcsOrderItemsWire,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

struct DcsOrderItemsWire(Vec<DcsOrderItem>);

struct DcsOrderItemsVisitor;

impl<'de> Visitor<'de> for DcsOrderItemsVisitor {
    type Value = DcsOrderItemsWire;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {MAX_DCS_ORDER_ITEMS} DCS order items")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_DCS_ORDER_ITEMS),
        );
        while items.len() < MAX_DCS_ORDER_ITEMS {
            let Some(item) = sequence.next_element::<DcsOrderItem>()? else {
                return Ok(DcsOrderItemsWire(items));
            };
            items.push(item);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS order exceeds {MAX_DCS_ORDER_ITEMS} items"
            )));
        }
        Ok(DcsOrderItemsWire(items))
    }
}

impl<'de> Deserialize<'de> for DcsOrderItemsWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DcsOrderItemsVisitor)
    }
}

impl<'de> Deserialize<'de> for DcsOrder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsOrderWire::deserialize(deserializer)?;
        Self::new(wire.items.0, wire.view_mode, wire.user_setting_id).map_err(de::Error::custom)
    }
}

/// Platform-evidenced comparison token for the bounded filter cohort.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsFilterComparisonType {
    Equal,
}

/// Platform-evidenced typed right operands. New value kinds require their own
/// XML/profile evidence before entering the canonical writer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsFilterValue {
    String(CanonicalText),
}

impl DcsFilterValue {
    pub fn string(value: CanonicalText) -> Result<Self, DcsBuildError> {
        if value.as_str().is_empty() {
            return Err(DcsBuildError::EmptyFilterStringValue);
        }
        Ok(Self::String(value))
    }

    pub const fn as_string(&self) -> &CanonicalText {
        match self {
            Self::String(value) => value,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum DcsFilterValueWire {
    String(CanonicalText),
}

impl<'de> Deserialize<'de> for DcsFilterValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match DcsFilterValueWire::deserialize(deserializer)? {
            DcsFilterValueWire::String(value) => Self::string(value).map_err(de::Error::custom),
        }
    }
}

/// One comparison item with exact presence-aware operands.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsFilterComparison {
    use_value: Option<bool>,
    field: CanonicalText,
    comparison_type: DcsFilterComparisonType,
    right: DcsFilterValue,
}

impl DcsFilterComparison {
    pub fn new(
        use_value: Option<bool>,
        field: CanonicalText,
        comparison_type: DcsFilterComparisonType,
        right: DcsFilterValue,
    ) -> Result<Self, DcsBuildError> {
        if use_value.is_some() {
            return Err(DcsBuildError::UnsupportedFilterUse);
        }
        if field.as_str().is_empty() {
            return Err(DcsBuildError::EmptyFilterField);
        }
        if right.as_string().as_str().is_empty() {
            return Err(DcsBuildError::EmptyFilterStringValue);
        }
        Ok(Self {
            use_value,
            field,
            comparison_type,
            right,
        })
    }

    pub const fn use_value(&self) -> Option<bool> {
        self.use_value
    }

    pub const fn field(&self) -> &CanonicalText {
        &self.field
    }

    pub const fn comparison_type(&self) -> DcsFilterComparisonType {
        self.comparison_type
    }

    pub const fn right(&self) -> &DcsFilterValue {
        &self.right
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsFilterComparisonWire {
    use_value: Option<bool>,
    field: CanonicalText,
    comparison_type: DcsFilterComparisonType,
    right: DcsFilterValue,
}

impl<'de> Deserialize<'de> for DcsFilterComparison {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsFilterComparisonWire::deserialize(deserializer)?;
        Self::new(wire.use_value, wire.field, wire.comparison_type, wire.right)
            .map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DcsFilterItem {
    Comparison(DcsFilterComparison),
}

/// Bounded filter in exact source order. The only empty form admitted by core
/// is the complete metadata-only pair observed in native Form output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsFilter {
    items: Vec<DcsFilterItem>,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

impl DcsFilter {
    pub fn new(
        items: Vec<DcsFilterItem>,
        view_mode: Option<EnumToken>,
        user_setting_id: Option<CanonicalText>,
    ) -> Result<Self, DcsBuildError> {
        if items.is_empty() && (view_mode.is_none() || user_setting_id.is_none()) {
            return Err(DcsBuildError::EmptyFilter);
        }
        if items.len() > MAX_DCS_FILTER_ITEMS {
            return Err(DcsBuildError::TooManyFilterItems {
                maximum: MAX_DCS_FILTER_ITEMS,
                actual: items.len(),
            });
        }
        Ok(Self {
            items,
            view_mode,
            user_setting_id,
        })
    }

    pub fn items(&self) -> &[DcsFilterItem] {
        &self.items
    }

    pub const fn view_mode(&self) -> Option<&EnumToken> {
        self.view_mode.as_ref()
    }

    pub const fn user_setting_id(&self) -> Option<&CanonicalText> {
        self.user_setting_id.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsFilterWire {
    items: DcsFilterItemsWire,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

struct DcsFilterItemsWire(Vec<DcsFilterItem>);

struct DcsFilterItemsVisitor;

impl<'de> Visitor<'de> for DcsFilterItemsVisitor {
    type Value = DcsFilterItemsWire;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "at most {MAX_DCS_FILTER_ITEMS} DCS filter items")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_DCS_FILTER_ITEMS),
        );
        while items.len() < MAX_DCS_FILTER_ITEMS {
            let Some(item) = sequence.next_element::<DcsFilterItem>()? else {
                return Ok(DcsFilterItemsWire(items));
            };
            items.push(item);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS filter exceeds {MAX_DCS_FILTER_ITEMS} items"
            )));
        }
        Ok(DcsFilterItemsWire(items))
    }
}

impl<'de> Deserialize<'de> for DcsFilterItemsWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DcsFilterItemsVisitor)
    }
}

impl<'de> Deserialize<'de> for DcsFilter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsFilterWire::deserialize(deserializer)?;
        Self::new(wire.items.0, wire.view_mode, wire.user_setting_id).map_err(de::Error::custom)
    }
}

/// The single appearance value authenticated for the initial 8.3.27 cohort.
/// XML lexical values and namespace prefixes remain schema/XML concerns.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsAppearanceColor {
    /// The web palette's red color.
    WebRed,
}

/// Appearance parameters authenticated for canonical emission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DcsAppearanceParameter {
    /// Text color in the pinned platform's source model.
    TextColor(DcsAppearanceColor),
}

/// One bounded conditional-appearance rule.
///
/// This type keeps the conditional selection's untyped XML item distinct
/// from the root selection while reusing the proven filter comparison model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsConditionalAppearanceItem {
    selected_field: CanonicalText,
    filter: DcsFilterComparison,
    appearance: DcsAppearanceParameter,
}

impl DcsConditionalAppearanceItem {
    pub fn new(
        selected_field: CanonicalText,
        filter: DcsFilterComparison,
        appearance: DcsAppearanceParameter,
    ) -> Result<Self, DcsBuildError> {
        if selected_field.as_str().is_empty() {
            return Err(DcsBuildError::EmptyConditionalAppearanceField);
        }
        if filter.field() != &selected_field {
            return Err(DcsBuildError::ConditionalAppearanceFieldMismatch);
        }
        Ok(Self {
            selected_field,
            filter,
            appearance,
        })
    }

    pub const fn selected_field(&self) -> &CanonicalText {
        &self.selected_field
    }

    pub const fn filter(&self) -> &DcsFilterComparison {
        &self.filter
    }

    pub const fn appearance(&self) -> DcsAppearanceParameter {
        self.appearance
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsConditionalAppearanceItemWire {
    selected_field: CanonicalText,
    filter: DcsFilterComparison,
    appearance: DcsAppearanceParameter,
}

impl<'de> Deserialize<'de> for DcsConditionalAppearanceItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsConditionalAppearanceItemWire::deserialize(deserializer)?;
        Self::new(wire.selected_field, wire.filter, wire.appearance).map_err(de::Error::custom)
    }
}

/// Bounded conditional-appearance settings. Empty items are admitted only
/// together with the exact metadata pair reconstructed by the platform.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsConditionalAppearance {
    items: Vec<DcsConditionalAppearanceItem>,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

impl DcsConditionalAppearance {
    pub fn new(
        items: Vec<DcsConditionalAppearanceItem>,
        view_mode: Option<EnumToken>,
        user_setting_id: Option<CanonicalText>,
    ) -> Result<Self, DcsBuildError> {
        if items.is_empty() && (view_mode.is_none() || user_setting_id.is_none()) {
            return Err(DcsBuildError::EmptyConditionalAppearance);
        }
        if items.len() > MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS {
            return Err(DcsBuildError::TooManyConditionalAppearanceItems {
                maximum: MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS,
                actual: items.len(),
            });
        }
        Ok(Self {
            items,
            view_mode,
            user_setting_id,
        })
    }

    pub fn items(&self) -> &[DcsConditionalAppearanceItem] {
        &self.items
    }

    pub const fn view_mode(&self) -> Option<&EnumToken> {
        self.view_mode.as_ref()
    }

    pub const fn user_setting_id(&self) -> Option<&CanonicalText> {
        self.user_setting_id.as_ref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsConditionalAppearanceWire {
    items: Vec<DcsConditionalAppearanceItem>,
    view_mode: Option<EnumToken>,
    user_setting_id: Option<CanonicalText>,
}

impl<'de> Deserialize<'de> for DcsConditionalAppearance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsConditionalAppearanceWire::deserialize(deserializer)?;
        Self::new(wire.items, wire.view_mode, wire.user_setting_id).map_err(de::Error::custom)
    }
}

/// The single evidenced `dcsset:outputParameters` item, authenticated by the
/// dedicated 2214 output-parameters cohort. Only one item, of xs:string
/// value type, is evidenced; a second item or any other value type is
/// outside the admitted cohort. The storage side canonicalizes the
/// parameter's localized name (`Заголовок` -> `Title`) while source XML
/// keeps the source spelling -- that lexical/QName concern remains
/// schema/XML policy, this IR owns only the semantic name/value pair.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsOutputParameters {
    parameter: CanonicalText,
    value: CanonicalText,
}

impl DcsOutputParameters {
    pub fn new(parameter: CanonicalText, value: CanonicalText) -> Result<Self, DcsBuildError> {
        if parameter.as_str().is_empty() {
            return Err(DcsBuildError::EmptyOutputParameterName);
        }
        Ok(Self { parameter, value })
    }

    pub const fn parameter(&self) -> &CanonicalText {
        &self.parameter
    }

    pub const fn value(&self) -> &CanonicalText {
        &self.value
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsOutputParametersWire {
    parameter: CanonicalText,
    value: CanonicalText,
}

impl<'de> Deserialize<'de> for DcsOutputParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsOutputParametersWire::deserialize(deserializer)?;
        Self::new(wire.parameter, wire.value).map_err(de::Error::custom)
    }
}

/// Verified typed minimum of `DataCompositionSettings`.
///
/// The optional root selection and scalar fields are structural model
/// semantics. Their XML spelling, omission rules, and order remain the
/// responsibility of verified writer rules. Unknown settings children and
/// attributes are retained in exact source order by `opaque_extensions`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DcsSettings {
    selection: Option<DcsSelection>,
    filter: Option<DcsFilter>,
    order: Option<DcsOrder>,
    conditional_appearance: Option<DcsConditionalAppearance>,
    output_parameters: Option<DcsOutputParameters>,
    items_user_setting_id: Option<CanonicalText>,
    items_view_mode: Option<EnumToken>,
    opaque_extensions: OpaqueFacets,
    provenance: SourceProvenance,
}

impl DcsSettings {
    /// Returns the optional root selection.
    pub const fn selection(&self) -> Option<&DcsSelection> {
        self.selection.as_ref()
    }

    /// Returns the optional root/Form filter.
    pub const fn filter(&self) -> Option<&DcsFilter> {
        self.filter.as_ref()
    }

    /// Returns the optional root order.
    pub const fn order(&self) -> Option<&DcsOrder> {
        self.order.as_ref()
    }

    /// Returns the optional root/Form conditional-appearance settings.
    pub const fn conditional_appearance(&self) -> Option<&DcsConditionalAppearance> {
        self.conditional_appearance.as_ref()
    }

    /// Returns the optional root `dcsset:outputParameters` item.
    pub const fn output_parameters(&self) -> Option<&DcsOutputParameters> {
        self.output_parameters.as_ref()
    }

    /// Returns the optional exact user-setting identifier.
    pub const fn items_user_setting_id(&self) -> Option<&CanonicalText> {
        self.items_user_setting_id.as_ref()
    }

    /// Returns the optional open settings item view-mode token.
    pub const fn items_view_mode(&self) -> Option<&EnumToken> {
        self.items_view_mode.as_ref()
    }

    /// Returns unsupported extensions in exact source order.
    pub const fn opaque_extensions(&self) -> &OpaqueFacets {
        &self.opaque_extensions
    }

    /// Returns exact source provenance for the typed settings value.
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
}

/// Builder for canonical DCS settings. Provenance is mandatory at the entry
/// point so adding new cohorts does not grow a positional constructor whose
/// arguments can be accidentally swapped.
#[derive(Clone, Debug)]
pub struct DcsSettingsBuilder {
    selection: Option<DcsSelection>,
    filter: Option<DcsFilter>,
    order: Option<DcsOrder>,
    conditional_appearance: Option<DcsConditionalAppearance>,
    output_parameters: Option<DcsOutputParameters>,
    items_user_setting_id: Option<CanonicalText>,
    items_view_mode: Option<EnumToken>,
    opaque_extensions: OpaqueFacets,
    provenance: SourceProvenance,
}

impl DcsSettingsBuilder {
    pub fn new(provenance: SourceProvenance) -> Self {
        Self {
            selection: None,
            filter: None,
            order: None,
            conditional_appearance: None,
            output_parameters: None,
            items_user_setting_id: None,
            items_view_mode: None,
            opaque_extensions: OpaqueFacets::new(Vec::new())
                .expect("empty DCS opaque facets are valid"),
            provenance,
        }
    }

    pub fn selection(mut self, selection: Option<DcsSelection>) -> Self {
        self.selection = selection;
        self
    }

    pub fn filter(mut self, filter: Option<DcsFilter>) -> Self {
        self.filter = filter;
        self
    }

    pub fn order(mut self, order: Option<DcsOrder>) -> Self {
        self.order = order;
        self
    }

    pub fn conditional_appearance(mut self, value: Option<DcsConditionalAppearance>) -> Self {
        self.conditional_appearance = value;
        self
    }

    pub fn output_parameters(mut self, value: Option<DcsOutputParameters>) -> Self {
        self.output_parameters = value;
        self
    }

    pub fn items_user_setting_id(mut self, value: Option<CanonicalText>) -> Self {
        self.items_user_setting_id = value;
        self
    }

    pub fn items_view_mode(mut self, value: Option<EnumToken>) -> Self {
        self.items_view_mode = value;
        self
    }

    pub fn opaque_extensions(mut self, value: OpaqueFacets) -> Self {
        self.opaque_extensions = value;
        self
    }

    pub fn build(self) -> Result<DcsSettings, DcsBuildError> {
        let settings = DcsSettings {
            selection: self.selection,
            filter: self.filter,
            order: self.order,
            conditional_appearance: self.conditional_appearance,
            output_parameters: self.output_parameters,
            items_user_setting_id: self.items_user_setting_id,
            items_view_mode: self.items_view_mode,
            opaque_extensions: self.opaque_extensions,
            provenance: self.provenance,
        };
        validate_settings(&settings)?;
        Ok(settings)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DcsSettingsWire {
    selection: Option<DcsSelection>,
    filter: Option<DcsFilter>,
    order: Option<DcsOrder>,
    conditional_appearance: Option<DcsConditionalAppearance>,
    #[serde(default)]
    output_parameters: Option<DcsOutputParameters>,
    items_user_setting_id: Option<CanonicalText>,
    items_view_mode: Option<EnumToken>,
    opaque_extensions: DcsOpaqueFacetsWire,
    provenance: SourceProvenance,
}

struct DcsOpaqueFacetsWire(Vec<OpaqueFacet>);

struct DcsOpaqueFacetsVisitor;

impl<'de> Visitor<'de> for DcsOpaqueFacetsVisitor {
    type Value = DcsOpaqueFacetsWire;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_DCS_OPAQUE_EXTENSIONS} DCS XML extensions retaining at most {MAX_DCS_RETAINED_BYTES} bytes"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut facets = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_DCS_OPAQUE_EXTENSIONS),
        );
        let mut retained_bytes = 0usize;
        while facets.len() < MAX_DCS_OPAQUE_EXTENSIONS {
            let Some(facet) = sequence.next_element::<OpaqueFacet>()? else {
                return Ok(DcsOpaqueFacetsWire(facets));
            };
            let byte_len = usize::try_from(facet.byte_len())
                .map_err(|_| de::Error::custom(DcsBuildError::RetainedByteCountOverflow))?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, byte_len).map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.provenance().retained_byte_len())
                    .map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.placement().kind().as_str().len())
                    .map_err(de::Error::custom)?;
            retained_bytes =
                checked_retained_bytes(retained_bytes, facet.media_kind().as_str().len())
                    .map_err(de::Error::custom)?;
            facets.push(facet);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "DCS settings exceed {MAX_DCS_OPAQUE_EXTENSIONS} opaque extensions"
            )));
        }
        Ok(DcsOpaqueFacetsWire(facets))
    }
}

impl<'de> Deserialize<'de> for DcsOpaqueFacetsWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(DcsOpaqueFacetsVisitor)
    }
}

impl<'de> Deserialize<'de> for DcsSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DcsSettingsWire::deserialize(deserializer)?;
        DcsSettingsBuilder::new(wire.provenance)
            .selection(wire.selection)
            .filter(wire.filter)
            .order(wire.order)
            .conditional_appearance(wire.conditional_appearance)
            .output_parameters(wire.output_parameters)
            .items_user_setting_id(wire.items_user_setting_id)
            .items_view_mode(wire.items_view_mode)
            .opaque_extensions(
                OpaqueFacets::new(wire.opaque_extensions.0).map_err(de::Error::custom)?,
            )
            .build()
            .map_err(de::Error::custom)
    }
}

/// Physical context in which the same canonical settings semantics occurred.
///
/// This distinction preserves the verified delegation boundary without
/// duplicating the settings model or introducing XML wrapper names into core.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "context",
    content = "settings",
    rename_all = "snake_case"
)]
pub enum DcsSettingsEnvelope {
    /// A standalone DCS settings document.
    Settings(DcsSettings),
    /// Settings delegated by a Form `ListSettings` feature.
    ListSettings(DcsSettings),
}

impl DcsSettingsEnvelope {
    /// Creates a standalone settings envelope.
    pub const fn settings(settings: DcsSettings) -> Self {
        Self::Settings(settings)
    }

    /// Creates a Form `ListSettings` envelope.
    pub const fn list_settings(settings: DcsSettings) -> Self {
        Self::ListSettings(settings)
    }

    /// Returns the shared canonical settings payload.
    pub const fn as_settings(&self) -> &DcsSettings {
        match self {
            Self::Settings(settings) | Self::ListSettings(settings) => settings,
        }
    }
}

fn validate_settings(settings: &DcsSettings) -> Result<(), DcsBuildError> {
    let DcsSettings {
        selection,
        filter,
        order,
        conditional_appearance,
        output_parameters,
        items_user_setting_id,
        items_view_mode,
        opaque_extensions,
        provenance,
    } = settings;
    if opaque_extensions.len() > MAX_DCS_OPAQUE_EXTENSIONS {
        return Err(DcsBuildError::TooManyOpaqueExtensions {
            maximum: MAX_DCS_OPAQUE_EXTENSIONS,
            actual: opaque_extensions.len(),
        });
    }

    let mut placements = BTreeSet::new();
    let mut retained_bytes = provenance.retained_byte_len();
    if let Some(selection) = selection {
        if selection.items().is_empty() {
            return Err(DcsBuildError::EmptySelection);
        }
        if selection.items().len() > MAX_DCS_SELECTION_ITEMS {
            return Err(DcsBuildError::TooManySelectionItems {
                maximum: MAX_DCS_SELECTION_ITEMS,
                actual: selection.items().len(),
            });
        }
        for item in selection.items() {
            if let DcsSelectedItem::Field(field) = item {
                if field.field().as_str().is_empty() {
                    return Err(DcsBuildError::EmptySelectionField);
                }
                retained_bytes =
                    checked_retained_bytes(retained_bytes, field.field().as_str().len())?;
            }
        }
    }
    if let Some(filter) = filter {
        if filter.items().is_empty()
            && (filter.view_mode().is_none() || filter.user_setting_id().is_none())
        {
            return Err(DcsBuildError::EmptyFilter);
        }
        if filter.items().len() > MAX_DCS_FILTER_ITEMS {
            return Err(DcsBuildError::TooManyFilterItems {
                maximum: MAX_DCS_FILTER_ITEMS,
                actual: filter.items().len(),
            });
        }
        for item in filter.items() {
            let DcsFilterItem::Comparison(comparison) = item;
            if comparison.use_value().is_some() {
                return Err(DcsBuildError::UnsupportedFilterUse);
            }
            if comparison.field().as_str().is_empty() {
                return Err(DcsBuildError::EmptyFilterField);
            }
            if comparison.right().as_string().as_str().is_empty() {
                return Err(DcsBuildError::EmptyFilterStringValue);
            }
            retained_bytes =
                checked_retained_bytes(retained_bytes, comparison.field().as_str().len())?;
            retained_bytes = checked_retained_bytes(
                retained_bytes,
                comparison.right().as_string().as_str().len(),
            )?;
        }
        if let Some(value) = filter.view_mode() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
        if let Some(value) = filter.user_setting_id() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
    }
    if let Some(order) = order {
        if order.items().is_empty()
            && (order.view_mode().is_none() || order.user_setting_id().is_none())
        {
            return Err(DcsBuildError::EmptyOrder);
        }
        if order.items().len() > MAX_DCS_ORDER_ITEMS {
            return Err(DcsBuildError::TooManyOrderItems {
                maximum: MAX_DCS_ORDER_ITEMS,
                actual: order.items().len(),
            });
        }
        for item in order.items() {
            if let DcsOrderItem::Field(field) = item {
                if field.field().as_str().is_empty() {
                    return Err(DcsBuildError::EmptyOrderField);
                }
                if field.use_value() == Some(true) {
                    return Err(DcsBuildError::UnsupportedOrderUseTrue);
                }
                retained_bytes =
                    checked_retained_bytes(retained_bytes, field.field().as_str().len())?;
            }
        }
        if let Some(value) = order.view_mode() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
        if let Some(value) = order.user_setting_id() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
    }
    if let Some(conditional_appearance) = conditional_appearance {
        if conditional_appearance.items().is_empty()
            && (conditional_appearance.view_mode().is_none()
                || conditional_appearance.user_setting_id().is_none())
        {
            return Err(DcsBuildError::EmptyConditionalAppearance);
        }
        if conditional_appearance.items().len() > MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS {
            return Err(DcsBuildError::TooManyConditionalAppearanceItems {
                maximum: MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS,
                actual: conditional_appearance.items().len(),
            });
        }
        for item in conditional_appearance.items() {
            if item.selected_field().as_str().is_empty() {
                return Err(DcsBuildError::EmptyConditionalAppearanceField);
            }
            if item.filter().field() != item.selected_field() {
                return Err(DcsBuildError::ConditionalAppearanceFieldMismatch);
            }
            retained_bytes =
                checked_retained_bytes(retained_bytes, item.selected_field().as_str().len())?;
            retained_bytes = checked_retained_bytes(
                retained_bytes,
                item.filter().right().as_string().as_str().len(),
            )?;
        }
        if let Some(value) = conditional_appearance.view_mode() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
        if let Some(value) = conditional_appearance.user_setting_id() {
            retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
        }
    }
    if let Some(output_parameters) = output_parameters {
        retained_bytes =
            checked_retained_bytes(retained_bytes, output_parameters.parameter().as_str().len())?;
        retained_bytes =
            checked_retained_bytes(retained_bytes, output_parameters.value().as_str().len())?;
    }
    if let Some(value) = items_user_setting_id {
        retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
    }
    if let Some(value) = items_view_mode {
        retained_bytes = checked_retained_bytes(retained_bytes, value.as_str().len())?;
    }

    for (index, facet) in opaque_extensions.as_slice().iter().enumerate() {
        validate_opaque_extension(index, facet, provenance, &mut placements)?;
        let byte_len = usize::try_from(facet.byte_len())
            .map_err(|_| DcsBuildError::RetainedByteCountOverflow)?;
        retained_bytes = checked_retained_bytes(retained_bytes, byte_len)?;
        retained_bytes =
            checked_retained_bytes(retained_bytes, facet.provenance().retained_byte_len())?;
        retained_bytes =
            checked_retained_bytes(retained_bytes, facet.placement().kind().as_str().len())?;
        retained_bytes = checked_retained_bytes(retained_bytes, facet.media_kind().as_str().len())?;
    }
    Ok(())
}

fn validate_opaque_extension<'a>(
    index: usize,
    facet: &'a OpaqueFacet,
    provenance: &SourceProvenance,
    placements: &mut BTreeSet<(
        &'a crate::provenance::CanonicalAnchor,
        &'a crate::opaque::OpaquePlacement,
    )>,
) -> Result<(), DcsBuildError> {
    if facet.source_profile() != provenance.source_profile() {
        return Err(DcsBuildError::OpaqueSourceProfileMismatch { index });
    }
    let placement = facet.placement().kind().as_str();
    if !placement.starts_with("xml:") {
        return Err(DcsBuildError::NonXmlPlacement {
            index,
            placement: placement.to_owned(),
        });
    }
    let media_kind = facet.media_kind().as_str();
    if !matches!(media_kind, "application/xml" | "text/xml")
        && !media_kind
            .strip_prefix("application/")
            .is_some_and(|subtype| subtype.ends_with("+xml"))
    {
        return Err(DcsBuildError::NonXmlMediaKind {
            index,
            media_kind: media_kind.to_owned(),
        });
    }
    if !placements.insert((facet.anchor(), facet.placement())) {
        return Err(DcsBuildError::DuplicateOpaquePlacement { index });
    }
    Ok(())
}

fn checked_retained_bytes(current: usize, additional: usize) -> Result<usize, DcsBuildError> {
    let actual = current
        .checked_add(additional)
        .ok_or(DcsBuildError::RetainedByteCountOverflow)?;
    if actual > MAX_DCS_RETAINED_BYTES {
        return Err(DcsBuildError::RetainedBytesExceeded {
            maximum: MAX_DCS_RETAINED_BYTES,
            actual,
        });
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::asset::MediaKind;
    use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use crate::opaque::OpaquePlacement;
    use crate::provenance::CanonicalAnchor;

    use super::*;

    fn anchor(property: &str) -> CanonicalAnchor {
        CanonicalAnchor::new(
            ObjectPath::new(vec![PathSegment::name("dcs_settings").unwrap()]).unwrap(),
            PropertyPath::new(vec![PathSegment::name(property).unwrap()]).unwrap(),
        )
    }

    fn provenance(profile: &str, property: &str) -> SourceProvenance {
        SourceProvenance::with_locator(
            ProfileId::parse(profile).unwrap(),
            anchor(property),
            "fixture:dcs/settings.xml",
        )
        .unwrap()
    }

    fn extension(profile: &str, ordinal: u32, bytes: &[u8]) -> OpaqueFacet {
        OpaqueFacet::new(
            provenance(profile, "extensions"),
            OpaquePlacement::new("xml:child", ordinal).unwrap(),
            bytes.to_vec(),
            MediaKind::new("application/xml").unwrap(),
        )
        .unwrap()
    }

    fn settings(extensions: Vec<OpaqueFacet>) -> DcsSettings {
        DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
            .items_user_setting_id(Some(CanonicalText::new("main-settings").unwrap()))
            .items_view_mode(Some(EnumToken::new("QuickAccess").unwrap()))
            .opaque_extensions(OpaqueFacets::new(extensions).unwrap())
            .build()
            .unwrap()
    }

    fn selected_field(field: &str) -> DcsSelectedItem {
        DcsSelectedItem::Field(DcsSelectedField::new(CanonicalText::new(field).unwrap()).unwrap())
    }

    fn order_field(use_value: Option<bool>, field: &str) -> DcsOrderItem {
        DcsOrderItem::Field(
            DcsOrderField::new(
                use_value,
                CanonicalText::new(field).unwrap(),
                DcsOrderType::Asc,
            )
            .unwrap(),
        )
    }

    fn filter_comparison_value(field: &str, right: &str) -> DcsFilterComparison {
        DcsFilterComparison::new(
            None,
            CanonicalText::new(field).unwrap(),
            DcsFilterComparisonType::Equal,
            DcsFilterValue::string(CanonicalText::new(right).unwrap()).unwrap(),
        )
        .unwrap()
    }

    fn filter_comparison(field: &str, right: &str) -> DcsFilterItem {
        DcsFilterItem::Comparison(filter_comparison_value(field, right))
    }

    #[test]
    fn selection_preserves_order_duplicates_and_wire_shape() {
        let selection = DcsSelection::new(vec![
            selected_field("Name"),
            DcsSelectedItem::Auto,
            selected_field("Name"),
        ])
        .unwrap();
        let settings = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .selection(Some(selection))
            .build()
            .unwrap();

        assert!(matches!(
            settings.selection().unwrap().items(),
            [
                DcsSelectedItem::Field(_),
                DcsSelectedItem::Auto,
                DcsSelectedItem::Field(_)
            ]
        ));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#"[{"field":{"field":"Name"}},"auto",{"field":{"field":"Name"}}]"#));
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn empty_selection_empty_field_and_excessive_items_fail_closed() {
        assert_eq!(
            DcsSelection::new(Vec::new()),
            Err(DcsBuildError::EmptySelection)
        );
        assert_eq!(
            DcsSelectedField::new(CanonicalText::new("").unwrap()),
            Err(DcsBuildError::EmptySelectionField)
        );
        assert!(matches!(
            DcsSelection::new(vec![DcsSelectedItem::Auto; MAX_DCS_SELECTION_ITEMS + 1]),
            Err(DcsBuildError::TooManySelectionItems { .. })
        ));

        let over_limit = serde_json::json!({
            "items": vec![serde_json::json!("auto"); MAX_DCS_SELECTION_ITEMS + 1]
        });
        let error = serde_json::from_value::<DcsSelection>(over_limit)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds 16384 items"));
    }

    #[test]
    fn order_preserves_presence_order_duplicates_and_wire_shape() {
        let order = DcsOrder::new(
            vec![order_field(None, "Name"), order_field(Some(false), "Name")],
            Some(EnumToken::new("Normal").unwrap()),
            Some(CanonicalText::new("88619765-ccb3-46c6-ac52-38e9c992ebd4").unwrap()),
        )
        .unwrap();
        let settings = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .order(Some(order))
            .build()
            .unwrap();

        let items = settings.order().unwrap().items();
        assert_eq!(items.len(), 2);
        let DcsOrderItem::Field(first) = &items[0] else {
            panic!("expected field order")
        };
        let DcsOrderItem::Field(second) = &items[1] else {
            panic!("expected field order")
        };
        assert_eq!(first.use_value(), None);
        assert_eq!(second.use_value(), Some(false));
        assert_eq!(first.field().as_str(), second.field().as_str());

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains(r#""use_value":null"#));
        assert!(json.contains(r#""use_value":false"#));
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&json).unwrap(),
            settings
        );
    }

    #[test]
    fn desc_multiple_items_and_metadata_only_orders_are_canonical() {
        let desc = DcsOrderItem::Field(
            DcsOrderField::new(
                None,
                CanonicalText::new("VersionNumber").unwrap(),
                DcsOrderType::Desc,
            )
            .unwrap(),
        );
        let multiple = DcsOrder::new(
            vec![desc.clone(), desc],
            Some(EnumToken::new("Normal").unwrap()),
            None,
        )
        .unwrap();
        assert_eq!(multiple.items().len(), 2);
        assert!(matches!(
            multiple.items(),
            [DcsOrderItem::Field(first), DcsOrderItem::Field(second)]
                if first.order_type() == DcsOrderType::Desc
                    && second.order_type() == DcsOrderType::Desc
        ));

        let metadata_only = DcsOrder::new(
            Vec::new(),
            Some(EnumToken::new("Normal").unwrap()),
            Some(CanonicalText::new("88619765-ccb3-46c6-ac52-38e9c992ebd4").unwrap()),
        )
        .unwrap();
        assert!(metadata_only.items().is_empty());
        assert_eq!(metadata_only.view_mode().unwrap().as_str(), "Normal");
        assert_eq!(
            serde_json::from_str::<DcsOrder>(&serde_json::to_string(&metadata_only).unwrap())
                .unwrap(),
            metadata_only
        );
    }

    #[test]
    fn unsupported_order_shapes_fail_closed() {
        assert_eq!(
            DcsOrder::new(Vec::new(), None, None),
            Err(DcsBuildError::EmptyOrder)
        );
        assert_eq!(
            DcsOrder::new(Vec::new(), Some(EnumToken::new("Normal").unwrap()), None,),
            Err(DcsBuildError::EmptyOrder)
        );
        assert_eq!(
            DcsOrderField::new(None, CanonicalText::new("").unwrap(), DcsOrderType::Asc,),
            Err(DcsBuildError::EmptyOrderField)
        );
        assert_eq!(
            DcsOrderField::new(
                Some(true),
                CanonicalText::new("Name").unwrap(),
                DcsOrderType::Asc,
            ),
            Err(DcsBuildError::UnsupportedOrderUseTrue)
        );
        assert!(matches!(
            DcsOrder::new(
                vec![DcsOrderItem::Auto; MAX_DCS_ORDER_ITEMS + 1],
                None,
                None,
            ),
            Err(DcsBuildError::TooManyOrderItems { .. })
        ));

        let over_limit = serde_json::json!({
            "items": vec![serde_json::json!("auto"); MAX_DCS_ORDER_ITEMS + 1],
            "view_mode": null,
            "user_setting_id": null
        });
        let error = serde_json::from_value::<DcsOrder>(over_limit)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeds 16384 items"));
    }

    #[test]
    fn filter_preserves_typed_operands_metadata_and_wire_shape() {
        let filter = DcsFilter::new(
            vec![filter_comparison("SortKey", "A")],
            Some(EnumToken::new("Normal").unwrap()),
            Some(CanonicalText::new("dfcece9d-5077-440b-b6b3-45a5cb4538eb").unwrap()),
        )
        .unwrap();
        let settings = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .filter(Some(filter))
            .build()
            .unwrap();
        let DcsFilterItem::Comparison(comparison) = &settings.filter().unwrap().items()[0];
        assert_eq!(comparison.use_value(), None);
        assert_eq!(comparison.field().as_str(), "SortKey");
        assert_eq!(comparison.comparison_type(), DcsFilterComparisonType::Equal);
        assert_eq!(comparison.right().as_string().as_str(), "A");
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&json).unwrap(),
            settings
        );

        let metadata_only = DcsFilter::new(
            Vec::new(),
            Some(EnumToken::new("Normal").unwrap()),
            Some(CanonicalText::new("dfcece9d-5077-440b-b6b3-45a5cb4538eb").unwrap()),
        )
        .unwrap();
        assert!(metadata_only.items().is_empty());
    }

    #[test]
    fn unsupported_filter_shapes_fail_closed_and_deserialize_bounded() {
        assert_eq!(
            DcsFilter::new(Vec::new(), None, None),
            Err(DcsBuildError::EmptyFilter)
        );
        assert_eq!(
            DcsFilterComparison::new(
                Some(false),
                CanonicalText::new("SortKey").unwrap(),
                DcsFilterComparisonType::Equal,
                DcsFilterValue::string(CanonicalText::new("A").unwrap()).unwrap(),
            ),
            Err(DcsBuildError::UnsupportedFilterUse)
        );
        assert_eq!(
            DcsFilterValue::string(CanonicalText::new("").unwrap()),
            Err(DcsBuildError::EmptyFilterStringValue)
        );
        assert!(matches!(
            DcsFilter::new(
                vec![filter_comparison("SortKey", "A"); MAX_DCS_FILTER_ITEMS + 1],
                None,
                None,
            ),
            Err(DcsBuildError::TooManyFilterItems { .. })
        ));
        let item = serde_json::to_value(filter_comparison("SortKey", "A")).unwrap();
        let over_limit = serde_json::json!({
            "items": vec![item; MAX_DCS_FILTER_ITEMS + 1],
            "view_mode": null,
            "user_setting_id": null
        });
        assert!(
            serde_json::from_value::<DcsFilter>(over_limit)
                .unwrap_err()
                .to_string()
                .contains("exceeds 16384 items")
        );
    }

    #[test]
    fn conditional_appearance_reuses_the_proven_filter_semantics_and_wire_shape() {
        let item = DcsConditionalAppearanceItem::new(
            CanonicalText::new("SortKey").unwrap(),
            filter_comparison_value("SortKey", "A"),
            DcsAppearanceParameter::TextColor(DcsAppearanceColor::WebRed),
        )
        .unwrap();
        let appearance = DcsConditionalAppearance::new(vec![item], None, None).unwrap();
        let settings = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .conditional_appearance(Some(appearance))
            .build()
            .unwrap();
        let value = settings.conditional_appearance().unwrap();
        assert_eq!(value.items()[0].selected_field().as_str(), "SortKey");
        assert_eq!(value.items()[0].filter().field().as_str(), "SortKey");
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&serde_json::to_string(&settings).unwrap())
                .unwrap(),
            settings
        );

        let metadata_only = DcsConditionalAppearance::new(
            Vec::new(),
            Some(EnumToken::new("Normal").unwrap()),
            Some(CanonicalText::new("b75fecce-942b-4aed-abc9-e6a02e460fb3").unwrap()),
        )
        .unwrap();
        assert!(metadata_only.items().is_empty());
    }

    #[test]
    fn output_parameters_are_bounded_and_serde_stable() {
        let output = DcsOutputParameters::new(
            CanonicalText::new("Заголовок").unwrap(),
            CanonicalText::new("Probe Title").unwrap(),
        )
        .unwrap();
        let settings = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .output_parameters(Some(output))
            .build()
            .unwrap();
        let value = settings.output_parameters().unwrap();
        assert_eq!(value.parameter().as_str(), "Заголовок");
        assert_eq!(value.value().as_str(), "Probe Title");
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&serde_json::to_string(&settings).unwrap())
                .unwrap(),
            settings
        );

        // Absent by default: existing settings values (with no
        // outputParameters at all) keep round-tripping unaffected.
        let without = DcsSettingsBuilder::new(provenance("platform:8.3.27", "settings"))
            .build()
            .unwrap();
        assert!(without.output_parameters().is_none());
        assert_eq!(
            serde_json::from_str::<DcsSettings>(&serde_json::to_string(&without).unwrap()).unwrap(),
            without
        );

        assert!(matches!(
            DcsOutputParameters::new(
                CanonicalText::new("").unwrap(),
                CanonicalText::new("Probe Title").unwrap(),
            ),
            Err(DcsBuildError::EmptyOutputParameterName)
        ));
    }

    #[test]
    fn unsupported_conditional_appearance_shapes_fail_closed() {
        assert_eq!(
            DcsConditionalAppearance::new(Vec::new(), None, None),
            Err(DcsBuildError::EmptyConditionalAppearance)
        );
        assert_eq!(
            DcsConditionalAppearanceItem::new(
                CanonicalText::new("Other").unwrap(),
                filter_comparison_value("SortKey", "A"),
                DcsAppearanceParameter::TextColor(DcsAppearanceColor::WebRed),
            ),
            Err(DcsBuildError::ConditionalAppearanceFieldMismatch)
        );
        let item = DcsConditionalAppearanceItem::new(
            CanonicalText::new("SortKey").unwrap(),
            filter_comparison_value("SortKey", "A"),
            DcsAppearanceParameter::TextColor(DcsAppearanceColor::WebRed),
        )
        .unwrap();
        assert!(matches!(
            DcsConditionalAppearance::new(
                vec![item; MAX_DCS_CONDITIONAL_APPEARANCE_ITEMS + 1],
                None,
                None,
            ),
            Err(DcsBuildError::TooManyConditionalAppearanceItems { .. })
        ));
    }

    #[test]
    fn standalone_and_list_settings_share_one_deterministic_payload() {
        let payload = settings(vec![extension(
            "platform:8.3.24",
            2,
            b"<future xmlns=\"urn:future\" exact=\"yes\"/>",
        )]);
        let standalone = DcsSettingsEnvelope::settings(payload.clone());
        let list_settings = DcsSettingsEnvelope::list_settings(payload);

        let standalone_json = serde_json::to_string(&standalone).unwrap();
        let list_json = serde_json::to_string(&list_settings).unwrap();
        assert!(standalone_json.starts_with(r#"{"context":"settings","settings":"#));
        assert!(list_json.starts_with(r#"{"context":"list_settings","settings":"#));
        assert_eq!(
            serde_json::from_str::<DcsSettingsEnvelope>(&standalone_json).unwrap(),
            standalone
        );
        assert_eq!(
            serde_json::from_str::<DcsSettingsEnvelope>(&list_json).unwrap(),
            list_settings
        );
        assert_eq!(
            serde_json::to_string(
                serde_json::from_str::<DcsSettingsEnvelope>(&list_json)
                    .unwrap()
                    .as_settings()
            )
            .unwrap(),
            serde_json::to_string(list_settings.as_settings()).unwrap()
        );
    }

    #[test]
    fn opaque_extension_retains_exact_bytes_placement_and_provenance() {
        let expected = b"<x:Future xmlns:x=\"urn:x\">\n  exact text\n</x:Future>";
        let value = settings(vec![extension("platform:8.3.24", 7, expected)]);
        let facet = &value.opaque_extensions().as_slice()[0];

        assert_eq!(facet.placement().kind().as_str(), "xml:child");
        assert_eq!(facet.placement().ordinal(), 7);
        assert_eq!(
            facet.provenance().locator().unwrap().as_str(),
            "fixture:dcs/settings.xml"
        );
        let profile = ProfileId::parse("platform:8.3.24").unwrap();
        assert_eq!(facet.emit_permit(&profile).unwrap().bytes(), expected);
    }

    #[test]
    fn mismatched_profile_and_non_xml_facets_fail_closed() {
        let mismatched = extension("platform:8.3.25", 0, b"<future/>");
        assert!(matches!(
            DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
                .opaque_extensions(OpaqueFacets::new(vec![mismatched]).unwrap())
                .build(),
            Err(DcsBuildError::OpaqueSourceProfileMismatch { index: 0 })
        ));

        let non_xml_placement = OpaqueFacet::new(
            provenance("platform:8.3.24", "extensions"),
            OpaquePlacement::new("binary:tail", 0).unwrap(),
            b"<future/>".to_vec(),
            MediaKind::new("application/xml").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
                .opaque_extensions(OpaqueFacets::new(vec![non_xml_placement]).unwrap())
                .build(),
            Err(DcsBuildError::NonXmlPlacement { index: 0, .. })
        ));

        let non_xml_media = OpaqueFacet::new(
            provenance("platform:8.3.24", "extensions"),
            OpaquePlacement::new("xml:child", 0).unwrap(),
            b"<future/>".to_vec(),
            MediaKind::new("application/octet-stream").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
                .opaque_extensions(OpaqueFacets::new(vec![non_xml_media]).unwrap())
                .build(),
            Err(DcsBuildError::NonXmlMediaKind { index: 0, .. })
        ));
    }

    #[test]
    fn duplicate_placement_and_excessive_extension_count_are_rejected() {
        let first = extension("platform:8.3.24", 1, b"<first/>");
        let duplicate = extension("platform:8.3.24", 1, b"<second/>");
        assert!(matches!(
            DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
                .opaque_extensions(OpaqueFacets::new(vec![first, duplicate]).unwrap())
                .build(),
            Err(DcsBuildError::DuplicateOpaquePlacement { index: 1 })
        ));

        let extensions = (0..=MAX_DCS_OPAQUE_EXTENSIONS)
            .map(|ordinal| extension("platform:8.3.24", u32::try_from(ordinal).unwrap(), b""))
            .collect();
        assert!(matches!(
            DcsSettingsBuilder::new(provenance("platform:8.3.24", "settings"))
                .opaque_extensions(OpaqueFacets::new(extensions).unwrap())
                .build(),
            Err(DcsBuildError::TooManyOpaqueExtensions { .. })
        ));
    }

    #[test]
    fn retained_byte_budget_rejects_limit_plus_one_and_overflow() {
        assert_eq!(
            checked_retained_bytes(MAX_DCS_RETAINED_BYTES, 1),
            Err(DcsBuildError::RetainedBytesExceeded {
                maximum: MAX_DCS_RETAINED_BYTES,
                actual: MAX_DCS_RETAINED_BYTES + 1,
            })
        );
        assert_eq!(
            checked_retained_bytes(usize::MAX, 1),
            Err(DcsBuildError::RetainedByteCountOverflow)
        );
    }

    #[test]
    fn deserialization_revalidates_invariants_and_denies_unknown_fields() {
        let value = settings(vec![extension("platform:8.3.24", 0, b"<future/>")]);
        let mut json = serde_json::to_value(value).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("guessed_qname".to_owned(), serde_json::json!("Settings"));
        assert!(serde_json::from_value::<DcsSettings>(json).is_err());

        let value = settings(vec![extension("platform:8.3.24", 0, b"<future/>")]);
        let mut json = serde_json::to_value(value).unwrap();
        json["opaque_extensions"][0]["provenance"]["source_profile"] =
            serde_json::json!("platform:8.3.25");
        let error = serde_json::from_value::<DcsSettings>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("different source profile"));

        let mut envelope =
            serde_json::to_value(DcsSettingsEnvelope::settings(settings(Vec::new()))).unwrap();
        envelope
            .as_object_mut()
            .unwrap()
            .insert("guessed_wrapper".to_owned(), serde_json::json!(true));
        assert!(serde_json::from_value::<DcsSettingsEnvelope>(envelope).is_err());

        let facet = serde_json::to_value(extension("platform:8.3.24", 0, b"<future/>")).unwrap();
        let over_limit = serde_json::json!({
            "items_user_setting_id": null,
            "items_view_mode": null,
            "opaque_extensions": vec![facet; MAX_DCS_OPAQUE_EXTENSIONS + 1],
            "provenance": provenance("platform:8.3.24", "settings"),
        });
        let error = serde_json::from_value::<DcsSettings>(over_limit)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceed 4096 opaque extensions"));
    }
}
