//! Read-only named extension export from one locked file infobase.
use crate::{
    legacy_version::InfobaseConfigSourceVersion,
    mssql_dump::{
        StorageImageSourceExportReport,
        cas::{CasHash, CasManifest, CasStorageRow, resolve_cas_storage_image_with_origin},
    },
};
use anyhow::{Context, Result, bail};
use ibcmd_core::storage::{MAX_STORAGE_ENTRIES, MAX_STORAGE_IMAGE_RETAINED_BYTES};
use ibcmd_filedb::{ConfigRow, FileDb};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct MissingExtension(pub String);
impl fmt::Display for MissingExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Операция невозможна: расширение {} не найдено", self.0)
    }
}
impl std::error::Error for MissingExtension {}

struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read_rows(db: &mut FileDb, table: &str) -> Result<Vec<ConfigRow>> {
    Ok(db.config_rows_bounded(
        table,
        MAX_STORAGE_IMAGE_RETAINED_BYTES as u64,
        MAX_STORAGE_ENTRIES,
    )?)
}
fn validate_part(row: &ConfigRow) -> Result<()> {
    if row.attributes != 0 || row.part_no != 0 || row.data_size != row.binary.len() as u64 {
        bail!(
            "extension row {:?}: unsupported Attributes, multipart or inconsistent DataSize",
            row.file_name
        );
    }
    Ok(())
}

pub(crate) fn export(
    db_path: &Path,
    name: &str,
    output: &Path,
    version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    if version != InfobaseConfigSourceVersion::V2_20 {
        bail!("file extension export supports only the measured 8.3.27 / XML 2.20 profile");
    }
    let mut db = FileDb::open(db_path)?;
    let registry = db.extension_registry()?;
    // Native 8.3.27 accepts Cyrillic case changes and trailing ASCII spaces;
    // it does not require the caller to reproduce the stored spelling.
    let requested = name.trim_end_matches(' ').to_lowercase();
    let mut matches = registry
        .iter()
        .filter(|row| row.name.to_lowercase() == requested);
    let selected = matches
        .next()
        .ok_or_else(|| MissingExtension(name.into()))?;
    if matches.next().is_some() {
        bail!("extension name {name:?} is ambiguous");
    }
    let mut ids = BTreeSet::new();
    if registry.iter().any(|row| !ids.insert(row.physical_id)) {
        bail!("duplicate physical extension registry identity");
    }
    // A pending main configuration can change the base-reference indexes.
    // Its interaction with extension preparation has not been measured.
    if !read_rows(&mut db, "CONFIGSAVE")?.is_empty() {
        bail!("prepared main configuration is not supported for file extension export");
    }
    let info = crate::mssql_extensions::decode_file_extension_zipped_info(&selected.zipped_info)
        .with_context(|| format!("invalid file registry envelope for extension {name:?}"))?;
    let prefix = format!(
        "{}__",
        crate::mssql_extension_stage::extension_namespace_prefix(selected.physical_id)
    );
    let mut active = BTreeMap::<CasHash, CasStorageRow>::new();
    for row in read_rows(&mut db, "CONFIGCAS")? {
        validate_part(&row)?;
        let hash = CasHash::parse_hex(&row.file_name)?;
        if active
            .insert(hash, CasStorageRow::new(hash, row.binary))
            .is_some()
        {
            bail!("duplicate active CAS row {hash}");
        }
    }
    let mut staged = Vec::new();
    let mut names = BTreeSet::new();
    for row in read_rows(&mut db, "CONFIGCASSAVE")? {
        let Some(logical) = row.file_name.strip_prefix(&prefix) else {
            continue;
        };
        validate_part(&row)?;
        if !names.insert(logical.to_owned()) {
            bail!("duplicate staged extension row {logical:?}");
        }
        staged.push((logical.to_owned(), i32::try_from(row.part_no)?, row.binary));
    }
    let root = if staged.is_empty() {
        CasHash::parse_hex(&info.active_cas_root)?
    } else {
        let packed_root = staged
            .iter()
            .find(|(name, _, _)| name == "configinfo")
            .context("the staged extension has no configinfo")?;
        let manifest = CasManifest::decode(&packed_root.2)?;
        let expected: BTreeMap<_, _> = manifest
            .entries()
            .iter()
            .map(|entry| (entry.logical_name.as_str(), entry.content_hash))
            .collect();
        for (logical, _, packed) in &staged {
            if logical != "configinfo"
                && expected.get(logical.as_str()) != Some(&CasHash::for_packed_bytes(packed))
            {
                bail!("staged extension row {logical:?} is not classified by configinfo");
            }
        }
        let (root, supplied) = crate::mssql_extension_export::staged_cas_rows(staged)?;
        for row in supplied {
            if let Some(old) = active.get(&row.hash) {
                if old.packed != row.packed {
                    bail!("conflicting active and staged CAS row {}", row.hash);
                }
            } else {
                active.insert(row.hash, row);
            }
        }
        root
    };
    let image = resolve_file_cas_image(root, active.into_values().collect())?;
    let base = crate::mssql_dump::file_extension_base_provider(&mut db, version)?;
    let stage = Stage(crate::mssql_extension_export::prepare_atomic_staging_root(
        output,
    )?);
    let payload = stage.0.join("payload");
    let mut report = crate::mssql_dump::extension::export_extension_image_to_source(
        &image,
        &payload,
        version,
        Some(base),
    )?;
    if report.storage.failed != 0 || report.storage.opaque != 0 {
        bail!(
            "extension writer is incomplete: {} failed and {} opaque storage rows",
            report.storage.failed,
            report.storage.opaque
        );
    }
    // Removing an existing empty directory is atomic with respect to its
    // contents: a concurrent new file makes remove_dir fail, never gets deleted.
    match fs::symlink_metadata(output) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => fs::remove_dir(output)?,
        Ok(_) => bail!(
            "refusing to replace non-directory or link {}",
            output.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::rename(&payload, output)?;
    report.output_dir = output.to_owned();
    // Keep db (and its read lock) alive through the publication above.
    drop(db);
    Ok(report)
}

fn resolve_file_cas_image(
    root: CasHash,
    rows: Vec<CasStorageRow>,
) -> Result<ibcmd_core::storage::StorageImage> {
    let root_row = rows
        .iter()
        .find(|row| row.hash == root)
        .context("missing file CAS root")?;
    let manifest = CasManifest::decode(&root_row.packed)?;
    if manifest.identity().storage_format != 80327 {
        bail!(
            "file extension storage format {} is not the measured 80327 profile",
            manifest.identity().storage_format
        );
    }
    let origin = ibcmd_core::storage::StorageOrigin::new(
        ibcmd_core::artifact::StorageProfileId::parse("storage:file-configcas")?,
        ibcmd_core::storage::StorageProvenance::new(&format!("file:configcas-root:{root}"))?,
    );
    Ok(resolve_cas_storage_image_with_origin(root, rows, origin)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(format: u32) -> Result<ibcmd_core::storage::StorageImage> {
        let content = crate::module_blob::deflate_raw(b"clean-room payload").unwrap();
        let hash = CasHash::for_packed_bytes(&content);
        let id = "00000000-0000-0000-0000-000000000001";
        let text = format!(
            "{{0,{{216,0,{{{format},0}}}}}},\r\n{{2,{id},}},\r\n{{1,\"{id}\",{}}}",
            crate::module_blob::encode_base64(hash.as_bytes())
        );
        let packed = crate::module_blob::deflate_raw(text.as_bytes()).unwrap();
        let root = CasHash::for_packed_bytes(&packed);
        resolve_file_cas_image(
            root,
            vec![
                CasStorageRow::new(root, packed),
                CasStorageRow::new(hash, content),
            ],
        )
    }

    #[test]
    fn file_cas_refuses_unmeasured_storage_formats() {
        assert!(image(80327).is_ok());
        for format in [80324, 80501] {
            assert!(
                image(format).is_err(),
                "unmeasured file storage format {format} was accepted"
            );
        }
    }

    #[test]
    fn file_cas_retains_its_file_origin() {
        let image = image(80327).unwrap();
        assert_eq!(
            image.source_profile().unwrap().as_str(),
            "storage:file-configcas"
        );
        assert!(
            image.entries()[0]
                .origin()
                .provenance()
                .as_str()
                .starts_with("file:configcas-root:")
        );
    }
}
