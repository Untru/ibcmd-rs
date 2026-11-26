//! Presence of a native metadata collection's outer envelope.
//!
//! The payload is deliberately left to the owning family's decoder. An absent
//! envelope is distinct from a present collection whose payload contains no items.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeOptionalMetadataCollection<'a> {
    Absent,
    Present(&'a str),
}

impl<'a> NativeOptionalMetadataCollection<'a> {
    /// Admit only the complete outer envelope after lexical field splitting.
    pub fn from_fields(fields: &[&'a str]) -> Option<Self> {
        match fields {
            [marker] if marker.trim() == "0" => Some(Self::Absent),
            [marker, payload] if marker.trim() == "1" => Some(Self::Present(payload)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NativeOptionalMetadataCollection as Collection;

    #[test]
    fn standard_attribute_presence_distinguishes_absence_from_present_empty_payload() {
        assert_eq!(Collection::from_fields(&[" 0 "]), Some(Collection::Absent));
        assert_eq!(
            Collection::from_fields(&["1", "{1,0}"]),
            Some(Collection::Present("{1,0}"))
        );
        for fields in [
            vec![],
            vec!["0", "{1,0}"],
            vec!["1"],
            vec!["2", "{1,0}"],
            vec!["1", "{1,0}", "0"],
        ] {
            assert_eq!(Collection::from_fields(&fields), None);
        }
    }
}
