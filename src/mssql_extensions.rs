//! Read-only access to the SQL Server 8.3.27 configuration-extension registry.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use crate::cli::MssqlExtensionListArgs;

const EXTENSION_INFO_MAGIC_8_3_27: [u8; 4] = [0x43, 0xc2, 0x9a, 0x14];
const SHA1_BYTES: usize = 20;
const ENVELOPE_PREFIX_BYTES: usize = 4 + SHA1_BYTES;
const MAX_EXTENSION_ROWS: usize = 1_024;
const MAX_EXTENSION_NAME_BYTES: usize = 1_024;
const MAX_EXTENSION_ZIPPED_INFO_BYTES: usize = 4_000;
const MAX_REGISTRY_TRANSPORT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MssqlExtensionListReport {
    pub schema_version: u32,
    pub storage_profile: &'static str,
    pub database: String,
    pub extensions: Vec<MssqlExtensionInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MssqlExtensionInfo {
    #[serde(skip)]
    pub(crate) physical_registry_id: [u8; 16],
    #[serde(skip)]
    pub(crate) registry_version: [u8; 8],
    #[serde(skip)]
    pub(crate) zipped_info: Vec<u8>,
    pub registry_id: String,
    pub name: String,
    pub version: String,
    pub order: i64,
    pub active: bool,
    pub purpose: MssqlExtensionPurpose,
    pub scope: MssqlExtensionScope,
    pub safe_mode: MssqlExtensionField<bool>,
    pub security_profile_name: MssqlExtensionField<String>,
    pub unsafe_action_protection: MssqlExtensionField<bool>,
    pub used_in_distributed_infobase: bool,
    pub update_timestamp: String,
    pub active_cas_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "availability", content = "value", rename_all = "kebab-case")]
pub enum MssqlExtensionField<T> {
    Available(T),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MssqlExtensionPurpose {
    Patch,
    Customization,
    AddOn,
}

impl MssqlExtensionPurpose {
    fn decode(value: i64) -> std::result::Result<Self, ExtensionRegistryDecodeError> {
        match value {
            0 => Ok(Self::Patch),
            1 => Ok(Self::Customization),
            2 => Ok(Self::AddOn),
            value => Err(ExtensionRegistryDecodeError::UnsupportedPurpose(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MssqlExtensionScope {
    DataSeparation,
    Infobase,
}

impl MssqlExtensionScope {
    fn decode(value: i64) -> std::result::Result<Self, ExtensionRegistryDecodeError> {
        match value {
            0 => Ok(Self::DataSeparation),
            1 => Ok(Self::Infobase),
            value => Err(ExtensionRegistryDecodeError::UnsupportedScope(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedExtensionZippedInfo {
    pub active_cas_root: String,
    pub version: String,
    pub active: bool,
    pub safe_mode: bool,
    pub unsafe_action_protection: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionRegistryDecodeError {
    Truncated { actual: usize, minimum: usize },
    UnsupportedMagic([u8; 4]),
    UnsupportedEnvelopeHeader,
    UnsupportedMetadataEncoding(u8),
    InvalidMetadataLength,
    UnsupportedBoolean { field: &'static str, value: u8 },
    UnsupportedTrailer,
    InvalidVersionEncoding,
    InvalidUtf8Version,
    UnsupportedPurpose(i64),
    UnsupportedScope(i64),
    InvalidHex,
    InvalidRegistryIdLength(usize),
}

impl std::fmt::Display for ExtensionRegistryDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated { actual, minimum } => write!(
                formatter,
                "8.3.27 _ExtensionZippedInfo is truncated: {actual} bytes, need at least {minimum}"
            ),
            Self::UnsupportedMagic(actual) => write!(
                formatter,
                "unsupported _ExtensionZippedInfo magic {:02x?}; expected 8.3.27 magic {:02x?}",
                actual, EXTENSION_INFO_MAGIC_8_3_27
            ),
            Self::UnsupportedEnvelopeHeader => {
                formatter.write_str("unsupported 8.3.27 _ExtensionZippedInfo envelope header")
            }
            Self::UnsupportedMetadataEncoding(value) => {
                write!(
                    formatter,
                    "unsupported extension metadata encoding marker 0x{value:02x}"
                )
            }
            Self::InvalidMetadataLength => {
                formatter.write_str("extension metadata length exceeds the registry body")
            }
            Self::UnsupportedBoolean { field, value } => {
                write!(formatter, "unsupported {field} marker 0x{value:02x}")
            }
            Self::UnsupportedTrailer => {
                formatter.write_str("unsupported extension registry trailer")
            }
            Self::InvalidVersionEncoding => {
                formatter.write_str("invalid trailing extension version encoding")
            }
            Self::InvalidUtf8Version => formatter.write_str("extension version is not UTF-8"),
            Self::UnsupportedPurpose(value) => {
                write!(formatter, "unsupported _ExtensionUsePurpose value {value}")
            }
            Self::UnsupportedScope(value) => {
                write!(formatter, "unsupported _ExtensionScope value {value}")
            }
            Self::InvalidHex => formatter.write_str("invalid SQL binary hex value"),
            Self::InvalidRegistryIdLength(actual) => write!(
                formatter,
                "invalid _ExtensionsInfo._IDRRef length {actual}; expected 16 bytes"
            ),
        }
    }
}

impl std::error::Error for ExtensionRegistryDecodeError {}

/// Decode the bounded 8.3.27 registry envelope.
///
/// The first 20 bytes after the platform marker are the active ConfigCAS SHA-1
/// root. The parser then validates the evidenced fixed header, UTF-8/UTF-16LE
/// metadata field, exact version position, and fixed trailer. Unknown bodies
/// fail closed.
pub fn decode_extension_zipped_info(
    bytes: &[u8],
) -> std::result::Result<DecodedExtensionZippedInfo, ExtensionRegistryDecodeError> {
    const BODY_HEADER_BYTES: usize = 15;
    const TRAILER_BYTES: usize = 3;
    let minimum = ENVELOPE_PREFIX_BYTES + BODY_HEADER_BYTES + TRAILER_BYTES;
    if bytes.len() < minimum {
        return Err(ExtensionRegistryDecodeError::Truncated {
            actual: bytes.len(),
            minimum,
        });
    }
    let magic: [u8; 4] = bytes[..4].try_into().expect("checked slice length");
    if magic != EXTENSION_INFO_MAGIC_8_3_27 {
        return Err(ExtensionRegistryDecodeError::UnsupportedMagic(magic));
    }

    let body = &bytes[ENVELOPE_PREFIX_BYTES..];
    if body[..3] != [0xa1, 0x9a, 0x08] {
        return Err(ExtensionRegistryDecodeError::UnsupportedEnvelopeHeader);
    }
    let safe_mode = decode_bool("safe-mode", body[11])?;
    let unsafe_action_protection = decode_bool("unsafe-action-protection", body[12])?;
    let metadata_units = usize::from(body[14]);
    let metadata_bytes = match body[13] {
        0x9a => metadata_units,
        0x97 => metadata_units
            .checked_mul(2)
            .ok_or(ExtensionRegistryDecodeError::InvalidMetadataLength)?,
        value => {
            return Err(ExtensionRegistryDecodeError::UnsupportedMetadataEncoding(
                value,
            ));
        }
    };
    let metadata_end = BODY_HEADER_BYTES
        .checked_add(metadata_bytes)
        .ok_or(ExtensionRegistryDecodeError::InvalidMetadataLength)?;
    if metadata_end > body.len().saturating_sub(TRAILER_BYTES) {
        return Err(ExtensionRegistryDecodeError::InvalidMetadataLength);
    }
    validate_metadata_payload(body[13], &body[BODY_HEADER_BYTES..metadata_end])?;
    let trailer_start = body.len() - TRAILER_BYTES;
    let version = decode_exact_version(&body[metadata_end..trailer_start])?;
    let trailer = &body[trailer_start..];
    if trailer[2] != 0x20 || !matches!(trailer[1], 0x81 | 0x82) {
        return Err(ExtensionRegistryDecodeError::UnsupportedTrailer);
    }
    let active = decode_bool("active", trailer[0])?;

    Ok(DecodedExtensionZippedInfo {
        active_cas_root: hex_lower(&bytes[4..ENVELOPE_PREFIX_BYTES]),
        version,
        active,
        safe_mode,
        unsafe_action_protection,
    })
}

fn decode_bool(
    field: &'static str,
    value: u8,
) -> std::result::Result<bool, ExtensionRegistryDecodeError> {
    match value {
        0x81 => Ok(false),
        0x82 => Ok(true),
        value => Err(ExtensionRegistryDecodeError::UnsupportedBoolean { field, value }),
    }
}

fn validate_metadata_payload(
    discriminator: u8,
    payload: &[u8],
) -> std::result::Result<String, ExtensionRegistryDecodeError> {
    match discriminator {
        0x9a => std::str::from_utf8(payload)
            .map(str::to_owned)
            .map_err(|_| ExtensionRegistryDecodeError::InvalidUtf8Version),
        0x97 => {
            let units = payload
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| ExtensionRegistryDecodeError::InvalidUtf8Version)
        }
        value => Err(ExtensionRegistryDecodeError::UnsupportedMetadataEncoding(
            value,
        )),
    }
}

fn decode_exact_version(field: &[u8]) -> std::result::Result<String, ExtensionRegistryDecodeError> {
    if field == [0x81] {
        return Ok(String::new());
    }
    if field.len() < 2 || field[0] != 0x9a {
        return Err(ExtensionRegistryDecodeError::InvalidVersionEncoding);
    }
    let length = usize::from(field[1]);
    if field.len() != length + 2 {
        return Err(ExtensionRegistryDecodeError::InvalidVersionEncoding);
    }
    std::str::from_utf8(&field[2..])
        .map(str::to_owned)
        .map_err(|_| ExtensionRegistryDecodeError::InvalidUtf8Version)
}

struct RawExtensionRegistryRow {
    registry_id: String,
    registry_version: String,
    name: String,
    extension_order: i64,
    purpose: i64,
    scope: i64,
    used_in_distributed_infobase: bool,
    update_timestamp: String,
    zipped_info: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionRegistryTransportError {
    MissingSqlPassword { user: String, environment: String },
    InvalidPreflight,
    TooManyRows { actual: usize, maximum: usize },
    NameTooLarge { actual: usize, maximum: usize },
    BlobTooLarge { actual: usize, maximum: usize },
    ReportTooLarge { actual: usize, maximum: usize },
    InvalidRow { line: usize },
}

impl std::fmt::Display for ExtensionRegistryTransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSqlPassword { user, environment } => write!(
                formatter,
                "SQL login {user:?} requires --sql-pwd or non-empty environment variable {environment}"
            ),
            Self::InvalidPreflight => {
                formatter.write_str("invalid _ExtensionsInfo preflight result")
            }
            Self::TooManyRows { actual, maximum } => write!(
                formatter,
                "_ExtensionsInfo has {actual} rows; maximum supported is {maximum}"
            ),
            Self::NameTooLarge { actual, maximum } => write!(
                formatter,
                "extension name is {actual} bytes; maximum supported is {maximum}"
            ),
            Self::BlobTooLarge { actual, maximum } => write!(
                formatter,
                "_ExtensionZippedInfo is {actual} bytes; maximum supported is {maximum}"
            ),
            Self::ReportTooLarge { actual, maximum } => write!(
                formatter,
                "extension registry transport is {actual} bytes; maximum supported is {maximum}"
            ),
            Self::InvalidRow { line } => write!(formatter, "invalid extension registry row {line}"),
        }
    }
}

impl std::error::Error for ExtensionRegistryTransportError {}

pub fn list_extensions(args: &MssqlExtensionListArgs) -> Result<MssqlExtensionListReport> {
    let password = resolve_password(args)?;
    let preflight_stdout = run_sqlcmd(
        &args.sqlcmd,
        &args.server,
        args.sql_user.as_deref(),
        password.as_deref(),
        args.sqlcmd_trust_cert,
        &extension_registry_preflight_query(&args.database),
    )?;
    let bounds = parse_preflight(&preflight_stdout)?;
    validate_preflight(bounds)?;
    let stdout = run_sqlcmd(
        &args.sqlcmd,
        &args.server,
        args.sql_user.as_deref(),
        password.as_deref(),
        args.sqlcmd_trust_cert,
        &extension_registry_rows_query(&args.database),
    )?;
    if stdout.len() > MAX_REGISTRY_TRANSPORT_BYTES {
        return Err(ExtensionRegistryTransportError::ReportTooLarge {
            actual: stdout.len(),
            maximum: MAX_REGISTRY_TRANSPORT_BYTES,
        }
        .into());
    }
    let rows = parse_registry_rows(&stdout)?;
    if rows.len() > MAX_EXTENSION_ROWS {
        return Err(ExtensionRegistryTransportError::TooManyRows {
            actual: rows.len(),
            maximum: MAX_EXTENSION_ROWS,
        }
        .into());
    }
    if rows.len() != bounds.rows {
        return Err(ExtensionRegistryTransportError::InvalidPreflight.into());
    }
    let extensions = rows
        .into_iter()
        .map(decode_registry_row)
        .collect::<Result<Vec<_>>>()?;
    Ok(MssqlExtensionListReport {
        schema_version: 1,
        storage_profile: "mssql-extensions-info-8.3.27",
        database: args.database.clone(),
        extensions,
    })
}

fn decode_registry_row(row: RawExtensionRegistryRow) -> Result<MssqlExtensionInfo> {
    let bytes = decode_hex(&row.zipped_info)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot decode _ExtensionZippedInfo for {:?}", row.name))?;
    let decoded = decode_extension_zipped_info(&bytes)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot decode _ExtensionZippedInfo for {:?}", row.name))?;
    let registry_id_bytes = decode_hex(&row.registry_id).map_err(anyhow::Error::new)?;
    let physical_registry_id: [u8; 16] =
        registry_id_bytes.try_into().map_err(|bytes: Vec<u8>| {
            ExtensionRegistryDecodeError::InvalidRegistryIdLength(bytes.len())
        })?;
    let version_bytes = decode_hex(&row.registry_version).map_err(anyhow::Error::new)?;
    let registry_version: [u8; 8] = version_bytes
        .try_into()
        .map_err(|_| anyhow!("invalid _ExtensionsInfo._Version length for {:?}", row.name))?;
    let registry_id = decode_registry_uuid(&row.registry_id)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("cannot decode _IDRRef for {:?}", row.name))?;
    Ok(MssqlExtensionInfo {
        physical_registry_id,
        registry_version,
        zipped_info: bytes,
        registry_id,
        name: row.name,
        version: decoded.version,
        order: row.extension_order,
        active: decoded.active,
        purpose: MssqlExtensionPurpose::decode(row.purpose)?,
        scope: MssqlExtensionScope::decode(row.scope)?,
        safe_mode: MssqlExtensionField::Available(decoded.safe_mode),
        // No non-empty security profile was evidenced in the 8.3.27 fixture.
        // Keep absence explicit instead of guessing a field inside the envelope.
        security_profile_name: MssqlExtensionField::Unavailable,
        unsafe_action_protection: MssqlExtensionField::Available(decoded.unsafe_action_protection),
        used_in_distributed_infobase: row.used_in_distributed_infobase,
        update_timestamp: row.update_timestamp,
        active_cas_root: decoded.active_cas_root,
    })
}

/// Convert a 1C `binary(16)` reference to its canonical UUID spelling.
pub fn decode_registry_uuid(
    value: &str,
) -> std::result::Result<String, ExtensionRegistryDecodeError> {
    let bytes = decode_hex(value)?;
    if bytes.len() != 16 {
        return Err(ExtensionRegistryDecodeError::InvalidRegistryIdLength(
            bytes.len(),
        ));
    }
    // SQL stores the UUID as node(8), clock sequence(2), time-mid(2),
    // time-low(4), with each component already in network byte order.
    let canonical = [
        &bytes[12..16],
        &bytes[10..12],
        &bytes[8..10],
        &bytes[0..2],
        &bytes[2..8],
    ];
    Ok(format!(
        "{}-{}-{}-{}-{}",
        hex_lower(canonical[0]),
        hex_lower(canonical[1]),
        hex_lower(canonical[2]),
        hex_lower(canonical[3]),
        hex_lower(canonical[4])
    ))
}

#[derive(Clone, Copy)]
struct RegistryBounds {
    rows: usize,
    max_name_bytes: usize,
    max_blob_bytes: usize,
}

fn extension_registry_preflight_query(database: &str) -> String {
    format!(
        "SET NOCOUNT ON; SELECT COUNT_BIG(*), COALESCE(MAX(DATALENGTH(_ExtName)),0), \
         COALESCE(MAX(DATALENGTH(_ExtensionZippedInfo)),0) FROM {}.dbo._ExtensionsInfo;",
        quote_identifier(database)
    )
}

fn extension_registry_rows_query(database: &str) -> String {
    format!(
        "SET NOCOUNT ON; SELECT TOP ({}) LOWER(CONVERT(varchar(32), _IDRRef, 2)), \
         LOWER(CONVERT(varchar(16), CONVERT(varbinary(8), _Version), 2)), \
         CONVERT(varchar(2048), CONVERT(varbinary(1024), _ExtName), 2), \
         DATALENGTH(_ExtName), \
         CONVERT(bigint, _ExtensionOrder), CONVERT(bigint, _ExtensionUsePurpose), \
         CONVERT(bigint, _ExtensionScope), \
         CASE WHEN _UsedInDistributedInfoBase=0x00 THEN 0 ELSE 1 END, \
         CONVERT(varchar(33), DATEADD(year, -2000, _UpdateTime), 126), \
         CONVERT(varchar(8000), _ExtensionZippedInfo, 2), \
         DATALENGTH(_ExtensionZippedInfo) \
         FROM {}.dbo._ExtensionsInfo ORDER BY _ExtensionOrder, _ExtName;",
        MAX_EXTENSION_ROWS + 1,
        quote_identifier(database)
    )
}

fn quote_identifier(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn resolve_password(args: &MssqlExtensionListArgs) -> Result<Option<String>> {
    resolve_password_with(args, |name| std::env::var(name).ok())
}

fn resolve_password_with(
    args: &MssqlExtensionListArgs,
    environment: impl FnOnce(&str) -> Option<String>,
) -> Result<Option<String>> {
    let Some(user) = args.sql_user.as_ref() else {
        return Ok(None);
    };
    args.sql_pwd
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| environment(&args.sql_pwd_env).filter(|value| !value.is_empty()))
        .map(Some)
        .ok_or_else(|| {
            ExtensionRegistryTransportError::MissingSqlPassword {
                user: user.clone(),
                environment: args.sql_pwd_env.clone(),
            }
            .into()
        })
}

fn run_sqlcmd(
    executable: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    trust_server_certificate: bool,
    sql: &str,
) -> Result<String> {
    let mut command = Command::new(executable);
    command.arg("-S").arg(server);
    if let Some(user) = user {
        command.arg("-U").arg(user);
        if let Some(password) = password {
            command.env("SQLCMDPASSWORD", password);
        }
    } else {
        command.arg("-E");
    }
    if trust_server_certificate {
        command.arg("-C");
    }
    let output = command
        .arg("-f")
        .arg("65001")
        .arg("-b")
        .arg("-w")
        .arg("16384")
        .arg("-h")
        .arg("-1")
        .arg("-W")
        .arg("-s")
        .arg("|")
        .arg("-Q")
        .arg(sql)
        .output()
        .with_context(|| format!("failed to launch sqlcmd at {}", executable.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "sqlcmd failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_preflight(stdout: &str) -> Result<RegistryBounds> {
    let line = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or(ExtensionRegistryTransportError::InvalidPreflight)?;
    let values = line.split('|').map(str::trim).collect::<Vec<_>>();
    if values.len() != 3 {
        return Err(ExtensionRegistryTransportError::InvalidPreflight.into());
    }
    Ok(RegistryBounds {
        rows: values[0]
            .parse()
            .map_err(|_| ExtensionRegistryTransportError::InvalidPreflight)?,
        max_name_bytes: values[1]
            .parse()
            .map_err(|_| ExtensionRegistryTransportError::InvalidPreflight)?,
        max_blob_bytes: values[2]
            .parse()
            .map_err(|_| ExtensionRegistryTransportError::InvalidPreflight)?,
    })
}

fn validate_preflight(bounds: RegistryBounds) -> Result<()> {
    if bounds.rows > MAX_EXTENSION_ROWS {
        return Err(ExtensionRegistryTransportError::TooManyRows {
            actual: bounds.rows,
            maximum: MAX_EXTENSION_ROWS,
        }
        .into());
    }
    if bounds.max_name_bytes > MAX_EXTENSION_NAME_BYTES {
        return Err(ExtensionRegistryTransportError::NameTooLarge {
            actual: bounds.max_name_bytes,
            maximum: MAX_EXTENSION_NAME_BYTES,
        }
        .into());
    }
    if bounds.max_blob_bytes > MAX_EXTENSION_ZIPPED_INFO_BYTES {
        return Err(ExtensionRegistryTransportError::BlobTooLarge {
            actual: bounds.max_blob_bytes,
            maximum: MAX_EXTENSION_ZIPPED_INFO_BYTES,
        }
        .into());
    }
    let estimated = bounds.rows.saturating_mul(
        bounds
            .max_name_bytes
            .saturating_mul(2)
            .saturating_add(bounds.max_blob_bytes.saturating_mul(2))
            .saturating_add(256),
    );
    if estimated > MAX_REGISTRY_TRANSPORT_BYTES {
        return Err(ExtensionRegistryTransportError::ReportTooLarge {
            actual: estimated,
            maximum: MAX_REGISTRY_TRANSPORT_BYTES,
        }
        .into());
    }
    Ok(())
}

fn parse_registry_rows(stdout: &str) -> Result<Vec<RawExtensionRegistryRow>> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| parse_registry_row(index + 1, line))
        .collect()
}

fn parse_registry_row(line_number: usize, line: &str) -> Result<RawExtensionRegistryRow> {
    let fields = line.split('|').map(str::trim).collect::<Vec<_>>();
    if fields.len() != 11 {
        return Err(ExtensionRegistryTransportError::InvalidRow { line: line_number }.into());
    }
    let invalid = || ExtensionRegistryTransportError::InvalidRow { line: line_number };
    let name_bytes: usize = fields[3].parse().map_err(|_| invalid())?;
    let blob_bytes: usize = fields[10].parse().map_err(|_| invalid())?;
    if name_bytes > MAX_EXTENSION_NAME_BYTES {
        return Err(ExtensionRegistryTransportError::NameTooLarge {
            actual: name_bytes,
            maximum: MAX_EXTENSION_NAME_BYTES,
        }
        .into());
    }
    if blob_bytes > MAX_EXTENSION_ZIPPED_INFO_BYTES {
        return Err(ExtensionRegistryTransportError::BlobTooLarge {
            actual: blob_bytes,
            maximum: MAX_EXTENSION_ZIPPED_INFO_BYTES,
        }
        .into());
    }
    if fields[0].len() != 32
        || fields[1].len() != 16
        || fields[2].len() != name_bytes.saturating_mul(2)
        || fields[9].len() != blob_bytes.saturating_mul(2)
    {
        return Err(invalid().into());
    }
    Ok(RawExtensionRegistryRow {
        registry_id: fields[0].to_owned(),
        registry_version: fields[1].to_owned(),
        name: decode_utf16_hex(fields[2]).map_err(|_| invalid())?,
        extension_order: fields[4].parse().map_err(|_| invalid())?,
        purpose: fields[5].parse().map_err(|_| invalid())?,
        scope: fields[6].parse().map_err(|_| invalid())?,
        used_in_distributed_infobase: match fields[7] {
            "0" => false,
            "1" => true,
            _ => return Err(invalid().into()),
        },
        update_timestamp: fields[8].to_owned(),
        zipped_info: fields[9].to_owned(),
    })
}

fn decode_utf16_hex(value: &str) -> std::result::Result<String, ExtensionRegistryDecodeError> {
    let bytes = decode_hex(value)?;
    if !bytes.len().is_multiple_of(2) {
        return Err(ExtensionRegistryDecodeError::InvalidHex);
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|_| ExtensionRegistryDecodeError::InvalidHex)
}

fn decode_hex(value: &str) -> std::result::Result<Vec<u8>, ExtensionRegistryDecodeError> {
    if !value.len().is_multiple_of(2) {
        return Err(ExtensionRegistryDecodeError::InvalidHex);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0])?;
            let low = hex_digit(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_digit(value: u8) -> std::result::Result<u8, ExtensionRegistryDecodeError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(ExtensionRegistryDecodeError::InvalidHex),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(HEX[usize::from(byte >> 4)] as char);
        result.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    result
}

pub fn render_extension_table(report: &MssqlExtensionListReport) -> String {
    let mut output = String::new();
    for extension in &report.extensions {
        let _ = writeln!(
            output,
            "order={} active={} safe={} unsafe={} distributed={} purpose={} scope={} version={} name={} cas_root={} updated={} security_profile={}",
            extension.order,
            yes_no(extension.active),
            render_bool_field(&extension.safe_mode),
            render_bool_field(&extension.unsafe_action_protection),
            yes_no(extension.used_in_distributed_infobase),
            purpose_name(extension.purpose),
            scope_name(extension.scope),
            serde_json::to_string(&extension.version).expect("string JSON cannot fail"),
            serde_json::to_string(&extension.name).expect("string JSON cannot fail"),
            extension.active_cas_root,
            extension.update_timestamp,
            render_string_field(&extension.security_profile_name)
        );
    }
    output
}

fn render_bool_field(value: &MssqlExtensionField<bool>) -> &'static str {
    match value {
        MssqlExtensionField::Available(value) => yes_no(*value),
        MssqlExtensionField::Unavailable => "unavailable",
    }
}

fn render_string_field(value: &MssqlExtensionField<String>) -> String {
    match value {
        MssqlExtensionField::Available(value) => {
            serde_json::to_string(value).expect("string JSON cannot fail")
        }
        MssqlExtensionField::Unavailable => "unavailable".to_owned(),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn purpose_name(value: MssqlExtensionPurpose) -> &'static str {
    match value {
        MssqlExtensionPurpose::Patch => "patch",
        MssqlExtensionPurpose::Customization => "customization",
        MssqlExtensionPurpose::AddOn => "add-on",
    }
}

fn scope_name(value: MssqlExtensionScope) -> &'static str {
    match value {
        MssqlExtensionScope::DataSeparation => "data-separation",
        MssqlExtensionScope::Infobase => "infobase",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SERVICE_DESK_HEX: &str = "43C29A14E1A4957CD700E47CEA9AD20E66D489F9E5B0BAC2A19A080000000000015EC281819A437B2223222C38373032343733382D666332612D343433362D616461312D6466373964333935633432342C0A7B312C227275222C22536572766963654465736B227D0A7D9A07312E372E302E30828120";
    const DEMO_EMPTY_HEX: &str = "43C29A144D6D417AD243F0CB8ECC3969CBD7AA2B429FD1AFA19A0800000001000000008181974F7B002200230022002C00380037003000320034003700330038002D0066006300320061002D0034003400330036002D0061006400610031002D006400660037003900640033003900350063003400320034002C000A007B0031002C0022007200750022002C002200140435043C043E043A0020001F044304410442043E043504200040043004410448043804400435043D043804350422007D000A007D009A07312E302E312E31828120";
    const AI_AGENT_EMPTY_VERSION_HEX: &str = "43C29A144A5D722F8572778E07ECD4315F5A4612EF27BC54A19A0800000000000127F28181973F7B002200230022002C00380037003000320034003700330038002D0066006300320061002D0034003400330036002D0061006400610031002D006400660037003900640033003900350063003400320034002C000A007B0031002C0022007200750022002C002200100418041004330435043D04420422007D000A007D0081828220";

    #[test]
    fn decodes_observed_8_3_27_utf8_registry_fixture() {
        let decoded = decode_extension_zipped_info(&decode_hex(SERVICE_DESK_HEX).unwrap()).unwrap();
        assert_eq!(
            decoded.active_cas_root,
            "e1a4957cd700e47cea9ad20e66d489f9e5b0bac2"
        );
        assert_eq!(decoded.version, "1.7.0.0");
        assert!(decoded.active);
        assert!(!decoded.safe_mode);
        assert!(!decoded.unsafe_action_protection);
    }

    #[test]
    fn decodes_observed_8_3_27_utf16_metadata_fixture_without_reading_metadata() {
        let decoded = decode_extension_zipped_info(&decode_hex(DEMO_EMPTY_HEX).unwrap()).unwrap();
        assert_eq!(
            decoded.active_cas_root,
            "4d6d417ad243f0cb8ecc3969cbd7aa2b429fd1af"
        );
        assert_eq!(decoded.version, "1.0.1.1");
        assert!(decoded.active);
        assert!(!decoded.safe_mode);
    }

    #[test]
    fn decodes_evidenced_empty_version_at_exact_field_position() {
        let decoded =
            decode_extension_zipped_info(&decode_hex(AI_AGENT_EMPTY_VERSION_HEX).unwrap()).unwrap();
        assert_eq!(decoded.version, "");
        assert!(decoded.active);
    }

    #[test]
    fn decodes_true_security_boolean_markers() {
        let mut bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        bytes[ENVELOPE_PREFIX_BYTES + 11] = 0x82;
        bytes[ENVELOPE_PREFIX_BYTES + 12] = 0x82;
        let decoded = decode_extension_zipped_info(&bytes).unwrap();
        assert!(decoded.safe_mode);
        assert!(decoded.unsafe_action_protection);
    }

    #[test]
    fn rejects_unknown_shape_instead_of_guessing() {
        let mut bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        bytes[0] = 0xff;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::UnsupportedMagic(_))
        ));
        bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        bytes[ENVELOPE_PREFIX_BYTES] = 0xff;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::UnsupportedEnvelopeHeader)
        ));
        bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        bytes[ENVELOPE_PREFIX_BYTES + 13] = 0xff;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::UnsupportedMetadataEncoding(
                0xff
            ))
        ));
        bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        bytes[ENVELOPE_PREFIX_BYTES + 14] = 0xff;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::InvalidMetadataLength)
        ));
        bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        let metadata_length = usize::from(bytes[ENVELOPE_PREFIX_BYTES + 14]);
        let version_marker = ENVELOPE_PREFIX_BYTES + 15 + metadata_length;
        bytes[version_marker] = 0x81;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::InvalidVersionEncoding)
        ));
        bytes = decode_hex(SERVICE_DESK_HEX).unwrap();
        let last = bytes.len() - 1;
        bytes[0..4].copy_from_slice(&EXTENSION_INFO_MAGIC_8_3_27);
        bytes[last] = 0xff;
        assert!(matches!(
            decode_extension_zipped_info(&bytes),
            Err(ExtensionRegistryDecodeError::UnsupportedTrailer)
        ));
    }

    #[test]
    fn query_quotes_database_identifier() {
        let preflight = extension_registry_preflight_query("db]name");
        let rows = extension_registry_rows_query("db]name");
        assert!(preflight.contains("FROM [db]]name].dbo._ExtensionsInfo"));
        assert!(rows.contains("FROM [db]]name].dbo._ExtensionsInfo"));
        assert!(rows.contains("SELECT TOP (1025)"));
        assert!(rows.matches("DATALENGTH(").count() >= 2);
        assert!(!rows.contains("varchar(max)"));
        assert!(!rows.contains("FOR JSON"));
    }

    #[test]
    fn decodes_1c_binary_reference_as_canonical_uuid() {
        assert_eq!(
            decode_registry_uuid("8780107B44A2858A11E80773347EEA01").unwrap(),
            "347eea01-0773-11e8-8780-107b44a2858a"
        );
    }

    #[test]
    fn transport_preflight_rejects_every_oversize_dimension() {
        for (bounds, expected) in [
            (
                RegistryBounds {
                    rows: MAX_EXTENSION_ROWS + 1,
                    max_name_bytes: 1,
                    max_blob_bytes: 1,
                },
                "rows",
            ),
            (
                RegistryBounds {
                    rows: 1,
                    max_name_bytes: MAX_EXTENSION_NAME_BYTES + 1,
                    max_blob_bytes: 1,
                },
                "name",
            ),
            (
                RegistryBounds {
                    rows: 1,
                    max_name_bytes: 1,
                    max_blob_bytes: MAX_EXTENSION_ZIPPED_INFO_BYTES + 1,
                },
                "blob",
            ),
        ] {
            let error = validate_preflight(bounds).unwrap_err();
            let transport = error
                .downcast_ref::<ExtensionRegistryTransportError>()
                .unwrap();
            assert!(match (expected, transport) {
                ("rows", ExtensionRegistryTransportError::TooManyRows { .. })
                | ("name", ExtensionRegistryTransportError::NameTooLarge { .. })
                | ("blob", ExtensionRegistryTransportError::BlobTooLarge { .. }) => true,
                _ => false,
            });
        }
        let error = validate_preflight(RegistryBounds {
            rows: MAX_EXTENSION_ROWS,
            max_name_bytes: MAX_EXTENSION_NAME_BYTES,
            max_blob_bytes: MAX_EXTENSION_ZIPPED_INFO_BYTES,
        })
        .unwrap_err();
        assert!(
            error
                .downcast_ref::<ExtensionRegistryTransportError>()
                .is_some_and(|error| matches!(
                    error,
                    ExtensionRegistryTransportError::ReportTooLarge { .. }
                ))
        );
    }

    fn args(user: Option<&str>, password: Option<&str>) -> MssqlExtensionListArgs {
        MssqlExtensionListArgs {
            sqlcmd: PathBuf::from("sqlcmd"),
            server: "localhost".to_owned(),
            sql_user: user.map(str::to_owned),
            sql_pwd: password.map(str::to_owned),
            sql_pwd_env: "TEST_PASSWORD".to_owned(),
            sqlcmd_trust_cert: false,
            database: "db".to_owned(),
            format: crate::cli::MssqlExtensionListFormat::Json,
        }
    }

    #[test]
    fn sql_password_resolution_covers_inline_environment_integrated_and_missing() {
        assert_eq!(
            resolve_password_with(&args(Some("sa"), Some("inline")), |_| None).unwrap(),
            Some("inline".to_owned())
        );
        assert_eq!(
            resolve_password_with(&args(Some("sa"), None), |_| Some("environment".to_owned()))
                .unwrap(),
            Some("environment".to_owned())
        );
        assert_eq!(
            resolve_password_with(&args(None, None), |_| None).unwrap(),
            None
        );
        let error = resolve_password_with(&args(Some("sa"), None), |_| None).unwrap_err();
        assert!(
            error
                .downcast_ref::<ExtensionRegistryTransportError>()
                .is_some_and(|error| matches!(
                    error,
                    ExtensionRegistryTransportError::MissingSqlPassword { .. }
                ))
        );
    }

    fn snapshot_report() -> MssqlExtensionListReport {
        MssqlExtensionListReport {
            schema_version: 1,
            storage_profile: "mssql-extensions-info-8.3.27",
            database: "Тестовая база".to_owned(),
            extensions: vec![MssqlExtensionInfo {
                physical_registry_id: [0; 16],
                registry_version: [0; 8],
                zipped_info: Vec::new(),
                registry_id: "347eea01-0773-11e8-8780-107b44a2858a".to_owned(),
                name: "Русское расширение с пробелом".to_owned(),
                version: String::new(),
                order: 1,
                active: true,
                purpose: MssqlExtensionPurpose::Customization,
                scope: MssqlExtensionScope::Infobase,
                safe_mode: MssqlExtensionField::Available(false),
                security_profile_name: MssqlExtensionField::Unavailable,
                unsafe_action_protection: MssqlExtensionField::Available(false),
                used_in_distributed_infobase: false,
                update_timestamp: "2026-08-29T10:00:00".to_owned(),
                active_cas_root: "4d6d417ad243f0cb8ecc3969cbd7aa2b429fd1af".to_owned(),
            }],
        }
    }

    #[test]
    fn json_snapshot_is_stable_and_explicit_about_unavailable_fields() {
        let json = serde_json::to_string_pretty(&snapshot_report()).unwrap();
        assert_eq!(
            json,
            include_str!("../tests/fixtures/mssql_extension_list_snapshot.json").trim()
        );
    }

    #[test]
    fn table_snapshot_quotes_names_and_empty_versions_unambiguously() {
        assert_eq!(
            render_extension_table(&snapshot_report()),
            "order=1 active=yes safe=no unsafe=no distributed=no purpose=customization scope=infobase version=\"\" name=\"Русское расширение с пробелом\" cas_root=4d6d417ad243f0cb8ecc3969cbd7aa2b429fd1af updated=2026-08-29T10:00:00 security_profile=unavailable\n"
        );
    }
}
