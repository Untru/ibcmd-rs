//! The root's side of the MOXCEL spreadsheet decoder, the `ibcmd-moxel` crate.
//!
//! The crate decodes a MOXCEL body and writes its XML. The entry points here
//! decode the compressed compatible-MXL container first
//! (`compiler::bodies::mxl`), apply the 8.5 primitive rewrite where the
//! export path did before, and hand the crate the root's built-in platform
//! type table (`builtin_type_reference`). `mssql_dump` reaches the crate's
//! items through this module, as it reached them when the decoder lived in
//! `mssql_dump/moxel.rs`.

use std::collections::BTreeMap;

pub(crate) use ibcmd_moxel::*;

#[doc(hidden)]
pub fn extract_moxel_spreadsheet_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    try_extract_moxel_spreadsheet_xml(bytes, object_refs).ok()
}

/// Decodes a compressed MOXCEL container, builds canonical spreadsheet IR and
/// then projects it to the already-evidenced XML writer.  Its diagnostics keep
/// decoder and writer failures distinct without changing the legacy `Option`
/// API used by the dump pipeline.
pub fn try_extract_moxel_spreadsheet_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Result<String, MxlDiagnostic> {
    try_extract_moxel_spreadsheet_xml_with_generated_types(
        bytes,
        object_refs,
        &NO_MOXEL_GENERATED_TYPES,
    )
}

/// The same extraction with the configuration's generated-type index in hand,
/// which is what a `{"#",<uuid>}` value type names.
#[doc(hidden)]
pub fn try_extract_moxel_spreadsheet_xml_with_generated_types(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    generated_types: &BTreeMap<String, String>,
) -> Result<String, MxlDiagnostic> {
    let body = ibcmd_compiler::compiler::bodies::mxl::decode_compatible_mxl(bytes)
        .map_err(|error| binary_container_diagnostic(error.to_string()))?;
    // An 8.5 spreadsheet spells its colour and font tuples one revision later.
    let text =
        super::form::layout_8_5_1::rewrite_primitives_8_5_1_in_place(body.native_body_text());
    extract_moxel_body_xml(
        &text,
        object_refs,
        generated_types,
        super::builtin_type_reference,
        None,
    )
}

#[doc(hidden)]
pub fn extract_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = ibcmd_compiler::compiler::bodies::mxl::decode_compatible_mxl(bytes).ok()?;
    extract_moxel_body_xml(
        body.native_body_text(),
        object_refs,
        &NO_MOXEL_GENERATED_TYPES,
        super::builtin_type_reference,
        trace_sink,
    )
    .ok()
}

/// Trace an already-inflated native MXL body retained by an offline dump.
/// Its MOXCEL framing is validated by the same codec routine as packed blobs.
#[doc(hidden)]
pub fn extract_inflated_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = ibcmd_compiler::compiler::bodies::mxl::decode_inflated_compatible_mxl(bytes).ok()?;
    extract_moxel_body_xml(
        body.native_body_text(),
        object_refs,
        &NO_MOXEL_GENERATED_TYPES,
        super::builtin_type_reference,
        trace_sink,
    )
    .ok()
}

/// [`ibcmd_moxel::parse_moxel_spreadsheet_text`] with the root's built-in
/// type table.
#[cfg(any(test, feature = "test-fixtures"))]
#[doc(hidden)]
pub fn parse_moxel_spreadsheet_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelSpreadsheet> {
    ibcmd_moxel::parse_moxel_spreadsheet_text(text, object_refs, super::builtin_type_reference)
}

/// [`ibcmd_moxel::parse_moxel_value_type`] with the root's built-in type
/// table.
#[cfg(test)]
pub(super) fn parse_moxel_value_type(
    text: &str,
    generated_types: &BTreeMap<String, String>,
) -> Option<MoxelValueType> {
    ibcmd_moxel::parse_moxel_value_type(text, generated_types, super::builtin_type_reference)
}

#[cfg(test)]
mod moxel_exact_parity_tests {
    use super::*;

    #[test]
    fn typed_extraction_reports_binary_container_as_decoder_failure() {
        let error = try_extract_moxel_spreadsheet_xml(&[0], &BTreeMap::new()).unwrap_err();

        assert_eq!(
            error.stage(),
            crate::mssql_dump::MxlDiagnosticStage::Decoder
        );
        assert_eq!(error.code(), "mxl.decoder.binary-container");
    }

    /// A template whose only format declares the value type "any catalog
    /// reference". Its stored descriptor `{"#",e61ef7b8-…}` is the built-in
    /// `cfg:CatalogRef` type id, which only the root's `builtin_type_reference`
    /// names, so the type set comes back only when every entry point hands the
    /// decoder that table.
    const CATALOG_REF_VALUE_TYPE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>1</f>
				</c>
			</c>
		</row>
	</rowsItem>
	<vgRows>0</vgRows>
	<format>
		<valueType>
			<v8:TypeSet xmlns:d4p1="http://v8.1c.ru/8.1/data/enterprise/current-config">d4p1:CatalogRef</v8:TypeSet>
		</valueType>
	</format>
</document>"#;

    #[test]
    fn every_entry_point_names_the_catalog_ref_type_set_through_the_root_table() {
        let source = r#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns><size>1</size></columns>
	<rowsItem><index>0</index><row><c><c><f>1</f></c></c></row></rowsItem>
	<format><valueType><v8:TypeSet xmlns:d4p1="http://v8.1c.ru/8.1/data/enterprise/current-config">d4p1:CatalogRef</v8:TypeSet></valueType></format>
</document>"#;
        let body = ibcmd_compiler::compiler::bodies::mxl_native::write_native_moxel_body(
            source.as_bytes(),
            None,
        )
        .unwrap();
        assert!(
            body.replace("\r\n", "")
                .contains("{\"Pattern\",{\"#\",e61ef7b8-f3e1-4f4b-8ac7-676e90524997}}"),
            "{body}"
        );
        let plain = format!("MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{body}");
        let blob =
            ibcmd_compiler_families::compiler::families::native::deflate_bytes(plain.as_bytes())
                .unwrap();
        let object_refs = BTreeMap::new();

        let exported = try_extract_moxel_spreadsheet_xml(&blob, &object_refs).unwrap();
        assert_eq!(
            exported,
            format!(
                "\u{feff}{}",
                CATALOG_REF_VALUE_TYPE_XML.replace('\n', "\r\n")
            )
        );
        assert_eq!(
            try_extract_moxel_spreadsheet_xml_with_generated_types(
                &blob,
                &object_refs,
                &BTreeMap::new()
            )
            .unwrap(),
            exported
        );
        assert_eq!(
            extract_moxel_spreadsheet_xml_with_line_trace(&blob, &object_refs, None).unwrap(),
            exported
        );
        assert_eq!(
            extract_inflated_moxel_spreadsheet_xml_with_line_trace(
                plain.as_bytes(),
                &object_refs,
                None
            )
            .unwrap(),
            exported
        );
        let spreadsheet = parse_moxel_spreadsheet_text(&body, &object_refs).unwrap();
        assert!(
            spreadsheet.value_types
                == vec![MoxelValueType {
                    items: vec![MoxelValueTypeItem::ConfigTypeSet("CatalogRef")],
                }]
        );
    }
}
