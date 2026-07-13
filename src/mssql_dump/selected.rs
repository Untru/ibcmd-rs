use std::collections::{BTreeMap, BTreeSet};

use super::{
    ConfigRow, MetadataCommandReference, MetadataTextRow, command_interface_placement_name,
    command_interface_reference_entries_from_text, command_interface_standard_command, decode_hex,
    inflate_raw_deflate, is_form_metadata_text, is_template_metadata_text, is_uuid_text,
    metadata_kind_needs_form_template_reference_indexes, metadata_text_row_from_text,
    nested_command_headers_from_text, parse_command_interface_common_flag,
    parse_generated_type_entries_from_text, recalculation_object_fields, split_1c_braced_fields,
    template_template_type_from_metadata, uuid_like_values,
};

pub(super) fn selected_export_needs_broad_metadata_indexes(
    extract_module_text: bool,
    file_names: &BTreeSet<String>,
    selected_metadata_texts: &[MetadataTextRow],
) -> bool {
    let metadata_by_id = selected_metadata_texts
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    for file_name in file_names {
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        let Some(row) = metadata_by_id.get(metadata_id) else {
            return true;
        };
        if !selected_body_suffix_is_self_contained(row.kind.as_deref(), suffix, extract_module_text)
        {
            return true;
        }
    }
    selected_metadata_texts
        .iter()
        .any(metadata_text_needs_broad_indexes)
}

#[derive(Clone, Copy)]
pub(super) struct SourceReferenceIndexNeeds {
    pub(super) command_refs: bool,
    pub(super) metadata_refs: bool,
    pub(super) type_index: bool,
    pub(super) form_refs: bool,
    pub(super) template_refs: bool,
    pub(super) subsystem_refs: bool,
    pub(super) object_refs: bool,
    pub(super) metadata_order: bool,
    pub(super) field_refs: bool,
    pub(super) functional_option_refs: bool,
    pub(super) help_refs: bool,
    pub(super) standalone_refs: bool,
    pub(super) body_owners: bool,
}

impl SourceReferenceIndexNeeds {
    pub(super) fn full() -> Self {
        Self {
            command_refs: true,
            metadata_refs: true,
            type_index: true,
            form_refs: true,
            template_refs: true,
            subsystem_refs: true,
            object_refs: true,
            metadata_order: true,
            field_refs: true,
            functional_option_refs: true,
            help_refs: true,
            standalone_refs: true,
            body_owners: true,
        }
    }

    fn none() -> Self {
        Self {
            command_refs: false,
            metadata_refs: false,
            type_index: false,
            form_refs: false,
            template_refs: false,
            subsystem_refs: false,
            object_refs: false,
            metadata_order: false,
            field_refs: false,
            functional_option_refs: false,
            help_refs: false,
            standalone_refs: false,
            body_owners: false,
        }
    }

    pub(super) fn needs_broad_metadata(self) -> bool {
        self.command_refs || self.needs_broad_metadata_without_command_refs()
    }

    pub(super) fn needs_broad_metadata_without_command_refs(self) -> bool {
        self.type_index
            || self.form_refs
            || self.template_refs
            || self.subsystem_refs
            || self.object_refs
            || self.metadata_order
            || self.field_refs
            || self.functional_option_refs
            || self.help_refs
            || self.standalone_refs
            || self.body_owners
    }
}

pub(super) fn selected_configuration_source_asset_index_needs(
    file_names: &BTreeSet<String>,
) -> Option<SourceReferenceIndexNeeds> {
    if file_names.is_empty() {
        return None;
    }
    let mut needs = SourceReferenceIndexNeeds::none();
    for file_name in file_names {
        if file_name == "versions" {
            continue;
        }
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            if is_uuid_text(file_name) {
                continue;
            }
            return None;
        };
        if metadata_id.is_empty() {
            return None;
        }
        match suffix {
            "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "10" | "b" | "c" | "15" | "16" => {}
            "8" => {
                needs.form_refs = true;
            }
            "9" => {
                needs.command_refs = true;
                needs.metadata_refs = true;
            }
            "a" => {
                needs.metadata_refs = true;
            }
            _ => return None,
        }
    }
    Some(needs)
}

pub(super) fn selected_configuration_source_asset_index_needs_with_metadata(
    file_names: &BTreeSet<String>,
    metadata_texts: &[MetadataTextRow],
) -> Option<SourceReferenceIndexNeeds> {
    let mut needs = selected_configuration_source_asset_index_needs(file_names)?;
    let metadata_by_id = metadata_texts
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    if file_names.iter().any(|file_name| {
        file_name
            .strip_suffix(".0")
            .and_then(|metadata_id| metadata_by_id.get(metadata_id))
            .is_some_and(|row| is_form_metadata_text(&row.text, &row.file_name))
    }) {
        needs.field_refs = true;
    }
    if file_names.iter().any(|file_name| {
        let Some(metadata_id) = file_name.strip_suffix(".0") else {
            return false;
        };
        let Some(row) = metadata_by_id.get(metadata_id) else {
            return true;
        };
        if !is_template_metadata_text(&row.text, &row.file_name) {
            return false;
        }
        match row
            .header
            .as_ref()
            .and_then(|header| template_template_type_from_metadata(header))
        {
            Some("DataCompositionSchema") | None => true,
            Some(_) => false,
        }
    }) {
        needs.object_refs = true;
    }
    if metadata_texts.iter().any(|row| {
        row.object_code == Some(4)
            && recalculation_object_fields(&row.text, &row.file_name).is_some()
    }) {
        needs.object_refs = true;
        needs.type_index = true;
    }
    Some(needs)
}

pub(super) fn selected_metadata_source_reference_index_needs(
    selected_metadata_texts: &[MetadataTextRow],
) -> Option<SourceReferenceIndexNeeds> {
    if selected_metadata_texts.is_empty() {
        return None;
    }

    let mut needs = SourceReferenceIndexNeeds::none();
    for row in selected_metadata_texts {
        match row.kind.as_deref() {
            Some(kind) if metadata_kind_needs_form_template_reference_indexes(kind) => {
                needs.form_refs = true;
                needs.template_refs = true;
                needs.type_index = true;
                if kind == "ExchangePlan" {
                    needs.object_refs = true;
                    needs.metadata_order = true;
                } else if kind == "CalculationRegister" {
                    needs.object_refs = true;
                }
            }
            _ => return None,
        }
    }

    Some(needs)
}

pub(super) fn selected_configuration_direct_metadata_reference_file_names(
    rows: &[ConfigRow],
) -> BTreeSet<String> {
    let mut file_names = BTreeSet::new();
    for row in rows {
        let Some((_, suffix)) = row.file_name.rsplit_once('.') else {
            continue;
        };
        if !matches!(suffix, "9" | "a") {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        file_names.extend(uuid_like_values(&text));
    }
    file_names
}

pub(super) fn selected_metadata_direct_reference_file_names(
    rows: &[MetadataTextRow],
) -> BTreeSet<String> {
    let selected = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut file_names = BTreeSet::new();
    for row in rows {
        for value in uuid_like_values(&row.text) {
            if !selected.contains(value.as_str()) {
                file_names.insert(value);
            }
        }
    }
    file_names
}

pub(super) fn selected_body_direct_reference_file_names(rows: &[ConfigRow]) -> BTreeSet<String> {
    let selected = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut file_names = BTreeSet::new();
    for row in rows {
        let Some((_, suffix)) = row.file_name.rsplit_once('.') else {
            continue;
        };
        if !matches!(
            suffix,
            "0" | "1" | "2" | "3" | "5" | "6" | "7" | "8" | "9" | "a" | "15" | "16"
        ) {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        for value in uuid_like_values(&text) {
            if !selected.contains(value.as_str()) {
                file_names.insert(value);
            }
        }
    }
    file_names
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct CommandInterfaceCommandReference {
    pub(super) code: String,
    pub(super) uuid: String,
}

pub(super) fn selected_configuration_command_interface_command_refs(
    rows: &[ConfigRow],
) -> Option<BTreeSet<CommandInterfaceCommandReference>> {
    let mut refs = BTreeSet::new();
    for row in rows {
        let Some((_, suffix)) = row.file_name.rsplit_once('.') else {
            continue;
        };
        if suffix != "9" {
            continue;
        }
        let bytes = decode_hex(&row.binary_hex).ok()?;
        refs.extend(command_interface_command_refs_from_blob(&bytes)?);
    }
    Some(refs)
}

pub(super) fn command_interface_command_refs_from_blob(
    bytes: &[u8],
) -> Option<BTreeSet<CommandInterfaceCommandReference>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }

    command_interface_order_command_refs(&fields)
        .or_else(|| command_interface_visibility_command_refs(&fields))
}

fn command_interface_order_command_refs(
    fields: &[&str],
) -> Option<BTreeSet<CommandInterfaceCommandReference>> {
    if fields.get(1)?.trim() != "0" {
        return None;
    }
    let count = fields.get(4)?.trim().parse::<usize>().ok()?;
    let default_group_uuid = fields.get(5)?.trim();
    if !is_uuid_text(default_group_uuid) {
        return None;
    }
    let mut refs = BTreeSet::new();
    let mut index = 6usize;
    for _ in 0..count {
        insert_command_interface_command_ref(fields.get(index)?, &mut refs)?;
        index += 1;
        if !is_uuid_text(fields.get(index)?.trim()) {
            return None;
        }
        index += 1;
    }
    Some(refs)
}

fn command_interface_visibility_command_refs(
    fields: &[&str],
) -> Option<BTreeSet<CommandInterfaceCommandReference>> {
    let count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let mut refs = BTreeSet::new();
    let mut index = 3usize;
    for _ in 0..count {
        insert_command_interface_command_ref(fields.get(index)?, &mut refs)?;
        index += 1;
        parse_command_interface_common_flag(fields.get(index)?)?;
        index += 1;
    }
    collect_command_interface_placement_tail_refs(fields, &mut index, &mut refs);
    collect_command_interface_order_tail_refs(fields, &mut index, &mut refs);
    Some(refs)
}

fn collect_command_interface_placement_tail_refs(
    fields: &[&str],
    index: &mut usize,
    refs: &mut BTreeSet<CommandInterfaceCommandReference>,
) -> Option<()> {
    if fields.get(*index)?.trim() != "1" {
        return None;
    }
    *index += 1;
    let count = fields.get(*index)?.trim().parse::<usize>().ok()?;
    *index += 1;
    for _ in 0..count {
        insert_command_interface_command_ref(fields.get(*index)?, refs)?;
        *index += 1;
        if !is_uuid_text(fields.get(*index)?.trim()) {
            return None;
        }
        *index += 1;
        command_interface_placement_name(fields.get(*index)?.trim())?;
        *index += 1;
    }
    Some(())
}

fn collect_command_interface_order_tail_refs(
    fields: &[&str],
    index: &mut usize,
    refs: &mut BTreeSet<CommandInterfaceCommandReference>,
) -> Option<()> {
    if fields.get(*index)?.trim() != "1" {
        return None;
    }
    *index += 1;
    let count = fields.get(*index)?.trim().parse::<usize>().ok()?;
    *index += 1;
    for _ in 0..count {
        if !is_uuid_text(fields.get(*index)?.trim()) {
            return None;
        }
        *index += 1;
        insert_command_interface_command_ref(fields.get(*index)?, refs)?;
        *index += 1;
    }
    Some(())
}

fn insert_command_interface_command_ref(
    field: &str,
    refs: &mut BTreeSet<CommandInterfaceCommandReference>,
) -> Option<()> {
    let command_ref = split_1c_braced_fields(field, 0)?;
    let code = command_ref.first()?.trim();
    let uuid = command_ref.get(1).map(|value| value.trim())?;
    if !code.chars().all(|ch| ch.is_ascii_digit()) || !is_uuid_text(uuid) {
        return None;
    }
    refs.insert(CommandInterfaceCommandReference {
        code: code.to_string(),
        uuid: uuid.to_string(),
    });
    Some(())
}

pub(super) fn unresolved_command_interface_reference_uuids(
    refs: &BTreeSet<CommandInterfaceCommandReference>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> BTreeSet<String> {
    refs.iter()
        .filter(|reference| {
            !command_interface_reference_is_resolved(reference, command_refs, metadata_refs)
        })
        .map(|reference| reference.uuid.clone())
        .collect()
}

fn command_interface_reference_is_resolved(
    reference: &CommandInterfaceCommandReference,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
) -> bool {
    if command_refs.contains_key(&reference.uuid) {
        return true;
    }
    let Some(metadata) = metadata_refs.get(&reference.uuid) else {
        return false;
    };
    (matches!(reference.code.as_str(), "0" | "100")
        && command_interface_standard_command(&metadata.kind).is_some())
        || reference.code == "1"
}

#[allow(dead_code)]
pub(super) fn selected_command_owner_metadata_rows(
    rows: &[ConfigRow],
    unresolved_command_refs: &BTreeSet<String>,
) -> Option<Vec<ConfigRow>> {
    let mut found = BTreeSet::new();
    let mut owner_rows = Vec::new();
    for row in rows {
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        let text = text.trim_start_matches('\u{feff}').to_string();
        if !unresolved_command_refs
            .iter()
            .any(|uuid| text.contains(uuid))
        {
            continue;
        }
        let Some(text_row) = metadata_text_row_from_text(&row.file_name, text) else {
            continue;
        };
        let matching_refs = command_interface_reference_entries_from_text(&text_row)
            .into_iter()
            .filter_map(|(uuid, _)| unresolved_command_refs.contains(&uuid).then_some(uuid))
            .collect::<BTreeSet<_>>();
        if matching_refs.is_empty() {
            continue;
        }
        found.extend(matching_refs);
        owner_rows.push(row.clone());
    }
    if unresolved_command_refs.is_subset(&found) {
        Some(owner_rows)
    } else {
        None
    }
}

pub(super) fn selected_owner_metadata_rows_for_uuids(
    rows: &[ConfigRow],
    unresolved_uuids: &BTreeSet<String>,
) -> (Vec<ConfigRow>, BTreeSet<String>) {
    let mut found = BTreeSet::new();
    let mut owner_rows = Vec::new();
    for row in rows {
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Ok(inflated) = inflate_raw_deflate(&bytes) else {
            continue;
        };
        let Ok(text) = String::from_utf8(inflated) else {
            continue;
        };
        let text = text.trim_start_matches('\u{feff}').to_string();
        if !unresolved_uuids.iter().any(|uuid| text.contains(uuid)) {
            continue;
        }
        let Some(text_row) = metadata_text_row_from_text(&row.file_name, text) else {
            continue;
        };
        let generated_type_matches = parse_generated_type_entries_from_text(&text_row)
            .into_iter()
            .flatten()
            .filter_map(|(type_id, _)| unresolved_uuids.contains(&type_id).then_some(type_id))
            .collect::<BTreeSet<_>>();
        let nested_command_matches =
            nested_command_headers_from_text(&text_row.text, &text_row.file_name)
                .into_iter()
                .filter_map(|header| {
                    unresolved_uuids
                        .contains(&header.uuid)
                        .then_some(header.uuid)
                })
                .collect::<BTreeSet<_>>();
        let matches = generated_type_matches
            .into_iter()
            .chain(nested_command_matches.into_iter())
            .collect::<BTreeSet<_>>();
        if matches.is_empty() {
            continue;
        }
        found.extend(matches);
        owner_rows.push(row.clone());
    }
    (owner_rows, found)
}

fn selected_body_suffix_is_self_contained(
    kind: Option<&str>,
    suffix: &str,
    extract_module_text: bool,
) -> bool {
    matches!((kind, suffix), (Some("WSReference"), "0"))
        || (extract_module_text && matches!((kind, suffix), (Some("IntegrationService"), "0")))
}

fn metadata_text_needs_broad_indexes(row: &MetadataTextRow) -> bool {
    !matches!(
        row.kind.as_deref(),
        Some("CommonModule")
            | Some("CommonPicture")
            | Some("DocumentNumerator")
            | Some("IntegrationService")
            | Some("Language")
            | Some("Role")
            | Some("StyleItem")
            | Some("WSReference")
    )
}
