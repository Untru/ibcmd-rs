//! Configuration extensions (.cfe). File and database exports use the same
//! adoption-aware source pipeline. The file adapter validates the CFE manifest
//! and retains its own reference context instead of fetching a base database.
//!
//! Every adopted object -- the root included -- carries its adoption in the
//! header after the comment: `…,"comment",1,N,(property uuid,state)×N,
//! <extended object uuid>,0}` (`0,0,<nil>,0` for an own object). A listed
//! property is controlled by the extension: printed as a property element, or
//! as `<xr:PropertyState>` `Extended` for a module or command interface the
//! extension extends.

pub mod adopted;
// The readers of an object's adoption and of its predefined items' extension
// state, moved to the dump readers' crate; their paths stay.
pub use ibcmd_dump_readers::extension::{adoption, predefined};
pub mod root;

pub(crate) mod build;

use std::path::Path;

use anyhow::Result;
use ibcmd_cf::archive::PackedCfArchive;

use crate::{
    legacy_version::InfobaseConfigSourceVersion,
    mssql_dump::{self, StorageImageSourceExportReport},
};

/// An extension container keeps `configinfo` and no `root`; a configuration
/// has `root` (and `version`/`versions`), an external object has `root`.
pub fn is_extension(archive: &PackedCfArchive) -> bool {
    let has = |wanted: &str| archive.entries().iter().any(|entry| entry.name() == wanted);
    has("configinfo") && !has("root")
}

/// `mssql_dump::export_packed_cf_archive_to_source`, with an extension's
/// output rewritten as the platform dumps an extension.
pub fn export_packed_cf_archive_to_source(
    archive: PackedCfArchive,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    if !is_extension(&archive) {
        return mssql_dump::export_packed_cf_archive_to_source(
            archive,
            output_dir,
            overwrite,
            source_version,
        );
    }
    let (_, source_profile, entries) = archive.into_parts();
    mssql_dump::extension::export_extension_entries_to_source(
        source_profile.as_str(),
        entries
            .into_iter()
            .map(|entry| entry.into_parts())
            .collect(),
        output_dir,
        overwrite,
        source_version,
    )
}

/// [`export_packed_cf_archive_to_source`] for `(name, packed payload)`
/// entries already taken out of the container -- a subset of them, for an
/// incremental export (`crate::update`).
pub fn export_entries_to_source(
    source_profile: &str,
    entries: Vec<(String, Vec<u8>)>,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    let has = |wanted: &str| entries.iter().any(|(name, _)| name == wanted);
    if has("configinfo") && !has("root") {
        return mssql_dump::extension::export_extension_entries_to_source(
            source_profile,
            entries,
            output_dir,
            overwrite,
            source_version,
        );
    }
    mssql_dump::export_packed_entries_to_source(
        source_profile,
        entries,
        output_dir,
        overwrite,
        source_version,
        None,
    )
}
