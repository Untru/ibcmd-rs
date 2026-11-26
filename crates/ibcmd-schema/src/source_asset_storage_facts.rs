//! Closed source-asset storage policies shared by physical decoders and writers.

/// Text-code admission is tied to the predefined source type. Numeric codes
/// and other owner families retain their spelling without catalog truncation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredefinedTextCodePolicy {
    Preserve,
    CatalogCodeLength,
}

impl PredefinedTextCodePolicy {
    pub fn for_source_type(xsi_type: &str) -> Self {
        match xsi_type {
            "CatalogPredefinedItems" => Self::CatalogCodeLength,
            _ => Self::Preserve,
        }
    }

    /// A zero or absent catalog length preserves the stored string. A positive
    /// length limits characters, rather than bytes, in every nested text code.
    pub fn maximum_text_code_length(self, declared_length: Option<usize>) -> Option<usize> {
        match self {
            Self::CatalogCodeLength => declared_length.filter(|length| *length > 0),
            Self::Preserve => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PredefinedTextCodePolicy;

    #[test]
    fn only_positive_catalog_string_code_length_admits_truncation() {
        let catalog = PredefinedTextCodePolicy::for_source_type("CatalogPredefinedItems");
        assert_eq!(catalog.maximum_text_code_length(None), None);
        assert_eq!(catalog.maximum_text_code_length(Some(0)), None);
        assert_eq!(catalog.maximum_text_code_length(Some(5)), Some(5));
        for source_type in ["ChartOfAccountsPredefinedItems", "unknown"] {
            let policy = PredefinedTextCodePolicy::for_source_type(source_type);
            assert_eq!(policy.maximum_text_code_length(Some(5)), None);
        }
    }
}
