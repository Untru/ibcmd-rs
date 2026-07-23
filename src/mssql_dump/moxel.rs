use super::*;

pub(super) struct MoxelSpreadsheet {
    pub(super) column_count: usize,
    pub(super) column_sets: Vec<MoxelColumnSet>,
    pub(super) column_formats: Vec<MoxelFormat>,
    pub(super) extra_formats: BTreeMap<usize, MoxelFormat>,
    pub(super) default_format_width: Option<usize>,
    pub(super) default_format: MoxelFormat,
    pub(super) formats: Vec<MoxelFormat>,
    pub(super) rows: Vec<MoxelRow>,
    pub(super) vertical_groups: Vec<MoxelVerticalGroup>,
    pub(super) merges: Vec<MoxelMerge>,
    pub(super) horizontal_unmerges: Vec<MoxelMerge>,
    pub(super) vertical_unmerges: Vec<MoxelMerge>,
    pub(super) named_items: Vec<MoxelNamedItem>,
    #[allow(dead_code)]
    pub(super) areas: Vec<MoxelArea>,
    pub(super) print_area: Option<MoxelArea>,
    pub(super) print_settings: Option<MoxelPrintSettings>,
    pub(super) lines: Vec<MoxelLine>,
    pub(super) fonts: Vec<MoxelFont>,
    pub(super) drawings: Vec<MoxelDrawing>,
    pub(super) pictures: Vec<MoxelPicture>,
    pub(super) empty_headers_footers: bool,
    pub(super) header_footer_format_index: Option<usize>,
    pub(super) default_format_index: Option<usize>,
    pub(super) source_format_map: Option<MoxelSourceFormatMap>,
    pub(super) height: usize,
}

pub(super) struct MoxelSourceFormatMap {
    source_to_internal: Vec<usize>,
    internal_to_source: Vec<usize>,
    output_source_order: Vec<usize>,
}

impl MoxelSourceFormatMap {
    fn try_new(
        format_count: usize,
        internal_column_sources: &[usize],
        output_column_sources: &[usize],
    ) -> Option<Self> {
        // A non-identity per-set order is the typed admission for this path.
        if format_count == 0
            || internal_column_sources.is_empty()
            || output_column_sources.is_empty()
            || internal_column_sources == output_column_sources
        {
            return None;
        }

        let internal_to_source =
            complete_moxel_source_format_order(format_count, internal_column_sources, false)?;
        let output_source_order =
            complete_moxel_source_format_order(format_count, output_column_sources, true)?;
        let mut source_to_internal = vec![0; format_count];
        for (internal_offset, source_format_index) in internal_to_source.iter().copied().enumerate()
        {
            let slot = source_to_internal.get_mut(source_format_index.checked_sub(1)?)?;
            if *slot != 0 {
                return None;
            }
            *slot = internal_offset + 1;
        }
        if source_to_internal
            .iter()
            .any(|format_index| *format_index == 0)
        {
            return None;
        }

        Some(Self {
            source_to_internal,
            internal_to_source,
            output_source_order,
        })
    }

    fn len(&self) -> usize {
        self.internal_to_source.len()
    }

    fn internal_for_source(&self, source_format_index: usize) -> Option<usize> {
        source_format_index
            .checked_sub(1)
            .and_then(|index| self.source_to_internal.get(index))
            .copied()
            .filter(|format_index| *format_index > 0)
    }

    fn output_internal_indices(&self, format_count: usize) -> Option<Vec<usize>> {
        if format_count != self.len() || self.output_source_order.len() != format_count {
            return None;
        }
        let mut seen = BTreeSet::new();
        let mut output = Vec::with_capacity(format_count);
        for source_format_index in &self.output_source_order {
            let internal_format_index = self.internal_for_source(*source_format_index)?;
            if self
                .internal_to_source
                .get(internal_format_index - 1)
                .copied()
                != Some(*source_format_index)
                || !seen.insert(internal_format_index)
            {
                return None;
            }
            output.push(internal_format_index);
        }
        (output.len() == format_count).then_some(output)
    }
}

struct MoxelSourceFontMap {
    source_to_output: Vec<usize>,
    output_to_source: Vec<usize>,
}

impl MoxelSourceFontMap {
    fn try_new(spreadsheet: &MoxelSpreadsheet, source_body_offset: usize) -> Option<Self> {
        let font_count = spreadsheet.fonts.len();
        if font_count < 2
            || !spreadsheet.fonts.iter().all(|font| {
                font.kind == "Absolute"
                    && font.ref_name.is_none()
                    && font.face_name.is_some()
                    && font.height.is_some()
                    && font.scale.is_some()
            })
        {
            return None;
        }

        let mut seen = vec![false; font_count];
        let mut output_to_source = Vec::with_capacity(font_count);
        let format_indices = if source_body_offset > 0 {
            moxel_sparse_source_font_format_indices(
                spreadsheet.column_formats.len(),
                moxel_output_format_count(spreadsheet),
                source_body_offset,
            )?
        } else {
            moxel_output_format_indices(spreadsheet)
        };
        for source_font_index in format_indices
            .into_iter()
            .filter_map(|format_index| moxel_format_for_index(spreadsheet, format_index).font)
        {
            let source_slot = seen.get_mut(source_font_index)?;
            if !*source_slot {
                *source_slot = true;
                output_to_source.push(source_font_index);
            }
        }
        if output_to_source.len() != font_count
            || output_to_source.iter().copied().eq(0..font_count)
        {
            return None;
        }

        let mut source_to_output = vec![usize::MAX; font_count];
        for (output_font_index, source_font_index) in output_to_source.iter().copied().enumerate() {
            let output_slot = source_to_output.get_mut(source_font_index)?;
            if *output_slot != usize::MAX {
                return None;
            }
            *output_slot = output_font_index;
        }
        if source_to_output
            .iter()
            .enumerate()
            .any(|(source_font_index, output_font_index)| {
                output_to_source.get(*output_font_index).copied() != Some(source_font_index)
            })
        {
            return None;
        }

        Some(Self {
            source_to_output,
            output_to_source,
        })
    }

    fn output_for_source(&self, source_font_index: usize) -> Option<usize> {
        let output_font_index = self.source_to_output.get(source_font_index).copied()?;
        (self.output_to_source.get(output_font_index).copied() == Some(source_font_index))
            .then_some(output_font_index)
    }

    fn output_fonts(&self, fonts: &[MoxelFont]) -> Option<Vec<MoxelFont>> {
        if fonts.len() != self.source_to_output.len() || fonts.len() != self.output_to_source.len()
        {
            return None;
        }
        self.output_to_source
            .iter()
            .map(|source_font_index| fonts.get(*source_font_index).cloned())
            .collect()
    }

    fn output_format_font(&self, format: &MoxelFormat) -> Option<Option<usize>> {
        match format.font {
            Some(source_font_index) => Some(Some(self.output_for_source(source_font_index)?)),
            None => Some(None),
        }
    }
}

fn complete_moxel_source_format_order(
    format_count: usize,
    leading_sources: &[usize],
    default_source_last: bool,
) -> Option<Vec<usize>> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::with_capacity(format_count);
    for source_format_index in leading_sources {
        if *source_format_index == 0 || *source_format_index > format_count {
            return None;
        }
        if seen.insert(*source_format_index) {
            ordered.push(*source_format_index);
        }
    }
    // Source slot 1 is the implicit default and trails unselected table slots.
    let remaining_start = if default_source_last { 2 } else { 1 };
    for source_format_index in remaining_start..=format_count {
        if seen.insert(source_format_index) {
            ordered.push(source_format_index);
        }
    }
    if default_source_last && seen.insert(1) {
        ordered.push(1);
    }
    (ordered.len() == format_count).then_some(ordered)
}

#[derive(Clone)]
pub(super) struct MoxelRow {
    pub(super) index: usize,
    pub(super) index_to: Option<usize>,
    pub(super) format_index: usize,
    pub(super) source_format_index: Option<usize>,
    pub(super) columns_id: Option<String>,
    pub(super) cells: Vec<MoxelCell>,
}

pub(super) struct MoxelColumnSet {
    pub(super) id: Option<String>,
    pub(super) default_format_index: Option<usize>,
    pub(super) source_default_format_index: Option<usize>,
    pub(super) size: usize,
    pub(super) columns: Vec<MoxelColumn>,
}

pub(super) struct MoxelColumn {
    pub(super) index: i32,
    pub(super) format_index: usize,
    pub(super) source_format_index: Option<usize>,
}

#[derive(Clone)]
pub(super) struct MoxelCell {
    pub(super) column_index: usize,
    pub(super) format_index: usize,
    pub(super) source_format_index: Option<usize>,
    pub(super) text: Option<String>,
    pub(super) parameter: Option<String>,
    pub(super) detail_parameter: Option<String>,
    pub(super) note: Option<MoxelNote>,
    pub(super) empty_text: bool,
}

#[derive(Clone)]
pub(super) struct MoxelNote {
    pub(super) format_index: usize,
    pub(super) source_format_index: usize,
    pub(super) text: MoxelLocalizedValue,
    pub(super) begin_row: i32,
    pub(super) begin_row_offset: i32,
    pub(super) end_row: i32,
    pub(super) end_row_offset: i32,
    pub(super) begin_column: i32,
    pub(super) begin_column_offset: i32,
    pub(super) end_column: i32,
    pub(super) end_column_offset: i32,
    pub(super) auto_size: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct MoxelLocalizedValue {
    pub(super) lang: String,
    pub(super) content: String,
}

#[derive(Clone)]
pub(super) enum MoxelNamedItem {
    Cells(MoxelArea),
    Drawing { name: String, drawing_id: usize },
}

#[derive(Clone)]
pub(super) struct MoxelArea {
    pub(super) name: String,
    pub(super) area_type: &'static str,
    pub(super) begin_row: i32,
    pub(super) end_row: i32,
    pub(super) begin_column: i32,
    pub(super) end_column: i32,
    pub(super) columns_id: Option<String>,
}

pub(super) struct MoxelVerticalGroup {
    pub(super) begin_row: usize,
    pub(super) end_row: usize,
    pub(super) level: usize,
}

#[derive(Clone)]
pub(super) struct MoxelMerge {
    pub(super) row: i32,
    pub(super) column: i32,
    pub(super) height: i32,
    pub(super) width: i32,
    pub(super) columns_id: Option<String>,
}

#[derive(Clone)]
pub(super) struct MoxelFont {
    pub(super) ref_name: Option<String>,
    pub(super) face_name: Option<String>,
    pub(super) height: Option<String>,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
    pub(super) strikeout: bool,
    pub(super) kind: &'static str,
    pub(super) scale: Option<usize>,
}

pub(super) struct MoxelLine {
    pub(super) style: &'static str,
    pub(super) line_type: &'static str,
    pub(super) width: usize,
}

pub(super) struct MoxelDrawing {
    pub(super) id: usize,
    pub(super) format_index: usize,
    pub(super) begin_row: i32,
    pub(super) begin_row_offset: i32,
    pub(super) end_row: i32,
    pub(super) end_row_offset: i32,
    pub(super) begin_column: i32,
    pub(super) begin_column_offset: i32,
    pub(super) end_column: i32,
    pub(super) end_column_offset: i32,
    pub(super) auto_size: bool,
    pub(super) z_order: usize,
    pub(super) kind: MoxelDrawingKind,
}

pub(super) enum MoxelDrawingKind {
    Picture {
        picture_size: &'static str,
        picture_index: usize,
    },
    Chart(MoxelChart),
}

pub(super) struct MoxelChart {
    series_cur_id: usize,
    points_cur_id: usize,
    is_series_design: bool,
    real_series: Vec<MoxelChartSeries>,
    real_extra_series: MoxelChartSeries,
    is_points_design: bool,
    real_points: Vec<MoxelChartPoint>,
    cur_series: usize,
    cur_point: usize,
    labels_location: &'static str,
    title: Vec<MoxelLocalizedValue>,
    values_scale_format: Vec<MoxelLocalizedValue>,
    is_auto_series_name: bool,
    max_series: usize,
    is_outline: bool,
    rebuild_time: usize,
    gauge_bands: Vec<MoxelChartGaugeBand>,
    user_max_value: String,
    user_min_value: String,
    real_data_items: Vec<MoxelChartDataItem>,
    spline_strain: usize,
    translucence_percent: String,
    funnel_neck_height_percent: String,
    funnel_neck_width_percent: String,
    funnel_gap_sum_percent: String,
    elements_chart: MoxelChartRectangle,
    elements_legend: MoxelChartRectangle,
    elements_title: MoxelChartRectangle,
    values_axis: MoxelChartAxis,
    points_axis: MoxelChartAxis,
}

pub(super) struct MoxelChartSeries {
    id: usize,
    color: String,
    line: MoxelChartLine,
    marker: &'static str,
    text: Vec<MoxelLocalizedValue>,
    str_is_changed: bool,
    is_expand: bool,
    is_indicator: bool,
    color_priority: bool,
}

pub(super) struct MoxelChartPoint {
    id: usize,
    color: String,
    line: MoxelChartLine,
    marker: &'static str,
    text: Vec<MoxelLocalizedValue>,
    str_is_changed: bool,
    is_expand: bool,
    is_indicator: bool,
    color_priority: bool,
}

pub(super) struct MoxelChartLine {
    width: usize,
}

pub(super) struct MoxelChartGaugeBand {
    begin: String,
    end: String,
    back_color: String,
    text: Vec<MoxelLocalizedValue>,
    tooltip: Vec<MoxelLocalizedValue>,
}

pub(super) struct MoxelChartDataItem {
    value: String,
    tooltip: String,
}

pub(super) struct MoxelChartRectangle {
    left: String,
    right: String,
    top: String,
    bottom: String,
}

#[derive(Default)]
pub(super) struct MoxelChartAxis {
    min_value: Option<String>,
    max_value: Option<String>,
}

pub(super) struct MoxelPicture {
    pub(super) index: usize,
    pub(super) ref_name: Option<String>,
    pub(super) payload: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct MoxelPrintSettings {
    pub(super) page_orientation: Option<&'static str>,
    pub(super) scale: Option<usize>,
    pub(super) collate: Option<bool>,
    pub(super) copies: Option<usize>,
    pub(super) per_page: Option<usize>,
    pub(super) top_margin: Option<usize>,
    pub(super) left_margin: Option<usize>,
    pub(super) bottom_margin: Option<usize>,
    pub(super) right_margin: Option<usize>,
    pub(super) header_size: Option<usize>,
    pub(super) footer_size: Option<usize>,
    pub(super) fit_to_page: Option<bool>,
    pub(super) black_and_white: Option<bool>,
    pub(super) printer_name: Option<String>,
    pub(super) paper: Option<usize>,
    pub(super) paper_source: Option<usize>,
    pub(super) page_width: Option<usize>,
    pub(super) page_height: Option<usize>,
    pub(super) duplex_type: Option<&'static str>,
    pub(super) page_placement_alternation: Option<&'static str>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct MoxelFormat {
    pub(super) font: Option<usize>,
    pub(super) border: Option<usize>,
    pub(super) left_border: Option<usize>,
    pub(super) top_border: Option<usize>,
    pub(super) right_border: Option<usize>,
    pub(super) bottom_border: Option<usize>,
    pub(super) height: Option<i32>,
    pub(super) border_color: Option<String>,
    pub(super) width: Option<usize>,
    pub(super) width_weight_factor: Option<usize>,
    pub(super) horizontal_alignment: Option<&'static str>,
    pub(super) vertical_alignment: Option<&'static str>,
    pub(super) back_color: Option<String>,
    pub(super) pattern_color: Option<String>,
    pub(super) pattern: Option<&'static str>,
    pub(super) text_color: Option<String>,
    pub(super) text_placement: Option<&'static str>,
    pub(super) text_orientation: Option<usize>,
    pub(super) fill_type: Option<&'static str>,
    pub(super) number_format_present: bool,
    pub(super) number_format: Vec<MoxelLocalizedValue>,
    pub(super) edit_format_present: bool,
    pub(super) edit_format: Vec<MoxelLocalizedValue>,
    pub(super) drawing_border: Option<usize>,
    pub(super) by_selected_columns: Option<bool>,
    pub(super) details_use: Option<&'static str>,
    pub(super) mark_negatives: Option<bool>,
    pub(super) hyper_link: Option<bool>,
    pub(super) auto_mark_incomplete: Option<bool>,
    pub(super) mark_incomplete: Option<bool>,
    pub(super) protection: Option<bool>,
    pub(super) hidden: Option<bool>,
    pub(super) indent: Option<usize>,
    pub(super) auto_indent: Option<usize>,
    pub(super) column_size_change: Option<&'static str>,
    pub(super) mask: Option<&'static str>,
    pub(super) pic_index: Option<usize>,
    pub(super) picture_size_mode: Option<&'static str>,
    pub(super) pic_horizontal_alignment: Option<&'static str>,
    pub(super) pic_vertical_alignment: Option<&'static str>,
    pub(super) text_position: Option<&'static str>,
    pub(super) left_margin: Option<usize>,
    pub(super) top_margin: Option<usize>,
    pub(super) right_margin: Option<usize>,
    pub(super) bottom_margin: Option<usize>,
}

impl MoxelFormat {
    pub(super) fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.border.is_none()
            && self.left_border.is_none()
            && self.top_border.is_none()
            && self.right_border.is_none()
            && self.bottom_border.is_none()
            && self.height.is_none()
            && self.border_color.is_none()
            && self.width.is_none()
            && self.width_weight_factor.is_none()
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.back_color.is_none()
            && self.pattern_color.is_none()
            && self.pattern.is_none()
            && self.text_color.is_none()
            && self.text_placement.is_none()
            && self.text_orientation.is_none()
            && self.fill_type.is_none()
            && !self.number_format_present
            && self.number_format.is_empty()
            && !self.edit_format_present
            && self.edit_format.is_empty()
            && self.drawing_border.is_none()
            && self.by_selected_columns.is_none()
            && self.details_use.is_none()
            && self.mark_negatives.is_none()
            && self.hyper_link.is_none()
            && self.auto_mark_incomplete.is_none()
            && self.mark_incomplete.is_none()
            && self.protection.is_none()
            && self.hidden.is_none()
            && self.indent.is_none()
            && self.auto_indent.is_none()
            && self.column_size_change.is_none()
            && self.mask.is_none()
            && self.pic_index.is_none()
            && self.picture_size_mode.is_none()
            && self.pic_horizontal_alignment.is_none()
            && self.pic_vertical_alignment.is_none()
            && self.text_position.is_none()
            && self.left_margin.is_none()
            && self.top_margin.is_none()
            && self.right_margin.is_none()
            && self.bottom_margin.is_none()
    }
}

pub(super) fn normalize_moxel_default_match_format(mut format: MoxelFormat) -> MoxelFormat {
    if format.font == Some(0) {
        format.font = None;
    }
    format
}

pub(super) fn resolve_existing_moxel_default_format_index(
    column_formats: &[MoxelFormat],
    formats: &[MoxelFormat],
    default_format: &MoxelFormat,
    default_format_width: Option<usize>,
) -> Option<(usize, bool)> {
    let all_formats = column_formats
        .iter()
        .chain(formats.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut target = default_format.clone();
    if target.width.is_none() {
        target.width = default_format_width;
    }
    if target.is_empty() {
        return None;
    }
    let preferred_target_exact = if default_format.is_empty() && default_format_width.is_some() {
        Some(MoxelFormat {
            font: Some(0),
            width: default_format_width,
            ..MoxelFormat::default()
        })
    } else {
        None
    };
    let target_exact = target.clone();
    let target_normalized = normalize_moxel_default_match_format(target);
    let last_exact_match = |target: &MoxelFormat| {
        all_formats
            .iter()
            .enumerate()
            .filter_map(|(index, format)| (format == target).then_some(index + 1))
            .last()
    };
    preferred_target_exact
        .as_ref()
        .and_then(|target| last_exact_match(target).map(|index| (index, true)))
        .or_else(|| last_exact_match(&target_exact).map(|index| (index, false)))
        .or_else(|| {
            all_formats
                .iter()
                .enumerate()
                .filter_map(|(index, format)| {
                    (normalize_moxel_default_match_format(format.clone()) == target_normalized)
                        .then_some((index + 1, false))
                })
                .last()
        })
}

pub(crate) fn extract_moxel_spreadsheet_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let body = crate::compiler::bodies::mxl::decode_compatible_mxl(bytes).ok()?;
    let spreadsheet = parse_moxel_spreadsheet_text(body.native_body_text(), object_refs)?;
    Some(format_moxel_spreadsheet_xml(&spreadsheet))
}

pub(super) fn parse_moxel_spreadsheet_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelSpreadsheet> {
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "8" {
        return None;
    }
    let raw_declared_column_count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let mut rows = parse_moxel_rows(&fields);
    if rows.is_empty() {
        return None;
    }
    let vertical_groups = parse_moxel_vertical_groups(&fields);
    let (merges, horizontal_unmerges, vertical_unmerges) = parse_moxel_merge_regions(&fields);
    let named_items = parse_moxel_named_items(&fields);
    let areas = named_items
        .iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area.clone()),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect::<Vec<_>>();
    let print_area = parse_moxel_print_area(&fields);
    trim_moxel_trailing_empty_rows(
        &mut rows,
        &areas,
        &merges,
        &horizontal_unmerges,
        &vertical_unmerges,
    );
    compact_moxel_empty_row_ranges(&mut rows);
    let (
        column_sets,
        row_column_ids,
        declared_sheet_height,
        source_column_format_order,
        has_explicit_sparse_column_set_default,
    ) = parse_moxel_column_sets_with_source_format_order(&fields);
    let fonts = parse_moxel_fonts(&fields);
    let pictures = parse_moxel_pictures(&fields, object_refs);
    let style_refs = parse_moxel_style_refs(&fields, object_refs);
    let mut default_format = parse_moxel_default_format(&fields, object_refs);
    let print_settings = parse_moxel_print_settings(&fields);
    let empty_headers_footers = parse_moxel_empty_headers_footers(&fields);
    let header_footer_format_ref = parse_moxel_uniform_header_footer_format_ref(&fields);
    let drawings = parse_moxel_drawings(&fields);
    let drawing_format_indices = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .collect::<BTreeSet<_>>();
    let zero_column_format_table_is_width_only =
        parse_moxel_format_table(&fields, 0, &style_refs, &drawing_format_indices, &[])
            .is_some_and(|formats| {
                formats.len() == 1 && formats.first().is_some_and(is_moxel_width_only_format)
            });
    let observed_column_count = rows
        .iter()
        .flat_map(|row| row.cells.iter().map(|cell| cell.column_index + 1))
        .max()
        .unwrap_or(0);
    // MOXCEL normally stores the last zero-based column index, hence the
    // usual `+ 1` above.  A structurally empty sheet is the one exception:
    // its raw value is zero even though it has no columns at all.  Treating
    // it as one implicit column manufactures an empty palette slot, shifts
    // every format reference, and materialises a height on export.
    let zero_column_width_only = raw_declared_column_count == 0
        && observed_column_count == 0
        && column_sets.is_empty()
        && rows.iter().all(is_moxel_compactable_empty_row)
        && parse_moxel_default_format_width(&fields, 0).is_some()
        && zero_column_format_table_is_width_only
        && default_format.is_empty()
        && fonts.is_empty()
        && pictures.is_empty()
        && style_refs.iter().all(Option::is_none)
        && declared_sheet_height.unwrap_or(0) == 0
        && vertical_groups.is_empty()
        && merges.is_empty()
        && horizontal_unmerges.is_empty()
        && vertical_unmerges.is_empty()
        && named_items.is_empty()
        && areas.is_empty()
        && print_area.is_none()
        && print_settings.is_none()
        && !empty_headers_footers
        && header_footer_format_ref.is_none()
        && drawings.is_empty();
    let declared_column_count = if zero_column_width_only {
        0
    } else {
        raw_declared_column_count + 1
    };
    let column_count = if observed_column_count > 0 {
        observed_column_count
    } else {
        declared_column_count
    };
    let mut column_sets = if column_sets.is_empty() {
        default_moxel_column_sets(column_count)
    } else {
        column_sets
    };
    let column_format_slots = moxel_column_format_slots(&column_sets, column_count);
    let source_column_format_refs = moxel_source_column_format_refs(&column_sets);
    let source_column_format_offset = moxel_source_column_format_offset(&column_sets);
    let has_high_source_column_format_refs = column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| column.source_format_index)
        .any(|source_format_index| source_format_index > column_format_slots);
    let needs_sparse_column_set_default_format =
        source_column_format_offset > 0 && header_footer_format_ref.is_some();
    if source_column_format_offset == 0 && column_format_slots == 0 {
        normalize_moxel_zero_column_format_refs(&mut rows);
    }
    let mut default_format_width = parse_moxel_default_format_width(&fields, column_format_slots);
    let has_equal_width_only_format_table =
        parse_moxel_equal_width_only_format_table(&fields, column_count).is_some();
    let sparse_source_format_refs = moxel_uses_sparse_source_format_refs(
        &column_sets,
        column_count,
        &rows,
        &default_format,
        default_format_width,
    );
    let sparse_body_source_offset = if sparse_source_format_refs {
        moxel_sparse_body_source_format_offset(&rows, &source_column_format_refs)
    } else {
        0
    };
    if sparse_source_format_refs
        && has_high_source_column_format_refs
        && source_column_format_refs.len() > 1
        && default_format_width.is_some()
        && default_format.border_color.is_none()
        && default_format.is_empty()
        && style_refs.first().and_then(|slot| slot.as_deref()) == Some("style:BorderColor")
    {
        default_format.font = Some(0);
        default_format.border_color = Some("style:BorderColor".to_string());
    }

    let format_offset = if sparse_source_format_refs || has_equal_width_only_format_table {
        0
    } else {
        column_format_slots.saturating_sub(1)
    };
    for row in &mut rows {
        if let Some(columns_id) = row_column_ids.get(&row.index) {
            row.columns_id = Some(columns_id.clone());
        }
        if source_column_format_offset == 0 {
            if row.format_index > 1 {
                row.format_index += format_offset;
            }
            for cell in &mut row.cells {
                if cell.format_index > 0 {
                    cell.format_index += format_offset;
                }
                if let Some(note) = &mut cell.note
                    && note.format_index > 0
                {
                    note.format_index += format_offset;
                }
            }
        }
    }
    let height = if zero_column_width_only {
        0
    } else {
        moxel_spreadsheet_height(
            &rows,
            &merges,
            &horizontal_unmerges,
            &vertical_unmerges,
            &areas,
        )
        .max(declared_sheet_height.unwrap_or(0))
    };
    let number_format_refs = parse_moxel_number_format_refs(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
    );
    if default_format.is_empty() && default_format_width.is_none() {
        if let Some(leading_default_format) =
            parse_moxel_leading_default_format(&fields, &style_refs, &number_format_refs)
        {
            default_format_width = leading_default_format.width;
            default_format = leading_default_format;
        }
    }
    let (column_formats, formats, source_format_map, leading_source_column_formats) =
        parse_moxel_formats_with_source_map(
            &fields,
            column_format_slots,
            sparse_source_format_refs,
            &source_column_format_refs,
            &source_column_format_order,
            &style_refs,
            &drawing_format_indices,
            &number_format_refs,
        );
    let (column_formats, mut formats) = (column_formats, formats);
    let source_format_map = source_format_map.filter(|source_format_map| {
        moxel_source_format_refs_are_complete(
            source_format_map,
            &column_sets,
            &rows,
            &drawings,
            header_footer_format_ref,
        )
    });
    if source_column_format_offset == 0 && column_formats.is_empty() && formats.is_empty() {
        restore_moxel_source_format_refs_without_format_table(&mut rows);
    }
    if source_column_format_offset > 0 {
        if sparse_source_format_refs {
            if let Some(source_format_map) = &source_format_map {
                remap_moxel_column_set_source_format_indices(&mut column_sets, source_format_map);
                remap_moxel_row_and_cell_source_format_indices(&mut rows, source_format_map);
            } else {
                remap_moxel_column_set_sparse_internal_format_indices(
                    &mut column_sets,
                    &source_column_format_refs,
                    column_formats.len(),
                    formats.len(),
                );
                remap_moxel_row_and_cell_sparse_internal_format_indices(
                    &mut rows,
                    &source_column_format_refs,
                    column_formats.len(),
                    formats.len(),
                    sparse_body_source_offset,
                );
            }
        } else if column_formats.len() > source_column_format_refs.len()
            || needs_sparse_column_set_default_format
        {
            let source_output_indices = moxel_source_derived_internal_output_order(
                &column_sets,
                column_formats.len(),
                formats.len(),
            );
            remap_moxel_column_set_internal_format_indices(
                &mut column_sets,
                column_formats.len(),
                formats.len(),
            );
            remap_moxel_row_and_cell_sparse_source_format_indices(
                &mut rows,
                &source_column_format_refs,
                &source_output_indices,
            );
        } else {
            remap_moxel_column_set_output_format_indices(
                &mut column_sets,
                &source_column_format_refs,
            );
            remap_moxel_row_and_cell_output_format_indices(&mut rows, &source_column_format_refs);
        }
    } else if leading_source_column_formats {
        remap_moxel_leading_source_column_format_indices(&mut rows);
    } else if sparse_source_format_refs && !source_column_format_refs.is_empty() {
        remap_moxel_column_set_output_format_indices(&mut column_sets, &source_column_format_refs);
        remap_moxel_row_and_cell_output_format_indices(&mut rows, &source_column_format_refs);
    }
    let extra_formats = BTreeMap::new();
    let header_footer_format_index = if needs_sparse_column_set_default_format {
        resolve_sparse_moxel_column_set_default_format_index(
            &mut column_sets,
            column_formats.len(),
            &formats,
            header_footer_format_ref,
            has_explicit_sparse_column_set_default,
        )
    } else {
        None
    };
    let all_formats = column_formats
        .iter()
        .chain(formats.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut fonts = fonts;
    normalize_moxel_fonts(&mut fonts, &all_formats);
    let has_sparse_column_sets = column_sets
        .iter()
        .any(|column_set| column_set.columns.len() != column_set.size);
    let mut lines = parse_moxel_lines(&fields, &all_formats, has_sparse_column_sets);
    normalize_moxel_single_set_report_header_tail(
        &column_sets,
        &column_formats,
        &style_refs,
        &mut lines,
        &mut formats,
    );
    let drawing_max_format_index = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    let row_cell_max_format_index = rows.iter().fold(
        moxel_column_format_slots(&column_sets, column_count).max(1),
        |max_index, row| {
            let row_max = row.cells.iter().fold(row.format_index, |cell_max, cell| {
                cell_max.max(cell.format_index).max(
                    cell.note
                        .as_ref()
                        .map(|note| note.format_index)
                        .unwrap_or(0),
                )
            });
            max_index.max(row_max)
        },
    );
    let max_format_index = row_cell_max_format_index.max(drawing_max_format_index);
    let format_table_fallback = column_formats.len() + formats.len() + 1;
    let mut default_format_index = moxel_default_format_index(
        &column_sets,
        print_settings.as_ref(),
        !default_format.is_empty() || default_format_width.is_some(),
        format_table_fallback.max(max_format_index + 1),
    );
    if default_format_index.is_some_and(|index| index > column_formats.len() + formats.len())
        && let Some((existing_index, exact_font_zero_match)) =
            resolve_existing_moxel_default_format_index(
                &column_formats,
                &formats,
                &default_format,
                default_format_width,
            )
    {
        default_format_index = Some(existing_index);
        if exact_font_zero_match && default_format.is_empty() {
            default_format.font = Some(0);
        }
    }
    if source_column_format_offset > 0
        && default_format.is_empty()
        && default_format_width.is_none()
        && column_formats.len() == source_column_format_refs.len()
        && source_column_format_refs
            .iter()
            .copied()
            .max()
            .is_some_and(|max_source_format_index| {
                max_source_format_index < column_formats.len() + formats.len()
            })
        && let Some(min_source_format_index) = source_column_format_refs.iter().copied().min()
        && min_source_format_index > 1
    {
        default_format_index = Some(column_formats.len() + min_source_format_index);
    }
    if header_footer_format_index.is_some()
        && default_format.is_empty()
        && default_format_width.is_none()
    {
        default_format_index = None;
    }
    if column_sets.len() == 1
        && let Some(shared_format_index) = header_footer_format_index
        && shared_format_index > column_formats.len()
        && let Some(shared_format) =
            moxel_internal_format(&column_formats, &formats, shared_format_index)
    {
        if shared_format.is_empty() {
            if default_format_index.is_none_or(|index| index <= column_formats.len()) {
                default_format_index = Some(shared_format_index);
            }
        } else {
            default_format_index = None;
        }
    }
    if column_sets.len() == 1
        && let Some(shared_format_index) = header_footer_format_index
        && shared_format_index > column_formats.len()
        && let Some(shared_format) =
            moxel_internal_format(&column_formats, &formats, shared_format_index)
        && shared_format.is_empty()
        && default_format_index.is_some_and(|index| index > shared_format_index)
        && let Some(default_set) = column_sets.first_mut()
    {
        default_set.default_format_index = Some(shared_format_index);
    }
    let mut spreadsheet = MoxelSpreadsheet {
        column_count,
        column_sets,
        column_formats,
        extra_formats,
        default_format_width,
        default_format,
        formats,
        rows,
        vertical_groups,
        merges,
        horizontal_unmerges,
        vertical_unmerges,
        named_items,
        areas,
        print_area,
        print_settings,
        lines,
        fonts,
        drawings,
        pictures,
        empty_headers_footers,
        header_footer_format_index,
        default_format_index,
        source_format_map,
        height,
    };
    if sparse_source_format_refs
        && let Some(source_font_map) =
            MoxelSourceFontMap::try_new(&spreadsheet, sparse_body_source_offset)
    {
        remap_moxel_source_fonts(&source_font_map, &mut spreadsheet);
    }
    Some(spreadsheet)
}

pub(super) fn normalize_moxel_fonts(fonts: &mut Vec<MoxelFont>, formats: &[MoxelFormat]) {
    let Some(max_used_index) = formats.iter().filter_map(|format| format.font).max() else {
        return;
    };
    if max_used_index != fonts.len() || fonts.is_empty() {
        return;
    }
    if fonts.iter().any(|font| font.kind == "StyleItem") {
        return;
    }
    if !fonts
        .last()
        .is_some_and(|font| font.kind == "Absolute" && font.ref_name.is_none())
    {
        return;
    }

    // Some MXL variants reference one implicit TextFont slot that is not present
    // in the raw font table. Native XML places it before the last explicit font.
    fonts.insert(
        fonts.len() - 1,
        MoxelFont {
            ref_name: Some("style:TextFont".to_string()),
            face_name: None,
            height: None,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            kind: "StyleItem",
            scale: None,
        },
    );
}

pub(super) fn default_moxel_column_sets(column_count: usize) -> Vec<MoxelColumnSet> {
    vec![MoxelColumnSet {
        id: None,
        default_format_index: None,
        source_default_format_index: None,
        size: column_count,
        columns: (0..column_count)
            .map(|index| MoxelColumn {
                index: index as i32,
                format_index: index + 1,
                source_format_index: None,
            })
            .collect(),
    }]
}

#[cfg(test)]
pub(super) fn parse_moxel_column_sets(
    fields: &[&str],
) -> (Vec<MoxelColumnSet>, BTreeMap<usize, String>, Option<usize>) {
    let (column_sets, row_column_ids, declared_sheet_height, _, _) =
        parse_moxel_column_sets_with_source_format_order(fields);
    (column_sets, row_column_ids, declared_sheet_height)
}

fn parse_moxel_column_sets_with_source_format_order(
    fields: &[&str],
) -> (
    Vec<MoxelColumnSet>,
    BTreeMap<usize, String>,
    Option<usize>,
    Vec<usize>,
    bool,
) {
    for index in 0..fields.len() {
        let Some(default_set) = parse_moxel_column_set(fields[index]) else {
            continue;
        };
        let Some(default_source_format_index) =
            parse_moxel_column_set_raw_default_format_index(fields[index])
        else {
            continue;
        };
        if default_set.id.is_some() || index + 2 >= fields.len() {
            continue;
        }
        let Some(declared_sheet_height) = fields
            .get(index + 1)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        let Some(additional_count) = fields
            .get(index + 2)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if additional_count > 64 || index + 3 + additional_count >= fields.len() {
            continue;
        }

        let mut column_sets = vec![default_set];
        let mut raw_default_format_indices = vec![default_source_format_index];
        let mut cursor = index + 3;
        for _ in 0..additional_count {
            let Some(column_set) = parse_moxel_column_set(fields[cursor]) else {
                column_sets.clear();
                break;
            };
            let Some(raw_default_format_index) =
                parse_moxel_column_set_raw_default_format_index(fields[cursor])
            else {
                column_sets.clear();
                break;
            };
            if column_set.id.is_none() {
                column_sets.clear();
                break;
            }
            column_sets.push(column_set);
            raw_default_format_indices.push(raw_default_format_index);
            cursor += 1;
        }
        if column_sets.is_empty() || column_sets.len() != raw_default_format_indices.len() {
            continue;
        }
        normalize_moxel_column_set_format_indices(&mut column_sets);
        let row_column_ids =
            parse_moxel_row_column_set_ids(fields, cursor, &column_sets[1..]).unwrap_or_default();
        let source_format_order =
            moxel_source_column_format_refs_in_set_order(&column_sets, &raw_default_format_indices);
        let has_explicit_sparse_column_set_default = raw_default_format_indices
            .iter()
            .skip(1)
            .any(|format_index| *format_index == 1);
        return (
            column_sets,
            row_column_ids,
            Some(declared_sheet_height),
            source_format_order,
            has_explicit_sparse_column_set_default,
        );
    }
    (Vec::new(), BTreeMap::new(), None, Vec::new(), false)
}

fn parse_moxel_column_set_raw_default_format_index(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    fields.get(1)?.trim().parse::<usize>().ok()
}

fn moxel_source_column_format_refs_in_set_order(
    column_sets: &[MoxelColumnSet],
    raw_default_format_indices: &[usize],
) -> Vec<usize> {
    if column_sets.len() != raw_default_format_indices.len() {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for (column_set, raw_default_format_index) in column_sets.iter().zip(raw_default_format_indices)
    {
        if *raw_default_format_index > 0 && seen.insert(*raw_default_format_index) {
            ordered.push(*raw_default_format_index);
        }
        for source_format_index in column_set
            .columns
            .iter()
            .filter_map(|column| column.source_format_index)
        {
            if source_format_index > 0 && seen.insert(source_format_index) {
                ordered.push(source_format_index);
            }
        }
    }
    ordered
}

pub(super) fn parse_moxel_vertical_groups(fields: &[&str]) -> Vec<MoxelVerticalGroup> {
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count == 0 || count > 2048 {
            continue;
        }
        let Some(last_group_field) = index.checked_add(count * 2) else {
            continue;
        };
        if last_group_field + 3 >= fields.len() {
            continue;
        }
        let mut groups = Vec::with_capacity(count);
        let mut cursor = index + 1;
        let mut valid = true;
        for _ in 0..count {
            let Some(group) = fields
                .get(cursor)
                .and_then(|field| parse_moxel_vertical_group(field))
            else {
                valid = false;
                break;
            };
            if fields.get(cursor + 1).map(|field| field.trim()) != Some("-1") {
                valid = false;
                break;
            }
            groups.push(group);
            cursor += 2;
        }
        if valid
            && !groups.is_empty()
            && fields.get(cursor).map(|field| field.trim()) == Some("0")
            && fields.get(cursor + 1).map(|field| field.trim()) == Some("0")
            && fields.get(cursor + 2).map(|field| field.trim()) == Some("0")
        {
            return groups;
        }
    }
    Vec::new()
}

pub(super) fn parse_moxel_vertical_group(text: &str) -> Option<MoxelVerticalGroup> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 6 || fields.get(3).map(|field| field.trim()) != Some("{1,0}") {
        return None;
    }
    Some(MoxelVerticalGroup {
        begin_row: fields.first()?.trim().parse::<usize>().ok()?,
        end_row: fields.get(1)?.trim().parse::<usize>().ok()?,
        level: fields.get(2)?.trim().parse::<usize>().ok()?,
    })
}

pub(super) fn parse_moxel_column_set(text: &str) -> Option<MoxelColumnSet> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 {
        return None;
    }
    let declared_count = fields.first()?.trim().parse::<usize>().ok()?;
    let raw_default_format_index = fields.get(1)?.trim().parse::<usize>().ok()?;
    let count = fields.get(3)?.trim().parse::<usize>().ok()?;
    if count > 2048 || fields.len() != count * 2 + 4 {
        return None;
    }
    let uuid = parse_uuid_field(fields.get(2)?.trim())?;
    let id = if uuid == "00000000-0000-0000-0000-000000000000" {
        None
    } else {
        Some(uuid)
    };
    let mut columns = Vec::with_capacity(count);
    for column_index in 0..count {
        let index = fields
            .get(column_index * 2 + 4)?
            .trim()
            .parse::<i32>()
            .ok()?;
        let format_index = fields
            .get(column_index * 2 + 5)?
            .trim()
            .parse::<usize>()
            .ok()?;
        columns.push(MoxelColumn {
            index,
            format_index,
            source_format_index: Some(format_index),
        });
    }
    Some(MoxelColumnSet {
        id,
        default_format_index: None,
        source_default_format_index: (raw_default_format_index > 1)
            .then_some(raw_default_format_index),
        size: declared_count,
        columns,
    })
}

pub(super) fn normalize_moxel_column_set_format_indices(column_sets: &mut [MoxelColumnSet]) {
    let mut normalized = BTreeMap::new();
    for column_set in column_sets.iter_mut() {
        for column in column_set.columns.iter_mut() {
            let source_format_index = column.source_format_index.unwrap_or(column.format_index);
            if source_format_index == 0 {
                column.format_index = 0;
                continue;
            }
            let next_index = normalized.len() + 1;
            column.format_index = *normalized.entry(source_format_index).or_insert(next_index);
        }
    }
}

pub(super) fn parse_moxel_uniform_header_footer_format_ref(fields: &[&str]) -> Option<usize> {
    fields.windows(6).find_map(|window| {
        let refs = window
            .iter()
            .map(|field| parse_moxel_header_footer_format_ref(field))
            .collect::<Option<Vec<_>>>()?;
        let first = refs.first().copied().flatten()?;
        refs.iter()
            .all(|candidate| *candidate == Some(first))
            .then_some(first)
    })
}

pub(super) fn parse_moxel_header_footer_format_ref(text: &str) -> Option<Option<usize>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 || fields.first().map(|field| field.trim()) != Some("0") {
        return None;
    }
    let format_index = fields.get(1)?.trim().parse::<usize>().ok()?;
    Some((format_index > 0).then_some(format_index))
}

pub(super) fn is_sparse_moxel_column_set_default_format(format: &MoxelFormat) -> bool {
    format.font == Some(0)
        && format.width == Some(72)
        && format.height.is_none()
        && format.border.is_none()
        && format.left_border.is_none()
        && format.top_border.is_none()
        && format.right_border.is_none()
        && format.bottom_border.is_none()
        && format.border_color.is_none()
        && format.width_weight_factor.is_none()
        && format.horizontal_alignment.is_none()
        && format.vertical_alignment.is_none()
        && format.back_color.is_none()
        && format.text_color.is_none()
        && format.text_placement.is_none()
        && format.text_orientation.is_none()
        && format.fill_type.is_none()
        && format.mark_negatives.is_none()
        && format.auto_mark_incomplete.is_none()
        && format.mark_incomplete.is_none()
        && format.column_size_change.is_none()
        && format.left_margin.is_none()
        && format.top_margin.is_none()
        && format.right_margin.is_none()
        && format.bottom_margin.is_none()
}

pub(super) fn resolve_sparse_moxel_column_set_default_format_index(
    column_sets: &mut [MoxelColumnSet],
    column_format_len: usize,
    formats: &[MoxelFormat],
    header_footer_format_ref: Option<usize>,
    has_explicit_sparse_column_set_default: bool,
) -> Option<usize> {
    if column_sets.is_empty() {
        return None;
    }
    let header_footer_format_index = header_footer_format_ref.and_then(|source_format_index| {
        moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            formats.len(),
        )
    });
    if column_sets.len() <= 1 {
        return header_footer_format_index;
    }
    let sparse_default_format_index = formats.iter().enumerate().find_map(|(index, format)| {
        is_sparse_moxel_column_set_default_format(format).then_some(column_format_len + index + 1)
    });
    if let Some(format_index) = sparse_default_format_index {
        if has_explicit_sparse_column_set_default {
            for column_set in column_sets.iter_mut().skip(1) {
                column_set.default_format_index = Some(format_index);
            }
        }
        return Some(format_index);
    }

    if let Some(format_index) = header_footer_format_index
        && format_index > column_format_len
    {
        if has_explicit_sparse_column_set_default {
            for column_set in column_sets.iter_mut().skip(1) {
                column_set.default_format_index = Some(format_index);
            }
        }
        return Some(format_index);
    }

    let format_index = header_footer_format_index?;
    if has_explicit_sparse_column_set_default {
        for column_set in column_sets.iter_mut().skip(1) {
            column_set.default_format_index = Some(format_index);
        }
    }
    Some(format_index)
}

pub(super) fn moxel_internal_format<'a>(
    column_formats: &'a [MoxelFormat],
    formats: &'a [MoxelFormat],
    format_index: usize,
) -> Option<&'a MoxelFormat> {
    if format_index == 0 {
        return None;
    }
    if format_index <= column_formats.len() {
        return column_formats.get(format_index - 1);
    }
    formats.get(format_index - column_formats.len() - 1)
}

pub(super) fn moxel_column_format_slots(
    column_sets: &[MoxelColumnSet],
    column_count: usize,
) -> usize {
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter().map(|column| column.format_index))
        .max()
        .unwrap_or_else(|| {
            if column_sets.is_empty() {
                column_count
            } else {
                0
            }
        })
}

pub(super) fn moxel_default_format_index(
    column_sets: &[MoxelColumnSet],
    _print_settings: Option<&MoxelPrintSettings>,
    has_default_format: bool,
    fallback: usize,
) -> Option<usize> {
    if has_default_format {
        return Some(fallback);
    }
    if column_sets.len() > 1 {
        return Some(fallback);
    }
    None
}

pub(super) fn parse_moxel_row_column_set_ids(
    fields: &[&str],
    index: usize,
    additional_sets: &[MoxelColumnSet],
) -> Option<BTreeMap<usize, String>> {
    if additional_sets.is_empty() {
        return Some(BTreeMap::new());
    }
    let count = fields.get(index)?.trim().parse::<usize>().ok()?;
    if count > 4096 || index + count >= fields.len() {
        return None;
    }
    if index + count * 2 < fields.len() {
        let mut row_column_ids = BTreeMap::new();
        let mut pair_mode = true;
        for pair_index in 0..count {
            let row_index = fields[index + 1 + pair_index * 2]
                .trim()
                .parse::<usize>()
                .ok();
            let set_index = fields[index + 2 + pair_index * 2]
                .trim()
                .parse::<usize>()
                .ok();
            let Some(row_index) = row_index else {
                pair_mode = false;
                break;
            };
            let Some(set_index) = set_index else {
                pair_mode = false;
                break;
            };
            let Some(columns_id) = additional_sets
                .get(set_index)
                .and_then(|set| set.id.as_ref())
            else {
                pair_mode = false;
                break;
            };
            row_column_ids.insert(row_index, columns_id.clone());
        }
        if pair_mode {
            return Some(row_column_ids);
        }
    }
    let first_columns_id = additional_sets.first()?.id.as_ref()?;
    let mut row_column_ids = BTreeMap::new();
    for field in &fields[index + 1..=index + count] {
        let row_index = field.trim().parse::<usize>().ok()?;
        row_column_ids.insert(row_index, first_columns_id.clone());
    }
    Some(row_column_ids)
}

pub(super) fn moxel_spreadsheet_height(
    rows: &[MoxelRow],
    merges: &[MoxelMerge],
    horizontal_unmerges: &[MoxelMerge],
    vertical_unmerges: &[MoxelMerge],
    areas: &[MoxelArea],
) -> usize {
    let row_max = rows
        .iter()
        .filter(|row| row.format_index > 1 || !row.cells.is_empty())
        .map(|row| row.index as i32)
        .max()
        .unwrap_or(0);
    let merge_max = merges
        .iter()
        .chain(horizontal_unmerges.iter())
        .chain(vertical_unmerges.iter())
        .map(|merge| merge.row + merge.height)
        .max()
        .unwrap_or(0);
    let area_max = areas.iter().map(|area| area.end_row).max().unwrap_or(0);
    row_max.max(merge_max).max(area_max).max(0) as usize + 1
}

pub(super) fn trim_moxel_trailing_empty_rows(
    rows: &mut Vec<MoxelRow>,
    areas: &[MoxelArea],
    merges: &[MoxelMerge],
    horizontal_unmerges: &[MoxelMerge],
    vertical_unmerges: &[MoxelMerge],
) {
    let Some(material_limit) = areas
        .iter()
        .map(|area| area.end_row.max(0) as usize + 1)
        .chain(
            merges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .chain(
            horizontal_unmerges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .chain(
            vertical_unmerges
                .iter()
                .map(|merge| (merge.row + merge.height).max(0) as usize + 1),
        )
        .max()
    else {
        return;
    };
    let mut last_trimmed_index = None;
    while rows.last().is_some_and(|row| {
        row.index > material_limit && row.format_index <= 1 && row.cells.is_empty()
    }) {
        if let Some(index) = rows.last().map(|row| row.index) {
            last_trimmed_index = Some(last_trimmed_index.unwrap_or(index).max(index));
        }
        rows.pop();
    }
    if let (Some(index_to), Some(row)) = (last_trimmed_index, rows.last_mut()) {
        if row.index == material_limit && row.format_index <= 1 && row.cells.is_empty() {
            row.index_to = Some(index_to);
        }
    }
}

pub(super) fn compact_moxel_empty_row_ranges(rows: &mut Vec<MoxelRow>) {
    let mut compacted = Vec::with_capacity(rows.len());
    let mut index = 0usize;
    while index < rows.len() {
        let mut row = rows[index].clone();
        if is_moxel_compactable_empty_row(&row) {
            let mut cursor = index + 1;
            while cursor < rows.len()
                && rows[cursor].index == rows[cursor - 1].index + 1
                && is_moxel_compactable_empty_row(&rows[cursor])
            {
                row.index_to = Some(rows[cursor].index);
                cursor += 1;
            }
            compacted.push(row);
            index = cursor;
        } else {
            compacted.push(row);
            index += 1;
        }
    }
    *rows = compacted;
}

pub(super) fn is_moxel_compactable_empty_row(row: &MoxelRow) -> bool {
    row.format_index <= 1 && row.columns_id.is_none() && row.cells.is_empty()
}

pub(super) fn parse_moxel_rows(fields: &[&str]) -> Vec<MoxelRow> {
    let mut best_rows = Vec::new();
    for index in 3..fields.len().saturating_sub(3) {
        if fields.get(index).map(|field| field.trim()) != Some("1")
            || fields.get(index + 1).map(|field| field.trim()) != Some("2")
        {
            continue;
        }
        let Some(height) = fields
            .get(index + 2)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if height == 0 || height > 1_000_000 {
            continue;
        }
        let mut rows = Vec::new();
        let mut cursor = index + 3;
        let mut expected_row_index = 0usize;
        while rows.len() < height {
            let Some((row, next_cursor)) = parse_moxel_row_at(fields, cursor, expected_row_index)
            else {
                break;
            };
            if next_cursor <= cursor {
                break;
            }
            rows.push(row);
            expected_row_index += 1;
            cursor = next_cursor;
        }
        if rows.len() > best_rows.len() {
            best_rows = rows;
        }
    }
    if best_rows.is_empty() {
        parse_moxel_rows_by_scanning(fields)
    } else {
        best_rows
    }
}

pub(super) fn parse_moxel_rows_by_scanning(fields: &[&str]) -> Vec<MoxelRow> {
    let mut best_rows = Vec::new();
    let mut index = 3usize;
    while index < fields.len() {
        let Some((row, next_index)) = parse_moxel_row_start_at_for_scanning(fields, index) else {
            index += 1;
            continue;
        };
        let mut rows = vec![row];
        let mut cursor = next_index;
        while cursor < fields.len() {
            let expected_row_index = rows.last().map(|row| row.index + 1).unwrap_or(0);
            let Some((row, next_cursor)) =
                parse_moxel_row_at_for_scanning(fields, cursor, expected_row_index)
            else {
                break;
            };
            if next_cursor <= cursor {
                break;
            }
            rows.push(row);
            cursor = next_cursor;
        }
        if rows.len() > best_rows.len() {
            best_rows = rows;
        }
        index = next_index.max(index + 1);
    }
    best_rows
}

pub(super) fn parse_moxel_row_start_at_for_scanning(
    fields: &[&str],
    index: usize,
) -> Option<(MoxelRow, usize)> {
    let expected_row_index = fields.get(index)?.trim().parse::<usize>().ok()?;
    parse_moxel_row_at_for_scanning(fields, index, expected_row_index)
}

pub(super) fn parse_moxel_row_at(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
) -> Option<(MoxelRow, usize)> {
    if let Some(row) = parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 0,
            format_offset: 1,
            cell_count_offset: 2,
            cells_offset: 3,
            allow_empty: true,
            validate_empty_prefix: false,
        },
    ) {
        return Some(row);
    }
    parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 3,
            format_offset: 4,
            cell_count_offset: 5,
            cells_offset: 6,
            allow_empty: true,
            validate_empty_prefix: true,
        },
    )
}

pub(super) fn parse_moxel_row_at_for_scanning(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
) -> Option<(MoxelRow, usize)> {
    if let Some(row) = parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 0,
            format_offset: 1,
            cell_count_offset: 2,
            cells_offset: 3,
            allow_empty: true,
            validate_empty_prefix: false,
        },
    ) {
        return Some(row);
    }
    if expected_row_index != 0 {
        return None;
    }
    parse_moxel_row_shape(
        fields,
        index,
        expected_row_index,
        MoxelRowShape {
            row_index_offset: 3,
            format_offset: 4,
            cell_count_offset: 5,
            cells_offset: 6,
            allow_empty: true,
            validate_empty_prefix: true,
        },
    )
}

#[derive(Clone, Copy)]
pub(super) struct MoxelRowShape {
    pub(super) row_index_offset: usize,
    pub(super) format_offset: usize,
    pub(super) cell_count_offset: usize,
    pub(super) cells_offset: usize,
    pub(super) allow_empty: bool,
    pub(super) validate_empty_prefix: bool,
}

pub(super) fn parse_moxel_row_shape(
    fields: &[&str],
    index: usize,
    expected_row_index: usize,
    shape: MoxelRowShape,
) -> Option<(MoxelRow, usize)> {
    let row_index = fields
        .get(index + shape.row_index_offset)?
        .trim()
        .parse::<usize>()
        .ok()?;
    if row_index != expected_row_index {
        return None;
    }
    let format_index = fields
        .get(index + shape.format_offset)?
        .trim()
        .parse::<usize>()
        .ok()?
        + 1;
    let cell_count = fields
        .get(index + shape.cell_count_offset)?
        .trim()
        .parse::<usize>()
        .ok()?;
    if (!shape.allow_empty && cell_count == 0) || cell_count > 2048 {
        return None;
    }
    if shape.validate_empty_prefix && cell_count == 0 {
        let prefix_left = fields.get(index)?.trim().parse::<usize>().ok()?;
        let prefix_right = fields.get(index + 1)?.trim().parse::<usize>().ok()?;
        if prefix_left == 0 || prefix_right == 0 {
            return None;
        }
    }
    let mut cells = Vec::with_capacity(cell_count);
    let mut cursor = index + shape.cells_offset;
    for _ in 0..cell_count {
        let column_index = fields.get(cursor)?.trim().parse::<usize>().ok()?;
        let cell = parse_moxel_cell(fields.get(cursor + 1)?, column_index)?;
        cells.push(cell);
        cursor += 2;
    }
    Some((
        MoxelRow {
            index: row_index,
            index_to: None,
            format_index,
            source_format_index: Some(format_index),
            columns_id: None,
            cells,
        },
        cursor,
    ))
}

pub(super) fn parse_moxel_cell(text: &str, column_index: usize) -> Option<MoxelCell> {
    let fields = split_1c_braced_fields(text, 0)?;
    let cell_kind = fields.first()?.trim();
    let format_index = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| if value == 0 { 0 } else { value + 1 })
        .unwrap_or(0);
    let detail_parameter_field = match cell_kind {
        // Native dumps also use these cell kinds for detail-only and note-bearing
        // spreadsheet cells, with detailParameter kept in slot 2.
        "8" | "24" | "56" => Some(2),
        _ => None,
    };
    let detail_parameter = detail_parameter_field
        .and_then(|index| fields.get(index))
        .and_then(|value| parse_1c_string(value));
    let localized_index = detail_parameter_field.map(|index| index + 1).unwrap_or(2);
    let localized = fields
        .get(localized_index)
        .and_then(|value| parse_moxel_localized_cell_value(value));
    let empty_text = matches!(localized, Some(None));
    let localized = localized.flatten();
    let text = localized
        .as_ref()
        .filter(|value| !value.lang.is_empty())
        .map(|value| value.content.clone());
    let parameter = localized
        .as_ref()
        .filter(|value| value.lang.is_empty())
        .map(|value| value.content.clone());
    let note = parse_moxel_cell_note(&fields, cell_kind);
    Some(MoxelCell {
        column_index,
        format_index,
        source_format_index: if format_index == 0 {
            None
        } else {
            Some(format_index)
        },
        text,
        parameter,
        detail_parameter,
        note,
        empty_text,
    })
}

fn parse_moxel_cell_note(fields: &[&str], cell_kind: &str) -> Option<MoxelNote> {
    let note_text_index = match cell_kind {
        "48" if fields.len() == 7 => 3,
        "56" if fields.len() == 8 => 4,
        _ => return None,
    };
    if fields.get(note_text_index + 1)?.trim() != "1"
        || fields.get(note_text_index + 3)?.trim() != "0"
    {
        return None;
    }

    let note_text_field = fields.get(note_text_index)?.trim();
    let text = parse_moxel_single_localized_value(note_text_field)?;
    let note_fields = split_1c_braced_fields(fields.get(note_text_index + 2)?.trim(), 0)?;
    if note_fields.len() != 12
        || note_fields.get(1)?.trim() != "6"
        || note_fields.get(10)?.trim() != "0"
    {
        return None;
    }

    let format_fields = split_1c_braced_fields(note_fields.first()?.trim(), 0)?;
    if format_fields.len() != 4
        || format_fields.first()?.trim() != "16"
        || format_fields.get(2)?.trim() != note_text_field
        || format_fields.get(3)?.trim() != "0"
        || parse_moxel_single_localized_value(format_fields.get(2)?.trim())? != text
    {
        return None;
    }
    let source_format_index = format_fields
        .get(1)?
        .trim()
        .parse::<usize>()
        .ok()?
        .checked_add(1)?;
    if source_format_index == 1 {
        return None;
    }
    let coordinate = |index: usize| note_fields.get(index)?.trim().parse::<i32>().ok();
    let auto_size = match note_fields.get(11)?.trim() {
        "0" => false,
        "1" => true,
        _ => return None,
    };

    Some(MoxelNote {
        format_index: source_format_index,
        source_format_index,
        text,
        begin_row: coordinate(3)?,
        begin_row_offset: coordinate(5)?,
        end_row: coordinate(7)?,
        end_row_offset: coordinate(9)?,
        begin_column: coordinate(2)?,
        begin_column_offset: coordinate(4)?,
        end_column: coordinate(6)?,
        end_column_offset: coordinate(8)?,
        auto_size,
    })
}

fn parse_moxel_single_localized_value(text: &str) -> Option<MoxelLocalizedValue> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "1" || fields.get(1)?.trim() != "1" {
        return None;
    }
    let pair = split_1c_braced_fields(fields.get(2)?.trim(), 0)?;
    if pair.len() != 2 {
        return None;
    }
    Some(MoxelLocalizedValue {
        lang: parse_1c_string(pair.first()?)?,
        content: parse_1c_string(pair.get(1)?)?,
    })
}

pub(super) fn parse_moxel_localized_cell_value(text: &str) -> Option<Option<MoxelLocalizedValue>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 {
        return Some(None);
    }
    let pair = split_1c_braced_fields(fields.iter().skip(2).take(count).next()?, 0)?;
    let lang = parse_1c_string(pair.first()?)?;
    let content = parse_1c_string(pair.get(1)?)?;
    Some(Some(MoxelLocalizedValue { lang, content }))
}

#[allow(dead_code)]
pub(super) fn parse_moxel_areas(fields: &[&str]) -> Vec<MoxelArea> {
    parse_moxel_named_items(fields)
        .into_iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect()
}

pub(super) fn parse_moxel_named_items(fields: &[&str]) -> Vec<MoxelNamedItem> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_named_item_list(field))
        .next()
        .unwrap_or_default()
}

pub(super) fn parse_moxel_print_area(fields: &[&str]) -> Option<MoxelArea> {
    fields.iter().find_map(|field| {
        let bounds = split_1c_braced_fields(field, 0)?;
        if bounds.len() != 6 {
            return None;
        }
        parse_moxel_bounds_area(&bounds, String::new())
    })
}

pub(super) fn parse_moxel_fonts(fields: &[&str]) -> Vec<MoxelFont> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_font(field))
        .collect()
}

pub(super) fn parse_moxel_font(text: &str) -> Option<MoxelFont> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    match fields.get(1)?.trim() {
        "0" if fields.len() >= 19 => {
            let height_raw = fields.get(3)?.trim().parse::<usize>().ok()?;
            let weight = fields.get(7)?.trim().parse::<usize>().ok()?;
            Some(MoxelFont {
                ref_name: None,
                face_name: Some(parse_1c_string(fields.get(16)?)?),
                height: Some(format_moxel_font_height(height_raw)),
                bold: weight >= 700,
                italic: fields.get(8)?.trim() != "0",
                underline: fields.get(9)?.trim() != "0",
                strikeout: fields.get(10)?.trim() != "0",
                kind: "Absolute",
                scale: Some(fields.get(18)?.trim().parse::<usize>().ok()?),
            })
        }
        "2" if fields.len() >= 10 => {
            let raw_fields = split_1c_braced_fields(fields.get(3)?, 0)?;
            let (ref_name, face_name) = match raw_fields.first()?.trim() {
                "-20" => (
                    "style:TextFont",
                    fields.get(8).and_then(|field| parse_1c_string(field)),
                ),
                "-31" => ("style:NormalTextFont", None),
                "-32" => ("style:LargeTextFont", None),
                _ => return None,
            };
            let weight = fields.get(4)?.trim().parse::<usize>().ok()?;
            Some(MoxelFont {
                ref_name: Some(ref_name.to_string()),
                face_name,
                height: None,
                bold: weight >= 700,
                italic: fields.get(5)?.trim() != "0",
                underline: fields.get(6)?.trim() != "0",
                strikeout: fields.get(7)?.trim() != "0",
                kind: "StyleItem",
                scale: None,
            })
        }
        "1" if fields.len() >= 6 => {
            let (height, weight, italic, underline, strikeout, scale) = if fields.len() >= 11 {
                (
                    fields
                        .get(4)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .map(format_moxel_font_height),
                    fields
                        .get(5)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .unwrap_or(400),
                    fields
                        .get(6)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(7)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(8)
                        .map(|field| field.trim() != "0")
                        .unwrap_or(false),
                    fields
                        .get(10)
                        .and_then(|field| field.trim().parse::<usize>().ok()),
                )
            } else if fields.len() >= 8 {
                (
                    fields
                        .get(4)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .map(format_moxel_font_height),
                    fields
                        .get(5)
                        .and_then(|field| field.trim().parse::<usize>().ok())
                        .unwrap_or(400),
                    false,
                    false,
                    false,
                    None,
                )
            } else {
                (None, 400, false, false, false, None)
            };
            Some(MoxelFont {
                ref_name: Some("sys:DefaultGUIFont".to_string()),
                face_name: None,
                height,
                bold: weight >= 700,
                italic,
                underline,
                strikeout,
                kind: "WindowsFont",
                scale,
            })
        }
        _ => None,
    }
}

pub(super) fn format_moxel_font_height(raw_height: usize) -> String {
    if raw_height % 10 == 0 {
        (raw_height / 10).to_string()
    } else {
        format!("{}.{}", raw_height / 10, raw_height % 10)
    }
}

pub(super) fn parse_moxel_lines(
    fields: &[&str],
    formats: &[MoxelFormat],
    shift_default_line_styles: bool,
) -> Vec<MoxelLine> {
    let used_indexes = moxel_used_line_indexes(formats);
    if used_indexes.is_empty() {
        return Vec::new();
    }
    let uses_drawing_line = formats.iter().any(|format| format.drawing_border.is_some());
    let uses_cell_line = formats.iter().any(|format| {
        format.border.is_some()
            || format.left_border.is_some()
            || format.top_border.is_some()
            || format.right_border.is_some()
            || format.bottom_border.is_some()
    });
    let has_thin_dashed_default_line_palette = has_moxel_thin_dashed_default_line_palette(fields);
    let mut lines = fields
        .iter()
        .filter_map(|field| parse_moxel_line(field))
        .collect::<Vec<_>>();
    if uses_drawing_line
        && !uses_cell_line
        && used_indexes.len() == 1
        && let Some(source_index) = used_indexes.iter().next().copied()
        && let Some(source) = lines.get(source_index)
    {
        return vec![MoxelLine {
            style: source.style,
            line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
            width: source.width,
        }];
    }
    if lines.len() > 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
    {
        lines.truncate(3);
    }
    let expected_line_slots =
        expected_moxel_line_slots(&used_indexes, uses_drawing_line, shift_default_line_styles);
    if expected_line_slots > 0
        && lines.len() > expected_line_slots
        && !(lines.len() == 3
            && lines.first().is_some_and(|line| line.style == "None")
            && lines.get(1).is_some_and(|line| line.style == "Solid")
            && lines.get(2).is_some_and(|line| line.style == "Dotted"))
    {
        lines.truncate(expected_line_slots);
    }
    if lines.len() == 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && used_indexes.len() == 4
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
        && used_indexes.contains(&3)
    {
        return vec![
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 3,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
        ];
    }
    if uses_drawing_line
        && lines.len() >= 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
        && used_indexes.len() == 4
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
        && used_indexes.contains(&3)
    {
        if has_thin_dashed_default_line_palette {
            return vec![
                MoxelLine {
                    style: "ThinDashed",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 1,
                },
                MoxelLine {
                    style: "None",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 0,
                },
                MoxelLine {
                    style: "Solid",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 2,
                },
                MoxelLine {
                    style: "None",
                    line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                    width: 1,
                },
            ];
        }
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                width: 1,
            },
        ];
    }
    if uses_drawing_line
        && lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
    {
        return vec![
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                width: 1,
            },
        ];
    }
    if lines.len() >= 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
        && shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && shift_default_line_styles
        && used_indexes.len() == 3
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 3,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && !shift_default_line_styles
        && used_indexes.len() == 3
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
        && used_indexes.contains(&2)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 2,
            },
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "None",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 0,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        return vec![
            MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            MoxelLine {
                style: "Dotted",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
        ];
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && used_indexes.len() == 1
        && used_indexes.contains(&0)
    {
        return vec![MoxelLine {
            style: "Solid",
            line_type: "v8ui:SpreadsheetDocumentCellLineType",
            width: 1,
        }];
    }
    if !lines.is_empty() {
        return lines;
    }
    vec![MoxelLine {
        style: "Solid",
        line_type: "v8ui:SpreadsheetDocumentCellLineType",
        width: 1,
    }]
}

/// The platform serializes the line palette as a count-prefixed table. This
/// four-slot variant contains three style identifiers and the Gray web color.
/// The count prefix is significant because the same style identifiers can
/// occur as unrelated top-level style references.
fn has_moxel_thin_dashed_default_line_palette(fields: &[&str]) -> bool {
    fields.windows(5).any(|window| {
        window[0].trim() == "4"
            && moxel_style_slot_marker(window[1]) == Some(("3", "-1"))
            && moxel_style_slot_marker(window[2]) == Some(("3", "-3"))
            && moxel_style_slot_marker(window[3]) == Some(("2", "52"))
            && moxel_style_slot_marker(window[4]) == Some(("3", "-10"))
    })
}

fn moxel_style_slot_marker(text: &str) -> Option<(&str, &str)> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    if payload.len() != 1 {
        return None;
    }
    Some((fields.get(1)?.trim(), payload.first()?.trim()))
}

pub(super) fn expected_moxel_line_slots(
    used_indexes: &BTreeSet<usize>,
    uses_drawing_line: bool,
    shift_default_line_styles: bool,
) -> usize {
    let mut expected = used_indexes
        .iter()
        .next_back()
        .copied()
        .map(|index| index + 1)
        .unwrap_or(0);
    if used_indexes.len() == 1 && used_indexes.contains(&0) {
        expected = expected.max(2);
    }
    if shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        expected = expected.max(3);
    }
    if uses_drawing_line {
        expected = expected.max(3);
    }
    expected
}

pub(super) fn moxel_used_line_indexes(formats: &[MoxelFormat]) -> BTreeSet<usize> {
    let mut indexes = BTreeSet::new();
    for format in formats {
        for value in [
            format.border,
            format.left_border,
            format.top_border,
            format.right_border,
            format.bottom_border,
            format.drawing_border,
        ] {
            if let Some(index) = value {
                indexes.insert(index);
            }
        }
    }
    indexes
}

pub(super) fn parse_moxel_pictures(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<MoxelPicture> {
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count == 0 || count > 512 || index + count >= fields.len() {
            continue;
        }
        let mut pictures = Vec::with_capacity(count);
        for (picture_index, field) in fields[index + 1..=index + count].iter().enumerate() {
            let Some(mut picture) = parse_moxel_picture(field, object_refs) else {
                pictures.clear();
                break;
            };
            picture.index = picture_index;
            pictures.push(picture);
        }
        if pictures.len() == count {
            return pictures;
        }
    }
    Vec::new()
}

pub(super) fn parse_moxel_picture(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelPicture> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let ref_name = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|picture_ref| {
            match picture_ref.first().map(|field| field.trim()) {
                Some("-13") => return Some("v8ui:Print".to_string()),
                Some("-6") => return Some("v8ui:InputFieldCalculator".to_string()),
                _ => {}
            }
            if picture_ref.first().map(|field| field.trim()) != Some("0") {
                return None;
            }
            let uuid = parse_uuid_field(picture_ref.get(1)?.trim())?;
            match uuid.as_str() {
                STD_PICTURE_INFORMATION_UUID => return Some("v8ui:Information".to_string()),
                STD_PICTURE_SAVE_FILE_UUID => return Some("v8ui:SaveFile".to_string()),
                _ => {}
            }
            object_refs
                .get(&uuid)
                .and_then(|reference| reference.strip_prefix("CommonPicture."))
                .map(|name| format!("v8ui:{name}"))
        });
    let payload = fields
        .iter()
        .find_map(|field| extract_base64_payload(field))
        .map(normalize_moxel_picture_payload);
    Some(MoxelPicture {
        index: fields.get(1)?.trim().parse::<usize>().ok()?,
        ref_name,
        payload,
    })
}

pub(super) fn normalize_moxel_picture_payload(payload: &str) -> String {
    let has_trailing_line_break = payload.ends_with('\n') || payload.ends_with('\r');
    let mut normalized = payload
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\r\n");
    if has_trailing_line_break && !normalized.is_empty() {
        normalized.push_str("\r\n");
    }
    normalized
}

pub(super) fn parse_moxel_drawings(fields: &[&str]) -> Vec<MoxelDrawing> {
    let mut drawings = Vec::new();
    for field in fields {
        let Some(mut drawing) = parse_moxel_drawing(field) else {
            continue;
        };
        if matches!(drawing.kind, MoxelDrawingKind::Chart(_)) {
            drawing.z_order = drawings.len() + 1;
        }
        drawings.push(drawing);
    }
    drawings
}

pub(super) fn parse_moxel_drawing(text: &str) -> Option<MoxelDrawing> {
    const CHART_TYPE_UUID: &str = "a8b97779-1a4b-4059-b09c-807f86d2a461";

    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 14 {
        return None;
    }
    let format_fields = split_1c_braced_fields(fields.first()?, 0)?;
    if format_fields.len() != 2 || format_fields.first()?.trim() != "0" {
        return None;
    }
    let begin_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    let begin_column_offset = fields.get(4)?.trim().parse::<i32>().ok()?;
    let begin_row_offset = fields.get(5)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(6)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(7)?.trim().parse::<i32>().ok()?;
    let end_column_offset = fields.get(8)?.trim().parse::<i32>().ok()?;
    let end_row_offset = fields.get(9)?.trim().parse::<i32>().ok()?;
    if begin_column < 0
        || begin_row < 0
        || end_column < begin_column
        || end_row < begin_row
        || begin_column_offset < 0
        || begin_row_offset < 0
        || end_column_offset < 0
        || end_row_offset < 0
    {
        return None;
    }
    let (kind, auto_size, z_order) = match fields.get(1)?.trim() {
        "5" => {
            let picture_index = fields.get(11)?.trim().parse::<usize>().ok()?;
            let picture_size = match fields.get(12)?.trim().parse::<usize>().ok()? {
                0 => "RealSize",
                1 => "Stretch",
                2 => "Proportionally",
                4 => "AutoSize",
                _ => return None,
            };
            (
                MoxelDrawingKind::Picture {
                    picture_size,
                    picture_index,
                },
                fields.get(13)?.trim() != "0",
                picture_index,
            )
        }
        "10" if fields.get(11)?.trim().eq_ignore_ascii_case(CHART_TYPE_UUID)
            && fields.get(13)?.trim() == "0" =>
        {
            (
                MoxelDrawingKind::Chart(parse_moxel_chart(fields.get(12)?)?),
                false,
                0,
            )
        }
        _ => return None,
    };
    let id = fields.get(10)?.trim().parse::<usize>().ok()?;
    Some(MoxelDrawing {
        id,
        format_index: format_fields.get(1)?.trim().parse::<usize>().ok()?,
        begin_row,
        begin_row_offset,
        end_row,
        end_row_offset,
        begin_column,
        begin_column_offset,
        end_column,
        end_column_offset,
        auto_size,
        z_order,
        kind,
    })
}

const MAX_MOXEL_CHART_BYTES: usize = 1024 * 1024;
const MAX_MOXEL_CHART_SERIES: usize = 64;
const MAX_MOXEL_CHART_POINTS: usize = 1024;
const MAX_MOXEL_CHART_LOCALIZED_VALUES: usize = 64;
const MAX_MOXEL_CHART_DECIMAL_BYTES: usize = 4096;
const MAX_MOXEL_CHART_DECIMAL_EXPONENT: u64 = 4096;

fn parse_moxel_chart(text: &str) -> Option<MoxelChart> {
    if text.len() > MAX_MOXEL_CHART_BYTES {
        return None;
    }
    let payload = split_1c_braced_fields(text, 0)?;
    if payload.len() != 2 || compact_moxel_chart_token(payload.first()?) != "{11}" {
        return None;
    }
    let data = split_1c_braced_fields(payload.get(1)?, 0)?;
    if data.first()?.trim() != "74" || data.len() > MAX_MOXEL_CHART_POINTS * 16 {
        return None;
    }
    let series_cur_id = parse_moxel_chart_usize(data.get(1)?)?;
    let points_cur_id = parse_moxel_chart_usize(data.get(2)?)?;
    let is_series_design = parse_moxel_chart_bool(data.get(3)?)?;
    let series_count = parse_moxel_chart_usize(data.get(4)?)?;
    if series_count == 0 || series_count > MAX_MOXEL_CHART_SERIES {
        return None;
    }

    let mut cursor = 5usize;
    let mut real_series = Vec::with_capacity(series_count);
    for _ in 0..series_count {
        real_series.push(parse_moxel_chart_series(
            data.get(cursor..cursor.checked_add(11)?)?,
        )?);
        cursor += 11;
    }
    let real_extra_series = parse_moxel_chart_series(data.get(cursor..cursor.checked_add(11)?)?)?;
    cursor += 11;
    let is_points_design = parse_moxel_chart_bool(data.get(cursor)?)?;
    let point_count = parse_moxel_chart_usize(data.get(cursor + 1)?)?;
    if point_count == 0 || point_count > MAX_MOXEL_CHART_POINTS {
        return None;
    }
    cursor += 2;
    let mut real_points = Vec::with_capacity(point_count);
    for _ in 0..point_count {
        real_points.push(parse_moxel_chart_point(
            data.get(cursor..cursor.checked_add(11)?)?,
        )?);
        cursor += 11;
    }
    let tail = data.get(cursor..)?;
    let real_data_count = series_count.checked_mul(point_count)?;
    let real_data_slots = real_data_count.checked_mul(3)?;
    let post_start = 100usize.checked_add(real_data_slots)?;
    let expected_tail_len = 200usize.checked_add(point_count.checked_mul(5)?)?;
    if series_count != 1 || tail.len() != expected_tail_len || post_start > tail.len() {
        return None;
    }

    validate_moxel_chart_v74_front(tail)?;
    let cur_series = parse_moxel_chart_usize(tail.first()?)?;
    let cur_point = parse_moxel_chart_usize(tail.get(1)?)?;
    let labels_location = match tail.get(5)?.trim() {
        "0" => "Edge",
        "4" => "Auto",
        _ => return None,
    };
    let title = parse_moxel_chart_localized(tail.get(11)?)?;
    let values_scale_format = parse_moxel_chart_localized(tail.get(39)?)?;
    let is_auto_series_name = parse_moxel_chart_bool(tail.get(43)?)?;
    let max_series = parse_moxel_chart_usize(tail.get(46)?)?;
    let is_outline = parse_moxel_chart_bool(tail.get(50)?)?;
    let gauge_bands = parse_moxel_chart_gauge_bands(tail.get(69)?)?;
    let user_max_value = normalize_moxel_chart_decimal(tail.get(78)?)?;
    let user_min_value = normalize_moxel_chart_decimal(tail.get(80)?)?;

    let mut real_data_items = Vec::with_capacity(real_data_count);
    for item_index in 0..real_data_count {
        let item_start = 100 + item_index * 3;
        real_data_items.push(parse_moxel_chart_data_item(
            tail.get(item_start)?,
            tail.get(item_start + 1)?,
            tail.get(item_start + 2)?,
        )?);
    }

    let post = tail.get(post_start..)?;
    if post.len() != 100 + point_count * 2 {
        return None;
    }
    validate_moxel_chart_v74_post(post, point_count)?;
    let rebuild_time = parse_moxel_chart_usize(post.get(21)?)?;
    let spline_strain = parse_moxel_chart_usize(post.get(12)?)?;
    let translucence_percent = normalize_moxel_chart_decimal(post.get(11)?)?;
    let funnel_neck_height_percent = moxel_chart_fraction_to_percent(post.get(13)?)?;
    let funnel_neck_width_percent = moxel_chart_fraction_to_percent(post.get(14)?)?;
    let funnel_gap_sum_percent = moxel_chart_fraction_to_percent(post.get(15)?)?;
    let values_axis = parse_moxel_chart_axis(post.get(29)?)?;
    let points_axis = parse_moxel_chart_axis(post.get(30)?)?;
    let rectangle_start = 66usize.checked_add(point_count)?;
    let elements_chart =
        parse_moxel_chart_rectangle(post.get(rectangle_start..rectangle_start + 4)?)?;
    let elements_legend =
        parse_moxel_chart_rectangle(post.get(rectangle_start + 4..rectangle_start + 8)?)?;
    let elements_title =
        parse_moxel_chart_rectangle(post.get(rectangle_start + 8..rectangle_start + 12)?)?;

    Some(MoxelChart {
        series_cur_id,
        points_cur_id,
        is_series_design,
        real_series,
        real_extra_series,
        is_points_design,
        real_points,
        cur_series,
        cur_point,
        labels_location,
        title,
        values_scale_format,
        is_auto_series_name,
        max_series,
        is_outline,
        rebuild_time,
        gauge_bands,
        user_max_value,
        user_min_value,
        real_data_items,
        spline_strain,
        translucence_percent,
        funnel_neck_height_percent,
        funnel_neck_width_percent,
        funnel_gap_sum_percent,
        elements_chart,
        elements_legend,
        elements_title,
        values_axis,
        points_axis,
    })
}

fn parse_moxel_chart_series(fields: &[&str]) -> Option<MoxelChartSeries> {
    if fields.len() != 11
        || compact_moxel_chart_token(fields.get(8)?) != "{\"U\"}"
        || compact_moxel_chart_token(fields.get(9)?) != "{\"U\"}"
    {
        return None;
    }
    Some(MoxelChartSeries {
        id: parse_moxel_chart_usize(fields.get(7)?)?,
        color: parse_moxel_chart_color(fields.first()?)?,
        line: parse_moxel_chart_line(fields.get(1)?)?,
        marker: moxel_chart_marker(fields.get(2)?)?,
        text: parse_moxel_chart_localized(fields.get(3)?)?,
        str_is_changed: parse_moxel_chart_bool(fields.get(4)?)?,
        is_expand: parse_moxel_chart_bool(fields.get(5)?)?,
        is_indicator: parse_moxel_chart_bool(fields.get(6)?)?,
        color_priority: parse_moxel_chart_bool(fields.get(10)?)?,
    })
}

fn parse_moxel_chart_point(fields: &[&str]) -> Option<MoxelChartPoint> {
    if fields.len() != 11
        || compact_moxel_chart_token(fields.get(8)?) != "{\"U\"}"
        || compact_moxel_chart_token(fields.get(9)?) != "{\"U\"}"
    {
        return None;
    }
    let str_is_changed = parse_moxel_chart_bool(fields.get(1)?)?;
    Some(MoxelChartPoint {
        id: parse_moxel_chart_usize(fields.get(2)?)?,
        color: if str_is_changed {
            parse_moxel_chart_color(fields.get(3)?)?
        } else {
            "auto".to_string()
        },
        line: parse_moxel_chart_line(fields.get(4)?)?,
        marker: moxel_chart_marker(fields.get(5)?)?,
        text: parse_moxel_chart_localized(fields.first()?)?,
        str_is_changed,
        is_expand: parse_moxel_chart_bool(fields.get(6)?)?,
        is_indicator: parse_moxel_chart_bool(fields.get(7)?)?,
        color_priority: parse_moxel_chart_bool(fields.get(10)?)?,
    })
}

fn parse_moxel_chart_localized(text: &str) -> Option<Vec<MoxelLocalizedValue>> {
    let values = parse_moxel_localized_values(text)?;
    if values.len() > MAX_MOXEL_CHART_LOCALIZED_VALUES
        || values
            .iter()
            .any(|value| value.lang.len() + value.content.len() > MAX_MOXEL_CHART_BYTES)
    {
        return None;
    }
    Some(values)
}

fn parse_moxel_chart_color(text: &str) -> Option<String> {
    const BORDER_COLOR_UUID: &str = "48312c09-257f-4b29-b280-284dd89efc1e";

    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    match fields.get(1)?.trim() {
        "4" if fields.len() == 3 && payload.as_slice() == ["0"] => Some("auto".to_string()),
        "3" if fields.len() == 3 && payload.len() == 1 => match payload.first()?.trim() {
            "-1" => Some("style:FormBackColor".to_string()),
            "-3" => Some("style:FormTextColor".to_string()),
            "-22" => Some("style:BorderColor".to_string()),
            _ => None,
        },
        "0" if payload.len() == 1 => {
            if fields.len() == 3 {
                parse_moxel_direct_color(payload.first()?.trim())
            } else if fields.len() == 7
                && payload.first()?.trim() == "0"
                && fields
                    .get(6)?
                    .trim()
                    .eq_ignore_ascii_case(BORDER_COLOR_UUID)
            {
                Some("style:BorderColor".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_moxel_chart_line(text: &str) -> Option<MoxelChartLine> {
    const SOLID_LINE_UUID: &str = "e5cabe59-d992-4d31-8086-3116931aff81";

    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 8
        || fields.first()?.trim() != "4"
        || fields.get(1)?.trim() != "0"
        || compact_moxel_chart_token(fields.get(2)?) != "{0}"
        || fields.get(3)?.trim() != "1"
        || fields.get(5)?.trim() != "0"
        || !fields.get(6)?.trim().eq_ignore_ascii_case(SOLID_LINE_UUID)
        || fields.get(7)?.trim() != "0"
    {
        return None;
    }
    Some(MoxelChartLine {
        width: parse_moxel_chart_usize(fields.get(4)?)?,
    })
}

fn moxel_chart_marker(text: &str) -> Option<&'static str> {
    match text.trim() {
        "0" => Some("None"),
        "1" => Some("Rect"),
        "2" => Some("Circle"),
        "3" => Some("Rhomb"),
        _ => None,
    }
}

fn parse_moxel_chart_gauge_bands(text: &str) -> Option<Vec<MoxelChartGaugeBand>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = parse_moxel_chart_usize(fields.get(1)?)?;
    if count > MAX_MOXEL_CHART_POINTS
        || fields.len() != count.checked_add(4)?
        || fields.get(count + 2)?.trim() != "0"
        || fields.get(count + 3)?.trim() != "0"
    {
        return None;
    }
    let mut bands = Vec::with_capacity(count);
    for item in fields.iter().skip(2).take(count) {
        let item = split_1c_braced_fields(item, 0)?;
        if item.len() != 12
            || item.first()?.trim() != "3"
            || normalize_moxel_chart_decimal(item.get(1)?)?
                != normalize_moxel_chart_decimal(item.get(10)?)?
            || normalize_moxel_chart_decimal(item.get(2)?)?
                != normalize_moxel_chart_decimal(item.get(11)?)?
        {
            return None;
        }
        bands.push(MoxelChartGaugeBand {
            begin: normalize_moxel_chart_decimal(item.get(1)?)?,
            end: normalize_moxel_chart_decimal(item.get(2)?)?,
            back_color: parse_moxel_chart_color(item.get(3)?)?,
            text: parse_moxel_chart_localized(item.get(4)?)?,
            tooltip: parse_moxel_chart_localized(item.get(5)?)?,
        });
    }
    Some(bands)
}

fn parse_moxel_chart_data_item(
    value: &str,
    value_info: &str,
    tooltip: &str,
) -> Option<MoxelChartDataItem> {
    let typed = split_1c_braced_fields(value, 0)?;
    if typed.len() != 2
        || parse_1c_string(typed.first()?)? != "N"
        || compact_moxel_chart_token(value_info) != "{\"U\"}"
    {
        return None;
    }
    Some(MoxelChartDataItem {
        value: normalize_moxel_chart_decimal(typed.get(1)?)?,
        tooltip: parse_1c_string(tooltip)?,
    })
}

fn parse_moxel_chart_axis(text: &str) -> Option<MoxelChartAxis> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 5
        || fields.first()?.trim() != "0"
        || fields.get(1)?.trim() != "0"
        || fields.get(3)?.trim() != "0"
        || fields.get(4)?.trim() != "0"
    {
        return None;
    }
    let range = split_1c_braced_fields(fields.get(2)?, 0)?;
    if range.len() != 5
        || range.first()?.trim() != "0"
        || range.get(1)?.trim() != "1"
        || range.get(3)?.trim() != "1"
    {
        return None;
    }
    let min = normalize_moxel_chart_decimal(range.get(2)?)?;
    let max = normalize_moxel_chart_decimal(range.get(4)?)?;
    Some(MoxelChartAxis {
        min_value: (min != "0").then_some(min),
        max_value: (max != "0").then_some(max),
    })
}

fn parse_moxel_chart_rectangle(fields: &[&str]) -> Option<MoxelChartRectangle> {
    if fields.len() != 4 {
        return None;
    }
    Some(MoxelChartRectangle {
        left: normalize_moxel_chart_decimal(fields.first()?)?,
        right: normalize_moxel_chart_decimal(fields.get(2)?)?,
        top: normalize_moxel_chart_decimal(fields.get(1)?)?,
        bottom: normalize_moxel_chart_decimal(fields.get(3)?)?,
    })
}

fn validate_moxel_chart_v74_front(tail: &[&str]) -> Option<()> {
    let expected = [
        (2, "0"),
        (3, "0"),
        (4, "\", \""),
        (6, "{1,0}"),
        (7, "{1,0}"),
        (8, "{3,3,{-3}}"),
        (9, "0"),
        (10, "0"),
        (12, "0"),
        (13, "0"),
        (14, "{3,0,{0},0,0,0,48312c09-257f-4b29-b280-284dd89efc1e}"),
        (15, "{3,3,{-22}}"),
        (16, "{3,0,{0},0,0,0,48312c09-257f-4b29-b280-284dd89efc1e}"),
        (17, "{3,3,{-22}}"),
        (18, "{3,0,{0},0,0,0,48312c09-257f-4b29-b280-284dd89efc1e}"),
        (19, "{3,3,{-22}}"),
        (20, "0"),
        (21, "{3,3,{-1}}"),
        (22, "1"),
        (23, "{3,3,{-1}}"),
        (24, "1"),
        (25, "{3,3,{-1}}"),
        (26, "0"),
        (27, "{3,0,{16777215}}"),
        (28, "{3,3,{-3}}"),
        (29, "{3,3,{-3}}"),
        (30, "{3,3,{-3}}"),
        (31, "{7,2,0,{-20},1,100}"),
        (32, "{7,2,0,{-20},1,100}"),
        (33, "{7,2,0,{-20},1,100}"),
        (34, "1"),
        (35, "1"),
        (36, "1"),
        (37, "1"),
        (38, "1"),
        (40, "0"),
        (41, "{4,0,{0},1,1,0,e5cabe59-d992-4d31-8086-3116931aff81,0}"),
        (42, "{3,0,{11119017}}"),
        (44, "0"),
        (45, "0"),
        (47, "30"),
        (48, "1"),
        (49, "0"),
        (51, "0"),
        (52, "0"),
        (53, "1"),
        (54, "0"),
        (55, "0"),
        (56, "0"),
        (57, "0"),
        (58, "1"),
        (59, "1"),
        (60, "2"),
        (61, "{1,0}"),
        (62, "1"),
        (63, "0"),
        (64, "0"),
        (65, "0"),
        (66, "{3,0,{169}}"),
        (67, "0"),
        (68, "0"),
        (70, "0"),
        (71, "180"),
        (72, "2"),
        (73, "1"),
        (74, "0"),
        (75, "5"),
        (76, "{3,0,{11119017}}"),
        (77, "1"),
        (79, "1"),
        (81, "1"),
        (82, "0"),
        (83, "0"),
        (84, "0"),
        (85, "0"),
        (86, "0"),
        (87, "0"),
        (88, "1"),
        (89, "1"),
        (90, "0"),
        (91, "0"),
        (92, "1"),
        (93, "1"),
        (94, "0"),
        (95, "{3,3,{-22}}"),
        (96, "{3,0,{0},0,0,0,48312c09-257f-4b29-b280-284dd89efc1e}"),
        (97, "\"\""),
        (98, "0"),
        (99, "1"),
    ];
    expected
        .iter()
        .all(|(index, value)| {
            tail.get(*index)
                .is_some_and(|slot| compact_moxel_chart_token(slot) == *value)
        })
        .then_some(())
}

fn validate_moxel_chart_v74_post(post: &[&str], point_count: usize) -> Option<()> {
    let expected = [
        (0, "14"),
        (1, "2"),
        (2, "{7,3,0,1,100}"),
        (3, "1"),
        (4, "{3,4,{0}}"),
        (5, "{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}"),
        (6, "{3,4,{0}}"),
        (7, "1"),
        (8, "1"),
        (9, "1"),
        (10, "0"),
        (16, "{4,0,{0},1,1,0,e5cabe59-d992-4d31-8086-3116931aff81,0}"),
        (17, "{3,0,{0}}"),
        (18, "2"),
        (19, "255"),
        (20, "0"),
        (22, "00000000-0000-0000-0000-000000000000"),
        (23, "2"),
        (24, "{0,1,0}"),
        (25, "{0,2,0}"),
        (26, "{0,0}"),
        (27, "{0,0}"),
        (28, "0"),
        (31, "0"),
        (32, "0"),
        (33, "2"),
        (34, "-2"),
        (35, "1"),
        (36, "10"),
        (37, "1"),
        (38, "20"),
        (39, "0"),
        (40, "0"),
        (44, "0"),
        (45, "0"),
        (46, "{3,4,{0}}"),
        (47, "{3,4,{0}}"),
        (48, "0"),
    ];
    if !expected.iter().all(|(index, value)| {
        post.get(*index)
            .is_some_and(|slot| compact_moxel_chart_token(slot) == *value)
    }) {
        return None;
    }
    let rectangle_start = 66usize.checked_add(point_count)?;
    [
        (rectangle_start - 5, "1"),
        (rectangle_start - 4, "1"),
        (rectangle_start - 3, "1"),
        (rectangle_start - 2, "6"),
        (rectangle_start - 1, "8"),
    ]
    .iter()
    .all(|(index, value)| {
        post.get(*index)
            .is_some_and(|slot| compact_moxel_chart_token(slot) == *value)
    })
    .then_some(())
}

fn parse_moxel_chart_bool(text: &str) -> Option<bool> {
    match text.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_moxel_chart_usize(text: &str) -> Option<usize> {
    text.trim().parse::<usize>().ok()
}

fn compact_moxel_chart_token(text: &str) -> String {
    let mut compact = String::with_capacity(text.len());
    let mut quoted = false;
    let mut chars = text.trim().chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            compact.push(ch);
            if quoted && chars.peek() == Some(&'"') {
                compact.push(chars.next().unwrap_or('"'));
            } else {
                quoted = !quoted;
            }
        } else if quoted || !ch.is_whitespace() {
            compact.push(ch);
        }
    }
    compact
}

pub(super) fn normalize_moxel_chart_decimal(text: &str) -> Option<String> {
    let value = text.trim();
    if value.is_empty() || value.len() > MAX_MOXEL_CHART_DECIMAL_BYTES {
        return None;
    }
    let (negative, unsigned) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let (mantissa, exponent) = unsigned.split_once(['e', 'E']).map_or(
        Some((unsigned, 0i64)),
        |(mantissa, exponent)| {
            let exponent = exponent.parse::<i64>().ok()?;
            (exponent.unsigned_abs() <= MAX_MOXEL_CHART_DECIMAL_EXPONENT)
                .then_some((mantissa, exponent))
        },
    )?;
    let mut digits = String::new();
    let mut fractional_digits = 0usize;
    let mut decimal_seen = false;
    for ch in mantissa.chars() {
        if ch == '.' && !decimal_seen {
            decimal_seen = true;
        } else if ch.is_ascii_digit() {
            digits.push(ch);
            if decimal_seen {
                fractional_digits += 1;
            }
        } else {
            return None;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let decimal_position = i64::try_from(digits.len())
        .ok()?
        .checked_sub(i64::try_from(fractional_digits).ok()?)?
        .checked_add(exponent)?;
    let unsigned_output_len = if decimal_position <= 0 {
        2usize
            .checked_add(usize::try_from(decimal_position.unsigned_abs()).ok()?)?
            .checked_add(digits.len())?
    } else {
        let decimal_position = usize::try_from(decimal_position).ok()?;
        if decimal_position >= digits.len() {
            decimal_position
        } else {
            digits.len().checked_add(1)?
        }
    };
    let output_len = unsigned_output_len.checked_add(usize::from(negative))?;
    if output_len > MAX_MOXEL_CHART_DECIMAL_BYTES {
        return None;
    }
    let mut normalized = if decimal_position <= 0 {
        format!(
            "0.{}{}",
            "0".repeat(usize::try_from(decimal_position.unsigned_abs()).ok()?),
            digits
        )
    } else if usize::try_from(decimal_position).ok()? >= digits.len() {
        let zeros = usize::try_from(decimal_position).ok()? - digits.len();
        format!("{digits}{}", "0".repeat(zeros))
    } else {
        let position = usize::try_from(decimal_position).ok()?;
        format!("{}.{}", &digits[..position], &digits[position..])
    };
    if normalized.contains('.') {
        while normalized.ends_with('0') {
            normalized.pop();
        }
        if normalized.ends_with('.') {
            normalized.pop();
        }
    }
    while normalized.len() > 1
        && normalized.starts_with('0')
        && normalized.as_bytes().get(1) != Some(&b'.')
    {
        normalized.remove(0);
    }
    if negative && normalized != "0" {
        normalized.insert(0, '-');
    }
    (normalized.len() <= MAX_MOXEL_CHART_DECIMAL_BYTES).then_some(normalized)
}

pub(super) fn moxel_chart_fraction_to_percent(text: &str) -> Option<String> {
    let value = normalize_moxel_chart_decimal(text)?;
    normalize_moxel_chart_decimal(&format!("{value}e2"))
}

pub(super) fn parse_moxel_default_format_width(
    fields: &[&str],
    column_count: usize,
) -> Option<usize> {
    if let Some((table_index, slots)) =
        parse_moxel_equal_width_only_format_table(fields, column_count)
        && slots.iter().any(|slot| slot.is_none())
        && let Some(width) = fields[..table_index]
            .iter()
            .rev()
            .find_map(|field| parse_moxel_column_width(field))
    {
        return Some(width);
    }
    let widths = fields
        .iter()
        .filter_map(|field| parse_moxel_column_width(field))
        .collect::<Vec<_>>();
    if widths.len() <= column_count {
        return fields
            .iter()
            .find_map(|field| parse_moxel_extended_default_format_width(field))
            .or_else(|| {
                fields
                    .iter()
                    .take(8)
                    .find_map(|field| parse_moxel_leading_default_format_width_129(field))
            });
    }
    widths.first().copied().or_else(|| {
        fields
            .iter()
            .take(8)
            .find_map(|field| parse_moxel_leading_default_format_width_129(field))
    })
}

pub(super) fn parse_moxel_extended_default_format_width(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 4
        || fields.first()?.trim() != "161"
        || fields.get(1)?.trim() != "0"
        || fields.get(2)?.trim() != "0"
    {
        return None;
    }
    fields.get(3)?.trim().parse::<usize>().ok()
}

pub(super) fn parse_moxel_leading_default_format_width_129(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "129" || fields.get(1)?.trim() != "0" {
        return None;
    }
    fields.get(2)?.trim().parse::<usize>().ok()
}

pub(super) fn parse_moxel_default_format(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> MoxelFormat {
    fields
        .iter()
        .filter_map(|field| parse_moxel_default_format_field(field, object_refs))
        .next()
        .unwrap_or_default()
}

pub(super) fn parse_moxel_leading_default_format(
    fields: &[&str],
    style_refs: &[Option<String>],
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<MoxelFormat> {
    fields
        .iter()
        .take(8)
        .filter_map(|field| parse_moxel_format(field, style_refs, number_format_refs))
        .find(|format| !format.is_empty())
}

pub(super) fn parse_moxel_default_format_field(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelFormat> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3
        || fields.first().map(|field| field.trim()) != Some("1")
        || fields.get(1).map(|field| field.trim()) != Some("0")
    {
        return None;
    }
    let border_color = fields
        .get(2)
        .and_then(|field| parse_moxel_style_ref_slot(field, object_refs))
        .flatten()?;
    Some(MoxelFormat {
        border_color: Some(border_color),
        ..MoxelFormat::default()
    })
}

pub(super) fn parse_moxel_print_settings(fields: &[&str]) -> Option<MoxelPrintSettings> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_print_settings_field(field))
        .next()
}

pub(super) fn parse_moxel_print_settings_field(text: &str) -> Option<MoxelPrintSettings> {
    let mut fields = split_1c_braced_fields(text, 0)?;
    if fields.len() == 1 && fields.first()?.trim_start().starts_with('{') {
        fields = split_1c_braced_fields(fields.first()?, 0)?;
    }
    if fields.len() < 4 || fields.first().map(|field| field.trim()) != Some("0") {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 20 || fields.len() != count * 2 + 2 {
        return None;
    }
    let mut settings = MoxelPrintSettings::default();
    let mut seen_keys = BTreeSet::new();
    for pair in fields[2..].chunks_exact(2) {
        let key = pair.first()?.trim().parse::<usize>().ok()?;
        if !matches!(key, 0..=17 | 19 | 20) || !seen_keys.insert(key) {
            return None;
        }
        let value = parse_moxel_print_settings_value(pair.get(1)?)?;
        match key {
            0 => settings.paper = value.as_usize(),
            1 => settings.page_orientation = value.as_usize().and_then(moxel_page_orientation),
            2 => settings.scale = value.as_usize(),
            3 => settings.collate = value.as_bool(),
            4 => settings.copies = value.as_usize(),
            5 => settings.per_page = value.as_usize(),
            6 => settings.top_margin = value.as_usize(),
            7 => settings.left_margin = value.as_usize(),
            8 => settings.bottom_margin = value.as_usize(),
            9 => settings.right_margin = value.as_usize(),
            10 => settings.header_size = value.as_usize(),
            11 => settings.footer_size = value.as_usize(),
            12 => settings.fit_to_page = value.as_bool(),
            13 => settings.black_and_white = value.as_bool(),
            14 => settings.printer_name = value.into_string(),
            15 => settings.paper_source = value.as_usize(),
            16 => settings.page_width = value.as_usize(),
            17 => settings.page_height = value.as_usize(),
            19 => {
                settings.duplex_type = Some(moxel_duplex_type(value.as_usize()?)?);
            }
            20 => {
                settings.page_placement_alternation =
                    Some(moxel_page_placement_alternation(value.as_usize()?)?);
            }
            _ => return None,
        }
    }
    let has_extended_keys = seen_keys.contains(&19) || seen_keys.contains(&20);
    if has_extended_keys
        && (count != 20
            || !seen_keys.contains(&19)
            || !seen_keys.contains(&20)
            || !(0..=17).all(|key| seen_keys.contains(&key)))
    {
        return None;
    }
    Some(settings)
}

pub(super) enum MoxelPrintSettingsValue {
    Number(usize),
    Text(String),
}

impl MoxelPrintSettingsValue {
    pub(super) fn as_usize(&self) -> Option<usize> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(_) => None,
        }
    }

    pub(super) fn as_bool(&self) -> Option<bool> {
        self.as_usize().map(|value| value != 0)
    }

    pub(super) fn into_string(self) -> Option<String> {
        match self {
            Self::Number(_) => None,
            Self::Text(value) => Some(value),
        }
    }
}

pub(super) fn parse_moxel_print_settings_value(text: &str) -> Option<MoxelPrintSettingsValue> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 {
        return None;
    }
    match fields.first()?.trim().trim_matches('"') {
        "N" => fields
            .get(1)?
            .trim()
            .parse::<usize>()
            .ok()
            .map(MoxelPrintSettingsValue::Number),
        "S" => Some(MoxelPrintSettingsValue::Text(
            unquote_moxel_string(fields.get(1)?.trim()).unwrap_or_else(|| fields[1].to_string()),
        )),
        _ => None,
    }
}

pub(super) fn unquote_moxel_string(value: &str) -> Option<String> {
    let value = value.trim();
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\"\"", "\""))
}

fn remap_moxel_source_fonts(
    source_font_map: &MoxelSourceFontMap,
    spreadsheet: &mut MoxelSpreadsheet,
) {
    let Some(output_fonts) = source_font_map.output_fonts(&spreadsheet.fonts) else {
        return;
    };
    let Some(output_default_font) = source_font_map.output_format_font(&spreadsheet.default_format)
    else {
        return;
    };
    let Some(output_column_fonts) = spreadsheet
        .column_formats
        .iter()
        .map(|format| source_font_map.output_format_font(format))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let Some(output_format_fonts) = spreadsheet
        .formats
        .iter()
        .map(|format| source_font_map.output_format_font(format))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let Some(output_extra_format_fonts) = spreadsheet
        .extra_formats
        .values()
        .map(|format| source_font_map.output_format_font(format))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };

    spreadsheet.fonts = output_fonts;
    spreadsheet.default_format.font = output_default_font;
    for (format, output_font) in spreadsheet
        .column_formats
        .iter_mut()
        .zip(output_column_fonts)
    {
        format.font = output_font;
    }
    for (format, output_font) in spreadsheet.formats.iter_mut().zip(output_format_fonts) {
        format.font = output_font;
    }
    for (format, output_font) in spreadsheet
        .extra_formats
        .values_mut()
        .zip(output_extra_format_fonts)
    {
        format.font = output_font;
    }
}

fn parse_moxel_formats_with_source_map(
    fields: &[&str],
    column_count: usize,
    sparse_source_format_refs: bool,
    source_column_format_refs: &[usize],
    source_column_format_order: &[usize],
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> (
    Vec<MoxelFormat>,
    Vec<MoxelFormat>,
    Option<MoxelSourceFormatMap>,
    bool,
) {
    if sparse_source_format_refs
        && !source_column_format_refs.is_empty()
        && !source_column_format_order.is_empty()
        && let Some(formats) = parse_moxel_format_table(
            fields,
            column_count,
            style_refs,
            drawing_format_indices,
            number_format_refs,
        )
    {
        let source_format_map = MoxelSourceFormatMap::try_new(
            formats.len(),
            source_column_format_refs,
            source_column_format_order,
        );
        let (column_formats, formats) =
            split_moxel_formats_by_source_refs(formats, source_column_format_refs);
        return (column_formats, formats, source_format_map, false);
    }

    let (column_formats, formats, leading_source_column_formats) = parse_moxel_formats_with_layout(
        fields,
        column_count,
        sparse_source_format_refs,
        source_column_format_refs,
        style_refs,
        drawing_format_indices,
        number_format_refs,
    );
    (column_formats, formats, None, leading_source_column_formats)
}

#[cfg(test)]
pub(super) fn parse_moxel_formats(
    fields: &[&str],
    column_count: usize,
    sparse_source_format_refs: bool,
    source_column_format_refs: &[usize],
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    let (column_formats, formats, _) = parse_moxel_formats_with_layout(
        fields,
        column_count,
        sparse_source_format_refs,
        source_column_format_refs,
        style_refs,
        drawing_format_indices,
        number_format_refs,
    );
    (column_formats, formats)
}

fn parse_moxel_formats_with_layout(
    fields: &[&str],
    column_count: usize,
    sparse_source_format_refs: bool,
    source_column_format_refs: &[usize],
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>, bool) {
    let all_formats = parse_moxel_format_table(
        fields,
        column_count,
        style_refs,
        drawing_format_indices,
        number_format_refs,
    );
    if let Some(formats) = all_formats {
        if sparse_source_format_refs && !source_column_format_refs.is_empty() {
            let (column_formats, formats) =
                split_moxel_formats_by_source_refs(formats, source_column_format_refs);
            return (column_formats, formats, false);
        }
        if prefers_moxel_leading_source_column_formats(&formats, source_column_format_refs) {
            let (column_formats, formats) =
                split_moxel_formats_by_source_refs(formats, source_column_format_refs);
            return (column_formats, formats, true);
        }
        let (column_formats, formats) = split_moxel_formats_for_output(
            formats,
            column_count,
            sparse_source_format_refs,
            drawing_format_indices,
        );
        return (column_formats, formats, false);
    }
    if let Some((_, slots)) = parse_moxel_equal_width_only_format_table(fields, column_count) {
        let formats = slots
            .into_iter()
            .map(|width| MoxelFormat {
                width,
                ..MoxelFormat::default()
            })
            .collect::<Vec<_>>();
        let (column_formats, formats) = split_moxel_formats_for_output(
            formats,
            column_count,
            sparse_source_format_refs,
            drawing_format_indices,
        );
        return (column_formats, formats, false);
    }
    (Vec::new(), Vec::new(), false)
}

pub(super) fn parse_moxel_format_table(
    fields: &[&str],
    column_count: usize,
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<Vec<MoxelFormat>> {
    for index in 0..fields.len() {
        if let Some(formats) = parse_moxel_nested_format_table(
            fields[index],
            column_count,
            style_refs,
            drawing_format_indices,
            number_format_refs,
        ) {
            return Some(formats);
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
        let mut formats = Vec::with_capacity(count);
        for (format_offset, field) in fields[index + 1..=index + count].iter().enumerate() {
            let Some(mut format) = parse_moxel_format(field, style_refs, number_format_refs) else {
                formats.clear();
                break;
            };
            if drawing_format_indices.contains(&(format_offset + 1)) {
                let pattern_color = parse_moxel_drawing_pattern_color(field, style_refs);
                normalize_moxel_drawing_format_with_pattern_color(&mut format, pattern_color);
            }
            formats.push(format);
        }
        if formats.len() == count {
            return Some(formats);
        }
    }
    None
}

pub(super) fn parse_moxel_equal_width_only_format_table(
    fields: &[&str],
    column_count: usize,
) -> Option<(usize, Vec<Option<usize>>)> {
    if column_count == 0 {
        return None;
    }
    for index in 0..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count != column_count || index + count >= fields.len() {
            continue;
        }
        let mut saw_width = false;
        let mut slots = Vec::with_capacity(count);
        let mut valid = true;
        for field in &fields[index + 1..=index + count] {
            let trimmed = field.trim();
            if trimmed == "{0}" {
                slots.push(None);
                continue;
            }
            let Some(width) = parse_moxel_column_width(trimmed) else {
                valid = false;
                break;
            };
            saw_width = true;
            slots.push(Some(width));
        }
        if valid && saw_width {
            return Some((index, slots));
        }
    }
    None
}

pub(super) fn parse_moxel_nested_format_table(
    text: &str,
    column_count: usize,
    style_refs: &[Option<String>],
    drawing_format_indices: &BTreeSet<usize>,
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<Vec<MoxelFormat>> {
    let nested = split_1c_braced_fields(text, 0)?;
    let count = nested.first()?.trim().parse::<usize>().ok()?;
    if count <= column_count || count > 2048 || nested.len() != count + 1 {
        return None;
    }
    let mut formats = Vec::with_capacity(count);
    for (format_offset, field) in nested.iter().skip(1).enumerate() {
        let Some(mut format) = parse_moxel_format(field, style_refs, number_format_refs) else {
            return None;
        };
        if drawing_format_indices.contains(&(format_offset + 1)) {
            let pattern_color = parse_moxel_drawing_pattern_color(field, style_refs);
            normalize_moxel_drawing_format_with_pattern_color(&mut format, pattern_color);
        }
        formats.push(format);
    }
    Some(formats)
}

fn parse_moxel_drawing_pattern_color(text: &str, style_refs: &[Option<String>]) -> Option<String> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    parse_moxel_format_style_ref(&values, 13, style_refs)
}

#[cfg(test)]
pub(super) fn normalize_moxel_drawing_format(format: &mut MoxelFormat) {
    normalize_moxel_drawing_format_with_pattern_color(format, None);
}

fn normalize_moxel_drawing_format_with_pattern_color(
    format: &mut MoxelFormat,
    pattern_color: Option<String>,
) {
    format.drawing_border = format.left_border;
    format.left_border = None;
    if pattern_color.is_some() {
        format.text_orientation = None;
        format.pattern_color = pattern_color;
    }
    if format.back_color.is_none() {
        match format.border_color.as_deref() {
            Some("style:ToolTipBackColor") => {
                format.back_color = Some("style:FormBackColor".to_string());
                format.border_color = None;
            }
            Some(
                "style:FormBackColor" | "style:FieldBackColor" | "style:FieldSelectionBackColor",
            ) => {
                format.back_color = format.border_color.take();
            }
            _ => {}
        }
    }
    if format.back_color.as_deref() == Some("style:ToolTipBackColor") {
        format.back_color = Some("style:FormBackColor".to_string());
    }
}

pub(super) fn normalize_moxel_single_set_report_header_tail(
    column_sets: &[MoxelColumnSet],
    column_formats: &[MoxelFormat],
    style_refs: &[Option<String>],
    lines: &mut [MoxelLine],
    formats: &mut [MoxelFormat],
) {
    if column_sets.len() != 1
        || column_formats.len() != 8
        || style_refs.get(2).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
        || style_refs.get(3).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
        || style_refs.get(4).and_then(|slot| slot.as_deref()) != Some("style:ReportHeaderBackColor")
    {
        return;
    }
    if let Some(line) = lines.get_mut(1)
        && line.style == "Dotted"
        && line.width == 1
    {
        line.style = "Solid";
        line.width = 2;
    }
    for (offset, format) in formats.iter_mut().enumerate() {
        let format_index = column_formats.len() + offset + 1;
        if format.back_color.as_deref() == Some("style:ReportHeaderBackColor")
            && format.border_color.is_none()
            && format.text_placement == Some("Wrap")
            && format_index >= 48
        {
            format.back_color = Some("#F4ECC5".to_string());
        }
    }
}

pub(super) fn split_moxel_formats_by_source_refs(
    formats: Vec<MoxelFormat>,
    source_column_format_refs: &[usize],
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    let mut selected_refs = BTreeSet::new();
    let mut column_formats = Vec::new();
    for source_format_index in source_column_format_refs {
        if *source_format_index == 0
            || *source_format_index > formats.len()
            || !selected_refs.insert(*source_format_index)
        {
            continue;
        }
        column_formats.push(formats[*source_format_index - 1].clone());
    }
    let formats = formats
        .into_iter()
        .enumerate()
        .filter_map(|(index, format)| {
            let source_format_index = index + 1;
            if selected_refs.contains(&source_format_index) {
                None
            } else {
                Some(format)
            }
        })
        .collect::<Vec<_>>();
    (column_formats, formats)
}

pub(super) fn prefers_moxel_leading_source_column_formats(
    formats: &[MoxelFormat],
    source_column_format_refs: &[usize],
) -> bool {
    if source_column_format_refs.is_empty() || source_column_format_refs.len() >= formats.len() {
        return false;
    }
    if !source_column_format_refs
        .iter()
        .enumerate()
        .all(|(index, source_format_index)| *source_format_index == index + 1)
    {
        return false;
    }
    if !source_column_format_refs.iter().all(|source_format_index| {
        formats
            .get(source_format_index - 1)
            .is_some_and(is_moxel_width_only_format)
    }) {
        return false;
    }
    formats
        .iter()
        .skip(source_column_format_refs.len())
        .any(|format| !is_moxel_width_only_format(format))
}

pub(super) fn is_moxel_width_only_format(format: &MoxelFormat) -> bool {
    format.width.is_some()
        && format.height.is_none()
        && format.font.is_none()
        && format.border.is_none()
        && format.left_border.is_none()
        && format.top_border.is_none()
        && format.right_border.is_none()
        && format.bottom_border.is_none()
        && format.drawing_border.is_none()
        && format.border_color.is_none()
        && format.horizontal_alignment.is_none()
        && format.vertical_alignment.is_none()
        && format.text_color.is_none()
        && format.back_color.is_none()
        && format.pattern_color.is_none()
        && format.pattern.is_none()
        && format.text_placement.is_none()
        && format.text_orientation.is_none()
        && format.fill_type.is_none()
        && !format.number_format_present
        && format.number_format.is_empty()
        && !format.edit_format_present
        && format.edit_format.is_empty()
        && format.hyper_link.is_none()
        && format.protection.is_none()
        && format.hidden.is_none()
        && format.indent.is_none()
        && format.auto_indent.is_none()
        && format.mask.is_none()
        && format.pic_index.is_none()
        && format.picture_size_mode.is_none()
        && format.pic_horizontal_alignment.is_none()
        && format.pic_vertical_alignment.is_none()
        && format.text_position.is_none()
        && format.details_use.is_none()
        && format.by_selected_columns.is_none()
        && format.mark_negatives.is_none()
        && format.auto_mark_incomplete.is_none()
        && format.mark_incomplete.is_none()
        && format.column_size_change.is_none()
        && format.left_margin.is_none()
        && format.top_margin.is_none()
        && format.right_margin.is_none()
        && format.bottom_margin.is_none()
}

pub(super) fn split_moxel_formats_for_output(
    mut formats: Vec<MoxelFormat>,
    column_count: usize,
    sparse_source_format_refs: bool,
    drawing_format_indices: &BTreeSet<usize>,
) -> (Vec<MoxelFormat>, Vec<MoxelFormat>) {
    if sparse_source_format_refs {
        let trailing_drawing_count = (1..=formats.len())
            .rev()
            .take_while(|format_index| drawing_format_indices.contains(format_index))
            .count();
        let column_start = formats
            .len()
            .saturating_sub(trailing_drawing_count + column_count);
        let column_end = formats.len().saturating_sub(trailing_drawing_count);
        let trailing_formats = formats.split_off(column_end);
        let column_formats = formats.split_off(column_start);
        formats.extend(trailing_formats);
        return (column_formats, formats);
    }
    let trailing_drawing_count = (1..=formats.len())
        .rev()
        .take_while(|format_index| drawing_format_indices.contains(format_index))
        .count();
    let column_start = formats
        .len()
        .saturating_sub(trailing_drawing_count + column_count);
    let column_end = formats.len().saturating_sub(trailing_drawing_count);
    let trailing_formats = formats.split_off(column_end);
    let column_formats = formats.split_off(column_start);
    formats.extend(trailing_formats);
    (column_formats, formats)
}

pub(super) fn parse_moxel_number_format_refs(
    fields: &[&str],
    column_count: usize,
    style_refs: &[Option<String>],
    _drawing_format_indices: &BTreeSet<usize>,
) -> Vec<Vec<MoxelLocalizedValue>> {
    let mut required_count = 0usize;
    let mut start = 0usize;
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
                && nested
                    .iter()
                    .skip(1)
                    .all(|field| parse_moxel_format(field, style_refs, &[]).is_some())
            {
                required_count = nested
                    .iter()
                    .skip(1)
                    .map(|field| parse_moxel_format_localized_value_required_count(field))
                    .max()
                    .unwrap_or(0);
                start = index + 1;
                break;
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
            .all(|field| parse_moxel_format(field, style_refs, &[]).is_some())
        {
            required_count = format_fields
                .iter()
                .map(|field| parse_moxel_format_localized_value_required_count(field))
                .max()
                .unwrap_or(0);
            start = index + count + 1;
            break;
        }
    }
    if required_count == 0 {
        return Vec::new();
    }
    for index in start..fields.len() {
        let Some(count) = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if count < required_count || count > 1024 || index + count >= fields.len() {
            continue;
        }
        let formats = fields[index + 1..=index + count]
            .iter()
            .map(|field| parse_moxel_localized_values(field))
            .collect::<Option<Vec<_>>>();
        if let Some(formats) = formats {
            return formats;
        }
    }
    Vec::new()
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
    let drawings = parse_moxel_drawings(&fields);
    let drawing_format_indices = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .collect::<BTreeSet<_>>();
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

pub(super) fn parse_moxel_format_number_format_index(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    parse_moxel_format_usize(&values, 24)
}

pub(super) fn parse_moxel_format_edit_format_index(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    parse_moxel_format_usize(&values, 32)
}

pub(super) fn parse_moxel_format_localized_value_required_count(text: &str) -> usize {
    [
        parse_moxel_format_number_format_index(text),
        parse_moxel_format_edit_format_index(text),
    ]
    .into_iter()
    .flatten()
    .max()
    .map(|index| index + 1)
    .unwrap_or(0)
}

pub(super) fn parse_moxel_localized_values(text: &str) -> Option<Vec<MoxelLocalizedValue>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != count + 2 {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|field| {
            let pair = split_1c_braced_fields(field, 0)?;
            if pair.len() != 2 {
                return None;
            }
            Some(MoxelLocalizedValue {
                lang: parse_1c_string(pair.first()?)?,
                content: parse_1c_string(pair.get(1)?)?,
            })
        })
        .collect()
}

pub(super) fn parse_moxel_format(
    text: &str,
    style_refs: &[Option<String>],
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<MoxelFormat> {
    let fields = split_1c_braced_fields(text, 0)?;
    let flags = fields.first()?.trim().parse::<u64>().ok()?;
    let values = moxel_format_values(flags, &fields)?;
    let explicit_pattern = values[12].is_some();
    let left_border = parse_moxel_format_usize(&values, 1);
    let top_border = parse_moxel_format_usize(&values, 2);
    let right_border = parse_moxel_format_usize(&values, 3);
    let bottom_border = parse_moxel_format_usize(&values, 4);
    let border = match (left_border, top_border, right_border, bottom_border) {
        (Some(left), Some(top), Some(right), Some(bottom))
            if left == top && top == right && right == bottom =>
        {
            Some(left)
        }
        _ => None,
    };
    let mut format = MoxelFormat {
        font: parse_moxel_format_usize(&values, 0),
        border,
        left_border: if border.is_some() { None } else { left_border },
        top_border: if border.is_some() { None } else { top_border },
        right_border: if border.is_some() { None } else { right_border },
        bottom_border: if border.is_some() {
            None
        } else {
            bottom_border
        },
        height: parse_moxel_format_i32(&values, 6),
        border_color: parse_moxel_format_style_ref(&values, 5, style_refs),
        width: parse_moxel_format_usize(&values, 7),
        width_weight_factor: parse_moxel_format_usize(&values, 41),
        horizontal_alignment: parse_moxel_format_usize(&values, 8)
            .and_then(moxel_horizontal_alignment),
        vertical_alignment: parse_moxel_format_usize(&values, 9).and_then(moxel_vertical_alignment),
        back_color: parse_moxel_format_style_ref(&values, 11, style_refs),
        pattern_color: None,
        pattern: parse_moxel_format_usize(&values, 12).and_then(moxel_format_pattern),
        text_color: parse_moxel_format_style_ref(&values, 10, style_refs),
        text_placement: parse_moxel_format_usize(&values, 14).and_then(moxel_text_placement),
        text_orientation: parse_moxel_format_usize(&values, 13)
            .or_else(|| parse_moxel_format_usize(&values, 18).and_then(moxel_explicit_zero)),
        fill_type: parse_moxel_format_usize(&values, 15).and_then(moxel_fill_type),
        number_format_present: values[24].is_some(),
        number_format: parse_moxel_format_usize(&values, 24)
            .and_then(|index| number_format_refs.get(index))
            .cloned()
            .unwrap_or_default(),
        edit_format_present: values[32].is_some(),
        edit_format: parse_moxel_format_usize(&values, 32)
            .and_then(|index| number_format_refs.get(index))
            .cloned()
            .unwrap_or_default(),
        drawing_border: None,
        by_selected_columns: parse_moxel_format_usize(&values, 20)
            .and_then(moxel_by_selected_columns),
        details_use: parse_moxel_format_usize(&values, 19).and_then(moxel_details_use),
        mark_negatives: parse_moxel_format_usize(&values, 21).and_then(moxel_bool_value),
        hyper_link: parse_moxel_format_usize(&values, 26).and_then(moxel_hyper_link),
        auto_mark_incomplete: parse_moxel_format_usize(&values, 28).and_then(moxel_bool_value),
        mark_incomplete: parse_moxel_format_usize(&values, 29).and_then(moxel_false_only),
        protection: parse_moxel_format_usize(&values, 16).and_then(moxel_protection),
        hidden: parse_moxel_format_usize(&values, 17).and_then(moxel_hidden),
        indent: parse_moxel_format_usize(&values, 30),
        auto_indent: parse_moxel_format_usize(&values, 31),
        column_size_change: parse_moxel_format_usize(&values, 33)
            .and_then(moxel_column_size_change),
        mask: parse_moxel_format_usize(&values, 34).and_then(moxel_mask),
        pic_index: parse_moxel_format_usize(&values, 35),
        pic_horizontal_alignment: parse_moxel_format_usize(&values, 36)
            .and_then(moxel_picture_horizontal_alignment),
        pic_vertical_alignment: parse_moxel_format_usize(&values, 37)
            .and_then(moxel_picture_vertical_alignment),
        picture_size_mode: parse_moxel_format_usize(&values, 38).and_then(moxel_picture_size_mode),
        text_position: parse_moxel_format_usize(&values, 39).and_then(moxel_text_position),
        left_margin: parse_moxel_format_usize(&values, 42).and_then(moxel_explicit_zero),
        top_margin: parse_moxel_format_usize(&values, 43).and_then(moxel_explicit_zero),
        right_margin: parse_moxel_format_usize(&values, 44).and_then(moxel_explicit_zero),
        bottom_margin: parse_moxel_format_usize(&values, 45).and_then(moxel_explicit_zero),
    };
    if !explicit_pattern
        && format.pattern.is_none()
        && format.back_color.is_some()
        && format.border_color.is_some()
        && matches!(format.text_placement, Some("Auto"))
    {
        format.pattern = Some("Solid");
    }
    Some(format)
}

pub(super) fn moxel_format_values<'a>(
    flags: u64,
    fields: &[&'a str],
) -> Option<[Option<&'a str>; 64]> {
    let mut values = [None; 64];
    if flags == 0 {
        return (fields.len() == 1).then_some(values);
    }
    let mut field_index = 1usize;
    for (bit, value) in values.iter_mut().enumerate() {
        if flags & (1u64 << bit) == 0 {
            continue;
        }
        let field = *fields.get(field_index)?;
        if moxel_format_bit_is_supported(bit) {
            *value = Some(field);
        }
        field_index += 1;
    }
    (field_index == fields.len()).then_some(values)
}

pub(super) fn moxel_format_bit_is_supported(bit: usize) -> bool {
    matches!(
        bit,
        0 | 1
            | 2
            | 3
            | 4
            | 5
            | 6
            | 7
            | 8
            | 9
            | 10
            | 11
            | 12
            | 13
            | 14
            | 15
            | 16
            | 17
            | 18
            | 19
            | 20
            | 21
            | 24
            | 26
            | 28
            | 29
            | 30
            | 31
            | 32
            | 33
            | 34
            | 35
            | 36
            | 37
            | 38
            | 39
            | 41
            | 42
            | 43
            | 44
            | 45
    )
}

pub(super) fn parse_moxel_format_usize(values: &[Option<&str>; 64], bit: usize) -> Option<usize> {
    values
        .get(bit)
        .and_then(|value| value.and_then(|value| value.trim().parse::<usize>().ok()))
}

pub(super) fn parse_moxel_format_i32(values: &[Option<&str>; 64], bit: usize) -> Option<i32> {
    values
        .get(bit)
        .and_then(|value| value.and_then(|value| value.trim().parse::<i32>().ok()))
}

pub(super) fn parse_moxel_format_style_ref(
    values: &[Option<&str>; 64],
    bit: usize,
    style_refs: &[Option<String>],
) -> Option<String> {
    let raw_index = parse_moxel_format_usize(values, bit)?;
    let style_ref_index = remap_moxel_format_style_ref_index(style_refs, raw_index);
    style_refs
        .get(style_ref_index)
        .cloned()
        .flatten()
        .and_then(|style_ref| resolve_moxel_format_style_ref(&style_ref, bit))
        .or_else(|| resolve_moxel_compact_style_ref_index(raw_index, bit))
}

pub(super) fn remap_moxel_format_style_ref_index(
    style_refs: &[Option<String>],
    raw_index: usize,
) -> usize {
    if raw_index == 0 || style_refs.len() < 5 {
        return raw_index;
    }
    let has_gray_embedded_prefix = raw_index == 2
        && style_refs.first().and_then(|slot| slot.as_deref()) == Some("moxel:f527:0:1")
        && style_refs.get(1).and_then(|slot| slot.as_deref()) == Some("moxel:f527:6:1")
        && style_refs.get(2).is_some_and(Option::is_none)
        && style_refs.get(3).and_then(|slot| slot.as_deref()) == Some("style:FormBackColor")
        && style_refs.get(4).and_then(|slot| slot.as_deref()) == Some("style:FormTextColor")
        && style_refs.get(5).and_then(|slot| slot.as_deref()) == Some("d3p1:Gray");
    if has_gray_embedded_prefix {
        return raw_index + 3;
    }
    let has_embedded_prefix = (style_refs[0].as_deref() == Some("moxel:f527:1:1")
        && style_refs[1].as_deref() == Some("moxel:f527:1:2")
        && style_refs[2].as_deref() == Some("moxel:f527:1:3"))
        || (style_refs[0].as_deref() == Some("moxel:f527:1:1")
            && style_refs[1].as_deref() == Some("moxel:f527:0:1"));
    if has_embedded_prefix
        && style_refs[3].as_deref() == Some("style:FormBackColor")
        && style_refs[4].as_deref() == Some("style:FormTextColor")
    {
        return raw_index + 3;
    }
    raw_index
}

pub(super) fn resolve_moxel_format_style_ref(style_ref: &str, bit: usize) -> Option<String> {
    if let Some((family, kind)) = parse_moxel_f527_style_ref(style_ref) {
        return match (bit, family, kind) {
            (11, "0", "1") | (5, "0", "1") => Some("style:ToolTipBackColor".to_string()),
            (10, "0", "1") => Some("style:ToolTipTextColor".to_string()),
            (11, "1", "1") | (5, "1", "1") => Some("style:FormBackColor".to_string()),
            (10, "1", "1") => Some("style:FormTextColor".to_string()),
            (11, "1", "2") | (5, "1", "2") => Some("style:FieldBackColor".to_string()),
            (10, "1", "2") => Some("style:FieldTextColor".to_string()),
            (11, "1", "3") | (10, "1", "3") | (5, "1", "3") => {
                Some("style:FieldSelectionBackColor".to_string())
            }
            _ => None,
        };
    }
    Some(style_ref.to_string())
}

pub(super) fn resolve_moxel_compact_style_ref_index(
    raw_index: usize,
    bit: usize,
) -> Option<String> {
    match (bit, raw_index) {
        (11 | 5, 0) => Some("style:ToolTipBackColor".to_string()),
        (10, 0) => Some("style:ToolTipTextColor".to_string()),
        (11 | 5, 1) => Some("style:FormBackColor".to_string()),
        (10, 1) => Some("style:FormTextColor".to_string()),
        (11 | 5, 2) => Some("style:FieldBackColor".to_string()),
        (10, 2) => Some("style:FieldTextColor".to_string()),
        _ => None,
    }
}

pub(super) fn parse_moxel_f527_style_ref(style_ref: &str) -> Option<(&str, &str)> {
    let suffix = style_ref.strip_prefix("moxel:f527:")?;
    let (family, kind) = suffix.split_once(':')?;
    Some((family, kind))
}

pub(super) fn parse_moxel_style_refs(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let mut style_refs: Vec<Option<String>> = Vec::new();
    let mut index = 0usize;
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
    };
    while index < fields.len() {
        let has_style_prefix = style_refs.len() >= 2
            && style_refs[style_refs.len() - 2].as_deref() == Some("style:FormBackColor")
            && style_refs[style_refs.len() - 1].as_deref() == Some("style:FormTextColor");
        if has_style_prefix
            && let Some(end) = index.checked_add(3)
            && let Some(slots) = fields.get(index..end)
            && is_moxel_direct_report_palette(slots)
            && let Some(white_ref) = parse_moxel_style_ref_slot(slots[2], object_refs)
        {
            style_refs.push(Some("#CCC085".to_string()));
            style_refs.push(Some("#F4ECC5".to_string()));
            style_refs.push(white_ref);
            index = end;
            continue;
        }
        if normalize(fields[index]) == "{1,3,{3,3,{-28}}}" {
            index += 1;
            continue;
        }
        let field = fields[index];
        if let Some(style_ref) = parse_moxel_style_ref_slot(field, object_refs) {
            style_refs.push(style_ref);
            index += 1;
            continue;
        }
        if let Some(overrides) = parse_moxel_indexed_style_ref_overrides(field, object_refs) {
            for (slot_index, style_ref) in overrides {
                if style_refs.len() <= slot_index {
                    style_refs.resize(slot_index + 1, None);
                }
                style_refs[slot_index] = style_ref;
            }
            index += 1;
            continue;
        }
        let wrapped_style_refs = parse_moxel_wrapped_style_refs(field, object_refs);
        if !wrapped_style_refs.is_empty() {
            style_refs.extend(wrapped_style_refs);
            index += 1;
            continue;
        }
        style_refs.extend(parse_moxel_embedded_style_refs(field, object_refs));
        index += 1;
    }
    if style_refs.len() >= 5
        && style_refs.first().is_some_and(Option::is_none)
        && style_refs.get(1).is_some_and(Option::is_none)
        && style_refs.get(2).and_then(|slot| slot.as_deref()) == Some("style:ReportLineColor")
        && style_refs.get(4).and_then(|slot| slot.as_deref()) == Some("auto")
    {
        style_refs[1] = Some("style:FormTextColor".to_string());
    }
    style_refs
}

fn is_moxel_direct_report_palette(slots: &[&str]) -> bool {
    slots.len() == 3
        && parse_moxel_raw_color_slot(slots[0]) == Some((0, 8765644))
        && parse_moxel_raw_color_slot(slots[1]) == Some((0, 12971252))
        && parse_moxel_raw_color_slot(slots[2]) == Some((2, 143))
}

fn parse_moxel_raw_color_slot(text: &str) -> Option<(u32, u32)> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let mode = fields.get(1)?.trim().parse::<u32>().ok()?;
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    if payload.len() != 1 {
        return None;
    }
    let value = payload.first()?.trim().parse::<u32>().ok()?;
    Some((mode, value))
}

pub(super) fn parse_moxel_indexed_style_ref_overrides(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<(usize, Option<String>)>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 5 || fields.first()?.trim() != "3" || fields.get(1)?.trim() != "2" {
        return None;
    }
    let mut overrides = Vec::new();
    let mut cursor = 3usize;
    while cursor + 1 < fields.len() {
        let slot_index = fields.get(cursor)?.trim().parse::<usize>().ok()?;
        let style_ref = parse_moxel_style_ref_slot(fields.get(cursor + 1)?, object_refs)?;
        overrides.push((slot_index, style_ref));
        cursor += 2;
    }
    (!overrides.is_empty()).then_some(overrides)
}

pub(super) fn parse_moxel_wrapped_style_refs(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    if fields.len() < 3 || fields.first().map(|field| field.trim()) != Some("1") {
        return Vec::new();
    }
    let mut refs = Vec::new();
    for field in fields.iter().skip(2) {
        if let Some(style_ref) = parse_moxel_style_ref_slot(field, object_refs) {
            refs.push(style_ref);
            continue;
        }
        let nested = parse_moxel_embedded_style_refs(field, object_refs);
        if nested.is_empty() {
            return Vec::new();
        }
        refs.extend(nested);
    }
    refs
}

pub(super) fn parse_moxel_empty_headers_footers(fields: &[&str]) -> bool {
    fields.windows(6).any(|window| {
        window
            .iter()
            .all(|field| parse_moxel_empty_header_footer(field))
    })
}

pub(super) fn parse_moxel_empty_header_footer(text: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return false;
    };
    if fields.len() != 5 || fields.first().map(|field| field.trim()) != Some("16") {
        return false;
    }
    if fields.get(1).map(|field| field.trim()) != Some("0")
        || fields.get(3).map(|field| field.trim()) != Some("1")
    {
        return false;
    }
    let Some(text_fields) = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return false;
    };
    let Some(format_fields) = fields
        .get(4)
        .and_then(|field| split_1c_braced_fields(field, 0))
    else {
        return false;
    };
    text_fields.len() == 2
        && text_fields.first().map(|field| field.trim()) == Some("1")
        && text_fields.get(1).map(|field| field.trim()) == Some("0")
        && format_fields.len() == 3
        && format_fields.first().map(|field| field.trim()) == Some("1")
        && format_fields.get(2).map(|field| field.trim()) == Some("1")
        && format_fields.get(1).and_then(|field| {
            let nested = split_1c_braced_fields(field, 0)?;
            Some(
                nested.len() == 2
                    && nested.first().map(|value| value.trim()) == Some("1")
                    && nested.get(1).map(|value| value.trim()) == Some("0"),
            )
        }) == Some(true)
}

pub(super) fn parse_moxel_style_ref_slot(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    match fields.get(1)?.trim() {
        "3" => match payload.first()?.trim() {
            "-1" => Some(Some("style:FormBackColor".to_string())),
            "-3" => Some(Some("style:FormTextColor".to_string())),
            "-10" => Some(Some("style:FieldBackColor".to_string())),
            "-11" => Some(Some("style:FieldTextColor".to_string())),
            "-13" => Some(Some("style:FieldTextColor".to_string())),
            "-14" => Some(Some("style:FieldSelectionBackColor".to_string())),
            "-16" => Some(Some("style:SpecialTextColor".to_string())),
            "-17" => Some(Some("style:NegativeTextColor".to_string())),
            "-21" => Some(Some("style:FieldSelectionBackColor".to_string())),
            "-23" => Some(Some("style:ToolTipBackColor".to_string())),
            "-24" => Some(Some("style:ToolTipTextColor".to_string())),
            "-7" => Some(Some("style:ButtonBackColor".to_string())),
            "-15" => Some(Some("style:ButtonTextColor".to_string())),
            "-22" => Some(Some("style:BorderColor".to_string())),
            "-25" => Some(Some("style:ReportHeaderBackColor".to_string())),
            "-26" => Some(Some("style:ReportGroup1BackColor".to_string())),
            "-27" => Some(Some("style:ReportGroup2BackColor".to_string())),
            "-28" => Some(Some("style:ReportLineColor".to_string())),
            "-34" => Some(Some("style:ButtonBorderColor".to_string())),
            "-35" => Some(Some("style:TableHeaderBackColor".to_string())),
            "-36" => Some(Some("style:TableHeaderTextColor".to_string())),
            "-37" => Some(Some("style:TableFooterBackColor".to_string())),
            "-38" => Some(Some("style:TableFooterTextColor".to_string())),
            "-42" => Some(Some("style:NavigationColor".to_string())),
            "-43" => Some(Some("style:AuxiliaryNavigationColor".to_string())),
            "-44" => Some(Some("style:ActivityColor".to_string())),
            "0" => {
                let uuid = parse_uuid_field(payload.get(1)?.trim())?;
                Some(moxel_style_ref_for_uuid(&uuid, object_refs))
            }
            _ => None,
        },
        "4" => match payload.first()?.trim() {
            "0" => Some(Some("auto".to_string())),
            _ => None,
        },
        "2" => payload
            .first()
            .and_then(|value| parse_moxel_web_color(value.trim()))
            .map(Some),
        "0" => payload
            .first()
            .and_then(|value| parse_moxel_style_color(value.trim()))
            .map(Some),
        _ => None,
    }
}

pub(super) fn parse_moxel_embedded_style_refs(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Vec<Option<String>> {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    if fields.len() < 3
        || fields.get(1).map(|field| field.trim()) != Some("1")
        || !matches!(fields.first().map(|field| field.trim()), Some("3"))
    {
        return Vec::new();
    }
    let container_kind = fields.first().map(|field| field.trim());
    if fields
        .get(2)
        .and_then(|field| parse_moxel_embedded_style_ref(field, container_kind, object_refs))
        .is_none()
    {
        return Vec::new();
    }
    let mut refs = fields
        .iter()
        .skip(2)
        .filter_map(|field| parse_moxel_embedded_style_ref(field, container_kind, object_refs))
        .collect::<Vec<_>>();
    if moxel_uses_sparse_f527_embedded_slots(&fields, &refs) {
        refs = vec![
            refs[0].clone(),
            None,
            refs[1].clone(),
            None,
            refs[2].clone(),
        ];
    }
    refs
}

pub(super) fn moxel_uses_sparse_f527_embedded_slots(
    fields: &[&str],
    refs: &[Option<String>],
) -> bool {
    let sparse_wrapper = fields.len() == 10
        && fields[3].trim() == "0"
        && fields[4].trim() == "1"
        && fields[6].trim() == "0"
        && fields[7].trim() == "1"
        && fields[9].trim() == "0";
    if !sparse_wrapper || refs.len() != 3 {
        return false;
    }
    matches!(
        (refs[0].as_deref(), refs[1].as_deref(), refs[2].as_deref(),),
        (
            Some("moxel:f527:0:1"),
            Some("moxel:f527:1:3"),
            Some("moxel:f527:1:1"),
        )
    )
}

pub(super) fn parse_moxel_embedded_style_ref(
    text: &str,
    container_kind: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 8 || fields.first()?.trim() != "4" || fields.get(1)?.trim() != "0" {
        return None;
    }
    let uuid = parse_uuid_field(fields.get(6)?.trim())?;
    Some(moxel_embedded_style_ref_for_uuid(
        &uuid,
        container_kind,
        fields.get(3).map(|field| field.trim()),
        fields.get(4).map(|field| field.trim()),
        object_refs,
    ))
}

pub(super) fn moxel_style_ref_for_uuid(
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    match uuid {
        "f527dc88-1d39-40b3-bcbb-d98b690ead68" => Some("style:FormBackColor".to_string()),
        _ => object_refs
            .get(uuid)
            .and_then(|reference| reference.strip_prefix("StyleItem."))
            .map(|name| format!("style:{name}")),
    }
}

pub(super) fn moxel_embedded_style_ref_for_uuid(
    uuid: &str,
    container_kind: Option<&str>,
    family: Option<&str>,
    kind: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    match (uuid, container_kind, family, kind) {
        ("f527dc88-1d39-40b3-bcbb-d98b690ead68", _, Some(family), Some(kind)) => {
            Some(format!("moxel:f527:{family}:{kind}"))
        }
        _ => moxel_style_ref_for_uuid(uuid, object_refs),
    }
}

pub(super) fn parse_moxel_web_color(value: &str) -> Option<String> {
    let name = match value.parse::<u32>().ok()? {
        8 => "Black",
        10 => "Blue",
        20 => "Cream",
        21 => "Crimson",
        23 => "DarkBlue",
        27 | 31 => "DarkGreen",
        33 => "DarkRed",
        37 => "DarkSlateGray",
        44 => "FireBrick",
        45 => "FloralWhite",
        46 => "ForestGreen",
        48 => "Gainsboro",
        52 => "Gray",
        53 => "Green",
        55 => "HoneyDew",
        64 => "LemonChiffon",
        67 => "LightCyan",
        68 => "LightGoldenRod",
        69 => "LightGoldenRodYellow",
        71 => "LightGray",
        72 => "LightPink",
        79 => "LightYellow",
        84 => "Maroon",
        97 => "MintCream",
        98 => "MistyRose",
        108 => "PaleGoldenrod",
        119 => "Red",
        120 => "RosyBrown",
        121 => "RoyalBlue",
        128 => "Silver",
        130 => "SlateBlue",
        134 => "SteelBlue",
        140 => "Violet",
        141 => "VioletRed",
        143 => "White",
        144 => "WhiteSmoke",
        145 => "Yellow",
        _ => return None,
    };
    Some(format!("d3p1:{name}"))
}

pub(super) fn parse_moxel_style_color(value: &str) -> Option<String> {
    match value.parse::<u32>().ok()? {
        12971252 => Some("style:ReportHeaderBackColor".to_string()),
        8765644 => Some("style:ReportLineColor".to_string()),
        _ => parse_moxel_direct_color(value),
    }
}

pub(super) fn parse_moxel_direct_color(value: &str) -> Option<String> {
    let color = value.parse::<u32>().ok()?;
    let red = color & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = (color >> 16) & 0xff;
    Some(format!("#{red:02X}{green:02X}{blue:02X}"))
}

pub(super) fn moxel_horizontal_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        2 => Some("Right"),
        4 => Some("Justify"),
        5 => Some("Auto"),
        6 => Some("Center"),
        7 => Some("Right"),
        _ => None,
    }
}

pub(super) fn moxel_vertical_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Top"),
        4 | 24 => Some("Center"),
        8 | 48 => Some("Bottom"),
        _ => None,
    }
}

pub(super) fn moxel_text_placement(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Auto"),
        1 => Some("Cut"),
        2 => Some("Block"),
        3 => Some("Wrap"),
        _ => None,
    }
}

pub(super) fn moxel_format_pattern(value: usize) -> Option<&'static str> {
    const PATTERNS: [&str; 19] = [
        "Solid",
        "Pattern1",
        "Pattern2",
        "Pattern3",
        "Pattern4",
        "Pattern5",
        "Pattern6",
        "Pattern7",
        "Pattern8",
        "Pattern9",
        "Pattern10",
        "Pattern11",
        "Pattern12",
        "Pattern13",
        "Pattern14",
        "Pattern15",
        "Pattern16",
        "Pattern17",
        "Pattern18",
    ];
    if value == 255 {
        Some("WithoutPattern")
    } else {
        PATTERNS.get(value).copied()
    }
}

pub(super) fn moxel_explicit_zero(value: usize) -> Option<usize> {
    (value == 0).then_some(0)
}

pub(super) fn moxel_false_only(value: usize) -> Option<bool> {
    (value == 0).then_some(false)
}

pub(super) fn moxel_bool_value(value: usize) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub(super) fn moxel_column_size_change(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Normal"),
        1 => Some("QuickChange"),
        _ => None,
    }
}

pub(super) fn moxel_page_orientation(value: usize) -> Option<&'static str> {
    match value {
        1 => Some("Portrait"),
        2 => Some("Landscape"),
        _ => None,
    }
}

pub(super) fn moxel_duplex_type(value: usize) -> Option<&'static str> {
    match value {
        1 => Some("None"),
        4 => Some("UsePrinterSettings"),
        _ => None,
    }
}

pub(super) fn moxel_page_placement_alternation(value: usize) -> Option<&'static str> {
    (value == 0).then_some("Auto")
}

pub(super) fn moxel_fill_type(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Text"),
        1 => Some("Parameter"),
        2 => Some("Template"),
        _ => None,
    }
}

pub(super) fn moxel_details_use(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Cell"),
        1 => Some("Row"),
        2 => Some("WithoutProcessing"),
        _ => None,
    }
}

pub(super) fn moxel_by_selected_columns(value: usize) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub(super) fn moxel_mask(value: usize) -> Option<&'static str> {
    match value {
        0 => Some(""),
        _ => None,
    }
}

pub(super) fn moxel_protection(value: usize) -> Option<bool> {
    match value {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

pub(super) fn moxel_hidden(value: usize) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

pub(super) fn moxel_hyper_link(value: usize) -> Option<bool> {
    match value {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

pub(super) fn moxel_picture_size_mode(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("RealSize"),
        1 => Some("Stretch"),
        2 => Some("Proportionally"),
        4 => Some("AutoSize"),
        7 => Some("ByFontSize"),
        _ => None,
    }
}

pub(super) fn moxel_picture_horizontal_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        2 => Some("Right"),
        5 => Some("Auto"),
        6 => Some("Center"),
        _ => None,
    }
}

pub(super) fn moxel_picture_vertical_alignment(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Top"),
        8 => Some("Bottom"),
        24 => Some("Center"),
        _ => None,
    }
}

pub(super) fn moxel_text_position(value: usize) -> Option<&'static str> {
    match value {
        0 => Some("Left"),
        1 => Some("Right"),
        5 => Some("Auto"),
        _ => None,
    }
}

pub(super) fn parse_moxel_column_width(text: &str) -> Option<usize> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 2 || fields.first()?.trim() != "128" {
        return None;
    }
    fields.get(1)?.trim().parse::<usize>().ok()
}

pub(super) fn parse_moxel_line(text: &str) -> Option<MoxelLine> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "3" || fields.get(1)?.trim() != "3" {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(2)?, 0)?;
    let style = match payload.first()?.trim() {
        "-1" => "None",
        "-3" => "Solid",
        "-10" => "Dotted",
        "-11" => "Dotted",
        _ => return None,
    };
    Some(MoxelLine {
        style,
        line_type: "v8ui:SpreadsheetDocumentCellLineType",
        width: 1,
    })
}

pub(super) fn parse_moxel_merge_regions(
    fields: &[&str],
) -> (Vec<MoxelMerge>, Vec<MoxelMerge>, Vec<MoxelMerge>) {
    let mut merges = Vec::new();
    let mut horizontal_unmerges = Vec::new();
    let mut vertical_unmerges = Vec::new();
    for (field_merges, field_horizontal_unmerges, field_vertical_unmerges) in fields
        .iter()
        .filter_map(|field| parse_moxel_merge_region_list(field))
    {
        merges.extend(field_merges);
        horizontal_unmerges.extend(field_horizontal_unmerges);
        vertical_unmerges.extend(field_vertical_unmerges);
    }
    normalize_moxel_merge_order(&mut merges);
    (merges, horizontal_unmerges, vertical_unmerges)
}

pub(super) fn normalize_moxel_merge_order(merges: &mut Vec<MoxelMerge>) {
    if merges.len() < 2 {
        return;
    }
    let mut ordered = Vec::with_capacity(merges.len());
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row >= 0 && merge.column >= 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row < 0 && merge.column >= 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row >= 0 && merge.column < 0)
            .cloned(),
    );
    ordered.extend(
        merges
            .iter()
            .filter(|merge| merge.row < 0 && merge.column < 0)
            .cloned(),
    );
    if ordered.len() == merges.len() {
        *merges = ordered;
    }
}

pub(super) fn parse_moxel_merge_region_list(
    text: &str,
) -> Option<(Vec<MoxelMerge>, Vec<MoxelMerge>, Vec<MoxelMerge>)> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 4096 || fields.len() != count + 1 {
        return None;
    }
    let mut merges = Vec::with_capacity(count);
    let mut horizontal_unmerges = Vec::new();
    let mut vertical_unmerges = Vec::new();
    for field in fields.iter().skip(1) {
        let (merge, kind) = parse_moxel_merge_region(field)?;
        match kind {
            0 => merges.push(merge),
            1 => horizontal_unmerges.push(merge),
            2 => vertical_unmerges.push(merge),
            _ => return None,
        }
    }
    Some((merges, horizontal_unmerges, vertical_unmerges))
}

pub(super) fn parse_moxel_merge_region(text: &str) -> Option<(MoxelMerge, usize)> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() < 4 {
        return None;
    }
    let begin_column = fields.first()?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(1)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    if end_row < begin_row || end_column < begin_column {
        return None;
    }
    let kind = fields
        .get(4)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let columns_id = fields
        .get(5)
        .and_then(|field| parse_non_zero_uuid(field.trim()));
    Some((
        MoxelMerge {
            row: begin_row,
            column: begin_column,
            height: end_row - begin_row,
            width: end_column - begin_column,
            columns_id,
        },
        kind,
    ))
}

#[allow(dead_code)]
pub(super) fn parse_moxel_area_list(text: &str) -> Option<Vec<MoxelArea>> {
    let items = parse_moxel_named_item_list(text)?;
    let areas = items
        .into_iter()
        .filter_map(|item| match item {
            MoxelNamedItem::Cells(area) => Some(area),
            MoxelNamedItem::Drawing { .. } => None,
        })
        .collect::<Vec<_>>();
    (!areas.is_empty()).then_some(areas)
}

pub(super) fn parse_moxel_named_item_list(text: &str) -> Option<Vec<MoxelNamedItem>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > 512 || fields.len() != count * 2 + 1 {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let name = parse_1c_string(fields.get(index * 2 + 1)?)?;
        if let Some(item) = parse_moxel_named_item(fields.get(index * 2 + 2)?, name) {
            items.push(item);
        }
    }
    (!items.is_empty()).then_some(items)
}

pub(super) fn parse_moxel_named_item(text: &str, name: String) -> Option<MoxelNamedItem> {
    let fields = split_1c_braced_fields(text, 0)?;
    match fields.first()?.trim() {
        "1" => {
            let bounds = split_1c_braced_fields(fields.get(1)?, 0)?;
            parse_moxel_bounds_area(&bounds, name).map(MoxelNamedItem::Cells)
        }
        "2" => Some(MoxelNamedItem::Drawing {
            name,
            drawing_id: fields.get(1)?.trim().parse::<usize>().ok()?,
        }),
        _ => None,
    }
}

#[allow(dead_code)]
pub(super) fn parse_moxel_area(text: &str, name: String) -> Option<MoxelArea> {
    match parse_moxel_named_item(text, name)? {
        MoxelNamedItem::Cells(area) => Some(area),
        MoxelNamedItem::Drawing { .. } => None,
    }
}

pub(super) fn parse_moxel_bounds_area(bounds: &[&str], name: String) -> Option<MoxelArea> {
    let area_type = match bounds.first()?.trim() {
        "1" => "Rows",
        "2" => "Columns",
        "3" => "Rectangle",
        _ => return None,
    };
    Some(MoxelArea {
        name,
        area_type,
        begin_column: bounds.get(1)?.trim().parse::<i32>().ok()?,
        begin_row: bounds.get(2)?.trim().parse::<i32>().ok()?,
        end_column: bounds.get(3)?.trim().parse::<i32>().ok()?,
        end_row: bounds.get(4)?.trim().parse::<i32>().ok()?,
        columns_id: bounds
            .get(5)
            .and_then(|value| parse_non_zero_uuid(value.trim())),
    })
}

pub(super) fn format_moxel_spreadsheet_xml(spreadsheet: &MoxelSpreadsheet) -> String {
    let output_format_indices = moxel_output_format_indices(spreadsheet);
    let output_format_index_map = moxel_output_format_index_map(&output_format_indices);
    let emit_first_row_format_index =
        moxel_column_format_slots(&spreadsheet.column_sets, spreadsheet.column_count) == 0;
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<languageSettings>\r\n\
\t\t<currentLanguage>ru</currentLanguage>\r\n\
\t\t<defaultLanguage>ru</defaultLanguage>\r\n\
\t\t<languageInfo>\r\n\
\t\t\t<id>ru</id>\r\n\
\t\t\t<code>Русский</code>\r\n\
\t\t\t<description>Русский</description>\r\n\
\t\t</languageInfo>\r\n\
\t</languageSettings>\r\n",
    );
    for column_set in &spreadsheet.column_sets {
        push_moxel_columns_xml(&mut xml, column_set, &output_format_index_map);
    }
    for row in &spreadsheet.rows {
        push_moxel_row_xml(
            &mut xml,
            row,
            &output_format_index_map,
            emit_first_row_format_index,
        );
    }
    for drawing in &spreadsheet.drawings {
        push_moxel_drawing_xml(&mut xml, drawing, &output_format_index_map);
    }
    if let Some(header_footer_format_index) = spreadsheet.header_footer_format_index {
        let header_footer_format_index = output_format_index_map
            .get(&header_footer_format_index)
            .copied()
            .unwrap_or(header_footer_format_index);
        push_moxel_header_footer_format_refs_xml(&mut xml, header_footer_format_index);
    } else if spreadsheet.empty_headers_footers {
        push_moxel_empty_headers_footers_xml(&mut xml);
    }
    xml.push_str("\t<templateMode>true</templateMode>\r\n");
    if let Some(default_format_index) = spreadsheet.default_format_index {
        let default_format_index = output_format_index_map
            .get(&default_format_index)
            .copied()
            .unwrap_or(default_format_index);
        xml.push_str(&format!(
            "\t<defaultFormatIndex>{default_format_index}</defaultFormatIndex>\r\n"
        ));
    }
    if spreadsheet.height > 0 || spreadsheet.column_count > 0 {
        xml.push_str(&format!("\t<height>{}</height>\r\n", spreadsheet.height));
    }
    if !spreadsheet.vertical_groups.is_empty() {
        let vg_levels = spreadsheet
            .vertical_groups
            .iter()
            .map(|group| group.level + 1)
            .max()
            .unwrap_or(0);
        if vg_levels > 0 {
            xml.push_str(&format!("\t<vgLevels>{vg_levels}</vgLevels>\r\n"));
        }
    }
    xml.push_str(&format!("\t<vgRows>{}</vgRows>\r\n", spreadsheet.height));
    for group in &spreadsheet.vertical_groups {
        push_moxel_vertical_group_xml(&mut xml, group);
    }
    for merge in &spreadsheet.merges {
        push_moxel_merge_xml(&mut xml, merge);
    }
    for vertical_unmerge in &spreadsheet.vertical_unmerges {
        push_moxel_vertical_unmerge_xml(&mut xml, vertical_unmerge);
    }
    for horizontal_unmerge in &spreadsheet.horizontal_unmerges {
        push_moxel_horizontal_unmerge_xml(&mut xml, horizontal_unmerge);
    }
    for named_item in &spreadsheet.named_items {
        push_moxel_named_item_xml(&mut xml, named_item);
    }
    if let Some(print_area) = &spreadsheet.print_area {
        push_moxel_print_area_xml(&mut xml, print_area);
    }
    if let Some(print_settings) = &spreadsheet.print_settings
        && !print_settings.is_default_margins_only()
    {
        push_moxel_print_settings_xml(&mut xml, print_settings);
    }
    for line in &spreadsheet.lines {
        push_moxel_line_xml(&mut xml, line);
    }
    for font in &spreadsheet.fonts {
        push_moxel_font_xml(&mut xml, font);
    }
    for format_index in output_format_indices {
        push_moxel_format_xml(&mut xml, spreadsheet, format_index);
    }
    for picture in &spreadsheet.pictures {
        push_moxel_picture_xml(&mut xml, picture);
    }
    xml.push_str("</document>");
    xml
}

pub(super) fn moxel_output_format_count(spreadsheet: &MoxelSpreadsheet) -> usize {
    let max_column_format_index = spreadsheet
        .column_sets
        .iter()
        .flat_map(|column_set| {
            column_set
                .columns
                .iter()
                .map(|column| column.format_index)
                .chain(column_set.default_format_index)
        })
        .max()
        .unwrap_or(0);
    let max_row_or_cell_format_index = spreadsheet.rows.iter().fold(0usize, |max_index, row| {
        let row_max = row.cells.iter().fold(row.format_index, |cell_max, cell| {
            cell_max.max(cell.format_index).max(
                cell.note
                    .as_ref()
                    .map(|note| note.format_index)
                    .unwrap_or(0),
            )
        });
        max_index.max(row_max)
    });
    let max_drawing_format_index = spreadsheet
        .drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    spreadsheet
        .default_format_index
        .unwrap_or(0)
        .max(spreadsheet.header_footer_format_index.unwrap_or(0))
        .max(spreadsheet.extra_formats.keys().copied().max().unwrap_or(0))
        .max(spreadsheet.column_formats.len() + spreadsheet.formats.len())
        .max(max_column_format_index)
        .max(max_row_or_cell_format_index)
        .max(max_drawing_format_index)
        .max(1)
}

pub(super) fn moxel_sparse_default_column_set_insertion_point(
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
) -> Option<usize> {
    if !spreadsheet
        .column_sets
        .iter()
        .skip(1)
        .any(|column_set| column_set.default_format_index == Some(format_index))
    {
        return None;
    }
    let default_set = spreadsheet.column_sets.first()?;
    let mut seen = BTreeSet::new();
    Some(
        default_set
            .columns
            .iter()
            .filter(|column| seen.insert(column.format_index))
            .count(),
    )
}

pub(super) fn moxel_sparse_source_output_order(
    spreadsheet: &MoxelSpreadsheet,
) -> Option<Vec<usize>> {
    let shared_default_format_index = spreadsheet.header_footer_format_index?;
    let selected_count = spreadsheet.column_formats.len();
    if selected_count == 0 {
        return None;
    }
    if spreadsheet.column_sets.len() == 1
        && shared_default_format_index > selected_count
        && spreadsheet
            .formats
            .get(shared_default_format_index - selected_count - 1)
            .is_some_and(MoxelFormat::is_empty)
        && spreadsheet
            .default_format_index
            .is_some_and(|index| index > shared_default_format_index)
    {
        let format_count = moxel_output_format_count(spreadsheet);
        let mut ordered = Vec::with_capacity(format_count);
        ordered.push(shared_default_format_index);
        for format_index in 1..=selected_count {
            ordered.push(format_index);
        }
        for format_index in (selected_count + 1)..=format_count {
            if format_index != shared_default_format_index {
                ordered.push(format_index);
            }
        }
        return Some(ordered);
    }
    if shared_default_format_index > selected_count
        && spreadsheet
            .column_sets
            .iter()
            .all(|column_set| column_set.default_format_index == Some(shared_default_format_index))
    {
        let format_count = moxel_output_format_count(spreadsheet);
        let mut ordered = Vec::with_capacity(format_count);
        ordered.push(shared_default_format_index);
        for format_index in 1..=selected_count {
            ordered.push(format_index);
        }
        for format_index in (selected_count + 1)..=format_count {
            if format_index != shared_default_format_index {
                ordered.push(format_index);
            }
        }
        return Some(ordered);
    }
    if spreadsheet.default_format_index.is_some() {
        return None;
    }
    if spreadsheet.column_sets.len() <= 1
        || !spreadsheet
            .column_sets
            .iter()
            .skip(1)
            .all(|column_set| column_set.default_format_index == Some(shared_default_format_index))
    {
        return None;
    }
    let default_set_selected_count = spreadsheet
        .column_sets
        .first()?
        .columns
        .iter()
        .map(|column| column.format_index)
        .collect::<BTreeSet<_>>()
        .len();
    let format_count = moxel_output_format_count(spreadsheet);
    let mut ordered = Vec::with_capacity(format_count);
    for format_index in 1..=default_set_selected_count.min(selected_count) {
        ordered.push(format_index);
    }
    if shared_default_format_index > 0 && shared_default_format_index <= format_count {
        ordered.push(shared_default_format_index);
    }
    for format_index in (default_set_selected_count + 1)..=selected_count {
        ordered.push(format_index);
    }
    for format_index in (selected_count + 1)..=format_count {
        if format_index != shared_default_format_index {
            ordered.push(format_index);
        }
    }
    Some(ordered)
}

pub(super) fn moxel_output_format_indices(spreadsheet: &MoxelSpreadsheet) -> Vec<usize> {
    let format_count = moxel_output_format_count(spreadsheet);
    if let Some(ordered) = spreadsheet
        .source_format_map
        .as_ref()
        .and_then(|source_format_map| source_format_map.output_internal_indices(format_count))
    {
        return ordered;
    }
    let source_column_format_offset = moxel_source_column_format_offset(&spreadsheet.column_sets);
    if (source_column_format_offset == 0 || spreadsheet.column_sets.len() > 1)
        && let Some(ordered) = moxel_sparse_source_output_order(spreadsheet)
    {
        return ordered;
    }
    if source_column_format_offset > 0 {
        let source_column_format_refs = moxel_source_column_format_refs(&spreadsheet.column_sets);
        if spreadsheet.column_formats.len() > source_column_format_refs.len() {
            let mut ordered = moxel_source_derived_internal_output_order(
                &spreadsheet.column_sets,
                spreadsheet.column_formats.len(),
                spreadsheet.formats.len(),
            );
            if spreadsheet.default_format_index.is_none()
                && let Some(extra_format_index) = spreadsheet.header_footer_format_index
                && let Some(insert_at) =
                    moxel_sparse_default_column_set_insertion_point(spreadsheet, extra_format_index)
            {
                if let Some(existing_pos) = ordered
                    .iter()
                    .position(|format_index| *format_index == extra_format_index)
                {
                    let format_index = ordered.remove(existing_pos);
                    ordered.insert(insert_at.min(ordered.len()), format_index);
                } else {
                    ordered.insert(insert_at.min(ordered.len()), extra_format_index);
                }
            }
            let mut seen_internal = ordered.iter().copied().collect::<BTreeSet<_>>();
            let mut push_internal = |format_index: usize| {
                if format_index > 0
                    && format_index <= format_count
                    && seen_internal.insert(format_index)
                {
                    ordered.push(format_index);
                }
            };
            for format_index in 1..=format_count {
                push_internal(format_index);
            }

            return ordered;
        }
        return (1..=format_count).collect();
    }
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::with_capacity(format_count);

    let mut push = |format_index: usize| {
        if format_index > 0 && format_index <= format_count && seen.insert(format_index) {
            ordered.push(format_index);
        }
    };

    let prioritize_shared_sparse_defaults = spreadsheet.default_format_index.is_none();
    for column_set in &spreadsheet.column_sets {
        if prioritize_shared_sparse_defaults
            && let Some(default_format_index) = column_set.default_format_index
        {
            push(default_format_index);
        }
        for column in &column_set.columns {
            push(column.format_index);
        }
    }
    for row in &spreadsheet.rows {
        push(row.format_index);
        for cell in &row.cells {
            push(cell.format_index);
            if let Some(note) = &cell.note {
                push(note.format_index);
            }
        }
    }
    for drawing in &spreadsheet.drawings {
        push(drawing.format_index);
    }
    if prioritize_shared_sparse_defaults
        && let Some(header_footer_format_index) = spreadsheet.header_footer_format_index
    {
        push(header_footer_format_index);
    }
    // The document-wide default format is a real reference, just like a row,
    // cell, or drawing. Native emits a standalone default before formats that
    // are only retained from the source palette. A default also used by the
    // header/footer belongs to that shared sparse palette instead, and keeps
    // its source position.
    if let Some(default_format_index) = spreadsheet.default_format_index
        && Some(default_format_index) != spreadsheet.header_footer_format_index
    {
        push(default_format_index);
    }
    for format_index in 1..=format_count {
        push(format_index);
    }

    ordered
}

pub(super) fn moxel_output_format_index_map(output_indices: &[usize]) -> BTreeMap<usize, usize> {
    output_indices
        .iter()
        .enumerate()
        .map(|(new_index, old_index)| (*old_index, new_index + 1))
        .collect()
}

pub(super) fn push_moxel_columns_xml(
    xml: &mut String,
    column_set: &MoxelColumnSet,
    output_format_index_map: &BTreeMap<usize, usize>,
) {
    xml.push_str("\t<columns>\r\n");
    if let Some(id) = &column_set.id {
        xml.push_str(&format!("\t\t<id>{}</id>\r\n", escape_xml_text(id)));
    }
    if let Some(default_format_index) = column_set.default_format_index {
        let default_format_index = output_format_index_map
            .get(&default_format_index)
            .copied()
            .unwrap_or(default_format_index);
        xml.push_str(&format!(
            "\t\t<formatIndex>{default_format_index}</formatIndex>\r\n"
        ));
    }
    xml.push_str(&format!("\t\t<size>{}</size>\r\n", column_set.size));
    for column in &column_set.columns {
        let column_index = column.index;
        let format_index = output_format_index_map
            .get(&column.format_index)
            .copied()
            .unwrap_or(column.format_index);
        xml.push_str(&format!(
            "\t\t<columnsItem>\r\n\
\t\t\t<index>{column_index}</index>\r\n\
\t\t\t<column>\r\n\
\t\t\t\t<formatIndex>{format_index}</formatIndex>\r\n\
\t\t\t</column>\r\n\
\t\t</columnsItem>\r\n"
        ));
    }
    xml.push_str("\t</columns>\r\n");
}

pub(super) fn moxel_source_column_format_offset(column_sets: &[MoxelColumnSet]) -> usize {
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| {
            column
                .source_format_index
                .and_then(|source| source.checked_sub(column.format_index))
        })
        .next()
        .unwrap_or(0)
}

pub(super) fn moxel_source_column_format_refs(column_sets: &[MoxelColumnSet]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for source_format_index in column_sets
        .iter()
        .filter_map(|column_set| column_set.source_default_format_index)
    {
        if source_format_index > 0 && seen.insert(source_format_index) {
            ordered.push(source_format_index);
        }
    }
    for column in column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
    {
        let source_format_index = column.source_format_index.unwrap_or(column.format_index);
        if source_format_index > 0 && seen.insert(source_format_index) {
            ordered.push(source_format_index);
        }
    }
    ordered
}

pub(super) fn remap_moxel_column_set_output_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_column_format_refs: &[usize],
) {
    if source_column_format_refs.is_empty() {
        return;
    }
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(position) = source_column_format_refs
                .iter()
                .position(|candidate| *candidate == source_format_index)
        {
            column_set.default_format_index = Some(position + 1);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let source_format_index = column.source_format_index.unwrap_or(column.format_index);
        if let Some(position) = source_column_format_refs
            .iter()
            .position(|candidate| *candidate == source_format_index)
        {
            column.format_index = position + 1;
        }
    }
}

pub(super) fn remap_moxel_row_or_cell_source_format_index(
    format_index: usize,
    source_column_format_refs: &[usize],
    is_row: bool,
) -> usize {
    if source_column_format_refs.is_empty() {
        return format_index;
    }
    if is_row {
        if format_index <= 1 {
            return format_index;
        }
    } else if format_index == 0 {
        return format_index;
    }
    let source_slot = format_index.saturating_sub(1);
    if let Some(position) = source_column_format_refs
        .iter()
        .position(|source_format_index| *source_format_index == source_slot)
    {
        return position + 1;
    }
    let removed_before = source_column_format_refs
        .iter()
        .filter(|source_format_index| **source_format_index < source_slot)
        .count();
    source_slot + source_column_format_refs.len() - removed_before
}

pub(super) fn moxel_internal_format_index_for_source_index(
    source_format_index: usize,
    column_format_len: usize,
    format_len: usize,
) -> Option<usize> {
    if source_format_index == 0 {
        return None;
    }
    let total_source_formats = column_format_len + format_len;
    if source_format_index > total_source_formats {
        return None;
    }
    let column_source_start = total_source_formats
        .saturating_sub(column_format_len)
        .saturating_add(1);
    if source_format_index >= column_source_start {
        return Some(source_format_index - column_source_start + 1);
    }
    Some(column_format_len + source_format_index)
}

pub(super) fn moxel_internal_format_index_for_sparse_source_index(
    source_format_index: usize,
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
) -> Option<usize> {
    if source_format_index == 0 {
        return None;
    }
    let total_source_formats = column_format_len + format_len;
    if source_format_index > total_source_formats {
        return None;
    }
    if let Some(position) = source_column_format_refs
        .iter()
        .position(|candidate| *candidate == source_format_index)
    {
        return Some(position + 1);
    }
    let removed_before = source_column_format_refs
        .iter()
        .filter(|candidate| **candidate < source_format_index)
        .count();
    Some(source_column_format_refs.len() + source_format_index - removed_before)
}

pub(super) fn moxel_source_derived_internal_output_order(
    column_sets: &[MoxelColumnSet],
    column_format_len: usize,
    format_len: usize,
) -> Vec<usize> {
    let total_source_formats = column_format_len + format_len;
    let mut seen_sources = BTreeSet::new();
    let mut seen_internal = BTreeSet::new();
    let mut ordered = Vec::with_capacity(total_source_formats.max(1));

    let mut push_source = |source_format_index: usize| {
        if source_format_index == 0
            || source_format_index > total_source_formats
            || !seen_sources.insert(source_format_index)
        {
            return;
        }
        if let Some(format_index) = moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            format_len,
        ) && seen_internal.insert(format_index)
        {
            ordered.push(format_index);
        }
    };

    for column in column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
    {
        push_source(column.source_format_index.unwrap_or(column.format_index));
    }
    for source_format_index in 1..=total_source_formats {
        push_source(source_format_index);
    }

    ordered
}

pub(super) fn remap_moxel_column_set_internal_format_indices(
    column_sets: &mut [MoxelColumnSet],
    column_format_len: usize,
    format_len: usize,
) {
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(format_index) = moxel_internal_format_index_for_source_index(
                source_format_index,
                column_format_len,
                format_len,
            )
        {
            column_set.default_format_index = Some(format_index);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let Some(source_format_index) = column.source_format_index else {
            continue;
        };
        if let Some(format_index) = moxel_internal_format_index_for_source_index(
            source_format_index,
            column_format_len,
            format_len,
        ) {
            column.format_index = format_index;
        }
    }
}

pub(super) fn remap_moxel_column_set_sparse_internal_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
) {
    for column_set in column_sets.iter_mut() {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
                source_format_index,
                source_column_format_refs,
                column_format_len,
                format_len,
            )
        {
            column_set.default_format_index = Some(format_index);
        }
    }
    for column in column_sets
        .iter_mut()
        .flat_map(|column_set| column_set.columns.iter_mut())
    {
        let Some(source_format_index) = column.source_format_index else {
            continue;
        };
        if let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
            source_format_index,
            source_column_format_refs,
            column_format_len,
            format_len,
        ) {
            column.format_index = format_index;
        }
    }
}

fn moxel_source_format_refs_are_complete(
    source_format_map: &MoxelSourceFormatMap,
    column_sets: &[MoxelColumnSet],
    rows: &[MoxelRow],
    drawings: &[MoxelDrawing],
    header_footer_format_ref: Option<usize>,
) -> bool {
    let direct_ref_is_valid = |source_format_index: usize| {
        source_format_index == 0
            || source_format_map
                .internal_for_source(source_format_index)
                .is_some()
    };
    let row_ref_is_valid = |source_format_index: usize| {
        source_format_index <= 1
            || source_format_map
                .internal_for_source(source_format_index - 1)
                .is_some()
    };
    let cell_ref_is_valid = |source_format_index: usize| {
        source_format_index == 0
            || (source_format_index > 1
                && source_format_map
                    .internal_for_source(source_format_index - 1)
                    .is_some())
    };

    column_sets.iter().all(|column_set| {
        column_set
            .source_default_format_index
            .is_none_or(direct_ref_is_valid)
            && column_set
                .columns
                .iter()
                .all(|column| column.source_format_index.is_none_or(direct_ref_is_valid))
    }) && rows.iter().all(|row| {
        row.source_format_index.is_none_or(row_ref_is_valid)
            && row.cells.iter().all(|cell| {
                cell.source_format_index.is_none_or(cell_ref_is_valid)
                    && cell
                        .note
                        .as_ref()
                        .is_none_or(|note| cell_ref_is_valid(note.source_format_index))
            })
    }) && drawings.iter().all(|drawing| {
        drawing.format_index == 0
            || source_format_map.internal_for_source(drawing.format_index)
                == Some(drawing.format_index)
    }) && header_footer_format_ref.is_none()
}

fn remap_moxel_column_set_source_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_format_map: &MoxelSourceFormatMap,
) {
    for column_set in column_sets {
        if let Some(source_format_index) = column_set.source_default_format_index
            && let Some(format_index) = source_format_map.internal_for_source(source_format_index)
        {
            column_set.default_format_index = Some(format_index);
        }
        for column in &mut column_set.columns {
            let Some(source_format_index) = column.source_format_index else {
                continue;
            };
            if source_format_index == 0 {
                column.format_index = 0;
            } else if let Some(format_index) =
                source_format_map.internal_for_source(source_format_index)
            {
                column.format_index = format_index;
            }
        }
    }
}

fn remap_moxel_row_and_cell_source_format_indices(
    rows: &mut [MoxelRow],
    source_format_map: &MoxelSourceFormatMap,
) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            if source_format_index <= 1 {
                // Raw row slot zero means that no row format was specified. Keep
                // that sentinel outside the typed source/internal slot map.
                row.format_index = 0;
            } else if let Some(format_index) =
                source_format_map.internal_for_source(source_format_index - 1)
            {
                row.format_index = format_index;
            }
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                if source_format_index == 0 {
                    cell.format_index = 0;
                } else if let Some(format_index) =
                    source_format_map.internal_for_source(source_format_index - 1)
                {
                    cell.format_index = format_index;
                }
            }
            if let Some(note) = &mut cell.note {
                if note.source_format_index == 0 {
                    note.format_index = 0;
                } else if let Some(format_index) =
                    source_format_map.internal_for_source(note.source_format_index - 1)
                {
                    note.format_index = format_index;
                }
            }
        }
    }
}

pub(super) fn remap_moxel_row_and_cell_sparse_source_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
    output_indices: &[usize],
) {
    let output_to_internal = output_indices
        .iter()
        .enumerate()
        .map(|(index, internal)| (index + 1, *internal))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            let output_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                true,
            );
            if let Some(format_index) = output_to_internal.get(&output_index).copied() {
                row.format_index = format_index;
            }
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                let output_index = remap_moxel_row_or_cell_source_format_index(
                    source_format_index,
                    source_column_format_refs,
                    false,
                );
                if let Some(format_index) = output_to_internal.get(&output_index).copied() {
                    cell.format_index = format_index;
                }
            }
            if let Some(note) = &mut cell.note {
                let output_index = remap_moxel_row_or_cell_source_format_index(
                    note.source_format_index,
                    source_column_format_refs,
                    false,
                );
                if let Some(format_index) = output_to_internal.get(&output_index).copied() {
                    note.format_index = format_index;
                }
            }
        }
    }
}

pub(super) fn moxel_sparse_body_source_format_offset(
    rows: &[MoxelRow],
    source_column_format_refs: &[usize],
) -> usize {
    let first_body_source_slot = rows
        .iter()
        .flat_map(|row| {
            row.source_format_index
                .into_iter()
                .chain(row.cells.iter().flat_map(|cell| {
                    cell.source_format_index
                        .into_iter()
                        .chain(cell.note.as_ref().map(|note| note.source_format_index))
                }))
        })
        .filter(|source_format_index| *source_format_index > 1)
        .map(|source_format_index| source_format_index - 1)
        .min()
        .unwrap_or(1);
    if first_body_source_slot <= 1
        || source_column_format_refs
            .iter()
            .any(|source_format_index| *source_format_index <= first_body_source_slot)
    {
        return 0;
    }
    first_body_source_slot - 1
}

pub(super) fn moxel_sparse_source_font_format_indices(
    column_format_count: usize,
    format_count: usize,
    source_body_offset: usize,
) -> Option<Vec<usize>> {
    let reserved_body_end = column_format_count.checked_add(source_body_offset)?;
    if reserved_body_end > format_count {
        return None;
    }
    let first_body_format = reserved_body_end.checked_add(1)?;
    Some(
        (first_body_format..=format_count)
            .chain((column_format_count + 1)..first_body_format)
            .chain(1..=column_format_count)
            .collect(),
    )
}

pub(super) fn remap_moxel_row_and_cell_sparse_internal_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
    column_format_len: usize,
    format_len: usize,
    source_body_offset: usize,
) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            if source_format_index <= 1 {
                row.format_index = source_format_index;
            } else if let Some(format_index) = moxel_internal_format_index_for_sparse_source_index(
                source_format_index
                    .saturating_sub(1)
                    .saturating_sub(source_body_offset),
                source_column_format_refs,
                column_format_len,
                format_len,
            ) {
                row.format_index = format_index;
            }
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                if source_format_index == 0 {
                    cell.format_index = 0;
                } else if let Some(format_index) =
                    moxel_internal_format_index_for_sparse_source_index(
                        source_format_index
                            .saturating_sub(1)
                            .saturating_sub(source_body_offset),
                        source_column_format_refs,
                        column_format_len,
                        format_len,
                    )
                {
                    cell.format_index = format_index;
                }
            }
            if let Some(note) = &mut cell.note {
                if note.source_format_index == 0 {
                    note.format_index = 0;
                } else if let Some(format_index) =
                    moxel_internal_format_index_for_sparse_source_index(
                        note.source_format_index
                            .saturating_sub(1)
                            .saturating_sub(source_body_offset),
                        source_column_format_refs,
                        column_format_len,
                        format_len,
                    )
                {
                    note.format_index = format_index;
                }
            }
        }
    }
}

pub(super) fn remap_moxel_row_and_cell_output_format_indices(
    rows: &mut [MoxelRow],
    source_column_format_refs: &[usize],
) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            row.format_index = remap_moxel_row_or_cell_source_format_index(
                source_format_index,
                source_column_format_refs,
                true,
            );
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                cell.format_index = remap_moxel_row_or_cell_source_format_index(
                    source_format_index,
                    source_column_format_refs,
                    false,
                );
            }
            if let Some(note) = &mut cell.note {
                note.format_index = remap_moxel_row_or_cell_source_format_index(
                    note.source_format_index,
                    source_column_format_refs,
                    false,
                );
            }
        }
    }
}

fn remap_moxel_leading_source_column_format_indices(rows: &mut [MoxelRow]) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            row.format_index = if source_format_index <= 1 {
                source_format_index
            } else {
                source_format_index - 1
            };
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                cell.format_index = source_format_index.saturating_sub(1);
            }
            if let Some(note) = &mut cell.note {
                note.format_index = note.source_format_index.saturating_sub(1);
            }
        }
    }
}

pub(super) fn normalize_moxel_zero_column_format_refs(rows: &mut [MoxelRow]) {
    for row in rows {
        if row.format_index > 0 {
            row.format_index -= 1;
        }
        row.source_format_index = Some(row.format_index);
        for cell in &mut row.cells {
            if cell.format_index > 0 {
                cell.format_index -= 1;
            }
            cell.source_format_index = if cell.format_index == 0 {
                None
            } else {
                Some(cell.format_index)
            };
            if let Some(note) = &mut cell.note {
                note.format_index = note.format_index.saturating_sub(1);
                note.source_format_index = note.format_index;
            }
        }
    }
}

pub(super) fn restore_moxel_source_format_refs_without_format_table(rows: &mut [MoxelRow]) {
    for row in rows {
        if let Some(source_format_index) = row.source_format_index {
            row.format_index = source_format_index;
        }
        for cell in &mut row.cells {
            if let Some(source_format_index) = cell.source_format_index {
                cell.format_index = source_format_index;
            }
            if let Some(note) = &mut cell.note {
                note.format_index = note.source_format_index;
            }
        }
    }
}

pub(super) fn moxel_uses_sparse_source_format_refs(
    column_sets: &[MoxelColumnSet],
    column_count: usize,
    _rows: &[MoxelRow],
    _default_format: &MoxelFormat,
    _default_format_width: Option<usize>,
) -> bool {
    let column_format_slots = moxel_column_format_slots(column_sets, column_count);
    column_sets
        .iter()
        .flat_map(|column_set| column_set.columns.iter())
        .filter_map(|column| column.source_format_index)
        .any(|source_format_index| source_format_index > column_format_slots)
}

pub(super) fn push_moxel_empty_headers_footers_xml(xml: &mut String) {
    for tag in [
        "leftHeader",
        "centerHeader",
        "rightHeader",
        "leftFooter",
        "centerFooter",
        "rightFooter",
    ] {
        xml.push_str(&format!(
            "\t<{tag}>\r\n\t\t<f>0</f>\r\n\t\t<tfl/>\r\n\t</{tag}>\r\n"
        ));
    }
}

pub(super) fn push_moxel_header_footer_format_refs_xml(xml: &mut String, format_index: usize) {
    for tag in [
        "leftHeader",
        "centerHeader",
        "rightHeader",
        "leftFooter",
        "centerFooter",
        "rightFooter",
    ] {
        xml.push_str(&format!(
            "\t<{tag}>\r\n\t\t<f>{format_index}</f>\r\n\t</{tag}>\r\n"
        ));
    }
}

pub(super) fn push_moxel_print_settings_xml(xml: &mut String, settings: &MoxelPrintSettings) {
    xml.push_str("\t<printSettings>\r\n");
    push_moxel_format_text(xml, "pageOrientation", settings.page_orientation);
    push_moxel_format_usize(xml, "scale", settings.scale);
    push_moxel_format_bool(xml, "collate", settings.collate);
    push_moxel_format_usize(xml, "copies", settings.copies);
    push_moxel_format_usize(xml, "perPage", settings.per_page);
    push_moxel_format_usize(xml, "topMargin", settings.top_margin);
    push_moxel_format_usize(xml, "leftMargin", settings.left_margin);
    push_moxel_format_usize(xml, "bottomMargin", settings.bottom_margin);
    push_moxel_format_usize(xml, "rightMargin", settings.right_margin);
    push_moxel_format_usize(xml, "headerSize", settings.header_size);
    push_moxel_format_usize(xml, "footerSize", settings.footer_size);
    push_moxel_format_bool(xml, "fitToPage", settings.fit_to_page);
    push_moxel_format_bool(xml, "blackAndWhite", settings.black_and_white);
    push_moxel_format_text(xml, "printerName", settings.printer_name.as_deref());
    push_moxel_format_usize(xml, "paper", settings.paper);
    push_moxel_format_usize(xml, "paperSource", settings.paper_source);
    push_moxel_format_usize(xml, "pageWidth", settings.page_width);
    push_moxel_format_usize(xml, "pageHeight", settings.page_height);
    push_moxel_format_text(xml, "duplexType", settings.duplex_type);
    push_moxel_format_text(
        xml,
        "pagePlacementAlternation",
        settings.page_placement_alternation,
    );
    xml.push_str("\t</printSettings>\r\n");
}

impl MoxelPrintSettings {
    pub(super) fn is_default_margins_only(&self) -> bool {
        self.page_orientation.is_none()
            && self.scale.is_none()
            && self.collate.is_none()
            && self.copies.is_none()
            && self.per_page.is_none()
            && self.top_margin == Some(1000)
            && self.left_margin == Some(1000)
            && self.bottom_margin == Some(1000)
            && self.right_margin == Some(1000)
            && self.header_size == Some(1000)
            && self.footer_size == Some(1000)
            && self.fit_to_page.is_none()
            && self.black_and_white.is_none()
            && self.printer_name.is_none()
            && self.paper.is_none()
            && self.paper_source.is_none()
            && self.page_width.is_none()
            && self.page_height.is_none()
            && self.duplex_type.is_none()
            && self.page_placement_alternation.is_none()
    }
}

pub(super) fn push_moxel_format_xml(
    xml: &mut String,
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
) {
    let format = moxel_format_for_index(spreadsheet, format_index);
    if format.is_empty() {
        xml.push_str("\t<format/>\r\n");
        return;
    };
    xml.push_str("\t<format>\r\n");
    push_moxel_format_usize(xml, "font", format.font);
    push_moxel_format_usize(xml, "border", format.border);
    if format.border.is_none() {
        push_moxel_format_usize(xml, "leftBorder", format.left_border);
        push_moxel_format_usize(xml, "topBorder", format.top_border);
        push_moxel_format_usize(xml, "rightBorder", format.right_border);
        push_moxel_format_usize(xml, "bottomBorder", format.bottom_border);
    }
    push_moxel_format_usize(xml, "drawingBorder", format.drawing_border);
    push_moxel_format_i32(xml, "height", format.height);
    push_moxel_format_color(xml, "borderColor", format.border_color.as_deref());
    push_moxel_format_usize(xml, "width", format.width);
    push_moxel_format_usize(xml, "widthWeightFactor", format.width_weight_factor);
    push_moxel_format_text(xml, "horizontalAlignment", format.horizontal_alignment);
    push_moxel_format_text(xml, "verticalAlignment", format.vertical_alignment);
    push_moxel_format_color(xml, "textColor", format.text_color.as_deref());
    push_moxel_format_color(xml, "backColor", format.back_color.as_deref());
    push_moxel_format_color(xml, "patternColor", format.pattern_color.as_deref());
    push_moxel_format_text(xml, "pattern", format.pattern);
    push_moxel_format_text(xml, "textPlacement", format.text_placement);
    push_moxel_format_text(xml, "fillType", format.fill_type);
    if let Some(protection) = format.protection {
        xml.push_str(&format!("\t\t<protection>{protection}</protection>\r\n"));
    }
    if let Some(hidden) = format.hidden {
        xml.push_str(&format!("\t\t<hidden>{hidden}</hidden>\r\n"));
    }
    push_moxel_format_usize(xml, "textOrientation", format.text_orientation);
    push_moxel_format_text(xml, "detailsUse", format.details_use);
    if let Some(by_selected_columns) = format.by_selected_columns {
        xml.push_str(&format!(
            "\t\t<bySelectedColumns>{by_selected_columns}</bySelectedColumns>\r\n"
        ));
    }
    if let Some(mark_negatives) = format.mark_negatives {
        xml.push_str(&format!(
            "\t\t<markNegatives>{mark_negatives}</markNegatives>\r\n"
        ));
    }
    push_moxel_localized_values_xml(
        xml,
        "format",
        &format.number_format,
        format.number_format_present,
    );
    if let Some(hyper_link) = format.hyper_link {
        xml.push_str(&format!("\t\t<hyperLink>{hyper_link}</hyperLink>\r\n"));
    }
    if let Some(auto_mark_incomplete) = format.auto_mark_incomplete {
        xml.push_str(&format!(
            "\t\t<autoMarkIncomplete>{auto_mark_incomplete}</autoMarkIncomplete>\r\n"
        ));
    }
    if let Some(mark_incomplete) = format.mark_incomplete {
        xml.push_str(&format!(
            "\t\t<markIncomplete>{mark_incomplete}</markIncomplete>\r\n"
        ));
    }
    push_moxel_format_usize(xml, "indent", format.indent);
    push_moxel_format_usize(xml, "autoIndent", format.auto_indent);
    push_moxel_localized_values_xml(
        xml,
        "editFormat",
        &format.edit_format,
        format.edit_format_present,
    );
    push_moxel_format_text(xml, "columnSizeChange", format.column_size_change);
    if let Some(mask) = format.mask {
        if mask.is_empty() {
            xml.push_str("\t\t<mask/>\r\n");
        } else {
            xml.push_str(&format!(
                "\t\t<mask>{}</mask>\r\n",
                escape_xml_element_text(mask)
            ));
        }
    }
    push_moxel_format_usize(xml, "picIndex", format.pic_index);
    push_moxel_format_text(xml, "pictureSizeMode", format.picture_size_mode);
    push_moxel_format_text(
        xml,
        "picHorizontalAlignment",
        format.pic_horizontal_alignment,
    );
    push_moxel_format_text(xml, "picVerticalAlignment", format.pic_vertical_alignment);
    push_moxel_format_text(xml, "textPosition", format.text_position);
    push_moxel_format_usize(xml, "leftMargin", format.left_margin);
    push_moxel_format_usize(xml, "topMargin", format.top_margin);
    push_moxel_format_usize(xml, "rightMargin", format.right_margin);
    push_moxel_format_usize(xml, "bottomMargin", format.bottom_margin);
    xml.push_str("\t</format>\r\n");
}

pub(super) fn push_moxel_localized_values_xml(
    xml: &mut String,
    tag: &str,
    values: &[MoxelLocalizedValue],
    present: bool,
) {
    if values.is_empty() && !present {
        return;
    }
    if values.is_empty() {
        xml.push_str(&format!("\t\t<{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!("\t\t<{tag}>\r\n"));
    for value in values {
        xml.push_str("\t\t\t<v8:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(&value.lang)
        ));
        xml.push_str(&format!(
            "\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&value.content)
        ));
        xml.push_str("\t\t\t</v8:item>\r\n");
    }
    xml.push_str(&format!("\t\t</{tag}>\r\n"));
}

pub(super) fn moxel_format_for_index(
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
) -> MoxelFormat {
    let column_format_slots = spreadsheet
        .column_formats
        .len()
        .max(moxel_column_format_slots(
            &spreadsheet.column_sets,
            spreadsheet.column_count,
        ));
    if let Some(format) = spreadsheet
        .column_formats
        .get(format_index.saturating_sub(1))
        .cloned()
    {
        return format;
    }
    if let Some(format) = spreadsheet.extra_formats.get(&format_index).cloned() {
        return format;
    }
    if spreadsheet.default_format_index == Some(format_index) {
        if spreadsheet.column_sets.len() == 1
            && spreadsheet.header_footer_format_index == Some(format_index)
            && format_index > column_format_slots
            && let Some(format) = spreadsheet
                .formats
                .get(format_index - column_format_slots - 1)
                .cloned()
        {
            return format;
        }
        let mut format = spreadsheet.default_format.clone();
        if format.width.is_none() {
            format.width = spreadsheet.default_format_width;
        }
        if !format.is_empty() {
            return format;
        }
        return MoxelFormat {
            width: spreadsheet.default_format_width,
            ..MoxelFormat::default()
        };
    }
    if format_index <= column_format_slots {
        return MoxelFormat::default();
    }
    spreadsheet
        .formats
        .get(format_index - column_format_slots - 1)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn push_moxel_format_usize(xml: &mut String, tag: &str, value: Option<usize>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{value}</{tag}>\r\n"));
    }
}

pub(super) fn push_moxel_format_i32(xml: &mut String, tag: &str, value: Option<i32>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{value}</{tag}>\r\n"));
    }
}

pub(super) fn push_moxel_format_bool(xml: &mut String, tag: &str, value: Option<bool>) {
    if let Some(value) = value {
        xml.push_str(&format!("\t\t<{tag}>{}</{tag}>\r\n", xml_bool(value)));
    }
}

pub(super) fn push_moxel_format_text(xml: &mut String, tag: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "\t\t<{tag}>{}</{tag}>\r\n",
            escape_xml_element_text(value)
        ));
    }
}

pub(super) fn push_moxel_format_color(xml: &mut String, tag: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| value.starts_with("d3p1:")) {
        xml.push_str(&format!(
            "\t\t<{tag} xmlns:d3p1=\"http://v8.1c.ru/8.1/data/ui/colors/web\">{}</{tag}>\r\n",
            escape_xml_element_text(value)
        ));
    } else {
        push_moxel_format_text(xml, tag, value);
    }
}

pub(super) fn push_moxel_picture_xml(xml: &mut String, picture: &MoxelPicture) {
    xml.push_str("\t<picture>\r\n");
    xml.push_str(&format!("\t\t<index>{}</index>\r\n", picture.index));
    if let Some(payload) = &picture.payload {
        xml.push_str(&format!(
            "\t\t<picture t=\"false\">{}</picture>\r\n",
            escape_xml_text(payload)
        ));
    } else if let Some(ref_name) = &picture.ref_name {
        xml.push_str(&format!(
            "\t\t<picture t=\"false\" ref=\"{}\"/>\r\n",
            escape_xml_text(ref_name)
        ));
    } else {
        xml.push_str("\t\t<picture/>\r\n");
    }
    xml.push_str("\t</picture>\r\n");
}

pub(super) fn push_moxel_drawing_xml(
    xml: &mut String,
    drawing: &MoxelDrawing,
    output_format_index_map: &BTreeMap<usize, usize>,
) {
    xml.push_str("\t<drawing>\r\n");
    let drawing_type = match drawing.kind {
        MoxelDrawingKind::Picture { .. } => "Picture",
        MoxelDrawingKind::Chart(_) => "Chart",
    };
    xml.push_str(&format!(
        "\t\t<drawingType>{drawing_type}</drawingType>\r\n"
    ));
    xml.push_str(&format!("\t\t<id>{}</id>\r\n", drawing.id));
    let format_index = output_format_index_map
        .get(&drawing.format_index)
        .copied()
        .unwrap_or(drawing.format_index);
    xml.push_str(&format!(
        "\t\t<formatIndex>{}</formatIndex>\r\n",
        format_index
    ));
    xml.push_str(&format!(
        "\t\t<beginRow>{}</beginRow>\r\n",
        drawing.begin_row
    ));
    xml.push_str(&format!(
        "\t\t<beginRowOffset>{}</beginRowOffset>\r\n",
        drawing.begin_row_offset
    ));
    xml.push_str(&format!("\t\t<endRow>{}</endRow>\r\n", drawing.end_row));
    xml.push_str(&format!(
        "\t\t<endRowOffset>{}</endRowOffset>\r\n",
        drawing.end_row_offset
    ));
    xml.push_str(&format!(
        "\t\t<beginColumn>{}</beginColumn>\r\n",
        drawing.begin_column
    ));
    xml.push_str(&format!(
        "\t\t<beginColumnOffset>{}</beginColumnOffset>\r\n",
        drawing.begin_column_offset
    ));
    xml.push_str(&format!(
        "\t\t<endColumn>{}</endColumn>\r\n",
        drawing.end_column
    ));
    xml.push_str(&format!(
        "\t\t<endColumnOffset>{}</endColumnOffset>\r\n",
        drawing.end_column_offset
    ));
    xml.push_str(&format!(
        "\t\t<autoSize>{}</autoSize>\r\n",
        xml_bool(drawing.auto_size)
    ));
    let picture_size = match &drawing.kind {
        MoxelDrawingKind::Picture { picture_size, .. } => *picture_size,
        MoxelDrawingKind::Chart(_) => "Stretch",
    };
    xml.push_str(&format!(
        "\t\t<pictureSize>{picture_size}</pictureSize>\r\n"
    ));
    xml.push_str(&format!("\t\t<zOrder>{}</zOrder>\r\n", drawing.z_order));
    match &drawing.kind {
        MoxelDrawingKind::Picture { picture_index, .. } => {
            xml.push_str(&format!(
                "\t\t<pictureIndex>{picture_index}</pictureIndex>\r\n"
            ));
        }
        MoxelDrawingKind::Chart(chart) => push_moxel_chart_xml(xml, chart),
    }
    xml.push_str("\t</drawing>\r\n");
}

fn push_moxel_chart_xml(xml: &mut String, chart: &MoxelChart) {
    xml.push_str(
        "\t\t<object xmlns:d3p1=\"http://v8.1c.ru/8.2/data/chart\" xsi:type=\"d3p1:Chart\">\r\n",
    );
    push_moxel_chart_text(xml, "seriesCurId", chart.series_cur_id);
    push_moxel_chart_text(xml, "pointsCurId", chart.points_cur_id);
    push_moxel_chart_bool(xml, "isSeriesDesign", chart.is_series_design);
    push_moxel_chart_text(xml, "realSeriesCount", chart.real_series.len());
    for series in &chart.real_series {
        push_moxel_chart_series_xml(xml, "realSeriesData", series);
    }
    push_moxel_chart_series_xml(xml, "realExSeriesData", &chart.real_extra_series);
    push_moxel_chart_bool(xml, "isPointsDesign", chart.is_points_design);
    push_moxel_chart_text(xml, "realPointCount", chart.real_points.len());
    for point in &chart.real_points {
        push_moxel_chart_point_xml(xml, point);
    }
    push_moxel_chart_text(xml, "curSeries", chart.cur_series);
    push_moxel_chart_text(xml, "curPoint", chart.cur_point);
    push_moxel_chart_literal(xml, "chartType", "Line");
    push_moxel_chart_literal(xml, "circleLabelType", "None");
    push_moxel_chart_literal(xml, "labelsDelimiter", ", ");
    push_moxel_chart_literal(xml, "labelsLocation", chart.labels_location);
    push_moxel_chart_empty(xml, "lbFormat");
    push_moxel_chart_empty(xml, "lbpFormat");
    push_moxel_chart_literal(xml, "labelsColor", "style:FormTextColor");
    xml.push_str("\t\t\t<d3p1:labelsFont kind=\"AutoFont\"/>\r\n");
    push_moxel_chart_bool(xml, "transparentLabelsBkg", true);
    push_moxel_chart_literal(xml, "labelsBkgColor", "auto");
    push_moxel_chart_border_xml(xml, "labelsBorder", 1, "Single");
    push_moxel_chart_literal(xml, "labelsBorderColor", "auto");
    push_moxel_chart_literal(xml, "circleExpandMode", "None");
    push_moxel_chart_literal(xml, "chart3Dcrd", "SouthWest");
    push_moxel_chart_localized_xml(xml, "title", &chart.title, 3);
    push_moxel_chart_bool(xml, "isShowTitle", false);
    push_moxel_chart_bool(xml, "isShowLegend", false);
    push_moxel_chart_border_xml(xml, "ttlBorder", 0, "WithoutBorder");
    push_moxel_chart_literal(xml, "ttlBorderColor", "style:BorderColor");
    push_moxel_chart_border_xml(xml, "lgBorder", 0, "WithoutBorder");
    push_moxel_chart_literal(xml, "lgBorderColor", "style:BorderColor");
    push_moxel_chart_border_xml(xml, "chBorder", 0, "WithoutBorder");
    push_moxel_chart_literal(xml, "chBorderColor", "style:BorderColor");
    push_moxel_chart_bool(xml, "transparent", false);
    push_moxel_chart_literal(xml, "bkgColor", "style:FormBackColor");
    push_moxel_chart_bool(xml, "isTrnspTtl", true);
    push_moxel_chart_literal(xml, "ttlColor", "style:FormBackColor");
    push_moxel_chart_bool(xml, "isTrnspLeg", true);
    push_moxel_chart_literal(xml, "legColor", "style:FormBackColor");
    push_moxel_chart_bool(xml, "isTrnspCh", false);
    push_moxel_chart_literal(xml, "chColor", "#FFFFFF");
    push_moxel_chart_literal(xml, "ttlTxtColor", "style:FormTextColor");
    push_moxel_chart_literal(xml, "legTxtColor", "style:FormTextColor");
    push_moxel_chart_literal(xml, "chTxtColor", "style:FormTextColor");
    push_moxel_chart_style_font(xml, "ttlFont");
    push_moxel_chart_style_font(xml, "legFont");
    push_moxel_chart_style_font(xml, "chFont");
    push_moxel_chart_bool(xml, "isShowScale", true);
    push_moxel_chart_bool(xml, "isShowScaleVL", true);
    push_moxel_chart_bool(xml, "isShowSeriesScale", true);
    push_moxel_chart_bool(xml, "isShowPointsScale", true);
    push_moxel_chart_bool(xml, "isShowValuesScale", true);
    push_moxel_chart_localized_xml(xml, "vsFormat", &chart.values_scale_format, 3);
    push_moxel_chart_literal(xml, "xLabelsOrientation", "Auto");
    push_moxel_chart_line_xml(xml, "scaleLine", &MoxelChartLine { width: 1 }, 3);
    push_moxel_chart_literal(xml, "scaleColor", "#A9A9A9");
    push_moxel_chart_bool(xml, "isAutoSeriesName", chart.is_auto_series_name);
    push_moxel_chart_bool(xml, "isAutoPointName", false);
    push_moxel_chart_literal(xml, "maxMode", "NotDefined");
    push_moxel_chart_text(xml, "maxSeries", chart.max_series);
    push_moxel_chart_text(xml, "maxSeriesPrc", 30);
    push_moxel_chart_literal(xml, "spaceMode", "Half");
    push_moxel_chart_text(xml, "baseVal", 0);
    push_moxel_chart_bool(xml, "isOutline", chart.is_outline);
    push_moxel_chart_text(xml, "realPiePoint", 0);
    push_moxel_chart_text(xml, "realStockSeries", 0);
    push_moxel_chart_bool(xml, "isLight", true);
    push_moxel_chart_bool(xml, "isGradient", false);
    push_moxel_chart_bool(xml, "isTransposition", false);
    push_moxel_chart_bool(xml, "hideBaseVal", false);
    push_moxel_chart_bool(xml, "dataTable", false);
    push_moxel_chart_bool(xml, "dtVerLines", true);
    push_moxel_chart_bool(xml, "dtHorLines", true);
    push_moxel_chart_literal(xml, "dtHAlign", "Right");
    push_moxel_chart_empty(xml, "dtFormat");
    push_moxel_chart_bool(xml, "dtKeys", true);
    push_moxel_chart_literal(xml, "paletteKind", "Auto");
    push_moxel_chart_literal(xml, "animation", "Auto");
    push_moxel_chart_text(xml, "rebuildTime", chart.rebuild_time);
    push_moxel_chart_bool(xml, "isTransposed", false);
    push_moxel_chart_bool(xml, "autoTransposition", false);
    push_moxel_chart_bool(xml, "legendScrollEnable", false);
    push_moxel_chart_literal(xml, "surfaceColor", "#A90000");
    push_moxel_chart_literal(xml, "radarScaleType", "Circle");
    push_moxel_chart_literal(xml, "gaugeValuesPresentation", "Needle");
    push_moxel_chart_gauge_bands_xml(xml, &chart.gauge_bands);
    push_moxel_chart_text(xml, "beginGaugeAngle", 0);
    push_moxel_chart_text(xml, "endGaugeAngle", 180);
    push_moxel_chart_text(xml, "gaugeThickness", 2);
    push_moxel_chart_literal(xml, "gaugeLabelsLocation", "InsideScale");
    push_moxel_chart_bool(xml, "gaugeLabelsArcDirection", false);
    push_moxel_chart_text(xml, "gaugeBushThickness", 5);
    push_moxel_chart_literal(xml, "gaugeBushColor", "#A9A9A9");
    push_moxel_chart_bool(xml, "autoMaxValue", true);
    push_moxel_chart_literal(xml, "userMaxValue", &chart.user_max_value);
    push_moxel_chart_bool(xml, "autoMinValue", true);
    push_moxel_chart_literal(xml, "userMinValue", &chart.user_min_value);
    push_moxel_chart_bool(xml, "elementsIsInit", true);
    push_moxel_chart_bool(xml, "titleIsInit", true);
    push_moxel_chart_bool(xml, "legendIsInit", true);
    push_moxel_chart_bool(xml, "chartIsInit", true);
    push_moxel_chart_rectangle_xml(xml, "elementsChart", &chart.elements_chart);
    push_moxel_chart_rectangle_xml(xml, "elementsLegend", &chart.elements_legend);
    push_moxel_chart_rectangle_xml(xml, "elementsTitle", &chart.elements_title);
    push_moxel_chart_literal(xml, "borderColor", "style:BorderColor");
    push_moxel_chart_border_xml(xml, "border", 0, "WithoutBorder");
    push_moxel_chart_empty(xml, "dataSourceDescription");
    push_moxel_chart_bool(xml, "isDataSourceMode", false);
    push_moxel_chart_bool(xml, "isRandomizedNewValues", true);
    push_moxel_chart_data_items_xml(xml, &chart.real_data_items);
    push_moxel_chart_text(xml, "splineStrain", chart.spline_strain);
    push_moxel_chart_literal(xml, "translucencePercent", &chart.translucence_percent);
    push_moxel_chart_literal(
        xml,
        "funnelNeckHeightPercent",
        &chart.funnel_neck_height_percent,
    );
    push_moxel_chart_literal(
        xml,
        "funnelNeckWidthPercent",
        &chart.funnel_neck_width_percent,
    );
    push_moxel_chart_literal(xml, "funnelGapSumPercent", &chart.funnel_gap_sum_percent);
    push_moxel_chart_line_xml(xml, "multiStageLinkLine", &MoxelChartLine { width: 1 }, 3);
    push_moxel_chart_literal(xml, "multiStageLinkColor", "#000000");
    push_moxel_chart_axis_xml(xml, "valuesAxis", &chart.values_axis);
    push_moxel_chart_axis_xml(xml, "pointsAxis", &chart.points_axis);
    if !chart.values_scale_format.is_empty() {
        xml.push_str("\t\t\t<d3p1:valuesScale>\r\n");
        xml.push_str("\t\t\t\t<d3p1:titleArea>\r\n");
        xml.push_str("\t\t\t\t\t<d3p1:font kind=\"AutoFont\"/>\r\n");
        xml.push_str("\t\t\t\t\t<d3p1:textColor>auto</d3p1:textColor>\r\n");
        xml.push_str("\t\t\t\t\t<d3p1:backColor>auto</d3p1:backColor>\r\n");
        xml.push_str("\t\t\t\t\t<d3p1:border width=\"1\">\r\n");
        xml.push_str(
            "\t\t\t\t\t\t<v8ui:style xsi:type=\"v8ui:ControlBorderType\">WithoutBorder</v8ui:style>\r\n",
        );
        xml.push_str("\t\t\t\t\t</d3p1:border>\r\n");
        xml.push_str("\t\t\t\t\t<d3p1:borderColor>auto</d3p1:borderColor>\r\n");
        xml.push_str("\t\t\t\t</d3p1:titleArea>\r\n");
        push_moxel_chart_localized_xml(xml, "labelFormat", &chart.values_scale_format, 4);
        xml.push_str("\t\t\t</d3p1:valuesScale>\r\n");
    }
    push_moxel_chart_literal(xml, "legendPlacement", "None");
    push_moxel_chart_literal(xml, "plotAreaPlacement", "UseCoordinates");
    push_moxel_chart_literal(xml, "titleAreaPlacement", "None");
    xml.push_str("\t\t</object>\r\n");
}

fn push_moxel_chart_series_xml(xml: &mut String, tag: &str, series: &MoxelChartSeries) {
    xml.push_str(&format!("\t\t\t<d3p1:{tag}>\r\n"));
    push_moxel_chart_text_indented(xml, "id", series.id, 4);
    push_moxel_chart_literal_indented(xml, "color", &series.color, 4);
    push_moxel_chart_line_xml(xml, "line", &series.line, 4);
    push_moxel_chart_literal_indented(xml, "marker", series.marker, 4);
    push_moxel_chart_localized_xml(xml, "text", &series.text, 4);
    push_moxel_chart_bool_indented(xml, "strIsChanged", series.str_is_changed, 4);
    push_moxel_chart_bool_indented(xml, "isExpand", series.is_expand, 4);
    push_moxel_chart_bool_indented(xml, "isIndicator", series.is_indicator, 4);
    push_moxel_chart_bool_indented(xml, "colorPriority", series.color_priority, 4);
    xml.push_str(&format!("\t\t\t</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_point_xml(xml: &mut String, point: &MoxelChartPoint) {
    xml.push_str("\t\t\t<d3p1:realPointData>\r\n");
    push_moxel_chart_text_indented(xml, "id", point.id, 4);
    push_moxel_chart_literal_indented(xml, "color", &point.color, 4);
    push_moxel_chart_line_xml(xml, "line", &point.line, 4);
    push_moxel_chart_literal_indented(xml, "marker", point.marker, 4);
    push_moxel_chart_localized_xml(xml, "text", &point.text, 4);
    push_moxel_chart_bool_indented(xml, "strIsChanged", point.str_is_changed, 4);
    push_moxel_chart_bool_indented(xml, "isExpand", point.is_expand, 4);
    push_moxel_chart_bool_indented(xml, "isIndicator", point.is_indicator, 4);
    push_moxel_chart_bool_indented(xml, "colorPriority", point.color_priority, 4);
    xml.push_str("\t\t\t</d3p1:realPointData>\r\n");
}

fn push_moxel_chart_line_xml(xml: &mut String, tag: &str, line: &MoxelChartLine, indent: usize) {
    let tabs = "\t".repeat(indent);
    xml.push_str(&format!(
        "{tabs}<d3p1:{tag} width=\"{}\" gap=\"false\">\r\n",
        line.width
    ));
    xml.push_str(&format!(
        "{tabs}\t<v8ui:style xsi:type=\"v8ui:ChartLineType\">Solid</v8ui:style>\r\n"
    ));
    xml.push_str(&format!("{tabs}</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_border_xml(xml: &mut String, tag: &str, width: usize, style: &str) {
    xml.push_str(&format!("\t\t\t<d3p1:{tag} width=\"{width}\">\r\n"));
    xml.push_str(&format!(
        "\t\t\t\t<v8ui:style xsi:type=\"v8ui:ControlBorderType\">{style}</v8ui:style>\r\n"
    ));
    xml.push_str(&format!("\t\t\t</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_style_font(xml: &mut String, tag: &str) {
    xml.push_str(&format!(
        "\t\t\t<d3p1:{tag} ref=\"style:TextFont\" kind=\"StyleItem\"/>\r\n"
    ));
}

fn push_moxel_chart_localized_xml(
    xml: &mut String,
    tag: &str,
    values: &[MoxelLocalizedValue],
    indent: usize,
) {
    let tabs = "\t".repeat(indent);
    if values.is_empty() {
        xml.push_str(&format!("{tabs}<d3p1:{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!("{tabs}<d3p1:{tag}>\r\n"));
    for value in values {
        xml.push_str(&format!("{tabs}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{tabs}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(&value.lang)
        ));
        xml.push_str(&format!(
            "{tabs}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&value.content)
        ));
        xml.push_str(&format!("{tabs}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{tabs}</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_gauge_bands_xml(xml: &mut String, bands: &[MoxelChartGaugeBand]) {
    xml.push_str("\t\t\t<d3p1:gaugeQualityBands useTextStr=\"false\" useTooltipStr=\"false\">\r\n");
    for band in bands {
        xml.push_str("\t\t\t\t<v8ui:item>\r\n");
        push_moxel_chart_plain_literal(xml, "v8ui", "begin", &band.begin, 5);
        push_moxel_chart_plain_literal(xml, "v8ui", "end", &band.end, 5);
        push_moxel_chart_plain_literal(xml, "v8ui", "backColor", &band.back_color, 5);
        push_moxel_chart_namespaced_localized_xml(xml, "v8ui", "text", &band.text, 5);
        push_moxel_chart_namespaced_localized_xml(xml, "v8ui", "tooltip", &band.tooltip, 5);
        xml.push_str("\t\t\t\t</v8ui:item>\r\n");
    }
    xml.push_str("\t\t\t</d3p1:gaugeQualityBands>\r\n");
}

fn push_moxel_chart_namespaced_localized_xml(
    xml: &mut String,
    prefix: &str,
    tag: &str,
    values: &[MoxelLocalizedValue],
    indent: usize,
) {
    let tabs = "\t".repeat(indent);
    if values.is_empty() {
        xml.push_str(&format!("{tabs}<{prefix}:{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!("{tabs}<{prefix}:{tag}>\r\n"));
    for value in values {
        xml.push_str(&format!("{tabs}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{tabs}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(&value.lang)
        ));
        xml.push_str(&format!(
            "{tabs}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&value.content)
        ));
        xml.push_str(&format!("{tabs}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{tabs}</{prefix}:{tag}>\r\n"));
}

fn push_moxel_chart_data_items_xml(xml: &mut String, items: &[MoxelChartDataItem]) {
    xml.push_str("\t\t\t<d3p1:realDataItems>\r\n");
    for item in items {
        xml.push_str("\t\t\t\t<d3p1:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t\t<d3p1:valData xsi:type=\"xs:decimal\">{}</d3p1:valData>\r\n",
            escape_xml_element_text(&item.value)
        ));
        xml.push_str("\t\t\t\t\t<d3p1:valInfo xsi:nil=\"true\"/>\r\n");
        if item.tooltip.is_empty() {
            xml.push_str("\t\t\t\t\t<d3p1:toolTip/>\r\n");
        } else {
            xml.push_str(&format!(
                "\t\t\t\t\t<d3p1:toolTip>{}</d3p1:toolTip>\r\n",
                escape_xml_element_text(&item.tooltip)
            ));
        }
        xml.push_str("\t\t\t\t</d3p1:item>\r\n");
    }
    xml.push_str("\t\t\t</d3p1:realDataItems>\r\n");
}

fn push_moxel_chart_rectangle_xml(xml: &mut String, tag: &str, rect: &MoxelChartRectangle) {
    xml.push_str(&format!("\t\t\t<d3p1:{tag}>\r\n"));
    push_moxel_chart_literal_indented(xml, "left", &rect.left, 4);
    push_moxel_chart_literal_indented(xml, "right", &rect.right, 4);
    push_moxel_chart_literal_indented(xml, "top", &rect.top, 4);
    push_moxel_chart_literal_indented(xml, "bottom", &rect.bottom, 4);
    xml.push_str(&format!("\t\t\t</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_axis_xml(xml: &mut String, tag: &str, axis: &MoxelChartAxis) {
    if axis.min_value.is_none() && axis.max_value.is_none() {
        push_moxel_chart_empty(xml, tag);
        return;
    }
    xml.push_str(&format!("\t\t\t<d3p1:{tag}>\r\n"));
    if let Some(value) = &axis.min_value {
        xml.push_str(&format!(
            "\t\t\t\t<d3p1:minValue xsi:type=\"xs:decimal\">{}</d3p1:minValue>\r\n",
            escape_xml_element_text(value)
        ));
    }
    if let Some(value) = &axis.max_value {
        xml.push_str(&format!(
            "\t\t\t\t<d3p1:maxValue xsi:type=\"xs:decimal\">{}</d3p1:maxValue>\r\n",
            escape_xml_element_text(value)
        ));
    }
    xml.push_str(&format!("\t\t\t</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_text(xml: &mut String, tag: &str, value: impl std::fmt::Display) {
    push_moxel_chart_text_indented(xml, tag, value, 3);
}

fn push_moxel_chart_text_indented(
    xml: &mut String,
    tag: &str,
    value: impl std::fmt::Display,
    indent: usize,
) {
    let tabs = "\t".repeat(indent);
    xml.push_str(&format!("{tabs}<d3p1:{tag}>{value}</d3p1:{tag}>\r\n"));
}

fn push_moxel_chart_bool(xml: &mut String, tag: &str, value: bool) {
    push_moxel_chart_bool_indented(xml, tag, value, 3);
}

fn push_moxel_chart_bool_indented(xml: &mut String, tag: &str, value: bool, indent: usize) {
    push_moxel_chart_text_indented(xml, tag, xml_bool(value), indent);
}

fn push_moxel_chart_literal(xml: &mut String, tag: &str, value: &str) {
    push_moxel_chart_literal_indented(xml, tag, value, 3);
}

fn push_moxel_chart_literal_indented(xml: &mut String, tag: &str, value: &str, indent: usize) {
    push_moxel_chart_plain_literal(xml, "d3p1", tag, value, indent);
}

fn push_moxel_chart_plain_literal(
    xml: &mut String,
    prefix: &str,
    tag: &str,
    value: &str,
    indent: usize,
) {
    let tabs = "\t".repeat(indent);
    xml.push_str(&format!(
        "{tabs}<{prefix}:{tag}>{}</{prefix}:{tag}>\r\n",
        escape_xml_element_text(value)
    ));
}

fn push_moxel_chart_empty(xml: &mut String, tag: &str) {
    xml.push_str(&format!("\t\t\t<d3p1:{tag}/>\r\n"));
}

pub(super) fn push_moxel_merge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<merge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</merge>\r\n");
}

pub(super) fn push_moxel_vertical_group_xml(xml: &mut String, group: &MoxelVerticalGroup) {
    xml.push_str("\t<vg>\r\n");
    xml.push_str(&format!("\t\t<b>{}</b>\r\n", group.begin_row));
    if group.end_row != group.begin_row {
        xml.push_str(&format!("\t\t<e>{}</e>\r\n", group.end_row));
    }
    xml.push_str("\t</vg>\r\n");
}

pub(super) fn push_moxel_vertical_unmerge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<verticalUnmerge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</verticalUnmerge>\r\n");
}

pub(super) fn push_moxel_horizontal_unmerge_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str("\t<horizontalUnmerge>\r\n");
    push_moxel_merge_body_xml(xml, merge);
    xml.push_str("\t</horizontalUnmerge>\r\n");
}

pub(super) fn push_moxel_merge_body_xml(xml: &mut String, merge: &MoxelMerge) {
    xml.push_str(&format!("\t\t<r>{}</r>\r\n", merge.row));
    xml.push_str(&format!("\t\t<c>{}</c>\r\n", merge.column));
    if merge.height > 0 {
        xml.push_str(&format!("\t\t<h>{}</h>\r\n", merge.height));
    }
    if merge.width > 0 {
        xml.push_str(&format!("\t\t<w>{}</w>\r\n", merge.width));
    }
    if let Some(columns_id) = &merge.columns_id {
        xml.push_str(&format!("\t\t<columnsID>{columns_id}</columnsID>\r\n"));
    }
}

pub(super) fn push_moxel_line_xml(xml: &mut String, line: &MoxelLine) {
    xml.push_str(&format!(
        "\t<line width=\"{}\" gap=\"false\">\r\n",
        line.width
    ));
    xml.push_str(&format!(
        "\t\t<v8ui:style xsi:type=\"{}\">{}</v8ui:style>\r\n",
        line.line_type, line.style
    ));
    xml.push_str("\t</line>\r\n");
}

pub(super) fn push_moxel_font_xml(xml: &mut String, font: &MoxelFont) {
    xml.push_str("\t<font");
    if let Some(ref_name) = &font.ref_name {
        if ref_name.starts_with("sys:") {
            xml.push_str(" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"");
        }
        xml.push_str(&format!(" ref=\"{}\"", escape_xml_text(ref_name)));
    }
    if let Some(face_name) = &font.face_name {
        xml.push_str(&format!(" faceName=\"{}\"", escape_xml_text(face_name)));
    }
    if let Some(height) = &font.height {
        xml.push_str(&format!(" height=\"{}\"", escape_xml_text(height)));
    }
    if font.kind == "WindowsFont" {
        xml.push_str(&format!(" bold=\"{}\"", font.bold));
        if font.italic {
            xml.push_str(" italic=\"true\"");
        }
        if font.underline {
            xml.push_str(" underline=\"true\"");
        }
        if font.strikeout {
            xml.push_str(" strikeout=\"true\"");
        }
        xml.push_str(" kind=\"WindowsFont\"");
    } else if font.kind == "StyleItem"
        && !font.bold
        && !font.italic
        && !font.underline
        && !font.strikeout
        && font.scale.is_none()
    {
        xml.push_str(" kind=\"StyleItem\"");
    } else {
        xml.push_str(&format!(
            " bold=\"{}\" italic=\"{}\" underline=\"{}\" strikeout=\"{}\" kind=\"{}\"",
            font.bold, font.italic, font.underline, font.strikeout, font.kind
        ));
        if let Some(scale) = font.scale {
            xml.push_str(&format!(" scale=\"{scale}\""));
        }
    }
    xml.push_str("/>\r\n");
}

pub(super) fn push_moxel_named_item_xml(xml: &mut String, named_item: &MoxelNamedItem) {
    match named_item {
        MoxelNamedItem::Cells(area) => push_moxel_area_xml(xml, area),
        MoxelNamedItem::Drawing { name, drawing_id } => {
            xml.push_str("\t<namedItem xsi:type=\"NamedItemDrawing\">\r\n");
            xml.push_str(&format!(
                "\t\t<name>{}</name>\r\n",
                escape_xml_element_text(name)
            ));
            xml.push_str(&format!("\t\t<drawingID>{drawing_id}</drawingID>\r\n"));
            xml.push_str("\t</namedItem>\r\n");
        }
    }
}

pub(super) fn push_moxel_area_xml(xml: &mut String, area: &MoxelArea) {
    xml.push_str("\t<namedItem xsi:type=\"NamedItemCells\">\r\n");
    xml.push_str(&format!(
        "\t\t<name>{}</name>\r\n",
        escape_xml_element_text(&area.name)
    ));
    xml.push_str("\t\t<area>\r\n");
    xml.push_str(&format!("\t\t\t<type>{}</type>\r\n", area.area_type));
    xml.push_str(&format!(
        "\t\t\t<beginRow>{}</beginRow>\r\n",
        area.begin_row
    ));
    xml.push_str(&format!("\t\t\t<endRow>{}</endRow>\r\n", area.end_row));
    xml.push_str(&format!(
        "\t\t\t<beginColumn>{}</beginColumn>\r\n",
        area.begin_column
    ));
    xml.push_str(&format!(
        "\t\t\t<endColumn>{}</endColumn>\r\n",
        area.end_column
    ));
    if let Some(columns_id) = &area.columns_id {
        xml.push_str(&format!(
            "\t\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    xml.push_str("\t\t</area>\r\n");
    xml.push_str("\t</namedItem>\r\n");
}

pub(super) fn push_moxel_print_area_xml(xml: &mut String, area: &MoxelArea) {
    xml.push_str("\t<printArea>\r\n");
    xml.push_str(&format!("\t\t<type>{}</type>\r\n", area.area_type));
    xml.push_str(&format!("\t\t<beginRow>{}</beginRow>\r\n", area.begin_row));
    xml.push_str(&format!("\t\t<endRow>{}</endRow>\r\n", area.end_row));
    xml.push_str(&format!(
        "\t\t<beginColumn>{}</beginColumn>\r\n",
        area.begin_column
    ));
    xml.push_str(&format!(
        "\t\t<endColumn>{}</endColumn>\r\n",
        area.end_column
    ));
    if let Some(columns_id) = &area.columns_id {
        xml.push_str(&format!(
            "\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    xml.push_str("\t</printArea>\r\n");
}

pub(super) fn push_moxel_row_xml(
    xml: &mut String,
    row: &MoxelRow,
    output_format_index_map: &BTreeMap<usize, usize>,
    emit_first_format_index: bool,
) {
    xml.push_str(&format!(
        "\t<rowsItem>\r\n\t\t<index>{}</index>\r\n",
        row.index
    ));
    if let Some(index_to) = row.index_to {
        xml.push_str(&format!("\t\t<indexTo>{index_to}</indexTo>\r\n"));
    }
    xml.push_str("\t\t<row>\r\n");
    let format_index = output_format_index_map
        .get(&row.format_index)
        .copied()
        .unwrap_or(row.format_index);
    if let Some(columns_id) = &row.columns_id {
        xml.push_str(&format!(
            "\t\t\t<columnsID>{}</columnsID>\r\n",
            escape_xml_text(columns_id)
        ));
    }
    let explicit_source_format_collapsed_to_one = format_index == 1
        && row
            .source_format_index
            .is_some_and(|source_format_index| source_format_index > 1);
    let leading_shared_default_shifted_row_format = format_index == 2
        && row.format_index == 1
        && row.source_format_index == Some(1)
        && output_format_index_map.get(&1).copied() == Some(2);
    if format_index > 1 && !leading_shared_default_shifted_row_format
        || (emit_first_format_index && format_index == 1)
        || explicit_source_format_collapsed_to_one
    {
        xml.push_str(&format!(
            "\t\t\t<formatIndex>{format_index}</formatIndex>\r\n"
        ));
    }
    if row.cells.is_empty() {
        xml.push_str("\t\t\t<empty>true</empty>\r\n");
        xml.push_str("\t\t</row>\r\n\t</rowsItem>\r\n");
        return;
    }
    let mut expected_column = 0usize;
    for cell in &row.cells {
        xml.push_str("\t\t\t<c>\r\n");
        if cell.column_index != expected_column {
            xml.push_str(&format!("\t\t\t\t<i>{}</i>\r\n", cell.column_index));
        }
        xml.push_str("\t\t\t\t<c>\r\n");
        let cell_format_index = if cell.format_index == 0 {
            0
        } else {
            output_format_index_map
                .get(&cell.format_index)
                .copied()
                .unwrap_or(cell.format_index)
        };
        xml.push_str(&format!("\t\t\t\t\t<f>{cell_format_index}</f>\r\n"));
        if let Some(text) = &cell.text {
            xml.push_str("\t\t\t\t\t<tl>\r\n");
            xml.push_str("\t\t\t\t\t\t<v8:item>\r\n");
            xml.push_str("\t\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(text)
            ));
            xml.push_str("\t\t\t\t\t\t</v8:item>\r\n");
            xml.push_str("\t\t\t\t\t</tl>\r\n");
        } else if cell.empty_text {
            xml.push_str("\t\t\t\t\t<tl/>\r\n");
        }
        if let Some(parameter) = &cell.parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<parameter>{}</parameter>\r\n",
                escape_xml_element_text(parameter)
            ));
        }
        if let Some(detail_parameter) = &cell.detail_parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<detailParameter>{}</detailParameter>\r\n",
                escape_xml_element_text(detail_parameter)
            ));
        }
        if let Some(note) = &cell.note {
            push_moxel_note_xml(xml, note, output_format_index_map);
        }
        xml.push_str("\t\t\t\t</c>\r\n");
        xml.push_str("\t\t\t</c>\r\n");
        expected_column = cell.column_index + 1;
    }
    xml.push_str("\t\t</row>\r\n\t</rowsItem>\r\n");
}

fn push_moxel_note_xml(
    xml: &mut String,
    note: &MoxelNote,
    output_format_index_map: &BTreeMap<usize, usize>,
) {
    let format_index = output_format_index_map
        .get(&note.format_index)
        .copied()
        .unwrap_or(note.format_index);
    xml.push_str("\t\t\t\t\t<note>\r\n");
    xml.push_str("\t\t\t\t\t\t<drawingType>Comment</drawingType>\r\n");
    xml.push_str("\t\t\t\t\t\t<id>0</id>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t\t\t<formatIndex>{format_index}</formatIndex>\r\n"
    ));
    xml.push_str("\t\t\t\t\t\t<text>\r\n");
    xml.push_str("\t\t\t\t\t\t\t<v8:item>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
        escape_xml_element_text(&note.text.lang)
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
        escape_xml_element_text(&note.text.content)
    ));
    xml.push_str("\t\t\t\t\t\t\t</v8:item>\r\n");
    xml.push_str("\t\t\t\t\t\t</text>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t\t\t<beginRow>{}</beginRow>\r\n",
        note.begin_row
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<beginRowOffset>{}</beginRowOffset>\r\n",
        note.begin_row_offset
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<endRow>{}</endRow>\r\n",
        note.end_row
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<endRowOffset>{}</endRowOffset>\r\n",
        note.end_row_offset
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<beginColumn>{}</beginColumn>\r\n",
        note.begin_column
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<beginColumnOffset>{}</beginColumnOffset>\r\n",
        note.begin_column_offset
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<endColumn>{}</endColumn>\r\n",
        note.end_column
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<endColumnOffset>{}</endColumnOffset>\r\n",
        note.end_column_offset
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<autoSize>{}</autoSize>\r\n",
        xml_bool(note.auto_size)
    ));
    xml.push_str("\t\t\t\t\t\t<pictureSize>Stretch</pictureSize>\r\n");
    xml.push_str("\t\t\t\t\t</note>\r\n");
}

#[cfg(test)]
mod moxel_exact_parity_tests {
    use super::*;

    #[test]
    fn remaps_leading_source_column_format_row_and_cell_refs() {
        let mut rows = vec![MoxelRow {
            index: 0,
            index_to: None,
            format_index: 5,
            source_format_index: Some(4),
            columns_id: None,
            cells: vec![MoxelCell {
                column_index: 0,
                format_index: 3,
                source_format_index: Some(2),
                text: None,
                parameter: None,
                detail_parameter: None,
                note: None,
                empty_text: false,
            }],
        }];

        remap_moxel_leading_source_column_format_indices(&mut rows);

        assert_eq!(rows[0].format_index, 3);
        assert_eq!(rows[0].cells[0].format_index, 1);
    }

    #[test]
    fn drawing_pattern_color_uses_slot_thirteen_and_native_order() {
        let style_refs = vec![
            Some("style:FormBackColor".to_string()),
            Some("style:FormTextColor".to_string()),
            None,
            Some("style:ИтогиФонГруппы".to_string()),
        ];
        let raw = "{14370,0,3,0,255,0}";
        let mut format = parse_moxel_format(raw, &style_refs, &[]).unwrap();
        let pattern_color = parse_moxel_drawing_pattern_color(raw, &style_refs);
        normalize_moxel_drawing_format_with_pattern_color(&mut format, pattern_color);
        let spreadsheet = MoxelSpreadsheet {
            column_count: 0,
            column_sets: Vec::new(),
            column_formats: Vec::new(),
            extra_formats: BTreeMap::new(),
            default_format_width: None,
            default_format: MoxelFormat::default(),
            formats: vec![format],
            rows: Vec::new(),
            vertical_groups: Vec::new(),
            merges: Vec::new(),
            horizontal_unmerges: Vec::new(),
            vertical_unmerges: Vec::new(),
            named_items: Vec::new(),
            areas: Vec::new(),
            print_area: None,
            print_settings: None,
            lines: Vec::new(),
            fonts: Vec::new(),
            drawings: Vec::new(),
            pictures: Vec::new(),
            empty_headers_footers: false,
            header_footer_format_index: None,
            default_format_index: None,
            source_format_map: None,
            height: 0,
        };
        let mut xml = String::new();
        push_moxel_format_xml(&mut xml, &spreadsheet, 1);

        assert_eq!(
            xml,
            "\t<format>\r\n\
\t\t<drawingBorder>0</drawingBorder>\r\n\
\t\t<borderColor>style:ИтогиФонГруппы</borderColor>\r\n\
\t\t<backColor>style:FormBackColor</backColor>\r\n\
\t\t<patternColor>style:FormBackColor</patternColor>\r\n\
\t\t<pattern>WithoutPattern</pattern>\r\n\
\t</format>\r\n"
        );
    }

    #[test]
    fn drawing_only_line_palette_does_not_emit_cell_lines() {
        let formats = vec![MoxelFormat {
            drawing_border: Some(0),
            ..MoxelFormat::default()
        }];
        let lines = parse_moxel_lines(&["{3,3,{-1}}", "{3,3,{-3}}"], &formats, true);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].style, "None");
        assert_eq!(
            lines[0].line_type,
            "v8ui:SpreadsheetDocumentDrawingLineType"
        );
        assert_eq!(lines[0].width, 1);
    }

    #[test]
    fn preserves_thin_dashed_default_palette_with_drawing_line() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(1),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(2),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                drawing_border: Some(3),
                ..MoxelFormat::default()
            },
        ];
        let lines = parse_moxel_lines(
            &["4", "{3,3,{-1}}", "{3,3,{-3}}", "{3,2,{52}}", "{3,3,{-10}}"],
            &formats,
            true,
        );

        assert_eq!(
            lines
                .iter()
                .map(|line| (line.style, line.line_type, line.width))
                .collect::<Vec<_>>(),
            vec![
                ("ThinDashed", "v8ui:SpreadsheetDocumentCellLineType", 1),
                ("None", "v8ui:SpreadsheetDocumentCellLineType", 0),
                ("Solid", "v8ui:SpreadsheetDocumentCellLineType", 2),
                ("None", "v8ui:SpreadsheetDocumentDrawingLineType", 1),
            ]
        );
    }

    #[test]
    fn ignores_same_palette_sequence_outside_count_prefixed_table() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(1),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                bottom_border: Some(2),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                drawing_border: Some(3),
                ..MoxelFormat::default()
            },
        ];
        let lines = parse_moxel_lines(
            &["0", "{3,3,{-1}}", "{3,3,{-3}}", "{3,2,{52}}", "{3,3,{-10}}"],
            &formats,
            true,
        );

        assert_eq!(
            lines
                .iter()
                .map(|line| (line.style, line.line_type, line.width))
                .collect::<Vec<_>>(),
            vec![
                ("Solid", "v8ui:SpreadsheetDocumentCellLineType", 1),
                ("None", "v8ui:SpreadsheetDocumentCellLineType", 1),
                ("Solid", "v8ui:SpreadsheetDocumentCellLineType", 2),
                ("None", "v8ui:SpreadsheetDocumentDrawingLineType", 1),
            ]
        );
    }

    #[test]
    fn preserves_explicit_terminal_picture_line_break() {
        assert_eq!(
            normalize_moxel_picture_payload("YWJj\r\ndef\r\n"),
            "YWJj\r\ndef\r\n"
        );
        assert_eq!(
            normalize_moxel_picture_payload("YWJj\r\r\ndef\r\r\n"),
            "YWJj\r\ndef\r\n"
        );
        assert_eq!(normalize_moxel_picture_payload("YWJj"), "YWJj");
        assert_eq!(normalize_moxel_picture_payload(""), "");
        assert_eq!(normalize_moxel_picture_payload("\r\n"), "");
    }
}
