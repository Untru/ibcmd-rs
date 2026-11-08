//! Closed source-less characteristic field markers witnessed by the public corpus.

/// When the characteristic's source has been removed, the platform retains this
/// standard-field marker as a number instead of inventing a metadata path.
/// Both native editions of `corpus-orphan-characteristics` carry this value.
pub const ORPHAN_STANDARD_ATTRIBUTE_MARKER: i32 = -2;
pub const SOURCE_ATTRIBUTE: &str = "from";

/// The explicit UUID form of a characteristic field payload.
pub fn is_uuid_field_kind(text: &str) -> bool {
    text == "0"
}

pub fn orphan_standard_attribute_marker(text: &str) -> Option<i32> {
    (text == ORPHAN_STANDARD_ATTRIBUTE_MARKER.to_string())
        .then_some(ORPHAN_STANDARD_ATTRIBUTE_MARKER)
}

/// XML vocabulary for identity-bearing fields, excluding the filter union value.
pub const PROPERTY: &str = "Characteristics";
pub fn is_identity_field(name: &str) -> bool {
    matches!(
        name,
        "KeyField"
            | "TypesFilterField"
            | "DataPathField"
            | "MultipleValuesUseField"
            | "ObjectField"
            | "TypeField"
            | "ValueField"
            | "MultipleValuesKeyField"
            | "MultipleValuesOrderField"
    )
}
/// The detached UUID spelling has a closed physical discriminator.
pub fn detached_uuid(text: &str) -> Option<&str> {
    text.strip_prefix("0:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_marker_domain_is_exact_and_closed() {
        assert_eq!(orphan_standard_attribute_marker("-2"), Some(-2));
        for text in ["", "-1", "0", "1", "-3", "-02", "-2 ", " -2", "-2147483649"] {
            assert_eq!(orphan_standard_attribute_marker(text), None, "{text}");
        }
    }
}
