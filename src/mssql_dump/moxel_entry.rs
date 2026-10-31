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

/// Explicit generated chart labels selected by the export surface.
/// Conversion into the decoder policy is exhaustive and independent of host state.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, clap::ValueEnum)]
pub enum MoxelChartSeriesAutomaticName {
    #[value(name = "english")]
    RepublishedInEnglish,
    #[value(name = "russian")]
    Russian,
    #[default]
    #[value(name = "stored")]
    Stored,
}

impl From<MoxelChartSeriesAutomaticName> for ibcmd_moxel::MoxelChartSeriesAutomaticName {
    fn from(policy: MoxelChartSeriesAutomaticName) -> Self {
        match policy {
            MoxelChartSeriesAutomaticName::Stored => Self::Stored,
            MoxelChartSeriesAutomaticName::Russian => Self::Russian,
            MoxelChartSeriesAutomaticName::RepublishedInEnglish => Self::RepublishedInEnglish,
        }
    }
}

impl MoxelChartSeriesAutomaticName {
    #[cfg(test)]
    const fn host() -> Self {
        Self::Stored
    }
}

#[cfg(all(test, feature = "mssql-live-tests"))]
use std::collections::BTreeSet;

#[cfg(all(test, feature = "mssql-live-tests"))]
use crate::module_blob::{LocalizedString, SpreadsheetNumberFormatHint};

pub(crate) fn extract_moxel_spreadsheet_xml(
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
pub(crate) fn try_extract_moxel_spreadsheet_xml_with_generated_types(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    generated_types: &BTreeMap<String, String>,
) -> Result<String, MxlDiagnostic> {
    try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
        bytes,
        object_refs,
        generated_types,
        MoxelChartSeriesAutomaticName::Stored,
    )
}

/// Packed export entry point retaining the root codec and built-in type table.
pub(crate) fn try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    generated_types: &BTreeMap<String, String>,
    policy: MoxelChartSeriesAutomaticName,
) -> Result<String, MxlDiagnostic> {
    let body = crate::compiler::bodies::mxl::decode_compatible_mxl(bytes)
        .map_err(|error| binary_container_diagnostic(error.to_string()))?;
    // An 8.5 spreadsheet spells its colour and font tuples one revision later.
    let text =
        super::form::layout_8_5_1::rewrite_primitives_8_5_1_in_place(body.native_body_text());
    extract_moxel_body_xml_with_policy(
        &text,
        object_refs,
        generated_types,
        super::builtin_type_reference,
        None,
        policy.into(),
    )
}

pub(crate) fn extract_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = crate::compiler::bodies::mxl::decode_compatible_mxl(bytes).ok()?;
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
pub(crate) fn extract_inflated_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = crate::compiler::bodies::mxl::decode_inflated_compatible_mxl(bytes).ok()?;
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
#[cfg(test)]
pub(super) fn parse_moxel_spreadsheet_text(
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

#[cfg(all(test, feature = "mssql-live-tests"))]
pub(super) fn spreadsheet_number_format_hint_from_text(
    raw_text: &str,
) -> Option<SpreadsheetNumberFormatHint> {
    let body_start = raw_text.find('{')?;
    let body = raw_text[body_start..].trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(body, 0)?;
    if fields.first()?.trim() != "8" {
        return None;
    }
    let declared_column_count = fields.get(2)?.trim().parse::<usize>().ok()? + 1;
    let rows = parse_moxel_rows(&fields);
    if rows.is_empty() {
        return None;
    }
    let (column_sets, _, _) = parse_moxel_column_sets(&fields);
    let style_refs = parse_moxel_style_refs(&fields, &BTreeMap::new());
    let default_format = parse_moxel_default_format(&fields, &BTreeMap::new());
    let observed_column_count = rows
        .iter()
        .flat_map(|row| row.cells.iter().map(|cell| cell.column_index + 1))
        .max()
        .unwrap_or(0);
    let column_count = if observed_column_count > 0 {
        observed_column_count
    } else {
        declared_column_count
    };
    let default_format_width = parse_moxel_default_format_width(
        &fields,
        moxel_column_format_slots(&column_sets, declared_column_count),
    );
    let column_sets = if column_sets.is_empty() {
        default_moxel_column_sets(column_count)
    } else {
        column_sets
    };
    let drawings = parse_moxel_drawings(&fields, &BTreeMap::new());
    // A cell note is a drawing record of its own -- `drawingType` `Comment`,
    // its own `<formatIndex>` -- so the format it names is a drawing format,
    // and that format's members 1, 3 and 4 are `drawingBorder`,
    // `drawingHave*Border` and `print` rather than
    // `leftBorder`/`rightBorder`/`bottomBorder`. This set was built from the
    // `<drawing>` list alone, which left every note-only format read as a
    // cell one.
    //
    // Evidence: of the 23 `<format>` elements that publish `<print>` across
    // ERP УХ 3.2.12.6, 1С:УТ 11.5.27.75, Документооборот КОРП 3.0.21.3 and
    // БСП demo/base, eleven sit in documents that carry no `<drawing>` at
    // all, and in every one the format they name is exactly the
    // `<formatIndex>` of a `<note>`. In
    // `Reports/РасчетСтоимостиЧистыхАктивов/Templates/РасчетСтоимостиЧистыхАктивов`
    // the whole `<formatIndex>` set is `{1..8, 31}`, disjoint from the `<f>`
    // set the cells cite, and format 31 is the one that publishes
    // `<print>false</print>`.
    //
    // A note-named format is only a drawing format where **no cell names it
    // too**: 1С:УТ 11.5.27.75's `Documents/ИзменениеАссортимента/Templates/
    // ЗагрузкаИзФайла` and four documents like it share one format between a
    // note and their own report-header cells, and the platform renders it as
    // a cell format there -- `leftBorder`/`rightBorder` and no
    // `drawingBorder`. In `РасчетСтоимостиЧистыхАктивов` the two sets are
    // disjoint, which is what makes format 31 a drawing format.
    let cell_format_indices = rows
        .iter()
        .flat_map(|row| {
            std::iter::once(row.format_index).chain(row.cells.iter().map(|cell| cell.format_index))
        })
        .collect::<BTreeSet<_>>();
    let mut drawing_format_indices = drawings
        .iter()
        .map(|drawing| (drawing.format_index, MoxelFormatRefKind::Drawing))
        .collect::<MoxelFormatRefKinds>();
    for index in rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .filter_map(|cell| cell.note.as_ref().map(|note| note.format_index))
        .filter(|index| *index > 0 && !cell_format_indices.contains(index))
    {
        // The set is read against the entry's own 1-based place in the
        // stored table, which a note's reference carries one higher; see the
        // same conversion in `parse_moxel_spreadsheet`.
        drawing_format_indices
            .entry(index - 1)
            .or_insert(MoxelFormatRefKind::Note);
    }
    let column_format_slots = moxel_column_format_slots(&column_sets, column_count);
    let _sparse_source_format_refs = moxel_uses_sparse_source_format_refs(
        &column_sets,
        column_count,
        &rows,
        &default_format,
        default_format_width,
    );
    let number_format_refs = parse_moxel_number_format_refs(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
    );
    let slots = number_format_refs
        .iter()
        .map(|slot| {
            slot.iter()
                .map(|value| LocalizedString {
                    lang: value.lang.clone(),
                    content: value.content.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for index in 0..fields.len() {
        if let Some(nested) = split_1c_braced_fields(fields[index], 0) {
            let Some(count) = nested
                .first()
                .and_then(|field| field.trim().parse::<usize>().ok())
            else {
                continue;
            };
            if count > column_count
                && count <= 2048
                && nested.len() == count + 1
                && nested.iter().skip(1).all(|field| {
                    parse_moxel_format(field, &style_refs, &number_format_refs).is_some()
                })
            {
                return Some(SpreadsheetNumberFormatHint {
                    slots,
                    format_slot_indices: nested
                        .iter()
                        .skip(1)
                        .map(|field| parse_moxel_format_number_format_index(field))
                        .collect(),
                });
            }
        }
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count <= column_count || count > 2048 || index + count >= fields.len() {
            continue;
        }
        let format_fields = &fields[index + 1..=index + count];
        if format_fields
            .iter()
            .all(|field| parse_moxel_format(field, &style_refs, &number_format_refs).is_some())
        {
            return Some(SpreadsheetNumberFormatHint {
                slots,
                format_slot_indices: format_fields
                    .iter()
                    .map(|field| parse_moxel_format_number_format_index(field))
                    .collect(),
            });
        }
    }
    None
}

#[cfg(all(test, feature = "mssql-live-tests"))]
#[derive(Debug, Clone)]
pub(crate) struct DebugMoxelSpreadsheetSummary {
    pub column_count: usize,
    pub column_format_slots: usize,
    pub source_column_format_offset: usize,
    pub default_format_index: Option<usize>,
    pub column_formats_len: usize,
    pub formats_len: usize,
    pub number_format_indices: Vec<usize>,
    pub first_rows: Vec<String>,
    pub first_columns: Vec<String>,
}

#[cfg(all(test, feature = "mssql-live-tests"))]
pub(crate) fn debug_moxel_spreadsheet_summary_from_blob(
    blob: &[u8],
) -> Option<DebugMoxelSpreadsheetSummary> {
    let inflated = super::inflate_raw_deflate(blob).ok()?;
    let raw_text = String::from_utf8(inflated).ok()?;
    let body_start = raw_text.find('{')?;
    let body = raw_text[body_start..].trim_start_matches('\u{feff}');
    let spreadsheet = parse_moxel_spreadsheet_text(body, &BTreeMap::new())?;
    let first_rows = spreadsheet
        .rows
        .iter()
        .take(6)
        .map(|row| {
            let first_cells = row
                .cells
                .iter()
                .take(4)
                .map(|cell| format!("c{}:f{}", cell.column_index, cell.format_index))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "r{}:f{}:{}",
                row.index,
                row.format_index,
                if first_cells.is_empty() {
                    "<empty>".to_string()
                } else {
                    first_cells
                }
            )
        })
        .collect::<Vec<_>>();
    let first_columns = spreadsheet
        .column_sets
        .iter()
        .flat_map(|set| set.columns.iter())
        .take(8)
        .map(|column| {
            format!(
                "c{}:{}->{}",
                column.index,
                column.format_index,
                column.source_format_index.unwrap_or(column.format_index)
            )
        })
        .collect::<Vec<_>>();
    let format_count = spreadsheet
        .default_format_index
        .unwrap_or(0)
        .max(spreadsheet.column_formats.len() + spreadsheet.formats.len())
        .max(1);
    let number_format_indices = (1..=format_count)
        .filter(|format_index| {
            let format = moxel_format_for_index(&spreadsheet, *format_index);
            !format.number_format.is_empty() || !format.edit_format.is_empty()
        })
        .collect::<Vec<_>>();
    Some(DebugMoxelSpreadsheetSummary {
        column_count: spreadsheet.column_count,
        column_format_slots: moxel_column_format_slots(
            &spreadsheet.column_sets,
            spreadsheet.column_count,
        ),
        source_column_format_offset: moxel_source_column_format_offset(&spreadsheet.column_sets),
        default_format_index: spreadsheet.default_format_index,
        column_formats_len: spreadsheet.column_formats.len(),
        formats_len: spreadsheet.formats.len(),
        number_format_indices,
        first_rows,
        first_columns,
    })
}

#[cfg(all(test, feature = "mssql-live-tests"))]
#[derive(Debug, Clone)]
pub(crate) struct DebugMoxelNumberFormatUsage {
    pub slots: Vec<String>,
    pub format_slot_indices: Vec<Option<usize>>,
}

#[cfg(all(test, feature = "mssql-live-tests"))]
pub(crate) fn debug_moxel_number_format_usage(
    raw_text: &str,
) -> Option<DebugMoxelNumberFormatUsage> {
    let hint = spreadsheet_number_format_hint_from_text(raw_text)?;
    Some(DebugMoxelNumberFormatUsage {
        slots: hint
            .slots
            .iter()
            .map(|slot| {
                if slot.is_empty() {
                    "<empty>".to_string()
                } else {
                    slot.iter()
                        .map(|value| format!("{}={}", value.lang, value.content))
                        .collect::<Vec<_>>()
                        .join("|")
                }
            })
            .collect(),
        format_slot_indices: hint.format_slot_indices,
    })
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
        let body =
            crate::compiler::bodies::mxl_native::write_native_moxel_body(source.as_bytes(), None)
                .unwrap();
        assert!(
            body.replace("\r\n", "")
                .contains("{\"Pattern\",{\"#\",e61ef7b8-f3e1-4f4b-8ac7-676e90524997}}"),
            "{body}"
        );
        let plain = format!("MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{body}");
        let blob = crate::compiler::families::native::deflate_bytes(plain.as_bytes()).unwrap();
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

#[cfg(test)]
mod series_projection_tests {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ut-chart-policy/explicit-english-zero-series/projection_tests.rs"
    ));
}
