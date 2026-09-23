//! The exporter's own reader for help rows and `HTMLDocument` template bodies,
//! run against a saved `Config_inflated` directory instead of a database: the
//! other half of the help writer's round trip.

use super::interface_audit::read_offline_metadata_rows;
use super::*;

/// The index the exporter names a stored help link or picture with, built by
/// the production builders from the metadata rows of a saved dump.
pub struct OfflineHelpContext {
    help_refs: BTreeMap<String, String>,
}

/// What the exporter writes for one help row: the `Help.xml` (or
/// `Template.xml`) and, by file name, every page and every attachment.
pub struct RenderedHelp {
    pub xml: Vec<u8>,
    pub pages: Vec<(String, Vec<u8>)>,
    pub files: Vec<(String, Vec<u8>)>,
}

impl OfflineHelpContext {
    pub fn from_inflated_dir(dir: &Path) -> Result<Self> {
        let (_, metadata) = read_offline_metadata_rows(dir)?;
        let form_refs = build_complete_form_source_reference_index(&metadata);
        let template_refs = build_template_source_reference_index_from_texts(&[], &metadata);
        let subsystem_refs = build_subsystem_source_reference_index_from_texts(&metadata);
        let object_refs = build_metadata_object_reference_indexes_from_texts(&metadata).references;
        Ok(Self {
            help_refs: build_help_reference_index(
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            ),
        })
    }

    /// The files the exporter writes for a row whose inflated plain text is
    /// `plain`, exactly as they land on disk.
    pub fn render(
        &self,
        plain: &[u8],
        source_version: InfobaseConfigSourceVersion,
    ) -> Result<RenderedHelp> {
        let blob = crate::compiler::families::native::deflate_bytes(plain)
            .map_err(|error| anyhow!("failed to deflate the row: {error}"))?;
        let help = parse_help_blob(&blob).context("the exporter cannot read the help row")?;
        let adapter = MssqlLegacyAdapter::from_legacy_selector(source_version);
        let xml = normalize_legacy_source_asset_xml_version_bytes(
            format_help_xml(&help.pages).as_bytes(),
            adapter.xml_dialect(),
        );
        Ok(RenderedHelp {
            xml,
            pages: help
                .pages
                .iter()
                .map(|page| {
                    (
                        page.file_name.clone(),
                        rewrite_help_links(&page.content, &self.help_refs),
                    )
                })
                .collect(),
            files: help
                .files
                .into_iter()
                .map(|file| (file.file_name, file.content))
                .collect(),
        })
    }
}
