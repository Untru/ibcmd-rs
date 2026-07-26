//! Canonical, source-independent model of metadata `Characteristics`.
//!
//! Native slots and XML qualified names intentionally do not appear here.
//! Adapters resolve every reference before constructing this model.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::identity::ObjectUuid;

/// Maximum number of characteristics retained by one metadata owner.
pub const MAX_CHARACTERISTICS: usize = 16_384;
/// Maximum UTF-8 length of one resolved metadata reference.
pub const MAX_CHARACTERISTIC_REFERENCE_BYTES: usize = 16_384;
/// Maximum UTF-8 length of one string filter value.
pub const MAX_CHARACTERISTIC_FILTER_BYTES: usize = 16_777_216;

/// Failure to construct a canonical characteristics model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacteristicsBuildError {
    TooManyItems { actual: usize },
    EmptyReference,
    ReferenceTooLong { actual: usize },
    FilterTooLong { actual: usize },
    InvalidSourceReference,
    ReferenceOutsideSource,
}

impl Display for CharacteristicsBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyItems { actual } => write!(
                formatter,
                "characteristics exceed {MAX_CHARACTERISTICS} items (actual {actual})"
            ),
            Self::EmptyReference => formatter.write_str("characteristic reference is empty"),
            Self::ReferenceTooLong { actual } => write!(
                formatter,
                "characteristic reference exceeds {MAX_CHARACTERISTIC_REFERENCE_BYTES} bytes (actual {actual})"
            ),
            Self::FilterTooLong { actual } => write!(
                formatter,
                "characteristic filter exceeds {MAX_CHARACTERISTIC_FILTER_BYTES} bytes (actual {actual})"
            ),
            Self::InvalidSourceReference => {
                formatter.write_str("characteristic source reference is not canonical")
            }
            Self::ReferenceOutsideSource => {
                formatter.write_str("characteristic field reference is outside its source")
            }
        }
    }
}

impl Error for CharacteristicsBuildError {}

/// A resolved metadata reference and optional exact source UUID provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacteristicReference {
    path: Box<str>,
    source_uuid: Option<ObjectUuid>,
}

impl CharacteristicReference {
    pub fn new(
        path: &str,
        source_uuid: Option<ObjectUuid>,
    ) -> Result<Self, CharacteristicsBuildError> {
        if path.is_empty() {
            return Err(CharacteristicsBuildError::EmptyReference);
        }
        if path.len() > MAX_CHARACTERISTIC_REFERENCE_BYTES {
            return Err(CharacteristicsBuildError::ReferenceTooLong { actual: path.len() });
        }
        Ok(Self {
            path: path.into(),
            source_uuid,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn source_uuid(&self) -> Option<ObjectUuid> {
        self.source_uuid
    }
}

/// Closed semantic representation of the two native field sentinels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacteristicFieldSentinel {
    /// The field is explicitly undefined.
    Undefined,
    /// The field is present but has no selected value.
    Empty,
}

/// Either a resolved field reference or a closed sentinel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacteristicField {
    Reference(CharacteristicReference),
    Sentinel(CharacteristicFieldSentinel),
}

impl CharacteristicField {
    pub const fn reference(&self) -> Option<&CharacteristicReference> {
        match self {
            Self::Reference(reference) => Some(reference),
            Self::Sentinel(_) => None,
        }
    }
}

/// The exactly observed design-time filter-value union.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacteristicFilterValue {
    String(Box<str>),
    /// `None` represents the exact empty DesignTimeRef.
    DesignTimeRef(Option<CharacteristicReference>),
}

impl CharacteristicFilterValue {
    pub fn string(value: &str) -> Result<Self, CharacteristicsBuildError> {
        if value.len() > MAX_CHARACTERISTIC_FILTER_BYTES {
            return Err(CharacteristicsBuildError::FilterTooLong {
                actual: value.len(),
            });
        }
        Ok(Self::String(value.into()))
    }
}

/// Source and fields describing characteristic types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacteristicTypes {
    source: CharacteristicReference,
    key_field: CharacteristicField,
    types_filter_field: CharacteristicField,
    types_filter_value: CharacteristicFilterValue,
    data_path_field: CharacteristicField,
    multiple_values_use_field: CharacteristicField,
}

impl CharacteristicTypes {
    pub fn new(
        source: CharacteristicReference,
        key_field: CharacteristicField,
        types_filter_field: CharacteristicField,
        types_filter_value: CharacteristicFilterValue,
        data_path_field: CharacteristicField,
        multiple_values_use_field: CharacteristicField,
    ) -> Result<Self, CharacteristicsBuildError> {
        validate_source(&source)?;
        for field in [
            &key_field,
            &types_filter_field,
            &data_path_field,
            &multiple_values_use_field,
        ] {
            validate_field_ancestry(field, &source)?;
        }
        Ok(Self {
            source,
            key_field,
            types_filter_field,
            types_filter_value,
            data_path_field,
            multiple_values_use_field,
        })
    }

    pub const fn source(&self) -> &CharacteristicReference {
        &self.source
    }
    pub const fn key_field(&self) -> &CharacteristicField {
        &self.key_field
    }
    pub const fn types_filter_field(&self) -> &CharacteristicField {
        &self.types_filter_field
    }
    pub const fn types_filter_value(&self) -> &CharacteristicFilterValue {
        &self.types_filter_value
    }
    pub const fn data_path_field(&self) -> &CharacteristicField {
        &self.data_path_field
    }
    pub const fn multiple_values_use_field(&self) -> &CharacteristicField {
        &self.multiple_values_use_field
    }
}

/// Source and fields describing characteristic values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacteristicValues {
    source: CharacteristicReference,
    object_field: CharacteristicField,
    type_field: CharacteristicField,
    value_field: CharacteristicField,
    multiple_values_key_field: CharacteristicField,
    multiple_values_order_field: CharacteristicField,
}

impl CharacteristicValues {
    pub fn new(
        source: CharacteristicReference,
        object_field: CharacteristicField,
        type_field: CharacteristicField,
        value_field: CharacteristicField,
        multiple_values_key_field: CharacteristicField,
        multiple_values_order_field: CharacteristicField,
    ) -> Result<Self, CharacteristicsBuildError> {
        validate_source(&source)?;
        for field in [
            &object_field,
            &type_field,
            &value_field,
            &multiple_values_key_field,
            &multiple_values_order_field,
        ] {
            validate_field_ancestry(field, &source)?;
        }
        Ok(Self {
            source,
            object_field,
            type_field,
            value_field,
            multiple_values_key_field,
            multiple_values_order_field,
        })
    }

    pub const fn source(&self) -> &CharacteristicReference {
        &self.source
    }
    pub const fn object_field(&self) -> &CharacteristicField {
        &self.object_field
    }
    pub const fn type_field(&self) -> &CharacteristicField {
        &self.type_field
    }
    pub const fn value_field(&self) -> &CharacteristicField {
        &self.value_field
    }
    pub const fn multiple_values_key_field(&self) -> &CharacteristicField {
        &self.multiple_values_key_field
    }
    pub const fn multiple_values_order_field(&self) -> &CharacteristicField {
        &self.multiple_values_order_field
    }
}

/// One characteristic, with the two required source groups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Characteristic {
    types: CharacteristicTypes,
    values: CharacteristicValues,
}

impl Characteristic {
    pub const fn new(types: CharacteristicTypes, values: CharacteristicValues) -> Self {
        Self { types, values }
    }

    pub const fn types(&self) -> &CharacteristicTypes {
        &self.types
    }
    pub const fn values(&self) -> &CharacteristicValues {
        &self.values
    }
}

/// Deterministically source-ordered characteristics of one owner.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Characteristics {
    items: Vec<Characteristic>,
}

impl Characteristics {
    pub fn new(items: Vec<Characteristic>) -> Result<Self, CharacteristicsBuildError> {
        if items.len() > MAX_CHARACTERISTICS {
            return Err(CharacteristicsBuildError::TooManyItems {
                actual: items.len(),
            });
        }
        Ok(Self { items })
    }

    pub fn items(&self) -> &[Characteristic] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn validate_source(source: &CharacteristicReference) -> Result<(), CharacteristicsBuildError> {
    let parts = source.path().split('.').collect::<Vec<_>>();
    let valid = match parts.as_slice() {
        [family, owner] => !family.is_empty() && !owner.is_empty(),
        [family, owner, "TabularSection", section] => {
            !family.is_empty() && !owner.is_empty() && !section.is_empty()
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(CharacteristicsBuildError::InvalidSourceReference)
}

fn validate_field_ancestry(
    field: &CharacteristicField,
    source: &CharacteristicReference,
) -> Result<(), CharacteristicsBuildError> {
    let Some(reference) = field.reference() else {
        return Ok(());
    };
    reference
        .path()
        .strip_prefix(source.path())
        .is_some_and(|suffix| suffix.starts_with('.') && suffix.len() > 1)
        .then_some(())
        .ok_or(CharacteristicsBuildError::ReferenceOutsideSource)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(path: &str) -> CharacteristicReference {
        CharacteristicReference::new(path, None).unwrap()
    }

    fn field(path: &str) -> CharacteristicField {
        CharacteristicField::Reference(reference(path))
    }

    #[test]
    fn constructors_enforce_both_source_ancestries() {
        let types = CharacteristicTypes::new(
            reference("Catalog.Types"),
            field("Catalog.Types.Attribute.Key"),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty),
            CharacteristicFilterValue::string("filter").unwrap(),
            field("Catalog.Types.Attribute.Path"),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined),
        )
        .unwrap();
        let values = CharacteristicValues::new(
            reference("Document.Values.TabularSection.Rows"),
            field("Document.Values.TabularSection.Rows.StandardAttribute.Ref"),
            field("Document.Values.TabularSection.Rows.Attribute.Kind"),
            field("Document.Values.TabularSection.Rows.Attribute.Value"),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined),
        )
        .unwrap();
        let model = Characteristics::new(vec![Characteristic::new(types, values)]).unwrap();
        assert_eq!(model.items().len(), 1);
    }

    #[test]
    fn reference_from_another_source_is_rejected() {
        let error = CharacteristicTypes::new(
            reference("Catalog.Types"),
            field("Catalog.Other.Attribute.Key"),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty),
            CharacteristicFilterValue::string("").unwrap(),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty),
            CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty),
        )
        .unwrap_err();
        assert_eq!(error, CharacteristicsBuildError::ReferenceOutsideSource);
    }
}
