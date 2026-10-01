//! Directory conversion keeps physical payloads in owned stages; the public model stays authoritative.
use crate::{
    ConversionOptions, Disposition, EdtError, codec,
    disk::{self, Inventory},
    provenance,
};
use ibcmd_core::model::CanonicalConfiguration;
use morph1c_core::{ir::Configuration, version::FormatVersion};
use morph1c_pipeline::Format;
use std::{fs, path::Path};

#[derive(Debug)]
pub struct DirectoryFileAccounting {
    pub path: String,
    pub disposition: Disposition,
}
#[derive(Debug)]
pub struct DirectorySource {
    _owner: tempfile::TempDir,
    inventory: Inventory,
}
#[derive(Debug)]
pub struct DirectoryConversion {
    _owner: tempfile::TempDir,
    output: Inventory,
    pub canonical: CanonicalConfiguration,
    pub accounting: Vec<DirectoryFileAccounting>,
    pub extensions: Vec<crate::SourceExtensionUse>,
}
/// Copies a complete input to a private owned snapshot; no payload-size/file-count validity ceiling.
pub fn read_directory_source(path: impl AsRef<Path>) -> Result<DirectorySource, EdtError> {
    let owner = stage()?;
    let inventory = Inventory::snapshot(path.as_ref(), &owner.path().join("input"))?;
    Ok(DirectorySource {
        _owner: owner,
        inventory,
    })
}
/// Copies and validates the required EDT project envelope before conversion.
pub fn read_directory_project(path: impl AsRef<Path>) -> Result<DirectorySource, EdtError> {
    let source = read_directory_source(path)?;
    source.require_project()?;
    Ok(source)
}
impl DirectorySource {
    fn require_project(&self) -> Result<(), EdtError> {
        if self.inventory.entry("DT-INF/PROJECT.PMF").is_none() {
            return Err(EdtError::new("missing DT-INF/PROJECT.PMF"));
        }
        if self
            .inventory
            .entry("src/Configuration/Configuration.mdo")
            .is_none()
        {
            return Err(EdtError::new("missing src/Configuration/Configuration.mdo"));
        }
        Ok(())
    }
    pub fn file_count(&self) -> usize {
        self.inventory.entries.len()
    }
    /// Visits complete metadata envelopes one at a time for the CLI's explicit profile check.
    pub fn visit_xml_descriptors<E: std::fmt::Display>(
        &self,
        mut visitor: impl FnMut(&str, &[u8]) -> Result<(), E>,
    ) -> Result<(), EdtError> {
        for index in 0..self.inventory.entries.len() {
            if self.inventory.entries[index].metadata {
                let bytes = self.inventory.read_entry(index)?;
                visitor(&self.inventory.entries[index].path, &bytes).map_err(EdtError::source)?;
            }
        }
        Ok(())
    }
    pub fn xml_to_edt(&self, options: &ConversionOptions) -> Result<DirectoryConversion, EdtError> {
        xml_to_edt(self, options)
    }
    pub fn edt_to_xml(&self, options: &ConversionOptions) -> Result<DirectoryConversion, EdtError> {
        edt_to_xml(self, options)
    }
}
impl DirectoryConversion {
    pub fn file_count(&self) -> usize {
        self.output.entries.len()
    }
    pub fn byte_len(&self) -> u64 {
        self.output.bytes
    }
    pub fn verify(&self) -> Result<(), EdtError> {
        self.output.verify()
    }
    /// Stream-copy into a sibling, verify all bytes and exclusively publish a new directory.
    pub fn publish_new(&self, destination: impl AsRef<Path>) -> Result<(), EdtError> {
        let destination = destination.as_ref();
        absent(destination)?;
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let temporary = tempfile::Builder::new()
            .prefix(".ibcmd-new-")
            .tempdir_in(parent)
            .map_err(EdtError::source)?;
        let staging = temporary.path().join("tree");
        let copied = self.output.copy_to(&staging)?;
        copied.verify()?;
        absent(destination)?;
        ibcmd_xml::source_tree::rename_directory_new(&staging, destination)
            .map_err(EdtError::source)
    }
}
fn absent(path: &Path) -> Result<(), EdtError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(EdtError::new("destination already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(EdtError::source(error)),
    }
}
fn stage() -> Result<tempfile::TempDir, EdtError> {
    tempfile::Builder::new()
        .prefix("ibcmd-edt-disk-")
        .tempdir()
        .map_err(EdtError::source)
}
fn render(
    format: Format,
    config: &Configuration,
    dest: &Path,
    version: FormatVersion,
) -> Result<Inventory, EdtError> {
    morph1c_core::version::with_roundtrip_target(version, || {
        morph1c_pipeline::write_config(format, config, dest)
    })
    .map_err(EdtError::source)?;
    Inventory::scan(dest)
}
fn read_config(
    format: Format,
    inventory: &Inventory,
    version: FormatVersion,
) -> Result<Configuration, EdtError> {
    inventory.verify()?;
    let config = codec::read_codec(format, &inventory.root, version)?;
    inventory.verify()?;
    Ok(config)
}
fn accounting(
    source: &Inventory,
    rebuilt: &Inventory,
    format: Format,
) -> Result<Vec<DirectoryFileAccounting>, EdtError> {
    source.verify()?;
    rebuilt.verify()?;
    let mut accounting = Vec::new();
    let mut failures = Vec::new();
    let mut count = 0usize;
    for (index, entry) in source.entries.iter().enumerate() {
        let result = match rebuilt.entry(&entry.path) {
            Some(_) if entry.path.ends_with(".mdo") || entry.metadata => {
                Ok(Some(Disposition::Converted))
            }
            Some(other) => disk::compare_bodies(source, rebuilt, index, other)
                .map(|same| same.then_some(Disposition::Converted)),
            None if format == Format::Designer && entry.path == "ConfigDumpInfo.xml" => {
                Ok(Some(Disposition::Retained))
            }
            None => Ok(None),
        };
        match result {
            Ok(Some(disposition)) => accounting.push(DirectoryFileAccounting {
                path: entry.path.clone(),
                disposition,
            }),
            outcome => {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| EdtError::new("file failure count overflow"))?;
                if failures.len() < 128 {
                    let reason =
                        outcome
                            .err()
                            .map(|error| error.to_string())
                            .unwrap_or_else(|| {
                                "source file has no complete regenerated body/descriptor".to_owned()
                            });
                    failures.push(format!(
                        "{}: {}",
                        entry.path,
                        reason.chars().take(1024).collect::<String>()
                    ));
                }
            }
        }
    }
    if count != 0 {
        return Err(EdtError::new(format!(
            "{count} source files failed complete body accounting (showing first {}):\n{}",
            failures.len(),
            failures.join("\n")
        )));
    }
    Ok(accounting)
}
fn xml_to_edt(
    source: &DirectorySource,
    options: &ConversionOptions,
) -> Result<DirectoryConversion, EdtError> {
    let version = codec::options(options)?;
    let runtime = options.runtime_version.as_deref().ok_or_else(|| {
        EdtError::new("XML to EDT requires an explicit runtime_version for PROJECT.PMF")
    })?;
    if codec::runtime_format(runtime)? != version {
        return Err(EdtError::new("runtime version disagrees with XML dialect"));
    }
    if source
        .inventory
        .entries
        .iter()
        .any(|entry| entry.path.starts_with(provenance::PREFIX))
    {
        return Err(EdtError::new(
            "XML input occupies reserved provenance namespace",
        ));
    }
    let owner = stage()?;
    let config = read_config(Format::Designer, &source.inventory, version)?;
    let rebuilt = render(
        Format::Designer,
        &config,
        &owner.path().join("rebuilt"),
        version,
    )?;
    let files = accounting(&source.inventory, &rebuilt, Format::Designer)?;
    fs::remove_dir_all(&rebuilt.root).map_err(EdtError::source)?;
    let project = owner.path().join("project");
    fs::create_dir(&project).map_err(EdtError::source)?;
    render(Format::Edt, &config, &project.join("src"), version)?;
    // Identical profile/control serialization to the existing public memory lane.
    codec::write_project_controls(&config, &project, runtime)?;
    let generated = Inventory::scan(&project)?;
    retain(&source.inventory, &generated, options, &config)?;
    let output = Inventory::scan(&project)?;
    let returned_source = Inventory::scan(&project.join("src"))?;
    let returned = read_config(Format::Edt, &returned_source, version)?;
    if provenance::semantic_digest(&returned)? != provenance::semantic_digest(&config)? {
        return Err(EdtError::new(
            "generated EDT differs from source typed semantics",
        ));
    }
    let canonical = codec::canonical_inventory_with_policy(
        &source.inventory,
        options,
        ibcmd_core::source_policy::SourceOperationPolicy::source_operation(),
    )?;
    source.inventory.verify()?;
    output.verify()?;
    Ok(DirectoryConversion {
        _owner: owner,
        output,
        canonical,
        accounting: files,
        extensions: crate::extensions::source_extensions_from_model(&config)?,
    })
}
fn edt_to_xml(
    source: &DirectorySource,
    options: &ConversionOptions,
) -> Result<DirectoryConversion, EdtError> {
    source.require_project()?;
    let version = codec::options(options)?;
    let manifest = source
        .inventory
        .entry("DT-INF/PROJECT.PMF")
        .ok_or_else(|| EdtError::new("PROJECT.PMF missing"))?;
    codec::project_version_bytes(&source.inventory.read_entry(manifest)?, options, version)?;
    if source
        .inventory
        .entry("src/Configuration/Configuration.mdo")
        .is_none()
    {
        return Err(EdtError::new("missing src/Configuration/Configuration.mdo"));
    }
    for (index, entry) in source
        .inventory
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.path.starts_with("src/"))
    {
        if matches!(
            entry.path.as_str(),
            "DT-INF/PROJECT.PMF" | ".project" | ".settings/org.eclipse.core.resources.prefs"
        ) {
            codec::validate_control(&entry.path, &source.inventory.read_entry(index)?)?;
        } else if !entry.path.starts_with(provenance::PREFIX) {
            return Err(EdtError::new(format!(
                "{}: unknown EDT project control file",
                entry.path
            )));
        }
    }
    let owner = stage()?;
    let typed_source = Inventory::scan(&source.inventory.root.join("src"))?;
    let config = read_config(Format::Edt, &typed_source, version)?;
    let rebuilt = render(Format::Edt, &config, &owner.path().join("rebuilt"), version)?;
    let mut files = accounting(&typed_source, &rebuilt, Format::Edt)?;
    for file in &mut files {
        file.path = format!("src/{}", file.path);
    }
    files.extend(
        source
            .inventory
            .entries
            .iter()
            .filter(|entry| !entry.path.starts_with("src/"))
            .map(|entry| DirectoryFileAccounting {
                path: entry.path.clone(),
                disposition: Disposition::Converted,
            }),
    );
    fs::remove_dir_all(&rebuilt.root).map_err(EdtError::source)?;
    let rendered = render(
        Format::Designer,
        &config,
        &owner.path().join("rendered"),
        version,
    )?;
    let output = match restore(&source.inventory, options, &config)? {
        Some(original) => {
            let original_config = read_config(Format::Designer, &original, version)?;
            let original_rebuilt = render(
                Format::Designer,
                &original_config,
                &owner.path().join("original-rebuilt"),
                version,
            )?;
            accounting(&original, &original_rebuilt, Format::Designer)?;
            if provenance::semantic_digest(&original_config)?
                != provenance::semantic_digest(&config)?
            {
                return Err(EdtError::new(
                    "retained original XML disagrees with current typed EDT semantics",
                ));
            }
            fs::remove_dir_all(&original_rebuilt.root).map_err(EdtError::source)?;
            original.copy_to(&owner.path().join("restored"))?
        }
        None => rendered,
    };
    let canonical = codec::canonical_inventory_with_policy(
        &output,
        options,
        ibcmd_core::source_policy::SourceOperationPolicy::source_operation(),
    )?;
    source.inventory.verify()?;
    output.verify()?;
    Ok(DirectoryConversion {
        _owner: owner,
        output,
        canonical,
        accounting: files,
        extensions: crate::extensions::source_extensions_from_model(&config)?,
    })
}
fn retain(
    original: &Inventory,
    generated: &Inventory,
    options: &ConversionOptions,
    config: &Configuration,
) -> Result<(), EdtError> {
    let manifest = provenance::Manifest {
        version: 1,
        edt_version: options.edt_version.clone(),
        xml_dialect: options.xml_dialect.clone(),
        runtime_version: options.runtime_version.clone(),
        semantics: provenance::semantic_digest(config)?,
        generated: generated.hashes(),
        original: original.hashes(),
    };
    let directory = generated.root.join(".ibcmd-provenance");
    fs::create_dir(&directory).map_err(EdtError::source)?;
    original.copy_to(&directory.join("xml"))?;
    fs::write(
        generated.root.join(provenance::MANIFEST),
        serde_json::to_vec_pretty(&manifest).map_err(EdtError::source)?,
    )
    .map_err(EdtError::source)
}
fn restore(
    project: &Inventory,
    options: &ConversionOptions,
    config: &Configuration,
) -> Result<Option<Inventory>, EdtError> {
    let Some(index) = project.entry(provenance::MANIFEST) else {
        if project
            .entries
            .iter()
            .any(|entry| entry.path.starts_with(provenance::PREFIX))
        {
            return Err(EdtError::new("provenance payload exists without manifest"));
        }
        return Ok(None);
    };
    let manifest: provenance::Manifest =
        serde_json::from_slice(&project.read_entry(index)?).map_err(EdtError::source)?;
    if manifest.version != 1
        || manifest.edt_version != options.edt_version
        || manifest.xml_dialect != options.xml_dialect
        || manifest.runtime_version != options.runtime_version
    {
        return Err(EdtError::new(
            "provenance profile differs from requested conversion; remove provenance for typed conversion",
        ));
    }
    let generated = project
        .entries
        .iter()
        .filter(|entry| !entry.path.starts_with(provenance::PREFIX))
        .map(|entry| (entry.path.clone(), disk::hex(entry.digest)))
        .collect::<std::collections::BTreeMap<_, _>>();
    if generated != manifest.generated || provenance::semantic_digest(config)? != manifest.semantics
    {
        return Err(EdtError::new(
            "stale provenance: EDT files changed; refusing to restore original XML. Remove .ibcmd-provenance for typed conversion",
        ));
    }
    let original = Inventory::scan(&project.root.join(".ibcmd-provenance/xml"))?;
    if original.hashes() != manifest.original
        || project
            .entries
            .iter()
            .filter(|entry| entry.path.starts_with(provenance::PREFIX))
            .count()
            != original.entries.len() + 1
    {
        return Err(EdtError::new(
            "provenance payload inventory/digest mismatch",
        ));
    }
    Ok(Some(original))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_stage_tampering_cannot_publish_and_existing_destination_stays_untouched() {
        for action in 0..4 {
            let owner = stage().unwrap();
            let root = owner.path().join("output");
            fs::create_dir(&root).unwrap();
            fs::write(root.join("body.bin"), b"1234").unwrap();
            let inventory = Inventory::scan(&root).unwrap();
            let destination_owner = stage().unwrap();
            let destination = destination_owner.path().join("published");
            let conversion = DirectoryConversion {
                _owner: owner,
                output: inventory,
                canonical: CanonicalConfiguration::new(Vec::new()).unwrap(),
                accounting: Vec::new(),
                extensions: Vec::new(),
            };
            match action {
                0 => fs::write(root.join("body.bin"), b"4321").unwrap(),
                1 => fs::remove_file(root.join("body.bin")).unwrap(),
                2 => fs::write(root.join("extra.bin"), b"extra").unwrap(),
                _ => {
                    fs::create_dir(&destination).unwrap();
                    fs::write(destination.join("existing.bin"), b"keep").unwrap();
                }
            }
            assert!(conversion.publish_new(&destination).is_err());
            if action == 3 {
                assert_eq!(fs::read(destination.join("existing.bin")).unwrap(), b"keep");
                assert!(!destination.join("body.bin").exists());
            } else {
                assert!(!destination.exists());
            }
        }
    }
}
