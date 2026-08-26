use super::*;
use crate::compiler::families::assets::{
    ConfigRowId, SourceAssetRelationError, SourceAssetRole, SourceAssetRoute,
};

#[derive(Debug, Clone)]
pub(super) struct BodyOwnerSourceReference {
    pub(super) kind: String,
    pub(super) canonical_name: String,
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
                canonical_name: header.name.clone(),
                object_path,
            },
        );
    }
    index
}

pub(super) fn configuration_module_groups(file_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in file_names {
        let Ok(row_id) = ConfigRowId::parse(file_name) else {
            continue;
        };
        suffixes_by_id
            .entry(row_id.owner())
            .or_default()
            .insert(row_id.suffix_component());
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
        let Ok(row_id) = ConfigRowId::parse(file_name) else {
            continue;
        };
        suffixes_by_id
            .entry(row_id.owner())
            .or_default()
            .insert(row_id.suffix_component());
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
    let row_id = ConfigRowId::parse(file_name).ok()?;
    let owner_uuid = row_id.owner();
    let suffix = row_id.suffix_component();

    if let Some(form_ref) = context.form_refs.get(owner_uuid)
        && suffix != "0"
        && parse_help_blob_pages(bytes).is_some()
    {
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        return Some(SourceAsset {
            primary_path: form_dir
                .join(crate::compiler::families::assets::SourceAssetRegistry.help_relative_path()),
            kind: SourceAssetKind::Help,
        });
    }

    if context.configuration_module_groups.contains(owner_uuid)
        && matches!(suffix, "9" | "a")
        && parse_command_interface_blob(bytes, context.command_refs, context.metadata_refs)
            .is_some()
    {
        let route = crate::compiler::families::assets::SourceAssetRegistry
            .route_by_suffix("Configuration", suffix)
            .filter(|route| {
                matches!(
                    route.role(),
                    crate::compiler::families::assets::SourceAssetRole::MainSectionCommandInterface
                        | crate::compiler::families::assets::SourceAssetRole::CommandInterface
                )
            })?;
        return Some(SourceAsset {
            primary_path: PathBuf::from(route.relative_path()),
            kind: SourceAssetKind::CommandInterface,
        });
    }

    let owner = context.body_owners.get(owner_uuid)?;
    if let Some(asset) = dynamic_owner_bound_source_asset(
        &row_id,
        owner,
        context.object_refs.get(owner_uuid).map(String::as_str),
        bytes,
        context.role_rights_object_refs,
        context.field_refs,
    ) {
        return Some(asset);
    }
    if let Some(route) = crate::compiler::families::assets::SourceAssetRegistry
        .route_by_suffix(&owner.kind, suffix)
        .filter(|route| {
            route.role() == crate::compiler::families::assets::SourceAssetRole::CommandInterface
        })
        && matches!(suffix, "0" | "1")
        && parse_command_interface_blob(bytes, context.command_refs, context.metadata_refs)
            .is_some_and(|interface| !interface.is_empty())
    {
        return Some(SourceAsset {
            primary_path: owner.object_path.join(route.relative_path()),
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
            primary_path: owner
                .object_path
                .join(crate::compiler::families::assets::SourceAssetRegistry.help_relative_path()),
            kind: SourceAssetKind::Help,
        });
    }
    if let Some(model) = predefined_data_source_model(&owner.kind)
        && predefined_data_suffix(&owner.kind) == Some(suffix)
        && parse_predefined_data_blob_with_model(bytes, context.type_index, model)
            .is_some_and(|items| !items.is_empty())
    {
        let route = predefined_data_route(&owner.kind)?;
        return Some(SourceAsset {
            primary_path: owner.object_path.join(route.relative_path()),
            kind: SourceAssetKind::PredefinedData { model },
        });
    }
    if let Some(module_route) = module_owner_route(&owner.kind, suffix)
        && unpack_module_blob_text(bytes).is_err()
        && is_binary_module_container(bytes)
    {
        return Some(SourceAsset {
            primary_path: owner
                .object_path
                .join(Path::new(module_route.relative_path()).with_extension("bin")),
            kind: SourceAssetKind::InflatedBinary,
        });
    }
    None
}

pub(super) fn dynamic_owner_bound_source_asset(
    row_id: &ConfigRowId<'_>,
    owner: &BodyOwnerSourceReference,
    owner_reference: Option<&str>,
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
) -> Option<SourceAsset> {
    let owner_family = FamilyId::parse(&owner.kind).ok()?;
    let route = crate::compiler::families::assets::SourceAssetRegistry
        .owner_bound_relation(&owner_family, row_id.suffix())
        .ok()?;
    owner_bound_source_asset(
        route,
        &owner.object_path,
        &owner.canonical_name,
        owner_reference,
        bytes,
        object_refs,
        field_refs,
    )
    .ok()
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum OwnerBoundSourceAssetFailure {
    Decoder,
    /// Decoded cleanly to zero items -- not a decoder failure, but still not
    /// an asset: the platform does not write `Ext/Aggregates.xml` for a
    /// register whose aggregates row decodes to an empty set (ERP UH
    /// `AccumulationRegisters/ОперацииБюджетов`, confirmed against its
    /// native tree). Kept distinct from `Decoder` so the miss reason stays
    /// honest about what actually happened.
    Empty,
    Relation(SourceAssetRelationError),
}

const ROLE_RIGHTS_DECODER_MISS: &str = "role_rights_decoder_failed";
const ACCUMULATION_REGISTER_AGGREGATES_DECODER_MISS: &str =
    "accumulation_register_aggregates_decoder_failed";
const ACCUMULATION_REGISTER_AGGREGATES_EMPTY: &str = "accumulation_register_aggregates_empty";

pub(super) fn owner_bound_source_asset(
    route: &SourceAssetRoute,
    object_path: &Path,
    header_name: &str,
    owner_reference: Option<&str>,
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
) -> std::result::Result<SourceAsset, OwnerBoundSourceAssetFailure> {
    let registry = crate::compiler::families::assets::SourceAssetRegistry;
    match route.role() {
        SourceAssetRole::Rights => {
            parse_role_rights_blob(bytes, object_refs, field_refs)
                .ok_or(OwnerBoundSourceAssetFailure::Decoder)?;
            registry
                .canonical_owner_name(route, header_name, owner_reference)
                .map_err(OwnerBoundSourceAssetFailure::Relation)?;
            Ok(SourceAsset {
                primary_path: object_path.join(route.relative_path()),
                kind: SourceAssetKind::RoleRights,
            })
        }
        SourceAssetRole::Aggregates => {
            let aggregates = parse_accumulation_register_aggregates_blob(bytes)
                .ok_or(OwnerBoundSourceAssetFailure::Decoder)?;
            if aggregates.is_empty() {
                return Err(OwnerBoundSourceAssetFailure::Empty);
            }
            let register_name = registry
                .canonical_owner_name(route, header_name, owner_reference)
                .map_err(OwnerBoundSourceAssetFailure::Relation)?
                .to_owned();
            Ok(SourceAsset {
                primary_path: object_path.join(route.relative_path()),
                kind: SourceAssetKind::AccumulationRegisterAggregates { register_name },
            })
        }
        _ => Err(OwnerBoundSourceAssetFailure::Relation(
            SourceAssetRelationError::UnsupportedFamily,
        )),
    }
}

fn owner_bound_decoder_miss(role: SourceAssetRole) -> Option<&'static str> {
    match role {
        SourceAssetRole::Rights => Some(ROLE_RIGHTS_DECODER_MISS),
        SourceAssetRole::Aggregates => Some(ACCUMULATION_REGISTER_AGGREGATES_DECODER_MISS),
        _ => None,
    }
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

/// Every storage entry whose output path another entry also claims, with the
/// message naming both claimants.
///
/// A collision is a refusal about those entries, not about the export: writing
/// either one would silently overwrite the other, so both are withheld and
/// named, and every entry that claims its path alone is still produced. Taking
/// down the whole run instead hides the rest of the picture, which is exactly
/// what a foreign configuration is exported to reveal.
pub(super) fn colliding_source_asset_paths(
    source_assets: &BTreeMap<String, SourceAsset>,
    diagnostics: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut claimants = BTreeMap::<String, Vec<&str>>::new();
    for (file_name, asset) in source_assets {
        let path = asset.primary_path.to_string_lossy().replace('\\', "/");
        claimants.entry(path).or_default().push(file_name.as_str());
    }
    let mut refused = BTreeMap::<String, String>::new();
    for (path, names) in claimants {
        if names.len() < 2 {
            continue;
        }
        let mut message = format!(
            "source asset output path {path} is produced by both {} and {}",
            names[0],
            names[1..].join(" and ")
        );
        for name in &names {
            append_source_asset_diagnostic(&mut message, name, diagnostics);
        }
        for name in names {
            refused.insert(name.to_string(), message.clone());
        }
    }
    refused
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum OutputWriteRoute {
    SourceAsset,
    MetadataXml,
    ModuleText,
}

/// Refused output paths for the write routes `colliding_source_asset_paths`
/// cannot see: a form/template/subsystem's own descriptor XML, and a
/// canonical module body. Grouped by write route rather than a single flat
/// file-name key because the same row uuid can be a claimant on more than
/// one route at once -- a form body is both a source asset (`Ext/Form.xml`)
/// and a module-text owner (`Ext/Form/Module.bsl`) -- so refusing one
/// route's collision must never silently withhold the other, unrelated
/// route's output for the same uuid.
#[derive(Debug, Default)]
pub(super) struct ReferenceOutputCollisions {
    /// Additional `source_assets` entries this pass refuses that
    /// `colliding_source_asset_paths` could not see, because the collision
    /// is with a form/template/subsystem/module path rather than another
    /// source asset. Keyed exactly like `source_assets`.
    pub(super) source_assets: BTreeMap<String, String>,
    /// Keyed by the metadata row's own uuid: a form, template, or
    /// subsystem's own descriptor-XML row.
    pub(super) metadata_xml: BTreeMap<String, String>,
    /// Keyed by the module body row id (`<uuid>.<suffix>`), matching
    /// `module_text_paths`.
    pub(super) module_text: BTreeMap<String, String>,
}

fn record_output_claim(
    claimants: &mut BTreeMap<String, Vec<(OutputWriteRoute, String)>>,
    route: OutputWriteRoute,
    file_name: &str,
    path: &Path,
) {
    let path = path.to_string_lossy().replace('\\', "/");
    claimants
        .entry(path)
        .or_default()
        .push((route, file_name.to_string()));
}

/// Every canonical output path claimed by more than one row across every
/// file-writing route: `source_assets`, form/template/subsystem descriptor
/// XML, and canonical module bodies. `colliding_source_asset_paths` only
/// sees `source_assets` -- forms, templates, subsystems, and modules resolve
/// their own output path through their own reference index
/// (`form_refs`/`template_refs`/`subsystem_refs`/`module_text_paths`) that
/// never passed through that check, so two objects resolving to the
/// identical path there raced a writer instead of being refused. Folding
/// `source_assets` back in here also catches a collision between routes, not
/// just within one. A collision is a refusal about those specific
/// claimants, not about the export: both are withheld and named, and every
/// path claimed once alone is still produced.
pub(super) fn colliding_reference_output_paths(
    source_assets: &BTreeMap<String, SourceAsset>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
    module_text_paths: &BTreeMap<String, PathBuf>,
) -> ReferenceOutputCollisions {
    let mut claimants = BTreeMap::<String, Vec<(OutputWriteRoute, String)>>::new();
    for (file_name, asset) in source_assets {
        record_output_claim(
            &mut claimants,
            OutputWriteRoute::SourceAsset,
            file_name,
            &asset.primary_path,
        );
    }
    for (uuid, form_ref) in form_refs {
        record_output_claim(
            &mut claimants,
            OutputWriteRoute::MetadataXml,
            uuid,
            &form_ref.relative_path,
        );
    }
    for (uuid, template_ref) in template_refs {
        record_output_claim(
            &mut claimants,
            OutputWriteRoute::MetadataXml,
            uuid,
            &template_ref.relative_path,
        );
    }
    for (uuid, subsystem_ref) in subsystem_refs {
        record_output_claim(
            &mut claimants,
            OutputWriteRoute::MetadataXml,
            uuid,
            &subsystem_ref.relative_path,
        );
    }
    for (file_name, path) in module_text_paths {
        record_output_claim(
            &mut claimants,
            OutputWriteRoute::ModuleText,
            file_name,
            path,
        );
    }

    let mut refused = ReferenceOutputCollisions::default();
    for (path, entries) in claimants {
        let mut distinct_names = entries
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>();
        distinct_names.sort_unstable();
        distinct_names.dedup();
        if distinct_names.len() < 2 {
            continue;
        }
        let message = format!(
            "output path {path} is produced by both {} and {}",
            distinct_names[0],
            distinct_names[1..].join(" and ")
        );
        for (route, name) in entries {
            let bucket = match route {
                OutputWriteRoute::SourceAsset => &mut refused.source_assets,
                OutputWriteRoute::MetadataXml => &mut refused.metadata_xml,
                OutputWriteRoute::ModuleText => &mut refused.module_text,
            };
            bucket.insert(name, message.clone());
        }
    }
    refused
}

#[derive(Clone, Copy)]
pub(crate) enum PredefinedDataRowsetLayout {
    NestedTable,
    Root,
}

#[derive(Clone, Copy)]
pub(crate) enum PredefinedItemLayout {
    Generic,
    /// Reads exactly like `Generic`, but its items carry a value-type slot, so
    /// every item writes a `Type` element -- empty for the folders that have no
    /// type. All 166 items of this shape in 1C:УТ 11.5.27.75 write one: 144
    /// typed leaves and 22 empty folders, with no counterexample.
    Characteristic,
    Account,
    Calculation,
}

#[derive(Clone, Copy)]
pub(crate) struct PredefinedDataSourceModel {
    xsi_type: &'static str,
    root_tag: &'static str,
    rowset_layout: PredefinedDataRowsetLayout,
    unwrap_single_root: bool,
    item_layout: PredefinedItemLayout,
}

/// Owner identity an `Ext/AdditionalIndexes.xml` body needs to be readable.
///
/// The stored body is a serialized 1C value whose records name their table by
/// uuid, so the owner has to travel with the asset: only it turns that uuid into
/// a source table name.
#[derive(Clone)]
pub(crate) struct AdditionalIndexesOwner {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) uuid: String,
}

#[derive(Clone)]
pub(crate) enum SourceAssetKind {
    AccumulationRegisterAggregates { register_name: String },
    AdditionalIndexes { owner: AdditionalIndexesOwner },
    CommandInterface,
    ClientApplicationInterface,
    ExchangePlanContent,
    BusinessProcessFlowchart,
    DataCompositionSchema,
    ExtPicture,
    Form { owner_reference: Option<String> },
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
    TemplateGraphicalScheme,
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

pub(super) enum WrittenSourceAsset {
    Emitted {
        primary_path: PathBuf,
        diagnostics: Vec<FormSourceAssetDiagnostic>,
    },
    OpaqueNotEmitted {
        primary_path: PathBuf,
        diagnostics: Vec<FormSourceAssetDiagnostic>,
    },
    RejectedNotEmitted {
        primary_path: PathBuf,
        diagnostics: Vec<FormSourceAssetDiagnostic>,
    },
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
        let Ok(row_id) = ConfigRowId::parse(file_name) else {
            continue;
        };
        suffixes_by_id
            .entry(row_id.owner())
            .or_default()
            .insert(row_id.suffix_component());
    }
    let role_rights_object_refs = build_role_rights_object_reference_index(object_refs, form_refs);

    let mut paths = BTreeMap::new();
    for (metadata_id, suffixes) in suffixes_by_id {
        if file_names.contains(metadata_id) {
            continue;
        }
        let is_configuration_group = is_configuration_module_group(&suffixes);
        for route in crate::compiler::families::assets::SourceAssetRegistry.configuration_routes() {
            let Some(kind) = configuration_source_asset_kind(route.role()) else {
                continue;
            };
            let suffix = route.suffix().trim_start_matches('.');
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
                        primary_path: PathBuf::from(route.relative_path()),
                        kind,
                    },
                );
            }
        }
        let standalone_id = format!("{metadata_id}.f");
        if is_configuration_group && file_names.contains(standalone_id.as_str()) {
            let route = crate::compiler::families::assets::SourceAssetRegistry
                .route(
                    "Configuration",
                    crate::compiler::families::assets::SourceAssetRole::StandaloneContent,
                )
                .expect("configuration standalone route is registered");
            paths.insert(
                standalone_id,
                SourceAsset {
                    primary_path: PathBuf::from(route.relative_path()),
                    kind: SourceAssetKind::StandaloneContent,
                },
            );
        }
        for role in [
            crate::compiler::families::assets::SourceAssetRole::MainSectionCommandInterface,
            crate::compiler::families::assets::SourceAssetRole::CommandInterface,
        ] {
            let route = crate::compiler::families::assets::SourceAssetRegistry
                .route("Configuration", role)
                .expect("configuration command-interface route is registered");
            let suffix = route.suffix().trim_start_matches('.');
            if !is_configuration_group && !suffixes.contains(suffix) {
                continue;
            }
            let interface_id = format!("{metadata_id}.{suffix}");
            let is_selected_header = rows_by_file_name
                .get(interface_id.as_str())
                .is_some_and(|row| row.binary_hex.is_empty());
            // A decoded-but-entirely-empty root command interface (wire
            // shape `{7,0,0,0,0,0,0}`) is a record the platform tracks by
            // identity without ever rendering a file for -- confirmed
            // against WMS5's `МодульWebОбмена_ERP25.cf`, whose Configuration
            // `.9`/`.a` records decode cleanly to zero sections and stay
            // absent from the native export tree. `CommandInterface::is_empty`
            // is the fail-closed line: it only suppresses emission when
            // every section decoded to nothing, never on a decode failure.
            let is_command_interface = is_selected_header
                || rows_by_file_name
                    .get(interface_id.as_str())
                    .and_then(|row| decode_hex(&row.binary_hex).ok())
                    .is_some_and(|bytes| {
                        parse_command_interface_blob(&bytes, &command_refs, &metadata_refs)
                            .is_some_and(|interface| !interface.is_empty())
                    });
            if is_command_interface {
                paths.insert(
                    interface_id,
                    SourceAsset {
                        primary_path: PathBuf::from(route.relative_path()),
                        kind: SourceAssetKind::CommandInterface,
                    },
                );
            }
        }
    }
    for row in metadata_texts {
        let Some(discovery) = source_assets_from_metadata_text_inner(
            row,
            &file_names,
            &rows_by_file_name,
            &command_refs,
            &metadata_refs,
            &role_rights_object_refs,
            &field_refs,
            &type_index,
            &subsystem_refs,
        ) else {
            continue;
        };
        for (body_id, asset) in discovery.assets {
            paths.insert(body_id, asset);
        }
    }
    paths.extend(form_help_asset_paths(rows, &rows_by_file_name, &form_refs));
    paths.extend(form_body_asset_paths(&form_refs, &file_names));
    paths.extend(template_body_asset_paths(&template_refs, &file_names));

    paths
}

#[allow(clippy::too_many_arguments)]
pub(super) fn source_asset_discovery_misses(
    metadata_texts: &[MetadataTextRow],
    file_names: &BTreeSet<&str>,
    rows_by_file_name: &BTreeMap<&str, &ConfigRow>,
    command_refs: &BTreeMap<String, String>,
    metadata_refs: &BTreeMap<String, MetadataCommandReference>,
    object_refs: &BTreeMap<String, String>,
    field_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> BTreeMap<String, String> {
    let mut misses = BTreeMap::new();
    for row in metadata_texts.iter().filter(|row| row.folder.is_some()) {
        match source_assets_from_metadata_text_inner(
            row,
            file_names,
            rows_by_file_name,
            command_refs,
            metadata_refs,
            object_refs,
            field_refs,
            type_index,
            subsystem_refs,
        ) {
            Some(discovery) => misses.extend(discovery.misses),
            None => {
                misses.insert(
                    row.file_name.clone(),
                    "metadata_source_asset_relation_unclassified".to_string(),
                );
            }
        }
    }
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in file_names {
        let Ok(row_id) = ConfigRowId::parse(file_name) else {
            continue;
        };
        suffixes_by_id
            .entry(row_id.owner())
            .or_default()
            .insert(row_id.suffix_component());
    }
    for (owner_id, suffixes) in suffixes_by_id {
        if !is_configuration_module_group(&suffixes) {
            continue;
        }
        for suffix in ["9", "a"] {
            let body_id = format!("{owner_id}.{suffix}");
            if !file_names.contains(body_id.as_str()) {
                continue;
            }
            let parsed = rows_by_file_name
                .get(body_id.as_str())
                .and_then(|row| decode_hex(&row.binary_hex).ok())
                .is_some_and(|bytes| {
                    parse_command_interface_blob(&bytes, command_refs, metadata_refs).is_some()
                });
            if !parsed {
                misses.insert(
                    body_id,
                    "configuration_command_interface_decoder_failed".to_string(),
                );
            }
        }
    }
    for form_uuid in form_refs.keys() {
        let help_id = format!("{form_uuid}.1");
        if !file_names.contains(help_id.as_str()) {
            continue;
        }
        let parsed = rows_by_file_name
            .get(help_id.as_str())
            .and_then(|row| decode_hex(&row.binary_hex).ok())
            .is_some_and(|bytes| parse_help_blob_pages(&bytes).is_some());
        if !parsed {
            misses.insert(help_id, "form_help_decoder_failed".to_string());
        }
    }
    for (uuid, template_ref) in template_refs {
        let body_id = format!("{uuid}.0");
        if file_names.contains(body_id.as_str())
            && template_body_source_asset(template_ref.template_type).is_none()
        {
            misses.insert(
                body_id,
                "template_source_asset_type_unclassified".to_string(),
            );
        }
    }
    misses
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
        "GraphicalSchema" => Some(("Template.xml", SourceAssetKind::TemplateGraphicalScheme)),
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
                kind: SourceAssetKind::Form {
                    owner_reference: form_owner_reference_name(form_ref),
                },
            },
        );
    }

    paths
}

fn configuration_source_asset_kind(
    role: crate::compiler::families::assets::SourceAssetRole,
) -> Option<SourceAssetKind> {
    use crate::compiler::families::assets::SourceAssetRole;
    match role {
        SourceAssetRole::Splash | SourceAssetRole::MainSectionPicture => {
            Some(SourceAssetKind::ExtPicture)
        }
        SourceAssetRole::Help => Some(SourceAssetKind::Help),
        SourceAssetRole::ParentConfigurations | SourceAssetRole::MobileClientSignature => {
            Some(SourceAssetKind::InflatedBinary)
        }
        SourceAssetRole::HomePageWorkArea => Some(SourceAssetKind::HomePageWorkArea),
        SourceAssetRole::ClientApplicationInterface => {
            Some(SourceAssetKind::ClientApplicationInterface)
        }
        _ => None,
    }
}

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
        .map(|discovery| discovery.assets)
        .unwrap_or_default()
}

#[allow(dead_code)]
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
    .map(|discovery| discovery.assets)
    .unwrap_or_default()
}

pub(super) struct SourceAssetDiscovery {
    pub(super) assets: Vec<(String, SourceAsset)>,
    pub(super) misses: BTreeMap<String, String>,
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
) -> Option<SourceAssetDiscovery> {
    let uuid = row.file_name.as_str();
    let kind = row.kind.as_deref()?;
    let folder = row.folder?;
    let header = row.header.as_ref()?;
    let owner_family = FamilyId::parse(kind).ok()?;
    let registry = crate::compiler::families::assets::SourceAssetRegistry;
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
    let mut misses = BTreeMap::new();

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
                    kind: SourceAssetKind::AdditionalIndexes {
                        owner: AdditionalIndexesOwner {
                            kind: kind.to_string(),
                            name: header.name.clone(),
                            uuid: uuid.to_string(),
                        },
                    },
                },
            ));
        }
    }

    if let Some(route) = registry.owner_bound_route(&owner_family) {
        let body_id = format!("{uuid}{}", route.suffix());
        if file_names.contains(body_id.as_str()) {
            let row_id = ConfigRowId::parse(&body_id).ok()?;
            let route = registry
                .owner_bound_relation(&owner_family, row_id.suffix())
                .ok()?;
            let relation_result = rows_by_file_name
                .get(body_id.as_str())
                .and_then(|row| decode_hex(&row.binary_hex).ok())
                .map(|bytes| {
                    owner_bound_source_asset(
                        route,
                        &object_path,
                        &header.name,
                        object_refs.get(uuid).map(String::as_str),
                        &bytes,
                        object_refs,
                        field_refs,
                    )
                })
                .unwrap_or(Err(OwnerBoundSourceAssetFailure::Decoder));
            match relation_result {
                Ok(asset) => assets.push((body_id, asset)),
                Err(OwnerBoundSourceAssetFailure::Decoder) => {
                    let reason = owner_bound_decoder_miss(route.role())?;
                    misses.insert(body_id, reason.to_owned());
                }
                Err(OwnerBoundSourceAssetFailure::Empty) => {
                    misses.insert(body_id, ACCUMULATION_REGISTER_AGGREGATES_EMPTY.to_owned());
                }
                Err(OwnerBoundSourceAssetFailure::Relation(_)) => {}
            }
        }
    }

    let body_id = format!("{uuid}.0");
    if file_names.contains(body_id.as_str()) {
        let asset = match kind {
            "CommonPicture" => crate::compiler::families::assets::SourceAssetRegistry
                .route(
                    "CommonPicture",
                    crate::compiler::families::assets::SourceAssetRole::Picture,
                )
                .map(|route| SourceAsset {
                    primary_path: object_path.join(route.relative_path()),
                    kind: SourceAssetKind::ExtPicture,
                }),
            "ScheduledJob" => Some(SourceAsset {
                primary_path: object_path.join("Ext").join("Schedule.xml"),
                kind: SourceAssetKind::Schedule,
            }),
            "XDTOPackage" => crate::compiler::families::assets::SourceAssetRegistry
                .route(
                    "XDTOPackage",
                    crate::compiler::families::assets::SourceAssetRole::Package,
                )
                .map(|route| SourceAsset {
                    primary_path: object_path.join(route.relative_path()),
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
            _ => None,
        };
        if let Some(asset) = asset {
            assets.push((body_id, asset));
        }
    }

    let command_mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .chain(misses.keys().cloned())
        .collect::<BTreeSet<_>>();
    for suffix in ["0", "1"] {
        let body_id = format!("{uuid}.{suffix}");
        if command_mapped_ids.contains(&body_id) {
            continue;
        }
        let is_command_relation = crate::compiler::families::assets::SourceAssetRegistry
            .route_by_suffix(kind, suffix)
            .is_some_and(|route| {
                route.role() == crate::compiler::families::assets::SourceAssetRole::CommandInterface
            });
        if !is_command_relation || !file_names.contains(body_id.as_str()) {
            continue;
        }
        let decoded = rows_by_file_name
            .get(body_id.as_str())
            .and_then(|row| decode_hex(&row.binary_hex).ok())
            .and_then(|bytes| parse_command_interface_blob(&bytes, command_refs, metadata_refs));
        match decoded {
            // Mirrors the identical `CommandInterface::is_empty` check in
            // `source_asset_paths_with_indexes`'s Configuration-root loop: a
            // decoded-but-entirely-empty command interface is a record the
            // platform tracks by identity without ever rendering a file for.
            // Six ERP UH nested subsystems (e.g.
            // `Subsystems/ГосИС/Subsystems/ЗЕРНО`) decode their own
            // `.0`/`.1` command interface to zero sections and stay absent
            // from the native export tree.
            Some(interface) if !interface.is_empty() => {
                let route = crate::compiler::families::assets::SourceAssetRegistry
                    .route_by_suffix(kind, suffix)?;
                assets.push((
                    body_id,
                    SourceAsset {
                        primary_path: object_path.join(route.relative_path()),
                        kind: SourceAssetKind::CommandInterface,
                    },
                ));
            }
            Some(_) => {
                misses.insert(body_id, "command_interface_empty".to_string());
            }
            None => {
                misses.insert(body_id, "command_interface_decoder_failed".to_string());
            }
        }
    }

    let preferred_help_body_id = preferred_help_body_id(kind, uuid);
    let mapped_ids = assets
        .iter()
        .map(|(body_id, _)| body_id.clone())
        .chain(misses.keys().cloned())
        .collect::<BTreeSet<_>>();
    // `rows_by_file_name` is keyed by `"<owner-uuid>.<suffix>"`, so every row
    // owned by this object lives in the exact lexicographic range
    // `[uuid+".", uuid+"/")` -- `.` (0x2E) and `/` (0x2F) are adjacent bytes,
    // so this range is tight: it holds precisely the keys whose owner prefix
    // equals `uuid`, nothing more and nothing less. Scanning that range
    // instead of the whole corpus turns an O(rows-per-object) owner lookup
    // into O(log rows), which matters here because this function runs once
    // per metadata object -- a full scan per object made the caller
    // quadratic in corpus size. The `row_id.owner() != uuid` check below is
    // kept as a defensive no-op so behavior is unchanged even if that
    // assumption is ever wrong for some key.
    let owner_range_start = format!("{uuid}.");
    let owner_range_end = format!("{uuid}/");
    for (body_id, body_row) in
        rows_by_file_name.range(owner_range_start.as_str()..owner_range_end.as_str())
    {
        let Ok(row_id) = ConfigRowId::parse(body_id) else {
            continue;
        };
        if row_id.owner() != uuid || mapped_ids.contains(*body_id) {
            continue;
        }
        let is_preferred_help = *body_id == preferred_help_body_id;
        let help_parsed = decode_hex(&body_row.binary_hex)
            .ok()
            .is_some_and(|help_bytes| parse_help_blob_pages(&help_bytes).is_some());
        if help_parsed {
            if rows_by_file_name.contains_key(preferred_help_body_id.as_str())
                && *body_id != preferred_help_body_id
            {
                continue;
            }
            assets.push((
                (*body_id).to_string(),
                SourceAsset {
                    primary_path: object_path.join(
                        crate::compiler::families::assets::SourceAssetRegistry.help_relative_path(),
                    ),
                    kind: SourceAssetKind::Help,
                },
            ));
            continue;
        }
        if let Some(model) = predefined_data_source_model(kind)
            && Some(row_id.suffix_component()) == predefined_data_suffix(kind)
        {
            let decoded = decode_hex(&body_row.binary_hex)
                .ok()
                .and_then(|bytes| parse_predefined_data_blob_with_model(&bytes, type_index, model));
            match decoded {
                Some(items) if !items.is_empty() => {
                    let route = predefined_data_route(kind)?;
                    assets.push((
                        (*body_id).to_string(),
                        SourceAsset {
                            primary_path: object_path.join(route.relative_path()),
                            kind: SourceAssetKind::PredefinedData { model },
                        },
                    ));
                }
                // Decoded cleanly to zero predefined items. The platform
                // does not write `Ext/Predefined.xml` for these -- confirmed
                // against 12 ERP UH catalogs/charts whose predefined-items
                // row decodes to an empty set and whose native tree has no
                // `Predefined.xml` at all. Recorded under its own reason,
                // distinct from a genuine decode failure.
                Some(_) => {
                    misses.insert((*body_id).to_string(), "predefined_data_empty".to_string());
                }
                None => {
                    misses.insert(
                        (*body_id).to_string(),
                        "predefined_data_decoder_failed".to_string(),
                    );
                }
            }
        } else if is_preferred_help {
            misses.insert((*body_id).to_string(), "help_decoder_failed".to_string());
        }
    }

    Some(SourceAssetDiscovery { assets, misses })
}

pub(super) fn additional_indexes_body_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "Document" => Some("3"),
        "AccumulationRegister" => Some("4"),
        _ => None,
    }
}

pub(super) fn preferred_help_body_id(kind: &str, uuid: &str) -> String {
    let suffix = crate::compiler::families::assets::SourceAssetRegistry
        .help_suffix(kind)
        .expect("source-asset registry defines the help suffix policy")
        .trim_start_matches('.');
    format!("{uuid}.{suffix}")
}

pub(super) fn write_source_xml_file(
    path: &Path,
    xml: impl AsRef<[u8]>,
    source_version: InfobaseConfigSourceVersion,
) -> Result<()> {
    let adapter = MssqlLegacyAdapter::from_legacy_selector(source_version);
    let normalized =
        normalize_legacy_source_asset_xml_version_bytes(xml.as_ref(), adapter.xml_dialect());
    fs::write(path, normalized).with_context(|| format!("failed to write {}", path.display()))
}

/// Preserves historical MSSQL source-asset output behavior only.
///
/// Replacing a root `version` attribute is not a dialect migration. Keeping
/// this helper inside the legacy source-assets module prevents the future XCF
/// adapter from treating the compatibility rewrite as a general conversion.
pub(super) fn normalize_legacy_source_asset_xml_version_bytes(
    bytes: &[u8],
    xml_dialect: &ibcmd_core::version::XmlDialect,
) -> Vec<u8> {
    let from = match xml_dialect.to_string().as_str() {
        "2.20" => "version=\"2.21\"",
        "2.21" => "version=\"2.20\"",
        _ => return bytes.to_vec(),
    };
    let to = format!("version=\"{xml_dialect}\"");
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    if text.contains(from) {
        text.replace(from, &to).into_bytes()
    } else {
        bytes.to_vec()
    }
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
) -> Result<WrittenSourceAsset> {
    let output_dir = context.output_dir;
    let mut diagnostics = Vec::new();
    let mut opaque_not_emitted = false;
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
        SourceAssetKind::Form { owner_reference } => {
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
            let adapter = MssqlLegacyAdapter::from_legacy_selector(context.source_version);
            let dcs_target_profile =
                ProfileId::parse(&format!("xml-{}", context.source_version.as_str()))
                    .expect("legacy source-version profiles are valid");
            let form_context = FormParseContext::new(
                context.type_index,
                context.type_index_collisions,
                context.dcs_type_index,
                context.form_object_refs,
                context.field_type_refs,
                context.information_register_field_refs,
                context.information_register_master_dimensions,
                owner_reference.as_deref(),
            )
            .with_form_reference_index(context.role_rights_object_refs)
            .with_metadata_command_refs(context.metadata_refs)
            .with_dcs_profiles(adapter.provider_id().clone(), dcs_target_profile);
            let extraction =
                extract_form_body_xml_from_body_detailed_timed(body, &form_context, Some(timings))
                    .with_context(|| {
                        format!(
                            "failed to extract form xml from source asset {}",
                            asset.primary_path.display()
                        )
                    })?;
            match extraction {
                DetailedFormBodyExtraction::Emitted {
                    xml,
                    diagnostics: extraction_diagnostics,
                } => {
                    diagnostics = extraction_diagnostics;
                    let path = output_dir.join(&asset.primary_path);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("failed to create {}", parent.display()))?;
                    }
                    write_source_xml_file(&path, xml, context.source_version)?;

                    let form_items_started = Instant::now();
                    for item_asset in extract_form_item_assets(bytes) {
                        let item_path = output_dir
                            .join(asset.primary_path.with_extension(""))
                            .join("Items")
                            .join(sanitize_source_path_segment(&item_asset.item_name))
                            .join(&item_asset.file_name);
                        if let Some(parent) = item_path.parent() {
                            fs::create_dir_all(parent).with_context(|| {
                                format!("failed to create {}", parent.display())
                            })?;
                        }
                        fs::write(&item_path, &item_asset.content)
                            .with_context(|| format!("failed to write {}", item_path.display()))?;
                    }
                    timings.source_asset_form_items_cpu_ms += elapsed_ms(form_items_started);
                }
                DetailedFormBodyExtraction::OpaqueNotEmitted {
                    diagnostics: extraction_diagnostics,
                } => {
                    debug_assert!(!extraction_diagnostics.is_empty());
                    diagnostics = extraction_diagnostics;
                    opaque_not_emitted = true;
                }
                DetailedFormBodyExtraction::Rejected {
                    diagnostics: extraction_diagnostics,
                    error,
                } => {
                    if context.collect_all_source_asset_diagnostics
                        && !extraction_diagnostics.is_empty()
                    {
                        return Ok(WrittenSourceAsset::RejectedNotEmitted {
                            primary_path: asset.primary_path.clone(),
                            diagnostics: extraction_diagnostics,
                        });
                    }
                    let diagnostic_codes = extraction_diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.code)
                        .collect::<Vec<_>>();
                    bail!(
                        "form source asset {} was rejected: {error:?}; diagnostics={diagnostic_codes:?}",
                        asset.primary_path.display()
                    );
                }
            }
            timings.source_asset_form_xml_cpu_ms += elapsed_ms(form_xml_started);
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
            let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
                crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
                bytes,
            )
            .with_context(|| {
                format!(
                    "failed to decode data-composition source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let adapter = MssqlLegacyAdapter::from_legacy_selector(context.source_version);
            let target_profile =
                ProfileId::parse(&format!("xml-{}", context.source_version.as_str()))
                    .expect("legacy source-version profiles are valid");
            let content = match body.layout() {
                crate::compiler::bodies::dcs::DcsBodyLayout::NativeThreeDocument => {
                    let documents = body.documents();
                    crate::mssql_dump::dcs::normalize_data_composition_schema_template_documents_with_profiles(
                        &documents,
                        context.dcs_type_index,
                        context.object_refs,
                        adapter.provider_id(),
                        &target_profile,
                    )
                    // The typed step-level reason travels out as this error's
                    // source, so `{error:#}` in the failed-row ledger names the
                    // stage that rejected the template instead of reporting a
                    // bare "failed to normalize".
                    .with_context(|| {
                        format!(
                            "failed to normalize native data-composition source asset {}",
                            asset.primary_path.display()
                        )
                    })?
                }
                crate::compiler::bodies::dcs::DcsBodyLayout::DirectXml => body.plaintext().to_vec(),
            };
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
        SourceAssetKind::AdditionalIndexes { owner } => {
            let inflated = inflate_raw_deflate(bytes).with_context(|| {
                format!(
                    "failed to inflate source asset {}",
                    asset.primary_path.display()
                )
            })?;
            let indexes = super::additional_indexes::parse_additional_indexes(
                &inflated,
                owner,
                context.object_refs,
            )
            .with_context(|| {
                format!(
                    "failed to extract additional indexes from source asset {}",
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
                super::additional_indexes::format_additional_indexes_xml(&indexes),
                context.source_version,
            )?;
        }
        SourceAssetKind::BusinessProcessFlowchart => {
            let flowchart = parse_business_process_flowchart_blob(
                bytes,
                context.object_refs,
                context.metadata_object_refs,
                context.type_index,
                context.type_index_collisions,
            )
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
            write_graphical_scheme_pictures(&path, &flowchart)?;
        }
        SourceAssetKind::TemplateGraphicalScheme => {
            // A standalone `GraphicalSchema` Template body comes in one of
            // two representations: some are already pre-serialized XML
            // after raw-deflate (the `8.3/xcf/scheme` marker `refs::
            // infer_template_type_from_body` checks first), which just
            // needs the same passthrough `InflatedBinary` uses. Others are
            // the platform's brace-tuple grammar -- the exact same one
            // `BusinessProcess.Flowchart`'s `Ext/Flowchart.xml` decodes via
            // `parse_business_process_flowchart_blob` above; see `mod::
            // flowchart_grammar_fields`'s doc comment for how the two
            // classes were told apart on real ERP UH bytes. There is no
            // third option: a body that is neither is a typed failure here,
            // not a silent default -- the defect this asset kind exists to
            // fix (`output-path-collisions-and-module-text-fallback-
            // 20260825.md` section 4) was exactly a silent default in the
            // other direction (misclassified as `TextDocument`).
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
            let text = std::str::from_utf8(&inflated)
                .ok()
                .map(|text| text.trim_start_matches('\u{feff}').trim_start())
                .filter(|text| looks_like_graphical_scheme_blob_text(text));
            if let Some(text) = text {
                let flowchart = parse_business_process_flowchart_text_with_types(
                    text,
                    context.object_refs,
                    context.metadata_object_refs,
                    context.type_index,
                    context.type_index_collisions,
                )
                .with_context(|| {
                    format!(
                        "failed to extract graphical scheme from source asset {}",
                        asset.primary_path.display()
                    )
                })?;
                write_source_xml_file(
                    &path,
                    format_business_process_flowchart_xml(&flowchart),
                    context.source_version,
                )?;
                write_graphical_scheme_pictures(&path, &flowchart)?;
            } else {
                write_source_xml_file(&path, inflated, context.source_version)?;
            }
        }
        SourceAssetKind::MoxelSpreadsheet => {
            let xml = extract_moxel_source_asset_xml(
                bytes,
                context.object_refs,
                context.moxel_generated_types,
                &asset.primary_path,
            )?;
            let path = output_dir.join(&asset.primary_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            write_source_xml_file(&path, xml, context.source_version)?;
        }
    }

    if opaque_not_emitted {
        Ok(WrittenSourceAsset::OpaqueNotEmitted {
            primary_path: asset.primary_path.clone(),
            diagnostics,
        })
    } else {
        Ok(WrittenSourceAsset::Emitted {
            primary_path: asset.primary_path.clone(),
            diagnostics,
        })
    }
}

/// Publishes the pictures a graphical scheme carries inline. The platform
/// puts each beside the scheme file, under the scheme's own stem:
/// `<stem>/Items/<item name>/Picture.<ext>`. Evidence: ERP УХ
/// `DataProcessors/ВыполнениеМаршрутныхЛистов/Templates/МетодикаББВ/Ext/
/// Template/Items/Декорация11/Picture.png` and the 69 other inline pictures
/// of that corpus.
fn write_graphical_scheme_pictures(
    scheme_path: &Path,
    flowchart: &BusinessProcessFlowchart,
) -> Result<()> {
    let pictures = flowchart.picture_files();
    if pictures.is_empty() {
        return Ok(());
    }
    let stem = scheme_path.with_extension("");
    for (item_name, file_name, data) in pictures {
        if item_name.is_empty() || item_name.contains(['/', '\\']) {
            bail!(
                "graphical scheme item name {item_name:?} cannot name a picture directory beside {}",
                scheme_path.display()
            );
        }
        let directory = stem.join("Items").join(&item_name);
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
        let file = directory.join(&file_name);
        fs::write(&file, &data).with_context(|| format!("failed to write {}", file.display()))?;
    }
    Ok(())
}

fn extract_moxel_source_asset_xml(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    generated_types: &BTreeMap<String, String>,
    source_path: &Path,
) -> Result<String> {
    try_extract_moxel_spreadsheet_xml_with_generated_types(bytes, object_refs, generated_types)
        .with_context(|| {
            format!(
                "failed to extract spreadsheet template from source asset {}",
                source_path.display()
            )
        })
}

#[cfg(test)]
mod mxl_source_asset_tests {
    use super::*;

    #[test]
    fn typed_mxl_diagnostic_survives_source_asset_context() {
        let error = extract_moxel_source_asset_xml(
            &[0],
            &BTreeMap::new(),
            &BTreeMap::new(),
            Path::new("Templates/Example/Ext/Template.xml"),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to extract spreadsheet template from source asset")
        );
        let diagnostic = error
            .chain()
            .find_map(|source| source.downcast_ref::<MxlDiagnostic>())
            .expect("typed MXL diagnostic must remain in the anyhow error chain");
        assert_eq!(diagnostic.stage(), MxlDiagnosticStage::Decoder);
        assert_eq!(diagnostic.code(), "mxl.decoder.binary-container");
    }
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
    pub(super) code: PredefinedItemCode,
    pub(super) description: String,
    pub(super) data: PredefinedItemData,
    pub(super) children: Vec<PredefinedItem>,
}

#[derive(Clone)]
pub(super) enum PredefinedItemCode {
    Text(String),
    Decimal(String),
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
    let text = rewrite_help_attachment_folder_refs(&text);
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
        // A stored help link may address an anchor inside the target page
        // (`../id<uuid>/<page>#<anchor>`). The platform keeps that fragment on
        // the rewritten `<Reference>/Help` link; only the storage path in front
        // of it is replaced.
        if let Some(relative_fragment) = text[uuid_end..quote_end].find('#') {
            output.push_str(&text[uuid_end + relative_fragment..quote_end]);
        }
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

/// Help and HTML-document attachments are exported into a single `_files`
/// directory beside the page, which this exporter already writes. Inside the
/// stored HTML the platform still addresses that directory through the storage
/// identifier of the owning document (`src="<uuid>_files/name.png"`); the
/// exported page addresses it relatively (`src="_files/name.png"`). Only an
/// attribute value that starts with a non-nil UUID immediately followed by
/// `_files/` is requalified; every other occurrence is copied through.
pub(super) fn rewrite_help_attachment_folder_refs(text: &str) -> String {
    const MARKER: &str = "_files/";
    const ATTRIBUTE_PREFIX: &str = "=\"";
    const UUID_LEN: usize = 36;
    const QUALIFIER_LEN: usize = ATTRIBUTE_PREFIX.len() + UUID_LEN;

    let mut output = String::with_capacity(text.len());
    let mut offset = 0usize;

    while let Some(relative_start) = text[offset..].find(MARKER) {
        let marker_start = offset + relative_start;
        let qualified = marker_start >= offset + QUALIFIER_LEN
            && text.get(marker_start - QUALIFIER_LEN..marker_start - UUID_LEN)
                == Some(ATTRIBUTE_PREFIX)
            && text
                .get(marker_start - UUID_LEN..marker_start)
                .and_then(parse_non_zero_uuid)
                .is_some();
        let copy_end = if qualified {
            marker_start - UUID_LEN
        } else {
            marker_start
        };
        output.push_str(&text[offset..copy_end]);
        output.push_str(MARKER);
        offset = marker_start + MARKER.len();
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
            xsi_type: "PlanOfCharacteristicKindPredefinedItems",
            root_tag: "1",
            rowset_layout: PredefinedDataRowsetLayout::NestedTable,
            unwrap_single_root: true,
            item_layout: PredefinedItemLayout::Characteristic,
        },
    ),
    (
        "ChartOfAccounts",
        PredefinedDataSourceModel {
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

fn predefined_data_route(
    kind: &str,
) -> Option<&'static crate::compiler::families::assets::SourceAssetRoute> {
    crate::compiler::families::assets::SourceAssetRegistry.route(
        kind,
        crate::compiler::families::assets::SourceAssetRole::Predefined,
    )
}

fn predefined_data_suffix(kind: &str) -> Option<&'static str> {
    predefined_data_route(kind)?.suffix().strip_prefix('.')
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
        ) && predefined_data_suffix(&owner.kind)
            .is_some_and(|suffix| file_names.contains(&format!("{owner_uuid}.{suffix}")))
    })
}

pub(super) fn predefined_data_body_file_names(
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
) -> BTreeSet<String> {
    body_owners
        .iter()
        .filter_map(|(owner_uuid, owner)| {
            predefined_data_source_model(&owner.kind)?;
            predefined_data_suffix(&owner.kind).map(|suffix| format!("{owner_uuid}.{suffix}"))
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
    let mut ambiguous_item_ids = BTreeSet::new();

    for (owner_uuid, owner) in body_owners {
        let Some(model) = predefined_data_source_model(&owner.kind) else {
            continue;
        };
        let Some(suffix) = predefined_data_suffix(&owner.kind) else {
            continue;
        };
        let file_name = format!("{owner_uuid}.{suffix}");
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
        insert_predefined_item_references(
            &mut index,
            &mut ambiguous_item_ids,
            owner_reference,
            &items,
        )?;
    }

    Ok(index)
}

/// Predefined-item references for every owner that stores predefined data.
///
/// [`build_predefined_item_reference_index`] is built for the owners whose own
/// XML needs the names; a form names a predefined item of *any* object, so this
/// one walks every owner that has a predefined-data body.  An owner the
/// reference index does not name, a body the blob reader does not read and a
/// collision inside one owner are all skipped rather than fatal: a form that
/// cannot resolve its reference keeps the raw identifiers the platform itself
/// keeps for an unresolvable one.
pub(super) fn build_form_predefined_item_reference_index(
    rows: &[ConfigRow],
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
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
        let Some(suffix) = predefined_data_suffix(&owner.kind) else {
            continue;
        };
        let Some(row) = rows_by_file_name.get(format!("{owner_uuid}.{suffix}").as_str()) else {
            continue;
        };
        let Ok(bytes) = decode_hex(&row.binary_hex) else {
            continue;
        };
        let Some(items) = parse_predefined_data_blob_with_model(&bytes, type_index, model) else {
            continue;
        };
        let Some(owner_reference) = object_refs.get(owner_uuid) else {
            continue;
        };
        let mut ambiguous_item_ids = BTreeSet::new();
        let mut owner_index = BTreeMap::new();
        if insert_predefined_item_references(
            &mut owner_index,
            &mut ambiguous_item_ids,
            owner_reference,
            &items,
        )
        .is_err()
        {
            continue;
        }
        for (key, reference) in owner_index {
            if metadata_owner_value_reference_key_parts(&key).is_some() {
                index.insert(key, reference);
            }
        }
    }
    index
}

pub(super) fn insert_predefined_item_references(
    index: &mut BTreeMap<String, String>,
    ambiguous_item_ids: &mut BTreeSet<String>,
    owner_reference: &str,
    items: &[PredefinedItem],
) -> Result<()> {
    for item in items {
        let reference = format!("{owner_reference}.{}", item.name);
        let qualified_key = metadata_owner_value_reference_key(owner_reference, &item.id);
        if let Some(previous) = index.insert(qualified_key, reference.clone())
            && previous != reference
        {
            bail!(
                "predefined item {} resolves to both {previous} and {reference} for owner {owner_reference}",
                item.id,
            );
        }
        if !ambiguous_item_ids.contains(&item.id) {
            if let Some(previous) = index.insert(item.id.clone(), reference.clone())
                && previous != reference
            {
                index.remove(&item.id);
                ambiguous_item_ids.insert(item.id.clone());
            }
        }
        insert_predefined_item_references(
            index,
            ambiguous_item_ids,
            owner_reference,
            &item.children,
        )?;
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
        PredefinedItemLayout::Generic | PredefinedItemLayout::Characteristic => {
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
    let code = parse_predefined_code_value(predefined_rowset_item_value(&fields, schema, 2)?)?;
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
    let code = parse_predefined_code_value(predefined_rowset_item_value(&fields, schema, 3)?)?;
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
        .and_then(|field| parse_predefined_code_value(field))
        .unwrap_or_else(|| PredefinedItemCode::Text(String::new()));
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

fn parse_predefined_code_value(value: &str) -> Option<PredefinedItemCode> {
    if let Some(value) = parse_predefined_string_value(value) {
        return Some(PredefinedItemCode::Text(value));
    }

    let fields = split_1c_braced_fields(value, 0)?;
    if fields.len() != 2 || fields.first()?.trim() != r#""N""# {
        return None;
    }
    let value = fields.get(1)?.trim();
    is_xml_schema_decimal(value).then(|| PredefinedItemCode::Decimal(value.to_string()))
}

fn is_xml_schema_decimal(value: &str) -> bool {
    let value = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let Some((integer, fraction)) = value.split_once('.') else {
        return !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    };
    (!integer.is_empty() || !fraction.is_empty())
        && integer.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod predefined_code_tests {
    use super::*;

    fn format_item_code(kind: &str, code: PredefinedItemCode) -> String {
        let data = match kind {
            "Catalog" => PredefinedItemData::Generic {
                value_types: Vec::new(),
                is_folder: false,
            },
            "ChartOfAccounts" => PredefinedItemData::Account {
                account_type: PredefinedAccountType::Active,
                off_balance: false,
                order: String::new(),
                accounting_flags: Vec::new(),
                ext_dimension_types: Vec::new(),
            },
            "ChartOfCalculationTypes" => PredefinedItemData::Calculation {
                action_period_is_base: false,
                displaced: Vec::new(),
                base: Vec::new(),
                leading: Vec::new(),
            },
            _ => unreachable!(),
        };
        let item = PredefinedItem {
            id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".to_string(),
            name: "Item".to_string(),
            code,
            description: String::new(),
            data,
            children: Vec::new(),
        };
        format_predefined_data_xml(
            predefined_data_source_model(kind).unwrap(),
            &[item],
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap()
    }

    #[test]
    fn numeric_catalog_code_is_emitted_as_xs_decimal() {
        let type_uuid = "ae135932-4f94-44df-92c1-c91f15a92848";
        let item_uuid = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        let value = format!(
            "{{2,1,7,{{\"#\",{type_uuid},{{1,{item_uuid}}}}},{{\"B\",0}},\
             {{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},\
             {{\"S\",\"Item\"}},{{\"N\",17}},{{\"S\",\"Description\"}},{{\"N\",0}},0}}"
        );
        let item = parse_predefined_item(&value, &BTreeMap::new()).unwrap();
        let xml = format_predefined_data_xml(
            predefined_data_source_model("Catalog").unwrap(),
            &[item],
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();

        assert!(xml.contains(r#"<Code xsi:type="xs:decimal">17</Code>"#));
    }

    #[test]
    fn all_predefined_layouts_preserve_numeric_and_text_code_representation() {
        for kind in ["Catalog", "ChartOfAccounts", "ChartOfCalculationTypes"] {
            let numeric =
                format_item_code(kind, parse_predefined_code_value(r#"{"N",0}"#).unwrap());
            assert!(
                numeric.contains(r#"<Code xsi:type="xs:decimal">0</Code>"#),
                "{kind}: {numeric}"
            );

            let text =
                format_item_code(kind, parse_predefined_code_value(r#"{"S","A01"}"#).unwrap());
            assert!(text.contains("<Code>A01</Code>"), "{kind}: {text}");
            assert!(!text.contains(r#"<Code xsi:type="#), "{kind}: {text}");
        }
    }

    #[test]
    fn rejects_non_decimal_numeric_catalog_code() {
        for value in [
            r#"{"N",""}"#,
            r#"{"N",1e2}"#,
            r#"{"N",1.2.3}"#,
            r#"{"N",1,0}"#,
        ] {
            assert!(parse_predefined_code_value(value).is_none(), "{value}");
        }
    }
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
    xml.push_str("</PredefinedData>");
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
    match &item.code {
        PredefinedItemCode::Text(value) => {
            push_predefined_text_element(xml, &tab, "Code", value);
        }
        PredefinedItemCode::Decimal(value) => {
            xml.push_str(&format!(
                "{tab}\t<Code xsi:type=\"xs:decimal\">{}</Code>\r\n",
                escape_xml_element_text(value)
            ));
        }
    }
    push_predefined_text_element(xml, &tab, "Description", &item.description);

    match (&item.data, layout) {
        (
            PredefinedItemData::Generic {
                value_types,
                is_folder,
            },
            PredefinedItemLayout::Generic | PredefinedItemLayout::Characteristic,
        ) => {
            let type_xml = format_predefined_type_xml(value_types, indent + 1);
            if type_xml.is_empty() && matches!(layout, PredefinedItemLayout::Characteristic) {
                xml.push_str(&format!("{tab}\t<Type/>\r\n"));
            } else {
                xml.push_str(&type_xml);
            }
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
    // The current-config prefix is numbered by the absolute element depth of the
    // `v8:Type` element it sits on, not by a constant: `PredefinedData` is 1, the
    // outermost `Item` is 2, its `Type` is 3 and the `v8:Type` inside it is 4,
    // and every `ChildItems`/`Item` pair below adds 2. All 466 occurrences in
    // 1C:УТ 11.5.27.75 follow it - 367 at depth 4, 68 at 6, 29 at 8 and 2 at 10.
    let prefix = format!("d{}p1", indent + 2);
    let mut xml = format!("{tab}<Type>\r\n");
    for value_type in value_types {
        match value_type {
            ConstantValueType::Reference { reference } if reference.starts_with("cfg:") => {
                xml.push_str(&format!(
                    "{tab}\t<v8:Type xmlns:{prefix}=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">{prefix}:{}</v8:Type>\r\n",
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
