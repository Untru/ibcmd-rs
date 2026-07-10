use super::*;

#[derive(Debug, Clone)]
pub(super) struct BodyOwnerSourceReference {
    pub(super) kind: String,
    pub(super) object_path: PathBuf,
}

#[allow(dead_code)]
pub(super) fn build_body_owner_source_index(
    rows: &[ConfigRow],
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, BodyOwnerSourceReference> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_body_owner_source_index_from_texts(&metadata_texts, subsystem_refs)
}

pub(super) fn build_body_owner_source_index_from_texts(
    rows: &[MetadataTextRow],
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, BodyOwnerSourceReference> {
    let mut index = BTreeMap::new();
    for row in rows {
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        let object_path = if kind == "Subsystem" {
            subsystem_refs
                .get(&row.file_name)
                .map(|subsystem_ref| subsystem_ref.relative_path.with_extension(""))
                .unwrap_or_else(|| {
                    PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
                })
        } else {
            PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
        };
        index.insert(
            row.file_name.clone(),
            BodyOwnerSourceReference {
                kind: kind.to_string(),
                object_path,
            },
        );
    }
    index
}

pub(super) fn configuration_module_groups(file_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in file_names {
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        if metadata_id.is_empty() {
            continue;
        }
        suffixes_by_id
            .entry(metadata_id)
            .or_default()
            .insert(suffix);
    }
    suffixes_by_id
        .into_iter()
        .filter(|(metadata_id, suffixes)| {
            !file_names.contains(*metadata_id) && is_configuration_module_group(suffixes)
        })
        .map(|(metadata_id, _)| metadata_id.to_string())
        .collect()
}

pub(super) fn file_names_have_standalone_content_asset<'a>(
    file_names: impl IntoIterator<Item = &'a str>,
) -> bool {
    !standalone_content_asset_file_names(file_names).is_empty()
}

pub(super) fn standalone_content_asset_file_names<'a>(
    file_names: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<String> {
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in file_names {
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        if metadata_id.is_empty() {
            continue;
        }
        suffixes_by_id
            .entry(metadata_id)
            .or_default()
            .insert(suffix);
    }

    suffixes_by_id
        .into_iter()
        .filter(|(_, suffixes)| suffixes.contains("f") && is_configuration_module_group(suffixes))
        .map(|(metadata_id, _)| format!("{metadata_id}.f"))
        .collect()
}

pub(super) fn standalone_content_reference_uuids_from_config_rows(
    rows: &[ConfigRow],
) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    for row in rows {
        if !row.file_name.ends_with(".f") {
            continue;
        }
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        refs.extend(standalone_content_reference_uuids_from_blob(&bytes));
    }
    refs
}

pub(super) fn standalone_content_reference_uuids_from_blob(bytes: &[u8]) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    let Ok(inflated) = inflate_raw_deflate(bytes) else {
        return refs;
    };
    let Ok(text) = String::from_utf8(inflated) else {
        return refs;
    };
    let Some(fields) = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0) else {
        return refs;
    };
    if fields.first().map(|field| field.trim()) != Some("2") {
        return refs;
    }
    let Some(count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return refs;
    };
    if fields.len() < 2 + count {
        return refs;
    }
    refs.extend(
        fields
            .iter()
            .skip(2)
            .take(count)
            .filter_map(|field| parse_non_zero_uuid(field.trim())),
    );

    let mut index = 2 + count;
    let Some(child_count) = fields
        .get(index)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return refs;
    };
    index += 1;
    if fields.len() < index + child_count {
        return refs;
    }
    refs.extend(
        fields
            .iter()
            .skip(index)
            .take(child_count)
            .filter_map(|field| parse_non_zero_uuid(field.trim())),
    );
    refs.extend(
        fields
            .iter()
            .skip(index + child_count)
            .filter_map(|field| parse_non_zero_uuid(field.trim())),
    );
    refs
}

pub(super) fn dynamic_source_asset(
    context: &DumpRowContext<'_>,
    file_name: &str,
    bytes: &[u8],
) -> Option<SourceAsset> {
    let (owner_uuid, suffix) = file_name.rsplit_once('.')?;
    if owner_uuid.is_empty() {
        return None;
    }

    if let Some(form_ref) = context.form_refs.get(owner_uuid)
        && suffix != "0"
        && parse_help_blob_pages(bytes).is_some()
    {
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        return Some(SourceAsset {
            primary_path: form_dir.join("Ext").join("Help.xml"),
            kind: SourceAssetKind::Help,
        });
    }

    if context.configuration_module_groups.contains(owner_uuid)
        && matches!(suffix, "9" | "a")
        && parse_command_interface_blob(bytes, context.command_refs, context.metadata_refs)
            .is_some()
    {
        let path = if suffix == "9" {
            "Ext/MainSectionCommandInterface.xml"
        } else {
            "Ext/CommandInterface.xml"
        };
        return Some(SourceAsset {
            primary_path: PathBuf::from(path),
            kind: SourceAssetKind::CommandInterface,
        });
    }

    let owner = context.body_owners.get(owner_uuid)?;
    if owner.kind == "Role"
        && suffix == "0"
        && parse_role_rights_blob(bytes, context.role_rights_object_refs, context.field_refs)
            .is_some()
    {
        return Some(SourceAsset {
            primary_path: owner.object_path.join("Ext").join("Rights.xml"),
            kind: SourceAssetKind::RoleRights,
        });
    }
    if owner.kind == "AccumulationRegister"
        && suffix == "3"
        && parse_accumulation_register_aggregates_blob(bytes).is_some()
    {
        let register_name = context
            .object_refs
            .get(owner_uuid)
            .and_then(|reference| reference.strip_prefix("AccumulationRegister."))
            .map(str::to_string)
            .or_else(|| {
                owner
                    .object_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })?;
        return Some(SourceAsset {
            primary_path: owner.object_path.join("Ext").join("Aggregates.xml"),
            kind: SourceAssetKind::AccumulationRegisterAggregates { register_name },
        });
    }
    if matches!(suffix, "0" | "1")
        && parse_command_interface_blob(bytes, context.command_refs, context.metadata_refs)
            .is_some()
    {
        return Some(SourceAsset {
            primary_path: owner.object_path.join("Ext").join("CommandInterface.xml"),
            kind: SourceAssetKind::CommandInterface,
        });
    }
    if parse_help_blob_pages(bytes).is_some() {
        let preferred_help_body_id = preferred_help_body_id(&owner.kind, owner_uuid);
        if context.file_names.contains(preferred_help_body_id.as_str())
            && file_name != preferred_help_body_id
        {
            return None;
        }
        return Some(SourceAsset {
            primary_path: owner.object_path.join("Ext").join("Help.xml"),
            kind: SourceAssetKind::Help,
        });
    }
    if let Some(model) = predefined_data_source_model(&owner.kind)
        && suffix == model.suffix
        && parse_predefined_data_blob_with_model(bytes, context.type_index, model).is_some()
    {
        return Some(SourceAsset {
            primary_path: owner.object_path.join("Ext").join("Predefined.xml"),
            kind: SourceAssetKind::PredefinedData { model },
        });
    }
    if let Some(module_file) = module_owner_module_file(&owner.kind, suffix)
        && unpack_module_blob_text(bytes).is_err()
        && is_binary_module_container(bytes)
    {
        return Some(SourceAsset {
            primary_path: owner
                .object_path
                .join("Ext")
                .join(Path::new(module_file).with_extension("bin")),
            kind: SourceAssetKind::InflatedBinary,
        });
    }
    None
}

pub(super) fn is_binary_module_container(bytes: &[u8]) -> bool {
    let Ok(inflated) = inflate_raw_deflate(bytes) else {
        return false;
    };
    let Some(names) = v8_container_element_names(&inflated) else {
        return false;
    };
    names.contains("image") && names.contains("info") && !names.contains("text")
}

pub(super) fn v8_container_element_names(bytes: &[u8]) -> Option<BTreeSet<String>> {
    const V8_MAGIC_NUMBER: u32 = 0x7fff_ffff;
    const FILE_HEADER_SIZE: usize = 16;
    const BLOCK_HEADER_SIZE: usize = 31;
    const ELEM_ADDR_SIZE: usize = 12;
    const ELEM_HEADER_PREFIX_SIZE: usize = 20;

    if bytes.len() < FILE_HEADER_SIZE + BLOCK_HEADER_SIZE {
        return None;
    }
    if read_le_u32(bytes, 0)? != V8_MAGIC_NUMBER {
        return None;
    }
    if !matches!(read_le_u32(bytes, 8)?, 1 | 2) {
        return None;
    }
    let toc_header = read_v8_block_header(bytes, FILE_HEADER_SIZE)?;
    let toc_start = FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
    let toc_end = toc_start.checked_add(toc_header.0)?;
    if toc_end > bytes.len() || toc_header.0 % ELEM_ADDR_SIZE != 0 {
        return None;
    }
    let mut names = BTreeSet::new();
    for entry in bytes[toc_start..toc_end].chunks_exact(ELEM_ADDR_SIZE) {
        if read_le_u32(entry, 8)? != V8_MAGIC_NUMBER {
            continue;
        }
        let header_addr = read_le_u32(entry, 0)? as usize;
        let header = read_v8_block_payload(bytes, header_addr)?;
        if header.len() < ELEM_HEADER_PREFIX_SIZE {
            return None;
        }
        let mut units = Vec::new();
        for pair in header[ELEM_HEADER_PREFIX_SIZE..].chunks_exact(2) {
            let unit = u16::from_le_bytes([pair[0], pair[1]]);
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        names.insert(String::from_utf16(&units).ok()?);
    }
    Some(names)
}

pub(super) fn read_v8_block_payload(bytes: &[u8], offset: usize) -> Option<&[u8]> {
    let header = read_v8_block_header(bytes, offset)?;
    let start = offset.checked_add(31)?;
    let data_end = start.checked_add(header.0)?;
    let page_end = start.checked_add(header.1)?;
    if data_end > bytes.len() || page_end > bytes.len() || header.2 != 0x7fff_ffff {
        return None;
    }
    Some(&bytes[start..data_end])
}

pub(super) fn read_v8_block_header(bytes: &[u8], offset: usize) -> Option<(usize, usize, u32)> {
    let end = offset.checked_add(31)?;
    let raw = bytes.get(offset..end)?;
    if raw[0] != b'\r'
        || raw[1] != b'\n'
        || raw[10] != b' '
        || raw[19] != b' '
        || raw[28] != b' '
        || raw[29] != b'\r'
        || raw[30] != b'\n'
    {
        return None;
    }
    Some((
        parse_hex_usize(&raw[2..10])?,
        parse_hex_usize(&raw[11..19])?,
        parse_hex_u32_bytes(&raw[20..28])?,
    ))
}

pub(super) fn read_le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

pub(super) fn parse_hex_usize(bytes: &[u8]) -> Option<usize> {
    usize::from_str_radix(std::str::from_utf8(bytes).ok()?, 16).ok()
}

pub(super) fn parse_hex_u32_bytes(bytes: &[u8]) -> Option<u32> {
    u32::from_str_radix(std::str::from_utf8(bytes).ok()?, 16).ok()
}

pub(super) fn ensure_unique_source_asset_paths(
    source_assets: &BTreeMap<String, SourceAsset>,
    diagnostics: &BTreeMap<String, String>,
) -> Result<()> {
    let mut paths = BTreeMap::<String, &str>::new();
    for (file_name, asset) in source_assets {
        let path = asset.primary_path.to_string_lossy().replace('\\', "/");
        if let Some(previous_file_name) = paths.insert(path.clone(), file_name.as_str()) {
            let mut message = format!(
                "source asset output path {path} is produced by both {previous_file_name} and {file_name}"
            );
            append_source_asset_diagnostic(&mut message, previous_file_name, diagnostics);
            append_source_asset_diagnostic(&mut message, file_name, diagnostics);
            bail!("{message}");
        }
    }
    Ok(())
}

fn append_source_asset_diagnostic(
    message: &mut String,
    file_name: &str,
    diagnostics: &BTreeMap<String, String>,
) {
    if let Some(diagnostic) = diagnostics.get(file_name) {
        message.push_str("; ");
        message.push_str(file_name);
        message.push_str(": ");
        message.push_str(diagnostic);
    }
}

#[derive(Clone, Copy)]
pub(crate) enum PredefinedDataRowsetLayout {
    NestedTable,
    Root,
}

#[derive(Clone, Copy)]
pub(crate) enum PredefinedItemLayout {
    Generic,
    Account,
    Calculation,
}

#[derive(Clone, Copy)]
pub(crate) struct PredefinedDataSourceModel {
    suffix: &'static str,
    xsi_type: &'static str,
    root_tag: &'static str,
    rowset_layout: PredefinedDataRowsetLayout,
    unwrap_single_root: bool,
    item_layout: PredefinedItemLayout,
}

#[derive(Clone)]
pub(crate) enum SourceAssetKind {
    AccumulationRegisterAggregates { register_name: String },
    CommandInterface,
    ClientApplicationInterface,
    ExchangePlanContent,
    BusinessProcessFlowchart,
    DataCompositionSchema,
    ExtPicture,
    Form,
    Help,
    HomePageWorkArea,
    InflatedBase64OrBinary,
    InflatedBinary,
    MoxelSpreadsheet,
    PredefinedData { model: PredefinedDataSourceModel },
    RoleRights,
    Schedule,
    StandaloneContent,
    StyleBody,
    WsDefinition,
}

#[derive(Clone, Default)]
pub(super) struct StandaloneContentReferences {
    pub(super) object_refs: BTreeMap<String, String>,
}

pub(super) struct SourceAsset {
    pub(super) primary_path: PathBuf,
    pub(super) kind: SourceAssetKind,
}

pub(super) fn source_asset_paths_with_indexes(
    rows: &[ConfigRow],
    metadata_texts: &[MetadataTextRow],
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> BTreeMap<String, SourceAsset> {
    let rows_by_file_name = rows
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in &file_names {
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        if metadata_id.is_empty() {
            continue;
        }
        suffixes_by_id
            .entry(metadata_id)
            .or_default()
            .insert(suffix);
    }
    let role_rights_object_refs = build_role_rights_object_reference_index(object_refs, form_refs);

    let mut paths = BTreeMap::new();
    for (metadata_id, suffixes) in suffixes_by_id {
        if file_names.contains(metadata_id) {
            continue;
        }
        let is_configuration_group = is_configuration_module_group(&suffixes);
        for (suffix, path, kind) in CONFIGURATION_SOURCE_ASSET_SUFFIXES {
            if !is_configuration_group && matches!(kind, SourceAssetKind::ExtPicture) {
                continue;
            }
            if !is_configuration_group && !suffixes.contains(suffix) {
                continue;
            }
            let body_id = format!("{metadata_id}.{suffix}");
            if file_names.contains(body_id.as_str()) {
                paths.insert(
                    body_id,
                    SourceAsset {
                        primary_path: PathBuf::from(path),
                        kind: kind.clone(),
                    },
                );
            }
        }
        let standalone_id = format!("{metadata_id}.f");
        if is_configuration_group && file_names.contains(standalone_id.as_str()) {
            paths.insert(
                standalone_id,
                SourceAsset {
                    primary_path: PathBuf::from("Ext/StandaloneConfigurationContent.bin"),
                    kind: SourceAssetKind::StandaloneContent,
                },
            );
        }
        for (suffix, path) in [
            ("9", "Ext/MainSectionCommandInterface.xml"),
            ("a", "Ext/CommandInterface.xml"),
        ] {
            if !is_configuration_group && !suffixes.contains(suffix) {
                continue;
            }
            let interface_id = format!("{metadata_id}.{suffix}");
            let is_selected_header = rows_by_file_name
                .get(interface_id.as_str())
                .is_some_and(|row| row.binary_hex.is_empty());
            let is_command_interface = is_selected_header
                || rows_by_file_name
                    .get(interface_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .is_some_and(|bytes| {
                        parse_command_interface_blob(&bytes, &command_refs, &metadata_refs)
                            .is_some()
                    });
            if is_command_interface {
                paths.insert(
                    interface_id,
                    SourceAsset {
                        primary_path: PathBuf::from(path),
                        kind: SourceAssetKind::CommandInterface,
                    },
                );
            }
        }
    }
    for row in metadata_texts {
        for (body_id, asset) in source_assets_from_metadata_text(
            row,
            &file_names,
            &rows_by_file_name,
            &command_refs,
            &metadata_refs,
            &role_rights_object_refs,
            &field_refs,
            &type_index,
            &subsystem_refs,
        ) {
            paths.insert(body_id, asset);
        }
    }
    paths.extend(form_help_asset_paths(rows, &rows_by_file_name, &form_refs));
    paths.extend(form_body_asset_paths(&form_refs, &file_names));
    paths.extend(template_body_asset_paths(&template_refs, &file_names));

    paths
}

pub(super) fn template_body_asset_paths(
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    file_names: &BTreeSet<&str>,
) -> BTreeMap<String, SourceAsset> {
    let mut paths = BTreeMap::new();
    for (uuid, template_ref) in template_refs {
        let body_id = format!("{uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let Some((file_name, kind)) = template_body_source_asset(template_ref.template_type) else {
            continue;
        };
        paths.insert(
            body_id,
            SourceAsset {
                primary_path: template_ref
                    .relative_path
                    .with_extension("")
                    .join("Ext")
                    .join(file_name),
                kind,
            },
        );
    }

    paths
}

pub(super) fn template_body_source_asset(
    template_type: &str,
) -> Option<(&'static str, SourceAssetKind)> {
    match template_type {
        "AddIn" => Some(("Template.bin", SourceAssetKind::InflatedBase64OrBinary)),
        "BinaryData" => Some(("Template.bin", SourceAssetKind::InflatedBase64OrBinary)),
        "DataCompositionAppearanceTemplate" => {
            Some(("Template.xml", SourceAssetKind::InflatedBinary))
        }
        "DataCompositionSchema" => Some(("Template.xml", SourceAssetKind::DataCompositionSchema)),
        "GraphicalSchema" => Some(("Template.xml", SourceAssetKind::InflatedBinary)),
        "HTMLDocument" => Some(("Template.xml", SourceAssetKind::Help)),
        "TextDocument" => Some(("Template.txt", SourceAssetKind::InflatedBinary)),
        "SpreadsheetDocument" => Some(("Template.xml", SourceAssetKind::MoxelSpreadsheet)),
        _ => None,
    }
}

pub(super) fn form_body_asset_paths(
    form_refs: &BTreeMap<String, FormSourceReference>,
    file_names: &BTreeSet<&str>,
) -> BTreeMap<String, SourceAsset> {
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in form_refs {
        let body_id = format!("{form_uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        paths.insert(
            body_id,
            SourceAsset {
                primary_path: form_dir.join("Ext").join("Form.xml"),
                kind: SourceAssetKind::Form,
            },
        );
    }

    paths
}

const CONFIGURATION_SOURCE_ASSET_SUFFIXES: &[(&str, &str, SourceAssetKind)] = &[
    ("2", "Ext/Splash.xml", SourceAssetKind::ExtPicture),
    ("3", "Ext/Help.xml", SourceAssetKind::Help),
    (
        "4",
        "Ext/ParentConfigurations.bin",
        SourceAssetKind::InflatedBinary,
    ),
    (
        "8",
        "Ext/HomePageWorkArea.xml",
        SourceAssetKind::HomePageWorkArea,
    ),
    (
        "10",
        "Ext/MobileClientSignature.bin",
        SourceAssetKind::InflatedBinary,
    ),
    (
        "b",
        "Ext/ClientApplicationInterface.xml",
        SourceAssetKind::ClientApplicationInterface,
    ),
    (
        "c",
        "Ext/MainSectionPicture.xml",
        SourceAssetKind::ExtPicture,
    ),
];

#[allow(dead_code)]
pub(super) fn source_assets_from_metadata_blob(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Vec<(String, SourceAsset)> {
    metadata_text_row_from_blob(uuid, blob)
        .and_then(|row| {
            source_assets_from_metadata_text_inner(
                &row,
                file_names,
                rows_by_file_name,
                command_refs,
                metadata_refs,
                object_refs,
                field_refs,
                type_index,
                subsystem_refs,
            )
        })
        .unwrap_or_default()
}

pub(super) fn source_assets_from_metadata_text(
    row: &MetadataTextRow,
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Vec<(String, SourceAsset)> {
    source_assets_from_metadata_text_inner(
        row,
        file_names,
        rows_by_file_name,
        command_refs,
        metadata_refs,
        object_refs,
        field_refs,
        type_index,
        subsystem_refs,
    )
    .unwrap_or_default()
}

pub(super) fn source_assets_from_metadata_text_inner(
    row: &MetadataTextRow,
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<Vec<(String, SourceAsset)>> {
    let uuid = row.file_name.as_str();
    let kind = row.kind.as_deref()?;
    let folder = row.folder?;
    let header = row.header.as_ref()?;
    let object_path = if kind == "Subsystem" {
        subsystem_refs
            .get(uuid)
            .map(|subsystem_ref| subsystem_ref.relative_path.with_extension(""))
            .unwrap_or_else(|| {
                PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
            })
    } else {
        PathBuf::from(folder).join(sanitize_source_path_segment(&header.name))
    };
    let mut assets = Vec::new();

    if kind == "ExchangePlan" {
        let content_id = format!("{uuid}.1");
        if file_names.contains(content_id.as_str()) {
            assets.push((
                content_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Content.xml"),
                    kind: SourceAssetKind::ExchangePlanContent,
                },
            ));
        }
    }

    if kind == "BusinessProcess" {
        let flowchart_id = format!("{uuid}.7");
        if file_names.contains(flowchart_id.as_str()) {
            assets.push((
                flowchart_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Flowchart.xml"),
                    kind: SourceAssetKind::BusinessProcessFlowchart,
                },
            ));
        }
    }

    if let Some(suffix) = additional_indexes_body_suffix(kind) {
        let additional_indexes_id = format!("{uuid}.{suffix}");
        if file_names.contains(additional_indexes_id.as_str()) {
            assets.push((
                additional_indexes_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("AdditionalIndexes.xml"),
                    kind: SourceAssetKind::InflatedBinary,
                },
            ));
        }
    }

    let body_id = format!("{uuid}.0");
    if file_names.contains(body_id.as_str()) {
        let asset = match kind {
            "CommonPicture" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Picture.xml"),
                kind: SourceAssetKind::ExtPicture,
            }),
            "ScheduledJob" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Schedule.xml"),
                kind: SourceAssetKind::Schedule,
            }),
            "XDTOPackage" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Package.bin"),
                kind: SourceAssetKind::InflatedBinary,
            }),
            "Style" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Style.xml"),
                kind: SourceAssetKind::StyleBody,
            }),
            "WSReference" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("WSDefinition.xml"),
                kind: SourceAssetKind::WsDefinition,
            }),
            "Role"
                if rows_by_file_name
                    .get(body_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .and_then(|bytes| parse_role_rights_blob(&bytes, object_refs, field_refs))
                    .is_some() =>
            {
                Some(SourceAsset {
                    primary_path: object_path.join("Ext").join("Rights.xml"),
                    kind: SourceAssetKind::RoleRights,
                })
            }
            _ => None,
        };
        if let Some(asset) = asset {
            assets.push((body_id, asset));
        }
    }

    let command_mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .collect::<BTreeSet<_>>();
    for suffix in ["0", "1"] {
        let body_id = format!("{uuid}.{suffix}");
        if command_mapped_ids.contains(&body_id) {
            continue;
        }
        if let Some(row) = rows_by_file_name.get(body_id.as_str())
            && let Ok(bytes) = decode_hex(&row.binary_hex)
            && parse_command_interface_blob(&bytes, command_refs, metadata_refs).is_some()
        {
            assets.push((
                body_id,
                SourceAsset {
                    primary_path: object_path.join("Ext").join("CommandInterface.xml"),
                    kind: SourceAssetKind::CommandInterface,
                },
            ));
        }
    }

    let mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .collect::<BTreeSet<_>>();
    let object_row_prefix = format!("{uuid}.");
    let preferred_help_body_id = preferred_help_body_id(kind, uuid);
    for (body_id, body_row) in rows_by_file_name {
        if !body_id.starts_with(&object_row_prefix) || mapped_ids.contains(*body_id) {
            continue;
        }
        if let Ok(help_bytes) = decode_hex(&body_row.binary_hex)
            && parse_help_blob_pages(&help_bytes).is_some()
        {
            if rows_by_file_name.contains_key(preferred_help_body_id.as_str())
                && *body_id != preferred_help_body_id
            {
                continue;
            }
            assets.push((
                (*body_id).to_string(),
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Help.xml"),
                    kind: SourceAssetKind::Help,
                },
            ));
            continue;
        }
        if let Some(model) = predefined_data_source_model(kind)
            && (*body_id).strip_prefix(&object_row_prefix) == Some(model.suffix)
            && let Ok(predefined_bytes) = decode_hex(&body_row.binary_hex)
            && parse_predefined_data_blob_with_model(&predefined_bytes, type_index, model).is_some()
        {
            assets.push((
                (*body_id).to_string(),
                SourceAsset {
                    primary_path: object_path.join("Ext").join("Predefined.xml"),
                    kind: SourceAssetKind::PredefinedData { model },
                },
            ));
        }
    }

    Some(assets)
}

pub(super) fn additional_indexes_body_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "Document" => Some("3"),
        "AccumulationRegister" => Some("4"),
        _ => None,
    }
}

pub(super) fn preferred_help_body_id(kind: &str, uuid: &str) -> String {
    let suffix = if matches!(kind, "Form" | "CommonForm") {
        "1"
    } else {
        "5"
    };
    format!("{uuid}.{suffix}")
}

pub(super) fn write_source_xml_file(
    path: &Path,
    xml: impl AsRef<[u8]>,
    source_version: InfobaseConfigSourceVersion,
) -> Result<()> {
    let normalized = normalize_source_xml_version_bytes(xml.as_ref(), source_version);
    fs::write(path, normalized).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn is_xml_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("xml")
}

pub(super) fn write_source_asset(
    context: &DumpRowContext<'_>,
    asset: &SourceAsset,
    bytes: &[u8],
    parsed_form_body: Option<&ParsedFormBodyBlob>,
    timings: &mut MssqlDumpTimingReport,
) -> Result<PathBuf> {
    let output_dir = context.output_dir;
    match &asset.kind {
        SourceAssetKind::ExtPicture => {
            let picture = extract_ext_picture(bytes).with_context(|| {
                format!(
                    "failed to extract picture from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let xml_path = output_dir.join(&asset.primary_path);
            if let Some(parent) = xml_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let picture_dir = output_dir.join(asset.primary_path.with_extension(""));
            fs::create_dir_all(&picture_dir)
                .with_context(|| format!("failed to create {}", picture_dir.display()))?;
            let picture_file_name = ext_picture_file_name(&picture.content);
            let picture_path = picture_dir.join(picture_file_name);
            fs::write(&picture_path, &picture.content)
                .with_context(|| format!("failed to write {}", picture_path.display()))?;
            write_source_xml_file(
                &xml_path,
                format_ext_picture_xml(
                    picture_file_name,
                    picture.transparent_pixel,
                    context.source_version,
                ),
                context.source_version,
            )?;
        }
        SourceAssetKind::Schedule => {
            let xml = extract_schedule_xml(bytes).with_context(|| {
                format!(
                    "failed to extract schedule from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
        SourceAssetKind::StandaloneContent => {
            let xml = extract_standalone_content_xml(bytes, context.standalone_refs).with_context(
                || {
                    format!(
                        "failed to extract standalone content from source asset {}",
                        asset.primary_path.display()
                    )
                },
            )?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
        SourceAssetKind::StyleBody => {
            let xml = extract_style_body_xml(bytes, context.object_refs).with_context(|| {
                format!(
                    "failed to extract style body from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
        SourceAssetKind::Form => {
            let form_xml_started = Instant::now();
            let owned_body;
            let body = if let Some(body) = parsed_form_body {
                body
            } else {
                owned_body = parse_form_body_blob(bytes).with_context(|| {
                    format!(
                        "failed to parse form body from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
                &owned_body
            };
            let xml = extract_form_body_xml_from_body_timed(
                body,
                context.type_index,
                context.object_refs,
                Some(timings),
            )
            .with_context(|| {
                format!(
                    "failed to extract form xml from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
            timings.source_asset_form_xml_cpu_ms += elapsed_ms(form_xml_started);

            let form_items_started = Instant::now();
            for item_asset in extract_form_item_assets(bytes) {
                let item_path = output_dir
                    .join(asset.primary_path.with_extension(""))
                    .join("Items")
                    .join(sanitize_source_path_segment(&item_asset.item_name))
                    .join(&item_asset.file_name);
                if let Some(parent) = item_path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&item_path, &item_asset.content)
                    .with_context(|| format!("failed to write {}", item_path.display()))?;
            }
            timings.source_asset_form_items_cpu_ms += elapsed_ms(form_items_started);
        }
        SourceAssetKind::Help => {
            let help = parse_help_blob(bytes).with_context(|| {
                format!(
                    "failed to extract help from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let xml_path = output_dir.join(&asset.primary_path);
            if let Some(parent) = xml_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }

            let help_dir = output_dir.join(asset.primary_path.with_extension(""));
            fs::create_dir_all(&help_dir)
                .with_context(|| format!("failed to create {}", help_dir.display()))?;
            for page in &help.pages {
                let page_path = help_dir.join(&page.file_name);
                fs::write(
                    &page_path,
                    rewrite_help_links(&page.content, context.help_refs),
                )
                .with_context(|| format!("failed to write {}", page_path.display()))?;
            }
            if !help.files.is_empty() {
                let files_dir = help_dir.join("_files");
                fs::create_dir_all(&files_dir)
                    .with_context(|| format!("failed to create {}", files_dir.display()))?;
                for file in &help.files {
                    let file_path = files_dir.join(&file.file_name);
                    fs::write(&file_path, &file.content)
                        .with_context(|| format!("failed to write {}", file_path.display()))?;
                }
            }
            write_source_xml_file(
                &xml_path,
                format_help_xml(&help.pages),
                context.source_version,
            )?;
        }
        SourceAssetKind::DataCompositionSchema => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let content = normalize_data_composition_schema_template_xml(
                &inflated,
                context.dcs_type_index,
                context.object_refs,
            )
            .unwrap_or(inflated);
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, content, context.source_version)?;
        }
        SourceAssetKind::WsDefinition => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let content = extract_ws_definition_xml(&inflated).unwrap_or(inflated);
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, content, context.source_version)?;
        }
        SourceAssetKind::HomePageWorkArea => {
            let work_area =
                parse_home_page_work_area_blob(bytes, context.form_refs).with_context(|| {
                    format!(
                        "failed to extract home page work area from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_home_page_work_area_xml(&work_area, context.source_version),
                context.source_version,
            )?;
        }
        SourceAssetKind::ClientApplicationInterface => {
            let interface = parse_client_application_interface_blob(bytes).with_context(|| {
                format!(
                    "failed to extract client application interface from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_client_application_interface_xml(&interface),
                context.source_version,
            )?;
        }
        SourceAssetKind::AccumulationRegisterAggregates { register_name } => {
            let aggregates =
                parse_accumulation_register_aggregates_blob(bytes).with_context(|| {
                    format!(
                        "failed to parse accumulation register aggregates from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let xml = format_accumulation_register_aggregates_xml(
                &aggregates,
                register_name,
                context.field_refs,
            )
            .with_context(|| {
                format!(
                    "failed to format accumulation register aggregates for source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
        SourceAssetKind::InflatedBinary => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            if is_xml_path(&asset.primary_path) {
                write_source_xml_file(&path, inflated, context.source_version)?;
            } else {
                fs::write(&path, inflated)
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }
        }
        SourceAssetKind::InflatedBase64OrBinary => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let content = if let Ok(text) = std::str::from_utf8(&inflated) {
                extract_base64_payload(text)
                    .and_then(decode_base64_mime)
                    .unwrap_or(inflated)
            } else {
                inflated
            };
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            if is_xml_path(&asset.primary_path) {
                write_source_xml_file(&path, content, context.source_version)?;
            } else {
                fs::write(&path, content)
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }
        }
        SourceAssetKind::PredefinedData { model } => {
            let items = parse_predefined_data_blob_with_model(bytes, context.type_index, *model)
                .with_context(|| {
                    format!(
                        "failed to extract predefined data from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let xml = format_predefined_data_xml(
                *model,
                &items,
                context.object_refs,
                context.predefined_item_refs,
            )
            .with_context(|| {
                format!(
                    "failed to serialize predefined data from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
        SourceAssetKind::RoleRights => {
            let rights =
                parse_role_rights_blob(bytes, context.role_rights_object_refs, context.field_refs)
                    .with_context(|| {
                        format!(
                            "failed to extract role rights from source asset {}",
                            asset.primary_path.display()
                        )
                    })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_role_rights_xml(&rights),
                context.source_version,
            )?;
        }
        SourceAssetKind::CommandInterface => {
            let entries = parse_command_interface_blob_with_subsystem_refs(
                bytes,
                context.command_refs,
                context.metadata_refs,
                context.subsystem_refs,
            )
            .with_context(|| {
                format!(
                    "failed to extract command interface from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_command_interface_xml(&entries),
                context.source_version,
            )?;
        }
        SourceAssetKind::ExchangePlanContent => {
            let items = parse_exchange_plan_content_blob(
                bytes,
                context.object_refs,
                context.type_index,
                context.metadata_order,
            )
            .with_context(|| {
                format!(
                    "failed to extract exchange plan content from source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_exchange_plan_content_xml(&items),
                context.source_version,
            )?;
        }
        SourceAssetKind::BusinessProcessFlowchart => {
            let flowchart = parse_business_process_flowchart_blob(bytes, context.object_refs)
                .with_context(|| {
                    format!(
                        "failed to extract business process flowchart from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(
                &path,
                format_business_process_flowchart_xml(&flowchart),
                context.source_version,
            )?;
        }
        SourceAssetKind::MoxelSpreadsheet => {
            let xml =
                extract_moxel_spreadsheet_xml(bytes, context.object_refs).with_context(|| {
                    format!(
                        "failed to extract spreadsheet template from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
    }

    Ok(asset.primary_path.clone())
}

pub(super) struct HelpPage {
    pub(super) page: String,
    pub(super) file_name: String,
    pub(super) content: Vec<u8>,
}

pub(super) struct HelpFile {
    pub(super) file_name: String,
    pub(super) content: Vec<u8>,
}

pub(super) struct HelpContent {
    pub(super) pages: Vec<HelpPage>,
    pub(super) files: Vec<HelpFile>,
}

pub(super) struct FormItemAsset {
    pub(super) item_name: String,
    pub(super) file_name: String,
    pub(super) content: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct PredefinedItem {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) code: String,
    pub(super) description: String,
    pub(super) data: PredefinedItemData,
    pub(super) children: Vec<PredefinedItem>,
}

#[derive(Clone)]
pub(super) enum PredefinedItemData {
    Generic {
        value_types: Vec<ConstantValueType>,
        is_folder: bool,
    },
    Account {
        account_type: PredefinedAccountType,
        off_balance: bool,
        order: String,
        accounting_flags: Vec<PredefinedFlag>,
        ext_dimension_types: Vec<PredefinedExtDimensionType>,
    },
    Calculation {
        action_period_is_base: bool,
        displaced: Vec<String>,
        base: Vec<String>,
        leading: Vec<String>,
    },
}

#[derive(Clone, Copy)]
pub(super) enum PredefinedAccountType {
    Active,
    Passive,
    ActivePassive,
}

#[derive(Clone)]
pub(super) struct PredefinedFlag {
    pub(super) reference_uuid: String,
    pub(super) value: bool,
}

#[derive(Clone)]
pub(super) struct PredefinedExtDimensionType {
    pub(super) item_uuid: String,
    pub(super) turnover: bool,
    pub(super) accounting_flags: Vec<PredefinedFlag>,
}

pub(super) struct AccumulationRegisterAggregate {
    pub(super) id: String,
    pub(super) use_code: i64,
    pub(super) periodicity_code: i64,
    pub(super) dimensions: Vec<(String, bool)>,
}

enum AggregateColumnKind {
    Id,
    Number,
    Dimension(String),
}

fn unquote_1c_token(token: &str) -> String {
    let token = token.trim();
    if token.len() >= 2 && token.starts_with('"') && token.ends_with('"') {
        token[1..token.len() - 1].replace("\"\"", "\"")
    } else {
        token.to_string()
    }
}

fn aggregate_dimension_uuid_from_column_name(name: &str) -> Option<String> {
    let inner = unquote_1c_token(name);
    let fields = split_1c_braced_fields(inner.trim(), 0)?;
    let dimension_ref = split_1c_braced_fields(fields.last()?.trim(), 0)?;
    let uuid = dimension_ref.get(1)?.trim();
    if is_uuid_text(uuid) {
        Some(uuid.to_string())
    } else {
        None
    }
}

fn parse_aggregate_column_kind(field: &str) -> Option<AggregateColumnKind> {
    let parts = split_1c_braced_fields(field.trim(), 0)?;
    let name = parts.get(1)?;
    let type_block = split_1c_braced_fields(parts.get(2)?.trim(), 0)?;
    let type_spec = split_1c_braced_fields(type_block.get(1)?.trim(), 0)?;
    match unquote_1c_token(type_spec.first()?).as_str() {
        "#" => Some(AggregateColumnKind::Id),
        "N" => Some(AggregateColumnKind::Number),
        "B" => Some(AggregateColumnKind::Dimension(
            aggregate_dimension_uuid_from_column_name(name)?,
        )),
        _ => None,
    }
}

fn aggregate_ref_cell_uuid(cell: &str) -> Option<String> {
    let fields = split_1c_braced_fields(cell.trim(), 0)?;
    let inner = split_1c_braced_fields(fields.last()?.trim(), 0)?;
    let uuid = inner.get(1)?.trim();
    if is_uuid_text(uuid) {
        Some(uuid.to_string())
    } else {
        None
    }
}

fn aggregate_number_cell(cell: &str) -> Option<i64> {
    let fields = split_1c_braced_fields(cell.trim(), 0)?;
    fields.get(1)?.trim().parse::<i64>().ok()
}

fn aggregate_bool_cell(cell: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(cell.trim(), 0)?;
    Some(fields.get(1)?.trim() == "1")
}

pub(super) fn parse_accumulation_register_aggregates_blob(
    bytes: &[u8],
) -> Option<Vec<AccumulationRegisterAggregate>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let top = split_1c_braced_fields(text.trim_start_matches('\u{feff}').trim(), 0)?;
    if top.first()?.trim() != "0" {
        return None;
    }
    let inner = split_1c_braced_fields(top.get(1)?.trim(), 0)?;
    if inner.first()?.trim() != "9" {
        return None;
    }

    let column_descriptors = split_1c_braced_fields(inner.get(1)?.trim(), 0)?;
    let column_count = column_descriptors.first()?.trim().parse::<usize>().ok()?;
    let mut columns = Vec::with_capacity(column_count);
    for descriptor in column_descriptors.iter().skip(1).take(column_count) {
        columns.push(parse_aggregate_column_kind(descriptor)?);
    }

    let data = split_1c_braced_fields(inner.get(2)?.trim(), 0)?;
    let data_column_count = data.get(1)?.trim().parse::<usize>().ok()?;
    let row_set = split_1c_braced_fields(data.get(2 + data_column_count * 2)?.trim(), 0)?;
    let row_count = row_set.get(1)?.trim().parse::<usize>().ok()?;

    let mut aggregates = Vec::with_capacity(row_count);
    for row_field in row_set.iter().skip(2).take(row_count) {
        let row = split_1c_braced_fields(row_field.trim(), 0)?;
        let row_column_count = row.get(2)?.trim().parse::<usize>().ok()?;
        let cells = row.get(3..3 + row_column_count)?;
        if cells.len() != columns.len() {
            return None;
        }

        let mut id = None;
        let mut numbers = Vec::new();
        let mut dimensions = Vec::new();
        for (column, cell) in columns.iter().zip(cells) {
            match column {
                AggregateColumnKind::Id => id = Some(aggregate_ref_cell_uuid(cell)?),
                AggregateColumnKind::Number => numbers.push(aggregate_number_cell(cell)?),
                AggregateColumnKind::Dimension(uuid) => {
                    dimensions.push((uuid.clone(), aggregate_bool_cell(cell)?));
                }
            }
        }

        if numbers.len() != 2 {
            return None;
        }
        aggregates.push(AccumulationRegisterAggregate {
            id: id?,
            use_code: numbers[0],
            periodicity_code: numbers[1],
            dimensions,
        });
    }

    Some(aggregates)
}

fn aggregate_use_token(code: i64) -> Option<&'static str> {
    match code {
        0 => Some("Auto"),
        1 => Some("Always"),
        _ => None,
    }
}

fn aggregate_periodicity_token(code: i64) -> Option<&'static str> {
    match code {
        0 => Some("Nonperiodical"),
        1 => Some("Auto"),
        2 => Some("Day"),
        3 => Some("Month"),
        4 => Some("Quarter"),
        5 => Some("HalfYear"),
        6 => Some("Year"),
        _ => None,
    }
}

pub(super) fn format_accumulation_register_aggregates_xml(
    aggregates: &[AccumulationRegisterAggregate],
    register_name: &str,
    field_refs: &BTreeMap<String, String>,
) -> Result<String> {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<AccumulationRegisterAggregates xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
    );
    for aggregate in aggregates {
        let use_token = aggregate_use_token(aggregate.use_code).with_context(|| {
            format!(
                "unsupported accumulation register aggregate Use code {}",
                aggregate.use_code
            )
        })?;
        let periodicity_token = aggregate_periodicity_token(aggregate.periodicity_code)
            .with_context(|| {
                format!(
                    "unsupported accumulation register aggregate Periodicity code {}",
                    aggregate.periodicity_code
                )
            })?;
        xml.push_str(&format!(
            "\t<Aggregate id=\"{}\">\r\n\
\t\t<Use>{}</Use>\r\n\
\t\t<Periodicity>{}</Periodicity>\r\n\
\t\t<Dimensions>\r\n",
            aggregate.id, use_token, periodicity_token
        ));
        for (dimension_uuid, included) in &aggregate.dimensions {
            let dimension_name = field_refs.get(dimension_uuid).with_context(|| {
                format!("unknown accumulation register aggregate dimension {dimension_uuid}")
            })?;
            xml.push_str(&format!(
                "\t\t\t<Dimension ref=\"AccumulationRegister.{}.Dimension.{}\">{}</Dimension>\r\n",
                escape_xml_text(register_name),
                escape_xml_text(dimension_name),
                included
            ));
        }
        xml.push_str(
            "\t\t</Dimensions>\r\n\
\t</Aggregate>\r\n",
        );
    }
    xml.push_str("</AccumulationRegisterAggregates>");
    Ok(xml)
}

pub(super) fn parse_help_blob_pages(bytes: &[u8]) -> Option<Vec<HelpPage>> {
    parse_help_blob(bytes).map(|help| help.pages)
}

pub(super) fn parse_help_blob(bytes: &[u8]) -> Option<HelpContent> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    if fields.first()?.trim() != "5" {
        return None;
    }
    let page_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut index = 2usize;
    let mut pages = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        let (language, _) = parse_1c_quoted_string_with_len(fields.get(index)?.trim())?;
        index += 1;
        let payload = extract_base64_payload(fields.get(index)?.trim())?;
        index += 1;
        let content = decode_base64_mime(payload)?;
        let page = sanitize_source_path_segment(&language);
        pages.push(HelpPage {
            file_name: format!("{page}.html"),
            page,
            content,
        });
    }

    if pages.is_empty() {
        return None;
    }

    let mut files = Vec::new();
    if let Some(count) = fields
        .get(index)
        .and_then(|field| field.trim().parse::<usize>().ok())
    {
        index += 1;
        for _ in 0..count {
            let (file_name, _) = parse_1c_quoted_string_with_len(fields.get(index)?.trim())?;
            index += 1;
            if fields
                .get(index)
                .is_some_and(|field| field.trim().chars().all(|ch| ch.is_ascii_digit()))
            {
                index += 1;
            }
            let payload = extract_base64_payload(fields.get(index)?.trim())?;
            index += 1;
            files.push(HelpFile {
                file_name: sanitize_source_path_segment(&file_name),
                content: decode_base64_mime(payload)?,
            });
        }
    }

    Some(HelpContent { pages, files })
}

pub(super) fn rewrite_help_links(content: &[u8], refs: &BTreeMap<String, String>) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(content) else {
        return content.to_vec();
    };
    let text = rewrite_help_picture_refs(text, refs);
    let pattern = "../id";
    let mut output = String::with_capacity(text.len());
    let mut offset = 0usize;

    while let Some(relative_start) = text[offset..].find(pattern) {
        let start = offset + relative_start;
        let uuid_start = start + pattern.len();
        let uuid_end = uuid_start + 36;
        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            break;
        };
        if parse_non_zero_uuid(uuid).is_none()
            || text.as_bytes().get(uuid_end).copied() != Some(b'/')
        {
            output.push_str(&text[offset..uuid_start]);
            offset = uuid_start;
            continue;
        }
        let Some(reference) = refs.get(uuid) else {
            output.push_str(&text[offset..uuid_end]);
            offset = uuid_end;
            continue;
        };
        let Some(relative_quote_end) = text[uuid_end..].find('"') else {
            break;
        };
        let quote_end = uuid_end + relative_quote_end;
        output.push_str(&text[offset..start]);
        output.push_str(reference);
        output.push_str("/Help");
        offset = quote_end;
    }
    output.push_str(&text[offset..]);
    output.replace("\r\n", "\n").into_bytes()
}

pub(super) fn rewrite_help_picture_refs(text: &str, refs: &BTreeMap<String, String>) -> String {
    let pattern = "../../mdpicture/";
    let mut output = String::with_capacity(text.len());
    let mut offset = 0usize;

    while let Some(relative_start) = text[offset..].find(pattern) {
        let start = offset + relative_start;
        let Some(relative_quote_end) = text[start..].find('"') else {
            break;
        };
        let value_end = start + relative_quote_end;
        let token = &text[start + pattern.len()..value_end];
        output.push_str(&text[offset..start]);
        match resolve_help_picture_reference(token, refs) {
            Some(reference) => output.push_str(&reference),
            None => output.push_str(&text[start..value_end]),
        }
        offset = value_end;
    }
    output.push_str(&text[offset..]);
    output
}

fn resolve_help_picture_reference(token: &str, refs: &BTreeMap<String, String>) -> Option<String> {
    if let Some(index) = token.strip_prefix("idn-") {
        return help_standard_picture_by_negative_index(index).map(str::to_string);
    }
    let uuid = token.strip_prefix("id")?.split('/').next()?;
    if parse_non_zero_uuid(uuid).is_none() {
        return None;
    }
    if let Some(reference) = refs.get(uuid)
        && reference.starts_with("CommonPicture.")
    {
        return Some(reference.clone());
    }
    common_command_standard_picture_name(uuid).map(str::to_string)
}

fn help_standard_picture_by_negative_index(index: &str) -> Option<&'static str> {
    match index {
        "1" => Some("StdPicture.InputFieldSelect"),
        "2" => Some("StdPicture.InputFieldClear"),
        "3" => Some("StdPicture.MoveUp"),
        "4" => Some("StdPicture.MoveDown"),
        "5" => Some("StdPicture.InputFieldCalendar"),
        "7" => Some("StdPicture.InputFieldOpen"),
        "8" => Some("StdPicture.MoveLeft"),
        "9" => Some("StdPicture.MoveRight"),
        "10" => Some("StdPicture.CheckAll"),
        "11" => Some("StdPicture.UncheckAll"),
        "13" => Some("StdPicture.Print"),
        _ => None,
    }
}

const PREDEFINED_DATA_SOURCE_MODELS: &[(&str, PredefinedDataSourceModel)] = &[
    (
        "Catalog",
        PredefinedDataSourceModel {
            suffix: "1c",
            xsi_type: "CatalogPredefinedItems",
            root_tag: "0",
            rowset_layout: PredefinedDataRowsetLayout::NestedTable,
            unwrap_single_root: true,
            item_layout: PredefinedItemLayout::Generic,
        },
    ),
    (
        "ChartOfCharacteristicTypes",
        PredefinedDataSourceModel {
            suffix: "7",
            xsi_type: "PlanOfCharacteristicKindPredefinedItems",
            root_tag: "1",
            rowset_layout: PredefinedDataRowsetLayout::NestedTable,
            unwrap_single_root: true,
            item_layout: PredefinedItemLayout::Generic,
        },
    ),
    (
        "ChartOfAccounts",
        PredefinedDataSourceModel {
            suffix: "9",
            xsi_type: "ChartOfAccountsPredefinedItems",
            root_tag: "2",
            rowset_layout: PredefinedDataRowsetLayout::NestedTable,
            unwrap_single_root: true,
            item_layout: PredefinedItemLayout::Account,
        },
    ),
    (
        "ChartOfCalculationTypes",
        PredefinedDataSourceModel {
            suffix: "2",
            xsi_type: "CalculationTypePredefinedItems",
            root_tag: "9",
            rowset_layout: PredefinedDataRowsetLayout::Root,
            unwrap_single_root: false,
            item_layout: PredefinedItemLayout::Calculation,
        },
    ),
];

pub(super) fn predefined_data_source_model(kind: &str) -> Option<PredefinedDataSourceModel> {
    PREDEFINED_DATA_SOURCE_MODELS
        .iter()
        .find_map(|(candidate, model)| (*candidate == kind).then_some(*model))
}

pub(super) fn predefined_data_needs_item_references(
    file_names: &BTreeSet<String>,
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
) -> bool {
    body_owners.iter().any(|(owner_uuid, owner)| {
        let Some(model) = predefined_data_source_model(&owner.kind) else {
            return false;
        };
        matches!(
            model.item_layout,
            PredefinedItemLayout::Account | PredefinedItemLayout::Calculation
        ) && file_names.contains(&format!("{owner_uuid}.{}", model.suffix))
    })
}

pub(super) fn predefined_data_body_file_names(
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
) -> BTreeSet<String> {
    body_owners
        .iter()
        .filter_map(|(owner_uuid, owner)| {
            predefined_data_source_model(&owner.kind)
                .map(|model| format!("{owner_uuid}.{}", model.suffix))
        })
        .collect()
}

pub(super) fn build_predefined_item_reference_index(
    rows: &[ConfigRow],
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let rows_by_file_name = rows
        .iter()
        .filter(|row| !row.binary_hex.is_empty())
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut index = BTreeMap::new();

    for (owner_uuid, owner) in body_owners {
        let Some(model) = predefined_data_source_model(&owner.kind) else {
            continue;
        };
        let file_name = format!("{owner_uuid}.{}", model.suffix);
        let Some(row) = rows_by_file_name.get(file_name.as_str()) else {
            continue;
        };
        let bytes = decode_hex(&row.binary_hex)
            .with_context(|| format!("failed to decode predefined data row {file_name}"))?;
        let Some(items) = parse_predefined_data_blob_with_model(&bytes, type_index, model) else {
            continue;
        };
        let owner_reference = object_refs.get(owner_uuid).with_context(|| {
            format!("missing metadata reference for predefined data owner {owner_uuid}")
        })?;
        insert_predefined_item_references(&mut index, owner_reference, &items)?;
    }

    Ok(index)
}

fn insert_predefined_item_references(
    index: &mut BTreeMap<String, String>,
    owner_reference: &str,
    items: &[PredefinedItem],
) -> Result<()> {
    for item in items {
        let reference = format!("{owner_reference}.{}", item.name);
        if let Some(previous) = index.insert(item.id.clone(), reference.clone())
            && previous != reference
        {
            bail!(
                "predefined item {} resolves to both {previous} and {reference}",
                item.id
            );
        }
        insert_predefined_item_references(index, owner_reference, &item.children)?;
    }
    Ok(())
}

#[allow(dead_code)]
pub(super) fn parse_predefined_data_blob(
    bytes: &[u8],
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    parse_predefined_data_blob_inner(bytes, type_index, None)
}

fn parse_predefined_data_blob_with_model(
    bytes: &[u8],
    type_index: &BTreeMap<String, String>,
    model: PredefinedDataSourceModel,
) -> Option<Vec<PredefinedItem>> {
    parse_predefined_data_blob_inner(bytes, type_index, Some(model))
}

fn parse_predefined_data_blob_inner(
    bytes: &[u8],
    type_index: &BTreeMap<String, String>,
    expected_model: Option<PredefinedDataSourceModel>,
) -> Option<Vec<PredefinedItem>> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(text, 0)?;
    let root_tag = fields.first()?.trim();
    let model = expected_model.or_else(|| {
        PREDEFINED_DATA_SOURCE_MODELS
            .iter()
            .find_map(|(_, model)| (model.root_tag == root_tag).then_some(*model))
    })?;
    if root_tag != model.root_tag {
        return None;
    }

    let (schema_value, rowset_value) = match model.rowset_layout {
        PredefinedDataRowsetLayout::NestedTable => {
            let table_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
            (*table_fields.get(1)?, *table_fields.get(2)?)
        }
        PredefinedDataRowsetLayout::Root => (*fields.get(1)?, *fields.get(2)?),
    };

    match model.item_layout {
        PredefinedItemLayout::Generic => {
            let root_items = parse_predefined_rowset_roots(rowset_value, type_index)?;
            if model.unwrap_single_root {
                let [root_item] = root_items.as_slice() else {
                    return None;
                };
                Some(root_item.children.clone())
            } else {
                Some(root_items)
            }
        }
        PredefinedItemLayout::Account => {
            parse_account_predefined_rowset(schema_value, rowset_value)
        }
        PredefinedItemLayout::Calculation => {
            parse_calculation_predefined_rowset(schema_value, rowset_value)
        }
    }
}

struct PredefinedRowsetColumn {
    id: i64,
    reference_uuid: Option<String>,
    is_boolean: bool,
}

struct PredefinedRowsetSchema {
    columns: Vec<PredefinedRowsetColumn>,
    value_offsets: BTreeMap<i64, usize>,
}

fn parse_predefined_rowset_schema<'a>(
    schema_value: &str,
    rowset_value: &'a str,
) -> Option<(PredefinedRowsetSchema, &'a str)> {
    let schema_fields = split_1c_braced_fields(schema_value, 0)?;
    let column_count = schema_fields.first()?.trim().parse::<usize>().ok()?;
    if schema_fields.len() != column_count.checked_add(1)? {
        return None;
    }

    let mut columns = Vec::with_capacity(column_count);
    for descriptor in schema_fields.iter().skip(1) {
        let fields = split_1c_braced_fields(descriptor, 0)?;
        let id = fields.first()?.trim().parse::<i64>().ok()?;
        let raw_reference = unquote_1c_token(fields.get(1)?.trim());
        let reference_uuid = if raw_reference.is_empty() {
            None
        } else {
            Some(parse_uuid_field(&raw_reference)?)
        };
        columns.push(PredefinedRowsetColumn {
            id,
            reference_uuid,
            is_boolean: fields
                .get(2)
                .is_some_and(|value| predefined_column_is_boolean(value)),
        });
    }

    let rowset_fields = split_1c_braced_fields(rowset_value, 0)?;
    if rowset_fields.first()?.trim() != "2"
        || rowset_fields.get(1)?.trim().parse::<usize>().ok()? != column_count
    {
        return None;
    }
    let mappings_end = 2usize.checked_add(column_count.checked_mul(2)?)?;
    let mut value_offsets = BTreeMap::new();
    for mapping in rowset_fields.get(2..mappings_end)?.chunks_exact(2) {
        let value_offset = mapping[0].trim().parse::<usize>().ok()?;
        let column_id = mapping[1].trim().parse::<i64>().ok()?;
        if value_offsets.insert(column_id, value_offset).is_some() {
            return None;
        }
    }
    if value_offsets.len() != column_count {
        return None;
    }
    let item_list = *rowset_fields.get(mappings_end)?;
    let item_list_fields = split_1c_braced_fields(item_list, 0)?;
    if item_list_fields.first()?.trim() != "1" {
        return None;
    }

    Some((
        PredefinedRowsetSchema {
            columns,
            value_offsets,
        },
        item_list,
    ))
}

#[cfg(test)]
mod predefined_rowset_schema_tests {
    use super::parse_predefined_rowset_schema;

    #[test]
    fn column_count_overflow_fails_closed() {
        let schema = format!("{{{}}}", usize::MAX);

        assert!(parse_predefined_rowset_schema(&schema, "{2,0,{0}}").is_none());
    }
}

fn predefined_column_is_boolean(value: &str) -> bool {
    let Some(pattern) = split_1c_braced_fields(value, 0) else {
        return false;
    };
    if pattern.first().map(|value| unquote_1c_token(value)) != Some("Pattern".to_string()) {
        return false;
    }
    pattern
        .get(1)
        .and_then(|value| split_1c_braced_fields(value, 0))
        .and_then(|fields| fields.first().map(|value| unquote_1c_token(value)))
        .as_deref()
        == Some("B")
}

fn parse_predefined_item_fields<'a>(value: &'a str) -> Option<(Vec<&'a str>, Option<&'a str>)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let value_count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let after_values = 3usize.checked_add(value_count)?;
    let child_list = match fields.get(after_values)?.trim() {
        "0" => None,
        "1" => Some(*fields.get(after_values + 1)?),
        _ => return None,
    };
    Some((fields, child_list))
}

fn predefined_rowset_item_value<'a>(
    fields: &[&'a str],
    schema: &PredefinedRowsetSchema,
    column_id: i64,
) -> Option<&'a str> {
    let value_offset = *schema.value_offsets.get(&column_id)?;
    fields.get(3usize.checked_add(value_offset)?).copied()
}

fn parse_predefined_item_list(
    value: &str,
    mut parse_item: impl FnMut(&str) -> Option<PredefinedItem>,
) -> Option<Vec<PredefinedItem>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let items = fields
        .iter()
        .skip(2)
        .take(count)
        .map(|field| parse_item(field))
        .collect::<Option<Vec<_>>>()?;
    (items.len() == count).then_some(items)
}

fn parse_account_predefined_rowset(
    schema_value: &str,
    rowset_value: &str,
) -> Option<Vec<PredefinedItem>> {
    let (schema, root_list) = parse_predefined_rowset_schema(schema_value, rowset_value)?;
    let root_fields = split_1c_braced_fields(root_list, 0)?;
    if root_fields.first()?.trim() != "1" || root_fields.get(1)?.trim() != "1" {
        return None;
    }
    let (_, child_list) = parse_predefined_item_fields(root_fields.get(2)?)?;
    parse_account_predefined_children(child_list?, &schema)
}

fn parse_account_predefined_children(
    value: &str,
    schema: &PredefinedRowsetSchema,
) -> Option<Vec<PredefinedItem>> {
    parse_predefined_item_list(value, |item| parse_account_predefined_item(item, schema))
}

fn parse_account_predefined_item(
    value: &str,
    schema: &PredefinedRowsetSchema,
) -> Option<PredefinedItem> {
    const FIXED_COLUMNS: &[i64] = &[0, 1, 2, 3, 4, 5, 6, 10_000, 20_000];

    let (fields, child_list) = parse_predefined_item_fields(value)?;
    let id = parse_predefined_uuid_value(predefined_rowset_item_value(&fields, schema, 0)?)?;
    let name = parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 1)?)?;
    let code = parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 2)?)?;
    let description =
        parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 3)?)?;
    let account_type =
        match parse_predefined_number_value(predefined_rowset_item_value(&fields, schema, 4)?)? {
            0 => PredefinedAccountType::Active,
            1 => PredefinedAccountType::Passive,
            2 => PredefinedAccountType::ActivePassive,
            _ => return None,
        };
    let off_balance =
        parse_predefined_bool_value(predefined_rowset_item_value(&fields, schema, 5)?)?;
    let ext_dimension_types =
        parse_predefined_ext_dimension_types(predefined_rowset_item_value(&fields, schema, 6)?)?;
    let order =
        parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 10_000)?)?;
    if parse_predefined_number_value(predefined_rowset_item_value(&fields, schema, 20_000)?)? != 0 {
        return None;
    }

    let accounting_flags = parse_predefined_dynamic_flags(&fields, schema, FIXED_COLUMNS)?;
    let children = match child_list {
        Some(value) => parse_account_predefined_children(value, schema)?,
        None => Vec::new(),
    };

    Some(PredefinedItem {
        id,
        name,
        code,
        description,
        data: PredefinedItemData::Account {
            account_type,
            off_balance,
            order,
            accounting_flags,
            ext_dimension_types,
        },
        children,
    })
}

fn parse_predefined_dynamic_flags(
    fields: &[&str],
    schema: &PredefinedRowsetSchema,
    fixed_columns: &[i64],
) -> Option<Vec<PredefinedFlag>> {
    schema
        .columns
        .iter()
        .filter(|column| !fixed_columns.contains(&column.id))
        .map(|column| {
            if !column.is_boolean {
                return None;
            }
            Some(PredefinedFlag {
                reference_uuid: column.reference_uuid.clone()?,
                value: parse_predefined_bool_value(predefined_rowset_item_value(
                    fields, schema, column.id,
                )?)?,
            })
        })
        .collect()
}

fn parse_predefined_ext_dimension_types(value: &str) -> Option<Vec<PredefinedExtDimensionType>> {
    let outer = split_1c_braced_fields(value, 0)?;
    if outer.first()?.trim() != r##""#""## {
        return None;
    }
    let payload = split_1c_braced_fields(outer.get(2)?, 0)?;
    if payload.first()?.trim() != "9" {
        return None;
    }
    let (schema, item_list) = parse_predefined_rowset_schema(payload.get(1)?, payload.get(2)?)?;
    let list_fields = split_1c_braced_fields(item_list, 0)?;
    let count = list_fields.get(1)?.trim().parse::<usize>().ok()?;
    let items = list_fields
        .iter()
        .skip(2)
        .take(count)
        .map(|item| {
            let (fields, child_list) = parse_predefined_item_fields(item)?;
            if child_list.is_some() {
                return None;
            }
            Some(PredefinedExtDimensionType {
                item_uuid: parse_predefined_uuid_value(predefined_rowset_item_value(
                    &fields, &schema, 0,
                )?)?,
                turnover: parse_predefined_bool_value(predefined_rowset_item_value(
                    &fields, &schema, 1,
                )?)?,
                accounting_flags: parse_predefined_dynamic_flags(&fields, &schema, &[0, 1])?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    (items.len() == count).then_some(items)
}

fn parse_calculation_predefined_rowset(
    schema_value: &str,
    rowset_value: &str,
) -> Option<Vec<PredefinedItem>> {
    let (schema, item_list) = parse_predefined_rowset_schema(schema_value, rowset_value)?;
    parse_predefined_item_list(item_list, |item| {
        parse_calculation_predefined_item(item, &schema)
    })
}

fn parse_calculation_predefined_item(
    value: &str,
    schema: &PredefinedRowsetSchema,
) -> Option<PredefinedItem> {
    let (fields, child_list) = parse_predefined_item_fields(value)?;
    if child_list.is_some()
        || schema
            .columns
            .iter()
            .any(|column| column.reference_uuid.is_some())
    {
        return None;
    }
    let id = parse_predefined_uuid_value(predefined_rowset_item_value(&fields, schema, 1)?)?;
    let name = parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 2)?)?;
    let code = parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 3)?)?;
    let description =
        parse_predefined_string_value(predefined_rowset_item_value(&fields, schema, 4)?)?;
    let action_period_is_base =
        parse_predefined_bool_value(predefined_rowset_item_value(&fields, schema, 5)?)?;
    let displaced =
        parse_predefined_item_reference_list(predefined_rowset_item_value(&fields, schema, 6)?)?;
    let base =
        parse_predefined_item_reference_list(predefined_rowset_item_value(&fields, schema, 7)?)?;
    let leading =
        parse_predefined_item_reference_list(predefined_rowset_item_value(&fields, schema, 8)?)?;
    if parse_predefined_number_value(predefined_rowset_item_value(&fields, schema, 9)?)? != 0 {
        return None;
    }

    Some(PredefinedItem {
        id,
        name,
        code,
        description,
        data: PredefinedItemData::Calculation {
            action_period_is_base,
            displaced,
            base,
            leading,
        },
        children: Vec::new(),
    })
}

fn parse_predefined_item_reference_list(value: &str) -> Option<Vec<String>> {
    let outer = split_1c_braced_fields(value, 0)?;
    if outer.first()?.trim() != r##""#""## {
        return None;
    }
    let payload = split_1c_braced_fields(outer.get(2)?, 0)?;
    if payload.first()?.trim() != "9" {
        return None;
    }
    let (schema, item_list) = parse_predefined_rowset_schema(payload.get(1)?, payload.get(2)?)?;
    if schema
        .columns
        .iter()
        .any(|column| column.id != 1 || column.reference_uuid.is_some() || column.is_boolean)
    {
        return None;
    }
    let list_fields = split_1c_braced_fields(item_list, 0)?;
    let count = list_fields.get(1)?.trim().parse::<usize>().ok()?;
    let items = list_fields
        .iter()
        .skip(2)
        .take(count)
        .map(|item| {
            let (fields, child_list) = parse_predefined_item_fields(item)?;
            if child_list.is_some() {
                return None;
            }
            parse_predefined_uuid_value(predefined_rowset_item_value(&fields, &schema, 1)?)
        })
        .collect::<Option<Vec<_>>>()?;
    (items.len() == count).then_some(items)
}

pub(super) fn parse_predefined_rowset_roots(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    fields
        .iter()
        .find_map(|field| parse_predefined_children(field, type_index))
}

pub(super) fn parse_predefined_item(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<PredefinedItem> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let value_count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let value_start = 3usize;
    let after_values = value_start.checked_add(value_count)?;
    if fields.len() < after_values {
        return None;
    }

    let id = parse_predefined_uuid_value(fields.get(value_start)?)?;
    let is_folder = fields
        .get(value_start + 1)
        .and_then(|field| parse_predefined_bool_value(field))
        .unwrap_or(false);
    let has_parent_ref = fields
        .get(value_start + 2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .and_then(|field| field.first().map(|value| value.trim() == r##""#""##))
        .unwrap_or(false);
    let name_offset = if has_parent_ref {
        value_start + 3
    } else {
        value_start + 2
    };
    let name = fields
        .get(name_offset)
        .and_then(|field| parse_predefined_string_value(field))?;
    let code = fields
        .get(name_offset + 1)
        .and_then(|field| parse_predefined_string_value(field))
        .unwrap_or_default();
    let description = fields
        .get(name_offset + 2)
        .and_then(|field| parse_predefined_string_value(field))
        .unwrap_or_default();
    let value_types = fields
        .get(name_offset + 3)
        .and_then(|field| parse_predefined_type_value(field, type_index))
        .unwrap_or_default();
    let children = if fields
        .get(after_values)
        .is_some_and(|field| field.trim() == "1")
    {
        fields
            .get(after_values + 1)
            .and_then(|field| parse_predefined_children(field, type_index))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    Some(PredefinedItem {
        id,
        name,
        code,
        description,
        data: PredefinedItemData::Generic {
            value_types,
            is_folder,
        },
        children,
    })
}

pub(super) fn parse_predefined_children(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<PredefinedItem>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let children = fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_predefined_item(field, type_index))
        .collect::<Vec<_>>();
    if children.len() == count {
        Some(children)
    } else {
        None
    }
}

pub(super) fn parse_predefined_type_value(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    parse_metadata_type_pattern(fields.get(2)?, type_index)
}

pub(super) fn parse_predefined_uuid_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    let ref_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
    let uuid = ref_fields.get(1)?.trim();
    parse_uuid_field(uuid)
}

pub(super) fn parse_predefined_bool_value(value: &str) -> Option<bool> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""B""# {
        return None;
    }
    parse_1c_bool_flag(fields.get(1)?.trim())
}

pub(super) fn parse_predefined_number_value(value: &str) -> Option<i64> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""N""# {
        return None;
    }
    fields.get(1)?.trim().parse().ok()
}

pub(super) fn parse_predefined_string_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""S""# {
        return None;
    }
    fields
        .get(1)
        .and_then(|field| parse_1c_quoted_string_with_len(field.trim()))
        .map(|(value, _)| value)
}

pub(super) struct ExtPictureAsset {
    pub(super) content: Vec<u8>,
    pub(super) transparent_pixel: Option<(i32, i32)>,
}

pub(super) fn extract_ext_picture(bytes: &[u8]) -> Result<ExtPictureAsset> {
    let inflated = inflate_raw_deflate(bytes)?;
    if let Ok(text) = std::str::from_utf8(&inflated) {
        let transparent_pixel = extract_ext_picture_transparent_pixel(text);
        if let Some(payload) = extract_base64_payload(text) {
            let content = decode_base64_mime(payload).context("failed to decode picture base64")?;
            return Ok(ExtPictureAsset {
                content,
                transparent_pixel,
            });
        }
    }
    Ok(ExtPictureAsset {
        content: inflated,
        transparent_pixel: None,
    })
}

pub(super) fn extract_ext_picture_transparent_pixel(text: &str) -> Option<(i32, i32)> {
    let mut offset = skip_ascii_ws_at(text.trim_start_matches('\u{feff}'), 0);
    let text = text.trim_start_matches('\u{feff}');
    if text.as_bytes().get(offset) != Some(&b'{') {
        return None;
    }
    offset += 1;
    let first_comma = text[offset..].find(',')? + offset;
    offset = skip_ascii_ws_at(text, first_comma + 1);
    let transparent_end = scan_1c_braced_value(text, offset)?;
    let transparent_fields = split_1c_braced_fields(&text[offset..transparent_end], 0)?;
    if !parse_1c_bool_flag(transparent_fields.first()?.trim())? {
        return None;
    }
    let x = transparent_fields.get(2)?.trim().parse().ok()?;
    let y = transparent_fields.get(3)?.trim().parse().ok()?;
    Some((x, y))
}

pub(super) fn format_ext_picture_xml(
    file_name: &str,
    transparent_pixel: Option<(i32, i32)>,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ExtPicture xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<Picture>\r\n\
\t\t<xr:Abs>{file_name}</xr:Abs>\r\n\
\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n",
        source_version.as_str(),
        xml_bool(transparent_pixel.is_some())
    );
    if let Some((x, y)) = transparent_pixel {
        xml.push_str(&format!(
            "\t\t<xr:TransparentPixel x=\"{x}\" y=\"{y}\"/>\r\n"
        ));
    }
    xml.push_str(
        "\t</Picture>\r\n\
</ExtPicture>",
    );
    xml
}

pub(super) fn extract_base64_payload(text: &str) -> Option<&str> {
    let prefix = "{#base64:";
    let start = text.find(prefix)? + prefix.len();
    let end = text[start..].find('}')? + start;
    Some(&text[start..end])
}

pub(super) fn ext_picture_file_name(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "Picture.png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "Picture.gif"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "Picture.jpg"
    } else if bytes.starts_with(b"BM") {
        "Picture.bmp"
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        "Picture.ico"
    } else if bytes.starts_with(b"PK\x03\x04") {
        "Picture.zip"
    } else if let Ok(text) = std::str::from_utf8(bytes) {
        let trimmed = text.trim_start_matches('\u{feff}').trim_start();
        if is_svg_text(text) {
            "Picture.svg"
        } else if trimmed.starts_with('<') {
            "Picture.xml"
        } else {
            "Picture.txt"
        }
    } else {
        "Picture.bin"
    }
}

pub(super) fn is_svg_content(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    is_svg_text(text)
}

pub(super) fn is_svg_text(text: &str) -> bool {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with("<svg") || text.starts_with("<?xml") && text.contains("<svg")
}

pub(super) fn decode_base64_mime(input: &str) -> Option<Vec<u8>> {
    let values = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if values.len() % 4 != 0 {
        return None;
    }

    let mut output = Vec::with_capacity(values.len() / 4 * 3);
    for chunk in values.chunks(4) {
        let mut decoded = [0u8; 4];
        let mut padding = 0usize;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if byte == b'=' {
                padding += 1;
                decoded[index] = 0;
                continue;
            }
            if padding > 0 {
                return None;
            }
            decoded[index] = base64_value(byte)?;
        }
        if padding > 2 {
            return None;
        }
        output.push((decoded[0] << 2) | (decoded[1] >> 4));
        if padding < 2 {
            output.push((decoded[1] << 4) | (decoded[2] >> 2));
        }
        if padding < 1 {
            output.push((decoded[2] << 6) | decoded[3]);
        }
    }

    Some(output)
}

#[cfg(test)]
pub(super) fn encode_base64_for_test(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(ALPHABET[(b0 >> 2) as usize] as char);
        output.push(ALPHABET[((b0 & 0x03) << 4 | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((b1 & 0x0f) << 2 | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

pub(super) fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub(super) fn format_help_xml(pages: &[HelpPage]) -> String {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Help xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
    );
    for page in pages {
        xml.push_str("\t<Page>");
        xml.push_str(&escape_xml_text(&page.page));
        xml.push_str("</Page>\r\n");
    }
    xml.push_str("</Help>");
    xml
}

pub(super) fn format_predefined_data_xml(
    model: PredefinedDataSourceModel,
    items: &[PredefinedItem],
    object_refs: &BTreeMap<String, String>,
    predefined_item_refs: &BTreeMap<String, String>,
) -> Result<String> {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<PredefinedData xmlns=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"{}\" version=\"2.20\">\r\n",
        escape_xml_text(model.xsi_type)
    );
    for item in items {
        push_predefined_item_xml(
            &mut xml,
            item,
            model.item_layout,
            object_refs,
            predefined_item_refs,
            1,
        )?;
    }
    xml.push_str("</PredefinedData>\r\n");
    Ok(xml)
}

pub(super) fn push_predefined_item_xml(
    xml: &mut String,
    item: &PredefinedItem,
    layout: PredefinedItemLayout,
    object_refs: &BTreeMap<String, String>,
    predefined_item_refs: &BTreeMap<String, String>,
    indent: usize,
) -> Result<()> {
    let tab = "\t".repeat(indent);
    xml.push_str(&format!(
        "{tab}<Item id=\"{}\">\r\n\
{tab}\t<Name>{}</Name>\r\n",
        escape_xml_text(&item.id),
        escape_xml_element_text(&item.name),
    ));
    push_predefined_text_element(xml, &tab, "Code", &item.code);
    push_predefined_text_element(xml, &tab, "Description", &item.description);

    match (&item.data, layout) {
        (
            PredefinedItemData::Generic {
                value_types,
                is_folder,
            },
            PredefinedItemLayout::Generic,
        ) => {
            xml.push_str(&format_predefined_type_xml(value_types, indent + 1));
            xml.push_str(&format!(
                "{tab}\t<IsFolder>{}</IsFolder>\r\n",
                xml_bool(*is_folder)
            ));
        }
        (
            PredefinedItemData::Account {
                account_type,
                off_balance,
                order,
                accounting_flags,
                ext_dimension_types,
            },
            PredefinedItemLayout::Account,
        ) => {
            let account_type = match account_type {
                PredefinedAccountType::Active => "Active",
                PredefinedAccountType::Passive => "Passive",
                PredefinedAccountType::ActivePassive => "ActivePassive",
            };
            xml.push_str(&format!(
                "{tab}\t<AccountType>{account_type}</AccountType>\r\n\
{tab}\t<OffBalance>{}</OffBalance>\r\n\
{tab}\t<Order>{}</Order>\r\n",
                xml_bool(*off_balance),
                escape_xml_text(order),
            ));
            push_predefined_flags_xml(xml, accounting_flags, object_refs, indent + 1)?;
            push_predefined_ext_dimension_types_xml(
                xml,
                ext_dimension_types,
                object_refs,
                predefined_item_refs,
                indent + 1,
            )?;
        }
        (
            PredefinedItemData::Calculation {
                action_period_is_base,
                displaced,
                base,
                leading,
            },
            PredefinedItemLayout::Calculation,
        ) => {
            xml.push_str(&format!(
                "{tab}\t<ActionPeriodIsBase>{}</ActionPeriodIsBase>\r\n",
                xml_bool(*action_period_is_base)
            ));
            push_predefined_calculation_type_refs_xml(
                xml,
                "Displaced",
                displaced,
                predefined_item_refs,
                indent + 1,
            )?;
            push_predefined_calculation_type_refs_xml(
                xml,
                "Base",
                base,
                predefined_item_refs,
                indent + 1,
            )?;
            push_predefined_calculation_type_refs_xml(
                xml,
                "Leading",
                leading,
                predefined_item_refs,
                indent + 1,
            )?;
        }
        _ => bail!(
            "predefined item {} does not match its source model",
            item.id
        ),
    }

    if !item.children.is_empty() {
        xml.push_str(&format!("{tab}\t<ChildItems>\r\n"));
        for child in &item.children {
            push_predefined_item_xml(
                xml,
                child,
                layout,
                object_refs,
                predefined_item_refs,
                indent + 2,
            )?;
        }
        xml.push_str(&format!("{tab}\t</ChildItems>\r\n"));
    }
    xml.push_str(&format!("{tab}</Item>\r\n"));
    Ok(())
}

fn push_predefined_text_element(xml: &mut String, tab: &str, name: &str, value: &str) {
    if value.is_empty() {
        xml.push_str(&format!("{tab}\t<{name}/>\r\n"));
    } else {
        xml.push_str(&format!(
            "{tab}\t<{name}>{}</{name}>\r\n",
            escape_xml_element_text(value)
        ));
    }
}

fn push_predefined_flags_xml(
    xml: &mut String,
    flags: &[PredefinedFlag],
    object_refs: &BTreeMap<String, String>,
    indent: usize,
) -> Result<()> {
    if flags.is_empty() {
        return Ok(());
    }
    let tab = "\t".repeat(indent);
    xml.push_str(&format!("{tab}<AccountingFlags>\r\n"));
    for flag in flags {
        let reference = object_refs.get(&flag.reference_uuid).with_context(|| {
            format!(
                "missing metadata reference for predefined accounting flag {}",
                flag.reference_uuid
            )
        })?;
        xml.push_str(&format!(
            "{tab}\t<Flag ref=\"{}\">{}</Flag>\r\n",
            escape_xml_text(reference),
            xml_bool(flag.value),
        ));
    }
    xml.push_str(&format!("{tab}</AccountingFlags>\r\n"));
    Ok(())
}

fn push_predefined_ext_dimension_types_xml(
    xml: &mut String,
    ext_dimension_types: &[PredefinedExtDimensionType],
    object_refs: &BTreeMap<String, String>,
    predefined_item_refs: &BTreeMap<String, String>,
    indent: usize,
) -> Result<()> {
    let tab = "\t".repeat(indent);
    if ext_dimension_types.is_empty() {
        xml.push_str(&format!("{tab}<ExtDimensionTypes/>\r\n"));
        return Ok(());
    }

    xml.push_str(&format!("{tab}<ExtDimensionTypes>\r\n"));
    for ext_dimension_type in ext_dimension_types {
        let reference = predefined_item_refs
            .get(&ext_dimension_type.item_uuid)
            .with_context(|| {
                format!(
                    "missing predefined item reference for ext dimension type {}",
                    ext_dimension_type.item_uuid
                )
            })?;
        xml.push_str(&format!(
            "{tab}\t<ExtDimensionType name=\"{}\">\r\n\
{tab}\t\t<Turnover>{}</Turnover>\r\n",
            escape_xml_text(reference),
            xml_bool(ext_dimension_type.turnover),
        ));
        push_predefined_flags_xml(
            xml,
            &ext_dimension_type.accounting_flags,
            object_refs,
            indent + 2,
        )?;
        xml.push_str(&format!("{tab}\t</ExtDimensionType>\r\n"));
    }
    xml.push_str(&format!("{tab}</ExtDimensionTypes>\r\n"));
    Ok(())
}

fn push_predefined_calculation_type_refs_xml(
    xml: &mut String,
    element_name: &str,
    item_uuids: &[String],
    predefined_item_refs: &BTreeMap<String, String>,
    indent: usize,
) -> Result<()> {
    if item_uuids.is_empty() {
        return Ok(());
    }
    let tab = "\t".repeat(indent);
    xml.push_str(&format!("{tab}<{element_name}>\r\n"));
    for item_uuid in item_uuids {
        let reference = predefined_item_refs.get(item_uuid).with_context(|| {
            format!("missing predefined calculation type reference {item_uuid}")
        })?;
        xml.push_str(&format!(
            "{tab}\t<CalculationType>{}</CalculationType>\r\n",
            escape_xml_text(reference),
        ));
    }
    xml.push_str(&format!("{tab}</{element_name}>\r\n"));
    Ok(())
}

pub(super) fn format_predefined_type_xml(
    value_types: &[ConstantValueType],
    indent: usize,
) -> String {
    if value_types.is_empty() {
        return String::new();
    }
    let tab = "\t".repeat(indent);
    let mut xml = format!("{tab}<Type>\r\n");
    for value_type in value_types {
        match value_type {
            ConstantValueType::Reference { reference } if reference.starts_with("cfg:") => {
                xml.push_str(&format!(
                    "{tab}\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d4p1:{}</v8:Type>\r\n",
                    escape_xml_text(reference.trim_start_matches("cfg:"))
                ));
            }
            _ => {
                xml.push_str(&format!(
                    "{tab}\t<v8:Type>{}</v8:Type>\r\n",
                    metadata_type_xml_name(value_type)
                ));
            }
        }
    }
    if let Some((length, allowed_length_flag)) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::String {
            length: Some(length),
            allowed_length_flag,
        } = value_type
        {
            Some((*length, *allowed_length_flag))
        } else {
            None
        }
    }) {
        xml.push_str(&format!("{tab}\t<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{tab}\t\t<v8:Length>{length}</v8:Length>\r\n"));
        xml.push_str(&format!(
            "{tab}\t\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            predefined_string_allowed_length_xml(allowed_length_flag)
        ));
        xml.push_str(&format!("{tab}\t</v8:StringQualifiers>\r\n"));
    }
    xml.push_str(&format!("{tab}</Type>\r\n"));
    xml
}

pub(super) fn predefined_string_allowed_length_xml(value: u8) -> &'static str {
    match value {
        0 => "Fixed",
        _ => "Variable",
    }
}

pub(super) struct JobSchedule {
    begin_date: String,
    end_date: String,
    begin_time: String,
    end_time: String,
    completion_time: String,
    completion_interval: String,
    repeat_period_in_day: String,
    repeat_pause: String,
    week_day_in_month: String,
    day_in_month: String,
    week_days: Vec<String>,
    months: Vec<String>,
    weeks_period: String,
    days_repeat_period: String,
    detailed_daily_schedules: Vec<JobSchedule>,
}

pub(super) fn extract_schedule_xml(bytes: &[u8]) -> Result<String> {
    let inflated = inflate_raw_deflate(bytes)?;
    let text = String::from_utf8(inflated).context("schedule blob is not UTF-8")?;
    let schedule = parse_job_schedule_text(text.trim_start_matches('\u{feff}'))
        .context("failed to parse compact schedule")?;
    Ok(format_job_schedule_xml(&schedule))
}

pub(super) fn extract_standalone_content_xml(
    bytes: &[u8],
    references: &StandaloneContentReferences,
) -> Result<Vec<u8>> {
    let inflated = inflate_raw_deflate(bytes).context("failed to inflate standalone content")?;
    let text = String::from_utf8(inflated).context("standalone content is not valid UTF-8")?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)
        .ok_or_else(|| anyhow!("standalone content is not a 1C braced value"))?;
    if fields.first().map(|field| field.trim()) != Some("2") {
        bail!("standalone content has unsupported marker");
    }
    let count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("standalone content has invalid item count"))?;
    if fields.len() < 2 + count {
        bail!("standalone content item count exceeds field count");
    }

    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<StandaloneContent xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
    );
    let mut selected_uuids = fields
        .iter()
        .skip(2)
        .take(count)
        .map(|uuid| uuid.trim())
        .collect::<Vec<_>>();
    selected_uuids.sort_unstable();
    for uuid in selected_uuids {
        let reference = references
            .object_refs
            .get(uuid)
            .ok_or_else(|| anyhow!("standalone content reference not found: {uuid}"))?;
        push_standalone_metadata_item_xml(&mut xml, "UsedItem", reference);
    }
    let mut index = 2 + count;
    let mut has_extended_sections = false;
    if let Some(child_count) = fields
        .get(index)
        .and_then(|field| field.trim().parse::<usize>().ok())
    {
        has_extended_sections = true;
        index += 1;
        if fields.len() < index + child_count {
            bail!("standalone content child item count exceeds field count");
        }
        let mut child_uuids = fields
            .iter()
            .skip(index)
            .take(child_count)
            .map(|uuid| uuid.trim())
            .collect::<Vec<_>>();
        child_uuids.sort_unstable();
        for uuid in child_uuids {
            let reference = references
                .object_refs
                .get(uuid)
                .ok_or_else(|| anyhow!("standalone content reference not found: {uuid}"))?;
            push_standalone_metadata_item_xml(&mut xml, "UnusedItem", reference);
        }
        let mut trailing_uuids = fields
            .iter()
            .skip(index + child_count)
            .filter_map(|uuid| parse_non_zero_uuid(uuid.trim()))
            .collect::<Vec<_>>();
        trailing_uuids.sort_unstable();
        for uuid in trailing_uuids {
            let reference = references
                .object_refs
                .get(&uuid)
                .ok_or_else(|| anyhow!("standalone content reference not found: {uuid}"))?;
            push_standalone_priority_item_xml(&mut xml, reference);
        }
        if child_count > 0 {
            xml.push_str(
                "\t<DataExchangeSettings>\r\n\
\t\t<ExchangeOnChangeData>true</ExchangeOnChangeData>\r\n\
\t\t<ExchangePeriod>300</ExchangePeriod>\r\n\
\t\t<TransactionCount>1000</TransactionCount>\r\n\
\t\t<InactiveNodesCleanupTimeout>0</InactiveNodesCleanupTimeout>\r\n\
\t</DataExchangeSettings>\r\n",
            );
        }
    }
    if has_extended_sections {
        xml.push_str("</StandaloneContent>");
    } else {
        xml.push_str("</StandaloneContent>\r\n");
    }
    Ok(xml.into_bytes())
}

pub(super) fn push_standalone_metadata_item_xml(xml: &mut String, tag: &str, reference: &str) {
    xml.push_str(&format!("\t<{tag}>\r\n"));
    xml.push_str(&format!(
        "\t\t<Metadata>{}</Metadata>\r\n",
        escape_xml_text(reference)
    ));
    xml.push_str(&format!("\t</{tag}>\r\n"));
}

pub(super) fn push_standalone_priority_item_xml(xml: &mut String, reference: &str) {
    xml.push_str("\t<PriorityItem>\r\n");
    xml.push_str(&format!(
        "\t\t<Metadata>{}</Metadata>\r\n",
        escape_xml_text(reference)
    ));
    xml.push_str("\t\t<Priority>LocalServer</Priority>\r\n");
    xml.push_str("\t</PriorityItem>\r\n");
}

pub(super) fn parse_job_schedule_text(text: &str) -> Option<JobSchedule> {
    let fields = split_1c_braced_fields(text, 0)?;
    parse_job_schedule_fields(&fields, true)
}

pub(super) fn parse_job_schedule_fields(
    fields: &[&str],
    include_details: bool,
) -> Option<JobSchedule> {
    let mut index = 0usize;
    let begin_date = format_1c_date(fields.get(index)?.trim())?;
    index += 1;
    let end_date = format_1c_date(fields.get(index)?.trim())?;
    index += 1;
    let begin_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let end_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let completion_time = format_1c_time(fields.get(index)?.trim())?;
    index += 1;
    let completion_interval = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let repeat_period_in_day = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let repeat_pause = parse_schedule_number(fields.get(index)?)?;
    index += 1;

    let week_days_count = fields.get(index)?.trim().parse::<usize>().ok()?;
    index += 1;
    let week_days = parse_schedule_number_list(&fields, &mut index, week_days_count)?;

    let week_day_in_month = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let day_in_month = parse_schedule_number(fields.get(index)?)?;
    index += 1;

    let months_count = fields.get(index)?.trim().parse::<usize>().ok()?;
    index += 1;
    let months = parse_schedule_number_list(&fields, &mut index, months_count)?;

    let weeks_period = parse_schedule_number(fields.get(index)?)?;
    index += 1;
    let days_repeat_period = parse_schedule_number(fields.get(index)?)?;
    index += 1;

    let detailed_daily_schedules = if include_details {
        let count = fields
            .get(index)
            .and_then(|field| field.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mut schedules = Vec::with_capacity(count);
        index += usize::from(fields.get(index).is_some());
        for field in fields.iter().skip(index).take(count) {
            let detail_fields = split_1c_braced_fields(field, 0)?;
            schedules.push(parse_job_schedule_fields(&detail_fields, false)?);
        }
        schedules
    } else {
        Vec::new()
    };

    Some(JobSchedule {
        begin_date,
        end_date,
        begin_time,
        end_time,
        completion_time,
        completion_interval,
        repeat_period_in_day,
        repeat_pause,
        week_day_in_month,
        day_in_month,
        week_days,
        months,
        weeks_period,
        days_repeat_period,
        detailed_daily_schedules,
    })
}

pub(super) fn parse_schedule_number_list(
    fields: &[&str],
    index: &mut usize,
    count: usize,
) -> Option<Vec<String>> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(parse_schedule_number(fields.get(*index)?)?);
        *index += 1;
    }
    Some(values)
}

pub(super) fn parse_schedule_number(value: &str) -> Option<String> {
    let value = value.trim();
    if value.chars().all(|ch| ch.is_ascii_digit()) {
        Some(value.to_string())
    } else {
        None
    }
}

pub(super) fn format_1c_date(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}",
        &value[0..4],
        &value[4..6],
        &value[6..8]
    ))
}

pub(super) fn format_1c_date_time(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}-{}-{}T{}:{}:{}",
        &value[0..4],
        &value[4..6],
        &value[6..8],
        &value[8..10],
        &value[10..12],
        &value[12..14]
    ))
}

pub(super) fn format_1c_time(value: &str) -> Option<String> {
    if value.len() != 14 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "{}:{}:{}",
        &value[8..10],
        &value[10..12],
        &value[12..14]
    ))
}

pub(super) fn format_job_schedule_xml(schedule: &JobSchedule) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<JobSchedule xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
\t<Schedule{}>\r\n",
        format_job_schedule_attrs(schedule)
    );
    push_job_schedule_lists_xml(&mut xml, "\t\t", schedule);
    for detail in &schedule.detailed_daily_schedules {
        xml.push_str(&format!(
            "\t\t<ent:DetailedDailySchedules{}>\r\n",
            format_job_schedule_attrs(detail)
        ));
        push_job_schedule_lists_xml(&mut xml, "\t\t\t", detail);
        xml.push_str("\t\t</ent:DetailedDailySchedules>\r\n");
    }
    xml.push_str("\t</Schedule>\r\n</JobSchedule>");
    xml
}

pub(super) fn format_job_schedule_attrs(schedule: &JobSchedule) -> String {
    format!(
        " BeginDate=\"{}\" EndDate=\"{}\" BeginTime=\"{}\" EndTime=\"{}\" CompletionTime=\"{}\" CompletionInterval=\"{}\" RepeatPeriodInDay=\"{}\" RepeatPause=\"{}\" WeekDayInMonth=\"{}\" DayInMonth=\"{}\" WeeksPeriod=\"{}\" DaysRepeatPeriod=\"{}\"",
        schedule.begin_date,
        schedule.end_date,
        schedule.begin_time,
        schedule.end_time,
        schedule.completion_time,
        schedule.completion_interval,
        schedule.repeat_period_in_day,
        schedule.repeat_pause,
        schedule.week_day_in_month,
        schedule.day_in_month,
        schedule.weeks_period,
        schedule.days_repeat_period
    )
}

pub(super) fn push_job_schedule_lists_xml(xml: &mut String, indent: &str, schedule: &JobSchedule) {
    push_job_schedule_list_xml(xml, indent, "WeekDays", &schedule.week_days);
    push_job_schedule_list_xml(xml, indent, "Months", &schedule.months);
}

pub(super) fn push_job_schedule_list_xml(
    xml: &mut String,
    indent: &str,
    name: &str,
    values: &[String],
) {
    if values.is_empty() {
        xml.push_str(&format!("{indent}<ent:{name}/>\r\n"));
    } else {
        xml.push_str(&format!(
            "{indent}<ent:{name}>{}</ent:{name}>\r\n",
            values.join(" ")
        ));
    }
}
