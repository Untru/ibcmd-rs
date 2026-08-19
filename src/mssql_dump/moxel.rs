use super::mxl_ir::{
    MxlDiagnostic, MxlFormatReferenceMap, MxlPaletteProvenance, MxlSpreadsheetWritePlan,
};
use super::*;

/// A decoded spreadsheet plus the exact palette/reference plan that the XML
/// projection is allowed to consume.  Keeping this private makes the current
/// slice additive: legacy parser helpers can remain available while production
/// extraction crosses the explicit boundary below.
struct DecodedMoxelSpreadsheet {
    spreadsheet: MoxelSpreadsheet,
    write_plan: MxlSpreadsheetWritePlan,
}

pub(super) struct MoxelSpreadsheet {
    pub(super) column_count: usize,
    /// The document's own language record, the fourth top-level field.
    ///
    /// Evidence (native 1С:УТ 11.5.27.75): all 674 distinct bodies behind the
    /// 683 standalone spreadsheet templates carry
    /// `{"ru","ru",0,1,"ru","Русский","Русский",0}` and publish exactly that
    /// block, so reading the field changes nothing there; the 22 spreadsheet
    /// blocks embedded in forms carry three forms, and the platform publishes
    /// a different thing for each. `None` is a record this reader cannot
    /// spell, which keeps the block it has always written.
    pub(super) language_settings: Option<MoxelLanguageSettings>,
    /// The template-mode flag, the fourteenth top-level field. Two embedded
    /// bodies are byte-equal but for this scalar and the platform's outputs
    /// differ by exactly the presence of `<templateMode>`; every standalone
    /// body stores 1.
    pub(super) template_mode: bool,
    pub(super) column_sets: Vec<MoxelColumnSet>,
    pub(super) column_formats: Vec<MoxelFormat>,
    pub(super) extra_formats: BTreeMap<usize, MoxelFormat>,
    pub(super) default_format_width: Option<usize>,
    /// Font slot carried by the same leading record the default width came
    /// from, published with that width when the default format is materialized.
    pub(super) default_format_font: Option<usize>,
    pub(super) default_format: MoxelFormat,
    pub(super) formats: Vec<MoxelFormat>,
    /// The document's format table in the order the body stores it, before the
    /// column/pool split reorders it.
    ///
    /// A header/footer record names a position in *this* table, not in the
    /// published pool, so the published reference can only be recovered from
    /// the record's own bytes. Empty when the body carries no format table the
    /// reader can spell, which leaves the reference on its previous path.
    pub(super) source_formats: Vec<MoxelFormat>,
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
    pub(super) header_footer_format_index: Option<usize>,
    /// The six header/footer slots in publication order, present only when the
    /// document publishes at least one of them.
    pub(super) header_footer_slots: Option<Vec<Option<MoxelHeaderFooter>>>,
    pub(super) default_format_index: Option<usize>,
    /// The document's leading default-format record, the fifth top-level field
    /// of the MOXCEL body, decoded as a format.
    pub(super) leading_default_format: Option<MoxelFormat>,
    pub(super) source_format_map: Option<MoxelSourceFormatMap>,
    pub(super) height: usize,
    /// Document-level tables the format members at bits 23, 25 and 34 index
    /// into.
    pub(super) value_types: Vec<MoxelValueType>,
    pub(super) control_types: Vec<String>,
    pub(super) mask_refs: Vec<Vec<MoxelLocalizedValue>>,
}

/// A decoded language record.
pub(super) enum MoxelLanguageSettings {
    /// The `#` placeholder: `{"#","",0,1,"#","Язык по умолчанию",…}`, which the
    /// platform answers with no `<languageSettings>` element at all.
    Placeholder,
    /// A named record: the current and default language, then the descriptors.
    Named {
        current: String,
        default: String,
        infos: Vec<MoxelLanguageInfo>,
    },
}

pub(super) struct MoxelLanguageInfo {
    pub(super) id: String,
    pub(super) code: String,
    pub(super) description: String,
}

/// `{current, default, 0, count, (id, code, description) * count, 0}`.
pub(super) fn parse_moxel_language_settings(text: &str) -> Option<MoxelLanguageSettings> {
    let fields = split_1c_braced_fields(text, 0)?;
    let current = parse_1c_string(fields.first()?)?;
    let default = parse_1c_string(fields.get(1)?)?;
    let count = fields.get(3)?.trim().parse::<usize>().ok()?;
    if count > 64 || fields.len() != count * 3 + 5 {
        return None;
    }
    if current == "#" {
        return Some(MoxelLanguageSettings::Placeholder);
    }
    let mut infos = Vec::with_capacity(count);
    for descriptor in fields[4..4 + count * 3].chunks_exact(3) {
        infos.push(MoxelLanguageInfo {
            id: parse_1c_string(descriptor.first()?)?,
            code: parse_1c_string(descriptor.get(1)?)?,
            description: parse_1c_string(descriptor.get(2)?)?,
        });
    }
    Some(MoxelLanguageSettings::Named {
        current,
        default,
        infos,
    })
}

pub(super) struct MoxelSourceFormatMap {
    source_to_internal: Vec<usize>,
    internal_to_source: Vec<usize>,
    output_source_order: Vec<usize>,
}

impl MoxelSourceFormatMap {
    pub(super) fn try_new(
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
    /// The set's default-format reference exactly as the body stores it: a
    /// position in the document's source format table, or 0 for none.
    ///
    /// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet templates): of
    /// the 1810 column sets, the 1734 that store 0 publish no `<formatIndex>`
    /// and the 76 that store anything else all publish one - no exception in
    /// either direction.
    pub(super) raw_default_format_index: usize,
    pub(super) size: usize,
    pub(super) columns: Vec<MoxelColumn>,
}

impl MoxelColumnSet {
    /// The stored reference as an option: `None` is the stored 0, which is the
    /// body's own way of saying the set names no format.
    pub(super) fn source_default_format_index(&self) -> Option<usize> {
        (self.raw_default_format_index > 0).then_some(self.raw_default_format_index)
    }
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
    /// The text list carries a trailing `1`: the platform spells the same
    /// content `<tfl>` instead of `<tl>`.
    pub(super) formatted_text: bool,
    pub(super) parameter: Option<String>,
    pub(super) detail_parameter: Option<String>,
    pub(super) picture_parameter: Option<String>,
    /// Base64 payload of an embedded control, published as `<control>`.
    pub(super) control: Option<String>,
    pub(super) value: Option<MoxelCellValue>,
    pub(super) detail_value: Option<MoxelCellValue>,
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

/// The text child a published header/footer element carries.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 `Templates/*/Ext/Template.xml`
/// that decode as spreadsheets): the record's fourth field is a flag, and it is
/// the only thing that distinguishes the two spellings. `0` closes a four-field
/// record and publishes `<tl>`; `1` opens a fifth field and publishes `<tfl>`.
/// A `{0,ref}` record has no text child at all. All 522 records of the 87
/// publishing documents fall into exactly these three cases.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MoxelHeaderFooterText {
    /// `{0,ref}`: a bare format reference, published without a text child.
    Absent,
    /// `{16,ref,texts,0}`: published as `<tl>`.
    Plain,
    /// `{16,ref,texts,1,formatted}`: published as `<tfl>`.
    Formatted,
}

/// One published header/footer slot.
#[derive(Clone)]
pub(super) struct MoxelHeaderFooter {
    /// The stored format reference. `0` publishes `<f>0</f>` verbatim; a
    /// non-zero reference has to be projected onto the output format table.
    pub(super) source_format_ref: usize,
    pub(super) text_kind: MoxelHeaderFooterText,
    pub(super) text: Vec<MoxelLocalizedValue>,
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
    /// Whether the group is expanded. The record stores the collapsed state, so
    /// this is its complement.
    ///
    /// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet templates): the
    /// 1703 group records split 1693 storing 0 and 10 storing 1, and exactly
    /// the 10 publish `<o>false</o>` - the other 1693 publish no `<o>` at all
    /// and no record in the corpus publishes `<o>true</o>`.
    pub(super) open: bool,
}

#[derive(Clone)]
pub(super) struct MoxelMerge {
    pub(super) row: i32,
    pub(super) column: i32,
    pub(super) height: i32,
    pub(super) width: i32,
    pub(super) columns_id: Option<String>,
}

/// A decoded MOXCEL font descriptor.
///
/// Every optional member is `None` when the descriptor's member mask does not
/// carry it, which is exactly when the platform omits the matching XML
/// attribute. A descriptor that sets no member at all is published as
/// `<font ref="..." kind="StyleItem"/>`.
#[derive(Clone)]
pub(super) struct MoxelFont {
    pub(super) ref_name: Option<String>,
    pub(super) face_name: Option<String>,
    pub(super) height: Option<String>,
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
    pub(super) underline: Option<bool>,
    pub(super) strikeout: Option<bool>,
    pub(super) kind: &'static str,
    pub(super) scale: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MoxelLine {
    pub(super) style: &'static str,
    pub(super) line_type: &'static str,
    pub(super) width: usize,
}

/// Internal provenance carried until the final `MoxelLine` projection.
#[derive(Clone)]
pub(super) struct ResolvedMoxelLine {
    pub(super) line: MoxelLine,
    pub(super) raw_parents: Vec<MoxelRawLineParent>,
    pub(super) transformations: Vec<MoxelLineTransformation>,
    pub(super) format_support: Vec<MoxelLineFormatSupport>,
    pub(super) ambiguous: bool,
    pub(super) fail_closed: bool,
}

impl std::ops::Deref for ResolvedMoxelLine {
    type Target = MoxelLine;

    fn deref(&self) -> &Self::Target {
        &self.line
    }
}

impl std::ops::DerefMut for ResolvedMoxelLine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.line
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MoxelRawLineParent {
    /// Field entry in the raw Moxel payload, never an output palette index.
    pub(super) raw_entry_index: usize,
    /// Entry in the raw line table after non-line payload fields are removed.
    pub(super) line_entry_index: usize,
    /// Exact UTF-8 byte span of the raw field in the native `{8,...}` body.
    pub(super) span_start: usize,
    pub(super) span_end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MoxelLineBorderSlot {
    Border,
    Left,
    Top,
    Right,
    Bottom,
    Drawing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MoxelLineFormatSupport {
    pub(super) format_index: usize,
    pub(super) border_slot: MoxelLineBorderSlot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MoxelLineTransformation {
    Truncated { reason: &'static str },
    DrawingOnlySelectedSource { source_index: usize },
    DefaultShift { reason: &'static str },
    Synthesized { reason: &'static str },
    PostNormalizer { reason: &'static str },
}

/// Receives one owned, final-palette line event.  The parser only materializes
/// this event when a caller opts in, so ordinary extraction retains its former
/// output-only path.
pub(crate) trait MoxelLineTraceSink {
    /// Reserves space before an owned trace event is materialized.  A false
    /// result is terminal for the current trace pass.
    fn try_reserve_event(&self) -> bool {
        true
    }

    fn record_moxel_line(&self, event: MoxelLineTraceEvent);
}

/// Additive, stable JSONL payload for a resolved MXL palette line.  Every
/// field comes from the carried `ResolvedMoxelLine`; it deliberately performs
/// no post-hoc XML/raw matching.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MoxelLineTraceEvent {
    pub output_line_index: usize,
    pub raw_parents: Vec<MoxelRawLineParentTrace>,
    pub transformations: Vec<MoxelLineTransformationTrace>,
    pub format_support: Vec<MoxelLineFormatSupportTrace>,
    pub final_style: &'static str,
    pub final_type: &'static str,
    pub final_width: usize,
    pub final_gap: bool,
    pub ambiguous: bool,
    pub fail_closed: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MoxelRawLineParentTrace {
    pub raw_entry_index: usize,
    pub line_entry_index: usize,
    pub span_start: usize,
    pub span_end: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MoxelLineFormatSupportTrace {
    pub format_index: usize,
    pub border_slot: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MoxelLineTransformationTrace {
    pub kind: &'static str,
    pub reason: Option<&'static str>,
    pub source_index: Option<usize>,
}

impl From<&ResolvedMoxelLine> for MoxelLineTraceEvent {
    fn from(line: &ResolvedMoxelLine) -> Self {
        Self {
            output_line_index: 0,
            raw_parents: line
                .raw_parents
                .iter()
                .map(|parent| MoxelRawLineParentTrace {
                    raw_entry_index: parent.raw_entry_index,
                    line_entry_index: parent.line_entry_index,
                    span_start: parent.span_start,
                    span_end: parent.span_end,
                })
                .collect(),
            transformations: line
                .transformations
                .iter()
                .map(MoxelLineTransformationTrace::from)
                .collect(),
            format_support: line
                .format_support
                .iter()
                .map(|support| MoxelLineFormatSupportTrace {
                    format_index: support.format_index,
                    border_slot: match support.border_slot {
                        MoxelLineBorderSlot::Border => "border",
                        MoxelLineBorderSlot::Left => "left",
                        MoxelLineBorderSlot::Top => "top",
                        MoxelLineBorderSlot::Right => "right",
                        MoxelLineBorderSlot::Bottom => "bottom",
                        MoxelLineBorderSlot::Drawing => "drawing",
                    },
                })
                .collect(),
            final_style: line.style,
            final_type: line.line_type,
            final_width: line.width,
            final_gap: false,
            ambiguous: line.ambiguous,
            fail_closed: line.fail_closed,
        }
    }
}

impl From<&MoxelLineTransformation> for MoxelLineTransformationTrace {
    fn from(transformation: &MoxelLineTransformation) -> Self {
        match transformation {
            MoxelLineTransformation::Truncated { reason } => Self {
                kind: "truncated",
                reason: Some(reason),
                source_index: None,
            },
            MoxelLineTransformation::DrawingOnlySelectedSource { source_index } => Self {
                kind: "drawing_only_selected_source",
                reason: None,
                source_index: Some(*source_index),
            },
            MoxelLineTransformation::DefaultShift { reason } => Self {
                kind: "default_shift",
                reason: Some(reason),
                source_index: None,
            },
            MoxelLineTransformation::Synthesized { reason } => Self {
                kind: "synthesized",
                reason: Some(reason),
                source_index: None,
            },
            MoxelLineTransformation::PostNormalizer { reason } => Self {
                kind: "post_normalizer",
                reason: Some(reason),
                source_index: None,
            },
        }
    }
}

fn trace_final_moxel_lines(
    lines: &[ResolvedMoxelLine],
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) {
    let Some(trace_sink) = trace_sink else {
        return;
    };
    for (output_line_index, resolved) in lines.iter().enumerate() {
        if !trace_sink.try_reserve_event() {
            break;
        }
        let mut event = MoxelLineTraceEvent::from(resolved);
        event.output_line_index = output_line_index;
        trace_sink.record_moxel_line(event);
    }
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
    pub(super) members: MoxelDrawingMembers,
    pub(super) kind: MoxelDrawingKind,
}

/// The optional members the drawing's leading `{mask, formatIndex, ...}` record
/// carries, published between `formatIndex` and `beginRow`.
#[derive(Default)]
pub(super) struct MoxelDrawingMembers {
    pub(super) text: Option<MoxelLocalizedValue>,
    pub(super) parameter: Option<String>,
    pub(super) value: Option<String>,
    pub(super) detail_parameter: Option<String>,
}

pub(super) enum MoxelDrawingKind {
    /// `Line`, `Rectangle` and `Text`: the tail-less record, which publishes no
    /// element after `zOrder` and always publishes `pictureSize` `Stretch`.
    Shape(&'static str),
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
    /// Page geometry is a decimal, not an integer: key 16 stores `60.5` in
    /// three of the corpus's 56 print-settings records, and reading the whole
    /// record through `usize` refused all of them - with them the entire
    /// `<printSettings>` element, twenty members, in every document that
    /// carries that record. The token is published as stored.
    pub(super) page_width: Option<String>,
    pub(super) page_height: Option<String>,
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
    pub(super) contains_value: Option<bool>,
    pub(super) value_type_index: Option<usize>,
    pub(super) control_type_index: Option<usize>,
    pub(super) drawing_border: Option<usize>,
    /// Whether the drawing this format decorates is printed. Member 4 of a
    /// drawing-referenced record, the slot an ordinary record spends on
    /// `bottomBorder`; the platform publishes it first, before `drawingBorder`.
    /// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet documents): three
    /// records publish `<print>`, all three store 1 and all three publish
    /// `false`, and no drawing-referenced record anywhere in the corpus
    /// publishes a `bottomBorder`. The inverted spelling is the one
    /// `protection` (member 16) already carries.
    pub(super) print: Option<bool>,
    /// The four `drawingHave*Border` flags, still packed. Member 3 of a
    /// drawing-referenced record, the slot an ordinary record spends on
    /// `rightBorder`. Evidence: 17 records publish the four flags, our reader
    /// published a `rightBorder` of 2, 0 or 15 for exactly those 17, and no
    /// drawing-referenced record in the corpus publishes a `rightBorder`.
    /// Mask 2 is the one that separates the members, publishing `top` alone, so
    /// `top` weighs 2 and the remaining three follow publication order; masks 0
    /// and 15 cannot separate them and the corpus holds no other value.
    pub(super) drawing_have_borders: Option<usize>,
    /// Member 40, published between `width` and `widthWeightFactor`. Evidence:
    /// nine records publish `<autoWidthCalculation>true</autoWidthCalculation>`
    /// and each stores member 40 as 1 next to the `width` (member 7) and
    /// `widthWeightFactor` (member 41) it is published between; the bit was
    /// the one hole left in the supported-member list between 39 and 41.
    pub(super) auto_width_calculation: Option<bool>,
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
    /// `<mask>` is a localized value, not an enumeration: format member 34
    /// indexes the document's own mask table. Evidence (native 1С:УТ
    /// 11.5.27.75, all 683 MOXCEL spreadsheet templates): 1379 published masks
    /// are the empty `<mask/>` and 37 carry an `<v8:item>` payload - `0`, `1`,
    /// `2`, `3`, `9`, `13`, `999`, `9999`, `9999999999`. Reading member 34 as
    /// an enumeration whose only value was `0 => <mask/>` dropped all 37.
    pub(super) mask_index: Option<usize>,
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
            && self.contains_value.is_none()
            && self.value_type_index.is_none()
            && self.control_type_index.is_none()
            && self.drawing_border.is_none()
            && self.print.is_none()
            && self.drawing_have_borders.is_none()
            && self.auto_width_calculation.is_none()
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
            && self.mask_index.is_none()
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
    let body = crate::compiler::bodies::mxl::decode_compatible_mxl(bytes).map_err(|error| {
        MxlDiagnostic::decoder("mxl.decoder.binary-container", error.to_string())
    })?;
    let decoded = decode_moxel_spreadsheet_ir(body.native_body_text(), object_refs, None)?;
    write_moxel_spreadsheet_xml(&decoded)
}

pub(crate) fn extract_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = crate::compiler::bodies::mxl::decode_compatible_mxl(bytes).ok()?;
    let decoded =
        decode_moxel_spreadsheet_ir(body.native_body_text(), object_refs, trace_sink).ok()?;
    write_moxel_spreadsheet_xml(&decoded).ok()
}

/// Trace an already-inflated native MXL body retained by an offline dump.
/// Its MOXCEL framing is validated by the same codec routine as packed blobs.
pub(crate) fn extract_inflated_moxel_spreadsheet_xml_with_line_trace(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<String> {
    let body = crate::compiler::bodies::mxl::decode_inflated_compatible_mxl(bytes).ok()?;
    let decoded =
        decode_moxel_spreadsheet_ir(body.native_body_text(), object_refs, trace_sink).ok()?;
    write_moxel_spreadsheet_xml(&decoded).ok()
}

/// Builds the bounded canonical hand-off without introducing any XML QName or
/// ordering decision into the decoder. Palette slots and canonical/XML format
/// identity are captured before the writer is called.
fn decode_moxel_spreadsheet_ir(
    text: &str,
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Result<DecodedMoxelSpreadsheet, MxlDiagnostic> {
    let spreadsheet = parse_moxel_spreadsheet_text_with_line_trace(text, object_refs, trace_sink)
        .ok_or_else(|| {
        MxlDiagnostic::decoder(
            "mxl.decoder.canonical-ir",
            "native MOXCEL body could not be decoded into supported spreadsheet IR",
        )
    })?;
    let body = text.trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(body, 0).ok_or_else(|| {
        MxlDiagnostic::decoder(
            "mxl.decoder.canonical-ir",
            "native MOXCEL body has no complete root field list",
        )
    })?;
    let palette = parse_moxel_raw_palette_provenance(&fields, object_refs);
    let write_plan = moxel_spreadsheet_write_plan(&spreadsheet, palette)?;
    Ok(DecodedMoxelSpreadsheet {
        spreadsheet,
        write_plan,
    })
}

fn write_moxel_spreadsheet_xml(decoded: &DecodedMoxelSpreadsheet) -> Result<String, MxlDiagnostic> {
    format_moxel_spreadsheet_xml_with_plan(&decoded.spreadsheet, &decoded.write_plan)
}

fn moxel_spreadsheet_write_plan(
    spreadsheet: &MoxelSpreadsheet,
    palette: MxlPaletteProvenance,
) -> Result<MxlSpreadsheetWritePlan, MxlDiagnostic> {
    let format_count = moxel_output_format_count(spreadsheet);
    let output_format_indices = moxel_output_format_indices(spreadsheet);
    let output_format_index_map = moxel_output_format_index_map(&output_format_indices);
    let format_map = if output_format_indices
        .iter()
        .enumerate()
        .all(|(offset, canonical_index)| *canonical_index == offset + 1)
    {
        MxlFormatReferenceMap::identity(format_count)
    } else {
        let canonical_to_xml = output_format_index_map.clone();
        let xml_to_canonical = output_format_indices
            .iter()
            .enumerate()
            .map(|(offset, canonical_index)| (offset + 1, *canonical_index))
            .collect();
        MxlFormatReferenceMap::explicit(canonical_to_xml, xml_to_canonical)?
    };
    MxlSpreadsheetWritePlan::new(
        palette,
        format_map,
        output_format_indices,
        output_format_index_map,
        format_count,
    )
}

pub(super) fn parse_moxel_spreadsheet_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelSpreadsheet> {
    parse_moxel_spreadsheet_text_with_line_trace(text, object_refs, None)
}

fn parse_moxel_spreadsheet_text_with_line_trace(
    text: &str,
    object_refs: &BTreeMap<String, String>,
    trace_sink: Option<&dyn MoxelLineTraceSink>,
) -> Option<MoxelSpreadsheet> {
    let body = text.trim_start_matches('\u{feff}');
    let spanned_fields = split_1c_braced_fields_with_spans(body, 0)?;
    let fields = spanned_fields
        .iter()
        .map(|(value, _, _)| *value)
        .collect::<Vec<_>>();
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
    let (
        column_sets,
        row_column_ids,
        declared_sheet_height,
        source_column_format_order,
        has_explicit_sparse_column_set_default,
    ) = parse_moxel_column_sets_with_source_format_order(&fields);
    // A row's column-set identity is part of the payload an empty run folds on,
    // so it has to be in place before the fold. Folding first let a run swallow
    // the row that opens a new column set and drop its `<columnsID>` with it.
    for row in &mut rows {
        if let Some(columns_id) = row_column_ids.get(&row.index) {
            row.columns_id = Some(columns_id.clone());
        }
    }
    compact_moxel_empty_row_ranges(&mut rows);
    let fonts = parse_moxel_fonts(&fields, object_refs);
    let pictures = parse_moxel_pictures(&fields, object_refs);
    let style_refs = parse_moxel_style_refs(&fields, object_refs);
    let mut default_format = parse_moxel_default_format(&fields, object_refs);
    let print_settings = parse_moxel_print_settings(&fields);
    let empty_headers_footers = parse_moxel_empty_headers_footers(&fields);
    let header_footer_slots = parse_moxel_header_footer_slots(&fields);
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
    // The leading default-format record writes a font member next to its width;
    // it is kept only while the width in force is still that record's own.
    let default_format_font = fields
        .iter()
        .take(8)
        .find_map(|field| parse_moxel_leading_default_format_record(field))
        .filter(|(width, _)| Some(*width) == default_format_width)
        .map(|(_, font)| font);
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
    // The sheet's own row count is a stored field - the scalar directly behind
    // the default column-set record - not something to be re-derived from the
    // rows, merges and named areas that happen to be present.  Evidence (native
    // 1С:УТ 11.5.27.75, all 683 MOXCEL spreadsheet templates): the declared
    // value equals the published `<height>` in 681 of them and the remaining
    // two declare 0, where the platform publishes `<vgRows>0</vgRows>` and no
    // `<height>` at all - zero counterexamples.  `<vgRows>` never disagrees
    // with `<height>`.  Re-deriving it overshoots wherever the body keeps rows
    // past the sheet's end (367 -> 1091 in
    // `Catalogs/КлассификаторУпаковкиЭПД/.../КлассификаторУпаковки`).
    let height = if zero_column_width_only {
        0
    } else if let Some(declared_sheet_height) = declared_sheet_height {
        declared_sheet_height
    } else {
        moxel_spreadsheet_height(
            &rows,
            &merges,
            &horizontal_unmerges,
            &vertical_unmerges,
            &areas,
        )
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
    // The same table the split above consumed, kept in the order the body
    // stores it: a header/footer record indexes this order, and nothing else in
    // the IR preserves it once the column formats are lifted out.
    let source_formats = parse_moxel_format_table(
        &fields,
        column_format_slots,
        &style_refs,
        &drawing_format_indices,
        &number_format_refs,
    )
    .unwrap_or_default();
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
            source_format_map.as_ref(),
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
    // The document's own line table is authoritative wherever it decodes; the
    // legacy reconstruction below only runs when the slot refuses.
    let resolved_lines = match parse_moxel_line_table(&fields)
        .filter(|lines| moxel_line_table_covers_references(lines, &all_formats))
    {
        Some(lines) => {
            let lines = finalize_moxel_line_slots(
                lines
                    .into_iter()
                    .map(|line| ResolvedMoxelLine {
                        line,
                        raw_parents: Vec::new(),
                        transformations: Vec::new(),
                        format_support: Vec::new(),
                        ambiguous: false,
                        fail_closed: false,
                    })
                    .collect(),
                &all_formats,
            );
            normalize_moxel_report_header_tail_back_color(
                &column_sets,
                &column_formats,
                &lines,
                &mut formats,
            );
            lines
        }
        None => {
            let mut lines = parse_moxel_lines_with_raw_spans(
                &fields,
                &spanned_fields,
                &all_formats,
                has_sparse_column_sets,
            );
            normalize_moxel_single_set_report_header_tail(
                &column_sets,
                &column_formats,
                &mut lines,
                &mut formats,
            );
            lines
        }
    };
    // XML keeps consuming the same projected palette as before.  The optional
    // sink sees exactly this final carried state, before it is projected away.
    trace_final_moxel_lines(&resolved_lines, trace_sink);
    let lines = resolved_lines
        .into_iter()
        .map(|resolved| resolved.line)
        .collect::<Vec<_>>();
    let drawing_max_format_index = drawings
        .iter()
        .map(|drawing| drawing.format_index)
        .max()
        .unwrap_or(0);
    // The highest slot any record actually names. The floor this used to carry
    // named slot 1 even where no column, row, cell, note or drawing names
    // anything, which pushed the materialized default format one slot along and
    // left the slot it skipped in the pool as an unreferenced `<format/>`.
    let row_cell_max_format_index = rows.iter().fold(
        moxel_column_format_slots(&column_sets, column_count),
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
        default_format_font,
        default_format,
        formats,
        source_formats,
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
        header_footer_format_index,
        header_footer_slots,
        default_format_index,
        language_settings: fields
            .get(3)
            .and_then(|field| parse_moxel_language_settings(field)),
        template_mode: !moxel_body_has_fixed_prefix(&fields)
            || fields.get(13).map(|field| field.trim()) != Some("0"),
        leading_default_format: fields
            .get(4)
            .and_then(|field| parse_moxel_format(field, &style_refs, &number_format_refs)),
        source_format_map,
        height,
        value_types: parse_moxel_value_types(&fields, object_refs),
        control_types: parse_moxel_control_types(&fields),
        mask_refs: parse_moxel_mask_refs(&fields),
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
            bold: None,
            italic: None,
            underline: None,
            strikeout: None,
            kind: "StyleItem",
            scale: None,
        },
    );
}

pub(super) fn default_moxel_column_sets(column_count: usize) -> Vec<MoxelColumnSet> {
    vec![MoxelColumnSet {
        id: None,
        default_format_index: None,
        raw_default_format_index: 0,
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
        open: fields.get(4)?.trim().parse::<usize>().ok()? == 0,
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
        raw_default_format_index,
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

/// First top-level field of the header/footer block.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet documents): fields
/// 7 through 12 of the document's top-level list are a run of six brace groups
/// in every single document, and every one of them decodes as a header/footer
/// record. That makes the block an anchor rather than something to search for:
/// the two window scans this replaces returned the same verdict as this anchor
/// on all 683 documents, so nothing that depended on them moves.
const MOXEL_HEADER_FOOTER_BLOCK_START: usize = 7;

/// Block slot -> publication slot, where publication order is `leftHeader`,
/// `centerHeader`, `rightHeader`, `leftFooter`, `centerFooter`, `rightFooter`.
///
/// The block interleaves the two families: slot 0 is the left header, slot 1
/// the left footer, and so on. The left pair is pinned by the 24 documents that
/// publish only slots 0 and 1 (`leftHeader` and `leftFooter`, never a centre or
/// right element), and the header/footer parity of the remaining slots is pinned
/// by the 10 documents whose header records differ from their footer records.
/// The centre/right distinction itself is not observable in this corpus - every
/// document that publishes those four slots carries four equal records - so the
/// natural left-to-right reading is used.
const MOXEL_HEADER_FOOTER_PUBLICATION_ORDER: [usize; 6] = [0, 3, 1, 4, 2, 5];

/// Element names in publication order.
const MOXEL_HEADER_FOOTER_TAGS: [&str; 6] = [
    "leftHeader",
    "centerHeader",
    "rightHeader",
    "leftFooter",
    "centerFooter",
    "rightFooter",
];

/// Whether the body still has its fixed leading block, which is what makes the
/// scalars around it readable by position at all.
///
/// The six header/footer records occupy slots 7..=12 of every native body -
/// all 674 distinct bodies behind the 683 standalone templates and all five
/// behind the embedded blocks. This project's own packer writes a looser
/// layout that puts row data there, so a repacked body must keep the element
/// the writer used to emit unconditionally rather than read a row scalar as
/// the template-mode flag.
pub(super) fn moxel_body_has_fixed_prefix(fields: &[&str]) -> bool {
    fields
        .get(MOXEL_HEADER_FOOTER_BLOCK_START..MOXEL_HEADER_FOOTER_BLOCK_START + 6)
        .is_some_and(|block| {
            block
                .iter()
                .all(|field| parse_moxel_header_footer_record(field).is_some())
        })
}

/// The six header/footer records, in publication order.
///
/// A slot is `None` exactly when its record is `{0,0}`, which is also exactly
/// when the platform omits the element: over all 683 spreadsheet documents the
/// predicate and the published set agree in every one of the 4098 slots.
/// `None` for the whole block means either the block decoded as six empty
/// records or a record refused; either way nothing is published, which is what
/// the platform does for the 596 documents that carry no header or footer.
pub(super) fn parse_moxel_header_footer_slots(
    fields: &[&str],
) -> Option<Vec<Option<MoxelHeaderFooter>>> {
    let block = fields.get(MOXEL_HEADER_FOOTER_BLOCK_START..MOXEL_HEADER_FOOTER_BLOCK_START + 6)?;
    let mut slots = vec![None, None, None, None, None, None];
    let mut any = false;
    for (block_slot, field) in block.iter().enumerate() {
        let record = parse_moxel_header_footer_record(field)?;
        any |= record.is_some();
        slots[MOXEL_HEADER_FOOTER_PUBLICATION_ORDER[block_slot]] = record;
    }
    any.then_some(slots)
}

/// One header/footer record. `Some(None)` is the empty record `{0,0}`.
fn parse_moxel_header_footer_record(text: &str) -> Option<Option<MoxelHeaderFooter>> {
    let fields = split_1c_braced_fields(text, 0)?;
    match fields.first()?.trim() {
        "0" => {
            if fields.len() != 2 {
                return None;
            }
            let source_format_ref = fields.get(1)?.trim().parse::<usize>().ok()?;
            if source_format_ref == 0 {
                return Some(None);
            }
            Some(Some(MoxelHeaderFooter {
                source_format_ref,
                text_kind: MoxelHeaderFooterText::Absent,
                text: Vec::new(),
            }))
        }
        "16" => {
            let source_format_ref = fields.get(1)?.trim().parse::<usize>().ok()?;
            let text = parse_moxel_localized_values(fields.get(2)?)?;
            let text_kind = match (fields.len(), fields.get(3)?.trim()) {
                (4, "0") => MoxelHeaderFooterText::Plain,
                (5, "1") => MoxelHeaderFooterText::Formatted,
                _ => return None,
            };
            Some(Some(MoxelHeaderFooter {
                source_format_ref,
                text_kind,
                text,
            }))
        }
        _ => None,
    }
}

/// The single format reference six `{0,ref}` records share, if that is the shape.
pub(super) fn parse_moxel_uniform_header_footer_format_ref(fields: &[&str]) -> Option<usize> {
    let slots = parse_moxel_header_footer_slots(fields)?;
    let mut refs = slots.iter().map(|slot| {
        slot.as_ref()
            .filter(|record| record.text_kind == MoxelHeaderFooterText::Absent)
            .map(|record| record.source_format_ref)
    });
    let first = refs.next().flatten()?;
    refs.all(|candidate| candidate == Some(first))
        .then_some(first)
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
    source_format_map: Option<&MoxelSourceFormatMap>,
    has_explicit_sparse_column_set_default: bool,
) -> Option<usize> {
    if column_sets.is_empty() {
        return None;
    }
    let header_footer_format_index = header_footer_format_ref.and_then(|source_format_index| {
        source_format_map
            .and_then(|map| map.internal_for_source(source_format_index))
            .or_else(|| {
                moxel_internal_format_index_for_source_index(
                    source_format_index,
                    column_format_len,
                    formats.len(),
                )
            })
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
    _column_sets: &[MoxelColumnSet],
    _print_settings: Option<&MoxelPrintSettings>,
    has_default_format: bool,
    fallback: usize,
) -> Option<usize> {
    if has_default_format {
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

/// `<indexTo>` collapses a run of adjacent cell-less rows that publish the same
/// `<row>` payload.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet documents, 67 707
/// published `rowsItem` elements): 361 of them carry an `indexTo`, and their
/// payloads are `empty` alone (331), `columnsID` + `empty` (14), `columnsID` +
/// `formatIndex` + `empty` (10) and `formatIndex` + `empty` (6) - never a
/// payload with a `<c>` in it.  The converse holds as a set: of the 9 573
/// adjacent pairs whose payloads are byte-equal and which the platform still
/// published separately, every single one carries cells, so "cell-less and equal
/// payload" is both necessary and sufficient over the whole corpus.
///
/// The predicate this replaces also demanded `formatIndex <= 1` and no
/// `columnsID`, which refused the 24 runs the platform collapses with one of
/// those two members present.  `is_moxel_compactable_empty_row` keeps the old
/// spelling because the structurally-empty-sheet guard is calibrated on it.
pub(super) fn compact_moxel_empty_row_ranges(rows: &mut Vec<MoxelRow>) {
    let mut compacted = Vec::with_capacity(rows.len());
    let mut index = 0usize;
    while index < rows.len() {
        let mut row = rows[index].clone();
        if row.cells.is_empty() {
            let mut cursor = index + 1;
            while cursor < rows.len()
                && rows[cursor].index == rows[cursor - 1].index + 1
                && moxel_rows_publish_equal_empty_payload(&row, &rows[cursor])
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

/// Do two cell-less rows render the same `<row>` payload? The payload is exactly
/// `columnsID` plus the projection of the format reference, and the projection
/// reads both the canonical and the source index, so both are compared.
fn moxel_rows_publish_equal_empty_payload(left: &MoxelRow, right: &MoxelRow) -> bool {
    right.cells.is_empty()
        && left.columns_id == right.columns_id
        && left.format_index == right.format_index
        && left.source_format_index == right.source_format_index
}

pub(super) fn is_moxel_compactable_empty_row(row: &MoxelRow) -> bool {
    row.format_index <= 1 && row.columns_id.is_none() && row.cells.is_empty()
}

pub(super) fn parse_moxel_rows(fields: &[&str]) -> Vec<MoxelRow> {
    // The row block sits right behind the header/footer block, and the scalar
    // this scan reads as its first marker is the template-mode flag, not part
    // of the block. Evidence (native 1С:УТ 11.5.27.75): field 14 is the literal
    // `2` in all 683 standalone bodies and in all five distinct blocks embedded
    // in forms, and field 15 is the stored row count; field 13 is `1` in every
    // standalone body but `0` in three of the five embedded ones. Requiring `1`
    // there therefore refused the anchor on exactly the bodies whose
    // template mode is off, and the scan then anchored somewhere else.
    let anchored_row_block =
        moxel_body_has_fixed_prefix(fields).then_some(MOXEL_HEADER_FOOTER_BLOCK_START + 6);
    let mut best_rows = Vec::new();
    for index in 3..fields.len().saturating_sub(3) {
        if fields.get(index + 1).map(|field| field.trim()) != Some("2")
            || (Some(index) != anchored_row_block
                && fields.get(index).map(|field| field.trim()) != Some("1"))
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

/// Member-mask bits of a MOXCEL cell record.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 `Templates/*/Ext/Template.xml`
/// that decode as spreadsheets): the 2 025 751 cell records use the fifteen
/// masks 0, 1, 2, 4, 8, 16, 20, 24, 28, 32, 48, 52, 56, 64 and 88 and no other,
/// and every record's field count is exactly two plus the widths of the members
/// its mask names. The reader used to key off whole mask values and recognised
/// only 0, 8, 16, 24, 48 and 56, so a cell carrying any other member was read
/// against the wrong slots: the mask is a member set, not a shape name.
const MOXCEL_CELL_CONTROL_BIT: usize = 0;
const MOXCEL_CELL_VALUE_BIT: usize = 1;
const MOXCEL_CELL_DETAIL_VALUE_BIT: usize = 2;
const MOXCEL_CELL_DETAIL_PARAMETER_BIT: usize = 3;
const MOXCEL_CELL_TEXT_BIT: usize = 4;
const MOXCEL_CELL_NOTE_BIT: usize = 5;
const MOXCEL_CELL_PICTURE_PARAMETER_BIT: usize = 6;
const MOXCEL_CELL_KNOWN_MASK: usize = (1 << MOXCEL_CELL_CONTROL_BIT)
    | (1 << MOXCEL_CELL_VALUE_BIT)
    | (1 << MOXCEL_CELL_DETAIL_VALUE_BIT)
    | (1 << MOXCEL_CELL_DETAIL_PARAMETER_BIT)
    | (1 << MOXCEL_CELL_TEXT_BIT)
    | (1 << MOXCEL_CELL_NOTE_BIT)
    | (1 << MOXCEL_CELL_PICTURE_PARAMETER_BIT);

/// One typed value stored in a cell member.
///
/// Evidence: the corpus stores five spellings — `{"U"}`, `{"S",text}`,
/// `{"N",number}`, `{"D",yyyymmddhhmmss}` and `{"#",type,{index}}` — and the
/// platform publishes them as `xsi:nil`, `xs:string`, `xs:decimal`,
/// `xs:dateTime` and a bare `<r>` reference respectively.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MoxelCellValue {
    Nil,
    Text(String),
    Number(String),
    DateTime(String),
    Reference(usize),
}

pub(super) fn parse_moxel_cell_value(text: &str) -> Option<MoxelCellValue> {
    let fields = split_1c_braced_fields(text, 0)?;
    match (parse_1c_string(fields.first()?)?.as_str(), fields.len()) {
        ("U", 1) => Some(MoxelCellValue::Nil),
        ("S", 2) => Some(MoxelCellValue::Text(parse_1c_string(fields.get(1)?)?)),
        ("N", 2) => {
            let number = fields.get(1)?.trim();
            number
                .chars()
                .all(|character| character.is_ascii_digit() || character == '-' || character == '.')
                .then(|| MoxelCellValue::Number(number.to_string()))
        }
        ("D", 2) => {
            let stamp = fields.get(1)?.trim();
            (stamp.len() == 14 && stamp.chars().all(|character| character.is_ascii_digit()))
                .then(|| MoxelCellValue::DateTime(stamp.to_string()))
        }
        ("#", 3) => split_1c_braced_fields(fields.get(2)?, 0)
            .filter(|reference| reference.len() == 1)
            .and_then(|reference| reference.first()?.trim().parse::<usize>().ok())
            .map(MoxelCellValue::Reference),
        _ => None,
    }
}

/// The base64 payload of an embedded control blob.
///
/// Evidence: the stored payload keeps its own line structure, each break
/// written as a carriage return ahead of the body's own newline; the platform
/// republishes exactly those lines joined by CRLF, including a trailing break
/// when the payload stores one. Re-wrapping at a fixed width instead loses the
/// three documents whose payload ends on a break.
fn parse_moxel_cell_control(text: &str) -> Option<String> {
    let inner = text.trim_end().strip_prefix('{')?.strip_suffix('}')?;
    let payload = inner.strip_prefix("#base64:")?;
    let mut lines = Vec::new();
    for line in payload.split('\n') {
        let line = line.trim_end_matches('\r');
        if !line.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '+'
                || character == '/'
                || character == '='
        }) {
            return None;
        }
        lines.push(line);
    }
    (!lines.is_empty() && !lines[0].is_empty()).then(|| lines.join("\r\n"))
}

/// Decodes one cell record against its member mask.
///
/// Storage order is fixed and is not the publication order: the control blob,
/// the value, the detail value, the detail parameter and the picture parameter
/// come first, then the cell's text list, then the note triple, and the text
/// list's own trailing "formatted" flag closes the record. All 2 025 751
/// records of the corpus are consumed exactly by this walk.
pub(super) fn parse_moxel_cell(text: &str, column_index: usize) -> Option<MoxelCell> {
    let fields = split_1c_braced_fields(text, 0)?;
    let mask = fields.first()?.trim().parse::<usize>().ok()?;
    if mask & !MOXCEL_CELL_KNOWN_MASK != 0 {
        return None;
    }
    let has = |bit: usize| mask & (1 << bit) != 0;
    let format_index = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|value| if value == 0 { 0 } else { value + 1 })
        .unwrap_or(0);
    let mut cursor = 2usize;
    let mut take = |width: usize| {
        let at = cursor;
        cursor += width;
        at
    };
    let control_at = has(MOXCEL_CELL_CONTROL_BIT).then(|| take(2));
    let value_at = has(MOXCEL_CELL_VALUE_BIT).then(|| take(1));
    let detail_value_at = has(MOXCEL_CELL_DETAIL_VALUE_BIT).then(|| take(1));
    let detail_parameter_at = has(MOXCEL_CELL_DETAIL_PARAMETER_BIT).then(|| take(1));
    let picture_parameter_at = has(MOXCEL_CELL_PICTURE_PARAMETER_BIT).then(|| take(1));
    let text_at = has(MOXCEL_CELL_TEXT_BIT).then(|| take(1));
    let note_at = has(MOXCEL_CELL_NOTE_BIT).then(|| take(3));
    let formatted_flag_at = has(MOXCEL_CELL_TEXT_BIT).then(|| take(1));
    let mut expected = cursor;
    let formatted = match formatted_flag_at.and_then(|at| fields.get(at)) {
        None => false,
        Some(flag) => match flag.trim() {
            "0" => false,
            "1" => {
                expected += 1;
                true
            }
            _ => return None,
        },
    };
    if fields.len() != expected {
        return None;
    }

    let control = match control_at {
        None => None,
        Some(at) => {
            if fields.get(at)?.trim() != "1" {
                return None;
            }
            Some(parse_moxel_cell_control(fields.get(at + 1)?)?)
        }
    };
    let value = match value_at {
        None => None,
        Some(at) => Some(parse_moxel_cell_value(fields.get(at)?)?),
    };
    let detail_value = match detail_value_at {
        None => None,
        Some(at) => Some(parse_moxel_cell_value(fields.get(at)?)?),
    };
    let detail_parameter = match detail_parameter_at {
        None => None,
        Some(at) => Some(parse_1c_string(fields.get(at)?)?),
    };
    let picture_parameter = match picture_parameter_at {
        None => None,
        Some(at) => Some(parse_1c_string(fields.get(at)?)?),
    };
    // The member walk is strict about which slots exist; the contents of the
    // text list and of the note are read as before, so a member this reader
    // cannot spell out is dropped rather than costing the cell.
    let localized = text_at.and_then(|at| parse_moxel_localized_cell_value(fields.get(at)?));
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
    let note = note_at.and_then(|at| parse_moxel_cell_note(&fields, at));
    Some(MoxelCell {
        column_index,
        format_index,
        source_format_index: if format_index == 0 {
            None
        } else {
            Some(format_index)
        },
        text,
        formatted_text: formatted,
        parameter,
        detail_parameter,
        picture_parameter,
        control,
        value,
        detail_value,
        note,
        empty_text,
    })
}

/// Decodes the note member, whose three fields start at `note_text_index`.
fn parse_moxel_cell_note(fields: &[&str], note_text_index: usize) -> Option<MoxelNote> {
    if fields.get(note_text_index + 1)?.trim() != "1" {
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

pub(super) fn parse_moxel_fonts(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<MoxelFont> {
    fields
        .iter()
        .filter_map(|field| parse_moxel_font(field, object_refs))
        .collect()
}

/// Member-mask bits of a MOXCEL font descriptor.
///
/// Evidence: the 3 143 font descriptors of the 674 distinct MOXCEL bodies
/// behind the 683 spreadsheet templates of 1С:УТ 11.5.27.75 use bits 0..=5 and
/// bit 9 and no other bit, and the variable-length forms carry exactly one
/// field per member bit. A member the mask omits is a member the platform omits
/// from the XML: reconstructing every published `<font>` element from these
/// bits reproduces all 683 published tables with no counterexample.
const MOXCEL_FONT_FACE_NAME_BIT: usize = 0;
const MOXCEL_FONT_HEIGHT_BIT: usize = 1;
const MOXCEL_FONT_WEIGHT_BIT: usize = 2;
const MOXCEL_FONT_ITALIC_BIT: usize = 3;
const MOXCEL_FONT_UNDERLINE_BIT: usize = 4;
const MOXCEL_FONT_STRIKEOUT_BIT: usize = 5;
const MOXCEL_FONT_SCALE_BIT: usize = 9;
const MOXCEL_FONT_KNOWN_MASK: usize = (1 << MOXCEL_FONT_FACE_NAME_BIT)
    | (1 << MOXCEL_FONT_HEIGHT_BIT)
    | (1 << MOXCEL_FONT_WEIGHT_BIT)
    | (1 << MOXCEL_FONT_ITALIC_BIT)
    | (1 << MOXCEL_FONT_UNDERLINE_BIT)
    | (1 << MOXCEL_FONT_STRIKEOUT_BIT)
    | (1 << MOXCEL_FONT_SCALE_BIT);
/// The one mask an absolute descriptor carries; it fills every slot anyway.
const MOXCEL_ABSOLUTE_FONT_MASK: usize = MOXCEL_FONT_KNOWN_MASK;
/// Members appear in slot order, which puts the face name last.
const MOXCEL_FONT_MEMBER_ORDER: [usize; 6] = [
    MOXCEL_FONT_HEIGHT_BIT,
    MOXCEL_FONT_WEIGHT_BIT,
    MOXCEL_FONT_ITALIC_BIT,
    MOXCEL_FONT_UNDERLINE_BIT,
    MOXCEL_FONT_STRIKEOUT_BIT,
    MOXCEL_FONT_FACE_NAME_BIT,
];
const MOXCEL_FONT_BOLD_WEIGHT: usize = 700;

fn moxel_font_mask_has(mask: usize, bit: usize) -> bool {
    mask >> bit & 1 == 1
}

/// Style items a font descriptor can name by predefined index.
fn moxel_predefined_font_style_ref(index: &str) -> Option<&'static str> {
    match index {
        "-20" => Some("style:TextFont"),
        "-30" => Some("style:SmallTextFont"),
        "-31" => Some("style:NormalTextFont"),
        "-32" => Some("style:LargeTextFont"),
        "-33" => Some("style:ExtraLargeTextFont"),
        _ => None,
    }
}

/// System fonts a Windows-font descriptor can name by index.
fn moxel_system_font_ref(index: &str) -> Option<&'static str> {
    match index {
        "0" => Some("sys:DefaultGUIFont"),
        "2" => Some("sys:ANSIFixedFont"),
        _ => None,
    }
}

pub(super) fn parse_moxel_font(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelFont> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    match fields.get(1)?.trim() {
        // An absolute descriptor writes every slot, so its members are read at
        // fixed offsets; only the one observed mask is admitted.
        "0" if fields.len() == 19 => {
            if fields.get(2)?.trim().parse::<usize>().ok()? != MOXCEL_ABSOLUTE_FONT_MASK {
                return None;
            }
            let height_raw = fields.get(3)?.trim().parse::<usize>().ok()?;
            let weight = fields.get(7)?.trim().parse::<usize>().ok()?;
            Some(MoxelFont {
                ref_name: None,
                face_name: Some(parse_1c_string(fields.get(16)?)?),
                height: Some(format_moxel_font_height(height_raw)),
                bold: Some(weight >= MOXCEL_FONT_BOLD_WEIGHT),
                italic: Some(fields.get(8)?.trim() != "0"),
                underline: Some(fields.get(9)?.trim() != "0"),
                strikeout: Some(fields.get(10)?.trim() != "0"),
                kind: "Absolute",
                scale: Some(fields.get(18)?.trim().parse::<usize>().ok()?),
            })
        }
        kind @ ("1" | "2") => {
            let reference = split_1c_braced_fields(fields.get(3)?, 0)?;
            let (ref_name, kind) = if kind == "1" {
                if reference.len() != 1 {
                    return None;
                }
                (
                    moxel_system_font_ref(reference.first()?.trim())?.to_string(),
                    "WindowsFont",
                )
            } else {
                let style_ref = match reference.len() {
                    1 => moxel_predefined_font_style_ref(reference.first()?.trim())?.to_string(),
                    2 if reference.first()?.trim() == "0" => {
                        let uuid = parse_uuid_field(reference.get(1)?.trim())?;
                        object_refs
                            .get(&uuid)
                            .and_then(|reference| reference.strip_prefix("StyleItem."))
                            .map(|name| format!("style:{name}"))?
                    }
                    _ => return None,
                };
                (style_ref, "StyleItem")
            };
            let mask = fields.get(2)?.trim().parse::<usize>().ok()?;
            if mask & !MOXCEL_FONT_KNOWN_MASK != 0 {
                return None;
            }
            let members = MOXCEL_FONT_MEMBER_ORDER
                .iter()
                .copied()
                .filter(|bit| moxel_font_mask_has(mask, *bit))
                .collect::<Vec<_>>();
            // Four framing fields, one field per member bit, then the two
            // trailing slots the descriptor always carries.
            if fields.len() != members.len() + 6 || fields.get(fields.len() - 2)?.trim() != "1" {
                return None;
            }
            let mut face_name = None;
            let mut height = None;
            let mut bold = None;
            let mut italic = None;
            let mut underline = None;
            let mut strikeout = None;
            for (bit, value) in members.iter().copied().zip(fields.iter().skip(4)) {
                match bit {
                    MOXCEL_FONT_FACE_NAME_BIT => face_name = Some(parse_1c_string(value)?),
                    MOXCEL_FONT_HEIGHT_BIT => {
                        height = Some(format_moxel_font_height(
                            value.trim().parse::<usize>().ok()?,
                        ));
                    }
                    MOXCEL_FONT_WEIGHT_BIT => {
                        bold = Some(value.trim().parse::<usize>().ok()? >= MOXCEL_FONT_BOLD_WEIGHT);
                    }
                    MOXCEL_FONT_ITALIC_BIT => italic = Some(value.trim() != "0"),
                    MOXCEL_FONT_UNDERLINE_BIT => underline = Some(value.trim() != "0"),
                    MOXCEL_FONT_STRIKEOUT_BIT => strikeout = Some(value.trim() != "0"),
                    _ => return None,
                }
            }
            let scale = if moxel_font_mask_has(mask, MOXCEL_FONT_SCALE_BIT) {
                Some(fields.last()?.trim().parse::<usize>().ok()?)
            } else {
                None
            };
            Some(MoxelFont {
                ref_name: Some(ref_name),
                face_name,
                height,
                bold,
                italic,
                underline,
                strikeout,
                kind,
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

fn resolved_moxel_line_from_parents(
    parents: &[ResolvedMoxelLine],
    line: MoxelLine,
    transformation: MoxelLineTransformation,
) -> ResolvedMoxelLine {
    let raw_parents = parents
        .iter()
        .flat_map(|parent| parent.raw_parents.iter().copied())
        .collect::<Vec<_>>();
    let mut transformations = parents
        .iter()
        .flat_map(|parent| parent.transformations.iter().cloned())
        .collect::<Vec<_>>();
    transformations.push(transformation);
    ResolvedMoxelLine {
        line,
        raw_parents,
        transformations,
        format_support: Vec::new(),
        ambiguous: false,
        fail_closed: false,
    }
}

fn finalize_moxel_line_slots(
    mut lines: Vec<ResolvedMoxelLine>,
    formats: &[MoxelFormat],
) -> Vec<ResolvedMoxelLine> {
    for (output_slot, line) in lines.iter_mut().enumerate() {
        line.format_support = moxel_line_format_support(formats, output_slot);
    }
    lines
}

#[cfg(test)]
pub(super) fn parse_moxel_lines(
    fields: &[&str],
    formats: &[MoxelFormat],
    shift_default_line_styles: bool,
) -> Vec<ResolvedMoxelLine> {
    parse_moxel_lines_with_raw_spans(fields, &[], formats, shift_default_line_styles)
}

/// Root slot of the MOXCEL line table.  It sits directly behind the language
/// settings and the column-size descriptor and holds the document's shared
/// `<line>` resources verbatim.
const MOXEL_LINE_TABLE_FIELD: usize = 5;
/// Line-kind identities carried by every line descriptor.
const MOXEL_CELL_LINE_KIND: &str = "f527dc88-1d39-40b3-bcbb-d98b690ead68";
const MOXEL_DRAWING_LINE_KIND: &str = "b7438842-27cc-42a3-846f-2250cd9c1bc3";
/// A shared line table is a document-level resource, not per-cell data.
const MAX_MOXEL_LINE_TABLE_ENTRIES: usize = 2048;
const MAX_MOXEL_LINE_WIDTH: usize = 1024;

/// Decodes the document's shared line table from its own root slot.
///
/// Shape: `{count, (1, {4,0,{0},style,width,0,kind,0}, 0) * count}`.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 604 spreadsheet templates the dump
/// emits): the declared count equals the published `<line>` count in every one
/// of the 604 documents — 0 mismatches — and the 1296 descriptor/line pairs
/// collapse onto 15 distinct descriptors, each mapping to exactly one published
/// line. `width` is the published width and `style`/`kind` decode as below.
/// A descriptor that does not match this shape is refused rather than guessed.
///
/// The style code is read against the descriptor's own line kind: the two
/// `v8ui` enumerations do not share an ordering. Re-measured over all 683
/// spreadsheet templates (1495 descriptor/line pairs, 0 count mismatches),
/// cell lines publish 0 None, 1 Solid, 2 Dotted, 3 Double, 4 ThinDashed,
/// 5 ThickDashed, 6 LargeDashed - while drawing lines publish 0 None (253),
/// 1 Solid (2) and **3 Dotted** (29). Reading 3 as the cell enum's `Double`
/// mislabelled every one of those 29 drawing lines. No other drawing code
/// occurs in the corpus, so any other pairing is a typed refusal rather than
/// a guess at the rest of that enumeration.
pub(super) fn parse_moxel_line_table(fields: &[&str]) -> Option<Vec<MoxelLine>> {
    let entries = split_1c_braced_fields(fields.get(MOXEL_LINE_TABLE_FIELD)?, 0)?;
    let count = entries.first()?.trim().parse::<usize>().ok()?;
    if count > MAX_MOXEL_LINE_TABLE_ENTRIES || entries.len() != count * 3 + 1 {
        return None;
    }
    let mut lines = Vec::with_capacity(count);
    for entry in entries.get(1..)?.chunks_exact(3) {
        if entry.first()?.trim() != "1" || entry.get(2)?.trim() != "0" {
            return None;
        }
        let descriptor = split_1c_braced_fields(entry.get(1)?, 0)?;
        if descriptor.len() != 8
            || descriptor.first()?.trim() != "4"
            || descriptor.get(1)?.trim() != "0"
            || descriptor.get(2)?.trim() != "{0}"
            || descriptor.get(5)?.trim() != "0"
            || descriptor.get(7)?.trim() != "0"
        {
            return None;
        }
        let width = descriptor.get(4)?.trim().parse::<usize>().ok()?;
        if width > MAX_MOXEL_LINE_WIDTH {
            return None;
        }
        let kind = descriptor.get(6)?.trim();
        let line_type = match kind {
            MOXEL_CELL_LINE_KIND => "v8ui:SpreadsheetDocumentCellLineType",
            MOXEL_DRAWING_LINE_KIND => "v8ui:SpreadsheetDocumentDrawingLineType",
            _ => return None,
        };
        let style = match (kind, descriptor.get(3)?.trim()) {
            (MOXEL_CELL_LINE_KIND, "0") => "None",
            (MOXEL_CELL_LINE_KIND, "1") => "Solid",
            (MOXEL_CELL_LINE_KIND, "2") => "Dotted",
            (MOXEL_CELL_LINE_KIND, "3") => "Double",
            (MOXEL_CELL_LINE_KIND, "4") => "ThinDashed",
            (MOXEL_CELL_LINE_KIND, "5") => "ThickDashed",
            (MOXEL_CELL_LINE_KIND, "6") => "LargeDashed",
            (MOXEL_DRAWING_LINE_KIND, "0") => "None",
            (MOXEL_DRAWING_LINE_KIND, "1") => "Solid",
            (MOXEL_DRAWING_LINE_KIND, "3") => "Dotted",
            _ => return None,
        };
        lines.push(MoxelLine {
            style,
            line_type,
            width,
        });
    }
    Some(lines)
}

/// A line table that does not cover every reference the format table makes is
/// not the table those formats were written against.
///
/// Native bodies never disagree here: in all 604 spreadsheet templates the
/// referenced slots are exactly `0..count`. Bodies this project packs itself
/// still carry their lines in the legacy palette slot, so the check routes
/// them back to the reconstruction path instead of dropping their lines.
fn moxel_line_table_covers_references(lines: &[MoxelLine], formats: &[MoxelFormat]) -> bool {
    moxel_used_line_indexes(formats)
        .iter()
        .all(|line_index| *line_index < lines.len())
}

fn parse_moxel_lines_with_raw_spans(
    fields: &[&str],
    raw_spans: &[(&str, usize, usize)],
    formats: &[MoxelFormat],
    shift_default_line_styles: bool,
) -> Vec<ResolvedMoxelLine> {
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
        .enumerate()
        .filter_map(|(raw_entry_index, field)| {
            parse_moxel_line(field).map(|line| (raw_entry_index, field.len(), line))
        })
        .enumerate()
        .map(
            |(line_entry_index, (raw_entry_index, raw_len, line))| ResolvedMoxelLine {
                line,
                raw_parents: vec![MoxelRawLineParent {
                    raw_entry_index,
                    line_entry_index,
                    span_start: raw_spans
                        .get(raw_entry_index)
                        .map(|(_, start, _)| *start)
                        .unwrap_or(0),
                    span_end: raw_spans
                        .get(raw_entry_index)
                        .map(|(_, _, end)| *end)
                        .unwrap_or(raw_len),
                }],
                transformations: Vec::new(),
                format_support: Vec::new(),
                ambiguous: false,
                fail_closed: false,
            },
        )
        .collect::<Vec<_>>();
    if uses_drawing_line
        && !uses_cell_line
        && used_indexes.len() == 1
        && let Some(source_index) = used_indexes.iter().next().copied()
        && let Some(source) = lines.get(source_index)
    {
        return finalize_moxel_line_slots(
            vec![resolved_moxel_line_from_parents(
                std::slice::from_ref(source),
                MoxelLine {
                    style: source.style,
                    line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                    width: source.width,
                },
                MoxelLineTransformation::DrawingOnlySelectedSource { source_index },
            )],
            formats,
        );
    }
    if lines.len() > 3
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && lines.get(2).is_some_and(|line| line.style == "Dotted")
    {
        let discarded = lines.split_off(3);
        if !discarded.is_empty() {
            for line in &mut lines {
                line.transformations
                    .push(MoxelLineTransformation::Truncated {
                        reason: "default palette tail after None/Solid/Dotted",
                    });
            }
        }
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
        let discarded = lines.split_off(expected_line_slots);
        if !discarded.is_empty() {
            for line in &mut lines {
                line.transformations
                    .push(MoxelLineTransformation::Truncated {
                        reason: "unused raw palette tail",
                    });
            }
        }
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
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..1],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "None/Solid expanded to four cell defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[1..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 3,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "None/Solid expanded to four cell defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 2,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "None/Solid expanded to four cell defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "None/Solid expanded to four cell defaults",
                    },
                ),
            ],
            formats,
        );
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
            return finalize_moxel_line_slots(
                vec![
                    resolved_moxel_line_from_parents(
                        &lines[0..3],
                        MoxelLine {
                            style: "ThinDashed",
                            line_type: "v8ui:SpreadsheetDocumentCellLineType",
                            width: 1,
                        },
                        MoxelLineTransformation::Synthesized {
                            reason: "thin dashed drawing palette",
                        },
                    ),
                    resolved_moxel_line_from_parents(
                        &lines[0..3],
                        MoxelLine {
                            style: "None",
                            line_type: "v8ui:SpreadsheetDocumentCellLineType",
                            width: 0,
                        },
                        MoxelLineTransformation::Synthesized {
                            reason: "thin dashed drawing palette",
                        },
                    ),
                    resolved_moxel_line_from_parents(
                        &lines[0..3],
                        MoxelLine {
                            style: "Solid",
                            line_type: "v8ui:SpreadsheetDocumentCellLineType",
                            width: 2,
                        },
                        MoxelLineTransformation::DefaultShift {
                            reason: "thin dashed drawing palette",
                        },
                    ),
                    resolved_moxel_line_from_parents(
                        &lines[0..3],
                        MoxelLine {
                            style: "None",
                            line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                            width: 1,
                        },
                        MoxelLineTransformation::Synthesized {
                            reason: "thin dashed drawing palette",
                        },
                    ),
                ],
                formats,
            );
        }
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..3],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "drawing palette defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..3],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "drawing palette defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..3],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 2,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "drawing palette defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..3],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "drawing palette defaults",
                    },
                ),
            ],
            formats,
        );
    }
    if uses_drawing_line
        && lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
    {
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..1],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "drawing palette cell default",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[1..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "drawing palette cell default",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentDrawingLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "drawing palette line default",
                    },
                ),
            ],
            formats,
        );
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
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[1..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "three-entry default shift",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[2..3],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 2,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "three-entry default shift",
                    },
                ),
            ],
            formats,
        );
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
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..1],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "two-entry default shift",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[1..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 2,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "two-entry default shift",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 3,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "two-entry default shift",
                    },
                ),
            ],
            formats,
        );
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
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 2,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "unshifted two-entry defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "unshifted two-entry defaults",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[0..2],
                    MoxelLine {
                        style: "None",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 0,
                    },
                    MoxelLineTransformation::Synthesized {
                        reason: "unshifted two-entry defaults",
                    },
                ),
            ],
            formats,
        );
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && shift_default_line_styles
        && used_indexes.len() == 2
        && used_indexes.contains(&0)
        && used_indexes.contains(&1)
    {
        return finalize_moxel_line_slots(
            vec![
                resolved_moxel_line_from_parents(
                    &lines[0..1],
                    MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "two-line default shift",
                    },
                ),
                resolved_moxel_line_from_parents(
                    &lines[1..2],
                    MoxelLine {
                        style: "Dotted",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    MoxelLineTransformation::DefaultShift {
                        reason: "two-line default shift",
                    },
                ),
            ],
            formats,
        );
    }
    if lines.len() >= 2
        && lines.first().is_some_and(|line| line.style == "None")
        && lines.get(1).is_some_and(|line| line.style == "Solid")
        && used_indexes.len() == 1
        && used_indexes.contains(&0)
    {
        return finalize_moxel_line_slots(
            vec![resolved_moxel_line_from_parents(
                &lines[0..1],
                MoxelLine {
                    style: "Solid",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 1,
                },
                MoxelLineTransformation::DefaultShift {
                    reason: "single default line",
                },
            )],
            formats,
        );
    }
    if !lines.is_empty() {
        return finalize_moxel_line_slots(lines, formats);
    }
    finalize_moxel_line_slots(
        vec![ResolvedMoxelLine {
            line: MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            raw_parents: Vec::new(),
            transformations: vec![MoxelLineTransformation::Synthesized {
                reason: "missing raw palette",
            }],
            format_support: Vec::new(),
            ambiguous: true,
            fail_closed: true,
        }],
        formats,
    )
}

fn moxel_line_format_support(
    formats: &[MoxelFormat],
    line_index: usize,
) -> Vec<MoxelLineFormatSupport> {
    let mut support = Vec::new();
    for (format_index, format) in formats.iter().enumerate() {
        for (value, border_slot) in [
            (format.border, MoxelLineBorderSlot::Border),
            (format.left_border, MoxelLineBorderSlot::Left),
            (format.top_border, MoxelLineBorderSlot::Top),
            (format.right_border, MoxelLineBorderSlot::Right),
            (format.bottom_border, MoxelLineBorderSlot::Bottom),
            (format.drawing_border, MoxelLineBorderSlot::Drawing),
        ] {
            if value == Some(line_index) {
                support.push(MoxelLineFormatSupport {
                    format_index,
                    border_slot,
                });
            }
        }
    }
    support
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

/// `<zOrder>` is not stored in the drawing record - the fourteen fields are
/// fully accounted for by format, kind, geometry, id, and the kind-specific
/// tail - it is the drawing's own position in the sequence.  Evidence (native
/// 1С:УТ 11.5.27.75, all 1428 `Templates/*/Ext/Template.xml`): 271 documents
/// publish 695 drawings and every one of them writes its 1-based ordinal, with
/// no gaps and no repeats, while the `<id>` beside it skips freely (3, 4, 5, 6,
/// 7, 10, 11 ... in `CommonTemplates/ОшибкиОтчетовСПАРКРиски`).
pub(super) fn parse_moxel_drawings(fields: &[&str]) -> Vec<MoxelDrawing> {
    let mut drawings = Vec::new();
    for field in fields {
        let Some(mut drawing) = parse_moxel_drawing(field) else {
            continue;
        };
        drawing.z_order = drawings.len() + 1;
        drawings.push(drawing);
    }
    drawings
}

/// Members the drawing's leading record may carry, keyed by the mask bit that
/// selects them.  The record is the same `{mask, index, ...values}` grammar the
/// cell record uses, so the slots appear in ascending bit order and the mask is
/// the only thing that says how many there are.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet documents, 695
/// drawings): the observed masks are 0 (597 records, no member), 8 (15), 16
/// (65), 24 (9) and 26 (9), and each one accounts for the record's arity
/// exactly - bit 1 and bit 3 take one slot each, bit 4 takes two.  The old code
/// required the mask to be literally `0` with arity 2, which refused all 98
/// records that carry a member.
const MOXEL_DRAWING_MEMBER_VALUE: usize = 1 << 1;
const MOXEL_DRAWING_MEMBER_DETAIL_PARAMETER: usize = 1 << 3;
const MOXEL_DRAWING_MEMBER_LOCALIZED: usize = 1 << 4;
const MOXEL_DRAWING_MEMBER_MASK: usize = MOXEL_DRAWING_MEMBER_VALUE
    | MOXEL_DRAWING_MEMBER_DETAIL_PARAMETER
    | MOXEL_DRAWING_MEMBER_LOCALIZED;

/// The drawing's leading record: `{mask, formatIndex, ...members}`.
fn parse_moxel_drawing_format_record(text: &str) -> Option<(usize, MoxelDrawingMembers)> {
    let fields = split_1c_braced_fields(text, 0)?;
    let mask = fields.first()?.trim().parse::<usize>().ok()?;
    if mask & !MOXEL_DRAWING_MEMBER_MASK != 0 {
        return None;
    }
    let format_index = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut members = MoxelDrawingMembers::default();
    let mut cursor = 2usize;
    if mask & MOXEL_DRAWING_MEMBER_VALUE != 0 {
        // A typed value; only the string tag `S` is observed (9 records).
        let value = split_1c_braced_fields(fields.get(cursor)?, 0)?;
        if value.len() != 2 || parse_1c_string(value.first()?)?.as_str() != "S" {
            return None;
        }
        members.value = Some(parse_1c_string(value.get(1)?)?);
        cursor += 1;
    }
    if mask & MOXEL_DRAWING_MEMBER_DETAIL_PARAMETER != 0 {
        members.detail_parameter = Some(parse_1c_string(fields.get(cursor)?)?);
        cursor += 1;
    }
    if mask & MOXEL_DRAWING_MEMBER_LOCALIZED != 0 {
        // The same container the cell record uses: an empty language identifier
        // marks a parameter name, a non-empty one a localized text.
        if let Some(localized) = parse_moxel_localized_cell_value(fields.get(cursor)?)? {
            if localized.lang.is_empty() {
                members.parameter = Some(localized.content);
            } else {
                members.text = Some(localized);
            }
        }
        if fields.get(cursor + 1)?.trim() != "0" {
            return None;
        }
        cursor += 2;
    }
    (cursor == fields.len()).then_some((format_index, members))
}

/// One drawing record.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 spreadsheet documents): the
/// record has twelve fields for the tail-less kinds - 1 `Line` (12 records),
/// 2 `Rectangle` (3) and 3 `Text` (89) - and fourteen for 5 `Picture` (576) and
/// 10 the chart family (15).  Reading those five kinds with this grammar pairs
/// every one of the 695 published drawings with exactly one record and leaves no
/// record unpaired, and re-rendering the 680 non-chart records reproduces the
/// platform's block byte for byte.  The previous grammar accepted only arity 14
/// with a bare `{0,index}` head, which is why 129 drawings were dropped.
///
/// The geometry is stored column-first (`beginColumn`, `beginRow`,
/// `beginColumnOffset`, `beginRowOffset`, then the same four for the end) and is
/// not ordered or non-negative: 9 records end left of where they begin, 2 end
/// above, and one publishes `beginColumnOffset` `-1`.  The range guard this
/// replaces refused those.
pub(super) fn parse_moxel_drawing(text: &str) -> Option<MoxelDrawing> {
    const CHART_TYPE_UUID: &str = "a8b97779-1a4b-4059-b09c-807f86d2a461";

    let fields = split_1c_braced_fields(text, 0)?;
    if fields.len() != 12 && fields.len() != 14 {
        return None;
    }
    let (format_index, members) = parse_moxel_drawing_format_record(fields.first()?)?;
    let begin_column = fields.get(2)?.trim().parse::<i32>().ok()?;
    let begin_row = fields.get(3)?.trim().parse::<i32>().ok()?;
    let begin_column_offset = fields.get(4)?.trim().parse::<i32>().ok()?;
    let begin_row_offset = fields.get(5)?.trim().parse::<i32>().ok()?;
    let end_column = fields.get(6)?.trim().parse::<i32>().ok()?;
    let end_row = fields.get(7)?.trim().parse::<i32>().ok()?;
    let end_column_offset = fields.get(8)?.trim().parse::<i32>().ok()?;
    let end_row_offset = fields.get(9)?.trim().parse::<i32>().ok()?;
    let id = fields.get(10)?.trim().parse::<usize>().ok()?;
    let kind_code = fields.get(1)?.trim();
    let (kind, auto_size) = if fields.len() == 12 {
        let shape = match kind_code {
            "1" => "Line",
            "2" => "Rectangle",
            "3" => "Text",
            _ => return None,
        };
        // The tail-less record keeps its own `autoSize` in the slot the picture
        // record spends on `pictureIndex`; all 104 observed records store 0.
        (
            MoxelDrawingKind::Shape(shape),
            fields.get(11)?.trim() != "0",
        )
    } else {
        match kind_code {
            "5" => {
                let picture_index = fields.get(11)?.trim().parse::<usize>().ok()?;
                let picture_size = match fields.get(12)?.trim().parse::<usize>().ok()? {
                    0 => "RealSize",
                    1 => "Stretch",
                    2 => "Proportionally",
                    4 => "AutoSize",
                    7 => "ByFontSize",
                    _ => return None,
                };
                (
                    MoxelDrawingKind::Picture {
                        picture_size,
                        picture_index,
                    },
                    fields.get(13)?.trim() != "0",
                )
            }
            // The chart family shares kind 10 and separates on the type uuid.
            // `GanttChart` (uuid e5fdc112-..., 2 records in 2 documents, each the
            // document's only drawing) has no decoder for its object payload, so
            // the record refuses rather than publishing a truncated subtree.
            "10" if fields.get(11)?.trim().eq_ignore_ascii_case(CHART_TYPE_UUID)
                && fields.get(13)?.trim() == "0" =>
            {
                (
                    MoxelDrawingKind::Chart(parse_moxel_chart(fields.get(12)?)?),
                    false,
                )
            }
            _ => return None,
        }
    };
    Some(MoxelDrawing {
        id,
        format_index,
        begin_row,
        begin_row_offset,
        end_row,
        end_row_offset,
        begin_column,
        begin_column_offset,
        end_column,
        end_column_offset,
        auto_size,
        // Assigned by `parse_moxel_drawings` from the sequence position.
        z_order: 0,
        members,
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

/// The leading default-format record, as `(width, font)`.
///
/// Its leading value is the format member mask. 129 names font slot 0 and width
/// slot 7, 161 names those two and the border-colour slot in between, so both
/// shapes carry a font member next to the width, and the platform publishes
/// that font reference whenever it writes this default format out as a
/// `<format>`.
///
/// Evidence (1С:УТ 11.5.27.75): 45 spreadsheet templates carry the 129 record
/// and 11 carry the 161 record; the font member is slot 0 in all 56. Of those,
/// 35 materialize the record as the document's trailing `<format>`, and the
/// platform writes `<font>0</font>` before that `<width>` in all 35. The rest
/// resolve their default format to an entry of the written table instead and
/// are left alone by the width filter at the call site.
pub(super) fn parse_moxel_leading_default_format_record(text: &str) -> Option<(usize, usize)> {
    let fields = split_1c_braced_fields(text, 0)?;
    let font = fields.get(1)?.trim().parse::<usize>().ok()?;
    match fields.first()?.trim() {
        "129" if fields.len() == 3 => Some((fields.get(2)?.trim().parse::<usize>().ok()?, font)),
        "161" if fields.len() == 4 && fields.get(2)?.trim() == "0" => {
            Some((fields.get(3)?.trim().parse::<usize>().ok()?, font))
        }
        _ => None,
    }
}

/// A cell's declared value type, as carried by the document's own type table.
#[derive(Clone, PartialEq, Eq)]
pub(super) enum MoxelValueType {
    Boolean,
    String {
        length: usize,
        allowed_length: &'static str,
    },
    Number {
        digits: usize,
        fraction_digits: usize,
        allowed_sign: &'static str,
    },
    Date {
        fractions: &'static str,
    },
    /// A configuration object reference, already rendered as its QName local
    /// part (`DocumentRef.РаспределениеНДС`).
    ConfigRef(String),
    /// A type the configuration does not name: published by identity.
    TypeId(String),
}

/// Decodes the document's value-type table.
///
/// Shape: a count-prefixed run of root fields, each `{"Pattern", {descriptor}}`.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 MOXCEL spreadsheet templates):
/// 53 documents carry the table and the descriptors collapse onto 36 distinct
/// shapes. Rendering every one of them through the rules below and comparing
/// against the published `<valueType>` blocks reproduces 52 of the 53 documents
/// exactly (the 53rd publishes none), with the qualifier defaults confirmed by
/// the bare forms: `{"S"}` is `Length 0`/`Variable` and `{"N"}` is
/// `Digits 0`/`FractionDigits 0`/`Any`.
///
/// A descriptor outside these shapes refuses the whole table rather than
/// letting some formats publish a type and others silently drop theirs.
pub(super) fn parse_moxel_value_types(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<MoxelValueType> {
    for (start, count_field) in fields.iter().enumerate() {
        let Some(count) = parse_moxel_canonical_positive_count(count_field) else {
            continue;
        };
        if count > MAX_MOXEL_VALUE_TYPES {
            continue;
        }
        let Some(entries) = fields.get(start + 1..start + 1 + count) else {
            continue;
        };
        if !entries
            .iter()
            .all(|entry| entry.trim_start().starts_with("{\"Pattern\""))
        {
            continue;
        }
        return entries
            .iter()
            .map(|entry| parse_moxel_value_type(entry, object_refs))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
    }
    Vec::new()
}

const MAX_MOXEL_VALUE_TYPES: usize = 2048;

pub(super) fn parse_moxel_value_type(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MoxelValueType> {
    let fields = split_1c_braced_fields(text, 0)?;
    if unquote_moxel_string(fields.first()?)? != "Pattern" || fields.len() != 2 {
        return None;
    }
    let payload = split_1c_braced_fields(fields.get(1)?, 0)?;
    match unquote_moxel_string(payload.first()?)?.as_str() {
        "B" if payload.len() == 1 => Some(MoxelValueType::Boolean),
        "S" if payload.len() == 1 => Some(MoxelValueType::String {
            length: 0,
            allowed_length: "Variable",
        }),
        "S" if payload.len() == 3 => Some(MoxelValueType::String {
            length: payload.get(1)?.trim().parse().ok()?,
            allowed_length: match payload.get(2)?.trim() {
                "0" => "Fixed",
                "1" => "Variable",
                _ => return None,
            },
        }),
        "N" if payload.len() == 1 => Some(MoxelValueType::Number {
            digits: 0,
            fraction_digits: 0,
            allowed_sign: "Any",
        }),
        "N" if payload.len() == 4 => Some(MoxelValueType::Number {
            digits: payload.get(1)?.trim().parse().ok()?,
            fraction_digits: payload.get(2)?.trim().parse().ok()?,
            allowed_sign: match payload.get(3)?.trim() {
                "0" => "Any",
                "1" => "Nonnegative",
                _ => return None,
            },
        }),
        "D" if payload.len() == 1 => Some(MoxelValueType::Date {
            fractions: "DateTime",
        }),
        "D" if payload.len() == 2 && unquote_moxel_string(payload.get(1)?)? == "D" => {
            Some(MoxelValueType::Date { fractions: "Date" })
        }
        "#" if payload.len() == 2 => {
            let uuid = parse_uuid_field(payload.get(1)?.trim())?;
            Some(match moxel_config_type_ref(&uuid, object_refs) {
                Some(reference) => MoxelValueType::ConfigRef(reference),
                None => MoxelValueType::TypeId(uuid),
            })
        }
        _ => None,
    }
}

/// Names a configuration object type the way `<v8:Type>` publishes it.
///
/// Evidence: `Reports/АнализРаспределенияНДС/Templates/Таблица` carries both
/// forms in one table - `fcd1e4a9-753c-4260-96ee-6b847c186dc5` is the УТ
/// document `РаспределениеНДС` and is published as
/// `d4p1:DocumentRef.РаспределениеНДС`, while
/// `48fa9d68-ae46-4d76-988a-88927f7a0ca6` names no configuration object and is
/// published as `<v8:TypeId>`. Only the object kind that corpus evidences is
/// mapped; any other kind falls back to the identity form rather than guessing
/// its reference suffix.
fn moxel_config_type_ref(uuid: &str, object_refs: &BTreeMap<String, String>) -> Option<String> {
    let name = object_refs.get(uuid)?.strip_prefix("Document.")?;
    Some(format!("DocumentRef.{name}"))
}

/// Fixed root trailer every MOXCEL body ends with: `0, 0, 1, 0, 0, 0`.
/// Constant across all 683 spreadsheet templates, which is what makes the
/// variable-length table in front of it addressable from the end.
const MOXEL_ROOT_TRAILER_FIELDS: usize = 6;

/// Decodes the document's input-mask table - a count-prefixed run of localized
/// values sitting directly in front of the fixed root trailer.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 MOXCEL spreadsheet templates):
/// walking back from `len - 6` over localized-value fields always lands on a
/// count that equals the run length, and resolving format member 34 through
/// the run reproduces every published `<mask>` - 1416 references over 683
/// documents, zero mismatches. 618 documents declare the empty table (`0`).
pub(super) fn parse_moxel_mask_refs(fields: &[&str]) -> Vec<Vec<MoxelLocalizedValue>> {
    let Some(end) = fields.len().checked_sub(MOXEL_ROOT_TRAILER_FIELDS) else {
        return Vec::new();
    };
    let mut start = end;
    while start > 0 && parse_moxel_localized_values(fields[start - 1]).is_some() {
        start -= 1;
    }
    let Some(count) = start
        .checked_sub(1)
        .and_then(|index| fields.get(index))
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    if count != end - start {
        return Vec::new();
    }
    fields[start..end]
        .iter()
        .map(|field| parse_moxel_localized_values(field).unwrap_or_default())
        .collect()
}

/// Decodes the document's control-type table: a count-prefixed run of bare
/// UUID root fields. Evidence (same corpus): 51 documents carry it, and the
/// UUID at the index named by format member 25 equals the published
/// `<controlType>` in all 51 - 285 references, zero mismatches.
pub(super) fn parse_moxel_control_types(fields: &[&str]) -> Vec<String> {
    for (start, count_field) in fields.iter().enumerate() {
        let Some(count) = parse_moxel_canonical_positive_count(count_field) else {
            continue;
        };
        if count > MAX_MOXEL_VALUE_TYPES {
            continue;
        }
        let Some(entries) = fields.get(start + 1..start + 1 + count) else {
            continue;
        };
        let Some(uuids) = entries
            .iter()
            .map(|entry| parse_uuid_field(entry.trim()))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        return uuids;
    }
    Vec::new()
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

/// The document's leading default-format record.
///
/// It is the body's fifth top-level field and nothing else: the writer already
/// reads that position when it publishes `<defaultFormatIndex>`. Sweeping the
/// first eight fields for anything that happens to decode as a non-empty format
/// let the sixth field - the column-set block - stand in for a document whose
/// own record is the empty `{0}`, which manufactured a default format, a
/// `<defaultFormatIndex>` the platform does not write, and an empty entry in
/// the pool slot that reference landed on.
pub(super) fn parse_moxel_leading_default_format(
    fields: &[&str],
    style_refs: &[Option<String>],
    number_format_refs: &[Vec<MoxelLocalizedValue>],
) -> Option<MoxelFormat> {
    fields
        .get(MOXEL_LEADING_DEFAULT_FORMAT_FIELD)
        .and_then(|field| parse_moxel_format(field, style_refs, number_format_refs))
        .filter(|format| !format.is_empty())
}

/// Position of the leading default-format record among the top-level fields.
pub(super) const MOXEL_LEADING_DEFAULT_FORMAT_FIELD: usize = 4;

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
            // Every member is strict about its own shape, so a token this
            // reader cannot spell refuses the record rather than dropping the
            // member - the strictness the value parser used to get by refusing
            // any `"N"` that was not an integer.
            0 => settings.paper = Some(value.as_usize()?),
            1 => settings.page_orientation = value.as_usize().and_then(moxel_page_orientation),
            2 => settings.scale = Some(value.as_usize()?),
            3 => settings.collate = Some(value.as_bool()?),
            4 => settings.copies = Some(value.as_usize()?),
            5 => settings.per_page = Some(value.as_usize()?),
            6 => settings.top_margin = Some(value.as_usize()?),
            7 => settings.left_margin = Some(value.as_usize()?),
            8 => settings.bottom_margin = Some(value.as_usize()?),
            9 => settings.right_margin = Some(value.as_usize()?),
            10 => settings.header_size = Some(value.as_usize()?),
            11 => settings.footer_size = Some(value.as_usize()?),
            12 => settings.fit_to_page = Some(value.as_bool()?),
            13 => settings.black_and_white = Some(value.as_bool()?),
            14 => settings.printer_name = Some(value.into_string()?),
            15 => settings.paper_source = Some(value.as_usize()?),
            16 => settings.page_width = Some(value.into_number_token()?),
            17 => settings.page_height = Some(value.into_number_token()?),
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
    // The two extended keys come as a pair, on top of a record that carries
    // every other key. The printer name (key 14) is the one that can be
    // absent: one document stores the nineteen keys without it, and demanding
    // a count of exactly twenty refused the whole record - the twenty-one
    // published lines that are its entire difference from the platform. The
    // record's own arity is already checked against its declared count above.
    let has_extended_keys = seen_keys.contains(&19) || seen_keys.contains(&20);
    if has_extended_keys
        && (!seen_keys.contains(&19)
            || !seen_keys.contains(&20)
            || !(0..=17).all(|key| key == 14 || seen_keys.contains(&key)))
    {
        return None;
    }
    Some(settings)
}

pub(super) enum MoxelPrintSettingsValue {
    Number(String),
    Text(String),
}

impl MoxelPrintSettingsValue {
    pub(super) fn as_usize(&self) -> Option<usize> {
        match self {
            Self::Number(value) => value.parse::<usize>().ok(),
            Self::Text(_) => None,
        }
    }

    /// The stored numeric token, for the members the platform publishes as a
    /// decimal. Admits digits with at most one fractional part, which is the
    /// whole of what the corpus's `"N"` values ever are.
    pub(super) fn into_number_token(self) -> Option<String> {
        let Self::Number(value) = self else {
            return None;
        };
        let (whole, fraction) = value.split_once('.').unwrap_or((value.as_str(), ""));
        (!whole.is_empty()
            && whole.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit()))
        .then_some(value)
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
        "N" => Some(MoxelPrintSettingsValue::Number(
            fields.get(1)?.trim().to_string(),
        )),
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
    // The source-ordered table carries the same font slots as the split ones,
    // so it has to move with them or a reference resolved through it would name
    // a font the output no longer numbers that way.
    let output_source_format_fonts = spreadsheet
        .source_formats
        .iter()
        .map(|format| source_font_map.output_format_font(format))
        .collect::<Option<Vec<_>>>();

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
    match output_source_format_fonts {
        Some(output_source_format_fonts) => {
            for (format, output_font) in spreadsheet
                .source_formats
                .iter_mut()
                .zip(output_source_format_fonts)
            {
                format.font = output_font;
            }
        }
        // A slot this map cannot project would leave the table half-renumbered,
        // which is worse than having no table at all.
        None => spreadsheet.source_formats.clear(),
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
    // The palette's count and typed descriptors can satisfy the loose legacy
    // format grammar. A confirmed palette is a structural boundary and is
    // excluded while legacy tables before it remain supported.
    let palette_span =
        locate_moxel_style_ref_palette(fields, &BTreeMap::new()).map(|(start, end, _)| start..end);
    let palette_after = palette_span.as_ref().map(|span| span.end);
    // New packed bodies place the actual table immediately after the palette;
    // try that structurally confirmed position before legacy global fallback.
    for index in palette_after.into_iter().chain(0..fields.len()) {
        if index >= fields.len() {
            continue;
        }
        if palette_span
            .as_ref()
            .is_some_and(|span| span.contains(&index))
        {
            continue;
        }
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
    // A drawing-referenced record spends three of the four border slots on
    // drawing members: member 1 is the drawing's own border, member 3 the four
    // packed `drawingHave*Border` flags and member 4 the print flag. Members 2
    // and 5 keep their ordinary meaning; no drawing-referenced record in the
    // corpus publishes a `leftBorder`, `topBorder`, `rightBorder`,
    // `bottomBorder` or `border` at all.
    format.drawing_border = format.left_border.take();
    format.drawing_have_borders = format.right_border.take();
    format.print = format.bottom_border.take().and_then(moxel_print_flag);
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

const REPORT_HEADER_TAIL_START: usize = 48;
const REPORT_HEADER_TAIL_LEN: usize = 11;

/// Locates the single-set report-header tail whose back colour the platform
/// writes as a literal instead of the style reference the body carries.
fn moxel_single_set_report_header_tail(
    column_sets: &[MoxelColumnSet],
    column_formats: &[MoxelFormat],
    formats: &[MoxelFormat],
) -> Option<usize> {
    let tail_start = REPORT_HEADER_TAIL_START.checked_sub(column_formats.len() + 1)?;
    let tail = formats.get(tail_start..tail_start + REPORT_HEADER_TAIL_LEN)?;
    (column_sets.len() == 1
        && column_formats.len() == 8
        && tail.iter().all(|format| {
            format.back_color.as_deref() == Some("style:ReportHeaderBackColor")
                && format.border_color.is_none()
                && format.text_placement == Some("Wrap")
        }))
    .then_some(tail_start)
}

fn apply_moxel_report_header_tail_back_color(formats: &mut [MoxelFormat], tail_start: usize) {
    for format in formats
        .iter_mut()
        .skip(tail_start)
        .take(REPORT_HEADER_TAIL_LEN)
    {
        format.back_color = Some("#F4ECC5".to_string());
    }
}

/// The back-colour half alone, for documents whose own line table decoded.
/// Such a document already carries the Solid/2 header line the legacy palette
/// reconstruction had to synthesize, so only the colour compensation is left.
fn normalize_moxel_report_header_tail_back_color(
    column_sets: &[MoxelColumnSet],
    column_formats: &[MoxelFormat],
    lines: &[ResolvedMoxelLine],
    formats: &mut [MoxelFormat],
) {
    let Some(line) = lines.get(1) else {
        return;
    };
    if line.style != "Solid" || line.width != 2 {
        return;
    }
    let Some(tail_start) =
        moxel_single_set_report_header_tail(column_sets, column_formats, formats)
    else {
        return;
    };
    apply_moxel_report_header_tail_back_color(formats, tail_start);
}

pub(super) fn normalize_moxel_single_set_report_header_tail(
    column_sets: &[MoxelColumnSet],
    column_formats: &[MoxelFormat],
    lines: &mut [ResolvedMoxelLine],
    formats: &mut [MoxelFormat],
) {
    let Some(line) = lines.get(1) else {
        return;
    };
    if line.style != "Dotted"
        || line.width != 1
        || !line.transformations.iter().any(|transformation| {
            matches!(transformation, MoxelLineTransformation::DefaultShift { .. })
        })
    {
        return;
    }
    let Some(tail_start) =
        moxel_single_set_report_header_tail(column_sets, column_formats, formats)
    else {
        return;
    };
    if let Some(line) = lines.get_mut(1) {
        line.style = "Solid";
        line.width = 2;
        line.transformations
            .push(MoxelLineTransformation::PostNormalizer {
                reason: "Dotted/1 to Solid/2",
            });
    }
    apply_moxel_report_header_tail_back_color(formats, tail_start);
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
        && format.mask_index.is_none()
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
    let format = MoxelFormat {
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
        // Member 13 is `patternColor` and member 18 is `textOrientation`; the
        // two were previously read as one slot, with 13 taken as the orientation
        // and 18 admitted only when it stored zero.  Evidence (native 1С:УТ
        // 11.5.27.75, all 683 spreadsheet documents): the platform publishes
        // exactly two orientations, 0 (503 formats) and 900 (10), and the 10
        // nine-hundreds are all that member 18 ever holds beside zero, while the
        // values member 13 holds - 2, 4 and 5 - are never published as an
        // orientation and always accompany a published `patternColor`.
        pattern_color: parse_moxel_format_style_ref(&values, 13, style_refs),
        pattern: parse_moxel_format_usize(&values, 12).and_then(moxel_format_pattern),
        text_color: parse_moxel_format_style_ref(&values, 10, style_refs),
        text_placement: parse_moxel_format_usize(&values, 14).and_then(moxel_text_placement),
        text_orientation: parse_moxel_format_usize(&values, 18),
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
        contains_value: parse_moxel_format_usize(&values, 22).and_then(moxel_bool_value),
        value_type_index: parse_moxel_format_usize(&values, 23),
        control_type_index: parse_moxel_format_usize(&values, 25),
        drawing_border: None,
        print: None,
        drawing_have_borders: None,
        auto_width_calculation: parse_moxel_format_usize(&values, 40).and_then(moxel_bool_value),
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
        mask_index: parse_moxel_format_usize(&values, 34),
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
    // `<pattern>` was synthesized for a record that stores no member 12 when
    // three unrelated members happened to line up. It is a fitted rule: the
    // platform publishes `<pattern>` only where member 12 is stored, and the
    // synthesis invents 15 patterns across 7 documents that the platform never
    // writes - which is the whole difference of those 7 documents.
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
            // 22 containsValue, 23 valueType index, 25 controlType index -
            // the three members the platform writes between markNegatives and
            // hyperLink.
            | 22
            | 23
            | 24
            | 25
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
            | 40
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
    let Some((_, palette_end, mut style_refs)) =
        locate_moxel_style_ref_palette(fields, object_refs)
    else {
        return Vec::new();
    };

    // Overrides are deliberately limited to explicit root containers and are
    // applied only after the bounded base table was located.
    for field in fields.iter().skip(palette_end) {
        if let Some(overrides) = parse_moxel_indexed_style_ref_overrides(field, object_refs) {
            for (slot_index, style_ref) in overrides {
                if slot_index >= MAX_MOXCEL_STYLE_REFS {
                    return Vec::new();
                }
                if style_refs.len() <= slot_index {
                    style_refs.resize(slot_index + 1, None);
                }
                style_refs[slot_index] = style_ref;
            }
        }
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

fn parse_moxel_raw_palette_provenance(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> MxlPaletteProvenance {
    let raw_slots = locate_moxel_style_ref_palette(fields, object_refs)
        .and_then(|(start, end, _)| fields.get(start + 1..end))
        .map(|slots| slots.iter().map(|slot| (*slot).to_string()).collect())
        .unwrap_or_default();
    MxlPaletteProvenance { raw_slots }
}

const MAX_MOXCEL_STYLE_REFS: usize = 2048;

fn parse_moxel_canonical_positive_count(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.first() == Some(&b'0') || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    text.parse::<usize>().ok()
}

/// Returns the strict root palette's half-open span and its slots.
fn locate_moxel_style_ref_palette(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<(usize, usize, Vec<Option<String>>)> {
    // A MOXCEL palette is a single, count-prefixed run in the root.  Do not
    // scan arbitrary nested/positional descriptors: those are common in row,
    // drawing and format data and used to corrupt the palette silently.
    let mut candidates = Vec::new();
    for (start, count_field) in fields.iter().enumerate() {
        let Some(count) = parse_moxel_canonical_positive_count(count_field) else {
            continue;
        };
        if count > MAX_MOXCEL_STYLE_REFS {
            continue;
        }
        let Some(end) = start
            .checked_add(1)
            .and_then(|value| value.checked_add(count))
        else {
            continue;
        };
        let Some(entries) = fields.get(start + 1..end) else {
            continue;
        };
        let Some(refs) = entries
            .iter()
            .map(|entry| parse_moxel_style_ref_slot(entry, object_refs))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        // A count which stops in the middle of a typed run is not canonical.
        if fields
            .get(end)
            .and_then(|entry| parse_moxel_style_ref_slot(entry, object_refs))
            .is_some()
        {
            continue;
        }
        candidates.push((start, end, refs));
    }
    // Multiple plausible spans are ambiguous by definition.  Preserve no
    // palette instead of guessing which unrelated root section won.
    if candidates.len() != 1 {
        return None;
    }
    candidates.pop()
}

/// The palette-override container is one count-prefixed table of
/// `(slot, style-ref)` pairs: `{count, slot, ref, slot, ref, ...}`.
///
/// Evidence (native 1С:УТ 11.5.27.75, all 683 MOXCEL spreadsheet templates):
/// 108 documents carry exactly one such container and none carries two. The
/// pair counts observed are 1 (80 documents), 2 (19), 3 (7) and 4 (2), and the
/// first slot named is 0, 2, 3, 4 or 5 - the leading value is the pair count,
/// never a slot index and never a container tag.
///
/// The previous reading took the leading `1` as a tag and hard-coded a second
/// shape `{3,2,...}` whose cursor started at the *second* pair: that shape is
/// simply `count = 3` whose first pair happens to name slot 2, so those six
/// documents silently lost their first override, and the 22 documents whose
/// count is 2, 4, or whose count-3 table starts at slot 3, matched neither
/// branch and lost every override. A container that does not decode as whole
/// pairs of `(index, style-ref slot)` is refused rather than partially read.
pub(super) fn parse_moxel_indexed_style_ref_overrides(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<(usize, Option<String>)>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    if count == 0 || count > MAX_MOXCEL_STYLE_REFS || fields.len() != count * 2 + 1 {
        return None;
    }
    let mut overrides = Vec::with_capacity(count);
    for pair in fields.get(1..)?.chunks_exact(2) {
        let slot_index = pair.first()?.trim().parse::<usize>().ok()?;
        let style_ref = parse_moxel_style_ref_slot(pair.get(1)?, object_refs)?;
        overrides.push((slot_index, style_ref));
    }
    Some(overrides)
}

/// Whether the anchored header/footer block is six empty formatted records.
///
/// Read at the block anchor rather than by scanning every six-field window; the
/// two agree on all 683 spreadsheet documents, and only the anchor can be wrong
/// for a reason the document states.
pub(super) fn parse_moxel_empty_headers_footers(fields: &[&str]) -> bool {
    fields
        .get(MOXEL_HEADER_FOOTER_BLOCK_START..MOXEL_HEADER_FOOTER_BLOCK_START + 6)
        .is_some_and(|block| {
            block
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
        "0" if payload.len() == 1 => payload
            .first()
            .filter(|value| value.trim().parse::<u32>().is_ok())
            .and_then(|value| parse_moxel_style_color(value.trim()))
            .map(Some),
        "1" => {
            (payload.len() == 1 && payload.first()?.trim().parse::<u32>().is_ok()).then_some(None)
        }
        "2" if payload.len() == 1 => payload
            .first()
            .filter(|value| value.trim().parse::<u32>().is_ok())
            .and_then(|value| parse_moxel_web_color(value.trim()))
            .map(Some),
        "3" if payload.len() == 1 => match payload.first()?.trim() {
            "-1" => Some(Some("style:FormBackColor".to_string())),
            "-3" => Some(Some("style:FormTextColor".to_string())),
            "-10" => Some(Some("style:FieldBackColor".to_string())),
            "-11" => Some(Some("style:FieldTextColor".to_string())),
            // Evidence (native 1С:УТ 11.5.27.75): `-13` occurs in exactly one
            // document, `ПечатьСтатусовТоваровФСС/.../ДанныеПроверкиТоваровФСС`,
            // whose palette is `-1, -3, -13` and whose only published style name
            // is `FieldAlternativeBackColor` - `-1` and `-3` publish nothing
            // there, so no other slot can account for it.
            "-13" => Some(Some("style:FieldAlternativeBackColor".to_string())),
            "-14" => Some(Some("style:FieldSelectionBackColor".to_string())),
            "-16" => Some(Some("style:SpecialTextColor".to_string())),
            "-17" => Some(Some("style:NegativeTextColor".to_string())),
            // Likewise `-21` occurs once, in
            // `ПечатьПодарочныхСертификатов/.../ПодарочныйСертификат`, whose
            // palette is `-1, -3, -21, -16` and which publishes exactly
            // `ButtonTextColor` and `SpecialTextColor`. `-16` is
            // `SpecialTextColor` in all 13 documents that carry it, which
            // leaves `ButtonTextColor` for `-21`.
            "-21" => Some(Some("style:ButtonTextColor".to_string())),
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
            _ => None,
        },
        "3" if payload.len() == 2 && payload.first()?.trim() == "0" => {
            let uuid = parse_uuid_field(payload.get(1)?.trim())?;
            Some(moxel_style_ref_for_uuid(&uuid, object_refs))
        }
        "4" if payload.len() == 1 && payload.first()?.trim() == "0" => {
            Some(Some("auto".to_string()))
        }
        _ => None,
    }
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

/// The platform's web-colour enumeration, indexed by its stored ordinal.
///
/// The ordinals are the enumeration's alphabetical positions. Six of them were
/// missing, and a missing ordinal is not a missing colour: a palette slot the
/// reader refuses makes the whole palette unrecognisable, so the thirteen
/// documents that store one lost every colour they had. Each addition is read
/// off the platform's own output - `Beige` (2 documents), `DarkGray` (4),
/// `DimGray` (1), `MediumBlue` (1), `MediumGray` (5) and `SaddleBrown` (1) are
/// the only published names those documents carry that no other ordinal of
/// theirs accounts for, and each lands in its alphabetical place.
pub(super) fn parse_moxel_web_color(value: &str) -> Option<String> {
    let name = match value.parse::<u32>().ok()? {
        6 => "Beige",
        8 => "Black",
        10 => "Blue",
        20 => "Cream",
        21 => "Crimson",
        23 => "DarkBlue",
        26 => "DarkGray",
        27 | 31 => "DarkGreen",
        33 => "DarkRed",
        37 => "DarkSlateGray",
        42 => "DimGray",
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
        86 => "MediumBlue",
        87 => "MediumGray",
        97 => "MintCream",
        98 => "MistyRose",
        108 => "PaleGoldenrod",
        119 => "Red",
        120 => "RosyBrown",
        121 => "RoyalBlue",
        122 => "SaddleBrown",
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
    parse_moxel_direct_color(value)
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

/// `<print>` is stored inverted, the spelling `protection` (member 16) carries:
/// the corpus holds only the value 1, published as `false`.
pub(super) fn moxel_print_flag(value: usize) -> Option<bool> {
    match value {
        0 => Some(true),
        1 => Some(false),
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
    // Isolated unchanged renderer for the legacy embedded-form caller. It does
    // not construct a typed plan and therefore cannot panic on plan admission.
    let output_format_indices = moxel_output_format_indices(spreadsheet);
    let output_format_index_map = moxel_output_format_index_map(&output_format_indices);
    render_moxel_spreadsheet_xml(
        spreadsheet,
        &output_format_indices,
        &output_format_index_map,
    )
}

/// The `<font>` table a spreadsheet document actually publishes.
///
/// Evidence (native 1С:УТ 11.5.27.75 dump, all 1335 `Templates/*/Ext/
/// Template.xml`): every published `<font>` table is exactly the set of fonts
/// the written `<format>` table references, listed in first-reference order.
/// 555 documents carry font references; every one of them is
/// first-reference-exact, none has an unreferenced entry, and there is no
/// counterexample. The MOXCEL font run itself is stored in an unrelated order.
///
/// The projection is derived from the very sequence of `<format>` elements the
/// writer is about to emit, which makes it an identity on any document that
/// already satisfies the invariant.
struct MoxelFontProjection {
    /// Published font slots, in order, as indices into the decoded run.
    fonts: Vec<usize>,
    /// Decoded font slot -> published slot, or `None` when unreferenced.
    font_slots: Vec<Option<usize>>,
}

impl MoxelFontProjection {
    fn font(&self, decoded_slot: Option<usize>) -> Option<usize> {
        decoded_slot.and_then(|slot| self.font_slots.get(slot).copied().flatten())
    }
}

fn moxel_font_projection(
    spreadsheet: &MoxelSpreadsheet,
    output_format_indices: &[usize],
) -> Option<MoxelFontProjection> {
    let mut fonts = Vec::new();
    let mut font_slots = vec![None; spreadsheet.fonts.len()];
    for format_index in output_format_indices.iter().copied() {
        let format = moxel_format_for_index(spreadsheet, format_index);
        // An empty format is written as `<format/>` and references nothing.
        if format.is_empty() {
            continue;
        }
        let Some(decoded_slot) = format.font else {
            continue;
        };
        // A dangling reference means the decoded run is not the one the format
        // table was written against; renumbering it would invent a font.
        let slot = font_slots.get_mut(decoded_slot)?;
        if slot.is_none() {
            *slot = Some(fonts.len());
            fonts.push(decoded_slot);
        }
    }
    Some(MoxelFontProjection { fonts, font_slots })
}

fn format_moxel_spreadsheet_xml_with_plan(
    spreadsheet: &MoxelSpreadsheet,
    plan: &MxlSpreadsheetWritePlan,
) -> Result<String, MxlDiagnostic> {
    // Palette slots and source-slot maps are retained for diagnostics and later
    // writer migration.  This renderer only receives their already-resolved
    // output projection and does not inspect raw MOXCEL values.
    let _ = (&plan.palette, &plan.format_map);
    Ok(render_moxel_spreadsheet_xml(
        spreadsheet,
        &plan.output_format_indices,
        &plan.output_format_index_map,
    ))
}

/// The four packed drawing-border flags, in publication order. Their weights
/// are 1, 2, 4 and 8: mask 2 is the only corpus value that separates a single
/// member and it publishes `top` alone, so `top` weighs 2 and the other three
/// follow the order the platform writes them in. The corpus holds no other
/// mask than 0, 2 and 15, which cannot separate left from right from bottom.
const MOXEL_DRAWING_HAVE_BORDER_TAGS: [&str; 4] = [
    "drawingHaveLeftBorder",
    "drawingHaveTopBorder",
    "drawingHaveRightBorder",
    "drawingHaveBottomBorder",
];

/// The published form of a format that carries no members.
const EMPTY_MOXEL_FORMAT_XML: &str = "\t<format/>\r\n";

/// The pool position the platform names for a format body.
///
/// Evidence (native 1С:УТ 11.5.27.75, every `Templates/*/Ext/Template.xml` that
/// decodes as a spreadsheet): 618 of the 683 documents publish
/// `<defaultFormatIndex>`, and in 614 of them the value is the position of the
/// *first* `<format>` element of the document whose published bytes equal the
/// bytes at the published position. The platform therefore names format
/// content, not a slot: a duplicate entry later in the pool is never named.
/// Four documents name a later duplicate and are left to the ordinary path.
fn first_equal_published_format(published_formats: &[String], position: usize) -> usize {
    let Some(body) = position
        .checked_sub(1)
        .and_then(|at| published_formats.get(at))
    else {
        return position;
    };
    published_formats
        .iter()
        .position(|candidate| candidate == body)
        .map(|at| at + 1)
        .unwrap_or(position)
}

fn render_moxel_spreadsheet_xml(
    spreadsheet: &MoxelSpreadsheet,
    output_format_indices: &[usize],
    output_format_index_map: &BTreeMap<usize, usize>,
) -> String {
    let emit_first_row_format_index =
        moxel_column_format_slots(&spreadsheet.column_sets, spreadsheet.column_count) == 0;
    let font_projection = moxel_font_projection(spreadsheet, output_format_indices);
    // The published format pool, rendered up front so that a reference into it
    // can be normalised against the bytes it names rather than against the
    // internal slot it came from.
    let published_formats = output_format_indices
        .iter()
        .map(|format_index| {
            let mut body = String::new();
            push_moxel_format_xml_with_fonts(
                &mut body,
                spreadsheet,
                *format_index,
                font_projection.as_ref(),
            );
            body
        })
        .collect::<Vec<_>>();
    // A stored reference names a position in the body's own format table, and
    // the platform answers it with the pool position that carries that entry's
    // published bytes - the same convention `<defaultFormatIndex>` follows.
    //
    // Evidence (native 1С:УТ 11.5.27.75, every `Templates/*/Ext/Template.xml`
    // that decodes as a spreadsheet): 96 header/footer references over 87
    // documents and 76 column-set references over 1810 column sets, and all
    // 172 name the *first* pool position holding their bytes - none names a
    // later duplicate and none points past the pool.
    let published_source_format = |source_format_ref: usize| -> Option<usize> {
        let format = source_format_ref
            .checked_sub(1)
            .and_then(|at| spreadsheet.source_formats.get(at))?;
        let mut body = String::new();
        push_moxel_format_body_xml(&mut body, spreadsheet, format, font_projection.as_ref());
        published_formats
            .iter()
            .position(|entry| *entry == body)
            .map(|at| at + 1)
    };
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
    );
    push_moxel_language_settings_xml(&mut xml, spreadsheet.language_settings.as_ref());
    for column_set in &spreadsheet.column_sets {
        push_moxel_columns_xml(
            &mut xml,
            column_set,
            &output_format_index_map,
            &published_source_format,
        );
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
    if let Some(slots) = &spreadsheet.header_footer_slots {
        // A zero reference publishes `<f>0</f>` as stored. A non-zero one names
        // a position in the body's own format table, and the platform answers
        // with the pool position that carries that entry's bytes.
        //
        // Evidence (native 1С:УТ 11.5.27.75, every `Templates/*/Ext/
        // Template.xml` that decodes as a spreadsheet): 87 documents publish a
        // header or footer, between them 96 distinct non-zero references, and
        // every one of the 96 names the *first* pool position holding its
        // bytes - no reference names a later duplicate and none points past the
        // pool. Where the bytes are absent from the pool the reference is not
        // this reader's case and keeps the shared-index path below.
        push_moxel_header_footer_slots_xml(&mut xml, slots, |source_format_ref| {
            if source_format_ref == 0 {
                return Some(0);
            }
            if let Some(published) = published_source_format(source_format_ref) {
                return Some(published);
            }
            let shared = spreadsheet.header_footer_format_index?;
            Some(
                output_format_index_map
                    .get(&shared)
                    .copied()
                    .unwrap_or(shared),
            )
        });
    }
    if spreadsheet.template_mode {
        xml.push_str("\t<templateMode>true</templateMode>\r\n");
    }
    // The leading default-format record names format content, not a slot: the
    // published index is the position of the first pool entry that carries the
    // record's own bytes.  An empty default format is never materialized, so a
    // document whose pool holds no empty `<format/>` publishes nothing.
    let leading_default_format_body = spreadsheet.leading_default_format.as_ref().map(|format| {
        let mut body = String::new();
        push_moxel_format_body_xml(&mut body, spreadsheet, format, font_projection.as_ref());
        body
    });
    let mut materialized_default_format = None;
    let published_default_format_index = match &leading_default_format_body {
        Some(body) => match published_formats.iter().position(|entry| entry == body) {
            Some(at) => Some(at + 1),
            None if body == EMPTY_MOXEL_FORMAT_XML => None,
            None => {
                materialized_default_format = Some(body.clone());
                Some(published_formats.len() + 1)
            }
        },
        None => spreadsheet
            .default_format_index
            .map(|default_format_index| {
                let position = output_format_index_map
                    .get(&default_format_index)
                    .copied()
                    .unwrap_or(default_format_index);
                first_equal_published_format(&published_formats, position)
            }),
    };
    if let Some(default_format_index) = published_default_format_index {
        xml.push_str(&format!(
            "\t<defaultFormatIndex>{default_format_index}</defaultFormatIndex>\r\n"
        ));
    }
    if spreadsheet.height > 0 {
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
    // Evidence (native 1С:УТ 11.5.27.75, all 1335 `Templates/*/Ext/
    // Template.xml`): where a document writes both, `<printSettings>` always
    // precedes `<printArea>` — 12 documents carry the pair and none inverts it.
    if let Some(print_settings) = &spreadsheet.print_settings
        && !print_settings.is_default_margins_only()
    {
        push_moxel_print_settings_xml(&mut xml, print_settings);
    }
    if let Some(print_area) = &spreadsheet.print_area {
        push_moxel_print_area_xml(&mut xml, print_area);
    }
    for line in &spreadsheet.lines {
        push_moxel_line_xml(&mut xml, line);
    }
    match &font_projection {
        Some(projection) => {
            for font in projection
                .fonts
                .iter()
                .filter_map(|slot| spreadsheet.fonts.get(*slot))
            {
                push_moxel_font_xml(&mut xml, font);
            }
        }
        None => {
            for font in &spreadsheet.fonts {
                push_moxel_font_xml(&mut xml, font);
            }
        }
    }
    for body in published_formats
        .iter()
        .chain(materialized_default_format.iter())
    {
        xml.push_str(body);
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
    // No floor: a document whose table is empty and whose records all name slot
    // 0 has no pool at all. Evidence (native 1С:УТ 11.5.27.75): none of the 683
    // standalone spreadsheet templates publishes an empty pool or a lone
    // `<format/>` - every one carries at least one populated entry, so the floor
    // never held a real document up - while two of the five distinct
    // spreadsheet blocks embedded in forms publish no `<format>` at all.
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
    published_source_format: &dyn Fn(usize) -> Option<usize>,
) {
    xml.push_str("\t<columns>\r\n");
    if let Some(id) = &column_set.id {
        xml.push_str(&format!("\t\t<id>{}</id>\r\n", escape_xml_text(id)));
    }
    // The stored reference decides whether the element exists at all: over all
    // 1810 column sets of the native corpus the 1734 that store 0 publish
    // nothing and the 76 that store anything else always publish. Its value is
    // the pool position carrying the referenced entry's bytes; a reference this
    // reader cannot place falls back to the internal slot it resolved to.
    if column_set.raw_default_format_index > 0
        && let Some(default_format_index) =
            published_source_format(column_set.raw_default_format_index).or_else(|| {
                column_set.default_format_index.map(|default_format_index| {
                    output_format_index_map
                        .get(&default_format_index)
                        .copied()
                        .unwrap_or(default_format_index)
                })
            })
    {
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
        .filter_map(|column_set| column_set.source_default_format_index())
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
        if let Some(source_format_index) = column_set.source_default_format_index()
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
        if let Some(source_format_index) = column_set.source_default_format_index()
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
        if let Some(source_format_index) = column_set.source_default_format_index()
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
    _header_footer_format_ref: Option<usize>,
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
            .source_default_format_index()
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
    })
}

fn remap_moxel_column_set_source_format_indices(
    column_sets: &mut [MoxelColumnSet],
    source_format_map: &MoxelSourceFormatMap,
) {
    for column_set in column_sets {
        if let Some(source_format_index) = column_set.source_default_format_index()
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

/// Writes the header/footer slots the document publishes.
///
/// `output_format_index` projects a non-zero stored reference onto the published
/// `<format>` table and returns `None` when that projection is not evidenced, in
/// which case the slot is left out rather than published with an invented index.
/// The language block the record names. A record this reader cannot spell
/// keeps the single block the writer used to emit unconditionally.
fn push_moxel_language_settings_xml(xml: &mut String, settings: Option<&MoxelLanguageSettings>) {
    let (current, default, infos) = match settings {
        Some(MoxelLanguageSettings::Placeholder) => return,
        Some(MoxelLanguageSettings::Named {
            current,
            default,
            infos,
        }) => (current.as_str(), default.as_str(), infos.as_slice()),
        None => ("ru", "ru", [].as_slice()),
    };
    xml.push_str("\t<languageSettings>\r\n");
    push_moxel_language_text(xml, "currentLanguage", current);
    push_moxel_language_text(xml, "defaultLanguage", default);
    if settings.is_none() {
        xml.push_str(
            "\t\t<languageInfo>\r\n\
\t\t\t<id>ru</id>\r\n\
\t\t\t<code>Русский</code>\r\n\
\t\t\t<description>Русский</description>\r\n\
\t\t</languageInfo>\r\n",
        );
    }
    for info in infos {
        xml.push_str("\t\t<languageInfo>\r\n");
        xml.push_str(&format!("\t\t\t<id>{}</id>\r\n", escape_xml_text(&info.id)));
        xml.push_str(&format!(
            "\t\t\t<code>{}</code>\r\n",
            escape_xml_text(&info.code)
        ));
        xml.push_str(&format!(
            "\t\t\t<description>{}</description>\r\n",
            escape_xml_text(&info.description)
        ));
        xml.push_str("\t\t</languageInfo>\r\n");
    }
    xml.push_str("\t</languageSettings>\r\n");
}

fn push_moxel_language_text(xml: &mut String, tag: &str, value: &str) {
    if value.is_empty() {
        xml.push_str(&format!("\t\t<{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!(
        "\t\t<{tag}>{}</{tag}>\r\n",
        escape_xml_text(value)
    ));
}

fn push_moxel_header_footer_slots_xml(
    xml: &mut String,
    slots: &[Option<MoxelHeaderFooter>],
    output_format_index: impl Fn(usize) -> Option<usize>,
) {
    for (tag, slot) in MOXEL_HEADER_FOOTER_TAGS.iter().zip(slots) {
        let Some(record) = slot else {
            continue;
        };
        let Some(format_index) = output_format_index(record.source_format_ref) else {
            continue;
        };
        xml.push_str(&format!("\t<{tag}>\r\n"));
        xml.push_str(&format!("\t\t<f>{format_index}</f>\r\n"));
        match record.text_kind {
            MoxelHeaderFooterText::Absent => {}
            MoxelHeaderFooterText::Plain => {
                push_moxel_header_footer_text_xml(xml, "tl", &record.text);
            }
            MoxelHeaderFooterText::Formatted => {
                push_moxel_header_footer_text_xml(xml, "tfl", &record.text);
            }
        }
        xml.push_str(&format!("\t</{tag}>\r\n"));
    }
}

fn push_moxel_header_footer_text_xml(xml: &mut String, tag: &str, values: &[MoxelLocalizedValue]) {
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
    push_moxel_format_text(xml, "pageWidth", settings.page_width.as_deref());
    push_moxel_format_text(xml, "pageHeight", settings.page_height.as_deref());
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

/// Writes one `<format>` with the decoded font slot verbatim.  Production
/// rendering goes through the font projection below; this entry point exists
/// for format-level unit coverage.
#[cfg(test)]
pub(super) fn push_moxel_format_xml(
    xml: &mut String,
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
) {
    push_moxel_format_xml_with_fonts(xml, spreadsheet, format_index, None);
}

fn push_moxel_format_xml_with_fonts(
    xml: &mut String,
    spreadsheet: &MoxelSpreadsheet,
    format_index: usize,
    font_projection: Option<&MoxelFontProjection>,
) {
    push_moxel_format_body_xml(
        xml,
        spreadsheet,
        &moxel_format_for_index(spreadsheet, format_index),
        font_projection,
    );
}

fn push_moxel_format_body_xml(
    xml: &mut String,
    spreadsheet: &MoxelSpreadsheet,
    format: &MoxelFormat,
    font_projection: Option<&MoxelFontProjection>,
) {
    if format.is_empty() {
        xml.push_str(EMPTY_MOXEL_FORMAT_XML);
        return;
    };
    // Without an admitted projection the decoded slot is written verbatim.
    let font = match font_projection {
        Some(projection) => projection.font(format.font),
        None => format.font,
    };
    xml.push_str("\t<format>\r\n");
    // The drawing members lead the element, ahead of `font`. Evidence (native
    // 1С:УТ 11.5.27.75): every record that publishes both `drawingBorder` and
    // `font` writes `drawingBorder` first - 9 records over 5 documents, none
    // inverted - and the three that publish `print` write it ahead of that.
    push_moxel_format_bool(xml, "print", format.print);
    push_moxel_format_usize(xml, "drawingBorder", format.drawing_border);
    if let Some(mask) = format.drawing_have_borders {
        for (bit, tag) in MOXEL_DRAWING_HAVE_BORDER_TAGS.iter().enumerate() {
            push_moxel_format_bool(xml, tag, Some(mask >> bit & 1 == 1));
        }
    }
    push_moxel_format_usize(xml, "font", font);
    push_moxel_format_usize(xml, "border", format.border);
    if format.border.is_none() {
        push_moxel_format_usize(xml, "leftBorder", format.left_border);
        push_moxel_format_usize(xml, "topBorder", format.top_border);
        push_moxel_format_usize(xml, "rightBorder", format.right_border);
        push_moxel_format_usize(xml, "bottomBorder", format.bottom_border);
    }
    push_moxel_format_i32(xml, "height", format.height);
    push_moxel_format_color(xml, "borderColor", format.border_color.as_deref());
    push_moxel_format_usize(xml, "width", format.width);
    push_moxel_format_bool(xml, "autoWidthCalculation", format.auto_width_calculation);
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
    if let Some(contains_value) = format.contains_value {
        xml.push_str(&format!(
            "\t\t<containsValue>{contains_value}</containsValue>\r\n"
        ));
    }
    if let Some(value_type) = format
        .value_type_index
        .and_then(|index| spreadsheet.value_types.get(index))
    {
        push_moxel_value_type_xml(xml, value_type);
    }
    push_moxel_localized_values_xml(
        xml,
        "format",
        &format.number_format,
        format.number_format_present,
    );
    if let Some(control_type) = format
        .control_type_index
        .and_then(|index| spreadsheet.control_types.get(index))
    {
        xml.push_str(&format!(
            "\t\t<controlType>{control_type}</controlType>\r\n"
        ));
    }
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
    if let Some(mask) = format
        .mask_index
        .and_then(|index| spreadsheet.mask_refs.get(index))
    {
        push_moxel_localized_values_xml(xml, "mask", mask, true);
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

pub(super) fn push_moxel_value_type_xml(xml: &mut String, value_type: &MoxelValueType) {
    xml.push_str("\t\t<valueType>\r\n");
    match value_type {
        MoxelValueType::Boolean => xml.push_str("\t\t\t<v8:Type>xs:boolean</v8:Type>\r\n"),
        MoxelValueType::String {
            length,
            allowed_length,
        } => xml.push_str(&format!(
            "\t\t\t<v8:Type>xs:string</v8:Type>\r\n\
             \t\t\t<v8:StringQualifiers>\r\n\
             \t\t\t\t<v8:Length>{length}</v8:Length>\r\n\
             \t\t\t\t<v8:AllowedLength>{allowed_length}</v8:AllowedLength>\r\n\
             \t\t\t</v8:StringQualifiers>\r\n"
        )),
        MoxelValueType::Number {
            digits,
            fraction_digits,
            allowed_sign,
        } => xml.push_str(&format!(
            "\t\t\t<v8:Type>xs:decimal</v8:Type>\r\n\
             \t\t\t<v8:NumberQualifiers>\r\n\
             \t\t\t\t<v8:Digits>{digits}</v8:Digits>\r\n\
             \t\t\t\t<v8:FractionDigits>{fraction_digits}</v8:FractionDigits>\r\n\
             \t\t\t\t<v8:AllowedSign>{allowed_sign}</v8:AllowedSign>\r\n\
             \t\t\t</v8:NumberQualifiers>\r\n"
        )),
        MoxelValueType::Date { fractions } => xml.push_str(&format!(
            "\t\t\t<v8:Type>xs:dateTime</v8:Type>\r\n\
             \t\t\t<v8:DateQualifiers>\r\n\
             \t\t\t\t<v8:DateFractions>{fractions}</v8:DateFractions>\r\n\
             \t\t\t</v8:DateQualifiers>\r\n"
        )),
        MoxelValueType::ConfigRef(reference) => xml.push_str(&format!(
            "\t\t\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">\
             d4p1:{}</v8:Type>\r\n",
            escape_xml_element_text(reference)
        )),
        MoxelValueType::TypeId(uuid) => {
            xml.push_str(&format!("\t\t\t<v8:TypeId>{uuid}</v8:TypeId>\r\n"))
        }
    }
    xml.push_str("\t\t</valueType>\r\n");
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
            if format.font.is_none() {
                format.font = spreadsheet.default_format_font;
            }
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
        MoxelDrawingKind::Shape(shape) => shape,
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
    // Member publication order is `text`/`parameter`, `value`,
    // `detailParameter`, which is the reverse of their slot order in the record;
    // the 9 records that carry all three pin it.
    if let Some(text) = &drawing.members.text {
        xml.push_str("\t\t<text>\r\n\t\t\t<v8:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(&text.lang)
        ));
        xml.push_str(&format!(
            "\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&text.content)
        ));
        xml.push_str("\t\t\t</v8:item>\r\n\t\t</text>\r\n");
    }
    if let Some(parameter) = &drawing.members.parameter {
        xml.push_str(&format!(
            "\t\t<parameter>{}</parameter>\r\n",
            escape_xml_element_text(parameter)
        ));
    }
    if let Some(value) = &drawing.members.value {
        xml.push_str(&format!(
            "\t\t<value xsi:type=\"xs:string\">{}</value>\r\n",
            escape_xml_element_text(value)
        ));
    }
    if let Some(detail_parameter) = &drawing.members.detail_parameter {
        xml.push_str(&format!(
            "\t\t<detailParameter>{}</detailParameter>\r\n",
            escape_xml_element_text(detail_parameter)
        ));
    }
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
        MoxelDrawingKind::Shape(_) | MoxelDrawingKind::Chart(_) => "Stretch",
    };
    xml.push_str(&format!(
        "\t\t<pictureSize>{picture_size}</pictureSize>\r\n"
    ));
    xml.push_str(&format!("\t\t<zOrder>{}</zOrder>\r\n", drawing.z_order));
    match &drawing.kind {
        MoxelDrawingKind::Shape(_) => {}
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
    if !group.open {
        xml.push_str("\t\t<o>false</o>\r\n");
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
    // A member the descriptor's mask does not carry is a member the platform
    // does not write, so each attribute is emitted exactly when it is present.
    for (member, name) in [
        (font.bold, "bold"),
        (font.italic, "italic"),
        (font.underline, "underline"),
        (font.strikeout, "strikeout"),
    ] {
        if let Some(value) = member {
            xml.push_str(&format!(" {name}=\"{value}\""));
        }
    }
    xml.push_str(&format!(" kind=\"{}\"", font.kind));
    if let Some(scale) = font.scale {
        xml.push_str(&format!(" scale=\"{scale}\""));
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
        if let Some(control) = &cell.control {
            push_moxel_cell_control_xml(xml, control);
        }
        let text_element = if cell.formatted_text { "tfl" } else { "tl" };
        if let Some(text) = &cell.text {
            xml.push_str(&format!("\t\t\t\t\t<{text_element}>\r\n"));
            xml.push_str("\t\t\t\t\t\t<v8:item>\r\n");
            xml.push_str("\t\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(text)
            ));
            xml.push_str("\t\t\t\t\t\t</v8:item>\r\n");
            xml.push_str(&format!("\t\t\t\t\t</{text_element}>\r\n"));
        } else if cell.empty_text {
            xml.push_str(&format!("\t\t\t\t\t<{text_element}/>\r\n"));
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
        if let Some(picture_parameter) = &cell.picture_parameter {
            xml.push_str(&format!(
                "\t\t\t\t\t<pictureParameter>{}</pictureParameter>\r\n",
                escape_xml_element_text(picture_parameter)
            ));
        }
        if let Some(value) = &cell.value {
            push_moxel_cell_value_xml(xml, "v", value);
        }
        if let Some(detail_value) = &cell.detail_value {
            push_moxel_cell_value_xml(xml, "d", detail_value);
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

/// Publishes one typed cell member.
///
/// A stored reference always publishes as `<r>`, whichever member carries it;
/// every other spelling keeps the member's own element name and carries the
/// XSD type the platform writes for it.
fn push_moxel_cell_value_xml(xml: &mut String, element: &str, value: &MoxelCellValue) {
    match value {
        MoxelCellValue::Nil => {
            xml.push_str(&format!("\t\t\t\t\t<{element} xsi:nil=\"true\"/>\r\n"));
        }
        MoxelCellValue::Text(text) if text.is_empty() => {
            xml.push_str(&format!(
                "\t\t\t\t\t<{element} xsi:type=\"xs:string\"/>\r\n"
            ));
        }
        MoxelCellValue::Text(text) => {
            xml.push_str(&format!(
                "\t\t\t\t\t<{element} xsi:type=\"xs:string\">{}</{element}>\r\n",
                escape_xml_element_text(text)
            ));
        }
        MoxelCellValue::Number(number) => {
            xml.push_str(&format!(
                "\t\t\t\t\t<{element} xsi:type=\"xs:decimal\">{number}</{element}>\r\n"
            ));
        }
        MoxelCellValue::DateTime(stamp) => {
            xml.push_str(&format!(
                "\t\t\t\t\t<{element} xsi:type=\"xs:dateTime\">{}-{}-{}T{}:{}:{}</{element}>\r\n",
                &stamp[0..4],
                &stamp[4..6],
                &stamp[6..8],
                &stamp[8..10],
                &stamp[10..12],
                &stamp[12..14]
            ));
        }
        MoxelCellValue::Reference(index) => {
            xml.push_str(&format!("\t\t\t\t\t<r>{index}</r>\r\n"));
        }
    }
}

/// Publishes an embedded control blob verbatim.
fn push_moxel_cell_control_xml(xml: &mut String, control: &str) {
    xml.push_str(&format!(
        "\t\t\t\t\t<control xsi:type=\"xs:base64Binary\">{control}</control>\r\n"
    ));
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

    /// Fixture: `tests/fixtures/moxel_reconciliation_report_headers_raw.txt`,
    /// 2005 bytes, sha256
    /// `c477882a48c0f27dbb8439de942c2ded6ef342c096a39a3aa2811c040f24fef3`. It
    /// is the native MOXCEL body of the `МакетЗаголовковОтчета` template of
    /// report `СверкаРасчетовСКонтрагентами` in 1С:Управление торговлей
    /// 11.5.27.75 (`1cv8.cf`), as produced by this project's compatible-MXL
    /// decoder.
    ///
    /// Its format table stores five records with member 40 set, and the
    /// platform publishes `<autoWidthCalculation>true</autoWidthCalculation>`
    /// for exactly those five, between the `width` (member 7) and the
    /// `widthWeightFactor` (member 41) each is written with.
    const RECONCILIATION_REPORT_HEADERS_RAW: &str =
        include_str!("../../tests/fixtures/moxel_reconciliation_report_headers_raw.txt");

    fn reconciliation_report_headers() -> MoxelSpreadsheet {
        parse_moxel_spreadsheet_text(RECONCILIATION_REPORT_HEADERS_RAW, &BTreeMap::new()).unwrap()
    }

    #[test]
    fn member_forty_publishes_the_automatic_width_flag() {
        let xml = format_moxel_spreadsheet_xml(&reconciliation_report_headers());

        assert_eq!(
            xml.matches("<autoWidthCalculation>true</autoWidthCalculation>")
                .count(),
            5
        );
        assert!(xml.contains(
            "\t\t<width>264</width>\r\n\
\t\t<autoWidthCalculation>true</autoWidthCalculation>\r\n\
\t\t<widthWeightFactor>4</widthWeightFactor>\r\n"
        ));
        // A record that stores member 40 without a width keeps the flag.
        assert!(xml.contains(
            "\t<format>\r\n\
\t\t<autoWidthCalculation>true</autoWidthCalculation>\r\n\
\t\t<widthWeightFactor>2</widthWeightFactor>\r\n\
\t</format>\r\n"
        ));
    }

    /// The record is `{67108882,1,1,1}` - members 1, 4 and 26 - the drawing
    /// format of the `ПФ_MXL_КарточкаТорговогоПредложения` template of data
    /// processor `ТорговыеПредложения` in 1С:УТ 11.5.27.75. The platform
    /// publishes `<print>false</print>`, `<drawingBorder>1</drawingBorder>`
    /// and `<hyperLink>true</hyperLink>` for it, in that order.
    #[test]
    fn a_drawing_record_spends_member_four_on_the_print_flag() {
        let mut format = parse_moxel_format("{67108882,1,1,1}", &[], &[]).unwrap();
        assert_eq!(format.bottom_border, Some(1));
        assert_eq!(format.left_border, Some(1));

        normalize_moxel_drawing_format(&mut format);

        assert_eq!(format.print, Some(false));
        assert_eq!(format.drawing_border, Some(1));
        assert_eq!(format.hyper_link, Some(true));
        assert_eq!(format.bottom_border, None);
        assert_eq!(format.left_border, None);

        let spreadsheet = reconciliation_report_headers();
        let mut xml = String::new();
        push_moxel_format_body_xml(&mut xml, &spreadsheet, &format, None);
        assert_eq!(
            xml,
            "\t<format>\r\n\
\t\t<print>false</print>\r\n\
\t\t<drawingBorder>1</drawingBorder>\r\n\
\t\t<hyperLink>true</hyperLink>\r\n\
\t</format>\r\n"
        );
    }

    /// Member 3 of a drawing record is the four packed `drawingHave*Border`
    /// flags. Mask 2 is the corpus value that separates a single member: the
    /// platform publishes `top` alone for it.
    #[test]
    fn a_drawing_record_unpacks_member_three_into_four_flags() {
        let mut format = parse_moxel_format("{8,2}", &[], &[]).unwrap();
        assert_eq!(format.right_border, Some(2));

        normalize_moxel_drawing_format(&mut format);

        assert_eq!(format.drawing_have_borders, Some(2));
        assert_eq!(format.right_border, None);

        let spreadsheet = reconciliation_report_headers();
        let mut xml = String::new();
        push_moxel_format_body_xml(&mut xml, &spreadsheet, &format, None);
        assert_eq!(
            xml,
            "\t<format>\r\n\
\t\t<drawingHaveLeftBorder>false</drawingHaveLeftBorder>\r\n\
\t\t<drawingHaveTopBorder>true</drawingHaveTopBorder>\r\n\
\t\t<drawingHaveRightBorder>false</drawingHaveRightBorder>\r\n\
\t\t<drawingHaveBottomBorder>false</drawingHaveBottomBorder>\r\n\
\t</format>\r\n"
        );
    }

    /// The print-settings record of the `ШаблонЭтикетки_34х58_ИС` template of
    /// catalog `ШаблоныЭтикетокИЦенников` in 1С:УТ 11.5.27.75. Key 16 stores a
    /// decimal, which the whole-record `usize` reading refused - and with it
    /// all twenty members of `<printSettings>`.
    #[test]
    fn page_geometry_is_a_decimal_and_does_not_refuse_the_record() {
        const RAW: &str = "{\n{0,20,0,\n{\"N\",9},1,\n{\"N\",1},2,\n{\"N\",100},3,\n{\"N\",0},4,\n\
{\"N\",1},5,\n{\"N\",1},6,\n{\"N\",1000},7,\n{\"N\",1000},8,\n{\"N\",1000},9,\n{\"N\",1000},10,\n\
{\"N\",1000},11,\n{\"N\",1000},12,\n{\"N\",0},13,\n{\"N\",0},14,\n\
{\"S\",\"\\\\\\\\mars.solar.local\\\\PRN-D9-4R-PCL6\"},15,\n{\"N\",256},16,\n{\"N\",60.5},17,\n\
{\"N\",40},19,\n{\"N\",4},20,\n{\"N\",0}\n}\n}";

        let settings = parse_moxel_print_settings_field(RAW).unwrap();

        assert_eq!(settings.page_width.as_deref(), Some("60.5"));
        assert_eq!(settings.page_height.as_deref(), Some("40"));
        assert_eq!(settings.paper, Some(9));
        assert_eq!(settings.duplex_type, Some("UsePrinterSettings"));

        let mut xml = String::new();
        push_moxel_print_settings_xml(&mut xml, &settings);
        assert!(xml.contains("\t\t<pageWidth>60.5</pageWidth>\r\n"));
        assert!(xml.contains("\t\t<pageHeight>40</pageHeight>\r\n"));
    }

    /// The three language records the corpus stores, and the block the
    /// platform answers each with.
    #[test]
    fn the_language_block_follows_the_stored_record() {
        let named =
            parse_moxel_language_settings("{\"ru\",\"ru\",0,1,\"ru\",\"Русский\",\"Русский\",0}")
                .unwrap();
        let empty = parse_moxel_language_settings("{\"\",\"\",0,0,0}").unwrap();
        let placeholder = parse_moxel_language_settings(
            "{\"#\",\"\",0,1,\"#\",\"Язык по умолчанию\",\"Язык по умолчанию\",0}",
        )
        .unwrap();

        let mut xml = String::new();
        push_moxel_language_settings_xml(&mut xml, Some(&named));
        assert_eq!(
            xml,
            "\t<languageSettings>\r\n\
\t\t<currentLanguage>ru</currentLanguage>\r\n\
\t\t<defaultLanguage>ru</defaultLanguage>\r\n\
\t\t<languageInfo>\r\n\
\t\t\t<id>ru</id>\r\n\
\t\t\t<code>Русский</code>\r\n\
\t\t\t<description>Русский</description>\r\n\
\t\t</languageInfo>\r\n\
\t</languageSettings>\r\n"
        );

        let mut xml = String::new();
        push_moxel_language_settings_xml(&mut xml, Some(&empty));
        assert_eq!(
            xml,
            "\t<languageSettings>\r\n\
\t\t<currentLanguage/>\r\n\
\t\t<defaultLanguage/>\r\n\
\t</languageSettings>\r\n"
        );

        let mut xml = String::new();
        push_moxel_language_settings_xml(&mut xml, Some(&placeholder));
        assert_eq!(xml, "");
    }

    /// Every standalone template stores the template-mode flag as 1, so the
    /// element stays; an embedded body that stores 0 loses it.
    #[test]
    fn the_template_mode_element_follows_its_stored_flag() {
        let spreadsheet = reconciliation_report_headers();
        assert!(spreadsheet.template_mode);
        assert!(
            format_moxel_spreadsheet_xml(&spreadsheet)
                .contains("<templateMode>true</templateMode>")
        );

        let cleared = RECONCILIATION_REPORT_HEADERS_RAW.replacen("},1,2,9,", "},0,2,9,", 1);
        assert_ne!(cleared, RECONCILIATION_REPORT_HEADERS_RAW);
        let spreadsheet = parse_moxel_spreadsheet_text(&cleared, &BTreeMap::new()).unwrap();
        assert!(!spreadsheet.template_mode);
        assert!(!format_moxel_spreadsheet_xml(&spreadsheet).contains("<templateMode>"));
    }

    /// Fixture: `tests/fixtures/moxel_buyer_instruction_picture_parameter_raw.txt`,
    /// 796 bytes, sha256
    /// `9346fe2d6d05e3eb826cdddde70a069e73b65385796690314e5082112ae4a00e`. It is
    /// the native MOXCEL body of common template `ИнструкцияПокупателейMAXБПО`
    /// in 1С:Управление торговлей 11.5.27.75 (`1cv8.cf`), as produced by this
    /// project's compatible-MXL decoder. It is the smallest document of the
    /// corpus that carries a picture-parameter cell member, and its leading
    /// default-format record names a width the format pool does not already
    /// hold, so the platform materializes it as the pool's last entry.
    const BUYER_INSTRUCTION_RAW: &str =
        include_str!("../../tests/fixtures/moxel_buyer_instruction_picture_parameter_raw.txt");

    #[test]
    fn picture_parameter_member_is_published_from_its_own_mask_bit() {
        let spreadsheet =
            parse_moxel_spreadsheet_text(BUYER_INSTRUCTION_RAW, &BTreeMap::new()).unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert!(
            xml.contains("\t\t\t\t\t<pictureParameter>КартинкаИнструкции</pictureParameter>\r\n"),
            "{xml}"
        );
    }

    #[test]
    fn unmaterialized_default_format_is_appended_and_named() {
        let spreadsheet =
            parse_moxel_spreadsheet_text(BUYER_INSTRUCTION_RAW, &BTreeMap::new()).unwrap();
        let xml = format_moxel_spreadsheet_xml(&spreadsheet);

        assert_eq!(xml.matches("<format").count(), 2, "{xml}");
        assert!(
            xml.contains("\t<format>\r\n\t\t<width>72</width>\r\n\t</format>\r\n"),
            "{xml}"
        );
        assert!(
            xml.contains("\t<defaultFormatIndex>2</defaultFormatIndex>\r\n"),
            "{xml}"
        );
    }

    #[test]
    fn cell_mask_names_a_member_set_and_not_a_record_shape() {
        let picture_and_detail = parse_moxel_cell(
            "{88,3,\"Расшифровка\",\"Поставщик\",{1,1,{\"\",\"ПоставщикПредставление\"}},0}",
            0,
        )
        .unwrap();
        assert_eq!(
            picture_and_detail.detail_parameter.as_deref(),
            Some("Расшифровка")
        );
        assert_eq!(
            picture_and_detail.picture_parameter.as_deref(),
            Some("Поставщик")
        );
        assert_eq!(
            picture_and_detail.parameter.as_deref(),
            Some("ПоставщикПредставление")
        );

        let detail_value_and_detail_parameter = parse_moxel_cell(
            "{28,11,{\"#\",3031edd8-c3df-47b2-98ca-47f628d4ec18,{15}},\"Расшифровка\",{1,1,{\"\",\"Цена\"}},0}",
            0,
        )
        .unwrap();
        assert_eq!(
            detail_value_and_detail_parameter.detail_value,
            Some(MoxelCellValue::Reference(15))
        );
        assert_eq!(
            detail_value_and_detail_parameter
                .detail_parameter
                .as_deref(),
            Some("Расшифровка")
        );

        let value_only = parse_moxel_cell("{2,15,{\"N\",0}}", 0).unwrap();
        assert_eq!(
            value_only.value,
            Some(MoxelCellValue::Number("0".to_string()))
        );
        assert!(!value_only.empty_text && value_only.text.is_none());

        let formatted = parse_moxel_cell(
            "{16,2,{1,1,{\"ru\",\"Код\"}},1,{1,{1,1,{\"ru\",\"Код\"}},1}}",
            0,
        )
        .unwrap();
        assert!(formatted.formatted_text);
        assert_eq!(formatted.text.as_deref(), Some("Код"));
    }

    #[test]
    fn cell_record_that_does_not_match_its_mask_is_refused() {
        // A member bit the reader does not know, and a field count that does
        // not match the mask, are both refusals rather than a shifted read.
        assert!(parse_moxel_cell("{128,3,\"Расшифровка\"}", 0).is_none());
        assert!(parse_moxel_cell("{8,3,\"Расшифровка\",\"Поставщик\"}", 0).is_none());
        assert!(parse_moxel_cell("{16,2,{1,0}}", 0).is_none());
    }

    #[test]
    fn typed_cell_members_publish_the_platform_spellings() {
        let mut xml = String::new();
        push_moxel_cell_value_xml(&mut xml, "v", &MoxelCellValue::Number("0".to_string()));
        push_moxel_cell_value_xml(&mut xml, "v", &MoxelCellValue::Text(String::new()));
        push_moxel_cell_value_xml(&mut xml, "v", &MoxelCellValue::Text("5".to_string()));
        push_moxel_cell_value_xml(
            &mut xml,
            "v",
            &MoxelCellValue::DateTime("00010101000000".to_string()),
        );
        push_moxel_cell_value_xml(&mut xml, "d", &MoxelCellValue::Nil);
        push_moxel_cell_value_xml(&mut xml, "d", &MoxelCellValue::Reference(15));

        assert_eq!(
            xml,
            "\t\t\t\t\t<v xsi:type=\"xs:decimal\">0</v>\r\n\
\t\t\t\t\t<v xsi:type=\"xs:string\"/>\r\n\
\t\t\t\t\t<v xsi:type=\"xs:string\">5</v>\r\n\
\t\t\t\t\t<v xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</v>\r\n\
\t\t\t\t\t<d xsi:nil=\"true\"/>\r\n\
\t\t\t\t\t<r>15</r>\r\n"
        );
    }

    #[test]
    fn every_stored_web_colour_ordinal_resolves() {
        // The six ordinals the reader used to refuse; a refused palette slot
        // costs the document its whole palette, not just one colour.
        for (ordinal, name) in [
            ("6", "d3p1:Beige"),
            ("26", "d3p1:DarkGray"),
            ("42", "d3p1:DimGray"),
            ("86", "d3p1:MediumBlue"),
            ("87", "d3p1:MediumGray"),
            ("122", "d3p1:SaddleBrown"),
        ] {
            assert_eq!(parse_moxel_web_color(ordinal).as_deref(), Some(name));
        }
    }

    #[test]
    fn a_literal_palette_colour_is_never_renamed_to_a_style() {
        // Two stored RGB values used to be rewritten as style references.
        assert_eq!(
            parse_moxel_style_color("8765644").as_deref(),
            Some("#CCC085")
        );
        assert_eq!(
            parse_moxel_style_color("12971252").as_deref(),
            Some("#F4ECC5")
        );
    }

    #[test]
    fn typed_extraction_reports_binary_container_as_decoder_failure() {
        let error = try_extract_moxel_spreadsheet_xml(&[0], &BTreeMap::new()).unwrap_err();

        assert_eq!(
            error.stage(),
            crate::mssql_dump::MxlDiagnosticStage::Decoder
        );
        assert_eq!(error.code(), "mxl.decoder.binary-container");
    }

    #[test]
    fn raw_palette_provenance_precedes_compatibility_synthesis() {
        let fields = [
            "5",
            "{3,1,{0}}",
            "{3,1,{0}}",
            "{3,3,{-28}}",
            "{3,1,{0}}",
            "{3,4,{0}}",
        ];
        let resolved = parse_moxel_style_refs(&fields, &BTreeMap::new());
        assert_eq!(resolved[1].as_deref(), Some("style:FormTextColor"));

        let provenance = parse_moxel_raw_palette_provenance(&fields, &BTreeMap::new());
        assert_eq!(
            provenance.raw_slots,
            fields[1..]
                .iter()
                .map(|slot| (*slot).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(provenance.raw_slots[1], "{3,1,{0}}");
    }

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
                formatted_text: false,
                picture_parameter: None,
                control: None,
                value: None,
                detail_value: None,
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
            language_settings: None,
            template_mode: true,
            column_count: 0,
            column_sets: Vec::new(),
            column_formats: Vec::new(),
            extra_formats: BTreeMap::new(),
            default_format_width: None,
            default_format_font: None,
            default_format: MoxelFormat::default(),
            formats: vec![format],
            source_formats: Vec::new(),
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
            header_footer_format_index: None,
            header_footer_slots: None,
            default_format_index: None,
            leading_default_format: None,
            source_format_map: None,
            value_types: Vec::new(),
            control_types: Vec::new(),
            mask_refs: Vec::new(),
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
            lines[0].raw_parents[0].line_entry_index, 0,
            "drawing-only normalization records the chosen raw source"
        );
        assert!(lines[0].transformations.iter().any(|transformation| {
            *transformation
                == MoxelLineTransformation::DrawingOnlySelectedSource { source_index: 0 }
        }));
        assert_eq!(
            lines[0].line_type,
            "v8ui:SpreadsheetDocumentDrawingLineType"
        );
        assert_eq!(lines[0].width, 1);
    }

    #[test]
    fn shifted_default_palette_keeps_raw_parents_and_reason() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                border: Some(1),
                ..MoxelFormat::default()
            },
        ];
        let lines = parse_moxel_lines(&["{3,3,{-1}}", "{3,3,{-3}}"], &formats, true);

        assert!(lines.iter().any(|line| {
            line.transformations.iter().any(|transformation| {
                matches!(transformation, MoxelLineTransformation::DefaultShift { .. })
            }) && !line.raw_parents.is_empty()
        }));
    }

    #[test]
    fn post_normalizer_appends_to_existing_shift_chain() {
        let mut lines = vec![
            ResolvedMoxelLine {
                line: MoxelLine {
                    style: "Solid",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 1,
                },
                raw_parents: vec![MoxelRawLineParent {
                    raw_entry_index: 1,
                    line_entry_index: 0,
                    span_start: 0,
                    span_end: 0,
                }],
                transformations: vec![MoxelLineTransformation::DefaultShift { reason: "fixture" }],
                format_support: Vec::new(),
                ambiguous: false,
                fail_closed: false,
            },
            ResolvedMoxelLine {
                line: MoxelLine {
                    style: "Dotted",
                    line_type: "v8ui:SpreadsheetDocumentCellLineType",
                    width: 1,
                },
                raw_parents: vec![MoxelRawLineParent {
                    raw_entry_index: 2,
                    line_entry_index: 1,
                    span_start: 0,
                    span_end: 0,
                }],
                transformations: vec![MoxelLineTransformation::DefaultShift { reason: "fixture" }],
                format_support: Vec::new(),
                ambiguous: false,
                fail_closed: false,
            },
        ];
        let column_sets = vec![MoxelColumnSet {
            id: None,
            default_format_index: None,
            raw_default_format_index: 0,
            size: 0,
            columns: Vec::new(),
        }];
        let column_formats = vec![MoxelFormat::default(); 8];
        let mut formats = vec![MoxelFormat::default(); 50];
        for format in formats.iter_mut().skip(39).take(11) {
            format.back_color = Some("style:ReportHeaderBackColor".to_string());
            format.text_placement = Some("Wrap");
        }
        normalize_moxel_single_set_report_header_tail(
            &column_sets,
            &column_formats,
            &mut lines,
            &mut formats,
        );

        assert_eq!(lines[1].style, "Solid");
        assert_eq!(lines[1].width, 2);
        assert_eq!(lines[1].raw_parents[0].raw_entry_index, 2);
        assert!(matches!(
            lines[1].transformations.as_slice(),
            [
                MoxelLineTransformation::DefaultShift { .. },
                MoxelLineTransformation::PostNormalizer { .. }
            ]
        ));
    }

    #[test]
    fn ukd_default_palette_shift_preserves_provenance_through_report_header_normalizer() {
        // This is the two-entry palette used by UKD report headers: the raw
        // None/Solid palette is shifted into the final cell-line slots.
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                right_border: Some(1),
                ..MoxelFormat::default()
            },
        ];
        let mut lines = parse_moxel_lines(&["{3,3,{-1}}", "{3,3,{-3}}"], &formats, true);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].style, "Solid");
        assert_eq!(lines[0].line_type, "v8ui:SpreadsheetDocumentCellLineType");
        assert_eq!(lines[0].width, 1);
        assert_eq!(
            lines[0].raw_parents,
            vec![MoxelRawLineParent {
                raw_entry_index: 0,
                line_entry_index: 0,
                span_start: 0,
                span_end: "{3,3,{-1}}".len(),
            }]
        );
        assert_eq!(
            lines[0].transformations,
            vec![MoxelLineTransformation::DefaultShift {
                reason: "two-line default shift",
            }]
        );
        assert_eq!(
            lines[0].format_support,
            vec![MoxelLineFormatSupport {
                format_index: 0,
                border_slot: MoxelLineBorderSlot::Border,
            }]
        );

        assert_eq!(lines[1].style, "Dotted");
        assert_eq!(lines[1].line_type, "v8ui:SpreadsheetDocumentCellLineType");
        assert_eq!(lines[1].width, 1);
        assert_eq!(
            lines[1].raw_parents,
            vec![MoxelRawLineParent {
                raw_entry_index: 1,
                line_entry_index: 1,
                span_start: 0,
                span_end: "{3,3,{-3}}".len(),
            }]
        );
        assert_eq!(
            lines[1].transformations,
            vec![MoxelLineTransformation::DefaultShift {
                reason: "two-line default shift",
            }]
        );
        assert_eq!(
            lines[1].format_support,
            vec![MoxelLineFormatSupport {
                format_index: 1,
                border_slot: MoxelLineBorderSlot::Right,
            }]
        );

        // Exercise the real strict palette plus its indexed root override;
        // the normalizer must not depend on its former fixed slot layout.
        let strict_palette = [
            "4",
            "{3,3,{-1}}",
            "{3,3,{-3}}",
            "{3,0,{12971252}}",
            "{3,0,{12971252}}",
            "{1,2,{3,3,{-25}}}",
        ];
        let style_refs = parse_moxel_style_refs(&strict_palette, &BTreeMap::new());
        assert_eq!(
            style_refs,
            vec![
                Some("style:FormBackColor".to_string()),
                Some("style:FormTextColor".to_string()),
                // Slot 2 is what the indexed override names; slot 3 keeps the
                // literal the palette stores. The reader used to rewrite this
                // stored RGB as a style name, which is what the platform's own
                // output contradicts.
                Some("style:ReportHeaderBackColor".to_string()),
                Some("#F4ECC5".to_string()),
            ]
        );

        let column_sets = vec![MoxelColumnSet {
            id: None,
            default_format_index: None,
            raw_default_format_index: 0,
            size: 0,
            columns: Vec::new(),
        }];
        let column_formats = vec![MoxelFormat::default(); 8];
        let report_header_wrap = parse_moxel_format("{18432,2,3}", &style_refs, &[]).unwrap();
        let mut formats = vec![MoxelFormat::default(); 51];
        for format in formats.iter_mut().skip(38).take(13) {
            *format = report_header_wrap.clone();
        }
        normalize_moxel_single_set_report_header_tail(
            &column_sets,
            &column_formats,
            &mut lines,
            &mut formats,
        );

        assert_eq!(lines[0].style, "Solid");
        assert_eq!(lines[0].width, 1);
        assert_eq!(
            lines[0].transformations,
            vec![MoxelLineTransformation::DefaultShift {
                reason: "two-line default shift",
            }]
        );
        assert_eq!(lines[1].style, "Solid");
        assert_eq!(lines[1].width, 2);
        assert_eq!(
            formats[38].back_color.as_deref(),
            Some("style:ReportHeaderBackColor")
        );
        assert!(
            formats[39..50]
                .iter()
                .all(|format| format.back_color.as_deref() == Some("#F4ECC5"))
        );
        assert_eq!(
            formats[50].back_color.as_deref(),
            Some("style:ReportHeaderBackColor")
        );
        assert_eq!(
            lines[1].raw_parents,
            vec![MoxelRawLineParent {
                raw_entry_index: 1,
                line_entry_index: 1,
                span_start: 0,
                span_end: "{3,3,{-3}}".len(),
            }]
        );
        assert_eq!(
            lines[1].format_support,
            vec![MoxelLineFormatSupport {
                format_index: 1,
                border_slot: MoxelLineBorderSlot::Right,
            }]
        );
        assert_eq!(
            lines[1].transformations,
            vec![
                MoxelLineTransformation::DefaultShift {
                    reason: "two-line default shift",
                },
                MoxelLineTransformation::PostNormalizer {
                    reason: "Dotted/1 to Solid/2",
                },
            ]
        );

        struct Sink(std::cell::RefCell<Vec<MoxelLineTraceEvent>>);
        impl MoxelLineTraceSink for Sink {
            fn record_moxel_line(&self, event: MoxelLineTraceEvent) {
                self.0.borrow_mut().push(event);
            }
        }
        let sink = Sink(std::cell::RefCell::new(Vec::new()));
        trace_final_moxel_lines(&lines, Some(&sink));
        let events = sink.0.into_inner();

        // The event is a direct projection of the final carried state: no
        // value-based raw matching is permitted between these assertions.
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].output_line_index, 0);
        assert_eq!(events[0].raw_parents[0].raw_entry_index, 0);
        assert_eq!(events[0].format_support[0].border_slot, "border");
        assert_eq!(events[1].output_line_index, 1);
        assert_eq!(events[1].raw_parents[0].raw_entry_index, 1);
        assert_eq!(events[1].format_support[0].format_index, 1);
        assert_eq!(events[1].format_support[0].border_slot, "right");
        assert_eq!(events[1].final_style, "Solid");
        assert_eq!(events[1].final_width, 2);
        assert!(events.iter().all(|event| !event.final_gap));
        assert_eq!(
            events[1]
                .transformations
                .iter()
                .map(|transformation| transformation.kind)
                .collect::<Vec<_>>(),
            vec!["default_shift", "post_normalizer"]
        );
        assert!(
            events
                .iter()
                .all(|event| !event.ambiguous && !event.fail_closed)
        );
    }

    #[test]
    fn duplicate_raw_lines_stay_distinct_without_value_matching() {
        let formats = vec![
            MoxelFormat {
                border: Some(0),
                ..MoxelFormat::default()
            },
            MoxelFormat {
                border: Some(1),
                ..MoxelFormat::default()
            },
        ];
        let source = "{8,{3,3,{-3}},{3,3,{-3}}}";
        let raw_spans = split_1c_braced_fields_with_spans(source, 0).unwrap();
        let fields = raw_spans
            .iter()
            .map(|(value, _, _)| *value)
            .collect::<Vec<_>>();
        let lines = parse_moxel_lines_with_raw_spans(&fields, &raw_spans, &formats, false);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].raw_parents[0].raw_entry_index, 1);
        assert_eq!(lines[0].raw_parents[0].line_entry_index, 0);
        assert_eq!(lines[0].raw_parents[0].span_start, 3);
        assert_eq!(lines[0].raw_parents[0].span_end, 13);
        assert_eq!(lines[1].raw_parents[0].raw_entry_index, 2);
        assert_eq!(lines[1].raw_parents[0].line_entry_index, 1);
        assert_eq!(lines[1].raw_parents[0].span_start, 14);
        assert_eq!(lines[1].raw_parents[0].span_end, 24);
        assert!(!lines[0].ambiguous && !lines[1].ambiguous);
    }

    #[test]
    fn trace_reserves_before_materializing_and_stops_at_first_overflow() {
        use std::cell::{Cell, RefCell};

        struct BoundedSink {
            remaining: Cell<usize>,
            reserve_calls: Cell<usize>,
            events: RefCell<Vec<MoxelLineTraceEvent>>,
        }
        impl MoxelLineTraceSink for BoundedSink {
            fn try_reserve_event(&self) -> bool {
                self.reserve_calls.set(self.reserve_calls.get() + 1);
                let remaining = self.remaining.get();
                if remaining == 0 {
                    false
                } else {
                    self.remaining.set(remaining - 1);
                    true
                }
            }

            fn record_moxel_line(&self, event: MoxelLineTraceEvent) {
                self.events.borrow_mut().push(event);
            }
        }

        let line = ResolvedMoxelLine {
            line: MoxelLine {
                style: "Solid",
                line_type: "v8ui:SpreadsheetDocumentCellLineType",
                width: 1,
            },
            // Large evidence would be cloned by `MoxelLineTraceEvent::from`;
            // it must remain untouched for entries after the rejected reserve.
            raw_parents: vec![MoxelRawLineParent {
                raw_entry_index: 0,
                line_entry_index: 0,
                span_start: 0,
                span_end: 1_000_000,
            }],
            transformations: Vec::new(),
            format_support: Vec::new(),
            ambiguous: false,
            fail_closed: false,
        };
        let sink = BoundedSink {
            remaining: Cell::new(1),
            reserve_calls: Cell::new(0),
            events: RefCell::new(Vec::new()),
        };
        trace_final_moxel_lines(
            &[
                line.clone(),
                line,
                ResolvedMoxelLine {
                    line: MoxelLine {
                        style: "Solid",
                        line_type: "v8ui:SpreadsheetDocumentCellLineType",
                        width: 1,
                    },
                    raw_parents: Vec::new(),
                    transformations: Vec::new(),
                    format_support: Vec::new(),
                    ambiguous: false,
                    fail_closed: false,
                },
            ],
            Some(&sink),
        );
        assert_eq!(sink.reserve_calls.get(), 2);
        assert_eq!(sink.events.borrow().len(), 1);
    }

    #[test]
    fn truncation_marks_survivors_without_reassigning_parents() {
        let formats = vec![MoxelFormat {
            border: Some(0),
            ..MoxelFormat::default()
        }];
        let lines = parse_moxel_lines(
            &["{3,3,{-1}}", "{3,3,{-3}}", "{3,3,{-10}}", "{3,3,{-3}}"],
            &formats,
            false,
        );

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].raw_parents[0].line_entry_index, 0);
        assert!(lines.iter().all(|line| line.transformations.iter().any(
            |transformation| matches!(transformation, MoxelLineTransformation::Truncated { .. })
        )));
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
