use super::*;
use uuid::Uuid;

const CONFIG_DUMP_INFO_FILE_NAME: &str = "ConfigDumpInfo.xml";
const VERSIONS_SERVICE_NAMES: [&str; 3] = ["root", "version", "versions"];

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

pub(super) fn write_config_dump_info(
    output_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
    versions_blob: &[u8],
    inventory: ConfigDumpInfoInventory<'_>,
) -> Result<()> {
    let versions = parse_versions_blob(versions_blob)?;
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

    let version_ids = versions
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut children_by_owner = build_config_dump_children(
        inventory.metadata_texts,
        inventory.object_refs,
        &canonical_refs,
        &version_ids,
        inventory.configuration_module_groups,
    )?;

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
    fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))
}

fn parse_versions_blob(blob: &[u8]) -> Result<Vec<ConfigVersionEntry>> {
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

    for service_name in VERSIONS_SERVICE_NAMES {
        if !named.contains_key(service_name) {
            bail!("Config versions row has no service entry {service_name}");
        }
    }
    Ok(named
        .into_iter()
        .filter(|(name, _)| !VERSIONS_SERVICE_NAMES.contains(&name.as_str()))
        .map(|(id, version)| ConfigVersionEntry { id, version })
        .collect())
}

fn validate_versions_inventory(
    versions: &[ConfigVersionEntry],
    file_names: &BTreeSet<String>,
) -> Result<()> {
    let version_names = versions
        .iter()
        .map(|entry| entry.id.as_str())
        .chain(VERSIONS_SERVICE_NAMES)
        .collect::<BTreeSet<_>>();
    let manifest_names = file_names
        .iter()
        .map(String::as_str)
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
        .filter_map(|row| {
            parse_configuration_reference_text_for_row(&row.text, &row.file_name)
        })
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
) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
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
        for (header, _) in
            nested_headers_with_offsets_from_text(&row.text, &row.file_name, |_| true)
        {
            if version_ids.contains(header.uuid.as_str()) {
                continue;
            }
            if configuration_module_groups.contains(&header.uuid) {
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
    Ok(children_by_owner)
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
