//! The spreadsheet probes of the root's live blob comparisons (`mssql`'s
//! `live_compares_native_and_packed_*` tests), formerly the live-test items of
//! the exporter's `moxel_entry.rs`. Only those root tests read them, so they
//! stay in the root when the exporter moves into its crate (#559).

use std::collections::{BTreeMap, BTreeSet};

use ibcmd_1c_vocab::inflate_raw_deflate;
use ibcmd_metadata_blob::{LocalizedString, SpreadsheetNumberFormatHint};
use ibcmd_moxel::*;

use super::moxel_entry::parse_moxel_spreadsheet_text;

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

pub(crate) fn debug_moxel_spreadsheet_summary_from_blob(
    blob: &[u8],
) -> Option<DebugMoxelSpreadsheetSummary> {
    let inflated = inflate_raw_deflate(blob).ok()?;
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

#[derive(Debug, Clone)]
pub(crate) struct DebugMoxelNumberFormatUsage {
    pub slots: Vec<String>,
    pub format_slot_indices: Vec<Option<usize>>,
}

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
