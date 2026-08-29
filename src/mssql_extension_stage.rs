//! Transactional staging primitives for 8.3.27 MSSQL configuration extensions.
//!
//! This module deliberately stops at `ConfigCASSave`. It neither publishes
//! rows into `ConfigCAS` nor changes `_ExtensionsInfo`; activation remains a
//! native platform operation.

use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::DeflateEncoder;
use ibcmd_core::storage::{MAX_STORAGE_ENTRIES, MAX_STORAGE_IMAGE_RETAINED_BYTES};
use sha1::{Digest, Sha1};
use uuid::Uuid;

const EVIDENCED_CONFIGINFO_STORAGE_FORMATS_8327: [u32; 4] = [80_310, 80_314, 80_321, 80_324];
const MAX_CONFIGINFO_BYTES: usize = 64 * 1024 * 1024;
const MAX_CONFIGINFO_DESCRIPTOR_BYTES: usize = 64 * 1024;
const MAX_EXTENSION_ZIPPED_INFO_BYTES: usize = 4_000;
const MAX_AGGREGATE_LOGICAL_NAME_UNITS: usize = 16 * 1024 * 1024;
const MAX_STAGE_SQL_SCRIPT_BYTES: usize = 32 * 1024 * 1024;
const STAGE_SQL_FIXED_OVERHEAD_BYTES: usize = 64 * 1024;
static TEMP_SCRIPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One logical packed row to write below an extension's ConfigCASSave prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStageRow {
    pub logical_name: String,
    pub attributes: i16,
    pub binary_data: Vec<u8>,
    pub part_no: i32,
}

impl ExtensionStageRow {
    pub fn new(logical_name: impl Into<String>, binary_data: Vec<u8>) -> Self {
        Self {
            logical_name: logical_name.into(),
            attributes: 0,
            binary_data,
            part_no: 0,
        }
    }

    pub fn data_size(&self) -> u64 {
        self.binary_data.len() as u64
    }
}

/// Optimistic registry snapshot captured before source compilation begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRegistrySnapshot {
    /// `_ExtensionsInfo._IDRRef` in its physical 1C byte order.
    pub extension_id: [u8; 16],
    /// Exact SQL `timestamp`/`rowversion` bytes from `_Version`.
    pub version: [u8; 8],
    /// Exact `_ExtensionZippedInfo` bytes, not a decoded approximation.
    pub zipped_info: Vec<u8>,
}

/// The non-mapping identity fields retained in `configinfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigInfoIdentity {
    /// Storage format retained from the active extension's `configinfo`.
    pub storage_format: u32,
    pub configuration_id: Uuid,
    /// Decoded bytes of the opaque third field in the `{2,...}` record.
    pub descriptor: Vec<u8>,
}

/// One logical-name-to-packed-SHA-1 entry in `configinfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigInfoEntry {
    pub logical_name: String,
    pub packed_sha1: [u8; 20],
}

/// Validated 8.3.27 `configinfo` plaintext.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigInfoManifest {
    pub storage_format: u32,
    pub identity: ConfigInfoIdentity,
    pub entries: Vec<ConfigInfoEntry>,
}

/// A complete, namespaced staging set. `rows` includes generated `configinfo`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStagePlan {
    namespace_prefix: String,
    rows: Vec<ExtensionStageRow>,
}

impl ExtensionStagePlan {
    pub fn namespace_prefix(&self) -> &str {
        &self.namespace_prefix
    }

    pub fn rows(&self) -> &[ExtensionStageRow] {
        &self.rows
    }
}

/// Bounded script payload and the exact postconditions it enforces.
/// `sql` is intended for a script file or process stdin, never `sqlcmd -Q`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStageScript {
    pub namespace_prefix: String,
    pub expected_rows: usize,
    pub expected_bytes: u64,
    sql: String,
}

impl ExtensionStageScript {
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionStageError {
    InvalidRow(String),
    InvalidConfigInfo(String),
    LimitExceeded(String),
    SafetyGate(String),
    Io(String),
    SqlcmdFailed { code: Option<i32>, stderr: String },
}

impl Display for ExtensionStageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRow(reason) => write!(formatter, "invalid extension stage row: {reason}"),
            Self::InvalidConfigInfo(reason) => {
                write!(formatter, "invalid extension configinfo: {reason}")
            }
            Self::LimitExceeded(reason) => {
                write!(formatter, "extension stage limit exceeded: {reason}")
            }
            Self::SafetyGate(reason) => write!(formatter, "extension stage safety gate: {reason}"),
            Self::Io(reason) => write!(formatter, "extension stage I/O failed: {reason}"),
            Self::SqlcmdFailed { code, stderr } => {
                write!(formatter, "sqlcmd failed with exit code {code:?}: {stderr}")
            }
        }
    }
}

impl Error for ExtensionStageError {}

/// Converts physical `_IDRRef` bytes to the canonical UUID used by native
/// ConfigCASSave keys.
pub fn extension_namespace_prefix(extension_id: [u8; 16]) -> String {
    let mut canonical = [0_u8; 16];
    canonical[0..4].copy_from_slice(&extension_id[12..16]);
    canonical[4..6].copy_from_slice(&extension_id[10..12]);
    canonical[6..8].copy_from_slice(&extension_id[8..10]);
    canonical[8..16].copy_from_slice(&extension_id[0..8]);
    Uuid::from_bytes(canonical).hyphenated().to_string()
}

/// Generates deterministic 8.3.27 `configinfo` plaintext from packed logical
/// rows. The returned bytes include the UTF-8 BOM observed in native storage.
pub fn generate_configinfo_manifest(
    identity: &ConfigInfoIdentity,
    rows: &[ExtensionStageRow],
) -> Result<Vec<u8>, ExtensionStageError> {
    validate_storage_format(identity.storage_format)?;
    validate_descriptor(&identity.descriptor)?;
    validate_content_rows(rows)?;
    if rows.iter().any(|row| row.logical_name == "configinfo") {
        return Err(ExtensionStageError::InvalidRow(
            "configinfo is generated and must not be supplied as a content row".to_owned(),
        ));
    }
    if !rows
        .iter()
        .any(|row| row.logical_name == identity.configuration_id.hyphenated().to_string())
    {
        return Err(ExtensionStageError::InvalidConfigInfo(format!(
            "configuration root row `{}` is missing",
            identity.configuration_id.hyphenated()
        )));
    }

    let mut entries = rows
        .iter()
        .map(|row| ConfigInfoEntry {
            logical_name: row.logical_name.clone(),
            packed_sha1: Sha1::digest(&row.binary_data).into(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.logical_name.cmp(&right.logical_name));

    let mut text = String::new();
    text.push('\u{feff}');
    write!(
        text,
        "{{0,\r\n{{216,0,\r\n{{{},0}}\r\n}}\r\n}},\r\n{{2,{},{}}},\r\n{{{}",
        identity.storage_format,
        identity.configuration_id.hyphenated(),
        encode_base64(&identity.descriptor),
        entries.len()
    )
    .expect("writing to String cannot fail");
    for entry in &entries {
        write!(
            text,
            ",\"{}\",{}",
            entry.logical_name.replace('"', "\"\""),
            encode_base64(&entry.packed_sha1)
        )
        .expect("writing to String cannot fail");
    }
    text.push('}');
    if text.len() > MAX_CONFIGINFO_BYTES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "configinfo plaintext is {} bytes; maximum is {MAX_CONFIGINFO_BYTES}",
            text.len()
        )));
    }
    let bytes = text.into_bytes();
    // Keep generation and validation coupled so malformed future formatting
    // changes fail before any SQL can be produced.
    validate_configinfo_manifest(&bytes)?;
    Ok(bytes)
}

/// Parses and validates the evidenced three-record 8.3.27 configinfo shape.
pub fn validate_configinfo_manifest(
    bytes: &[u8],
) -> Result<ConfigInfoManifest, ExtensionStageError> {
    if bytes.len() > MAX_CONFIGINFO_BYTES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "configinfo plaintext is {} bytes; maximum is {MAX_CONFIGINFO_BYTES}",
            bytes.len()
        )));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ExtensionStageError::InvalidConfigInfo(format!("manifest is not UTF-8: {error}"))
    })?;
    let records = split_top_level_records(text.trim_start_matches('\u{feff}'))?;
    if records.len() != 3 {
        return Err(ExtensionStageError::InvalidConfigInfo(format!(
            "expected three top-level records, got {}",
            records.len()
        )));
    }

    let header = split_braced_fields(records[0])?;
    if header.len() != 2 || header[0].trim() != "0" {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "header must be `{0,{216,0,{80321,0}}}`".to_owned(),
        ));
    }
    let header_body = split_braced_fields(header[1])?;
    if header_body.len() != 3 || header_body[0].trim() != "216" || header_body[1].trim() != "0" {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "unsupported configinfo header".to_owned(),
        ));
    }
    let format = split_braced_fields(header_body[2])?;
    if format.len() != 2 || format[1].trim() != "0" {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "invalid storage-format record".to_owned(),
        ));
    }
    let storage_format = format[0].trim().parse::<u32>().map_err(|_| {
        ExtensionStageError::InvalidConfigInfo("storage format is not an integer".to_owned())
    })?;
    validate_storage_format(storage_format)?;

    let identity = split_braced_fields(records[1])?;
    if identity.len() != 3 || identity[0].trim() != "2" {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "invalid configuration identity record".to_owned(),
        ));
    }
    let configuration_id = Uuid::parse_str(identity[1].trim()).map_err(|error| {
        ExtensionStageError::InvalidConfigInfo(format!("invalid configuration UUID: {error}"))
    })?;
    let descriptor_encoded_len = identity[2]
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .count();
    let maximum_descriptor_base64 = MAX_CONFIGINFO_DESCRIPTOR_BYTES.div_ceil(3) * 4;
    if descriptor_encoded_len > maximum_descriptor_base64 {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "configinfo descriptor base64 has {descriptor_encoded_len} characters; maximum is {maximum_descriptor_base64}"
        )));
    }
    let descriptor = decode_base64_mime(identity[2]).ok_or_else(|| {
        ExtensionStageError::InvalidConfigInfo("identity descriptor is not base64".to_owned())
    })?;
    validate_descriptor(&descriptor)?;

    let mapping = split_braced_fields(records[2])?;
    let count = parse_count(mapping.first().copied().unwrap_or_default())?;
    if count > MAX_STORAGE_ENTRIES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "configinfo declares {count} rows; maximum is {MAX_STORAGE_ENTRIES}"
        )));
    }
    if mapping.len() != count.saturating_mul(2).saturating_add(1) {
        return Err(ExtensionStageError::InvalidConfigInfo(format!(
            "configinfo declares {count} rows but has {} mapping fields",
            mapping.len()
        )));
    }
    let mut entries = Vec::with_capacity(count);
    let mut previous: Option<&str> = None;
    for index in 0..count {
        let logical_name = parse_quoted(mapping[index * 2 + 1]).ok_or_else(|| {
            ExtensionStageError::InvalidConfigInfo(format!(
                "mapping entry {index} has an invalid logical name"
            ))
        })?;
        validate_logical_name(&logical_name)?;
        if previous.is_some_and(|value| value >= logical_name.as_str()) {
            return Err(ExtensionStageError::InvalidConfigInfo(
                "mapping logical names must be unique and sorted".to_owned(),
            ));
        }
        let digest = decode_base64_mime(mapping[index * 2 + 2]).ok_or_else(|| {
            ExtensionStageError::InvalidConfigInfo(format!(
                "mapping entry {index} digest is not base64"
            ))
        })?;
        let packed_sha1: [u8; 20] = digest.try_into().map_err(|digest: Vec<u8>| {
            ExtensionStageError::InvalidConfigInfo(format!(
                "mapping entry {index} digest has {} bytes instead of 20",
                digest.len()
            ))
        })?;
        entries.push(ConfigInfoEntry {
            logical_name,
            packed_sha1,
        });
        previous = entries.last().map(|entry| entry.logical_name.as_str());
    }
    if !entries
        .iter()
        .any(|entry| entry.logical_name == configuration_id.hyphenated().to_string())
    {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "mapping does not contain its configuration root UUID".to_owned(),
        ));
    }
    Ok(ConfigInfoManifest {
        storage_format,
        identity: ConfigInfoIdentity {
            storage_format,
            configuration_id,
            descriptor,
        },
        entries,
    })
}

/// Adds the generated, raw-deflated `configinfo` row to a validated content set.
pub fn prepare_extension_stage(
    extension_id: [u8; 16],
    identity: &ConfigInfoIdentity,
    mut content_rows: Vec<ExtensionStageRow>,
) -> Result<ExtensionStagePlan, ExtensionStageError> {
    let configinfo_plain = generate_configinfo_manifest(identity, &content_rows)?;
    let configinfo_packed = deflate_raw(&configinfo_plain)?;
    content_rows.sort_by(|left, right| left.logical_name.cmp(&right.logical_name));
    content_rows.push(ExtensionStageRow::new("configinfo", configinfo_packed));
    validate_content_rows(&content_rows)?;
    let namespace_prefix = extension_namespace_prefix(extension_id);
    validate_namespaced_lengths(&namespace_prefix, &content_rows)?;
    Ok(ExtensionStagePlan {
        namespace_prefix,
        rows: content_rows,
    })
}

/// Builds one all-or-nothing SQL *script* for the selected extension namespace.
/// The batch locks and rechecks the exact registry version and blob before it
/// touches staging rows, and never writes ConfigCAS or `_ExtensionsInfo`.
///
/// The result contains binary literals and MUST be passed through a script file
/// or process stdin. It is deliberately not a `sqlcmd -Q` command: real
/// extensions exceed Windows command-line limits by orders of magnitude.
pub fn build_configcassave_stage_sql(
    database: &str,
    snapshot: &ExtensionRegistrySnapshot,
    plan: &ExtensionStagePlan,
    replace_prefix: bool,
    allow_non_lab: bool,
) -> Result<ExtensionStageScript, ExtensionStageError> {
    require_write_confirmation(allow_non_lab)?;
    if snapshot.zipped_info.len() > MAX_EXTENSION_ZIPPED_INFO_BYTES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "_ExtensionZippedInfo has {} bytes; maximum is {MAX_EXTENSION_ZIPPED_INFO_BYTES}",
            snapshot.zipped_info.len()
        )));
    }
    validate_stage_plan(snapshot, plan)?;
    let rows = plan.rows();
    validate_content_rows(rows)?;
    let prefix = extension_namespace_prefix(snapshot.extension_id);
    validate_namespaced_lengths(&prefix, rows)?;
    let expected_rows = rows.len();
    let expected_bytes = rows.iter().try_fold(0_u64, |total, row| {
        total.checked_add(row.data_size()).ok_or_else(|| {
            ExtensionStageError::LimitExceeded("total staged byte count overflows u64".to_owned())
        })
    })?;
    let script_capacity = estimate_stage_sql_bytes(database, snapshot, &prefix, rows)?;
    let pattern = format!("{}~_~_%", prefix);
    let mut sql = String::with_capacity(script_capacity);
    write!(
        sql,
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;\n\
         USE {database};\n\
         BEGIN TRANSACTION;\n\
         DECLARE @ExtensionId binary(16) = 0x{extension_id};\n\
         DECLARE @ExpectedVersion binary(8) = 0x{version};\n\
         DECLARE @ExpectedZippedInfo varbinary(max) = 0x{zipped_info};\n\
         IF NOT EXISTS (\n\
             SELECT 1 FROM dbo._ExtensionsInfo WITH (UPDLOCK, HOLDLOCK)\n\
             WHERE _IDRRef = @ExtensionId\n\
               AND _Version = @ExpectedVersion\n\
               AND DATALENGTH(_ExtensionZippedInfo) = DATALENGTH(@ExpectedZippedInfo)\n\
               AND _ExtensionZippedInfo = @ExpectedZippedInfo\n\
         ) THROW 57100, 'Extension registry changed after source compilation', 1;\n\
         DECLARE @ExistingPrefixRows bigint = (\n\
             SELECT COUNT_BIG(*) FROM dbo.ConfigCASSave WITH (UPDLOCK, HOLDLOCK)\n\
             WHERE FileName LIKE N'{pattern}' ESCAPE N'~'\n\
         );\n",
        database = quote_ident(database)?,
        extension_id = encode_hex(&snapshot.extension_id),
        version = encode_hex(&snapshot.version),
        zipped_info = encode_hex(&snapshot.zipped_info),
        pattern = quote_string(&pattern),
    )
    .expect("writing to String cannot fail");
    if replace_prefix {
        writeln!(
            sql,
            "DELETE FROM dbo.ConfigCASSave WHERE FileName LIKE N'{}' ESCAPE N'~';",
            quote_string(&pattern)
        )
        .expect("writing to String cannot fail");
        sql.push_str("IF @@ROWCOUNT <> @ExistingPrefixRows THROW 57101, 'ConfigCASSave prefix changed while replacing', 1;\n");
    } else {
        sql.push_str("IF @ExistingPrefixRows <> 0 THROW 57102, 'ConfigCASSave prefix is dirty; explicit replacement is required', 1;\n");
    }

    for row in rows {
        let file_name = format!("{}__{}", prefix, row.logical_name);
        writeln!(
            sql,
            "INSERT INTO dbo.ConfigCASSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) VALUES (N'{}', SYSUTCDATETIME(), SYSUTCDATETIME(), {}, {}, 0x{}, {});",
            quote_string(&file_name),
            row.attributes,
            row.binary_data.len(),
            encode_hex(&row.binary_data),
            row.part_no
        )
        .expect("writing to String cannot fail");
        sql.push_str("IF @@ROWCOUNT <> 1 THROW 57103, 'ConfigCASSave row insert failed', 1;\n");
    }
    write!(
        sql,
        "IF (SELECT COUNT_BIG(*) FROM dbo.ConfigCASSave WHERE FileName LIKE N'{pattern}' ESCAPE N'~') <> {expected_rows} THROW 57104, 'Unexpected ConfigCASSave prefix row count after staging', 1;\n\
         IF (SELECT COALESCE(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0) FROM dbo.ConfigCASSave WHERE FileName LIKE N'{pattern}' ESCAPE N'~') <> {expected_bytes} THROW 57105, 'Unexpected ConfigCASSave prefix byte count after staging', 1;\n\
         IF (SELECT COALESCE(SUM(DataSize), 0) FROM dbo.ConfigCASSave WHERE FileName LIKE N'{pattern}' ESCAPE N'~') <> {expected_bytes} THROW 57106, 'Unexpected ConfigCASSave prefix DataSize after staging', 1;\n\
         COMMIT TRANSACTION;\n",
        pattern = quote_string(&pattern),
    )
    .expect("writing to String cannot fail");
    if sql.len() > MAX_STAGE_SQL_SCRIPT_BYTES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "generated SQL script is {} bytes; maximum is {MAX_STAGE_SQL_SCRIPT_BYTES}",
            sql.len()
        )));
    }
    Ok(ExtensionStageScript {
        namespace_prefix: prefix,
        expected_rows,
        expected_bytes,
        sql,
    })
}

fn validate_stage_plan(
    snapshot: &ExtensionRegistrySnapshot,
    plan: &ExtensionStagePlan,
) -> Result<(), ExtensionStageError> {
    let expected_prefix = extension_namespace_prefix(snapshot.extension_id);
    if plan.namespace_prefix != expected_prefix {
        return Err(ExtensionStageError::InvalidRow(format!(
            "plan namespace `{}` does not match selected extension `{expected_prefix}`",
            plan.namespace_prefix
        )));
    }
    validate_content_rows(&plan.rows)?;
    let configinfo_rows = plan
        .rows
        .iter()
        .filter(|row| row.logical_name == "configinfo")
        .collect::<Vec<_>>();
    if configinfo_rows.len() != 1 {
        return Err(ExtensionStageError::InvalidConfigInfo(format!(
            "stage plan must contain exactly one configinfo row, got {}",
            configinfo_rows.len()
        )));
    }
    let configinfo = configinfo_rows[0];
    if configinfo.attributes != 0 || configinfo.part_no != 0 {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "configinfo must use Attributes=0 and PartNo=0".to_owned(),
        ));
    }
    let plain = inflate_raw_bounded(&configinfo.binary_data, MAX_CONFIGINFO_BYTES)?;
    let manifest = validate_configinfo_manifest(&plain)?;
    let mut content = plan
        .rows
        .iter()
        .filter(|row| row.logical_name != "configinfo")
        .collect::<Vec<_>>();
    content.sort_by(|left, right| left.logical_name.cmp(&right.logical_name));
    if manifest.entries.len() != content.len() {
        return Err(ExtensionStageError::InvalidConfigInfo(format!(
            "configinfo maps {} content rows but plan contains {}",
            manifest.entries.len(),
            content.len()
        )));
    }
    for (entry, row) in manifest.entries.iter().zip(content) {
        let actual_sha1: [u8; 20] = Sha1::digest(&row.binary_data).into();
        if entry.logical_name != row.logical_name || entry.packed_sha1 != actual_sha1 {
            return Err(ExtensionStageError::InvalidConfigInfo(format!(
                "configinfo mapping for `{}` does not match staged packed content",
                row.logical_name
            )));
        }
    }
    Ok(())
}

fn estimate_stage_sql_bytes(
    database: &str,
    snapshot: &ExtensionRegistrySnapshot,
    prefix: &str,
    rows: &[ExtensionStageRow],
) -> Result<usize, ExtensionStageError> {
    let zipped_hex = snapshot.zipped_info.len().checked_mul(2).ok_or_else(|| {
        ExtensionStageError::LimitExceeded("SQL script size estimate overflow".to_owned())
    })?;
    let mut estimated = STAGE_SQL_FIXED_OVERHEAD_BYTES
        .checked_add(database.len())
        .and_then(|value| value.checked_add(zipped_hex))
        .ok_or_else(|| {
            ExtensionStageError::LimitExceeded("SQL script size estimate overflow".to_owned())
        })?;
    for row in rows {
        let quoted_name = row.logical_name.len().checked_mul(2).ok_or_else(|| {
            ExtensionStageError::LimitExceeded("SQL script size estimate overflow".to_owned())
        })?;
        let binary_hex = row.binary_data.len().checked_mul(2).ok_or_else(|| {
            ExtensionStageError::LimitExceeded("SQL script size estimate overflow".to_owned())
        })?;
        estimated = estimated
            .checked_add(prefix.len())
            .and_then(|value| value.checked_add(quoted_name))
            .and_then(|value| value.checked_add(binary_hex))
            .and_then(|value| value.checked_add(512))
            .ok_or_else(|| {
                ExtensionStageError::LimitExceeded("SQL script size estimate overflow".to_owned())
            })?;
        if estimated > MAX_STAGE_SQL_SCRIPT_BYTES {
            return Err(ExtensionStageError::LimitExceeded(format!(
                "estimated SQL script is {estimated} bytes; maximum is {MAX_STAGE_SQL_SCRIPT_BYTES}"
            )));
        }
    }
    Ok(estimated)
}

fn validate_content_rows(rows: &[ExtensionStageRow]) -> Result<(), ExtensionStageError> {
    if rows.is_empty() {
        return Err(ExtensionStageError::InvalidRow(
            "at least one row is required".to_owned(),
        ));
    }
    if rows.len() > MAX_STORAGE_ENTRIES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "{} rows supplied; maximum is {MAX_STORAGE_ENTRIES}",
            rows.len()
        )));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut retained_bytes = 0_usize;
    let mut logical_name_units = 0_usize;
    for row in rows {
        validate_logical_name(&row.logical_name)?;
        logical_name_units = logical_name_units
            .checked_add(row.logical_name.encode_utf16().count())
            .ok_or_else(|| {
                ExtensionStageError::LimitExceeded(
                    "aggregate logical-name length overflows usize".to_owned(),
                )
            })?;
        if logical_name_units > MAX_AGGREGATE_LOGICAL_NAME_UNITS {
            return Err(ExtensionStageError::LimitExceeded(format!(
                "aggregate logical names use {logical_name_units} UTF-16 units; maximum is {MAX_AGGREGATE_LOGICAL_NAME_UNITS}"
            )));
        }
        if row.part_no != 0 {
            return Err(ExtensionStageError::InvalidRow(format!(
                "`{}` has PartNo {}; only evidenced PartNo 0 is supported",
                row.logical_name, row.part_no
            )));
        }
        if !names.insert(row.logical_name.as_str()) {
            return Err(ExtensionStageError::InvalidRow(format!(
                "duplicate logical name `{}`",
                row.logical_name
            )));
        }
        retained_bytes = retained_bytes
            .checked_add(row.binary_data.len())
            .ok_or_else(|| {
                ExtensionStageError::LimitExceeded(
                    "aggregate staged bytes overflow usize".to_owned(),
                )
            })?;
        if retained_bytes > MAX_STORAGE_IMAGE_RETAINED_BYTES {
            return Err(ExtensionStageError::LimitExceeded(format!(
                "aggregate staged bytes are {retained_bytes}; maximum is {MAX_STORAGE_IMAGE_RETAINED_BYTES}"
            )));
        }
    }
    Ok(())
}

fn validate_storage_format(storage_format: u32) -> Result<(), ExtensionStageError> {
    if EVIDENCED_CONFIGINFO_STORAGE_FORMATS_8327.contains(&storage_format) {
        Ok(())
    } else {
        Err(ExtensionStageError::InvalidConfigInfo(format!(
            "unsupported storage format {storage_format}"
        )))
    }
}

fn validate_descriptor(descriptor: &[u8]) -> Result<(), ExtensionStageError> {
    if descriptor.len() <= MAX_CONFIGINFO_DESCRIPTOR_BYTES {
        Ok(())
    } else {
        Err(ExtensionStageError::LimitExceeded(format!(
            "configinfo descriptor has {} bytes; maximum is {MAX_CONFIGINFO_DESCRIPTOR_BYTES}",
            descriptor.len()
        )))
    }
}

fn require_write_confirmation(allow_non_lab: bool) -> Result<(), ExtensionStageError> {
    if allow_non_lab {
        Ok(())
    } else {
        Err(ExtensionStageError::SafetyGate(
            "direct extension writes require explicit allow_non_lab confirmation".to_owned(),
        ))
    }
}

fn validate_logical_name(name: &str) -> Result<(), ExtensionStageError> {
    if name.is_empty() || name.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err(ExtensionStageError::InvalidRow(format!(
            "logical name `{name}` is empty or contains a control character"
        )));
    }
    Ok(())
}

fn validate_namespaced_lengths(
    prefix: &str,
    rows: &[ExtensionStageRow],
) -> Result<(), ExtensionStageError> {
    for row in rows {
        let file_name = format!("{prefix}__{}", row.logical_name);
        let units = file_name.encode_utf16().count();
        if units > 256 {
            return Err(ExtensionStageError::InvalidRow(format!(
                "namespaced FileName `{file_name}` uses {units} UTF-16 units; maximum is 256"
            )));
        }
    }
    Ok(())
}

fn deflate_raw(bytes: &[u8]) -> Result<Vec<u8>, ExtensionStageError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(|error| {
        ExtensionStageError::InvalidConfigInfo(format!("failed to deflate configinfo: {error}"))
    })?;
    encoder.finish().map_err(|error| {
        ExtensionStageError::InvalidConfigInfo(format!("failed to finish configinfo: {error}"))
    })
}

fn inflate_raw_bounded(bytes: &[u8], maximum: usize) -> Result<Vec<u8>, ExtensionStageError> {
    use std::io::Read as _;

    let mut decoder = flate2::read::DeflateDecoder::new(bytes).take((maximum as u64) + 1);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).map_err(|error| {
        ExtensionStageError::InvalidConfigInfo(format!(
            "configinfo raw-deflate is invalid: {error}"
        ))
    })?;
    if output.len() > maximum {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "inflated configinfo exceeds {maximum} bytes"
        )));
    }
    Ok(output)
}

/// Executes a previously validated stage script through a temporary `-i` file.
/// `-b` makes SQL errors propagate as a non-zero process exit. Binary payloads
/// never enter the Windows command line and `-Q` is never used.
pub fn execute_configcassave_stage_script(
    sqlcmd: &Path,
    server: &str,
    sql_user: Option<&str>,
    sql_password: Option<&str>,
    trust_server_certificate: bool,
    script: &ExtensionStageScript,
    allow_non_lab: bool,
) -> Result<(), ExtensionStageError> {
    require_write_confirmation(allow_non_lab)?;
    if script.sql.len() > MAX_STAGE_SQL_SCRIPT_BYTES {
        return Err(ExtensionStageError::LimitExceeded(format!(
            "SQL script is {} bytes; maximum is {MAX_STAGE_SQL_SCRIPT_BYTES}",
            script.sql.len()
        )));
    }
    if sql_user.is_some() != sql_password.is_some() {
        return Err(ExtensionStageError::SafetyGate(
            "SQL user and password must be supplied together".to_owned(),
        ));
    }
    let temp = TempSqlScript::create(script.sql.as_bytes())?;
    let mut command = sqlcmd_command(
        sqlcmd,
        server,
        sql_user,
        sql_password,
        trust_server_certificate,
        temp.path(),
    );
    let output = command.output().map_err(|error| {
        ExtensionStageError::Io(format!("failed to start {}: {error}", sqlcmd.display()))
    })?;
    propagate_sqlcmd_exit(output)
}

/// Builds, validates, and executes one extension staging transaction.
/// This is the integration boundary used by the load command; it returns the
/// exact postcondition metadata only after `sqlcmd -b` exits successfully.
pub fn execute_configcassave_stage(
    sqlcmd: &Path,
    server: &str,
    sql_user: Option<&str>,
    sql_password: Option<&str>,
    database: &str,
    snapshot: &ExtensionRegistrySnapshot,
    plan: &ExtensionStagePlan,
    replace_prefix: bool,
    allow_non_lab: bool,
    trust_server_certificate: bool,
) -> Result<ExtensionStageScript, ExtensionStageError> {
    let script =
        build_configcassave_stage_sql(database, snapshot, plan, replace_prefix, allow_non_lab)?;
    execute_configcassave_stage_script(
        sqlcmd,
        server,
        sql_user,
        sql_password,
        trust_server_certificate,
        &script,
        allow_non_lab,
    )?;
    Ok(script)
}

fn sqlcmd_command(
    sqlcmd: &Path,
    server: &str,
    sql_user: Option<&str>,
    sql_password: Option<&str>,
    trust_server_certificate: bool,
    script_path: &Path,
) -> Command {
    let mut command = Command::new(sqlcmd);
    command
        .arg("-S")
        .arg(server)
        .arg("-b")
        // Disable client-side $(NAME) expansion. Names and values are already
        // quoted as T-SQL; sqlcmd substitution would happen before SQL Server
        // sees those quotes and may expose SQLCMDPASSWORD.
        .arg("-x")
        // TempSqlScript is encoded as UTF-8 and identifiers may be Unicode.
        .arg("-f")
        .arg("65001")
        .arg("-r")
        .arg("1")
        .arg("-i")
        .arg(script_path);
    if trust_server_certificate {
        command.arg("-C");
    }
    if let (Some(user), Some(password)) = (sql_user, sql_password) {
        command.arg("-U").arg(user);
        command.env("SQLCMDPASSWORD", password);
    } else {
        command.arg("-E");
    }
    command
}

fn propagate_sqlcmd_exit(output: Output) -> Result<(), ExtensionStageError> {
    if output.status.success() {
        return Ok(());
    }
    let mut stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        stderr = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    }
    if stderr.len() > 8 * 1024 {
        let mut boundary = 8 * 1024;
        while !stderr.is_char_boundary(boundary) {
            boundary -= 1;
        }
        stderr.truncate(boundary);
        stderr.push_str("...[truncated]");
    }
    Err(ExtensionStageError::SqlcmdFailed {
        code: output.status.code(),
        stderr,
    })
}

struct TempSqlScript {
    path: PathBuf,
}

impl TempSqlScript {
    fn create(bytes: &[u8]) -> Result<Self, ExtensionStageError> {
        let directory = std::env::temp_dir();
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..32 {
            let sequence = TEMP_SCRIPT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(
                "ibcmd-rs-extension-stage-{}-{epoch}-{sequence}.sql",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(ExtensionStageError::Io(format!(
                            "failed to write temporary SQL script {}: {error}",
                            path.display()
                        )));
                    }
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(ExtensionStageError::Io(format!(
                        "failed to create temporary SQL script in {}: {error}",
                        directory.display()
                    )));
                }
            }
        }
        Err(ExtensionStageError::Io(
            "failed to allocate a unique temporary SQL script".to_owned(),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempSqlScript {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn split_top_level_records(value: &str) -> Result<Vec<&str>, ExtensionStageError> {
    let mut records = Vec::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b',') {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        if bytes[index] != b'{' {
            return Err(ExtensionStageError::InvalidConfigInfo(
                "top-level value is not a braced record".to_owned(),
            ));
        }
        let start = index;
        let mut depth = 0_usize;
        let mut quoted = false;
        while index < bytes.len() {
            match bytes[index] {
                b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => index += 1,
                b'"' => quoted = !quoted,
                b'{' if !quoted => depth += 1,
                b'}' if !quoted => {
                    depth = depth.checked_sub(1).ok_or_else(|| {
                        ExtensionStageError::InvalidConfigInfo("unbalanced braces".to_owned())
                    })?;
                    if depth == 0 {
                        index += 1;
                        records.push(&value[start..index]);
                        break;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        if depth != 0 || quoted {
            return Err(ExtensionStageError::InvalidConfigInfo(
                "unterminated top-level record".to_owned(),
            ));
        }
    }
    Ok(records)
}

fn split_braced_fields(value: &str) -> Result<Vec<&str>, ExtensionStageError> {
    let value = value.trim();
    let inner = value
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| ExtensionStageError::InvalidConfigInfo("value is not braced".to_owned()))?;
    let bytes = inner.as_bytes();
    let mut fields = Vec::new();
    let mut start = 0;
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if quoted && bytes.get(index + 1) == Some(&b'"') => index += 1,
            b'"' => quoted = !quoted,
            b'{' if !quoted => depth += 1,
            b'}' if !quoted => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    ExtensionStageError::InvalidConfigInfo("unbalanced braces".to_owned())
                })?
            }
            b',' if !quoted && depth == 0 => {
                fields.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if depth != 0 || quoted {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "unterminated nested value".to_owned(),
        ));
    }
    fields.push(&inner[start..]);
    Ok(fields)
}

fn parse_count(value: &str) -> Result<usize, ExtensionStageError> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ExtensionStageError::InvalidConfigInfo(
            "mapping count is not an unsigned integer".to_owned(),
        ));
    }
    value.parse().map_err(|_| {
        ExtensionStageError::InvalidConfigInfo("mapping count does not fit usize".to_owned())
    })
}

fn parse_quoted(value: &str) -> Option<String> {
    let inner = value.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut output = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' && chars.next_if_eq(&'"').is_none() {
            return None;
        }
        output.push(ch);
    }
    Some(output)
}

fn quote_ident(value: &str) -> Result<String, ExtensionStageError> {
    if value.is_empty() || value.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err(ExtensionStageError::InvalidRow(
            "database name is empty or contains a control character".to_owned(),
        ));
    }
    let units = value.encode_utf16().count();
    if units > 128 {
        return Err(ExtensionStageError::InvalidRow(format!(
            "database identifier uses {units} UTF-16 units; SQL Server maximum is 128"
        )));
    }
    Ok(format!("[{}]", value.replace(']', "]]")))
}

fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
}

fn encode_base64(bytes: &[u8]) -> String {
    crate::module_blob::encode_base64(bytes)
}

fn decode_base64_mime(value: &str) -> Option<Vec<u8>> {
    crate::module_blob::decode_base64_mime(value.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENSION_ID: [u8; 16] = [
        0x87, 0x80, 0x10, 0x7b, 0x44, 0xa2, 0x85, 0x8a, 0x11, 0xe8, 0x07, 0x73, 0x34, 0x7e, 0xea,
        0x01,
    ];

    fn identity() -> ConfigInfoIdentity {
        ConfigInfoIdentity {
            storage_format: 80_321,
            configuration_id: Uuid::parse_str("6a5f6dac-3040-4142-b853-482f745e1e4b").unwrap(),
            descriptor: b"opaque-native-config-descriptor".to_vec(),
        }
    }

    fn content_rows() -> Vec<ExtensionStageRow> {
        vec![
            ExtensionStageRow::new("efc14f05-e4ab-4235-8c63-9082143c3ca9", vec![2, 3]),
            ExtensionStageRow::new("6a5f6dac-3040-4142-b853-482f745e1e4b", vec![1]),
        ]
    }

    #[test]
    fn derives_native_namespace_prefix_from_1c_guid_bytes() {
        assert_eq!(
            extension_namespace_prefix(EXTENSION_ID),
            "347eea01-0773-11e8-8780-107b44a2858a"
        );
    }

    #[test]
    fn configinfo_is_deterministic_sorted_and_self_validating() {
        let plain = generate_configinfo_manifest(&identity(), &content_rows()).unwrap();
        assert!(plain.starts_with(b"\xef\xbb\xbf{0,\r\n{216,0,\r\n{80321,0}"));
        let parsed = validate_configinfo_manifest(&plain).unwrap();
        assert_eq!(parsed.storage_format, 80_321);
        assert_eq!(parsed.identity, identity());
        assert_eq!(
            parsed
                .entries
                .iter()
                .map(|entry| entry.logical_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "6a5f6dac-3040-4142-b853-482f745e1e4b",
                "efc14f05-e4ab-4235-8c63-9082143c3ca9"
            ]
        );
        let expected_digest: [u8; 20] = Sha1::digest([1]).into();
        assert_eq!(parsed.entries[0].packed_sha1, expected_digest);
    }

    #[test]
    fn validates_observed_8327_configinfo_from_disposable_clone() {
        const OBSERVED: &str = "\u{feff}{0,\r\n{216,0,\r\n{80321,0}\r\n}\r\n},\r\n{2,6a5f6dac-3040-4142-b853-482f745e1e4b,Isv9796IMjgEO/krkgUBBp0jqtSi0d7gjfXa86TeIBkZ2NKe+L/1b0PRwiErPs0O\r\r\n2o1xJ7fu36lgSvepLng9FaQzpfs3cu2KgG9VtgGsIuUMUBQ4SiFkJA0v6aqE04Es\r\r\n+NzoyoLc3Lu/ITq662fLSMY6dxDHnwrioIL1CPH1A8vDMm4mjSP/BBQJD9AkRr9K\r\r\nBLQglGRk1gwGVn8lbzTUqY/ARrAbKdsIp/BYKXD0Zsw=},\r\n{2,\"6a5f6dac-3040-4142-b853-482f745e1e4b\",YH+2hS0BbTZJ43VJ2BAY0d2Yu5Y=,\"efc14f05-e4ab-4235-8c63-9082143c3ca9\",IjlhwQxSukXqNZWmFzb6tUtWuOY=}";
        let parsed = validate_configinfo_manifest(OBSERVED.as_bytes()).unwrap();
        assert_eq!(
            parsed.identity.configuration_id,
            Uuid::parse_str("6a5f6dac-3040-4142-b853-482f745e1e4b").unwrap()
        );
        assert_eq!(parsed.identity.descriptor.len(), 176);
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(
            encode_hex(&parsed.entries[0].packed_sha1),
            "607FB6852D016D3649E37549D81018D1DD98BB96"
        );
        assert_eq!(
            encode_hex(&parsed.entries[1].packed_sha1),
            "223961C10C52BA45EA3595A61736FAB54B56B8E6"
        );
    }

    #[test]
    fn configinfo_rejects_wrong_profile_and_digest_width() {
        let plain = generate_configinfo_manifest(&identity(), &content_rows()).unwrap();
        let text = String::from_utf8(plain).unwrap();
        let wrong_profile = text.replace("80321", "99999");
        assert!(matches!(
            validate_configinfo_manifest(wrong_profile.as_bytes()),
            Err(ExtensionStageError::InvalidConfigInfo(reason)) if reason.contains("unsupported storage format")
        ));
        let short_digest = text.replace("5ba93c9db0cff93f52b521d7420e43f6eda2784f", "x");
        // The manifest stores base64, so mutate one digest to a valid but short value.
        let first_digest = encode_base64(&Sha1::digest([1]));
        let short_digest = short_digest.replace(&first_digest, "AQ==");
        assert!(matches!(
            validate_configinfo_manifest(short_digest.as_bytes()),
            Err(ExtensionStageError::InvalidConfigInfo(reason)) if reason.contains("instead of 20")
        ));
    }

    #[test]
    fn accepts_all_and_only_evidenced_8327_storage_formats() {
        for format in EVIDENCED_CONFIGINFO_STORAGE_FORMATS_8327 {
            let mut identity = identity();
            identity.storage_format = format;
            let plain = generate_configinfo_manifest(&identity, &content_rows()).unwrap();
            assert_eq!(
                validate_configinfo_manifest(&plain).unwrap().storage_format,
                format
            );
        }
        let mut identity = identity();
        identity.storage_format = 80_999;
        assert!(generate_configinfo_manifest(&identity, &content_rows()).is_err());
    }

    #[test]
    fn stage_plan_adds_only_one_generated_configinfo_row() {
        let plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        assert_eq!(
            plan.namespace_prefix(),
            "347eea01-0773-11e8-8780-107b44a2858a"
        );
        assert_eq!(plan.rows().len(), 3);
        assert_eq!(plan.rows().last().unwrap().logical_name, "configinfo");
        assert!(!plan.rows().last().unwrap().binary_data.is_empty());
    }

    #[test]
    fn sql_is_serializable_optimistic_and_prefix_scoped() {
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [1, 2, 3, 4, 5, 6, 7, 8],
            zipped_info: vec![0xaa, 0xbb],
        };
        let plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        assert!(matches!(
            build_configcassave_stage_sql("lab]db", &snapshot, &plan, false, false),
            Err(ExtensionStageError::SafetyGate(_))
        ));
        let script =
            build_configcassave_stage_sql("lab]db", &snapshot, &plan, false, true).unwrap();
        assert_eq!(script.expected_rows, 3);
        assert!(script.expected_bytes > 3);
        let sql = &script.sql;
        assert!(sql.contains("SET XACT_ABORT ON"));
        assert!(sql.contains("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"));
        assert!(sql.contains("_ExtensionsInfo WITH (UPDLOCK, HOLDLOCK)"));
        assert!(sql.contains("_Version = @ExpectedVersion"));
        assert!(sql.contains("_ExtensionZippedInfo = @ExpectedZippedInfo"));
        assert!(sql.contains("USE [lab]]db]"));
        assert!(sql.contains("347eea01-0773-11e8-8780-107b44a2858a__configinfo"));
        assert!(sql.contains("LIKE N'347eea01-0773-11e8-8780-107b44a2858a~_~_%' ESCAPE N'~'"));
        assert!(sql.contains("explicit replacement is required"));
        assert!(sql.contains("<> 3 THROW 57104"));
        assert!(!sql.contains("INSERT INTO dbo.ConfigCAS "));
        assert!(!sql.contains("UPDATE dbo._ExtensionsInfo"));
        assert!(!sql.contains("DELETE FROM dbo._ExtensionsInfo"));
    }

    #[test]
    fn replacement_deletes_only_the_selected_prefix() {
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [0; 8],
            zipped_info: vec![],
        };
        let plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        let script = build_configcassave_stage_sql("lab", &snapshot, &plan, true, true).unwrap();
        let sql = &script.sql;
        assert!(sql.contains("DELETE FROM dbo.ConfigCASSave WHERE FileName LIKE"));
        assert!(!sql.contains("DELETE FROM dbo.ConfigCASSave;"));
        assert!(sql.contains("IF @@ROWCOUNT <> @ExistingPrefixRows"));
    }

    #[test]
    fn rejects_duplicates_parts_and_nvarchar_overflow_before_sql() {
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [0; 8],
            zipped_info: vec![],
        };
        let duplicate = ExtensionStagePlan {
            namespace_prefix: extension_namespace_prefix(EXTENSION_ID),
            rows: vec![
                ExtensionStageRow::new("same", vec![]),
                ExtensionStageRow::new("same", vec![]),
            ],
        };
        assert!(build_configcassave_stage_sql("lab", &snapshot, &duplicate, false, true).is_err());
        let mut multipart = ExtensionStageRow::new("row", vec![]);
        multipart.part_no = 1;
        let multipart = ExtensionStagePlan {
            namespace_prefix: extension_namespace_prefix(EXTENSION_ID),
            rows: vec![multipart],
        };
        assert!(build_configcassave_stage_sql("lab", &snapshot, &multipart, false, true).is_err());
        let too_long = ExtensionStageRow::new("я".repeat(220), vec![]);
        assert!(
            validate_namespaced_lengths(&extension_namespace_prefix(EXTENSION_ID), &[too_long])
                .is_err()
        );
    }

    #[test]
    fn builder_rechecks_configinfo_mapping_and_selected_namespace() {
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [0; 8],
            zipped_info: vec![],
        };
        let mut plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        plan.rows[0].binary_data.push(99);
        assert!(matches!(
            build_configcassave_stage_sql("lab", &snapshot, &plan, false, true),
            Err(ExtensionStageError::InvalidConfigInfo(reason)) if reason.contains("does not match")
        ));

        let mut plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        plan.namespace_prefix = "00000000-0000-0000-0000-000000000000".to_owned();
        assert!(matches!(
            build_configcassave_stage_sql("lab", &snapshot, &plan, false, true),
            Err(ExtensionStageError::InvalidRow(reason)) if reason.contains("does not match selected")
        ));
    }

    #[test]
    fn bounds_descriptor_registry_blob_and_sql_script_before_encoding() {
        let mut too_large_identity = identity();
        too_large_identity.descriptor = vec![0; MAX_CONFIGINFO_DESCRIPTOR_BYTES + 1];
        assert!(matches!(
            generate_configinfo_manifest(&too_large_identity, &content_rows()),
            Err(ExtensionStageError::LimitExceeded(reason)) if reason.contains("descriptor")
        ));

        let snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [0; 8],
            zipped_info: vec![0; MAX_EXTENSION_ZIPPED_INFO_BYTES + 1],
        };
        let plan = prepare_extension_stage(EXTENSION_ID, &identity(), content_rows()).unwrap();
        assert!(matches!(
            build_configcassave_stage_sql("lab", &snapshot, &plan, false, true),
            Err(ExtensionStageError::LimitExceeded(reason)) if reason.contains("_ExtensionZippedInfo")
        ));

        let large_row =
            ExtensionStageRow::new("large", vec![0; MAX_STAGE_SQL_SCRIPT_BYTES / 2 + 1]);
        let bounded_snapshot = ExtensionRegistrySnapshot {
            extension_id: EXTENSION_ID,
            version: [0; 8],
            zipped_info: vec![],
        };
        assert!(matches!(
            estimate_stage_sql_bytes(
                "lab",
                &bounded_snapshot,
                &extension_namespace_prefix(EXTENSION_ID),
                &[large_row]
            ),
            Err(ExtensionStageError::LimitExceeded(reason)) if reason.contains("estimated SQL script")
        ));
    }

    #[test]
    fn sqlcmd_transport_uses_script_file_b_and_never_q() {
        let command = sqlcmd_command(
            Path::new("sqlcmd"),
            "server",
            None,
            None,
            true,
            Path::new("stage.sql"),
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.windows(2).any(|pair| pair == ["-i", "stage.sql"]));
        assert!(args.contains(&"-b".to_owned()));
        assert!(args.contains(&"-x".to_owned()));
        assert!(args.windows(2).any(|pair| pair == ["-f", "65001"]));
        assert!(args.contains(&"-E".to_owned()));
        assert!(args.contains(&"-C".to_owned()));
        assert!(!args.contains(&"-Q".to_owned()));
    }

    #[test]
    fn sqlcmd_nonzero_exit_is_propagated() {
        #[cfg(windows)]
        let status = {
            use std::os::windows::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(7)
        };
        #[cfg(unix)]
        let status = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(7 << 8)
        };
        let error = propagate_sqlcmd_exit(Output {
            status,
            stdout: Vec::new(),
            stderr: b"native sql error".to_vec(),
        })
        .unwrap_err();
        assert!(matches!(
            error,
            ExtensionStageError::SqlcmdFailed { code: Some(7), stderr }
                if stderr == "native sql error"
        ));
    }
}
