//! The exporter's own reader for the command-interface family of configuration
//! assets, run against a saved `Config_inflated` directory instead of a
//! database: the other half of the interface writers' round trip.

use super::*;

/// Which exporter reader a row goes through.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum InterfaceAssetKind {
    /// A subsystem's (row `.1`) or common command's (`.0`) `Ext/CommandInterface.xml`, published only when it says something.
    ObjectCommandInterface,
    /// The configuration's `Ext/CommandInterface.xml` (`.a`) or
    /// `Ext/MainSectionCommandInterface.xml` (`.9`).
    ConfigurationCommandInterface,
    HomePageWorkArea,
    ClientApplicationInterface,
    StandaloneContent,
}

/// Every file name of a saved `Config_inflated` directory, and its metadata
/// rows (file names without a dot) read the way an export reads them.
pub(super) fn read_offline_metadata_rows(
    dir: &Path,
) -> Result<(BTreeSet<String>, Vec<MetadataTextRow>)> {
    let mut file_names = BTreeSet::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().to_string();
        if let Some(file_name) = name.strip_suffix("__part0.txt") {
            file_names.insert(file_name.to_string());
        }
    }
    let metadata_file_names = file_names
        .iter()
        .filter(|file_name| !file_name.contains('.'))
        .collect::<Vec<_>>();
    let row_audits = parallel::install(|| {
        metadata_file_names
            .par_iter()
            .filter_map(|file_name| {
                let text = fs::read_to_string(dir.join(format!("{file_name}__part0.txt"))).ok()?;
                Some(metadata_text_row_audit_from_text(
                    file_name,
                    text.trim_start_matches('\u{feff}').to_string(),
                ))
            })
            .collect::<Vec<_>>()
    })?;
    let mut metadata = Vec::new();
    for audit in row_audits {
        match audit {
            MetadataTextRowAudit::Extracted(mut row)
            | MetadataTextRowAudit::ExtractedWithWarning(mut row, _) => {
                normalize_direct_form_metadata(&mut row);
                metadata.push(row);
            }
            MetadataTextRowAudit::Miss(_) => {}
        }
    }
    Ok((file_names, metadata))
}

/// The reference indexes the exporter names these rows with, built by the
/// production builders from the metadata rows of a saved dump.
pub struct OfflineInterfaceContext {
    command_refs: BTreeMap<String, String>,
    metadata_refs: BTreeMap<String, MetadataCommandReference>,
    subsystem_refs: BTreeMap<String, SubsystemSourceReference>,
    form_refs: BTreeMap<String, FormSourceReference>,
    standalone_refs: StandaloneContentReferences,
}

impl OfflineInterfaceContext {
    /// Reads every `<file name>__part0.txt` of `dir`. The metadata rows (file
    /// names without a dot) feed the builders; every file name counts as a
    /// storage record, the way the table's own file names do in an export.
    pub fn from_inflated_dir(dir: &Path) -> Result<Self> {
        let (file_names, metadata) = read_offline_metadata_rows(dir)?;
        let command_refs = build_command_interface_reference_index_from_texts(&metadata);
        let metadata_refs = build_metadata_command_reference_index_from_texts(&metadata);
        let form_refs = build_complete_form_source_reference_index(&metadata);
        let template_refs = build_template_source_reference_index_from_texts(&[], &metadata);
        let subsystem_refs = build_subsystem_source_reference_index_from_texts(&metadata);
        let object_refs = build_metadata_object_reference_indexes_from_texts(&metadata).references;
        let configuration_root_object_refs =
            build_configuration_root_object_reference_index_from_texts(&metadata, &object_refs);
        let mut standalone_refs = build_standalone_content_references(
            &metadata,
            &configuration_root_object_refs,
            &form_refs,
            &template_refs,
            &subsystem_refs,
        );
        standalone_refs.storage_record_uuids =
            storage_record_uuids_from_file_names(file_names.iter().map(String::as_str));
        Ok(Self {
            command_refs,
            metadata_refs,
            subsystem_refs,
            form_refs,
            standalone_refs,
        })
    }

    /// The file the exporter writes for a row whose inflated plain text is
    /// `plain`, exactly as it lands on disk, or why it writes none.
    pub fn render(
        &self,
        kind: InterfaceAssetKind,
        plain: &[u8],
        source_version: InfobaseConfigSourceVersion,
    ) -> Result<Vec<u8>> {
        let blob = crate::compiler::families::native::deflate_bytes(plain)
            .map_err(|error| anyhow!("failed to deflate the row: {error}"))?;
        let xml = match kind {
            InterfaceAssetKind::ObjectCommandInterface
            | InterfaceAssetKind::ConfigurationCommandInterface => {
                // A subsystem row is published only when it says something
                // (`dynamic_source_asset`); the configuration's always is.
                if kind == InterfaceAssetKind::ObjectCommandInterface
                    && !parse_command_interface_blob(&blob, &self.command_refs, &self.metadata_refs)
                        .is_some_and(|interface| !interface.is_empty())
                {
                    bail!("the exporter publishes no file for this subsystem row");
                }
                let interface = parse_command_interface_blob_with_subsystem_refs(
                    &blob,
                    &self.command_refs,
                    &self.metadata_refs,
                    &self.subsystem_refs,
                )
                .context("the exporter cannot read the command interface")?;
                format_command_interface_xml(&interface).into_bytes()
            }
            InterfaceAssetKind::HomePageWorkArea => {
                let work_area =
                    parse_home_page_work_area_blob(&blob, &self.form_refs, &self.metadata_refs)
                        .context("the exporter cannot read the home page work area")?;
                format_home_page_work_area_xml(&work_area, source_version).into_bytes()
            }
            InterfaceAssetKind::ClientApplicationInterface => {
                let interface = parse_client_application_interface_blob(&blob)
                    .context("the exporter cannot read the client application interface")?;
                format_client_application_interface_xml(&interface).into_bytes()
            }
            InterfaceAssetKind::StandaloneContent => {
                extract_standalone_content_xml(&blob, &self.standalone_refs)?
            }
        };
        let adapter = MssqlLegacyAdapter::from_legacy_selector(source_version);
        Ok(normalize_legacy_source_asset_xml_version_bytes(
            &xml,
            adapter.xml_dialect(),
        ))
    }
}
