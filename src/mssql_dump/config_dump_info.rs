use super::*;
use uuid::Uuid;

const CONFIG_DUMP_INFO_FILE_NAME: &str = "ConfigDumpInfo.xml";
/// Service names the streamed MSSQL `Config` table's `versions` row embeds
/// inside its own blob content, and which are therefore stripped from the
/// parsed entry list.
const VERSIONS_EMBEDDED_SERVICE_NAMES: [&str; 3] = ["root", "version", "versions"];

/// Storage records that stand beside the per-object ones and carry no
/// metadata object of their own, so the `versions` inventory does not list
/// them and the export writes no file for them.
///
/// `deleted` is the fourth, and it appears only in a .cf the platform itself
/// saved: `ibcmd infobase config save` writes it (4 unpacked bytes, content
/// `0`), while none of the five vendor distributions on hand carries it at
/// all -- УТ 11.5.27.75, БСП демо 3.1.12.297, ERP УХ 3.2.12.6 and both ERP УХ
/// mini-configurations have zero elements by that name. The platform's own
/// export of such an infobase writes no `deleted` file either. Counting it as
/// an object with no version entry failed the whole export of every .cf the
/// platform saves -- which is every .cf a purpose-built seed configuration
/// can produce.
const MANIFEST_SERVICE_NAMES: [&str; 3] = ["root", "version", "versions"];

/// A service record that is present in some images and absent in others, so it
/// is taken *out of* the comparison rather than required by it -- the same
/// treatment [`is_dynamic_update_entry`] gets, and for the same reason.
///
/// `ibcmd infobase config save` writes it (4 unpacked bytes, content `0`);
/// none of the five vendor distributions on hand carries it at all -- УТ
/// 11.5.27.75, БСП демо 3.1.12.297, ERP УХ 3.2.12.6 and both ERP УХ
/// mini-configurations have zero elements by that name -- and the platform's
/// own export writes no file for it either way. Requiring it would fail every
/// vendor .cf; counting it as an object with no version entry failed every
/// .cf the platform saves, and so every purpose-built seed configuration.
const OPTIONAL_SERVICE_NAME: &str = "deleted";

/// The Configuration object's own metadata text always embeds a `{1,0,...}`
/// header reference to its (thick-client) `CommandInterface` sub-object at
/// this fixed, well-known uuid — confirmed identical across three
/// independent native-evidence corpora (T1/T2/T3,
/// `scratchpad/evidence-batch/session12`), each reusing the owning
/// Configuration's own display name as the embedded header's name. Unlike
/// `ClientApplicationInterface` (embedded at `{uuid}.b`, which *does* get
/// its own row and canonical reference when customized), an uncustomized
/// `CommandInterface` never gets a row of its own, so it is unresolvable via
/// `object_refs` — and it never appears as a `<Metadata>` entry in any of
/// the three corpora's native ConfigDumpInfo.xml. Skipping it here mirrors
/// that observed, evidence-backed native behavior exactly, the same way
/// `configuration_module_groups` already skips other well-known aggregate
/// references that don't correspond to their own exportable child object.
const CONFIGURATION_COMMAND_INTERFACE_UUID: &str = "00000000-0000-0000-0000-000000000002";

/// Where the raw `versions` row/record came from, since its *content* shape
/// differs by source even though `validate_versions_inventory`'s separate
/// manifest-membership check (are `root`/`version`/`versions` present as
/// their own rows/records alongside the per-object ids?) holds for both.
///
/// The streamed MSSQL `Config` table's `versions` row embeds `root`,
/// `version`, and `versions` as extra name/uuid pairs *inside* its own
/// blob content, in addition to those three existing as separate rows.
/// A CF archive's `versions` storage element does not: confirmed by
/// decoding the `versions` element's content against three independent
/// native-evidence corpora (T1/T2/T3, `scratchpad/evidence-batch/session12`)
/// — each has `root`/`version`/`versions` as their own top-level storage
/// records (verified via `cf inspect`), but the `versions` element's own
/// parsed pairs are exclusively per-object uuid keys, never those three
/// names. Requiring them unconditionally would fail-closed a completely
/// well-formed CF export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VersionsBlobOrigin {
    MssqlConfigTable,
    CfStorageImage,
}

impl VersionsBlobOrigin {
    const fn embeds_service_entries(self) -> bool {
        matches!(self, Self::MssqlConfigTable)
    }
}

struct ConfigVersionEntry {
    id: String,
    version: Uuid,
}

struct ConfigDumpMetadata {
    name: String,
    id: String,
    config_version: String,
    children: Vec<ConfigDumpChildMetadata>,
}

struct ConfigDumpChildMetadata {
    name: String,
    id: String,
}

pub(super) struct ConfigDumpInfoInventory<'a> {
    pub(super) file_names: &'a BTreeSet<String>,
    pub(super) metadata_texts: &'a [MetadataTextRow],
    pub(super) object_refs: &'a BTreeMap<String, String>,
    pub(super) form_refs: &'a BTreeMap<String, FormSourceReference>,
    pub(super) template_refs: &'a BTreeMap<String, TemplateSourceReference>,
    pub(super) subsystem_refs: &'a BTreeMap<String, SubsystemSourceReference>,
    pub(super) module_text_paths: &'a BTreeMap<String, PathBuf>,
    pub(super) source_assets: &'a BTreeMap<String, SourceAsset>,
    pub(super) emitted_source_asset_paths: &'a BTreeMap<String, PathBuf>,
    pub(super) configuration_module_groups: &'a BTreeSet<String>,
}

/// How [`write_config_dump_info`] responds when the decoded inventory cannot
/// canonically route every versioned entry.
///
/// A full MSSQL `Config`-table dump decodes every metadata text, so a route
/// that fails to resolve there is an internal inconsistency and must fail
/// the export ([`Self::Fail`]). A CF storage image, by contrast, is allowed
/// to be only partially *recognized* by this exporter -- unrecognized
/// records are already disclosed as `opaque` in the export report, and every
/// unroutable entry stems from exactly such a record -- so a
/// ConfigDumpInfo.xml that cannot be complete is skipped rather than
/// half-written or turned into a whole-export failure ([`Self::Skip`]).
/// Structural corruption (an undecodable/mismatched `versions` blob,
/// duplicate metadata names) stays a hard error under both policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigDumpInfoPartialInventoryPolicy {
    Fail,
    Skip,
}

/// Returns whether `ConfigDumpInfo.xml` was written (`false` = skipped under
/// [`ConfigDumpInfoPartialInventoryPolicy::Skip`]).
pub(super) fn write_config_dump_info(
    output_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
    versions_blob: &[u8],
    versions_blob_origin: VersionsBlobOrigin,
    partial_inventory_policy: ConfigDumpInfoPartialInventoryPolicy,
    inventory: ConfigDumpInfoInventory<'_>,
) -> Result<bool> {
    let versions = parse_versions_blob(versions_blob, versions_blob_origin)?;
    validate_versions_inventory(&versions, inventory.file_names)?;

    let mut canonical_refs = inventory.object_refs.clone();
    for (id, form_ref) in inventory.form_refs {
        let name = form_source_reference_name(form_ref)
            .ok_or_else(|| anyhow!("form {id} has no canonical metadata reference"))?;
        canonical_refs.insert(id.clone(), name);
    }
    for (id, template_ref) in inventory.template_refs {
        let name = template_source_reference_name(template_ref)
            .ok_or_else(|| anyhow!("template {id} has no canonical metadata reference"))?;
        canonical_refs.insert(id.clone(), name);
    }
    for (id, subsystem_ref) in inventory.subsystem_refs {
        let name = subsystem_source_reference_name(subsystem_ref)
            .ok_or_else(|| anyhow!("subsystem {id} has no canonical metadata reference"))?;
        canonical_refs.insert(id.clone(), name);
    }
    for row in inventory.metadata_texts {
        if row.object_code == Some(0)
            && is_defined_type_metadata_text(&row.text, &row.file_name)
            && let Some(header) = row.header.as_ref()
        {
            canonical_refs.insert(
                row.file_name.clone(),
                format!("DefinedType.{}", header.name),
            );
        }
    }
    add_configuration_group_references(
        &mut canonical_refs,
        inventory.metadata_texts,
        inventory.configuration_module_groups,
    )?;
    if !canonical_refs.contains_key(CONFIGURATION_COMMAND_INTERFACE_UUID)
        && let Some(configuration_reference) =
            configuration_top_level_reference(inventory.object_refs)
    {
        canonical_refs.insert(
            CONFIGURATION_COMMAND_INTERFACE_UUID.to_owned(),
            configuration_reference.to_owned(),
        );
    }
    add_configuration_root_command_interface_references(
        &mut canonical_refs,
        inventory.metadata_texts,
        inventory.object_refs,
    );

    let version_ids = versions
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let Some(mut children_by_owner) = build_config_dump_children(
        inventory.metadata_texts,
        inventory.object_refs,
        &canonical_refs,
        &version_ids,
        inventory.configuration_module_groups,
        partial_inventory_policy,
    )?
    else {
        return Ok(false);
    };

    let mut metadata = Vec::with_capacity(versions.len());
    let mut names = BTreeMap::<String, String>::new();
    let mut unresolved_top_routes = Vec::<String>::new();
    for entry in versions {
        let name = match config_dump_top_name(
            &entry.id,
            &canonical_refs,
            inventory.module_text_paths,
            inventory.source_assets,
            inventory.emitted_source_asset_paths,
        ) {
            Ok(name) => name,
            Err(error) => {
                unresolved_top_routes.push(format!("{}: {error}", entry.id));
                continue;
            }
        };
        if let Some(previous_id) = names.insert(name.clone(), entry.id.clone()) {
            bail!(
                "ConfigDumpInfo metadata name {name} is produced by both {previous_id} and {}",
                entry.id
            );
        }
        let children = children_by_owner
            .remove(&entry.id)
            .unwrap_or_default()
            .into_iter()
            .map(|(id, name)| ConfigDumpChildMetadata { name, id })
            .collect();
        metadata.push(ConfigDumpMetadata {
            name,
            id: entry.id,
            config_version: config_version(entry.version),
            children,
        });
    }
    if !unresolved_top_routes.is_empty() {
        if partial_inventory_policy == ConfigDumpInfoPartialInventoryPolicy::Skip {
            return Ok(false);
        }
        let unresolved = unresolved_top_routes
            .iter()
            .take(64)
            .cloned()
            .collect::<Vec<_>>();
        bail!(
            "ConfigDumpInfo has {} entries without canonical routes [{}]",
            unresolved_top_routes.len(),
            unresolved.join(", ")
        );
    }
    if !children_by_owner.is_empty() {
        let owners = children_by_owner
            .keys()
            .take(8)
            .cloned()
            .collect::<Vec<_>>();
        bail!(
            "ConfigDumpInfo child metadata has no versioned owner: {}",
            owners.join(", ")
        );
    }

    metadata.sort_by(|left, right| left.name.cmp(&right.name));
    let xml = format_config_dump_info_xml(source_version, &metadata);
    let path = output_dir.join(CONFIG_DUMP_INFO_FILE_NAME);
    fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

fn parse_versions_blob(blob: &[u8], origin: VersionsBlobOrigin) -> Result<Vec<ConfigVersionEntry>> {
    let plain = inflate_raw_deflate(blob).context("failed to inflate Config versions row")?;
    let text = std::str::from_utf8(&plain).context("Config versions row is not valid UTF-8")?;
    let text = text.trim_start_matches('\u{feff}');
    let fields = split_1c_braced_fields(text, 0)
        .ok_or_else(|| anyhow!("Config versions row is not a structured 1C list"))?;
    if fields.first().map(|field| field.trim()) != Some("1") {
        bail!("Config versions row has an unsupported root discriminator");
    }
    let count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Config versions row has no valid pair count"))?;
    let expected_fields = 2usize
        .checked_add(
            count
                .checked_mul(2)
                .ok_or_else(|| anyhow!("Config versions pair count overflows"))?,
        )
        .ok_or_else(|| anyhow!("Config versions field count overflows"))?;
    if fields.len() != expected_fields {
        bail!(
            "Config versions row declares {count} pairs but contains {} fields",
            fields.len()
        );
    }

    let mut named = BTreeMap::<String, Uuid>::new();
    let mut generation_seen = false;
    for (pair_index, pair) in fields[2..].chunks_exact(2).enumerate() {
        let name = parse_1c_quoted_string(pair[0].trim())
            .ok_or_else(|| anyhow!("Config versions row contains an invalid entry name"))?;
        let version_text = pair[1].trim();
        let version = Uuid::parse_str(version_text)
            .with_context(|| format!("Config versions entry {name:?} has invalid UUID"))?;
        if name.is_empty() {
            if pair_index != 0 || generation_seen {
                bail!("Config versions generation entry is not the first pair");
            }
            generation_seen = true;
            continue;
        }
        if named.insert(name.clone(), version).is_some() {
            bail!("Config versions row contains duplicate entry {name}");
        }
    }
    if !generation_seen {
        bail!("Config versions row has no generation entry");
    }

    if origin.embeds_service_entries() {
        for service_name in VERSIONS_EMBEDDED_SERVICE_NAMES {
            if !named.contains_key(service_name) {
                bail!("Config versions row has no service entry {service_name}");
            }
        }
    }
    Ok(named
        .into_iter()
        .filter(|(name, _)| !VERSIONS_EMBEDDED_SERVICE_NAMES.contains(&name.as_str()))
        .map(|(id, version)| ConfigVersionEntry { id, version })
        .collect())
}

/// The stamp a dynamic update leaves on the entries it writes beside the live
/// ones: `<id>_dynupdate_<session uuid>`, optionally with the usual `.part`
/// suffix.
///
/// These entries are **not** newer versions of the entry whose id they carry.
/// In ERP «Управление холдингом» 3.2.12.6 the id `024d5d02-…` is the scheduled
/// job `ЗакрытиеМесяца` in the live set and a form record named `Форма` in the
/// stamped set -- a scheduled job does not become a form, so the two sets name
/// different object spaces rather than two states of one.
///
/// The platform answers for itself which set is the configuration: the
/// `versions` row, its own inventory, lists the live entries and never the
/// stamped ones. Comparing the manifest against it therefore has to compare
/// the live half, and a stamped entry is outside that comparison by
/// construction instead of missing from it. Nothing here decides whether such
/// an entry should ever be exported; it decides only that its presence is not
/// an inventory defect.
const DYNAMIC_UPDATE_STAMP: &str = "_dynupdate_";

pub(super) fn is_dynamic_update_entry(name: &str) -> bool {
    name.contains(DYNAMIC_UPDATE_STAMP)
}

fn validate_versions_inventory(
    versions: &[ConfigVersionEntry],
    file_names: &BTreeSet<String>,
) -> Result<()> {
    let version_names = versions
        .iter()
        .map(|entry| entry.id.as_str())
        .chain(MANIFEST_SERVICE_NAMES)
        .collect::<BTreeSet<_>>();
    let manifest_names = file_names
        .iter()
        .map(String::as_str)
        .filter(|name| !is_dynamic_update_entry(name) && *name != OPTIONAL_SERVICE_NAME)
        .collect::<BTreeSet<_>>();
    if version_names == manifest_names {
        return Ok(());
    }

    let missing = manifest_names
        .difference(&version_names)
        .take(8)
        .copied()
        .collect::<Vec<_>>();
    let unknown = version_names
        .difference(&manifest_names)
        .take(8)
        .copied()
        .collect::<Vec<_>>();
    bail!(
        "Config versions/manifest inventory mismatch: missing versions [{}], unknown versions [{}]",
        missing.join(", "),
        unknown.join(", ")
    )
}

fn add_configuration_group_references(
    canonical_refs: &mut BTreeMap<String, String>,
    metadata_texts: &[MetadataTextRow],
    configuration_module_groups: &BTreeSet<String>,
) -> Result<()> {
    if configuration_module_groups.is_empty() {
        return Ok(());
    }
    let configuration_names = metadata_texts
        .iter()
        .filter_map(|row| parse_configuration_reference_text_for_row(&row.text, &row.file_name))
        .collect::<BTreeSet<_>>();
    let mut names = configuration_names.into_iter();
    let name = names
        .next()
        .ok_or_else(|| anyhow!("configuration row-role group has no Configuration metadata"))?;
    if names.next().is_some() {
        bail!("configuration row-role group has multiple Configuration metadata owners");
    }
    let reference = format!("Configuration.{name}");
    for group in configuration_module_groups {
        canonical_refs.insert(group.clone(), reference.clone());
    }
    Ok(())
}

/// Finds the Configuration root's own canonical reference (`"Configuration.<Name>"`,
/// with no further `.` suffix) among `object_refs`.
///
/// The index is already built from the canonical root-envelope decoder by the
/// time this runs, so scanning it directly avoids re-decoding raw blob text
/// (which this function has no access to anyway).
fn configuration_top_level_reference(object_refs: &BTreeMap<String, String>) -> Option<&str> {
    object_refs
        .values()
        .find(|reference| {
            reference
                .strip_prefix("Configuration.")
                .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.'))
        })
        .map(String::as_str)
}

/// Registers `Configuration.<Name>.CommandInterface` /
/// `.MainSectionCommandInterface` for the Configuration record's own
/// embedded self-header identity, when that identity is a real uuid
/// distinct from the Configuration's storage key.
///
/// The Configuration object's own metadata text embeds a `{1,0,<uuid>}`
/// self-header. For an uncustomized configuration that uuid is the fixed
/// sentinel [`CONFIGURATION_COMMAND_INTERFACE_UUID`] (handled above), but
/// the platform allocates a real, per-configuration uuid there once the
/// Configuration-root command interface pair (`.9`/`.a`) has its own
/// storage records — confirmed against WMS5's `МодульWebОбмена_ERP25.cf`:
/// embedded self-header uuid `11a420f7-edda-47e2-bb56-c76b400a0bf6`,
/// carrying real `.9`/`.a` records that decode cleanly (to an all-empty
/// command interface, see [`CommandInterface::is_empty`]), and versioned
/// as their own top-level `versions` entries `11a420f7-....9` / `.a`.
/// Native `ConfigDumpInfo.xml` still names both
/// `Configuration.WebОбменERP25.MainSectionCommandInterface` /
/// `.CommandInterface` even though the export tree never materializes
/// their (empty) `Ext/*.xml` file — so the canonical name is derived here,
/// directly from the fixed suffix-to-role route, rather than depending on
/// a written/emitted source asset that may not exist for an empty record.
///
/// Harmless when the self-header identity never appears as its own
/// `versions` entry (confirmed for ERP УХ's `Web_Service`/`MDM_Management`,
/// whose analogous identity carries no `.9`/`.a` storage at all): the
/// alias is simply never looked up. Redundant-but-consistent when a full
/// application (confirmed for БСП demo, УТ) already resolves the same
/// entries through [`config_dump_top_name`]'s role-path fallback, since
/// both paths derive the identical route-based role name.
fn add_configuration_root_command_interface_references(
    canonical_refs: &mut BTreeMap<String, String>,
    metadata_texts: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
) {
    let Some(configuration_reference) = configuration_top_level_reference(object_refs) else {
        return;
    };
    for row in metadata_texts {
        let Some(identity) = parse_configuration_header_uuid(&row.text) else {
            continue;
        };
        for suffix in ["9", "a"] {
            let Some(role) = crate::compiler::families::assets::SourceAssetRegistry
                .route_by_suffix("Configuration", suffix)
                .and_then(|route| Path::new(route.relative_path()).file_stem())
                .and_then(|role| role.to_str())
            else {
                continue;
            };
            canonical_refs
                .entry(format!("{identity}.{suffix}"))
                .or_insert_with(|| format!("{configuration_reference}.{role}"));
        }
    }
}

fn config_dump_top_name(
    id: &str,
    canonical_refs: &BTreeMap<String, String>,
    module_text_paths: &BTreeMap<String, PathBuf>,
    source_assets: &BTreeMap<String, SourceAsset>,
    emitted_source_asset_paths: &BTreeMap<String, PathBuf>,
) -> Result<String> {
    if let Some(reference) = canonical_refs.get(id) {
        return Ok(reference.clone());
    }
    let (base_id, _) = id
        .rsplit_once('.')
        .ok_or_else(|| anyhow!("ConfigDumpInfo entry {id} has no canonical metadata reference"))?;
    let base = canonical_refs
        .get(base_id)
        .ok_or_else(|| anyhow!("ConfigDumpInfo entry {id} has unknown metadata owner {base_id}"))?;
    let role_path = emitted_source_asset_paths
        .get(id)
        .or_else(|| source_assets.get(id).map(|asset| &asset.primary_path))
        .or_else(|| module_text_paths.get(id))
        .ok_or_else(|| anyhow!("ConfigDumpInfo entry {id} has no typed row-role route"))?;
    let role = role_path
        .file_stem()
        .and_then(|role| role.to_str())
        .filter(|role| !role.is_empty())
        .ok_or_else(|| anyhow!("ConfigDumpInfo entry {id} has an invalid row-role route"))?;
    Ok(format!("{base}.{role}"))
}

fn build_config_dump_children(
    metadata_texts: &[MetadataTextRow],
    object_refs: &BTreeMap<String, String>,
    canonical_refs: &BTreeMap<String, String>,
    version_ids: &BTreeSet<&str>,
    configuration_module_groups: &BTreeSet<String>,
    partial_inventory_policy: ConfigDumpInfoPartialInventoryPolicy,
) -> Result<Option<BTreeMap<String, BTreeMap<String, String>>>> {
    let indexed_child_ids = object_refs
        .keys()
        .filter(|id| !version_ids.contains(id.as_str()))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut discovered_child_ids = BTreeSet::<String>::new();
    let mut children_by_owner = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut unresolved_child_roles = BTreeSet::<String>::new();

    for row in metadata_texts {
        let Some(owner_name) = canonical_refs.get(&row.file_name) else {
            continue;
        };
        if !version_ids.contains(row.file_name.as_str()) {
            continue;
        }
        // The Configuration record opens with its own metadata header, whose
        // uuid is the configuration's identity rather than a child of it: the
        // platform addresses the configuration by its storage key and its
        // parts by `<identity>.<n>`, and the bare identity never appears as a
        // `<Metadata>` id (0 occurrences across УТ, БСП demo, ERP УХ and both
        // ERP УХ mini-configurations).
        //
        // That identity was previously recognized only through
        // `configuration_module_groups`, which infers it from *file names* --
        // an owner id carrying module-like suffixes but no bare record. ERP
        // УХ's `Web_Service` and `MDM_Management` ship no configuration module
        // records at all, so that set comes out empty, the identity header is
        // taken for an unnameable child, and ConfigDumpInfo.xml is skipped
        // whole. Reading it from the record that states it holds either way.
        let configuration_identity = parse_configuration_header_uuid(&row.text);
        for (header, _) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if version_ids.contains(header.uuid.as_str()) {
                continue;
            }
            if configuration_identity.as_deref() == Some(header.uuid.as_str()) {
                continue;
            }
            if configuration_module_groups.contains(&header.uuid) {
                continue;
            }
            if header.uuid == CONFIGURATION_COMMAND_INTERFACE_UUID
                && !object_refs.contains_key(&header.uuid)
            {
                continue;
            }
            let child_name = if let Some(child_name) = object_refs.get(&header.uuid) {
                child_name.clone()
            } else if row.kind.as_deref() == Some("DocumentJournal") {
                format!("{owner_name}.Column.{}", header.name)
            } else {
                unresolved_child_roles.insert(format!(
                    "{} {}: {} ({})",
                    row.kind.as_deref().unwrap_or("<unknown>"),
                    row.file_name,
                    header.uuid,
                    header.name
                ));
                continue;
            };
            if !child_name
                .strip_prefix(owner_name)
                .is_some_and(|suffix| suffix.starts_with('.'))
            {
                bail!(
                    "ConfigDumpInfo child {} ({child_name}) is not owned by {} ({owner_name})",
                    header.uuid,
                    row.file_name
                );
            }
            if !discovered_child_ids.insert(header.uuid.clone()) {
                bail!(
                    "ConfigDumpInfo child {} is present under multiple metadata owners",
                    header.uuid
                );
            }
            children_by_owner
                .entry(row.file_name.clone())
                .or_default()
                .insert(header.uuid, child_name);
        }
    }

    if !unresolved_child_roles.is_empty() {
        // Same partial-inventory rule as the top-level routes: on a CF storage
        // image an unresolved child role always traces back to a record this
        // exporter did not recognize (already disclosed as `opaque` in the
        // report), so the incomplete ConfigDumpInfo.xml is skipped rather than
        // half-written; a full MSSQL Config-table dump decodes every text, so
        // there the same condition stays a hard error.
        if partial_inventory_policy == ConfigDumpInfoPartialInventoryPolicy::Skip {
            return Ok(None);
        }
        let unresolved = unresolved_child_roles
            .iter()
            .take(64)
            .cloned()
            .collect::<Vec<_>>();
        bail!(
            "ConfigDumpInfo has {} children without canonical metadata roles [{}]",
            unresolved_child_roles.len(),
            unresolved.join(", ")
        );
    }

    let discovered = discovered_child_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !indexed_child_ids.is_subset(&discovered) {
        if partial_inventory_policy == ConfigDumpInfoPartialInventoryPolicy::Skip {
            return Ok(None);
        }
        let missing = indexed_child_ids
            .difference(&discovered)
            .take(8)
            .copied()
            .collect::<Vec<_>>();
        bail!(
            "ConfigDumpInfo child inventory is missing indexed metadata [{}]",
            missing.join(", ")
        );
    }
    Ok(Some(children_by_owner))
}

fn config_version(version: Uuid) -> String {
    let mut value = String::with_capacity(40);
    for byte in version.to_bytes_le() {
        value.push_str(&format!("{byte:02x}"));
    }
    value.push_str("00000000");
    value
}

fn format_config_dump_info_xml(
    source_version: InfobaseConfigSourceVersion,
    metadata: &[ConfigDumpMetadata],
) -> Vec<u8> {
    let mut xml = String::new();
    xml.push('\u{feff}');
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    xml.push_str("<ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" format=\"Hierarchical\" version=\"");
    xml.push_str(source_version.as_str());
    xml.push_str("\">\r\n\t<ConfigVersions>\r\n");
    for entry in metadata {
        xml.push_str("\t\t<Metadata name=\"");
        xml.push_str(&escape_xml_text(&entry.name));
        xml.push_str("\" id=\"");
        xml.push_str(&escape_xml_text(&entry.id));
        xml.push_str("\" configVersion=\"");
        xml.push_str(&entry.config_version);
        if entry.children.is_empty() {
            xml.push_str("\"/>\r\n");
            continue;
        }
        xml.push_str("\">\r\n");
        for child in &entry.children {
            xml.push_str("\t\t\t<Metadata name=\"");
            xml.push_str(&escape_xml_text(&child.name));
            xml.push_str("\" id=\"");
            xml.push_str(&escape_xml_text(&child.id));
            xml.push_str("\"/>\r\n");
        }
        xml.push_str("\t\t</Metadata>\r\n");
    }
    xml.push_str("\t</ConfigVersions>\r\n</ConfigDumpInfo>");
    xml.into_bytes()
}
