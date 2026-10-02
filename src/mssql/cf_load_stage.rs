//! The offline half of `infobase config load <file.cf>` (Untru/ibcmd-rs#353):
//! the `.cf` read into the rows a load stages in `ConfigSave`, written as the
//! bulk stage writes any row set -- a bcp native rows file, the prepare
//! script and the apply script (`--script-only`). Nothing here reaches a
//! server: running the scripts, and the `deleted` list a load needs for what
//! the target's `Config` holds and the file does not, stay with SQL Server.
//!
//! The rows are the container's records, one per element, in the
//! container's order, each element's packed bytes as they are: the inverse of
//! `mssql_dump::config_save`, on the same evidence -- an element of a `.cf` is
//! the raw-deflate stream a `Config` row stores, which the platform keeps byte
//! for byte through `config load`, `config apply` and `config save --db`
//! (`tests/fixtures/native-evidence/8.3.27.2214/dcs-form-attributes-conditional-appearance/manifest.json`,
//! `reattested`). A row over the platform's part size is cut into parts
//! (`write_bulk_stage_rows`, `CONFIG_ROW_PART_BYTES`), each part carrying the
//! whole row's `DataSize`, as its own import writes it. The file replaces the
//! whole main configuration, so the apply script is the base-free stage's: it
//! empties `ConfigSave` and inserts every row, `root`, `version` and
//! `versions` among them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_cf::archive::decode_packed_archive;
use ibcmd_cf::payload::{PayloadDecoder, PayloadEncoding};
use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::limits::ResourceLimits;
use serde::Serialize;

use crate::cli::MssqlLoadConfigArgs;

use super::{
    BulkStageRow, CONFIG_ROW_PART_BYTES, build_bulk_stage_prepare_sql, bulk_stage_part_count,
    bulk_stage_paths, bulk_stage_table_name, empty_stage::build_base_free_bulk_stage_apply_sql,
    write_bulk_stage_rows,
};

/// The rows a configuration's stage cannot do without; a `.cf` without them
/// is not a configuration (an extension's `.cfe` holds `configinfo`).
const REQUIRED_ROWS: [&str; 3] = ["root", "version", "versions"];

/// One row of the stage: a container element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfStageRow {
    pub file_name: String,
    /// The element's packed bytes: what `BinaryData` stores.
    pub blob: Vec<u8>,
}

/// What the scripts of a load hold.
#[derive(Debug, Clone, Serialize)]
pub struct LoadConfigReport {
    pub input: PathBuf,
    pub database: String,
    /// Rows staged: one per element of the file.
    pub rows: usize,
    /// `ConfigSave` rows they make (a row over the part size makes several).
    pub parts: usize,
    /// Rows written in more than one part.
    pub multi_part_rows: usize,
    pub packed_bytes: u64,
    /// Whether the file carries the stage's `deleted` row of its own.
    pub carries_deleted_row: bool,
    pub rows_file: PathBuf,
    pub scripts: Vec<PathBuf>,
    /// Always false: this version writes the scripts only.
    pub written_to_database: bool,
}

/// `mssql-load-config --script-only <file.cf>`.
pub fn load_config_command(args: &MssqlLoadConfigArgs) -> Result<LoadConfigReport> {
    if !args.script_only {
        bail!(
            "mssql-load-config writes the stage's rows file and scripts only (--script-only); \
             writing them into ConfigSave is not done by this version"
        );
    }
    if args.database.trim().is_empty() {
        bail!("--database names the database the scripts are written for");
    }
    let rows = cf_stage_rows(&args.input)?;
    let script_output = args.script_output.clone().unwrap_or_else(|| {
        std::env::temp_dir()
            .join("ibcmd-rs")
            .join(format!("{}_load.sql", args.database))
    });
    write_stage_scripts(&args.input, &rows, &args.database, &script_output)
}

/// The rows a load of the `.cf` at `path` stages: one per element, in the
/// container's order, every element a complete raw-deflate stream (checked
/// under the reader's limits, so a damaged file is refused by element name
/// before anything is written).
pub fn cf_stage_rows(path: &Path) -> Result<Vec<CfStageRow>> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let length = file
        .metadata()
        .with_context(|| format!("failed to read {}", path.display()))?
        .len();
    let limits = ResourceLimits::for_input_bytes(length);
    let archive = decode_packed_archive(file, limits, StorageProfileId::parse("storage:cf-cli")?)
        .map_err(|error| anyhow!("{} is not a readable .cf: {error}", path.display()))?;
    let (_, _, entries) = archive.into_parts();
    for required in REQUIRED_ROWS {
        if !entries.iter().any(|entry| entry.name() == required) {
            bail!(
                "{} holds no `{required}` element: it is not a configuration file",
                path.display()
            );
        }
    }
    let mut decoder = PayloadDecoder::new(limits);
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        decoder
            .decode(PayloadEncoding::RawDeflate, entry.payload())
            .map_err(|error| {
                anyhow!(
                    "element {} of {} is not one raw-deflate stream: {error}",
                    entry.name(),
                    path.display()
                )
            })?;
        let (file_name, blob) = entry.into_parts();
        rows.push(CfStageRow { file_name, blob });
    }
    Ok(rows)
}

/// Writes `rows` as the bulk stage of `database`: the rows file and the two
/// scripts, beside `script_output` (named as `mssql-stage-source-objects`
/// names them).
pub fn write_stage_scripts(
    input: &Path,
    rows: &[CfStageRow],
    database: &str,
    script_output: &Path,
) -> Result<LoadConfigReport> {
    let bulk = rows
        .iter()
        .map(|row| BulkStageRow {
            file_name: &row.file_name,
            requires_config_row: false,
            blob: &row.blob,
        })
        .collect::<Vec<_>>();
    let (rows_path, prepare_path, apply_path) =
        bulk_stage_paths(Some(&script_output.to_path_buf()), database);
    if let Some(parent) = rows_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let table = bulk_stage_table_name(database);
    let parts = bulk_stage_part_count(&bulk);
    write_bulk_stage_rows(&rows_path, &bulk)?;
    fs::write(&prepare_path, build_bulk_stage_prepare_sql(&table))
        .with_context(|| format!("failed to write {}", prepare_path.display()))?;
    fs::write(
        &apply_path,
        build_base_free_bulk_stage_apply_sql(database, &table, parts),
    )
    .with_context(|| format!("failed to write {}", apply_path.display()))?;
    Ok(LoadConfigReport {
        input: input.to_path_buf(),
        database: database.to_string(),
        rows: rows.len(),
        parts,
        multi_part_rows: rows
            .iter()
            .filter(|row| row.blob.len() > CONFIG_ROW_PART_BYTES)
            .count(),
        packed_bytes: rows.iter().map(|row| row.blob.len() as u64).sum(),
        carries_deleted_row: rows.iter().any(|row| row.file_name == "deleted"),
        rows_file: rows_path,
        scripts: vec![prepare_path, apply_path],
        written_to_database: false,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{Compression, write::DeflateEncoder};
    use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};

    use super::*;

    fn deflate(bytes: &[u8], level: Compression) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), level);
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    /// The rows of a bcp native file of the bulk stage's table, part by part:
    /// `(FileName, Kind, DataSize, PartNo, BinaryData)`.
    fn read_bcp(bytes: &[u8]) -> Vec<(String, u8, i64, i32, Vec<u8>)> {
        let mut rows = Vec::new();
        let mut at = 0;
        let take = |at: &mut usize, len: usize| {
            let slice = &bytes[*at..*at + len];
            *at += len;
            slice
        };
        while at < bytes.len() {
            let name_len = u16::from_le_bytes(take(&mut at, 2).try_into().unwrap()) as usize;
            let units = take(&mut at, name_len)
                .chunks(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let kind = take(&mut at, 1)[0];
            let data_size = i64::from_le_bytes(take(&mut at, 8).try_into().unwrap());
            let part = i32::from_le_bytes(take(&mut at, 4).try_into().unwrap());
            let len = i64::from_le_bytes(take(&mut at, 8).try_into().unwrap()) as usize;
            let blob = take(&mut at, len).to_vec();
            rows.push((
                String::from_utf16(&units).unwrap(),
                kind,
                data_size,
                part,
                blob,
            ));
        }
        rows
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-cf-load-stage-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_row_over_the_part_size_is_staged_in_parts_of_the_whole_size() {
        let dir = scratch("parts");
        // A body that does not compress: its stored stream is over the
        // platform's part size, so the table keeps it in two parts.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let noise = (0..CONFIG_ROW_PART_BYTES + 4096)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect::<Vec<_>>();
        let big = deflate(&noise, Compression::none());
        assert!(big.len() > CONFIG_ROW_PART_BYTES);
        let small = |text: &str| deflate(text.as_bytes(), Compression::default());
        let elements = [
            ("0a1b.0", big.clone()),
            ("root", small("{2,0a1b,}")),
            ("version", small("{{216,0}}")),
            ("versions", small("{1,1,\"\",0a1b}")),
        ];
        let document = Format15Document::new(
            4,
            elements
                .iter()
                .map(|(name, data)| Format15Element::named(name, Some(data.clone())))
                .collect(),
        );
        let cf = dir.join("big.cf");
        fs::write(&cf, write_format15_to_vec(&document).unwrap()).unwrap();

        let rows = cf_stage_rows(&cf).unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row.file_name.as_str())
                .collect::<Vec<_>>(),
            ["0a1b.0", "root", "version", "versions"]
        );
        let report = write_stage_scripts(&cf, &rows, "Db", &dir.join("load.sql")).unwrap();
        assert_eq!(
            (report.rows, report.parts, report.multi_part_rows),
            (4, 5, 1)
        );
        assert!(!report.carries_deleted_row && !report.written_to_database);

        let staged = read_bcp(&fs::read(&report.rows_file).unwrap());
        assert_eq!(staged.len(), 5);
        let (first, second) = (&staged[0], &staged[1]);
        assert_eq!((first.0.as_str(), first.3), ("0a1b.0", 0));
        assert_eq!((second.0.as_str(), second.3), ("0a1b.0", 1));
        assert_eq!(first.2, big.len() as i64);
        assert_eq!(second.2, big.len() as i64);
        assert_eq!(first.4.len(), CONFIG_ROW_PART_BYTES);
        assert_eq!([first.4.clone(), second.4.clone()].concat(), big);
        for (row, (name, data)) in staged[2..].iter().zip(&elements[1..]) {
            assert_eq!(
                (row.0.as_str(), row.1, row.2, row.3),
                (*name, 0, data.len() as i64, 0)
            );
            assert_eq!(&row.4, data);
        }
        let apply = fs::read_to_string(&report.scripts[1]).unwrap();
        assert!(apply.contains("USE [Db];"), "{apply}");
        assert!(apply.contains(") <> 5\n"), "{apply}");
        assert!(apply.contains("DELETE FROM dbo.ConfigSave;"), "{apply}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_no_configuration_or_is_damaged_is_refused() {
        let dir = scratch("refused");
        let cf = dir.join("extension.cfe");
        let document = Format15Document::new(
            1,
            vec![Format15Element::named(
                "configinfo",
                Some(deflate(b"{}", Compression::default())),
            )],
        );
        fs::write(&cf, write_format15_to_vec(&document).unwrap()).unwrap();
        let error = cf_stage_rows(&cf).unwrap_err().to_string();
        assert!(error.contains("`root`"), "{error}");

        let damaged = dir.join("damaged.cf");
        let document = Format15Document::new(
            3,
            ["root", "version", "versions"]
                .into_iter()
                .map(|name| {
                    Format15Element::named(
                        name,
                        Some(if name == "version" {
                            b"not deflate".to_vec()
                        } else {
                            deflate(b"{}", Compression::default())
                        }),
                    )
                })
                .collect(),
        );
        fs::write(&damaged, write_format15_to_vec(&document).unwrap()).unwrap();
        let error = cf_stage_rows(&damaged).unwrap_err().to_string();
        assert!(error.contains("element version"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }
}
