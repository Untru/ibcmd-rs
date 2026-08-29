//! Typed access to the 8.3.27 MSSQL configuration-extension CAS.
//!
//! `ConfigCAS` is not a flat configuration table.  The registry points at a
//! SHA-1 named, raw-deflated manifest.  That manifest maps the configuration's
//! logical storage names to SHA-1 named, raw-deflated payload rows.  Resolving
//! the manifest first is important: fetching all CAS rows and treating their
//! hashes as storage names would mix independent extensions.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use flate2::read::DeflateDecoder;
use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::storage::{
    CompressionKind, MAX_STORAGE_ENTRIES, MAX_STORAGE_IMAGE_RETAINED_BYTES,
    MAX_STORAGE_PAYLOAD_BYTES, MultipartIdentity, OpaqueStorageMetadata, StorageBuildError,
    StorageEntry, StorageImage, StorageKey, StorageName, StorageOrigin, StoragePayloads,
    StorageProvenance,
};
use sha1::{Digest, Sha1};

use super::config_rows::BinaryConfigRow;
use super::fetch::{
    BCP_INLINE_QUERY_MAX_CHARS, fetch_binary_rows_bcp, run_sql_capture_tsv,
    split_selected_file_names_for_bcp_query,
};

const CAS_HASH_BYTES: usize = 20;
const CAS_HASH_HEX_CHARS: usize = CAS_HASH_BYTES * 2;
const MAX_CAS_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_CAS_FETCH_PACKED_BYTES: usize = MAX_STORAGE_IMAGE_RETAINED_BYTES / 2;
const MAX_CAS_FETCH_PHYSICAL_ROWS: usize = MAX_STORAGE_ENTRIES;

/// A supported physical MSSQL configuration storage table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MssqlStorageTable {
    Config,
    ConfigSave,
    ConfigCas,
    ConfigCasSave,
}

impl MssqlStorageTable {
    pub const fn sql_name(self) -> &'static str {
        match self {
            Self::Config => "Config",
            Self::ConfigSave => "ConfigSave",
            Self::ConfigCas => "ConfigCAS",
            Self::ConfigCasSave => "ConfigCASSave",
        }
    }

    pub const fn is_content_addressed(self) -> bool {
        matches!(self, Self::ConfigCas)
    }
}

/// Exact SHA-1 identity used as a `ConfigCAS.FileName`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CasHash([u8; CAS_HASH_BYTES]);

impl CasHash {
    pub fn parse_hex(value: &str) -> std::result::Result<Self, CasGraphError> {
        if value.len() != CAS_HASH_HEX_CHARS || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(CasGraphError::InvalidHash(value.to_owned()));
        }
        let mut bytes = [0_u8; CAS_HASH_BYTES];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_value(pair[0]).expect("validated hex") << 4)
                | hex_value(pair[1]).expect("validated hex");
        }
        Ok(Self(bytes))
    }

    fn from_base64(value: &str) -> std::result::Result<Self, CasGraphError> {
        let decoded = crate::module_blob::decode_base64_mime(value).ok_or_else(|| {
            CasGraphError::InvalidManifest("invalid CAS digest base64".to_owned())
        })?;
        let bytes: [u8; CAS_HASH_BYTES] = decoded.try_into().map_err(|decoded: Vec<u8>| {
            CasGraphError::InvalidManifest(format!(
                "CAS digest has {} bytes instead of {CAS_HASH_BYTES}",
                decoded.len()
            ))
        })?;
        Ok(Self(bytes))
    }

    pub fn for_packed_bytes(bytes: &[u8]) -> Self {
        Self(Sha1::digest(bytes).into())
    }

    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(CAS_HASH_HEX_CHARS);
        for byte in self.0 {
            use std::fmt::Write as _;
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }

    pub const fn as_bytes(&self) -> &[u8; CAS_HASH_BYTES] {
        &self.0
    }
}

impl Display for CasHash {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

/// One assembled physical CAS row. Multipart SQL rows are joined by the
/// existing bounded BCP reader before reaching this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasStorageRow {
    pub hash: CasHash,
    pub packed: Vec<u8>,
}

impl CasStorageRow {
    pub fn new(hash: CasHash, packed: Vec<u8>) -> Self {
        Self { hash, packed }
    }

    fn try_from_binary(row: BinaryConfigRow) -> std::result::Result<Self, CasGraphError> {
        if row.part_no != 0 {
            return Err(CasGraphError::InvalidRow {
                hash: row.file_name,
                reason: format!("assembled row has non-zero part {}", row.part_no),
            });
        }
        if row.data_size < 0 || row.data_size as usize != row.binary.len() {
            return Err(CasGraphError::InvalidRow {
                hash: row.file_name,
                reason: format!(
                    "DataSize {} does not match {} packed bytes",
                    row.data_size,
                    row.binary.len()
                ),
            });
        }
        Ok(Self {
            hash: CasHash::parse_hex(&row.file_name)?,
            packed: row.binary,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasManifestEntry {
    pub logical_name: String,
    pub content_hash: CasHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CasManifest {
    entries: Vec<CasManifestEntry>,
    identity: crate::mssql_extension_stage::ConfigInfoIdentity,
}

impl CasManifest {
    pub fn decode(packed_root: &[u8]) -> std::result::Result<Self, CasGraphError> {
        let plain = inflate_bounded(packed_root, MAX_CAS_MANIFEST_BYTES)
            .map_err(CasGraphError::InvalidRootCompression)?;
        let text = std::str::from_utf8(&plain).map_err(|error| {
            CasGraphError::InvalidManifest(format!("manifest is not UTF-8: {error}"))
        })?;
        let decoded = crate::mssql_extension_stage::validate_configinfo_manifest(text.as_bytes())
            .map_err(|error| CasGraphError::InvalidManifest(error.to_string()))?;
        let mut names = BTreeSet::new();
        let mut entries = Vec::with_capacity(decoded.entries.len());
        for item in decoded.entries {
            let logical_name = item.logical_name;
            if !names.insert(logical_name.clone()) {
                return Err(CasGraphError::DuplicateLogicalName(logical_name));
            }
            let content_hash = CasHash(item.packed_sha1);
            entries.push(CasManifestEntry {
                logical_name,
                content_hash,
            });
        }
        Ok(Self {
            entries,
            identity: decoded.identity,
        })
    }

    pub fn entries(&self) -> &[CasManifestEntry] {
        &self.entries
    }

    pub const fn identity(&self) -> &crate::mssql_extension_stage::ConfigInfoIdentity {
        &self.identity
    }

    pub fn referenced_hashes(&self) -> BTreeSet<CasHash> {
        self.entries
            .iter()
            .map(|entry| entry.content_hash)
            .collect()
    }
}

/// Materializes exactly the logical files reachable from `root_hash`.
/// Unreferenced rows are deliberately ignored, including rows belonging to
/// another extension.
pub fn resolve_cas_storage_image(
    root_hash: CasHash,
    rows: impl IntoIterator<Item = CasStorageRow>,
) -> std::result::Result<StorageImage, CasGraphError> {
    let mut by_hash = BTreeMap::new();
    let mut row_count = 0_usize;
    let mut packed_bytes = 0_usize;
    for row in rows {
        row_count = checked_budget_add(
            row_count,
            1,
            MAX_STORAGE_ENTRIES.saturating_add(1),
            "assembled CAS rows",
        )?;
        packed_bytes = checked_budget_add(
            packed_bytes,
            row.packed.len(),
            MAX_CAS_FETCH_PACKED_BYTES.saturating_add(MAX_CAS_MANIFEST_BYTES),
            "CAS packed bytes",
        )?;
        let row_hash = row.hash;
        if by_hash.insert(row_hash, row).is_some() {
            return Err(CasGraphError::DuplicateRow(row_hash));
        }
    }
    let root = by_hash
        .get(&root_hash)
        .ok_or(CasGraphError::MissingRoot(root_hash))?;
    if root.packed.len() > MAX_CAS_MANIFEST_BYTES {
        return Err(CasGraphError::ResourceBudgetExceeded {
            resource: "packed CAS root",
            maximum: MAX_CAS_MANIFEST_BYTES,
            actual: root.packed.len(),
        });
    }
    validate_content_hash(root)?;
    let manifest = CasManifest::decode(&root.packed)?;

    let source_profile = StorageProfileId::parse("storage:mssql-configcas")
        .expect("static CAS storage profile is valid");
    let provenance = StorageProvenance::new(&format!("mssql:configcas-root:{root_hash}"))
        .expect("CAS root provenance is bounded and valid");
    let origin = StorageOrigin::new(source_profile, provenance);
    let mut entries = Vec::with_capacity(manifest.entries.len());
    let mut retained_bytes = 0_usize;
    for reference in manifest.entries {
        if reference.content_hash == root_hash {
            return Err(CasGraphError::RootSelfReference {
                logical_name: reference.logical_name,
                hash: root_hash,
            });
        }
        let row =
            by_hash
                .get(&reference.content_hash)
                .ok_or_else(|| CasGraphError::MissingContent {
                    logical_name: reference.logical_name.clone(),
                    hash: reference.content_hash,
                })?;
        validate_content_hash(row)?;
        let fixed_retained_bytes =
            entry_fixed_retained_bytes(&reference.logical_name, row.packed.len(), &origin)?;
        let retained_before_unpack = checked_budget_add(
            retained_bytes,
            fixed_retained_bytes,
            MAX_STORAGE_IMAGE_RETAINED_BYTES,
            "CAS storage image retained bytes",
        )?;
        let maximum_unpacked_bytes = MAX_STORAGE_IMAGE_RETAINED_BYTES
            .saturating_sub(retained_before_unpack)
            .min(MAX_STORAGE_PAYLOAD_BYTES);
        let unpacked = inflate_bounded(&row.packed, maximum_unpacked_bytes).map_err(|reason| {
            CasGraphError::InvalidContentCompression {
                logical_name: reference.logical_name.clone(),
                hash: reference.content_hash,
                reason,
            }
        })?;
        let entry = StorageEntry::new(
            StorageName::new(&reference.logical_name)?,
            // A digest can be shared by several logical files. StorageImage
            // keys describe logical identity, while the physical CAS address
            // is retained verbatim in opaque metadata.
            StorageKey::new(&reference.logical_name)?,
            MultipartIdentity::single(),
            OpaqueStorageMetadata::new(reference.content_hash.as_bytes().to_vec(), Vec::new())?,
            StoragePayloads::new(row.packed.clone(), unpacked)?,
            CompressionKind::raw_deflate(),
            origin.clone(),
        )?;
        retained_bytes = checked_budget_add(
            retained_bytes,
            entry.retained_byte_len()?,
            MAX_STORAGE_IMAGE_RETAINED_BYTES,
            "CAS storage image retained bytes",
        )?;
        entries.push(entry);
    }
    StorageImage::new(entries).map_err(Into::into)
}

/// Fetches the manifest first and then only its referenced CAS rows.
pub fn fetch_cas_storage_image_bcp(
    sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: MssqlStorageTable,
    root_hash: CasHash,
) -> Result<StorageImage> {
    Ok(fetch_cas_storage_image_with_manifest_bcp(
        sqlcmd, bcp, server, user, password, database, table, root_hash,
    )?
    .0)
}

/// Fetches one reachable CAS graph and retains its configuration identity for
/// extension staging.
pub fn fetch_cas_storage_image_with_manifest_bcp(
    sqlcmd: &Path,
    bcp: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: MssqlStorageTable,
    root_hash: CasHash,
) -> Result<(StorageImage, CasManifest)> {
    if !table.is_content_addressed() {
        bail!(
            "{} is not a content-addressed storage table",
            table.sql_name()
        );
    }
    let root_names = BTreeSet::from([root_hash.to_hex()]);
    preflight_cas_fetch(
        sqlcmd,
        server,
        user,
        password,
        database,
        table,
        &root_names,
        MAX_CAS_MANIFEST_BYTES,
    )?;
    let root_rows = fetch_binary_rows_bcp(
        sqlcmd,
        bcp,
        server,
        user,
        password,
        database,
        table.sql_name(),
        &root_names,
        false,
    )?;
    let root_rows = root_rows
        .into_iter()
        .map(CasStorageRow::try_from_binary)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let root = root_rows
        .iter()
        .find(|row| row.hash == root_hash)
        .ok_or(CasGraphError::MissingRoot(root_hash))?;
    validate_content_hash(root)?;
    let manifest = CasManifest::decode(&root.packed)?;
    let child_names = manifest
        .referenced_hashes()
        .into_iter()
        .map(CasHash::to_hex)
        .collect::<BTreeSet<_>>();
    if child_names.contains(&root_hash.to_hex()) {
        bail!("CAS root {root_hash} references itself");
    }
    preflight_manifest_names(&manifest)?;
    let child_rows = if child_names.is_empty() {
        Vec::new()
    } else {
        preflight_cas_fetch(
            sqlcmd,
            server,
            user,
            password,
            database,
            table,
            &child_names,
            MAX_CAS_FETCH_PACKED_BYTES,
        )?;
        fetch_binary_rows_bcp(
            sqlcmd,
            bcp,
            server,
            user,
            password,
            database,
            table.sql_name(),
            &child_names,
            false,
        )?
    };
    let mut rows = root_rows;
    rows.extend(
        child_rows
            .into_iter()
            .map(CasStorageRow::try_from_binary)
            .collect::<std::result::Result<Vec<_>, _>>()?,
    );
    let image = resolve_cas_storage_image(root_hash, rows)
        .context("failed to resolve selected CAS root")?;
    Ok((image, manifest))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CasFetchStats {
    physical_rows: usize,
    logical_rows: usize,
    packed_bytes: usize,
}

fn preflight_cas_fetch(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: MssqlStorageTable,
    selected_hashes: &BTreeSet<String>,
    maximum_packed_bytes: usize,
) -> Result<()> {
    if selected_hashes.is_empty() {
        return Ok(());
    }
    let batches = split_selected_file_names_for_bcp_query(
        database,
        table.sql_name(),
        selected_hashes,
        BCP_INLINE_QUERY_MAX_CHARS,
    );
    let mut total = CasFetchStats::default();
    for batch in batches {
        let sql = build_cas_fetch_stats_sql(database, table, &batch);
        let stdout = run_sql_capture_tsv(sqlcmd, server, user, password, &sql)?;
        let batch = parse_cas_fetch_stats(&stdout)?;
        total.physical_rows = checked_budget_add(
            total.physical_rows,
            batch.physical_rows,
            MAX_CAS_FETCH_PHYSICAL_ROWS,
            "CAS physical rows",
        )?;
        total.logical_rows = checked_budget_add(
            total.logical_rows,
            batch.logical_rows,
            MAX_STORAGE_ENTRIES,
            "CAS logical rows",
        )?;
        total.packed_bytes = checked_budget_add(
            total.packed_bytes,
            batch.packed_bytes,
            maximum_packed_bytes,
            "CAS BCP packed bytes",
        )?;
    }
    if total.logical_rows != selected_hashes.len() {
        return Err(CasGraphError::MissingPreflightRows {
            expected: selected_hashes.len(),
            actual: total.logical_rows,
        }
        .into());
    }
    Ok(())
}

fn build_cas_fetch_stats_sql(
    database: &str,
    table: MssqlStorageTable,
    selected_hashes: &BTreeSet<String>,
) -> String {
    let values = selected_hashes
        .iter()
        .map(|value| format!("N'{}'", super::quote_string(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SET NOCOUNT ON;\n\
         SELECT COUNT_BIG(*) AS physical_rows,\n\
                COUNT_BIG(DISTINCT FileName) AS logical_rows,\n\
                COALESCE(SUM(CONVERT(decimal(38,0), DATALENGTH(BinaryData))), 0) AS packed_bytes\n\
         FROM {}\n\
         WHERE FileName IN ({values});",
        super::qualified_storage_table(database, table.sql_name())
    )
}

fn parse_cas_fetch_stats(stdout: &str) -> std::result::Result<CasFetchStats, CasGraphError> {
    let mut parsed = None;
    for line in stdout.lines().map(str::trim_end) {
        if line.trim().is_empty()
            || line.contains("physical_rows")
            || line.chars().all(|ch| ch == '-' || ch == '\t' || ch == ' ')
        {
            continue;
        }
        let fields = line.split('\t').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 3 {
            continue;
        }
        let values = fields
            .iter()
            .map(|value| value.parse::<usize>())
            .collect::<std::result::Result<Vec<_>, _>>();
        let Ok(values) = values else {
            continue;
        };
        if parsed.is_some() {
            return Err(CasGraphError::InvalidPreflightOutput(
                "more than one aggregate row".to_owned(),
            ));
        }
        parsed = Some(CasFetchStats {
            physical_rows: values[0],
            logical_rows: values[1],
            packed_bytes: values[2],
        });
    }
    parsed
        .ok_or_else(|| CasGraphError::InvalidPreflightOutput("aggregate row is missing".to_owned()))
}

fn preflight_manifest_names(manifest: &CasManifest) -> std::result::Result<(), CasGraphError> {
    let mut retained_name_bytes = 0_usize;
    for entry in manifest.entries() {
        StorageName::new(&entry.logical_name)?;
        StorageKey::new(&entry.logical_name)?;
        retained_name_bytes = checked_budget_add(
            retained_name_bytes,
            entry.logical_name.len().saturating_mul(2),
            MAX_STORAGE_IMAGE_RETAINED_BYTES,
            "CAS manifest logical-name bytes",
        )?;
    }
    Ok(())
}

fn entry_fixed_retained_bytes(
    logical_name: &str,
    packed_bytes: usize,
    origin: &StorageOrigin,
) -> std::result::Result<usize, CasGraphError> {
    [
        logical_name.len(),
        logical_name.len(),
        CAS_HASH_BYTES,
        packed_bytes,
        CompressionKind::raw_deflate().as_str().len(),
        origin.source_profile().as_str().len(),
        origin.provenance().as_str().len(),
    ]
    .into_iter()
    .try_fold(0_usize, |total, increment| {
        checked_budget_add(
            total,
            increment,
            MAX_STORAGE_IMAGE_RETAINED_BYTES,
            "CAS entry fixed retained bytes",
        )
    })
}

fn checked_budget_add(
    current: usize,
    increment: usize,
    maximum: usize,
    resource: &'static str,
) -> std::result::Result<usize, CasGraphError> {
    let actual = current
        .checked_add(increment)
        .ok_or(CasGraphError::ResourceBudgetExceeded {
            resource,
            maximum,
            actual: usize::MAX,
        })?;
    if actual > maximum {
        return Err(CasGraphError::ResourceBudgetExceeded {
            resource,
            maximum,
            actual,
        });
    }
    Ok(actual)
}

#[derive(Debug)]
pub enum CasGraphError {
    InvalidHash(String),
    InvalidRow {
        hash: String,
        reason: String,
    },
    DuplicateRow(CasHash),
    MissingRoot(CasHash),
    HashMismatch {
        declared: CasHash,
        actual: CasHash,
    },
    InvalidRootCompression(String),
    InvalidManifest(String),
    DuplicateLogicalName(String),
    RootSelfReference {
        logical_name: String,
        hash: CasHash,
    },
    MissingContent {
        logical_name: String,
        hash: CasHash,
    },
    InvalidContentCompression {
        logical_name: String,
        hash: CasHash,
        reason: String,
    },
    ResourceBudgetExceeded {
        resource: &'static str,
        maximum: usize,
        actual: usize,
    },
    MissingPreflightRows {
        expected: usize,
        actual: usize,
    },
    InvalidPreflightOutput(String),
    Storage(StorageBuildError),
}

impl Display for CasGraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHash(value) => write!(formatter, "invalid CAS SHA-1 `{value}`"),
            Self::InvalidRow { hash, reason } => {
                write!(formatter, "invalid CAS row `{hash}`: {reason}")
            }
            Self::DuplicateRow(hash) => write!(formatter, "duplicate CAS row `{hash}`"),
            Self::MissingRoot(hash) => write!(formatter, "CAS root `{hash}` is missing"),
            Self::HashMismatch { declared, actual } => write!(
                formatter,
                "CAS row `{declared}` packed-byte SHA-1 is `{actual}`"
            ),
            Self::InvalidRootCompression(reason) => {
                write!(
                    formatter,
                    "CAS root raw-deflate payload is invalid: {reason}"
                )
            }
            Self::InvalidManifest(reason) => write!(formatter, "CAS manifest is invalid: {reason}"),
            Self::DuplicateLogicalName(name) => {
                write!(
                    formatter,
                    "CAS manifest contains duplicate logical name `{name}`"
                )
            }
            Self::RootSelfReference { logical_name, hash } => write!(
                formatter,
                "CAS root `{hash}` references itself as `{logical_name}`"
            ),
            Self::MissingContent { logical_name, hash } => {
                write!(
                    formatter,
                    "CAS content `{hash}` for `{logical_name}` is missing"
                )
            }
            Self::InvalidContentCompression {
                logical_name,
                hash,
                reason,
            } => write!(
                formatter,
                "CAS content `{hash}` for `{logical_name}` is not valid raw-deflate: {reason}"
            ),
            Self::ResourceBudgetExceeded {
                resource,
                maximum,
                actual,
            } => write!(
                formatter,
                "{resource} exceeds budget: maximum {maximum}, actual {actual}"
            ),
            Self::MissingPreflightRows { expected, actual } => write!(
                formatter,
                "CAS preflight found {actual} logical rows; expected {expected}"
            ),
            Self::InvalidPreflightOutput(reason) => {
                write!(formatter, "invalid CAS SQL preflight output: {reason}")
            }
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl Error for CasGraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StorageBuildError> for CasGraphError {
    fn from(error: StorageBuildError) -> Self {
        Self::Storage(error)
    }
}

fn validate_content_hash(row: &CasStorageRow) -> std::result::Result<(), CasGraphError> {
    let actual = CasHash::for_packed_bytes(&row.packed);
    if actual != row.hash {
        return Err(CasGraphError::HashMismatch {
            declared: row.hash,
            actual,
        });
    }
    Ok(())
}

fn inflate_bounded(input: &[u8], maximum: usize) -> std::result::Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(input).take((maximum as u64).saturating_add(1));
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|error| error.to_string())?;
    if output.len() > maximum {
        return Err(format!("inflated payload exceeds {maximum} bytes"));
    }
    Ok(output)
}

fn parse_count(value: &str) -> std::result::Result<usize, CasGraphError> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CasGraphError::InvalidManifest(
            "mapping count is not an unsigned integer".to_owned(),
        ));
    }
    value
        .parse()
        .map_err(|_| CasGraphError::InvalidManifest("mapping count does not fit usize".to_owned()))
}

fn parse_quoted_string(value: &str) -> Option<String> {
    let value = value.trim();
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut output = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            if chars.next_if_eq(&'"').is_none() {
                return None;
            }
        }
        output.push(ch);
    }
    Some(output)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::DeflateEncoder;

    use super::*;

    fn deflate(value: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(value).unwrap();
        encoder.finish().unwrap()
    }

    fn base64(hash: CasHash) -> String {
        crate::module_blob::encode_base64(hash.as_bytes())
    }

    fn row(plain: &[u8]) -> CasStorageRow {
        let packed = deflate(plain);
        CasStorageRow::new(CasHash::for_packed_bytes(&packed), packed)
    }

    fn manifest(entries: &[(&str, CasHash)]) -> CasStorageRow {
        assert!(
            !entries.is_empty(),
            "a native configinfo always maps its root UUID"
        );
        let configuration_id = "00000000-0000-0000-0000-000000000001";
        let mut mappings = if entries.iter().any(|(name, _)| *name == configuration_id) {
            Vec::new()
        } else {
            vec![(configuration_id, entries[0].1)]
        };
        mappings.extend_from_slice(entries);
        mappings.sort_by_key(|(name, _)| *name);
        let fields = mappings
            .iter()
            .map(|(name, hash)| format!(",\"{}\",{}", name.replace('"', "\"\""), base64(*hash)))
            .collect::<String>();
        row(format!(
            "{{0,{{216,0,{{80324,0}}}}}},\r\n{{2,{configuration_id},}},\r\n{{{}{fields}}}",
            mappings.len()
        )
        .as_bytes())
    }

    #[test]
    fn storage_table_boundary_names_all_four_tables() {
        assert_eq!(MssqlStorageTable::Config.sql_name(), "Config");
        assert_eq!(MssqlStorageTable::ConfigSave.sql_name(), "ConfigSave");
        assert_eq!(MssqlStorageTable::ConfigCas.sql_name(), "ConfigCAS");
        assert_eq!(MssqlStorageTable::ConfigCasSave.sql_name(), "ConfigCASSave");
        assert!(!MssqlStorageTable::Config.is_content_addressed());
        assert!(MssqlStorageTable::ConfigCas.is_content_addressed());
        assert!(!MssqlStorageTable::ConfigCasSave.is_content_addressed());
    }

    #[test]
    fn fetch_rejects_namespaced_config_cas_save_before_launching_tools() {
        let root = CasHash::parse_hex("e1a4957cd700e47cea9ad20e66d489f9e5b0bac2").unwrap();
        let error = fetch_cas_storage_image_bcp(
            Path::new("does-not-exist-sqlcmd"),
            Path::new("does-not-exist-bcp"),
            "localhost",
            None,
            None,
            "TestDb",
            MssqlStorageTable::ConfigCasSave,
            root,
        )
        .unwrap_err();
        assert!(error.to_string().contains("not a content-addressed"));
    }

    #[test]
    fn cas_hash_hex_boundary_is_exact_and_canonical() {
        let value = "E1A4957CD700E47CEA9AD20E66D489F9E5B0BAC2";
        let hash = CasHash::parse_hex(value).unwrap();
        assert_eq!(hash.to_hex(), value.to_ascii_lowercase());
        assert!(CasHash::parse_hex("e1a4957c").is_err());
        assert!(CasHash::parse_hex("z1a4957cd700e47cea9ad20e66d489f9e5b0bac2").is_err());
    }

    #[test]
    fn selected_root_isolated_from_other_extension_rows() {
        let alpha = row(b"alpha payload");
        let beta = row(b"beta payload");
        let alpha_root = manifest(&[("alpha", alpha.hash)]);
        let beta_root = manifest(&[("beta", beta.hash)]);

        let image = resolve_cas_storage_image(
            alpha_root.hash,
            vec![beta_root, beta, alpha_root.clone(), alpha.clone()],
        )
        .unwrap();

        assert_eq!(image.len(), 2);
        let alpha_entry = image
            .entries()
            .iter()
            .find(|entry| entry.logical_name().as_str() == "alpha")
            .unwrap();
        assert_eq!(alpha_entry.packed_payload(), alpha.packed);
        assert_eq!(alpha_entry.unpacked_payload(), b"alpha payload");
        assert_eq!(alpha_entry.attributes(), alpha.hash.as_bytes());
    }

    #[test]
    fn shared_content_hash_can_back_distinct_logical_files() {
        let content = row(b"same bytes");
        let root = manifest(&[("first", content.hash), ("second", content.hash)]);
        let image = resolve_cas_storage_image(root.hash, vec![root.clone(), content]).unwrap();
        assert_eq!(image.len(), 3);
        assert!(
            image
                .entries()
                .iter()
                .any(|entry| entry.logical_key().as_str() == "first")
        );
        assert!(
            image
                .entries()
                .iter()
                .any(|entry| entry.logical_key().as_str() == "second")
        );
    }

    #[test]
    fn minimal_manifest_resolves_without_unrelated_rows() {
        let configuration = row(b"configuration");
        let unrelated = row(b"not reachable");
        let root = manifest(&[("00000000-0000-0000-0000-000000000001", configuration.hash)]);
        let image =
            resolve_cas_storage_image(root.hash, vec![root.clone(), configuration, unrelated])
                .unwrap();
        assert_eq!(image.len(), 1);
    }

    #[test]
    fn missing_referenced_content_is_a_typed_error() {
        let absent = row(b"absent");
        let root = manifest(&[("missing", absent.hash)]);
        assert!(matches!(
            resolve_cas_storage_image(root.hash, vec![root.clone()]),
            Err(CasGraphError::MissingContent { hash, .. }) if hash == absent.hash
        ));
    }

    #[test]
    fn rejects_hash_mismatch_and_malformed_manifest() {
        let content = row(b"content");
        let root = manifest(&[("content", content.hash)]);
        let corrupt = CasStorageRow::new(content.hash, deflate(b"different"));
        assert!(matches!(
            resolve_cas_storage_image(root.hash, vec![root.clone(), corrupt]),
            Err(CasGraphError::HashMismatch { .. })
        ));

        let malformed = row(b"{0,{216,0,{80324,0}}},{2,x,},{2,\"only-one\"}");
        assert!(matches!(
            resolve_cas_storage_image(malformed.hash, vec![malformed.clone()]),
            Err(CasGraphError::InvalidManifest(_))
        ));
    }

    #[test]
    fn rejects_manifest_above_core_entry_limit_before_mapping_allocation() {
        let root = row(format!(
            "{{0,{{216,0,{{80324,0}}}}}},{{2,00000000-0000-0000-0000-000000000000,}},{{{}}}",
            MAX_STORAGE_ENTRIES + 1
        )
        .as_bytes());
        assert!(matches!(
            resolve_cas_storage_image(root.hash, vec![root.clone()]),
            Err(CasGraphError::InvalidManifest(reason))
                if reason.contains("maximum")
        ));
    }

    #[test]
    fn parses_preflight_stats_and_rejects_resource_overflow() {
        let stdout = "physical_rows\tlogical_rows\tpacked_bytes\n-------------\t------------\t------------\n2\t1\t4096\n";
        assert_eq!(
            parse_cas_fetch_stats(stdout).unwrap(),
            CasFetchStats {
                physical_rows: 2,
                logical_rows: 1,
                packed_bytes: 4096,
            }
        );
        assert!(matches!(
            checked_budget_add(8, 5, 10, "fixture"),
            Err(CasGraphError::ResourceBudgetExceeded {
                resource: "fixture",
                maximum: 10,
                actual: 13,
            })
        ));
    }

    #[test]
    fn preflight_sql_quotes_database_and_selected_hashes() {
        let selected = BTreeSet::from(["abc'def".to_owned()]);
        let sql = build_cas_fetch_stats_sql("db]name", MssqlStorageTable::ConfigCas, &selected);
        assert!(sql.contains("FROM [db]]name].dbo.[ConfigCAS]"));
        assert!(sql.contains("FileName IN (N'abc''def')"));
    }

    #[test]
    fn rejects_duplicate_logical_names() {
        let first = row(b"one");
        let second = row(b"two");
        let root = manifest(&[("same", first.hash), ("same", second.hash)]);
        let error =
            resolve_cas_storage_image(root.hash, vec![root.clone(), first, second]).unwrap_err();
        match error {
            CasGraphError::DuplicateLogicalName(name) => assert_eq!(name, "same"),
            CasGraphError::InvalidManifest(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }
}
