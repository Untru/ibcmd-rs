//! Independently measured SQL extension envelopes. The legacy header is
//! witnessed on 8.3.27; exact 8.5.1.1150 also creates the current header.

pub struct ExtensionRegistryLayout;
impl ExtensionRegistryLayout {
    pub const LEGACY_HEADER: [u8; 3] = [0xa1, 0x9a, 0x08];
    pub const CURRENT_HEADER: [u8; 3] = [0xa2, 0x9a, 0x08];
    pub const BODY_HEADER_BYTES: usize = 15;
    pub const METADATA_ENCODING: usize = 13;
    pub const METADATA_UNITS: usize = 14;
    pub const TRAILER_BYTES: usize = 3;
    pub const UTF16_MARKER: u8 = 0x97;
    pub const UTF8_MARKER: u8 = 0x9a;
    pub const EMPTY_STRING_MARKER: u8 = 0x81;

    pub fn accepts_sql_header(header: &[u8]) -> bool {
        header == Self::LEGACY_HEADER || header == Self::CURRENT_HEADER
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_independently_measured_sql_headers_are_admitted() {
        assert!(ExtensionRegistryLayout::accepts_sql_header(&[
            0xa1, 0x9a, 8
        ]));
        assert!(ExtensionRegistryLayout::accepts_sql_header(&[
            0xa2, 0x9a, 8
        ]));
        for tag in [0, 0xa0, 0xa3, 0xff] {
            assert!(!ExtensionRegistryLayout::accepts_sql_header(&[
                tag, 0x9a, 8
            ]));
        }
        assert!(!ExtensionRegistryLayout::accepts_sql_header(&[
            0xa2, 0x9a, 9
        ]));
    }
}
