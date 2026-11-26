//! The size guard of the own restructuring (S1-J, #406; `docs/apply/restructure-size-limit.md`).
//!
//! The structure phase copies every rebuilt table into its `NG` twin, builds the indexes, drops the old
//! table and renames, all in the apply's one transaction. Under full recovery the log of that work is held
//! until `COMMIT`, and it is `2 x data + indexes` of the rebuilt tables (measured at 0.52 to 1.05 of
//! it on 17 runs and probes, 10 thousand to 4.8 million rows, four row shapes and real tables; section 4): the heap load
//! writes the data once, the clustered index built over the heap once more, the other indexes once. A
//! rollback writes back part of it. A stage above the limit is therefore not rebuilt this way: it is refused
//! and the operator is sent to the native `config apply`, which commits in steps. Copying in chunks is 0.5.
//!
//! The check is a gate-side one. The tables to rebuild are known only after the plan, and their sizes are
//! database state, so [`check_tables`] runs after the gate has its plan (`s1::S1Gate::check`, and the
//! standalone `mssql-restructure`), over the tables the plan rebuilds, whatever the operation. The limit is
//! on the **sum** over the stage, because one transaction rebuilds every table of the stage; the refusal
//! names the largest table, the totals and the limit. [`evaluate`] is pure. The byte limit is on the bytes
//! the rebuild writes ([`TableSize::rebuild_bytes`]), not on the size of the tables.
//!
//! The limit is a default (measured) that the operator may raise or lower: the flags
//! `--restructure-limit-rows` / `--restructure-limit-bytes` of `mssql-config-apply` and
//! `mssql-restructure`, and for every command the settings chain (`IBCMD_RS_RESTRUCTURE_LIMIT_ROWS` /
//! `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES`, the keys `restructure-limit-rows` / `restructure-limit-bytes` of
//! `ibcmd-rs.toml`), in that order (`resolve_limit`). A raised limit changes nothing else: the apply still
//! refuses a restructuring without a backup option.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::json;

use crate::restructure::reader::RowSource;

/// Rows of the rebuilt tables a stage may hold (the sum), by default. The byte limit binds first for every
/// row of 210 bytes or more; this one is for the thin rows, whose log the byte model reads low.
pub const DEFAULT_LIMIT_ROWS: u64 = 10_000_000;
/// Bytes the rebuild of a stage may write ([`TableSize::rebuild_bytes`], the sum), by default: 2 GiB, half of
/// the 4 GiB log the limit is derived from (docs/apply/restructure-size-limit.md, section 4).
pub const DEFAULT_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// The log of the rebuild under full recovery per byte of [`TableSize::rebuild_bytes`]: the worst measured is
/// 1.05 (17 runs and probes, 0.52 to 1.05), rounded up to 1.10. Used to say what a refused stage would have cost.
pub const LOG_PER_REBUILD_BYTE: f64 = 1.10;
/// What [`LimitSetting`] says of a limit nobody named.
pub const DEFAULT_SOURCE: &str = "the default";

/// The limit: refuse when a stage rebuilds more than this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RestructureLimit {
    pub rows: u64,
    pub bytes: u64,
}

impl Default for RestructureLimit {
    fn default() -> Self {
        Self {
            rows: DEFAULT_LIMIT_ROWS,
            bytes: DEFAULT_LIMIT_BYTES,
        }
    }
}

/// The limit and where each half of it came from (a flag, a variable, a settings file, the default).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LimitSetting {
    pub limit: RestructureLimit,
    pub rows_source: String,
    pub bytes_source: String,
}

impl Default for LimitSetting {
    fn default() -> Self {
        Self {
            limit: RestructureLimit::default(),
            rows_source: DEFAULT_SOURCE.to_owned(),
            bytes_source: DEFAULT_SOURCE.to_owned(),
        }
    }
}

/// What one table holds: the rows and the bytes of its data (heap or clustered index, LOB pages included)
/// and of its other indexes, from `sys.dm_db_partition_stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TableSize {
    pub rows: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
}

impl TableSize {
    /// What the table and its indexes hold.
    pub fn bytes(&self) -> u64 {
        self.data_bytes.saturating_add(self.index_bytes)
    }

    /// What the rebuild writes to the log under full recovery: the data twice (the load into a heap, then
    /// the clustered index built over it) and the other indexes once.
    pub fn rebuild_bytes(&self) -> u64 {
        self.data_bytes
            .saturating_mul(2)
            .saturating_add(self.index_bytes)
    }
}

/// The verdict on a stage: the totals against the limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizeCheck {
    pub setting: LimitSetting,
    pub tables: BTreeMap<String, TableSize>,
    pub total_rows: u64,
    /// The tables and their indexes, as they are.
    pub total_bytes: u64,
    /// What the rebuild writes (`2 x data + indexes`): the figure the byte limit is on.
    pub total_rebuild_bytes: u64,
    pub total_data_bytes: u64,
    pub total_index_bytes: u64,
    /// The table with the most to write (ties: the first by name).
    pub largest: Option<(String, TableSize)>,
    pub over_rows: bool,
    pub over_bytes: bool,
}

/// Exactly at the limit passes; one row or one byte above refuses.
pub fn evaluate(tables: BTreeMap<String, TableSize>, setting: &LimitSetting) -> SizeCheck {
    let mut total_rows = 0u64;
    let mut total_data = 0u64;
    let mut total_index = 0u64;
    let mut largest: Option<(String, TableSize)> = None;
    for (name, size) in &tables {
        total_rows = total_rows.saturating_add(size.rows);
        total_data = total_data.saturating_add(size.data_bytes);
        total_index = total_index.saturating_add(size.index_bytes);
        if largest
            .as_ref()
            .is_none_or(|(_, best)| size.rebuild_bytes() > best.rebuild_bytes())
        {
            largest = Some((name.clone(), *size));
        }
    }
    let total_bytes = total_data.saturating_add(total_index);
    let total_rebuild_bytes = total_data.saturating_mul(2).saturating_add(total_index);
    SizeCheck {
        setting: setting.clone(),
        over_rows: total_rows > setting.limit.rows,
        over_bytes: total_rebuild_bytes > setting.limit.bytes,
        tables,
        total_rows,
        total_bytes,
        total_rebuild_bytes,
        total_data_bytes: total_data,
        total_index_bytes: total_index,
        largest,
    }
}

impl SizeCheck {
    pub fn within_limit(&self) -> bool {
        !self.over_rows && !self.over_bytes
    }

    /// The bytes of log the rebuild is expected to write under full recovery (the worst measured).
    pub fn estimated_log_bytes(&self) -> u64 {
        (self.total_rebuild_bytes as f64 * LOG_PER_REBUILD_BYTE) as u64
    }

    /// The reason to refuse, or `None` when the stage is within the limit. Ends with what to do.
    pub fn refusal(&self) -> Option<String> {
        if self.within_limit() {
            return None;
        }
        let limit = &self.setting.limit;
        let mut text = String::new();
        let _ = write!(
            text,
            "the stage rebuilds {} table{} with {} rows, {} of data and {} of indexes; the own restructuring copies them in one transaction, whose log would grow by about {}",
            self.tables.len(),
            if self.tables.len() == 1 { "" } else { "s" },
            self.total_rows,
            format_bytes(self.total_data_bytes),
            format_bytes(self.total_index_bytes),
            format_bytes(self.estimated_log_bytes())
        );
        let mut above = Vec::new();
        if self.over_rows {
            above.push(format!(
                "rows: {} above the limit of {} ({})",
                self.total_rows, limit.rows, self.setting.rows_source
            ));
        }
        if self.over_bytes {
            above.push(format!(
                "bytes to write (the data twice and the indexes once): {} above the limit of {} ({})",
                format_bytes(self.total_rebuild_bytes),
                format_bytes(limit.bytes),
                self.setting.bytes_source
            ));
        }
        let _ = write!(text, "; {}", above.join("; "));
        if let Some((name, size)) = &self.largest {
            let _ = write!(
                text,
                ". The largest is {name}: {} rows, {} to write",
                size.rows,
                format_bytes(size.rebuild_bytes())
            );
        }
        text.push_str(
            ". Run the native `ibcmd infobase config apply` for this stage (it commits in steps), or raise the limit: --restructure-limit-rows / --restructure-limit-bytes of `mssql-config-apply`, or restructure-limit-rows / restructure-limit-bytes in ibcmd-rs.toml, or IBCMD_RS_RESTRUCTURE_LIMIT_ROWS / IBCMD_RS_RESTRUCTURE_LIMIT_BYTES",
        );
        Some(text)
    }

    /// For the report of the apply and of `mssql-restructure`: the limit, where it came from, and what the
    /// stage rebuilds.
    pub fn to_json(&self) -> serde_json::Value {
        let tables: BTreeMap<&String, serde_json::Value> = self
            .tables
            .iter()
            .map(|(name, size)| {
                (
                    name,
                    json!({"rows": size.rows, "data_bytes": size.data_bytes, "index_bytes": size.index_bytes}),
                )
            })
            .collect();
        json!({
            "limit": {
                "rows": self.setting.limit.rows,
                "bytes": self.setting.limit.bytes,
                "rows_from": self.setting.rows_source,
                "bytes_from": self.setting.bytes_source,
            },
            "rebuilt": {
                "tables": self.tables.len(),
                "rows": self.total_rows,
                "bytes": self.total_bytes,
                "rebuild_bytes": self.total_rebuild_bytes,
                "data_bytes": self.total_data_bytes,
                "index_bytes": self.total_index_bytes,
            },
            "largest": self.largest.as_ref().map(|(name, size)| json!({"table": name, "rows": size.rows, "rebuild_bytes": size.rebuild_bytes()})),
            "estimated_log_bytes": self.estimated_log_bytes(),
            "within_limit": self.within_limit(),
            "tables": tables,
        })
    }
}

/// `1.5 GiB`, `512.0 MiB`, `980 B`: binary units, for messages.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// `4294967296`, `4GB`, `4 GiB`, `1.5g`, `512MB`: bytes, every suffix a power of 1024.
pub fn parse_byte_size(text: &str) -> Result<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_'))
        .unwrap_or(text.len());
    let (number, suffix) = text.split_at(split);
    let number = number.replace('_', "");
    if number.is_empty() {
        bail!("`{text}` is not a size: a number, then an optional unit (B, KB, MB, GB, TB)");
    }
    let multiplier: u64 = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1 << 10,
        "m" | "mb" | "mib" => 1 << 20,
        "g" | "gb" | "gib" => 1 << 30,
        "t" | "tb" | "tib" => 1 << 40,
        other => bail!("`{text}`: unknown unit `{other}` (B, KB, MB, GB, TB)"),
    };
    if !number.contains('.') {
        let whole: u64 = number
            .parse()
            .with_context(|| format!("`{text}` is not a size"))?;
        return whole
            .checked_mul(multiplier)
            .with_context(|| format!("`{text}` is more than a 64-bit byte count"));
    }
    let fractional: f64 = number
        .parse()
        .with_context(|| format!("`{text}` is not a size"))?;
    let bytes = fractional * multiplier as f64;
    if !bytes.is_finite() || bytes >= u64::MAX as f64 {
        bail!("`{text}` is more than a 64-bit byte count");
    }
    Ok(bytes.round() as u64)
}

/// The query for the sizes of the given tables: rows and used pages of the heap or clustered index, and
/// of the other indexes.
fn sizes_query(tables: &[String]) -> Result<String> {
    let mut names = Vec::with_capacity(tables.len());
    for table in tables {
        if table.is_empty()
            || !table
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            bail!("the table name {table:?} is not a plain identifier");
        }
        names.push(format!("N'{table}'"));
    }
    Ok(format!(
        "SELECT o.name, \
         SUM(CASE WHEN ps.index_id IN (0, 1) THEN ps.row_count ELSE 0 END), \
         SUM(CASE WHEN ps.index_id IN (0, 1) THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END) * 8192, \
         SUM(CASE WHEN ps.index_id > 1 THEN CONVERT(bigint, ps.used_page_count) ELSE 0 END) * 8192 \
         FROM sys.dm_db_partition_stats ps JOIN sys.objects o ON o.object_id = ps.object_id \
         WHERE o.type = 'U' AND o.schema_id = SCHEMA_ID(N'dbo') AND o.name IN ({}) GROUP BY o.name",
        names.join(", ")
    ))
}

/// The sizes of the tables a plan rebuilds. A table the database does not have yet (a new object's) is 0.
pub fn read_sizes(
    source: &mut dyn RowSource,
    tables: &[String],
) -> Result<BTreeMap<String, TableSize>> {
    let mut sizes: BTreeMap<String, TableSize> = tables
        .iter()
        .map(|table| (table.clone(), TableSize::default()))
        .collect();
    if tables.is_empty() {
        return Ok(sizes);
    }
    let query = sizes_query(tables)?;
    source
        .rows(&query, &mut |mut row| {
            let name = row.take_text(0)?;
            let count = |value: i64| u64::try_from(value).unwrap_or(0);
            let size = TableSize {
                rows: count(row.i64(1)?),
                data_bytes: count(row.i64(2)?),
                index_bytes: count(row.i64(3)?),
            };
            let key = sizes
                .keys()
                .find(|table| table.eq_ignore_ascii_case(&name))
                .cloned()
                .unwrap_or(name);
            sizes.insert(key, size);
            Ok(())
        })
        .context("the sizes of the tables to rebuild")?;
    Ok(sizes)
}

/// Reads the sizes of the tables and judges them.
pub fn check_tables(
    source: &mut dyn RowSource,
    tables: &[String],
    setting: &LimitSetting,
) -> Result<SizeCheck> {
    Ok(evaluate(read_sizes(source, tables)?, setting))
}
