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
    let object_code = parse_metadata_object_code(&text);
    let fields_missing = metadata_object_fields(&text).is_none();
    let header = parse_metadata_header_from_text(&text, file_name);
    let (kind, folder) = match object_code {
        Some(12) => (Some("CommonModule".to_string()), Some("CommonModules")),
        Some(code) => metadata_source_for_text(code, &text, file_name)
            .map(|(kind, folder)| (Some(kind.to_string()), Some(folder)))
            .unwrap_or((None, None)),
        None => (None, None),
    };
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
    } else if row.folder.is_none() {
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

pub(super) fn metadata_source_for_object_text(
    code: u32,
    object_text: &str,
    uuid: &str,
) -> Option<(&'static str, &'static str)> {
    let wrapped_text = format!("{{1,{object_text}}}");
    let fields = split_1c_braced_fields(object_text, 0)?;
    metadata_source_for_object_fields(code, &wrapped_text, uuid, &fields)
}

pub(super) fn metadata_source_for_object_fields(
    code: u32,
    text: &str,
    uuid: &str,
    fields: &[&str],
) -> Option<(&'static str, &'static str)> {
    let header_index = metadata_header_field_index(&fields, uuid);

    match code {
        0 if header_index == Some(1) && field_starts_with(fields.get(2), "{0,") => {
            Some(("FunctionalOptionsParameter", "FunctionalOptionsParameters"))
        }
        0 if header_index == Some(1) && field_is_quoted_string(fields.get(2)) => {
            Some(("Language", "Languages"))
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
        2 if contains_wrapped_metadata_object_code(text, 9, uuid) => {
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
        16 => Some(("Constant", "Constants")),
        17 => Some(("DataProcessor", "DataProcessors")),
        19 => Some(("Report", "Reports")),
        20 if header_index == Some(5) => Some(("Enum", "Enums")),
        20 if header_index == Some(3) => Some(("Report", "Reports")),
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
    let marker = format!("{{1,0,{uuid}}},");
    let mut offset = text.find(&marker)? + marker.len();
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
