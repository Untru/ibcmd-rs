//! The test of `compiler::bodies::mxl` (in `ibcmd-compiler`) that reads a
//! compiled body back through the root's exporter
//! (`mssql_dump::extract_moxel_spreadsheet_xml`).

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::compiler::bodies::mxl::{MxlCodecProfile, compile_mxl, decode_mxl};

    // The codec's own constant is private to it.
    const MOXCEL_HEADER: &[u8] = b"MOXCEL\0\x08\0\x01\0\x0c\0";

    const SIMPLE_SPREADSHEET: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns><size>3</size></columns>
	<rowsItem><index>0</index><row>
		<c><c><f>0</f><tl><v8:item><v8:lang>en</v8:lang><v8:content>Hello</v8:content></v8:item></tl></c></c>
		<c><i>2</i><c><f>0</f><parameter>Name</parameter></c></c>
	</row></rowsItem>
	<defaultFormatIndex>1</defaultFormatIndex>
	<format><width>72</width></format>
</document>"#;

    #[test]
    fn spreadsheet_compiles_deterministically_and_roundtrips_semantically() {
        let profile = MxlCodecProfile::fixture();
        let first = compile_mxl(&profile, SIMPLE_SPREADSHEET, None, None).unwrap();
        let second = compile_mxl(&profile, SIMPLE_SPREADSHEET, None, None).unwrap();
        assert_eq!(first, second);

        let decoded = decode_mxl(&profile, &first).unwrap();
        assert!(decoded.plaintext().starts_with(MOXCEL_HEADER));
        // Field 2 is the platform's constant 12 whatever the column count.
        assert!(decoded.native_body_text().starts_with("{8,1,12,"));
        assert_eq!(decoded.declared_columns(), 13);
        assert!(decoded.native_fields() >= 8);

        let xml = crate::mssql_dump::extract_moxel_spreadsheet_xml(&first, &BTreeMap::new())
            .expect("evidenced MOXCEL body must remain exportable");
        assert!(xml.contains("<v8:content>Hello</v8:content>"));
        // The parameter's content round-trips, but not the element it is
        // spelled with: this compiler stores the cell on format slot 0, and a
        // cell whose format states no `fillType` `Parameter` publishes its
        // container as the text list -- the rule
        // `restore_moxel_text_of_cells_whose_format_states_no_parameter`
        // reads off the eight native corpora, where every one of the 94 332
        // published `<parameter>` cells names a `fillType`-`Parameter` format
        // and no cell on slot 0 publishes one at all. The assertion this
        // replaces read the container's empty leading language instead, which
        // is what a one-item text list stores too, and spelled three native
        // ERP УХ cells as parameters against the platform's own `<tl>`.
        assert!(xml.contains("<v8:content>Name</v8:content>"), "{xml}");
    }
}
