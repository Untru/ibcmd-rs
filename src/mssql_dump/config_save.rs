//! `infobase config save` (Untru/ibcmd-rs#352): the configuration a storage
//! table publishes, written as a `.cf` straight from its rows -- no XML in
//! between. The drop-in command line (`crate::dropin`, through
//! `crate::infobase::save_config`) and the research command
//! `mssql-save-config` (SQL Server or `--rows-dir`) both run [`save_config`].
//!
//! What the container holds, and the evidence for each rule:
//!
//! - **One element per row, its stored bytes as they are.** A `Config` row's
//!   `BinaryData` is the raw-deflate stream a `.cf` element holds: the
//!   platform keeps a form body byte for byte through `config load`,
//!   `config apply` and `config save --db`
//!   (`tests/fixtures/native-evidence/8.3.27.2214/dcs-form-attributes-conditional-appearance/manifest.json`,
//!   `reattested`), and the base-free stage's rows become a container's
//!   entries the same way (`crate::mssql::base_free_cf`, #351).
//! - **A row stored in parts is one element.** `Config` keeps a row over
//!   10 000 000 bytes as `PartNo` 0, 1, ... (`docs/import/patch-mode.md`,
//!   section 7); the element is the parts concatenated, `DataSize` long, as
//!   the export assembles them (`fetch::assemble_binary_config_rows`).
//! - **The published rows, not the stored ones.** The same view the export
//!   reads (`install_storage_overlay`): an active online generation's alias
//!   rows under their plain names, and -- without `--db` -- a completed
//!   import's stage in `ConfigSave` in place of the rows it replaces. The
//!   generation marker `DynamicallyUpdated` says how the table holds the
//!   generations, not what the configuration is, and is left out.
//! - **`deleted` only when the published rows have it.** The platform's
//!   `config save` of an infobase whose import was not applied yet wrote one
//!   (4 unpacked bytes, an empty removal list: the stage's own row,
//!   `config_dump_info::OPTIONAL_SERVICE_NAME`), while none of the 34
//!   platform `.cf` files of `tests/fixtures/native-evidence` (saved from
//!   applied infobases, `config save --db` where the steps are recorded) and
//!   none of the 80 `.cf`/`.cfe` files of `tests/fixtures/external` carries
//!   it. In `Config` itself the row marks an unfinished operation
//!   (`mssql_config_apply::sqlgen::UNFINISHED_NAMES`); such a table, like one
//!   holding `commit` or a `*.new` row, is refused: it needs `config repair`
//!   first.
//! - **Elements in byte order of their names**, as every platform-written
//!   container of the fixtures lists them (102 of 102).
//! - **The container is the base-free bootstrap's Format15**
//!   (`ibcmd_cf::bootstrap`): 8.3.27.2214 loads it (`/LoadCfg`, commit
//!   859d171) and refuses that writer's Format16. The platform itself writes
//!   Format16 behind a Format15 preamble for a configuration in a recent
//!   compatibility mode; that preamble is not reproduced.
//! - **The third word of the file header is the element count.** Every one of
//!   the 102 platform-written containers in the fixtures has it so (a
//!   12-element `.cf` says 12, the 5-entry Format16 preamble says 5); the
//!   bootstrap's default 5 appears only in containers built without the
//!   platform (zero header times).
//! - **Element headers carry no time** (zeros), as the bootstrap writes them;
//!   the platform stamps the moment of the save into every header.
//!
//! The byte-level differences from a platform-saved file that remain are
//! listed in `docs/evidence/cf-config-save.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_cf::bootstrap::{BootstrapCfProfile, publish_bootstrap_patch_new};
use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::limits::ResourceLimits;
use ibcmd_core::storage::{
    MultipartIdentity, StorageKey, StoragePatch, StoragePatchEntry, StoragePatchOutcome,
    StoragePatchTarget, StorageProvenance,
};
use ibcmd_v8::format::Revision;
use serde::Serialize;

use crate::adapters::mssql_legacy::MssqlConfigurationTableRole;
use crate::cli::MssqlSaveConfigArgs;

use super::{
    DYNAMIC_UPDATE_MARKER_ROW, STAGE_NEW_SUFFIX, build_dump_file_name_batches, dynamic_generation,
    fetch_binary_rows, fetch_row_headers, install_storage_overlay, offline_rows, sql_password,
};

/// The storage profile the saved container's entries are recorded under.
const STORAGE_PROFILE: &str = "storage:mssql-config-rows";

/// Which configuration a save writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SavedConfiguration {
    /// `config save`: the main configuration -- a completed import's stage in
    /// `ConfigSave` over `Config`, or `Config` when nothing is staged.
    Main,
    /// `config save --db`: the database configuration, `Config` alone.
    Database,
}

/// One save: where the rows come from and where the `.cf` goes.
pub struct ConfigSaveRequest<'a> {
    /// The server; a detached one when `rows_dir` answers every read.
    pub sql: &'a crate::sql::SqlExec,
    pub database: &'a str,
    /// A folder of `<FileName>__part<N>.bin` files read as the `Config`
    /// table instead of a server (`offline_rows`).
    pub rows_dir: Option<&'a Path>,
    pub configuration: SavedConfiguration,
    pub output: &'a Path,
    /// Replace an existing file (the drop-in) instead of refusing it.
    pub overwrite: bool,
}

/// What a save wrote.
#[derive(Debug, Clone, Serialize)]
pub struct ConfigSaveReport {
    pub output: PathBuf,
    pub configuration: SavedConfiguration,
    /// `Config`, `Config+ConfigSave` (a completed stage published over it) or
    /// `rows-dir`.
    pub source: &'static str,
    /// Elements written: one per published row.
    pub elements: usize,
    /// Of them, rows read from the stage in `ConfigSave`.
    pub staged_elements: usize,
    /// Of them, rows an online generation keeps under an alias.
    pub generation_aliases: usize,
    /// Of them, rows the table stores in more than one part.
    pub multi_part_rows: usize,
    /// Published rows that are not configuration and were left out.
    pub left_out: Vec<String>,
    /// The elements' bytes, as stored.
    pub packed_bytes: u64,
    pub revision: &'static str,
    /// The file header's third word: the element count.
    pub storage_word: u32,
    pub bytes_written: u64,
}

/// The rows of a configuration, as a `.cf` holds them.
#[derive(Debug, Default)]
pub(crate) struct PublishedRows {
    /// File name -> stored bytes (parts concatenated), in byte order.
    pub rows: BTreeMap<String, Vec<u8>>,
    pub staged: usize,
    pub generation_aliases: usize,
    pub multi_part: usize,
    pub left_out: Vec<String>,
}

/// `mssql-save-config`: the research command over [`save_config`].
pub fn save_config_command(args: &MssqlSaveConfigArgs) -> Result<ConfigSaveReport> {
    let configuration = if args.db {
        SavedConfiguration::Database
    } else {
        SavedConfiguration::Main
    };
    let sql = match &args.rows_dir {
        Some(_) => crate::sql::SqlExec::detached("--rows-dir reads every row from its folder"),
        None => {
            if args.database.trim().is_empty() {
                bail!("--database is required unless --rows-dir is given");
            }
            let password = sql_password(
                args.sql_user.as_deref(),
                args.sql_pwd.as_deref(),
                &args.sql_pwd_env,
            );
            crate::sql::SqlExec::from_options(crate::sql::SqlOptions {
                sqlcmd: args.sqlcmd.as_deref(),
                bcp: args.bcp_executable.as_deref(),
                server: &args.server,
                user: args.sql_user.as_deref(),
                password: password.as_deref(),
                password_env: &args.sql_pwd_env,
                trust_server_certificate: true,
            })?
        }
    };
    save_config(&ConfigSaveRequest {
        sql: &sql,
        database: &args.database,
        rows_dir: args.rows_dir.as_deref(),
        configuration,
        output: &args.output,
        overwrite: args.overwrite,
    })
}

/// Reads the rows the request names and writes them as a `.cf`.
pub fn save_config(request: &ConfigSaveRequest<'_>) -> Result<ConfigSaveReport> {
    // The view this save resolves ends with it (#409 F-2), as an export's.
    let _views = dynamic_generation::StorageViewScope::begin(request.database);
    let _offline = request.rows_dir.map(offline_rows::activate).transpose()?;
    let published = published_rows(
        request.sql,
        request.database,
        request.configuration == SavedConfiguration::Main,
    )?;
    let source = if request.rows_dir.is_some() {
        "rows-dir"
    } else if published.staged > 0 {
        "Config+ConfigSave"
    } else {
        "Config"
    };
    let PublishedRows {
        rows,
        staged,
        generation_aliases,
        multi_part,
        left_out,
    } = published;
    let written = write_rows_cf(rows, request.output, request.overwrite)?;
    Ok(ConfigSaveReport {
        output: request.output.to_path_buf(),
        configuration: request.configuration,
        source,
        elements: written.elements,
        staged_elements: staged,
        generation_aliases,
        multi_part_rows: multi_part,
        left_out,
        packed_bytes: written.packed_bytes,
        revision: "format15",
        storage_word: written.storage_word,
        bytes_written: written.bytes_written,
    })
}

/// The rows `Config` of `database` publishes -- with a completed stage of
/// `ConfigSave` over it when `main_configuration` -- each whole.
pub(crate) fn published_rows(
    sql: &crate::sql::SqlExec,
    database: &str,
    main_configuration: bool,
) -> Result<PublishedRows> {
    let table = MssqlConfigurationTableRole::Current.sql_name();
    let stored = fetch_row_headers(sql, database, table, &BTreeSet::new())?;
    refuse_unfinished_operation(stored.iter().map(|row| row.file_name.as_str()))?;
    let headers = install_storage_overlay(
        sql,
        database,
        table,
        &BTreeSet::new(),
        stored,
        main_configuration,
    )?;
    let overlay = dynamic_generation::storage_generation_overlay_for(database, table);
    let staged = overlay
        .as_ref()
        .and_then(|overlay| overlay.staged_names().map(BTreeSet::len))
        .unwrap_or(0);
    let generation_aliases = overlay
        .as_ref()
        .map_or(0, |overlay| overlay.renames().len());

    let mut names = BTreeSet::new();
    let mut multi_part = BTreeSet::new();
    let mut left_out = BTreeSet::new();
    for header in &headers {
        if header.file_name == DYNAMIC_UPDATE_MARKER_ROW {
            left_out.insert(header.file_name.clone());
            continue;
        }
        if header.part_no > 0 {
            multi_part.insert(header.file_name.clone());
        }
        names.insert(header.file_name.clone());
    }
    let rows = read_rows(sql, database, table, &headers, &names)?;
    Ok(PublishedRows {
        rows,
        staged,
        generation_aliases,
        multi_part: multi_part.len(),
        left_out: left_out.into_iter().collect(),
    })
}

/// A table that holds the rows of an operation the platform did not finish
/// is no configuration to save: the apply refuses it for the same names and
/// sends it to `config repair` (`mssql_config_apply`, `UNFINISHED_NAMES`).
fn refuse_unfinished_operation<'a>(stored: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let unfinished = stored
        .into_iter()
        .filter(|name| {
            crate::mssql_config_apply::sqlgen::UNFINISHED_NAMES.contains(name)
                || name.ends_with(STAGE_NEW_SUFFIX)
        })
        .collect::<BTreeSet<_>>();
    if unfinished.is_empty() {
        return Ok(());
    }
    bail!(
        "в таблице Config остались строки незавершенной операции ({}); сначала выполните `ibcmd infobase config repair`",
        unfinished.into_iter().collect::<Vec<_>>().join(", ")
    )
}

/// Every row of `names`, assembled from its parts. The batches are the
/// export's (contiguous names, bounded bytes); a batch is read by its name
/// range, which the server compares in its own collation, so a row the range
/// left out (or brought in from another batch) is read again by name.
fn read_rows(
    sql: &crate::sql::SqlExec,
    database: &str,
    table: &str,
    headers: &[super::ConfigRowHeader],
    names: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut rows = BTreeMap::new();
    for batch in build_dump_file_name_batches(headers, names) {
        let batch = batch.into_iter().collect::<BTreeSet<_>>();
        for row in fetch_binary_rows(sql, database, table, &batch, true)? {
            if batch.contains(&row.file_name) {
                rows.insert(row.file_name, row.binary);
            }
        }
    }
    let missing = names
        .iter()
        .filter(|name| !rows.contains_key(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing.is_empty() {
        for row in fetch_binary_rows(sql, database, table, &missing, false)? {
            if missing.contains(&row.file_name) {
                rows.insert(row.file_name, row.binary);
            }
        }
    }
    if let Some(name) = names.iter().find(|name| !rows.contains_key(*name)) {
        bail!("the row {name} of {database}.{table} is listed but could not be read");
    }
    Ok(rows)
}

/// What [`write_rows_cf`] published.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RowsCf {
    pub elements: usize,
    pub packed_bytes: u64,
    pub storage_word: u32,
    pub bytes_written: u64,
}

/// Writes `rows` (file name -> stored bytes) as a Format15 `.cf` at `output`:
/// one element per row in byte order of the names, the bytes as they are.
///
/// The bootstrap publication does the work (`ibcmd_cf::bootstrap`): every
/// element must be one complete raw-deflate stream and `root`, `version` and
/// `versions` must be there before a byte is written; the file is written
/// beside `output`, reopened and compared element by element, and only then
/// takes the name. An existing `output` is refused unless `overwrite`, which
/// replaces it only once the new file is complete.
pub(crate) fn write_rows_cf(
    rows: BTreeMap<String, Vec<u8>>,
    output: &Path,
    overwrite: bool,
) -> Result<RowsCf> {
    let elements = rows.len();
    let packed_bytes = rows.values().map(|bytes| bytes.len() as u64).sum::<u64>();
    let storage_word =
        u32::try_from(elements).map_err(|_| anyhow!("{elements} rows do not fit one container"))?;
    let mut entries = Vec::with_capacity(elements);
    for (name, bytes) in rows {
        entries.push(StoragePatchEntry::new(
            StoragePatchTarget::new(
                StorageKey::new(&name).with_context(|| format!("row name {name}"))?,
                MultipartIdentity::single(),
                StorageProvenance::new(&format!("config-save:{name}"))?,
            ),
            StoragePatchOutcome::compiled(bytes).with_context(|| format!("row {name}"))?,
        ));
    }
    // The patch keeps every row's bytes once, plus keys and digests.
    let budget = usize::try_from(packed_bytes)
        .unwrap_or(usize::MAX)
        .saturating_mul(2);
    let patch = StoragePatch::with_retained_byte_limit(entries, budget)?;
    let profile = BootstrapCfProfile::new(
        Revision::Format15,
        storage_word,
        StorageProfileId::parse(STORAGE_PROFILE)?,
    );
    let limits = ResourceLimits::for_input_bytes(packed_bytes);
    let report = publish(patch, profile, output, limits, overwrite)?;
    Ok(RowsCf {
        elements,
        packed_bytes,
        storage_word,
        bytes_written: report.published_bytes,
    })
}

fn publish(
    patch: StoragePatch,
    profile: BootstrapCfProfile,
    output: &Path,
    limits: ResourceLimits,
    overwrite: bool,
) -> Result<ibcmd_cf::bootstrap::AtomicBootstrapReport> {
    let failed = |error: ibcmd_cf::bootstrap::BootstrapError| {
        anyhow!(
            "не удалось записать файл конфигурации {}: {error}",
            output.display()
        )
    };
    if output.is_dir() {
        bail!("{} является каталогом, а не файлом", output.display());
    }
    if !overwrite || !output.exists() {
        return publish_bootstrap_patch_new(patch, profile, output, limits).map_err(failed);
    }
    // The new file is published under a name of its own beside the old one
    // and then renamed over it: the old file stays whole until the new one
    // is complete and checked.
    let file_name = output
        .file_name()
        .ok_or_else(|| anyhow!("{} names no file", output.display()))?;
    let mut aside = std::ffi::OsString::from(".");
    aside.push(file_name);
    aside.push(format!(".ibcmd-rs-save-{}", std::process::id()));
    let aside = output.with_file_name(aside);
    let _ = std::fs::remove_file(&aside);
    let report = publish_bootstrap_patch_new(patch, profile, &aside, limits).map_err(failed)?;
    if let Err(error) = std::fs::rename(&aside, output) {
        let _ = std::fs::remove_file(&aside);
        return Err(error).with_context(|| format!("не удалось заменить {}", output.display()));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unfinished_operation_is_refused_by_name() {
        assert!(refuse_unfinished_operation(["root", "version", "versions", "a.0"]).is_ok());
        for marker in ["commit", "deleted", "dbStruFinal", "a.0.new"] {
            let error = refuse_unfinished_operation(["root", marker])
                .unwrap_err()
                .to_string();
            assert!(error.contains(marker), "{error}");
            assert!(error.contains("config repair"), "{error}");
        }
    }
}
