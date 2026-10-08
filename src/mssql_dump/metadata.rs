use super::*;

/// Why a Config row could not be promoted to a metadata XML candidate.
///
/// This is deliberately an audit classification rather than an extraction
/// error: legacy callers may still ignore rows they cannot decode, while a
/// full-export gate can report every omission deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataExtractionMissReason {
    Inflate,
    Utf8,
    ObjectCode,
    Fields,
    Header,
    Family,
    Formatter,
}

impl MetadataExtractionMissReason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Inflate => "inflate",
            Self::Utf8 => "utf8",
            Self::ObjectCode => "object_code",
            Self::Fields => "fields",
            Self::Header => "header",
            Self::Family => "family",
            Self::Formatter => "formatter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataExtractionMiss {
    pub file_name: String,
    pub reason: MetadataExtractionMissReason,
}

/// Result of classifying one raw metadata row without silently discarding the
/// reason for a failed candidate.
#[allow(dead_code)]
#[derive(Debug)]
pub(super) enum MetadataTextRowAudit {
    Extracted(MetadataTextRow),
    ExtractedWithWarning(MetadataTextRow, MetadataExtractionMiss),
    Miss(MetadataExtractionMiss),
}

#[derive(Debug, Clone)]
pub(super) struct MetadataTextRow {
    pub(super) file_name: String,
    pub(super) text: String,
    pub(super) object_code: Option<u32>,
    pub(super) header: Option<MetadataHeader>,
    pub(super) kind: Option<String>,
    pub(super) folder: Option<&'static str>,
}

pub(super) fn metadata_text_row_from_blob(file_name: &str, blob: &[u8]) -> Option<MetadataTextRow> {
    match metadata_text_row_audit_from_blob(file_name, blob) {
        MetadataTextRowAudit::Extracted(row) => Some(row),
        MetadataTextRowAudit::ExtractedWithWarning(row, _) => Some(row),
        MetadataTextRowAudit::Miss(_) => None,
    }
}

pub(super) fn metadata_text_row_audit_from_blob(
    file_name: &str,
    blob: &[u8],
) -> MetadataTextRowAudit {
    let inflated = match inflate_raw_deflate(blob) {
        Ok(value) => value,
        Err(_) => return metadata_text_row_miss(file_name, MetadataExtractionMissReason::Inflate),
    };
    let text = match String::from_utf8(inflated) {
        Ok(value) => value,
        Err(_) => return metadata_text_row_miss(file_name, MetadataExtractionMissReason::Utf8),
    };
    metadata_text_row_audit_from_text(file_name, text.trim_start_matches('\u{feff}').to_string())
}

pub(super) fn metadata_text_row_from_text(
    file_name: &str,
    text: String,
) -> Option<MetadataTextRow> {
    match metadata_text_row_audit_from_text(file_name, text) {
        MetadataTextRowAudit::Extracted(row) => Some(row),
        MetadataTextRowAudit::ExtractedWithWarning(row, _) => Some(row),
        MetadataTextRowAudit::Miss(_) => None,
    }
}

pub(super) fn metadata_text_row_audit_from_text(
    file_name: &str,
    text: String,
) -> MetadataTextRowAudit {
    // Keep the legacy row builder deliberately permissive. Index consumers
    // have historically received textual rows even when their structure was
    // only partially understood; the audit path records that loss of
    // confidence without changing those successful callers.
    // An 8.5 metadata text spells its colour and font tuples one revision
    // later; every reader below knows the 8.3.27 spelling.
    let text = match super::form::layout_8_5_1::rewrite_primitives_8_5_1_in_place(&text) {
        std::borrow::Cow::Borrowed(_) => text,
        std::borrow::Cow::Owned(rewritten) => rewritten,
    };
    let text = match without_adoption_headers(&text) {
        std::borrow::Cow::Borrowed(_) => text,
        std::borrow::Cow::Owned(rewritten) => rewritten,
    };
    // Header blocks in an older platform's spelling read in the current one
    // (see `upgrade_header_blocks` for the evidence).
    let text = super::upgrade::upgrade_record_by_shape(&text, file_name).unwrap_or(text);
    let object_code = parse_metadata_object_code(&text);
    let fields_missing = metadata_object_fields(&text).is_none();
    let header = parse_metadata_header_from_text(&text, file_name);
    let (kind, folder) = match object_code {
        Some(12) if is_direct_code14_form_metadata_text(&text, file_name) => {
            (Some("Form".to_string()), None)
        }
        Some(12) => (Some("CommonModule".to_string()), Some("CommonModules")),
        // A form descriptor (`{0,{13,<header>,0,1,…}}`) shares an integration
        // service's code and header slot but names nothing behind the
        // header; read as a service, an orphan one (its owner not read) wrote
        // its body as `IntegrationServices/<form>/Ext/Module.bsl` (21 files
        // over two real configurations). It is a form, which has no folder of
        // its own -- the kind `normalize_direct_form_metadata` gives the other
        // form descriptor shape.
        Some(0)
            if metadata_source_for_text(0, &text, file_name)
                == Some(("IntegrationService", "IntegrationServices"))
                && !metadata_text_names_identifiers_behind_header(&text, file_name) =>
        {
            (Some("Form".to_string()), None)
        }
        Some(code) => metadata_source_for_text(code, &text, file_name)
            .map(|(kind, folder)| (Some(kind.to_string()), Some(folder)))
            .unwrap_or((None, None)),
        None => (None, None),
    };
    // Old child records are respelled once here so every reader of the row
    // sees the current layout (`legacy_child`).
    let text = super::legacy_child::upgrade_legacy_child_records(&text).unwrap_or(text);
    let row = MetadataTextRow {
        file_name: file_name.to_string(),
        text,
        object_code,
        header,
        kind,
        folder,
    };
    let reason = if object_code.is_none() {
        Some(MetadataExtractionMissReason::ObjectCode)
    } else if fields_missing {
        Some(MetadataExtractionMissReason::Fields)
    } else if row.header.is_none() {
        Some(MetadataExtractionMissReason::Header)
    } else if row.folder.is_none() && row.kind.as_deref() != Some("Form") {
        Some(MetadataExtractionMissReason::Family)
    } else {
        None
    };
    if let Some(reason) = reason {
        MetadataTextRowAudit::ExtractedWithWarning(
            row,
            MetadataExtractionMiss {
                file_name: file_name.to_string(),
                reason,
            },
        )
    } else {
        MetadataTextRowAudit::Extracted(row)
    }
}

/// `text` with every object header's adoption part cleared: an extension
/// stores an adopted object -- and each adopted attribute, tabular section,
/// … inside it -- as `{3,{1,0,<uuid>},<name>,<synonym>,<comment>,1,N,
/// (<property>,<state>)×N,<extended object>,0}`, which every reader here
/// refuses (a real extension: 65 catalogs and documents). Read as the object of the
/// extension it also is (`…,0,0,<nil>,0`), it takes the path an own object
/// takes; the extension writer (`crate::extension`) reads the adoption from
/// the stored row and rewrites the XML (fixture `adopted/document_children`).
pub(super) fn without_adoption_headers(text: &str) -> std::borrow::Cow<'_, str> {
    const NIL: &str = "00000000-0000-0000-0000-000000000000";
    // The configuration root keeps its own: its readers take the adopted
    // header, and it holds no children's headers.
    if text.trim_start().starts_with("{2,")
        && text.contains("{9cd510cd-abfc-11d4-9434-004095e12fc7,")
    {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut edits = Vec::new();
    let mut search = 0;
    while let Some(relative) = text[search..].find("{1,0,") {
        let marker = search + relative;
        search = marker + 1;
        // The header list opens right before the marker: `{3,` or `{2,`.
        let before = text[..marker].trim_end();
        let Some(before) = before.strip_suffix(',') else {
            continue;
        };
        let before = before.trim_end();
        let version = if before.ends_with("{3") {
            "3"
        } else if before.ends_with("{2") {
            "2"
        } else {
            continue;
        };
        let start = before.len() - 2;
        let Some(fields) = split_1c_braced_fields(text, start) else {
            continue;
        };
        // Counts come from the file: one beyond the fields is no header.
        let Some(pairs) = fields
            .get(6)
            .and_then(|field| field.trim().parse::<usize>().ok())
            .filter(|pairs| *pairs <= fields.len())
        else {
            continue;
        };
        let belonging = fields.get(5).map(|field| field.trim());
        if !(belonging == Some("1") || (belonging == Some("0") && pairs > 0)) {
            continue;
        }
        // `{3,…}` ends with the widened properties: a count, then
        // `(property, state, extend value)` each (a real extension: 15 widened types).
        let tail = if version == "3" {
            let Some(widened) = fields
                .get(8 + 2 * pairs)
                .and_then(|field| field.trim().parse::<usize>().ok())
                .filter(|widened| *widened <= fields.len())
            else {
                continue;
            };
            2 + 3 * widened
        } else {
            1
        };
        if fields.len() != 7 + 2 * pairs + tail {
            continue;
        }
        // An adopted object names the object it extends; one that names none
        // is no header this reads.
        if belonging == Some("1") && fields[7 + 2 * pairs].trim() == NIL {
            continue;
        }
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let mut plain = format!(
            "{{{version},{},{},{},{},0,0,{NIL}",
            fields[1].trim(),
            fields[2].trim(),
            fields[3].trim(),
            fields[4].trim()
        );
        if version == "3" {
            plain.push_str(",0");
        }
        plain.push('}');
        edits.push((start, end, plain));
        search = end;
        // A widened type: the field after the header holds the type the
        // extension checks the adopted one against, the header the types the
        // extension adds. The readers take the added ones as the object's
        // type; the extension writer prints both (`ExtendedProperty`).
        if version == "3" {
            let widened_at = 9 + 2 * pairs;
            let widened_type = (0..(tail - 2) / 3).find_map(|index| {
                let at = widened_at + 3 * index;
                (fields.get(at)?.trim() == WIDENED_TYPE)
                    .then(|| widened_type_pattern(fields.get(at + 2)?))
                    .flatten()
            });
            if let Some(pattern) = widened_type {
                let after = text[end..].trim_start();
                let type_start = text.len() - after.len();
                if let Some(value) = after.strip_prefix(',') {
                    let value_start = type_start + 1 + (value.len() - value.trim_start().len());
                    if text[value_start..].starts_with("{\"Pattern\"")
                        && let Some(value_end) = scan_1c_braced_value(text, value_start)
                    {
                        edits.push((value_start, value_end, pattern.to_owned()));
                        search = value_end;
                    }
                }
            }
        }
    }
    if edits.is_empty() {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (start, end, plain) in edits {
        out.push_str(&text[at..start]);
        out.push_str(&plain);
        at = end;
    }
    out.push_str(&text[at..]);
    std::borrow::Cow::Owned(out)
}

/// The type property (`Type` of a defined type, an attribute, a dimension,
/// a filter criterion, …) in an adoption header.
const WIDENED_TYPE: &str = "b1053250-abe6-11d4-9434-004095e12fc7";

/// The `{"Pattern",…}` inside a widened value `{"#",<TypeDescription>,…}`.
fn widened_type_pattern(value: &str) -> Option<&str> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    (fields.len() == 3
        && fields[0].trim() == r##""#""##
        && fields[1].trim() == "f5c65050-3bbb-11d5-b988-0050bae0a95d"
        && fields[2].trim().starts_with("{\"Pattern\""))
    .then(|| fields[2].trim())
}

fn metadata_text_row_miss(
    file_name: &str,
    reason: MetadataExtractionMissReason,
) -> MetadataTextRowAudit {
    MetadataTextRowAudit::Miss(MetadataExtractionMiss {
        file_name: file_name.to_string(),
        reason,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct MetadataHeader {
    pub(super) uuid: String,
    pub(super) name: String,
    pub(super) synonyms: Vec<(String, String)>,
    pub(super) comment: String,
    pub(super) template_type_code: Option<u32>,
}

pub(super) fn parse_metadata_object_code(text: &str) -> Option<u32> {
    let after_root = text.trim_start().strip_prefix("{1,")?;
    let after_root = after_root.trim_start();
    let after_open = after_root.strip_prefix('{')?;
    let digits = after_open
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

pub(super) fn metadata_source_for_text(
    code: u32,
    text: &str,
    uuid: &str,
) -> Option<(&'static str, &'static str)> {
    let fields = metadata_object_fields(text)?;
    metadata_source_for_object_fields(code, text, uuid, &fields)
}

/// Whether the object record carries an identifier right behind its header
/// (`{0,<header>,<uuid>,…}`), as an integration service does and a form
/// descriptor (`{0,{13,<header>,0,1,…}}`) does not.
pub(super) fn metadata_text_names_identifiers_behind_header(text: &str, uuid: &str) -> bool {
    metadata_object_fields(text).is_some_and(|fields| {
        metadata_header_field_index(&fields, uuid) == Some(1)
            && fields
                .get(2)
                .is_some_and(|field| is_uuid_text(field.trim()))
    })
}

pub(super) fn metadata_source_for_object_text(
    code: u32,
    object_text: &str,
    uuid: &str,
) -> Option<(&'static str, &'static str)> {
    let wrapped_text = format!("{{1,{object_text}}}");
    let fields = split_1c_braced_fields(object_text, 0)?;
    metadata_source_for_object_fields(code, &wrapped_text, uuid, &fields)
}

/// Fixed evidenced ValueId every `Command`-family `GeneratedType` pair
/// carries (`docs/evidence/utility-objects-8.3.27.md`); independently
/// redefined per module elsewhere in this crate (`compiler/families/
/// commands.rs`, `business_object.rs`, `utility.rs`) rather than shared,
/// matching the existing convention.
const COMMAND_VALUE_UUID: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

fn contains_common_command_value_marker(text: &str, uuid: &str) -> bool {
    text.contains(&format!("{{2,{uuid},{COMMAND_VALUE_UUID}}}"))
}

pub(super) fn metadata_source_for_object_fields(
    code: u32,
    text: &str,
    uuid: &str,
    fields: &[&str],
) -> Option<(&'static str, &'static str)> {
    let header_index = metadata_header_field_index(&fields, uuid);

    match code {
        0 if header_index == Some(1)
            && ibcmd_schema::websocket_client::WebSocketClientLayout::recognizes_fields(fields) =>
        {
            Some(ibcmd_schema::websocket_client::WebSocketClientLayout::identity())
        }
        0 if header_index == Some(1) && field_starts_with(fields.get(2), "{0,") => {
            Some(("FunctionalOptionsParameter", "FunctionalOptionsParameters"))
        }
        0 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("Language", "Languages"))
        }
        // A platform 8.5 palette colour: the header and one colour tuple,
        // `{0,<header>,{3,<space>,{<payload>}}}` once the 8.5 colour is read
        // in its 8.3.27 spelling (8.5.1.1150 BSP `PaletteColors/*`, 2 of 2).
        // The integration service below shares the code and the header slot
        // but carries identifiers after it, never a single colour.
        0 if header_index == Some(1)
            && fields.len() == 3
            && fields.get(2).is_some_and(|field| {
                let field = field.trim();
                field.starts_with("{3,")
                    || super::form::layout_8_5_1::palette_color_8_5_1(field).is_some()
            }) =>
        {
            Some(("PaletteColor", "PaletteColors"))
        }
        0 if header_index == Some(1) => Some(("IntegrationService", "IntegrationServices")),
        1 if header_index == Some(1) && field_starts_with(fields.get(2), r#"{"Pattern""#) => {
            Some(("EventSubscription", "EventSubscriptions"))
        }
        1 if header_index == Some(1) && field_starts_with(fields.get(1), "{2,") => {
            Some(("SessionParameter", "SessionParameters"))
        }
        1 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("XDTOPackage", "XDTOPackages"))
        }
        1 if header_index == Some(1) => Some(("Bot", "Bots")),
        // The command wrapper's own leading count (`{8,` or `{9,`, one
        // trailing `OnMainServerUnavailableBehavior` slot apart -- see
        // `parse_common_command_properties_from_text`) is not a stable
        // discriminator: a literal `{9,` substring check missed every
        // record that happened to omit that slot. The `{2,<uuid>,<value
        // id>}` GeneratedType pair right after the code is stable instead
        // -- `COMMAND_VALUE_UUID` is the fixed, evidenced ValueId every
        // Command-family object carries there regardless of the wrapper's
        // own declared count (see docs/evidence/utility-objects-8.3.27.md).
        2 if contains_common_command_value_marker(text, uuid) => {
            Some(("CommonCommand", "CommonCommands"))
        }
        2 if header_index == Some(2) && field_is_quoted_string(fields.get(1)) => {
            Some(("HTTPService", "HTTPServices"))
        }
        2 if header_index == Some(2) && field_starts_with(fields.get(1), "{") => {
            Some(("WSReference", "WSReferences"))
        }
        4 if header_index == Some(2) && field_is_quoted_string(fields.get(1)) => {
            Some(("WebService", "WebServices"))
        }
        2 if header_index == Some(1)
            && fields.get(2).copied().and_then(parse_uuid_field).is_some()
            && field_starts_with(fields.get(3), "{0,") =>
        {
            Some(("FunctionalOption", "FunctionalOptions"))
        }
        2 if header_index == Some(1) && field_starts_with(fields.get(1), "{0,") => {
            Some(("SettingsStorage", "SettingsStorages"))
        }
        3 if header_index == Some(6) => Some(("CommandGroup", "CommandGroups")),
        3 if header_index == Some(3) => Some(("StyleItem", "StyleItems")),
        // Code 3 with a lone header field is shared between `Style` and a
        // `CommonPicture` written in the older of the two layouts the platform
        // has used. They part on the header block's own leading member, the
        // one that states how many members the block carries:
        //
        // * `{3,{3,{1,0,<uuid>},<Name>,<Synonym>,<Comment>,0,0,<nil uuid>,0}}`
        //   -- eight-member block -- is a `Style`. ERP УХ 3.2.12.6 carries
        //   exactly two, `Основной` among them.
        // * `{3,{2,{1,0,<uuid>},<Name>,<Synonym>,<Comment>,0,0,<nil uuid>}}`
        //   -- seven members, no trailing `0` -- is a `CommonPicture`.
        //
        // The same picture re-imported through the platform comes back as
        // code 4 with the eight-member block, so only a distribution written
        // before that change carries the code-3 spelling; УТ 11.5.27.75 and
        // БСП демо 3.1.12.297 have none. ERP УХ has 673, and all 673 were
        // routed to `Styles/` while the very same 673 `CommonPictures/` went
        // unwritten -- the two sets coincide exactly.
        3 if header_index == Some(1)
            && fields.len() == 2
            && field_starts_with(fields.get(1), "{2,") =>
        {
            Some(("CommonPicture", "CommonPictures"))
        }
        3 if header_index == Some(1) && fields.len() == 2 => Some(("Style", "Styles")),
        3 if header_index == Some(1) => Some(("DocumentNumerator", "DocumentNumerators")),
        2 if header_index == Some(1)
            && field_is_quoted_string(fields.get(2))
            && field_is_quoted_string(fields.get(3)) =>
        {
            Some(("ScheduledJob", "ScheduledJobs"))
        }
        4 if is_form_metadata_text(text, uuid) => Some(("CommonForm", "CommonForms")),
        4 if is_common_template_metadata_fields(&fields, uuid) => {
            Some(("CommonTemplate", "CommonTemplates"))
        }
        4 if header_index == Some(1) => Some(("CommonPicture", "CommonPictures")),
        5 => Some(("CommonAttribute", "CommonAttributes")),
        6 if header_index == Some(1) => Some(("Role", "Roles")),
        6 => Some(("Sequence", "Sequences")),
        9 => Some(("CommonCommand", "CommonCommands")),
        12 if header_index == Some(1) => Some(("CommonModule", "CommonModules")),
        14 => Some(("FilterCriterion", "FilterCriteria")),
        16 if header_index == Some(3) && fields.len() == 12 => {
            Some(("DataProcessor", "DataProcessors"))
        }
        16 => Some(("Constant", "Constants")),
        17 => Some(("DataProcessor", "DataProcessors")),
        19 => Some(("Report", "Reports")),
        20 if header_index == Some(5) => Some(("Enum", "Enums")),
        20 if header_index == Some(3) => Some(("Report", "Reports")),
        // Discriminator 21 is shared between the register families and
        // Subsystem: a real Calculation/Accounting register always embeds a
        // long fixed preamble of bare well-known type-id UUIDs before its
        // `{1,0,<uuid>}` header (confirmed on ERP UH 3.2.12.6's real
        // `Начисления` calculation register and `МеждународныйБезКорреспонденции`
        // accounting register: header_index == 15 for both), while a
        // Subsystem's header is the very next field after the code (header_index
        // == 1, exactly as for the unambiguous code-22 Subsystem shape below).
        // Confirmed against the same corpus for the Subsystem `WebСервисУХ`,
        // which the platform exports as `Subsystems/WebСервисУХ.xml` while we
        // previously misrouted it to `CalculationRegisters/WebСервисУХ.xml`.
        21 if header_index == Some(1) => Some(("Subsystem", "Subsystems")),
        21 if is_code21_accounting_register_fields(&fields, uuid) => {
            Some(("AccountingRegister", "AccountingRegisters"))
        }
        21 => Some(("CalculationRegister", "CalculationRegisters")),
        22 if header_index == Some(1) => Some(("Subsystem", "Subsystems")),
        22 if field_is_unsigned_integer(fields.get(1)) => {
            Some(("AccountingRegister", "AccountingRegisters"))
        }
        26 => Some(("DocumentJournal", "DocumentJournals")),
        28 => Some(("AccumulationRegister", "AccumulationRegisters")),
        30 => Some(("BusinessProcess", "BusinessProcesses")),
        32 => Some(("ChartOfAccounts", "ChartsOfAccounts")),
        33 if header_index == Some(1) => Some(("Task", "Tasks")),
        33 => Some(("InformationRegister", "InformationRegisters")),
        34 => Some(("ChartOfCharacteristicTypes", "ChartsOfCharacteristicTypes")),
        35 => Some(("ChartOfCalculationTypes", "ChartsOfCalculationTypes")),
        36 | 37 => Some(("ExchangePlan", "ExchangePlans")),
        40 => Some(("Document", "Documents")),
        56 | 57 => Some(("Catalog", "Catalogs")),
        _ => None,
    }
}

pub(super) fn is_code21_accounting_register_fields(fields: &[&str], uuid: &str) -> bool {
    if fields.first().map(|field| field.trim()) != Some("21") {
        return false;
    }
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return false;
    };

    field_is_unsigned_integer(fields.get(header_index + 1))
        && field_is_unsigned_integer(fields.get(header_index + 2))
        && fields
            .get(header_index + 3)
            .copied()
            .and_then(parse_uuid_field)
            .is_some()
        && fields
            .get(header_index + 4)
            .copied()
            .and_then(parse_uuid_field)
            .is_some()
        && field_is_unsigned_integer(fields.get(header_index + 5))
        && field_is_unsigned_integer(fields.get(header_index + 6))
        && field_is_unsigned_integer(fields.get(header_index + 7))
        && field_is_unsigned_integer(fields.get(header_index + 8))
        && field_starts_with(fields.get(header_index + 9), "{")
}

pub(super) fn parse_metadata_header_from_text(text: &str, uuid: &str) -> Option<MetadataHeader> {
    // `Name`, `Synonym` and `Comment` follow the object's own header tuple in
    // both header-block layouts the platform writes; only the tuple's own
    // first member differs. See `metadata_header_field_index` for the two
    // spellings and where each was observed.
    let mut offset = match super::registered_header_marker(text, uuid) {
        Some(found) => found?,
        None => [format!("{{1,0,{uuid}}},"), format!("{{0,0,{uuid}}},")]
            .into_iter()
            .find_map(|marker| text.find(&marker).map(|start| start + marker.len()))?,
    };
    offset = skip_ascii_ws_at(text, offset);
    let (name, consumed) = parse_1c_quoted_string_with_len(&text[offset..])?;
    offset += consumed;
    offset = expect_comma_at(text, offset)?;
    offset = skip_ascii_ws_at(text, offset);
    let synonym_end = scan_1c_braced_value(text, offset)?;
    let synonyms = parse_1c_synonyms(&text[offset..synonym_end]);
    offset = expect_comma_at(text, synonym_end)?;
    offset = skip_ascii_ws_at(text, offset);
    let (comment, _) = parse_1c_quoted_string_with_len(&text[offset..])?;

    Some(MetadataHeader {
        uuid: uuid.to_string(),
        name,
        synonyms,
        comment,
        template_type_code: template_type_code_from_metadata_text(text, uuid),
    })
}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    #[test]
    fn a_count_beyond_the_fields_leaves_the_text_as_it_is() {
        // Counts come from the file: arithmetic on them must not overflow.
        for text in [
            "{3,{1,0,00000000-0000-0000-0000-000000000015},\"Р\",{0},\"\",1,18446744073709551615,x}",
            "{3,{1,0,00000000-0000-0000-0000-000000000015},\"Р\",{0},\"\",1,0,\
             00000000-0000-0000-0000-000000000001,18446744073709551615}",
        ] {
            let text = text.replace("18446744073709551615", "18446744073709551615");
            assert_eq!(without_adoption_headers(&text), text.as_str());
        }
    }
}
