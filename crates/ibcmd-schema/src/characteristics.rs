//! Closed source-less characteristic field markers witnessed by the public corpus.

/// When the characteristic's source has been removed, the platform retains this
/// standard-field marker as a number instead of inventing a metadata path.
/// Both native editions of `corpus-orphan-characteristics` carry this value.
pub const ORPHAN_STANDARD_ATTRIBUTE_MARKER: i32 = -2;
pub const SOURCE_ATTRIBUTE: &str = "from";
pub const FILTER_VALUE: &str = "TypesFilterValue";

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

/// Bounded incremental recognition of the exact orphan marker, ignoring only
/// outer whitespace. Callers can retain a witnessed marker at XML boundaries
/// even when later unknown content makes the complete field noncanonical.
#[derive(Default)]
pub struct OrphanMarkerProbe {
    state: MarkerProbeState,
}

#[derive(Clone, Copy, Default)]
enum MarkerProbeState {
    #[default]
    Leading,
    Minus,
    Complete,
    Trailing,
    Rejected,
}

impl OrphanMarkerProbe {
    pub fn observe(&mut self, text: &str) {
        for character in text.chars() {
            self.state = match self.state {
                MarkerProbeState::Leading if character.is_whitespace() => MarkerProbeState::Leading,
                MarkerProbeState::Leading if character == '-' => MarkerProbeState::Minus,
                MarkerProbeState::Minus if character == '2' => MarkerProbeState::Complete,
                MarkerProbeState::Complete | MarkerProbeState::Trailing
                    if character.is_whitespace() =>
                {
                    MarkerProbeState::Trailing
                }
                _ => MarkerProbeState::Rejected,
            };
            if matches!(self.state, MarkerProbeState::Rejected) {
                return;
            }
        }
    }

    pub fn is_marker(&self) -> bool {
        matches!(
            self.state,
            MarkerProbeState::Complete | MarkerProbeState::Trailing
        )
    }
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
    #[test]
    fn bounded_probe_agrees_with_the_closed_domain_and_never_searches_uuid_substrings() {
        for text in [
            "",
            "-2",
            " -2 ",
            "-1",
            "-02",
            "-2tail",
            "tail-2",
            "0:11111111-2111-4111-8111-111111111111",
        ] {
            let mut probe = OrphanMarkerProbe::default();
            probe.observe(text);
            assert_eq!(
                probe.is_marker(),
                orphan_standard_attribute_marker(text.trim()).is_some(),
                "{text}"
            );
        }
        let mut probe = OrphanMarkerProbe::default();
        for text in ["0:11111111", "-2", "111-4111-8111-111111111111"] {
            probe.observe(text);
            assert!(!probe.is_marker());
        }
        let mut probe = OrphanMarkerProbe::default();
        for text in [" ", "-", "2", " "] {
            probe.observe(text);
        }
        assert!(probe.is_marker());
    }
}
