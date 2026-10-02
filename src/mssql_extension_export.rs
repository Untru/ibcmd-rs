//! Direct read-only export of SQL Server configuration extensions.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_cf::export::StorageExportPlan;
use ibcmd_core::storage::StorageImage;
use serde::Serialize;
use uuid::Uuid;

use crate::cli::{
    MssqlDumpExtensionArgs, MssqlExtensionImage, MssqlExtensionListArgs, MssqlExtensionListFormat,
};
use crate::mssql_dump::StorageImageSourceExportReport;
use crate::mssql_dump::cas::{
    CasHash, CasStorageRow, MssqlStorageTable, fetch_cas_storage_image,
    fetch_cas_storage_image_layered,
};
use crate::mssql_extension_stage::extension_namespace_prefix;
use crate::mssql_extensions::{MssqlExtensionInfo, list_extensions};
use crate::sql::{SqlExec, SqlOptions};

/// Bounds of a staged namespace (the activation layer's).
const MAX_STAGED_ROWS: usize = 100_000;
const MAX_STAGED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct MssqlExtensionDumpReport {
    pub schema_version: u32,
    pub database: String,
    pub all_extensions: bool,
    pub extensions: Vec<MssqlExtensionDumpEntry>,
}

#[derive(Debug, Serialize)]
pub struct MssqlExtensionDumpEntry {
    pub name: String,
    pub active_cas_root: String,
    /// `active` or `staged`: the stored image that was exported.
    pub image: &'static str,
    /// SHA-1 of the root (`configinfo`) of the image that was exported.
    pub image_cas_root: String,
    pub output_dir: String,
    pub complete: bool,
    pub storage_rows_complete: bool,
    pub native_xml_parity: bool,
    pub warnings: Vec<String>,
    pub export: StorageImageSourceExportReport,
}

pub fn dump_extensions(args: &MssqlDumpExtensionArgs) -> Result<MssqlExtensionDumpReport> {
    let registry = list_extensions(&MssqlExtensionListArgs {
        sqlcmd: args.sqlcmd.clone(),
        server: args.server.clone(),
        sql_user: args.sql_user.clone(),
        sql_pwd: args.sql_pwd.clone(),
        sql_pwd_env: args.sql_pwd_env.clone(),
        sqlcmd_trust_cert: args.sqlcmd_trust_cert,
        database: args.database.clone(),
        format: MssqlExtensionListFormat::Json,
    })?;
    let selected = select_extensions(
        &registry.extensions,
        args.extension.as_deref(),
        args.all_extensions,
    )?;
    validate_selected_names(&selected)?;
    let password = resolve_password(args)?;
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: args.sqlcmd.as_deref(),
        bcp: args.bcp_executable.as_deref(),
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: args.sqlcmd_trust_cert,
    })?;
    let mut fetched = Vec::with_capacity(selected.len());
    let mut sources = Vec::with_capacity(selected.len());
    for extension in selected {
        let (image, source) = fetch_extension_image(&sql, &args.database, extension, args.image)
            .with_context(|| format!("failed to fetch extension {:?}", extension.name))?;
        fetched.push((extension, image));
        sources.push(source);
    }
    write_lab_rows_out(&fetched)?;

    let staging_root = prepare_atomic_staging_root(&args.output_dir)?;
    let export_result = (|| -> Result<Vec<MssqlExtensionDumpEntry>> {
        let mut extensions = Vec::with_capacity(fetched.len());
        for ((extension, image), source) in fetched.iter().zip(&sources) {
            let staging_dir = if args.all_extensions {
                staging_root.join(&extension.name)
            } else {
                staging_root.join("payload")
            };
            let final_dir = if args.all_extensions {
                args.output_dir.join(&extension.name)
            } else {
                args.output_dir.clone()
            };
            let export = crate::mssql_dump::extension::export_extension_image_to_source(
                image,
                &staging_dir,
                args.source_version,
                Some(crate::mssql_dump::extension::base_index_provider(
                    &sql,
                    &args.database,
                    args.source_version,
                )),
            )
            .with_context(|| format!("failed to export extension {:?}", extension.name))?;
            let storage_rows_complete = export.storage.failed == 0 && export.storage.opaque == 0;
            // Every converter reads fail-closed: a row it cannot state exactly
            // is reported as opaque or failed and never written approximately,
            // so a tree without such rows is the native one. Measured on the
            // four БСП 8.3.27 extensions: `source-diff` against the native
            // export finds no difference (docs/extensions/parity.md).
            let native_xml_parity = storage_rows_complete;
            let complete = storage_rows_complete && native_xml_parity;
            let mut warnings = Vec::new();
            if !storage_rows_complete {
                warnings.push(format!(
                    "extension writer is incomplete: {} opaque and {} failed storage rows",
                    export.storage.opaque, export.storage.failed
                ));
            }
            let mut export = export;
            export.output_dir = final_dir.clone();
            extensions.push(MssqlExtensionDumpEntry {
                name: extension.name.clone(),
                active_cas_root: extension.active_cas_root.clone(),
                image: source.image,
                image_cas_root: source.root.to_hex(),
                output_dir: final_dir.display().to_string(),
                complete,
                storage_rows_complete,
                native_xml_parity,
                warnings,
                export,
            });
        }
        Ok(extensions)
    })();
    let extensions = match export_result {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_root);
            return Err(error);
        }
    };
    let publication_tree = if args.all_extensions {
        staging_root.clone()
    } else {
        staging_root.join("payload")
    };
    publish_atomic_tree(&publication_tree, &args.output_dir, args.overwrite)?;
    if !args.all_extensions {
        let _ = fs::remove_dir(&staging_root);
    }
    Ok(MssqlExtensionDumpReport {
        schema_version: 1,
        database: args.database.clone(),
        all_extensions: args.all_extensions,
        extensions,
    })
}

/// Which stored image of an extension was read.
pub(crate) struct ImageSource {
    /// `active` or `staged`.
    pub(crate) image: &'static str,
    pub(crate) root: CasHash,
}

/// The image of one extension: the staged one (`ConfigCASSave`) when the
/// extension has staged rows and `choice` allows it, else the active one.
///
/// The native `config export --extension` exports the staged state: the БСП 8.5
/// clone keeps two changed modules of ServiceDesk staged and the native tree
/// holds them. A staged namespace is the changed rows and a new `configinfo`;
/// the rows it leaves out are the ones `ConfigCAS` already holds under the
/// same digest.
pub(crate) fn fetch_extension_image(
    sql: &SqlExec,
    database: &str,
    extension: &MssqlExtensionInfo,
    choice: MssqlExtensionImage,
) -> Result<(StorageImage, ImageSource)> {
    let staged = if choice == MssqlExtensionImage::Active {
        Vec::new()
    } else {
        let prefix = format!(
            "{}__",
            extension_namespace_prefix(extension.physical_registry_id)
        );
        crate::mssql_dump::fetch_extension_activation_rows(
            sql,
            database,
            "ConfigCASSave",
            &prefix,
            MAX_STAGED_ROWS,
            MAX_STAGED_BYTES,
        )?
        .into_iter()
        .map(|row| {
            let logical_name = row
                .file_name
                .strip_prefix(&prefix)
                .ok_or_else(|| anyhow!("the staged snapshot returned an unrelated row"))?
                .to_owned();
            Ok((logical_name, row.part_no, row.binary_data))
        })
        .collect::<Result<Vec<_>>>()?
    };
    if staged.is_empty() {
        if choice == MssqlExtensionImage::Staged {
            bail!("the extension has no staged rows in ConfigCASSave");
        }
        let root = CasHash::parse_hex(&extension.active_cas_root)
            .with_context(|| "the extension has an invalid CAS root")?;
        let image = fetch_cas_storage_image(sql, database, MssqlStorageTable::ConfigCas, root)?;
        return Ok((
            image,
            ImageSource {
                image: "active",
                root,
            },
        ));
    }
    let (root, supplied) = staged_cas_rows(staged)?;
    let (image, _) = fetch_cas_storage_image_layered(
        sql,
        database,
        MssqlStorageTable::ConfigCas,
        root,
        supplied,
    )?;
    Ok((
        image,
        ImageSource {
            image: "staged",
            root,
        },
    ))
}

/// The staged rows of one namespace as content-addressed rows, and the digest
/// of the staged `configinfo` that roots them.
fn staged_cas_rows(staged: Vec<(String, i32, Vec<u8>)>) -> Result<(CasHash, Vec<CasStorageRow>)> {
    let mut root = None;
    let mut rows = Vec::with_capacity(staged.len());
    for (logical_name, part_no, packed) in staged {
        if part_no != 0 {
            bail!("staged row {logical_name:?} has PartNo {part_no}; only PartNo 0 is supported");
        }
        let hash = CasHash::for_packed_bytes(&packed);
        if logical_name == "configinfo" {
            root = Some(hash);
        }
        rows.push(CasStorageRow::new(hash, packed));
    }
    let root = root.ok_or_else(|| anyhow!("the staged rows have no configinfo"))?;
    Ok((root, rows))
}

/// Lab aid: `IBCMD_RS_EXTENSION_ROWS_OUT=<dir>` also writes every fetched
/// extension's logical rows as `<dir>/<extension>/<name>__part0.bin`, the
/// layout `mssql-dump-config --rows-dir` reads (the packed payload exactly as
/// the CAS keeps it). Nothing is written without the variable.
fn write_lab_rows_out(fetched: &[(&MssqlExtensionInfo, StorageImage)]) -> Result<()> {
    let Some(root) = std::env::var_os("IBCMD_RS_EXTENSION_ROWS_OUT").filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    for (extension, image) in fetched {
        let dir = PathBuf::from(&root).join(&extension.name);
        fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
        let plan = StorageExportPlan::from_image(image);
        for record in plan.records() {
            let payload = record.packed_payload().map_err(|error| {
                anyhow!(
                    "row {} of {:?}: {error}",
                    record.logical_name(),
                    extension.name
                )
            })?;
            let path = dir.join(format!("{}__part0.bin", record.logical_name()));
            fs::write(&path, &*payload)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

fn prepare_atomic_staging_root(output: &Path) -> Result<PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".ibcmd-rs-extension-export-{}", Uuid::new_v4()));
    fs::create_dir(&staging)?;
    Ok(staging)
}

fn publish_atomic_tree(staging: &Path, output: &Path, overwrite: bool) -> Result<()> {
    if !output.exists() {
        fs::rename(staging, output)?;
        return Ok(());
    }
    if !overwrite {
        let _ = fs::remove_dir_all(staging);
        bail!(
            "output path {} already exists; use --overwrite",
            output.display()
        );
    }
    let metadata = fs::symlink_metadata(output)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        let _ = fs::remove_dir_all(staging);
        bail!(
            "refusing to replace non-directory or link {}",
            output.display()
        );
    }
    let backup = output.with_file_name(format!(".ibcmd-rs-extension-backup-{}", Uuid::new_v4()));
    fs::rename(output, &backup)?;
    if let Err(error) = fs::rename(staging, output) {
        let _ = fs::rename(&backup, output);
        return Err(error.into());
    }
    fs::remove_dir_all(backup)?;
    Ok(())
}

fn select_extensions<'a>(
    extensions: &'a [MssqlExtensionInfo],
    requested: Option<&str>,
    all: bool,
) -> Result<Vec<&'a MssqlExtensionInfo>> {
    match (requested, all) {
        (Some(_), true) | (None, false) => {
            bail!("exactly one of --extension or --all-extensions must be specified")
        }
        (None, true) => {
            if extensions.is_empty() {
                bail!("the infobase contains no configuration extensions");
            }
            Ok(extensions.iter().collect())
        }
        (Some(name), false) => {
            let mut matches = extensions.iter().filter(|extension| extension.name == name);
            let selected = matches
                .next()
                .ok_or_else(|| anyhow!("configuration extension {name:?} was not found"))?;
            if matches.next().is_some() {
                bail!("configuration extension name {name:?} is ambiguous");
            }
            Ok(vec![selected])
        }
    }
}

fn validate_extension_directory_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    let mut components = path.components();
    let one_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    let trimmed = name.trim_end_matches([' ', '.']);
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    // A separator of either platform: the tree is read on Windows too, where
    // `a\b` is two folders.
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || trimmed != name
        || reserved
        || !one_normal_component
    {
        bail!("extension name {name:?} is unsafe as an output directory");
    }
    Ok(())
}

fn validate_selected_names(selected: &[&MssqlExtensionInfo]) -> Result<()> {
    let mut normalized = BTreeSet::new();
    for extension in selected {
        validate_extension_directory_name(&extension.name)?;
        let key = extension.name.trim_end_matches([' ', '.']).to_lowercase();
        if !normalized.insert(key) {
            bail!("extension names collide on the output filesystem");
        }
    }
    Ok(())
}

fn resolve_password(args: &MssqlDumpExtensionArgs) -> Result<Option<String>> {
    if args.sql_user.is_none() {
        return Ok(None);
    }
    args.sql_pwd
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| {
            std::env::var(&args.sql_pwd_env)
                .ok()
                .filter(|value| !value.is_empty())
        })
        .map(Some)
        .ok_or_else(|| {
            anyhow!(
                "SQL login was provided but no password was found in --sql-pwd or environment variable {}",
                args.sql_pwd_env
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_extensions::{
        MssqlExtensionField, MssqlExtensionPurpose, MssqlExtensionScope,
    };

    fn extension(name: &str) -> MssqlExtensionInfo {
        MssqlExtensionInfo {
            physical_registry_id: [0; 16],
            registry_version: [0; 8],
            zipped_info: Vec::new(),
            registry_id: "347eea01-0773-11e8-8780-107b44a2858a".to_owned(),
            name: name.to_owned(),
            version: "1.0".to_owned(),
            order: 1,
            active: true,
            purpose: MssqlExtensionPurpose::Customization,
            scope: MssqlExtensionScope::Infobase,
            safe_mode: MssqlExtensionField::Available(false),
            security_profile_name: MssqlExtensionField::Unavailable,
            unsafe_action_protection: MssqlExtensionField::Available(false),
            used_in_distributed_infobase: false,
            update_timestamp: "2026-08-29T00:00:00".to_owned(),
            active_cas_root: "e1a4957cd700e47cea9ad20e66d489f9e5b0bac2".to_owned(),
        }
    }

    #[test]
    fn selects_one_or_all_extensions() {
        let values = vec![extension("One"), extension("Two")];
        assert_eq!(
            select_extensions(&values, Some("Two"), false).unwrap()[0].name,
            "Two"
        );
        assert_eq!(select_extensions(&values, None, true).unwrap().len(), 2);
        assert!(select_extensions(&values, None, false).is_err());
        assert!(select_extensions(&values, Some("Missing"), false).is_err());
    }

    #[test]
    fn rejects_unsafe_extension_directory_names() {
        for name in [
            "", ".", "..", "a/b", r"a\b", "CON", "com1.txt", "Name.", "Name ",
        ] {
            assert!(validate_extension_directory_name(name).is_err(), "{name:?}");
        }
        validate_extension_directory_name("_ДемоРасширение").unwrap();
    }

    #[test]
    fn rejects_case_insensitive_all_extension_collisions() {
        let values = vec![extension("ServiceDesk"), extension("servicedesk")];
        let selected = select_extensions(&values, None, true).unwrap();
        assert!(validate_selected_names(&selected).is_err());
    }

    /// A staged namespace holds the rows a change touched and a new `configinfo`
    /// that names them; the export reads them as content-addressed rows rooted
    /// at that `configinfo`.
    #[test]
    fn staged_rows_are_content_addressed_and_rooted_at_the_configinfo() {
        let staged = vec![
            ("module".to_owned(), 0, b"changed module".to_vec()),
            ("configinfo".to_owned(), 0, b"new manifest".to_vec()),
        ];
        let (root, rows) = staged_cas_rows(staged).unwrap();
        assert_eq!(root, CasHash::for_packed_bytes(b"new manifest"));
        assert_eq!(rows.len(), 2);
        for row in &rows {
            assert_eq!(row.hash, CasHash::for_packed_bytes(&row.packed));
        }
    }

    #[test]
    fn a_staged_namespace_without_a_configinfo_or_with_parts_is_refused() {
        assert!(staged_cas_rows(vec![("module".to_owned(), 0, b"x".to_vec())]).is_err());
        assert!(staged_cas_rows(vec![("configinfo".to_owned(), 1, b"x".to_vec())]).is_err());
    }

    #[test]
    fn atomic_publish_replaces_the_whole_tree() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-extension-publish-test-{}",
            Uuid::new_v4()
        ));
        let output = root.join("output");
        let staging = root.join("staging");
        fs::create_dir_all(&output).unwrap();
        fs::create_dir_all(&staging).unwrap();
        fs::write(output.join("stale.txt"), b"stale").unwrap();
        fs::write(staging.join("fresh.txt"), b"fresh").unwrap();

        publish_atomic_tree(&staging, &output, true).unwrap();

        assert!(!output.join("stale.txt").exists());
        assert_eq!(fs::read(output.join("fresh.txt")).unwrap(), b"fresh");
        fs::remove_dir_all(root).unwrap();
    }
}
