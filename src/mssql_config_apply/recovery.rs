//! The recovery artifact: what the apply overwrites, saved before it runs.
//!
//! The apply is one transaction, so a failure leaves the database untouched;
//! the artifact is for taking a *successful* apply back. It holds, in a
//! directory:
//!
//! - `config_replaced.tsv`: one line per `Config` row the staged rows replace
//!   (every part), with its hash and, when its bytes were kept, where they are
//!   (`rows.pack@<offset>+<length>`);
//! - `rows.pack`: the bytes of the rows the apply changes (or of none, with
//!   [`RecoveryBlobs::None`]), one after the other, as they are stored in the
//!   database (already deflated, so a second compression would gain nothing).
//!   One file instead of one per row: a whole-tree stage saves 9 000 rows, and
//!   9 000 small files cost more than their bytes;
//! - `special_rows.tsv` and their bytes (in the pack): the dynamic-update
//!   markers and alias rows the apply folds away;
//! - `removed_rows.tsv` and their bytes (in the pack): the `Config` rows of the
//!   forms and templates a `deleted` list removes;
//! - `files_before.tsv` and `change_registrations_before.tsv`: the
//!   `MobileVersions.dat` head and the `_MessageNo` values it resets;
//! - `params_replaced.tsv` and their bytes: the search-information rows a new
//!   form or template makes the apply rewrite;
//! - `new_registrations.tsv`: the objects it registers and the files it lists;
//! - `manifest.json` and `README.txt`.
//!
//! Taking an apply back is: put the kept rows back into `Config` (delete the
//! names in `config_replaced.tsv`, insert the rows), restore the special rows,
//! `MobileVersions.dat` and the `_MessageNo` values, and stage nothing.
//!
//! Where no directory is named, the artifact goes to
//! `%TEMP%\ibcmd-rs\config-apply-recovery\<database>-<token>` and a successful
//! run removes the older ones of the same database beyond the newest
//! [`KEEP_DEFAULT`] ([`prune`]); a directory the caller names is never touched.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::sql::{SqlClient, SqlValue};

use super::RecoveryBlobs;
use super::model::{RowMeta, quote_ident, quote_string};
use super::objects::NewObjects;
use super::sqlgen::{ParamsRewrite, staged_objects_predicate_with};

pub struct RecoveryRequest<'a> {
    pub database: &'a str,
    pub dir: &'a Path,
    pub token: &'a str,
    pub blobs: RecoveryBlobs,
    pub staged: &'a [RowMeta],
    pub replaced: &'a [RowMeta],
    pub mobile_versions_before: Option<&'a [u8]>,
    pub reset_change_registrations: bool,
    /// The new objects and body rows the apply registers, and the `Params`
    /// rows it rewrites for them.
    pub new_objects: &'a NewObjects,
    /// The forms and templates the stage removes: their `Config` rows are saved.
    pub removals: &'a super::removals::Removals,
    /// The change registrations inserted for nodes that had none, and the owners of the rows a
    /// `deleted` list names.
    pub registration: &'a super::registrations::RegistrationPlan,
    /// Every `Params` row the script rewrites.
    pub params_rewrites: &'a [ParamsRewrite],
    /// Dynamic-only immutable Params preimages captured against the publication plan.
    /// None retains the legacy capture route; Some requires exact rewrite coverage.
    pub bound_params_preimages: Option<&'a [BoundParamsPreimage]>,
    /// The backup taken (or acknowledged) before a restructuring; the artifact
    /// names it.
    pub backup: Option<&'a super::BackupRecord>,
    /// The generation a dynamic apply publishes, when it is one: the artifact then keeps the
    /// `DynamicallyUpdated` markers only (the alias rows of the earlier generations stay where they
    /// are; the apply writes new ones and folds none) and says how to take the generation back.
    pub dynamic_generation: Option<&'a str>,
    /// Exact planned additions to existing file lists, from a preimage CAS-bound by dynamic SQL.
    pub appended_existing_rows: &'a [AppendedExistingRow],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendedExistingRow {
    pub registration_id: String,
    pub key_hex: String,
    pub file_name: String,
}

/// Metadata and raw bytes of the same judged Params row. Private fields prevent a caller
/// from replacing only its bytes or headers after construction.
#[derive(Clone)]
pub struct BoundParamsPreimage {
    meta: RowMeta,
    bytes: Vec<u8>,
}

impl BoundParamsPreimage {
    pub(super) fn new(meta: RowMeta, bytes: Vec<u8>) -> Result<Self> {
        ensure!(
            meta.name.eq_ignore_ascii_case("siVersions")
                && meta.part == 0
                && meta.attributes == 0
                && usize::try_from(meta.data_size).ok() == Some(bytes.len())
                && usize::try_from(meta.byte_len).ok() == Some(bytes.len())
                && bytes.len() <= 16 * 1024
                && meta
                    .sha256
                    .eq_ignore_ascii_case(&super::model::hex_lower(&Sha256::digest(&bytes))),
            "unbound or unmeasured dynamic Params recovery preimage"
        );
        Ok(Self { meta, bytes })
    }
}

fn validate_bound_params(request: &RecoveryRequest<'_>) -> Result<()> {
    if let Some(rows) = request.bound_params_preimages {
        ensure!(
            rows.len() == request.params_rewrites.len(),
            "dynamic Params recovery coverage differs from the plan"
        );
        let mut names = std::collections::HashSet::new();
        for row in rows {
            ensure!(
                names.insert(row.meta.name.to_ascii_lowercase()),
                "duplicate dynamic Params recovery preimage"
            );
            let rewrite = request
                .params_rewrites
                .iter()
                .find(|rewrite| rewrite.file_name.eq_ignore_ascii_case(&row.meta.name))
                .context("dynamic Params recovery preimage has no planned rewrite")?;
            ensure!(
                rewrite.old_data_size == row.meta.data_size
                    && rewrite
                        .old_sha256_hex
                        .eq_ignore_ascii_case(&row.meta.sha256),
                "dynamic Params recovery preimage differs from the planned rewrite"
            );
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct Manifest<'a> {
    schema_version: u32,
    database: &'a str,
    token: &'a str,
    created_unix_seconds: u64,
    staged_rows: usize,
    replaced_rows: usize,
    replaced_rows_saved: usize,
    saved_bytes: u64,
    /// The file that holds the saved bytes.
    pack: &'a str,
    special_rows: usize,
    change_registrations_reset: usize,
    params_rows_saved: usize,
    new_objects: usize,
    /// `_ConfigChngR` rows inserted for nodes that had none (`added_registrations.tsv`).
    registrations_added: usize,
    appended_files: usize,
    /// `Config` rows of removed forms and templates (`removed_rows.tsv`, their bytes in the pack).
    removed_rows: usize,
    blobs: &'a str,
    /// The way back for the tables: a backup file, or the operator's word.
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<&'a super::BackupRecord>,
}

fn tsv(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

/// How many artifacts of one database a successful run leaves in the default
/// directory.
pub const KEEP_DEFAULT: usize = 5;
/// The name of the file that holds the saved bytes.
pub const PACK_FILE: &str = "rows.pack";

/// The saved rows, one after the other in a single file.
struct Pack {
    writer: BufWriter<fs::File>,
    offset: u64,
}

impl Pack {
    fn create(dir: &Path) -> Result<Self> {
        let path = dir.join(PACK_FILE);
        Ok(Self {
            writer: BufWriter::with_capacity(
                1 << 20,
                fs::File::create(&path)
                    .with_context(|| format!("failed to create {}", path.display()))?,
            ),
            offset: 0,
        })
    }

    /// Appends the bytes; returns where they are: `rows.pack@<offset>+<length>`.
    fn add(&mut self, bytes: &[u8]) -> Result<String> {
        let at = format!("{PACK_FILE}@{}+{}", self.offset, bytes.len());
        self.writer
            .write_all(bytes)
            .with_context(|| format!("failed to write {PACK_FILE}"))?;
        self.offset += bytes.len() as u64;
        Ok(at)
    }

    fn finish(mut self) -> Result<()> {
        self.writer.flush().context("failed to flush rows.pack")
    }
}

/// The bytes a `rows.pack@<offset>+<length>` reference names.
pub fn read_pack_entry(dir: &Path, reference: &str) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let (file, range) = reference
        .split_once('@')
        .with_context(|| format!("{reference} is no pack reference"))?;
    let (offset, length) = range
        .split_once('+')
        .with_context(|| format!("{reference} has no length"))?;
    let mut handle = fs::File::open(dir.join(file))
        .with_context(|| format!("failed to open {}", dir.join(file).display()))?;
    handle.seek(SeekFrom::Start(offset.parse()?))?;
    let mut bytes = vec![0u8; length.parse()?];
    handle.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Where the artifact goes when the caller names no directory.
pub fn default_root() -> PathBuf {
    std::env::temp_dir()
        .join("ibcmd-rs")
        .join("config-apply-recovery")
}

/// The artifact directory of one plan of one database under `root`.
pub fn artifact_dir(root: &Path, database: &str, token: &str) -> PathBuf {
    root.join(format!("{}-{token}", super::safe_stem(database)))
}

/// Removes the artifacts of `database` under `root` beyond the newest `keep`
/// (by the time in `manifest.json`, else the directory's own). Only a
/// directory named `<database>-<16 hex digits>` is a candidate: another
/// database, a longer name that starts alike and anything else stay. Returns
/// the removed directories.
pub fn prune(root: &Path, database: &str, keep: usize) -> Result<Vec<PathBuf>> {
    let prefix = format!("{}-", super::safe_stem(database));
    let mut found: Vec<(u64, PathBuf)> = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", root.display()));
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(token) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !path.is_dir()
            || token.len() != 16
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        let created = fs::read(path.join("manifest.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|manifest| manifest["created_unix_seconds"].as_u64())
            .or_else(|| {
                entry
                    .metadata()
                    .ok()?
                    .modified()
                    .ok()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|elapsed| elapsed.as_secs())
            })
            .unwrap_or(0);
        found.push((created, path));
    }
    // newest first; equal times by name, so the choice is stable
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    let doomed = found.split_off(keep.min(found.len()));
    let mut removed = Vec::new();
    for (_, path) in doomed {
        fs::remove_dir_all(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
        removed.push(path);
    }
    Ok(removed)
}

pub fn write_recovery(client: &dyn SqlClient, request: &RecoveryRequest<'_>) -> Result<()> {
    // Validate before creating/replacing artifact files, and before the caller can run SQL.
    validate_bound_params(request)?;
    let db = quote_ident(request.database)?;
    fs::create_dir_all(request.dir)
        .with_context(|| format!("failed to create {}", request.dir.display()))?;
    let mut pack = Pack::create(request.dir)?;
    let mut saved: std::collections::HashMap<(String, i32), String> =
        std::collections::HashMap::new();
    let mut saved_bytes = 0u64;

    // Bytes of the rows the apply changes (the server compares, so unchanged
    // rows never travel).
    if request.blobs == RecoveryBlobs::Changed {
        let query = format!(
            "SELECT c.FileName, c.PartNo, c.BinaryData FROM {db}.dbo.Config c WHERE EXISTS (SELECT 1 FROM {db}.dbo.ConfigSave s WHERE s.FileName = c.FileName) \
             AND NOT EXISTS (SELECT 1 FROM {db}.dbo.ConfigSave s WHERE s.FileName = c.FileName AND s.PartNo = c.PartNo AND s.DataSize = c.DataSize AND s.BinaryData = c.BinaryData) ORDER BY c.FileName, c.PartNo"
        );
        client.read_rows(&query, &[], &mut |mut row| {
            let name = row.take_text(0)?;
            let part = i32::try_from(row.i64(1)?)?;
            let bytes = row.take_binary(2)?;
            let at = pack.add(&bytes)?;
            saved_bytes += bytes.len() as u64;
            saved.insert((name.to_lowercase(), part), at);
            Ok(())
        })?;
    }

    let mut replaced_file = BufWriter::new(
        fs::File::create(request.dir.join("config_replaced.tsv")).context("config_replaced.tsv")?,
    );
    writeln!(
        replaced_file,
        "name\tpart\tdata_size\tbyte_len\tattributes\tcreation\tmodified\tsha256\tfile"
    )?;
    for row in request.replaced {
        writeln!(
            replaced_file,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            tsv(&row.name),
            row.part,
            row.data_size,
            row.byte_len,
            row.attributes,
            row.creation,
            row.modified,
            row.sha256,
            saved.get(&row.key()).map_or("-", String::as_str)
        )?;
    }
    replaced_file.flush()?;

    // The dynamic-update leftovers, always with their bytes (they are small).
    let mut special_file = BufWriter::new(
        fs::File::create(request.dir.join("special_rows.tsv")).context("special_rows.tsv")?,
    );
    writeln!(
        special_file,
        "table\tname\tpart\tattributes\tcreation\tmodified\tfile"
    )?;
    let mut special_count = 0usize;
    // An exclusive apply folds the overlay away, so it keeps every alias row; a dynamic apply adds a
    // generation and folds nothing, so the markers it rewrites are all it has to keep.
    let config_special = if request.dynamic_generation.is_some() {
        "FileName = N'DynamicallyUpdated'"
    } else {
        "FileName = N'DynamicallyUpdated' OR FileName LIKE N'%\\_dynupdate\\_%' ESCAPE N'\\'"
    };
    for (table, filter) in [
        ("Config", config_special.to_owned()),
        ("Params", "FileName = N'DynamicallyUpdated'".to_owned()),
    ] {
        let query = format!(
            "SELECT FileName, PartNo, CONVERT(int, Attributes), CONVERT(varchar(27), Creation, 121), CONVERT(varchar(27), Modified, 121), BinaryData FROM {db}.dbo.{table} WHERE {filter} ORDER BY FileName, PartNo"
        );
        client.read_rows(&query, &[], &mut |mut row| {
            let name = row.take_text(0)?;
            let part = row.i64(1)?;
            let attributes = row.i64(2)?;
            let creation = row.text(3)?.to_owned();
            let modified = row.text(4)?.to_owned();
            let bytes = row.take_binary(5)?;
            let file = pack.add(&bytes)?;
            saved_bytes += bytes.len() as u64;
            special_count += 1;
            writeln!(
                special_file,
                "{table}\t{}\t{part}\t{attributes}\t{creation}\t{modified}\t{file}",
                tsv(&name)
            )?;
            Ok(())
        })?;
    }
    special_file.flush()?;

    // The rows of the removed forms and templates, always with their bytes.
    let mut removed_count = 0usize;
    if !request.removals.rows.is_empty() {
        let mut file = BufWriter::new(
            fs::File::create(request.dir.join("removed_rows.tsv")).context("removed_rows.tsv")?,
        );
        writeln!(file, "name\tpart\tattributes\tcreation\tmodified\tfile")?;
        for chunk in request.removals.rows.chunks(200) {
            let names = chunk
                .iter()
                .map(|name| format!("N'{}'", quote_string(name)))
                .collect::<Vec<_>>()
                .join(", ");
            client.read_rows(
                &format!(
                    "SELECT FileName, PartNo, CONVERT(int, Attributes), CONVERT(varchar(27), Creation, 121), CONVERT(varchar(27), Modified, 121), BinaryData FROM {db}.dbo.Config WHERE FileName IN ({names}) ORDER BY FileName, PartNo"
                ),
                &[],
                &mut |mut row| {
                    let name = row.take_text(0)?;
                    let part = row.i64(1)?;
                    let attributes = row.i64(2)?;
                    let creation = row.text(3)?.to_owned();
                    let modified = row.text(4)?.to_owned();
                    let bytes = row.take_binary(5)?;
                    let stored = pack.add(&bytes)?;
                    saved_bytes += bytes.len() as u64;
                    removed_count += 1;
                    writeln!(
                        file,
                        "{}\t{part}\t{attributes}\t{creation}\t{modified}\t{stored}",
                        tsv(&name)
                    )?;
                    Ok(())
                },
            )?;
        }
        file.flush()?;
    }

    if let Some(bytes) = request.mobile_versions_before {
        fs::write(request.dir.join("MobileVersions.dat.before"), bytes)?;
        saved_bytes += bytes.len() as u64;
    }

    // The `_MessageNo` values the reset overwrites.
    let mut reset_count = 0usize;
    if request.reset_change_registrations {
        let mut file = BufWriter::new(
            fs::File::create(request.dir.join("change_registrations_before.tsv"))
                .context("change_registrations_before.tsv")?,
        );
        writeln!(file, "node_type_ref\tnode_ref\tobject_id\tmessage_no")?;
        let predicate = staged_objects_predicate_with(
            &format!("{db}.dbo."),
            &request.registration.extra_objects,
        );
        let query = format!(
            "SELECT CONVERT(varchar(16), r._NodeTRef, 2), CONVERT(varchar(64), r._NodeRRef, 2), CONVERT(varchar(64), r._MDObjID, 2), CONVERT(bigint, r._MessageNo) FROM {db}.dbo._ConfigChngR r              WHERE r._MessageNo IS NOT NULL AND {predicate} ORDER BY 1, 2, 3"
        );
        client.read_rows(&query, &[], &mut |row| {
            reset_count += 1;
            let message = match row.value(3)? {
                SqlValue::Int(value) => value.to_string(),
                other => other.to_text(),
            };
            writeln!(
                file,
                "{}\t{}\t{}\t{}",
                row.text(0)?,
                row.text(1)?,
                row.text(2)?,
                message
            )?;
            Ok(())
        })?;
        file.flush()?;
    }

    // The Params rows the apply rewrites for new objects (the search
    // information): the bytes they had.
    let mut params_saved = 0usize;
    if !request.params_rewrites.is_empty() {
        let mut file = BufWriter::new(
            fs::File::create(request.dir.join("params_replaced.tsv"))
                .context("params_replaced.tsv")?,
        );
        writeln!(file, "name\tpart\tattributes\tcreation\tmodified\tfile")?;
        if let Some(rows) = request.bound_params_preimages {
            // Never reread a dynamic preimage: an A->B->A change during artifact capture
            // must not save B while the locked publication guard later admits A.
            for row in rows {
                let stored = pack.add(&row.bytes)?;
                saved_bytes += row.bytes.len() as u64;
                params_saved += 1;
                writeln!(
                    file,
                    "{}\t{}\t{}\t{}\t{}\t{stored}",
                    tsv(&row.meta.name),
                    row.meta.part,
                    row.meta.attributes,
                    tsv(&row.meta.creation),
                    tsv(&row.meta.modified)
                )?;
            }
        } else {
            for rewrite in request.params_rewrites {
                client.read_rows(
                &format!(
                    "SELECT PartNo, CONVERT(int, Attributes), CONVERT(varchar(27), Creation, 121), CONVERT(varchar(27), Modified, 121), BinaryData FROM {db}.dbo.Params WHERE FileName = @P1 ORDER BY PartNo"
                ),
                &[crate::sql::SqlParam::Text(&rewrite.file_name)],
                &mut |mut row| {
                    let part = row.i64(0)?;
                    let attributes = row.i64(1)?;
                    let creation = row.text(2)?.to_owned();
                    let modified = row.text(3)?.to_owned();
                    let bytes = row.take_binary(4)?;
                    let stored = pack.add(&bytes)?;
                    saved_bytes += bytes.len() as u64;
                    params_saved += 1;
                    writeln!(
                        file,
                        "{}\t{part}\t{attributes}\t{creation}\t{modified}\t{stored}",
                        tsv(&rewrite.file_name)
                    )?;
                    Ok(())
                },
            )?;
            }
        }
        file.flush()?;
    }

    // What the apply registers for new objects and appends to existing ones.
    let mut new_object_count = 0usize;
    let mut appended_count = 0usize;
    // Explicit directories may be reused: always truncate the recipe, including a zero-addition
    // plan, so an older manifest's triples cannot be mistaken for this plan's additions.
    {
        let mut file = BufWriter::new(fs::File::create(
            request.dir.join("appended_existing_rows.tsv"),
        )?);
        writeln!(file, "registration_id\tkey_hex\tfile_name")?;
        for row in request.appended_existing_rows {
            writeln!(
                file,
                "{}\t{}\t{}",
                row.registration_id,
                row.key_hex,
                tsv(&row.file_name)
            )?;
            appended_count += 1;
        }
        file.flush()?;
    }
    if !request.new_objects.is_empty() {
        let mut file = BufWriter::new(
            fs::File::create(request.dir.join("new_registrations.tsv"))
                .context("new_registrations.tsv")?,
        );
        writeln!(file, "what\tobject\tvalue")?;
        for node in &request.new_objects.nodes {
            writeln!(file, "node\t{}\t{}", node.type_ref, node.reference)?;
        }
        for object in &request.new_objects.objects {
            new_object_count += 1;
            writeln!(
                file,
                "new {}\t{}\t{}",
                object.kind, object.uuid, object.owner
            )?;
            for body in &object.bodies {
                writeln!(file, "file\t{}\t{}", object.uuid, tsv(body))?;
            }
        }
        for body in &request.new_objects.bodies {
            appended_count += 1;
            writeln!(
                file,
                "appended file\t{}\t{}",
                body.object,
                tsv(&body.file_name)
            )?;
        }
        file.flush()?;
    }

    // The change registrations inserted for nodes that had no row: undone by deleting these rows (and
    // their file lists).
    let mut added_registration_count = 0usize;
    if !request.registration.missing.is_empty() {
        let mut file = BufWriter::new(
            fs::File::create(request.dir.join("added_registrations.tsv"))
                .context("added_registrations.tsv")?,
        );
        writeln!(file, "node_type_ref\tnode_ref\tobject_id\tfiles")?;
        for (index, object) in &request.registration.missing {
            let node = &request.registration.nodes[*index];
            let files = request
                .registration
                .additions
                .iter()
                .find(|addition| &addition.object_hex == object)
                .map(|addition| addition.files.join("|"))
                .unwrap_or_default();
            added_registration_count += 1;
            writeln!(
                file,
                "{}\t{}\t{}\t{}",
                node.type_ref,
                node.reference,
                object,
                tsv(&files)
            )?;
        }
        file.flush()?;
    }

    pack.finish()?;

    let manifest = Manifest {
        schema_version: 2,
        database: request.database,
        token: request.token,
        created_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
        staged_rows: request.staged.len(),
        replaced_rows: request.replaced.len(),
        replaced_rows_saved: saved.len(),
        saved_bytes,
        pack: PACK_FILE,
        special_rows: special_count,
        change_registrations_reset: reset_count,
        params_rows_saved: params_saved,
        new_objects: new_object_count,
        registrations_added: added_registration_count,
        appended_files: appended_count,
        removed_rows: removed_count,
        blobs: match request.blobs {
            RecoveryBlobs::Changed => "changed",
            RecoveryBlobs::None => "none",
        },
        backup: request.backup,
    };
    fs::write(
        request.dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let dynamic_note = request.dynamic_generation.map_or_else(String::new, |generation| {
        format!(
            "\nThis artifact describes a PLANNED DYNAMIC apply, saved before execution. Only after confirming\n\
             COMMIT, the generation {generation} was published beside the active rows\n\
             (`<name>_dynupdate_{generation}` in Config, `versions_dynupdate_{generation}`), `root` and\n\
             `version` were replaced in place and the DynamicallyUpdated markers rewritten. A pending-generation apply also\n\
             writes deleted_dynupdate_{generation} in Config and <uuid>_dynupdate_{generation}.si in Params.\n\
             After confirmed COMMIT, to take it back while stopped: delete ONLY this generation's aliases in BOTH\n\
             Config and Params, and restore siVersions from params_replaced.tsv when it was saved,\n\
             put back `root` and `version` from config_replaced.tsv and the markers from special_rows.tsv,\n\
             then MobileVersions.dat, the message numbers and the added registrations as below.\n"
        )
    });
    fs::write(
        request.dir.join("README.txt"),
        format!(
            "ibcmd-rs config apply recovery artifact for database {db}\n\
             token {token}\n\n\
             The apply uses one transaction. A confirmed SQL rollback changes nothing; a connection\n\
             failure can leave the COMMIT outcome uncertain. Inspect storage and markers before retry.\n\
             This directory\n\
             lets a successful run be taken back.\n\n\
             rows.pack             the saved bytes of every row below, one after the other; a `file` value `rows.pack@<offset>+<length>` names a byte range\n\
             config_replaced.tsv   every Config row (all parts) the staged rows replaced; `file` is where its saved bytes are\n\
             special_rows.tsv      the DynamicallyUpdated markers and _dynupdate_ alias rows that were folded away\n\
             removed_rows.tsv      the Config rows of the forms and templates the stage's deleted list removed, with their bytes\n\
             MobileVersions.dat.before   Files.MobileVersions.dat before the new head GUID\n\
             change_registrations_before.tsv   _ConfigChngR rows whose _MessageNo was reset to NULL\n\
             params_replaced.tsv   the search-information rows of Params (and siVersions) rewritten for new forms/templates, with their old bytes\n\
             new_registrations.tsv the new objects registered in _ConfigChngR (per node) and the files listed for them\n\
             added_registrations.tsv the change registrations inserted for nodes that had no row for a changed object (a node with an initial image has none): node, object, files\n\
             appended_existing_rows.tsv exact PLANNED missing entries appended to existing file lists;\n\
             the transaction's script/token checks their exact preimage before publication.\n\
                                       After confirming this generation committed, while stopped, delete\n\
                                       ONLY matching registration_id/key_hex/file_name triples. They may\n\
                                       name OLD generation aliases; keep every preexisting file-list entry.\n\
             manifest.json `backup`  the backup taken before a restructuring (file, seconds) or the operator's word that one exists;\n\
                                   the rebuilt tables come back only from it\n\n\
             To take the apply back: stage the saved rows in ConfigSave (name, part, sizes, attributes,\n\
             creation, modified, bytes), then run `ibcmd-rs mssql-config-apply` (or the native apply)\n\
             on the stopped database; restore special_rows.tsv, MobileVersions.dat, the Params rows and\n\
             the message numbers with plain INSERT/UPDATE statements, and delete the rows of\n\
             new_registrations.tsv from _ConfigChngR and _ConfigChngR_ExtProps. The `sha256` column proves each row.\n\
             Names are quoted: {quoted}\n{dynamic_note}",
            db = request.database,
            token = request.token,
            quoted = quote_string(request.database)
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow};

    struct TransientParamsStorage {
        current: Vec<SqlRow>,
        params_capture_reads: std::sync::atomic::AtomicUsize,
    }

    impl SqlClient for TransientParamsStorage {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("no SQL execution during capture")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("no writes during capture")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("no JSON query")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("no writes during capture")
        }
        fn read_rows(
            &self,
            sql: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            if sql.contains("dbo.Params WHERE FileName = @P1") {
                self.params_capture_reads
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                for row in &self.current {
                    each(row.clone())?;
                }
            }
            Ok(())
        }
    }

    fn planned_si_preimage() -> BoundParamsPreimage {
        let bytes = b"immutable planned A".to_vec();
        BoundParamsPreimage::new(
            RowMeta {
                name: "siVersions".to_owned(),
                part: 0,
                data_size: bytes.len() as i64,
                byte_len: bytes.len() as i64,
                attributes: 0,
                creation: "4026-10-01 12:00:00.000000".to_owned(),
                modified: "4026-10-01 12:01:00.000000".to_owned(),
                sha256: super::super::model::hex_lower(&Sha256::digest(&bytes)),
            },
            bytes,
        )
        .unwrap()
    }

    #[test]
    fn immutable_dynamic_params_recovery_preserves_a_through_transient_b_missing_parts_and_headers()
    {
        let planned = planned_si_preimage();
        let capture = |part, flags, creation: &str, modified: &str, bytes: &[u8]| SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Int(part),
                SqlValue::Int(flags),
                SqlValue::Text(creation.to_owned()),
                SqlValue::Text(modified.to_owned()),
                SqlValue::Binary(bytes.to_vec()),
            ],
        };
        let a = capture(
            0,
            0,
            &planned.meta.creation,
            &planned.meta.modified,
            &planned.bytes,
        );
        let cases = [
            ("a", vec![a.clone()]),
            (
                "different-bytes-b",
                vec![capture(
                    0,
                    0,
                    &planned.meta.creation,
                    &planned.meta.modified,
                    b"transient wrong B",
                )],
            ),
            ("missing", vec![]),
            (
                "multipart",
                vec![
                    a.clone(),
                    capture(
                        1,
                        0,
                        &planned.meta.creation,
                        &planned.meta.modified,
                        b"extra part",
                    ),
                ],
            ),
            (
                "flags",
                vec![capture(
                    0,
                    1,
                    &planned.meta.creation,
                    &planned.meta.modified,
                    &planned.bytes,
                )],
            ),
            (
                "creation",
                vec![capture(
                    0,
                    0,
                    "changed creation",
                    &planned.meta.modified,
                    &planned.bytes,
                )],
            ),
            (
                "modified",
                vec![capture(
                    0,
                    0,
                    &planned.meta.creation,
                    "changed modified",
                    &planned.bytes,
                )],
            ),
        ];
        for (case, current) in cases {
            let client = TransientParamsStorage {
                current,
                params_capture_reads: std::sync::atomic::AtomicUsize::new(0),
            };
            let dir = scratch(&format!("immutable-params-{case}"));
            let new_objects = NewObjects::default();
            let removals = super::super::removals::Removals::default();
            let registration = super::super::registrations::RegistrationPlan::default();
            let rewrites = [ParamsRewrite {
                file_name: planned.meta.name.clone(),
                old_data_size: planned.meta.data_size,
                old_sha256_hex: planned.meta.sha256.clone(),
                new_bytes: b"new value".to_vec(),
                set_creation: false,
            }];
            let preimages = [planned.clone()];
            let request = RecoveryRequest {
                database: "labdb",
                dir: &dir,
                token: case,
                blobs: RecoveryBlobs::None,
                staged: &[],
                replaced: &[],
                mobile_versions_before: None,
                reset_change_registrations: false,
                new_objects: &new_objects,
                removals: &removals,
                registration: &registration,
                params_rewrites: &rewrites,
                bound_params_preimages: Some(&preimages),
                backup: None,
                dynamic_generation: Some("11111111-1111-4111-8111-111111111111"),
                appended_existing_rows: &[],
            };
            write_recovery(&client, &request).unwrap();
            let tsv = fs::read_to_string(dir.join("params_replaced.tsv")).unwrap();
            let entries: Vec<_> = tsv.lines().skip(1).collect();
            assert_eq!(entries.len(), 1, "{case}");
            let fields: Vec<_> = entries[0].split('\t').collect();
            assert_eq!(
                &fields[..5],
                &[
                    "siVersions",
                    "0",
                    "0",
                    &planned.meta.creation,
                    &planned.meta.modified
                ],
                "{case}"
            );
            assert_eq!(
                read_pack_entry(&dir, fields[5]).unwrap(),
                planned.bytes,
                "{case}"
            );
            assert_eq!(
                client
                    .params_capture_reads
                    .load(std::sync::atomic::Ordering::Relaxed),
                0,
                "never save transient storage instead of A: {case}"
            );
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn dynamic_params_recovery_rejects_unbound_metadata_and_missing_or_duplicate_coverage() {
        let planned = planned_si_preimage();
        for change in ["digest", "size", "length", "part", "flags"] {
            let mut meta = planned.meta.clone();
            match change {
                "digest" => meta.sha256 = "00".repeat(32),
                "size" => meta.data_size += 1,
                "length" => meta.byte_len += 1,
                "part" => meta.part = 1,
                _ => meta.attributes = 1,
            }
            assert!(
                BoundParamsPreimage::new(meta, planned.bytes.clone()).is_err(),
                "{change}"
            );
        }
        let dir = scratch("immutable-params-bad-coverage");
        let new_objects = NewObjects::default();
        let removals = super::super::removals::Removals::default();
        let registration = super::super::registrations::RegistrationPlan::default();
        let rewrites = vec![ParamsRewrite {
            file_name: planned.meta.name.clone(),
            old_data_size: planned.meta.data_size,
            old_sha256_hex: planned.meta.sha256.clone(),
            new_bytes: vec![1],
            set_creation: false,
        }];
        for images in [vec![], vec![planned.clone(), planned.clone()]] {
            let request = RecoveryRequest {
                database: "labdb",
                dir: &dir,
                token: "invalid",
                blobs: RecoveryBlobs::None,
                staged: &[],
                replaced: &[],
                mobile_versions_before: None,
                reset_change_registrations: false,
                new_objects: &new_objects,
                removals: &removals,
                registration: &registration,
                params_rewrites: &rewrites,
                bound_params_preimages: Some(&images),
                backup: None,
                dynamic_generation: None,
                appended_existing_rows: &[],
            };
            assert!(write_recovery(&EmptyRecoveryStorage, &request).is_err());
            assert!(!dir.join("rows.pack").exists());
        }
        for change in ["name", "size", "digest"] {
            let mut rewrite = rewrites[0].clone();
            match change {
                "name" => rewrite.file_name = "other.si".to_owned(),
                "size" => rewrite.old_data_size += 1,
                _ => rewrite.old_sha256_hex = "00".repeat(32),
            }
            let rewrites = vec![rewrite];
            let images = [planned.clone()];
            let request = RecoveryRequest {
                database: "labdb",
                dir: &dir,
                token: "invalid",
                blobs: RecoveryBlobs::None,
                staged: &[],
                replaced: &[],
                mobile_versions_before: None,
                reset_change_registrations: false,
                new_objects: &new_objects,
                removals: &removals,
                registration: &registration,
                params_rewrites: &rewrites,
                bound_params_preimages: Some(&images),
                backup: None,
                dynamic_generation: None,
                appended_existing_rows: &[],
            };
            assert!(
                write_recovery(&EmptyRecoveryStorage, &request).is_err(),
                "{change}"
            );
            assert!(!dir.join("rows.pack").exists());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    struct EmptyRecoveryStorage;
    impl SqlClient for EmptyRecoveryStorage {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("artifact only")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("artifact only")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("artifact only")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("artifact only")
        }
        fn read_rows(
            &self,
            _: &str,
            _: &[SqlParam<'_>],
            _: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn reused_recovery_directory_replaces_old_append_triples_with_empty_recipe() {
        let workspace = std::env::current_dir().unwrap().canonicalize().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = workspace.join("target").join(format!(
            "recovery-appends-reuse-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let new_objects = NewObjects::default();
        let removals = super::super::removals::Removals::default();
        let registration = super::super::registrations::RegistrationPlan::default();
        let rows = [AppendedExistingRow {
            registration_id: "11".repeat(16),
            key_hex: "00000002".to_owned(),
            file_name: "old_generation_alias.0".to_owned(),
        }];
        let mut request = RecoveryRequest {
            database: "labdb",
            dir: &dir,
            token: "first",
            blobs: RecoveryBlobs::None,
            staged: &[],
            replaced: &[],
            mobile_versions_before: None,
            reset_change_registrations: false,
            new_objects: &new_objects,
            removals: &removals,
            registration: &registration,
            params_rewrites: &[],
            bound_params_preimages: None,
            backup: None,
            dynamic_generation: Some("11111111-1111-4111-8111-111111111111"),
            appended_existing_rows: &rows,
        };
        write_recovery(&EmptyRecoveryStorage, &request).unwrap();
        assert!(
            fs::read_to_string(dir.join("appended_existing_rows.tsv"))
                .unwrap()
                .contains("old_generation_alias.0")
        );
        let first: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(first["appended_files"], 1);
        request.token = "second";
        request.appended_existing_rows = &[];
        write_recovery(&EmptyRecoveryStorage, &request).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("appended_existing_rows.tsv")).unwrap(),
            "registration_id\tkey_hex\tfile_name\n"
        );
        let second: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(second["appended_files"], 0);
        assert_eq!(second["token"], "second");
        assert!(dir.canonicalize().unwrap().starts_with(&workspace));
        fs::remove_dir_all(dir).unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-recovery-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn artifact(root: &Path, database: &str, token: &str, created: u64) -> PathBuf {
        let dir = artifact_dir(root, database, token);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("manifest.json"),
            format!("{{\"created_unix_seconds\": {created}}}"),
        )
        .unwrap();
        fs::write(dir.join(PACK_FILE), b"x").unwrap();
        dir
    }

    #[test]
    fn the_pack_hands_back_the_bytes_by_their_reference() {
        let dir = scratch("pack");
        let mut pack = Pack::create(&dir).unwrap();
        let first = pack.add(b"hello").unwrap();
        let second = pack.add(&[0u8, 1, 2, 255]).unwrap();
        let empty = pack.add(b"").unwrap();
        pack.finish().unwrap();
        assert_eq!(first, "rows.pack@0+5");
        assert_eq!(second, "rows.pack@5+4");
        assert_eq!(empty, "rows.pack@9+0");
        assert_eq!(read_pack_entry(&dir, &first).unwrap(), b"hello");
        assert_eq!(
            read_pack_entry(&dir, &second).unwrap(),
            vec![0u8, 1, 2, 255]
        );
        assert!(read_pack_entry(&dir, &empty).unwrap().is_empty());
        assert!(read_pack_entry(&dir, "no-reference").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_the_newest_artifacts_of_the_database_and_nothing_else_goes() {
        let root = scratch("prune");
        let ours: Vec<PathBuf> = (0..7u64)
            .map(|index| artifact(&root, "labdb", &format!("{index:016x}"), 100 + index))
            .collect();
        // another database whose name starts alike, and things that are no artifact
        let longer: Vec<PathBuf> = (0..3u64)
            .map(|index| artifact(&root, "labdb-2", &format!("{index:016x}"), 1 + index))
            .collect();
        fs::create_dir_all(root.join("labdb-notatoken")).unwrap();
        fs::create_dir_all(root.join("labdb-00000000000000zz")).unwrap();
        fs::write(
            root.join("labdb-0000000000000009"),
            b"a file, not a directory",
        )
        .unwrap();

        let removed = prune(&root, "labdb", 5).unwrap();
        assert_eq!(removed.len(), 2);
        // the two oldest are gone, the five newest stay
        assert!(!ours[0].exists() && !ours[1].exists());
        assert!(ours[2..].iter().all(|dir| dir.exists()));
        // nothing of the other database or of the odd names is touched
        assert!(longer.iter().all(|dir| dir.exists()));
        assert!(root.join("labdb-notatoken").exists());
        assert!(root.join("labdb-00000000000000zz").exists());
        assert!(root.join("labdb-0000000000000009").exists());
        // a second run finds nothing more; a missing root is not an error
        assert!(prune(&root, "labdb", 5).unwrap().is_empty());
        assert!(prune(&root.join("absent"), "labdb", 5).unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prune_orders_by_the_manifest_time_not_by_the_name() {
        let root = scratch("order");
        // the newest artifact has the smallest token
        let newest = artifact(&root, "db", "0000000000000000", 900);
        let old_a = artifact(&root, "db", "ffffffffffffffff", 100);
        let old_b = artifact(&root, "db", "eeeeeeeeeeeeeeee", 200);
        let removed = prune(&root, "db", 1).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(newest.exists() && !old_a.exists() && !old_b.exists());
        let _ = fs::remove_dir_all(&root);
    }
}
