//! `infobase config import --db-path` (#374): the import of an XML tree into
//! a file infobase.
//!
//! The platform's own import into a file infobase stages the whole
//! configuration in `CONFIGSAVE` -- every row, `root`, `version`, `versions`
//! and a `deleted` row -- for `config apply` to take. This import builds the
//! same stage from parts that already do it for `.cf` files:
//!
//! - the target's `CONFIG` holds no row of the tree's configuration (an empty
//!   infobase, another configuration): every row is compiled from the tree
//!   (`mssql::base_free_cf`);
//! - it holds the configuration: the tree is loaded onto the target's own
//!   rows as `cf load` loads it onto a `.cf` (`crate::load`), so every object
//!   the tree left alone keeps the target's bytes.
//!
//! The staged state is exported and compared with the tree before anything is
//! written (unless `--no-verify`), and `ibcmd-filedb` writes `CONFIGSAVE` in a
//! locked copy of the data file, keeping the original as a backup.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_filedb::{ConfigRow, Timestamp};

use crate::cli::{InfobaseConfigImportArgs, InfobaseImportStageMode, InfobaseImportVerify};
use crate::infobase::{ConnectionConfig, InfobaseConfigImportReport, absolute_path};
use crate::legacy_version::InfobaseConfigSourceVersion;

/// A stored row longer than this is written as several parts, each carrying
/// the whole row's size, as the platform splits it (10 000 000 bytes,
/// measured on 8.5.1.1150 file infobases).
const PART_BYTES: usize = 10_000_000;

pub(crate) fn import(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<InfobaseConfigImportReport> {
    let data_file = config
        .file_infobase
        .as_deref()
        .ok_or_else(|| anyhow!("no file infobase"))?;
    let (source_root, source_version) = import_source(config, args)?;
    // The lock is taken before the tree is compiled and held until the stage
    // is written: an open infobase is refused at once, and nothing changes
    // CONFIG between the read the stage is built on and the write.
    let mut session = ibcmd_filedb::open_for_write(data_file).map_err(|err| anyhow!("{err}"))?;
    let stage = plan_stage(
        session
            .config_rows("CONFIG")
            .map_err(|err| anyhow!("{err}"))?,
        &source_root,
        source_version,
        args,
    )?;
    let written = session
        .replace("CONFIGSAVE", &stage.rows)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("не удалось записать CONFIGSAVE в {}", data_file.display()))?;
    Ok(stage.report(
        config,
        source_root,
        Some(source_version),
        "file-infobase-configsave-stage",
        (written.rows_before, written.rows_after),
        Some(written.backup),
    ))
}

/// The tree an import reads and the XML version it reads it in.
pub(crate) fn import_source(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<(PathBuf, InfobaseConfigSourceVersion)> {
    if !args.path_prefix.is_empty() {
        bail!(
            "path-prefix is not supported by whole-tree file import; use the verified editor partial import API"
        );
    }
    if !args.files.is_empty() {
        bail!("Частичная загрузка файлов в эту информационную базу пока не поддерживается");
    }
    let source_root = absolute_path(&args.source_dir)?;
    if !source_root.is_dir() {
        bail!(
            "каталог файлов конфигурации не найден: {}",
            args.source_dir.display()
        );
    }
    let source_version = if config.xml_version_given {
        config.legacy_source_version()?
    } else {
        let found = crate::metadata_model::export::tree_version(&source_root)
            .unwrap_or_else(|| InfobaseConfigSourceVersion::V2_20.as_str().to_string());
        <InfobaseConfigSourceVersion as clap::ValueEnum>::from_str(&found, true).map_err(|_| {
            anyhow!(
                "версия XML {found} дерева {} не поддерживается",
                source_root.display()
            )
        })?
    };
    Ok((source_root, source_version))
}

/// The `CONFIGSAVE` stage of an import, built over the target's `CONFIG`
/// rows and checked before anything is written.
pub(crate) struct Stage {
    /// The rows to write, parts split, `deleted` last.
    pub(crate) rows: Vec<ConfigRow>,
    base_free: bool,
    reason: String,
    target_rows: usize,
    verification: Option<crate::mssql::StageVerification>,
}

/// Builds the stage of `source_root` over the target's `CONFIG` rows: the
/// whole configuration compiled from the tree when `CONFIG` holds none of
/// it, else the tree loaded onto its rows (see the module docs).
pub(crate) fn plan_stage(
    config_rows: Vec<ConfigRow>,
    source_root: &Path,
    source_version: InfobaseConfigSourceVersion,
    args: &InfobaseConfigImportArgs,
) -> Result<Stage> {
    let target = crate::mssql_dump::file_infobase_config(config_rows)?;
    let configuration = tree_configuration_uuid(source_root)?;
    let (base_free, reason) = match args.stage_mode {
        InfobaseImportStageMode::BaseFree => (true, "asked for (--base-free)".to_string()),
        InfobaseImportStageMode::Patch => (false, "asked for".to_string()),
        InfobaseImportStageMode::Auto => {
            if target.published.is_empty() {
                (true, "the target's CONFIG is empty".to_string())
            } else if target.published.contains_key(&configuration) {
                (
                    false,
                    format!(
                        "the target's CONFIG ({} rows) holds configuration {configuration}",
                        target.published.len()
                    ),
                )
            } else {
                (
                    true,
                    format!(
                        "the target's CONFIG ({} rows) holds no row of configuration {configuration}",
                        target.published.len()
                    ),
                )
            }
        }
    };
    if !base_free && !target.published.contains_key(&configuration) {
        bail!(
            "в CONFIG информационной базы нет конфигурации {configuration}: загрузите дерево без --patch"
        );
    }
    let crate::mssql_dump::FileInfobaseConfig {
        published,
        stored_names,
    } = target;

    let entries = if base_free {
        crate::mssql::base_free_cf::base_free_entries(source_root, source_version)?
    } else {
        load_onto_target(source_root, published, source_version, args)?
    };
    for service in ["root", "version", "versions"] {
        if !entries.contains_key(service) {
            bail!("стадия загрузки без строки {service}: конфигурация не собрана");
        }
    }

    // The rows of the target the configuration no longer has: the apply
    // removes what the `deleted` row lists.
    let deleted = stored_names
        .iter()
        .filter(|name| !entries.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();

    let verification =
        if crate::mssql::stage_guard_wanted(!matches!(args.verify, InfobaseImportVerify::Off)) {
            let staged = entries
                .iter()
                .map(|(name, bytes)| crate::mssql_dump::StagedRow {
                    file_name: name,
                    bytes,
                })
                .collect::<Vec<_>>();
            Some(crate::mssql::verify_staged_state(
                &crate::mssql::GuardRequest {
                    kind: if base_free {
                        crate::mssql::StageKind::BaseFree
                    } else {
                        crate::mssql::StageKind::Patch
                    },
                    source_root,
                    tree: crate::mssql::TreeFiles::Unlisted,
                    removed: &[],
                    path_prefix: &[],
                    source_version: Some(source_version),
                },
                crate::mssql_dump::StateBase::Nothing,
                &staged,
            )?)
        } else {
            None
        };

    Ok(Stage {
        rows: stage_rows(&entries, &deleted)?,
        base_free,
        reason,
        target_rows: stored_names.len(),
        verification,
    })
}

/// `infobase config load <file.cf>` into a file infobase (#353).
pub(crate) fn load_cf(
    config: &ConnectionConfig,
    input: &Path,
) -> Result<InfobaseConfigImportReport> {
    let data_file = config
        .file_infobase
        .as_deref()
        .ok_or_else(|| anyhow!("no file infobase"))?;
    let mut session = ibcmd_filedb::open_for_write(data_file).map_err(|err| anyhow!("{err}"))?;
    let stage = plan_cf_stage(
        session
            .config_rows("CONFIG")
            .map_err(|err| anyhow!("{err}"))?,
        input,
    )?;
    let written = session
        .replace("CONFIGSAVE", &stage.rows)
        .map_err(|err| anyhow!("{err}"))
        .with_context(|| format!("не удалось записать CONFIGSAVE в {}", data_file.display()))?;
    Ok(stage.load_report(
        config,
        input,
        "file-infobase-configsave-stage",
        (written.rows_before, written.rows_after),
        Some(written.backup),
    ))
}

/// The stage of a `.cf` over the target's `CONFIG` rows: every element of
/// the file as it is (the platform's own load keeps them byte for byte:
/// 8.5.1.1529 into an empty PostgreSQL infobase staged `tiny`'s elements
/// unchanged), then `deleted` naming what the target stores and the file
/// does not. The file replaces the whole main configuration.
pub(crate) fn plan_cf_stage(config_rows: Vec<ConfigRow>, input: &Path) -> Result<Stage> {
    let target = crate::mssql_dump::file_infobase_config(config_rows)?;
    let entries = crate::mssql::cf_load_stage::cf_load_rows(input)?
        .into_iter()
        .map(|row| (row.file_name, row.blob))
        .collect::<BTreeMap<_, _>>();
    let deleted = target
        .stored_names
        .iter()
        .filter(|name| !entries.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    Ok(Stage {
        rows: stage_rows(&entries, &deleted)?,
        base_free: true,
        reason: format!(
            "the .cf replaces the main configuration ({} elements)",
            entries.len()
        ),
        target_rows: target.stored_names.len(),
        verification: None,
    })
}

impl Stage {
    /// What `config load` did once the stage is written.
    pub(crate) fn load_report(
        self,
        config: &ConnectionConfig,
        input: &Path,
        backend: &'static str,
        rows: (usize, usize),
        backup: Option<PathBuf>,
    ) -> InfobaseConfigImportReport {
        let mut report = self.report(config, input.to_path_buf(), None, backend, rows, backup);
        report.operation = "infobase config load";
        report.format = "cf";
        report
    }
}

impl Stage {
    /// What the import did once the stage is written: `rows` is the stage
    /// table's row count before and after.
    pub(crate) fn report(
        self,
        config: &ConnectionConfig,
        source_root: PathBuf,
        source_version: Option<InfobaseConfigSourceVersion>,
        backend: &'static str,
        rows: (usize, usize),
        backup: Option<PathBuf>,
    ) -> InfobaseConfigImportReport {
        InfobaseConfigImportReport {
            operation: "infobase config import",
            backend,
            format: "xml",
            source_version: source_version.map(|version| version.as_str().to_string()),
            dbms: config.dbms.clone(),
            db_server: config.db_server.clone(),
            db_name: config.db_name.clone(),
            db_user: config.db_user.clone(),
            native_config: config.native_config.clone(),
            source_dir: source_root,
            stage_mode: if self.base_free { "base-free" } else { "patch" },
            stage_mode_reason: self.reason,
            files: Vec::new(),
            target_config_rows: self.target_rows as i64,
            staged_rows_before: rows.0 as i64,
            staged_rows_after: rows.1 as i64,
            scripts: Vec::new(),
            backup,
            verification: self.verification,
            overrides: None,
        }
    }
}

/// The tree loaded onto the target's rows, through a `.cf` of them as
/// `cf load` loads a tree onto a file: name -> stored bytes of the result.
fn load_onto_target(
    source_root: &Path,
    published: BTreeMap<String, Vec<u8>>,
    source_version: InfobaseConfigSourceVersion,
    args: &InfobaseConfigImportArgs,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let scratch = scratch_dir(args)?;
    let result = (|| {
        let base = scratch.join("target.cf");
        let output = scratch.join("staged.cf");
        crate::mssql_dump::config_save::write_rows_cf(published, &base, false)
            .context("не удалось собрать .cf из CONFIG файловой базы")?;
        match crate::load::load_onto_base(source_root, &base, &output, source_version) {
            Ok(_) => read_cf_entries(&output),
            // The tree is the target's configuration: the stage is its rows.
            Err(crate::load::LoadError::NoChanges) => read_cf_entries(&base),
            Err(error) => Err(anyhow!("{error}"))
                .context("не удалось загрузить дерево поверх конфигурации файловой базы"),
        }
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn scratch_dir(args: &InfobaseConfigImportArgs) -> Result<PathBuf> {
    let parent = args
        .script_output
        .as_deref()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("ibcmd-rs"));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = parent.join(format!("file-import-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).with_context(|| format!("не удалось создать {}", dir.display()))?;
    Ok(dir)
}

pub(crate) fn read_cf_entries(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let length = bytes.len() as u64;
    let archive = ibcmd_cf::archive::decode_packed_archive(
        std::io::Cursor::new(bytes),
        ibcmd_core::limits::ResourceLimits::for_input_bytes(length),
        ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli")?,
    )
    .map_err(|error| anyhow!("{error}"))
    .with_context(|| format!("failed to decode {}", path.display()))?;
    let mut entries = BTreeMap::new();
    for entry in archive.entries() {
        if entries
            .insert(entry.name().to_string(), entry.payload().to_vec())
            .is_some()
        {
            bail!("{}: entry {} appears twice", path.display(), entry.name());
        }
    }
    Ok(entries)
}

/// `CONFIGSAVE` rows: every entry split into parts, then the `deleted` row,
/// stamped with the current time and no attributes, as the platform's import
/// writes them.
pub(crate) fn stage_rows(
    entries: &BTreeMap<String, Vec<u8>>,
    deleted: &[String],
) -> Result<Vec<ConfigRow>> {
    let now = Timestamp::now_utc();
    let deleted_row = crate::mssql::stage_deleted_row(deleted)?;
    let mut rows = Vec::with_capacity(entries.len() + 1);
    let all = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .chain(std::iter::once(("deleted", deleted_row.as_slice())));
    for (name, bytes) in all {
        let parts: Vec<&[u8]> = if bytes.is_empty() {
            vec![bytes]
        } else {
            bytes.chunks(PART_BYTES).collect()
        };
        for (part_no, part) in parts.into_iter().enumerate() {
            rows.push(ConfigRow {
                file_name: name.to_string(),
                part_no: part_no as u32,
                creation: now,
                modified: now,
                attributes: 0,
                data_size: bytes.len() as u64,
                binary: part.to_vec(),
            });
        }
    }
    Ok(rows)
}

/// The uuid of the configuration a tree describes (its `Configuration.xml`).
fn tree_configuration_uuid(source_root: &Path) -> Result<String> {
    let path = source_root.join("Configuration.xml");
    let xml = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(crate::metadata_model::root::configuration_facts(&xml)
        .with_context(|| format!("failed to read {}", path.display()))?
        .uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_rows_are_split_like_the_platform_splits_them() {
        let mut entries = BTreeMap::new();
        entries.insert("big".to_string(), vec![7u8; PART_BYTES * 2 + 5]);
        entries.insert("small".to_string(), vec![1u8; 3]);
        let rows = stage_rows(&entries, &[]).unwrap();
        let big: Vec<_> = rows.iter().filter(|r| r.file_name == "big").collect();
        assert_eq!(big.len(), 3);
        assert!(
            big.iter()
                .all(|r| r.data_size == (PART_BYTES * 2 + 5) as u64)
        );
        assert_eq!(big[2].binary.len(), 5);
        assert_eq!(big.iter().map(|r| r.part_no).collect::<Vec<_>>(), [0, 1, 2]);
        assert!(
            rows.iter()
                .any(|r| r.file_name == "deleted" && r.part_no == 0)
        );
    }
}
