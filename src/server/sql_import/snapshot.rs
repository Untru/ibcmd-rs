//! Canonical read-only SQL acquisition and closed storage-semantics observation.
//!
//! The private CommonModule observation binds current execution identity and the
//! same extended schema seal used by capture and held CAS. It does not identify
//! a native platform build. Recording tests do not establish live SQL behavior.
use super::readset::{Builder, Check, Input, Key, Limits, Part, Readset, Table};
use crate::{
    mssql_live_continue::DatabaseIdentity,
    sql::{Dbms, SqlClient, SqlParam, SqlRow, SqlSession, SqlValue},
};
use anyhow::{Context, Result, bail, ensure};
use std::sync::Arc;

/// Operator-owned measured profile, never accepted from an import RPC argument.
pub struct Profile {
    pub identity: DatabaseIdentity,
    pub sql_version: String,
    /// SHA-256 of the batch's canonical schema JSON in UTF-16LE (SQL HASHBYTES).
    pub schema: [u8; 32],
}
const MAX_SCHEMA_BYTES: i64 = 32 * 1024;

/// One call to read_rows retains the server transaction until the full tables
/// have streamed. Any error, incomplete seal or cancellation discards all rows.
/// SqlClient must discard its connection on callback failure (the built-in TDS
/// client does); no separate query/execute calls or retries are used here.
pub fn capture(
    client: &dyn SqlClient,
    profile: &Profile,
    limits: Limits,
    check: Check,
) -> Result<Readset> {
    check()?;
    ensure!(
        client.dbms() == Dbms::SqlServer,
        "snapshot requires SQL Server"
    );
    ensure!(
        client.detached_reason().is_none(),
        "snapshot requires a connected SQL client"
    );
    Ok(
        capture_inner(profile, limits, check, false, |query, params, each| {
            client.read_rows(query, params, each)
        })?
        .readset,
    )
}

/// Closed schema/current-execution-context observation, not a native build or
/// edit capability. Only the canonical read-only observer can construct it.
pub struct CommonModuleStorageProfile {
    snapshot: Profile,
}
impl CommonModuleStorageProfile {
    pub fn snapshot_profile(&self) -> &Profile {
        &self.snapshot
    }
}

/// Observe and validate the narrow staging schema in one read-only transaction.
/// This admits neither a native build nor an edit: the caller must still capture
/// the full readset, validate its selected owner/body and retain source custody.
pub fn observe_common_module_profile(
    client: &dyn SqlClient,
    database: &str,
    limits: Limits,
    check: Check,
) -> Result<CommonModuleStorageProfile> {
    check()?;
    ensure!(
        client.dbms() == Dbms::SqlServer && client.detached_reason().is_none(),
        "profile observation requires connected SQL Server"
    );
    ensure!(
        !database.is_empty() && !database.contains('\0') && database.encode_utf16().count() <= 128,
        "invalid observation database identifier"
    );
    let count = i64::try_from(limits.max_parts)?;
    ensure!(
        count > 0 && count < i64::MAX,
        "invalid observation part limit"
    );
    let budget = i64::try_from(limits.max_metadata_bytes.min(MAX_SCHEMA_BYTES as u64))?;
    ensure!(budget > 0, "invalid observation schema limit");
    let nonce = uuid::Uuid::new_v4().to_string();
    let query = observation_query()?.replace(
        "__DATABASE__",
        &format!("[{}]", database.replace(']', "]]")),
    );
    // Positional holes are unused numeric parameters, never profile assertions.
    let params = [
        SqlParam::I64(0),
        SqlParam::I64(count),
        SqlParam::I64(0),
        SqlParam::I64(0),
        SqlParam::I64(0),
        SqlParam::I64(0),
        SqlParam::Text(&nonce),
        SqlParam::I64(budget),
    ];
    let mut observer = ProfileObserver {
        database,
        nonce: &nonce,
        limits,
        check,
        profile: None,
        sealed: false,
    };
    let mut failed = None;
    let outcome = client.read_rows(&query, &params, &mut |row| {
        if failed.is_some() {
            bail!("SQL profile stream was invalidated");
        }
        if let Err(error) = observer.accept(row) {
            failed = Some(error);
            bail!("SQL profile stream was invalidated");
        }
        Ok(())
    });
    if let Some(error) = failed {
        return Err(error);
    }
    outcome?;
    (observer.check)()?;
    ensure!(
        observer.sealed,
        "SQL profile observation has no rollback seal"
    );
    Ok(CommonModuleStorageProfile {
        snapshot: observer
            .profile
            .context("SQL profile observation has no identity")?,
    })
}

fn observation_query() -> Result<String> {
    // Reuse the canonical locked metadata producer verbatim; no parallel schema
    // serializer or weaker catalog visibility path can mint an admission.
    let sql = include_str!("snapshot.sql");
    let anchor = " IF @SchemaHash<>@P1 OR ";
    ensure!(
        sql.matches(anchor).count() == 1,
        "snapshot profile anchor changed"
    );
    let prefix = sql
        .split_once(anchor)
        .context("snapshot profile anchor missing")?
        .0;
    Ok(format!(
        "{prefix} SELECT CONVERT(nvarchar(128),SERVERPROPERTY('ServerName')),d.name,
 CONVERT(varchar(36),r.database_guid),CONVERT(varchar(36),r.family_guid),
 CONVERT(varchar(36),r.recovery_fork_guid),
 CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion')),@SchemaHash,@Schema
 FROM sys.databases d JOIN sys.database_recovery_status r ON r.database_id=d.database_id
 WHERE d.database_id=DB_ID() AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER';
 ROLLBACK TRANSACTION;
 SELECT @P7;
END TRY
BEGIN CATCH
 IF @@TRANCOUNT>0 ROLLBACK TRANSACTION;
 THROW;
END CATCH;"
    ))
}

struct ProfileObserver<'a> {
    database: &'a str,
    nonce: &'a str,
    limits: Limits,
    check: Check,
    profile: Option<Profile>,
    sealed: bool,
}
impl ProfileObserver<'_> {
    fn accept(&mut self, row: SqlRow) -> Result<()> {
        (self.check)()?;
        ensure!(!self.sealed, "SQL profile rows follow the rollback seal");
        match row.result_set {
            0 => {
                ensure!(self.profile.is_none(), "SQL profile identity repeated");
                let mut fields = row.values;
                let json = text(fields.pop().context("SQL schema JSON missing")?)?;
                let profile = decode_profile_fields(
                    fields
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("SQL profile metadata shape is invalid"))?,
                )?;
                ensure!(
                    profile.identity.database == self.database,
                    "SQL profile database changed"
                );
                ensure!(
                    profile.sql_version == "16.0.1000.6",
                    "SQL staging engine is outside the measured operation cohort"
                );
                verify_common_module_storage_schema(&json)?;
                use sha2::{Digest, Sha256};
                let bytes: Vec<_> = json.encode_utf16().flat_map(u16::to_le_bytes).collect();
                ensure!(
                    Sha256::digest(&bytes)[..] == profile.schema[..],
                    "SQL profile schema byte hash changed"
                );
                // Also validate all identity/limit arithmetic before returning a
                // profile. No synthetic headers or physical rows are created.
                Builder::new(
                    profile.identity.clone(),
                    profile.schema,
                    self.limits,
                    self.check.clone(),
                )?;
                self.profile = Some(profile);
            }
            1 => {
                ensure!(
                    self.profile.is_some() && row.values.len() == 1,
                    "SQL profile rollback seal preceded identity"
                );
                ensure!(
                    text(row.values.into_iter().next().unwrap())? == self.nonce,
                    "SQL profile rollback nonce changed"
                );
                self.sealed = true;
            }
            _ => bail!("SQL profile resultset is unsupported"),
        }
        Ok(())
    }
}

fn decode_profile_fields(fields: [SqlValue; 7]) -> Result<Profile> {
    let [server, database, guid, family, fork, version, schema] = fields;
    Ok(Profile {
        identity: DatabaseIdentity {
            server: text(server)?,
            database: text(database)?,
            database_guid: text(guid)?,
            family_guid: text(family)?,
            recovery_fork: text(fork)?,
        },
        sql_version: text(version)?,
        schema: binary(schema)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("SQL schema hash shape is invalid"))?,
    })
}

pub(super) struct HeldReadset {
    pub readset: Readset,
    pub session_id: i64,
}
struct Captured {
    readset: Readset,
    session_id: Option<i64>,
}

pub(super) fn capture_held(
    session: &mut dyn SqlSession,
    profile: &Profile,
    limits: Limits,
    check: Check,
) -> Result<HeldReadset> {
    let captured = capture_inner(profile, limits, check, true, |query, params, each| {
        session.read_rows(query, params, each)
    })?;
    Ok(HeldReadset {
        readset: captured.readset,
        session_id: captured
            .session_id
            .ok_or_else(|| anyhow::anyhow!("held SQL session seal is missing"))?,
    })
}

fn held_query() -> Result<String> {
    // Reuse the exact schema/row producer. Fail closed if a canonical control
    // anchor changes; never silently fork its serializer or rollback behavior.
    let mut sql = include_str!("snapshot.sql").to_owned();
    for (from, to, count) in [
        ("USE __DATABASE__;\n", "", 1),
        (
            "IF @@TRANCOUNT <> 0 THROW 57620, 'snapshot requires an idle session', 1;",
            "IF @@TRANCOUNT <> 1 OR XACT_STATE() <> 1 THROW 57620, 'held snapshot requires one committable transaction', 1;",
            1,
        ),
        (" BEGIN TRANSACTION;\n", "", 1),
        ("WITH (TABLOCK,HOLDLOCK)", "WITH (TABLOCKX,HOLDLOCK)", 3),
        (
            " ROLLBACK TRANSACTION;\n SELECT @P7,@NC,@NS,@BC,@BS;",
            " SELECT @P7,@NC,@NS,@BC,@BS,@@TRANCOUNT,XACT_STATE(),@@SPID;",
            1,
        ),
    ] {
        ensure!(
            sql.matches(from).count() == count,
            "canonical snapshot control anchor changed"
        );
        sql = sql.replace(from, to);
    }
    Ok(sql)
}

fn capture_inner(
    profile: &Profile,
    limits: Limits,
    check: Check,
    held: bool,
    run: impl FnOnce(&str, &[SqlParam<'_>], &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()>,
) -> Result<Captured> {
    check()?;
    let db = &profile.identity.database;
    ensure!(
        !db.is_empty() && !db.contains('\0') && db.encode_utf16().count() <= 128,
        "invalid snapshot database identifier"
    );
    ensure!(
        !profile.sql_version.is_empty() && profile.sql_version.len() <= 128,
        "snapshot SQL version is not measured"
    );
    // Validate bounds/identity before touching the client.
    let _ = Builder::new(
        profile.identity.clone(),
        profile.schema,
        limits,
        check.clone(),
    )?;
    let count = i64::try_from(limits.max_parts)?;
    ensure!(count < i64::MAX, "snapshot part limit is too large");
    let part = i64::try_from(limits.max_part_bytes)?;
    let payload = i64::try_from(limits.max_payload_bytes)?;
    // Builder reserves these exact UTF-8 identity bytes before row admission.
    // Give SQL only the remaining budget, so it cannot stream payload that
    // would fail cumulative metadata admission at the client boundary.
    let identity_bytes = [
        &profile.identity.server,
        &profile.identity.database,
        &profile.identity.database_guid,
        &profile.identity.family_guid,
        &profile.identity.recovery_fork,
    ]
    .into_iter()
    .try_fold(0u64, |sum, value| sum.checked_add(value.len() as u64))
    .ok_or_else(|| anyhow::anyhow!("SQL snapshot identity size overflow"))?;
    let metadata = i64::try_from(
        limits
            .max_metadata_bytes
            .checked_sub(identity_bytes)
            .ok_or_else(|| anyhow::anyhow!("SQL snapshot identity exceeds metadata budget"))?,
    )?;
    let name = i64::try_from(limits.max_name_units)?;
    ensure!(name <= 128, "snapshot name limit exceeds storage profile");
    let nonce = uuid::Uuid::new_v4().to_string();
    let query = if held {
        held_query()?
    } else {
        include_str!("snapshot.sql")
            .replace("__DATABASE__", &format!("[{}]", db.replace(']', "]]")))
    };
    let params = [
        SqlParam::Binary(&profile.schema),
        SqlParam::I64(count),
        SqlParam::I64(part),
        SqlParam::I64(payload),
        SqlParam::I64(metadata),
        SqlParam::I64(name),
        SqlParam::Text(&nonce),
        SqlParam::I64(MAX_SCHEMA_BYTES),
        SqlParam::Text(&profile.sql_version),
        SqlParam::Text(&profile.identity.server),
        SqlParam::Text(&profile.identity.database),
        SqlParam::Text(&profile.identity.database_guid),
        SqlParam::Text(&profile.identity.family_guid),
        SqlParam::Text(&profile.identity.recovery_fork),
    ];
    let mut decoder = Decoder {
        profile,
        limits,
        check,
        nonce: &nonce,
        builder: None,
        last_set: 0,
        counts: [0; 2],
        bytes: [0; 2],
        sealed: false,
        held,
        session_id: None,
    };
    let mut failed = None;
    let outcome = run(&query, &params, &mut |row| {
        if failed.is_some() {
            bail!("SQL snapshot stream was invalidated");
        }
        if let Err(error) = decoder.accept(row) {
            // Retain the original typed cancellation even if a client wraps or
            // accidentally ignores callback errors. Such a stream cannot seal.
            failed = Some(error);
            bail!("SQL snapshot stream was invalidated");
        }
        Ok(())
    });
    if let Some(error) = failed {
        return Err(error);
    }
    outcome?;
    (decoder.check)()?;
    ensure!(decoder.sealed, "SQL snapshot stream has no completion seal");
    let session_id = decoder.session_id;
    let readset = decoder
        .builder
        .ok_or_else(|| anyhow::anyhow!("SQL snapshot identity is missing"))?
        .finish()?;
    Ok(Captured {
        readset,
        session_id,
    })
}

struct Decoder<'a> {
    profile: &'a Profile,
    limits: Limits,
    check: Check,
    nonce: &'a str,
    builder: Option<Builder>,
    last_set: usize,
    counts: [u64; 2],
    bytes: [u64; 2],
    sealed: bool,
    held: bool,
    session_id: Option<i64>,
}
impl Decoder<'_> {
    fn accept(&mut self, row: SqlRow) -> Result<()> {
        (self.check)()?;
        ensure!(
            !self.sealed && row.result_set >= self.last_set,
            "SQL snapshot frame order is invalid"
        );
        self.last_set = row.result_set;
        match row.result_set {
            0 => {
                ensure!(self.builder.is_none(), "SQL snapshot identity repeated");
                let observed = decode_profile_fields(
                    row.values
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("SQL snapshot metadata shape is invalid"))?,
                )?;
                ensure!(
                    observed.identity == self.profile.identity,
                    "SQL snapshot target identity changed"
                );
                ensure!(
                    observed.sql_version == self.profile.sql_version,
                    "SQL snapshot version is not measured"
                );
                ensure!(
                    observed.schema == self.profile.schema,
                    "SQL snapshot schema is not measured"
                );
                let identity = observed.identity;
                self.builder = Some(Builder::new(
                    identity,
                    self.profile.schema,
                    self.limits,
                    self.check.clone(),
                )?);
            }
            1 | 2 => {
                let index = row.result_set - 1;
                let [name, part, size, creation, modified, attributes, blob]: [SqlValue; 7] = row
                    .values
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("SQL snapshot part shape is invalid"))?;
                let file_name = text(name)?;
                let part_no = u32::try_from(integer(part)?)?;
                let data_size = u64::try_from(integer(size)?)?;
                let mut header = Vec::new();
                for value in [creation, modified, attributes] {
                    let bytes = binary(value)?;
                    ensure!(
                        !bytes.is_empty() && bytes.len() <= 64,
                        "SQL snapshot header field is unsupported"
                    );
                    header.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                    header.extend_from_slice(&bytes);
                }
                let blob = binary(blob)?;
                self.counts[index] = self.counts[index]
                    .checked_add(1)
                    .ok_or_else(|| anyhow::anyhow!("SQL snapshot count overflow"))?;
                self.bytes[index] = self.bytes[index]
                    .checked_add(blob.len() as u64)
                    .ok_or_else(|| anyhow::anyhow!("SQL snapshot byte count overflow"))?;
                self.builder
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("SQL snapshot parts preceded identity"))?
                    .push(
                        if index == 0 {
                            Table::Config
                        } else {
                            Table::ConfigSave
                        },
                        Input {
                            key: Key { file_name, part_no },
                            part: Part {
                                data_size,
                                header,
                                binary: Arc::new(blob),
                            },
                        },
                    )?;
            }
            3 => {
                let mut row = row;
                if self.held {
                    ensure!(row.values.len() == 8, "held SQL seal shape is invalid");
                    let spid = integer(row.values.pop().unwrap())?;
                    let state = integer(row.values.pop().unwrap())?;
                    let count = integer(row.values.pop().unwrap())?;
                    ensure!(
                        count == 1 && state == 1 && spid > 0,
                        "held SQL transaction/session state changed"
                    );
                    self.session_id = Some(spid);
                }
                let [nonce, nc, ns, bc, bs]: [SqlValue; 5] = row
                    .values
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("SQL snapshot seal shape is invalid"))?;
                ensure!(
                    text(nonce)? == self.nonce,
                    "SQL snapshot completion nonce changed"
                );
                let counts = [u64::try_from(integer(nc)?)?, u64::try_from(integer(ns)?)?];
                let bytes = [u64::try_from(integer(bc)?)?, u64::try_from(integer(bs)?)?];
                ensure!(
                    counts == self.counts && bytes == self.bytes,
                    "SQL snapshot completion counts differ"
                );
                ensure!(self.builder.is_some(), "SQL snapshot identity is missing");
                self.sealed = true;
            }
            _ => bail!("SQL snapshot resultset is unsupported"),
        }
        Ok(())
    }
}
fn text(value: SqlValue) -> Result<String> {
    match value {
        SqlValue::Text(value) => Ok(value),
        _ => bail!("SQL snapshot expected text"),
    }
}
fn binary(value: SqlValue) -> Result<Vec<u8>> {
    match value {
        SqlValue::Binary(value) => Ok(value),
        _ => bail!("SQL snapshot expected binary"),
    }
}
fn integer(value: SqlValue) -> Result<i64> {
    match value {
        SqlValue::Int(value) => Ok(value),
        _ => bail!("SQL snapshot expected integer"),
    }
}

/// Verify the initial, narrow common-module staging storage contract.
///
/// This pure check establishes no native-platform identity, snapshot coherence,
/// or mutation authority. The acquisition/publication owner must compute the
/// safety projection from catalogs and bind this exact JSON under its locks.
pub fn verify_common_module_storage_schema(json: &str) -> Result<()> {
    ensure!(
        json.encode_utf16().take(16 * 1024 + 1).count() <= 16 * 1024,
        "SQL staging schema exceeds its budget"
    );
    let schema: StagingSchema = serde_json::from_str(json)
        .map_err(|_| anyhow::anyhow!("SQL staging schema shape is unsupported"))?;
    ensure!(
        schema.staging_safety_version == 1
            && schema.unsafe_storage_features == 0
            && schema.metadata_sysadmin
            && schema.params_marker_absent
            && schema.principal_sid_sha256.len() == 64
            && schema
                .principal_sid_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && schema.collation_name == "Cyrillic_General_CI_AS"
            && schema.compatibility_level == 160,
        "SQL staging database profile is unsupported"
    );
    ensure!(
        schema.tables.len() == 2,
        "SQL staging tables are unsupported"
    );
    for (table, name) in schema.tables.iter().zip(["Config", "ConfigSave"]) {
        ensure!(
            table.name == name
                && !table.is_memory_optimized
                && table.temporal_type == 0
                && table.checks.is_empty()
                && table.triggers.is_empty(),
            "SQL staging table features are unsupported"
        );
        let expected = [
            ("FileName", 231, 256, 0, 0),
            ("Creation", 42, 6, 19, 0),
            ("Modified", 42, 6, 19, 0),
            ("Attributes", 52, 2, 5, 0),
            ("DataSize", 127, 8, 19, 0),
            ("BinaryData", 165, -1, 0, 0),
            ("PartNo", 56, 4, 10, 0),
        ];
        ensure!(
            table.columns.len() == expected.len(),
            "SQL staging columns are unsupported"
        );
        for (index, (column, (name, ty, length, precision, scale))) in
            table.columns.iter().zip(expected).enumerate()
        {
            ensure!(
                column.column_id == index + 1
                    && column.name == name
                    && column.system_type_id == ty
                    && column.user_type_id == ty
                    && column.max_length == length
                    && column.precision == precision
                    && column.scale == scale
                    && !column.is_nullable
                    && !column.is_identity
                    && !column.is_computed
                    && column.generated_always_type == 0
                    && column.encryption_type.0.is_none()
                    && column.computed_definition.0.is_none()
                    && column.is_persisted.0.is_none()
                    && column.default_definition.0.is_none()
                    && column.collation_name.0.as_deref()
                        == (index == 0).then_some(schema.collation_name.as_str()),
                "SQL staging column layout is unsupported"
            );
        }
        ensure!(
            table.indexes.len() == 1,
            "SQL staging indexes are unsupported"
        );
        let index = &table.indexes[0];
        ensure!(
            !index.name.is_empty()
                && !index.name.contains('\0')
                && index.name.encode_utf16().count() <= 128
                && index.index_type == 1
                && index.is_unique
                && !index.is_unique_constraint
                && !index.is_disabled
                && !index.has_filter
                && index.filter_definition.0.is_none()
                && index.columns.len() == 2,
            "SQL staging index features are unsupported"
        );
        // Both primary-key and ordinary unique clustered indexes are admitted.
        // The opaque index name and primary-key designation grant no authority.
        let _ = index.is_primary_key;
        for (position, (column, id)) in index.columns.iter().zip([1, 7]).enumerate() {
            ensure!(
                column.index_column_id == position + 1
                    && column.column_id == id
                    && column.key_ordinal == position + 1
                    && !column.is_descending_key
                    && !column.is_included_column,
                "SQL staging index key layout is unsupported"
            );
        }
    }
    Ok(())
}

// Using a non-Option field whose deserializer starts with deserialize_any makes
// a missing field an error, while preserving the explicit SQL JSON null value.
// Nullable fields here are scalars; no nested object bypasses typed field checks.
struct RequiredNullable<T>(Option<T>);
impl<'de, T: serde::de::DeserializeOwned> serde::Deserialize<'de> for RequiredNullable<T> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
        serde_json::from_value(value)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingSchema {
    staging_safety_version: u32,
    unsafe_storage_features: u64,
    metadata_sysadmin: bool,
    params_marker_absent: bool,
    principal_sid_sha256: String,
    collation_name: String,
    compatibility_level: u16,
    // Both database defaults are supported: every storage read is protected by
    // retained HOLDLOCK table locks (TABLOCKX during publication). Keep this
    // required typed field in the exact canonical JSON/schema hash nevertheless.
    #[serde(rename = "is_read_committed_snapshot_on")]
    _is_read_committed_snapshot_on: bool,
    tables: Vec<StagingTable>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingTable {
    name: String,
    is_memory_optimized: bool,
    temporal_type: u8,
    columns: Vec<StagingColumn>,
    indexes: Vec<StagingIndex>,
    checks: Vec<serde::de::IgnoredAny>,
    triggers: Vec<serde::de::IgnoredAny>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingColumn {
    column_id: usize,
    name: String,
    system_type_id: u16,
    user_type_id: u16,
    max_length: i16,
    precision: u8,
    scale: u8,
    collation_name: RequiredNullable<String>,
    is_nullable: bool,
    is_identity: bool,
    is_computed: bool,
    generated_always_type: u8,
    encryption_type: RequiredNullable<u8>,
    computed_definition: RequiredNullable<String>,
    is_persisted: RequiredNullable<bool>,
    default_definition: RequiredNullable<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingIndex {
    name: String,
    #[serde(rename = "type")]
    index_type: u8,
    is_unique: bool,
    is_primary_key: bool,
    is_unique_constraint: bool,
    is_disabled: bool,
    has_filter: bool,
    filter_definition: RequiredNullable<String>,
    columns: Vec<StagingIndexColumn>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingIndexColumn {
    index_column_id: usize,
    column_id: usize,
    key_ordinal: usize,
    is_descending_key: bool,
    is_included_column: bool,
}

#[cfg(test)]
mod staging_schema_contracts {
    use super::verify_common_module_storage_schema;
    use serde_json::{Value, json};

    // Synthetic decoder contract, never a native schema observation or profile.
    fn fixture() -> Value {
        let columns: Vec<_> = [
            ("FileName", 231, 256, 0),
            ("Creation", 42, 6, 19),
            ("Modified", 42, 6, 19),
            ("Attributes", 52, 2, 5),
            ("DataSize", 127, 8, 19),
            ("BinaryData", 165, -1, 0),
            ("PartNo", 56, 4, 10),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, ty, length, precision))| {
            json!({
                "column_id": i + 1, "name": name, "system_type_id": ty,
                "user_type_id": ty, "max_length": length, "precision": precision,
                "scale": 0, "collation_name": (i == 0).then_some("Cyrillic_General_CI_AS"),
                "is_nullable": false, "is_identity": false, "is_computed": false,
                "generated_always_type": 0, "encryption_type": null,
                "computed_definition": null, "is_persisted": null, "default_definition": null
            })
        })
        .collect();
        let tables: Vec<_> = ["Config", "ConfigSave"]
            .into_iter()
            .map(|name| {
                json!({
                    "name": name, "is_memory_optimized": false, "temporal_type": 0,
                    "columns": columns, "checks": [], "triggers": [],
                    "indexes": [{"name": "synthetic-index", "type": 1, "is_unique": true,
                        "is_primary_key": false, "is_unique_constraint": false,
                        "is_disabled": false, "has_filter": false, "filter_definition": null,
                        "columns": [
                            {"index_column_id": 1, "column_id": 1, "key_ordinal": 1,
                                "is_descending_key": false, "is_included_column": false},
                            {"index_column_id": 2, "column_id": 7, "key_ordinal": 2,
                                "is_descending_key": false, "is_included_column": false}
                        ]}]
                })
            })
            .collect();
        json!({"staging_safety_version": 1, "unsafe_storage_features": 0,
            "metadata_sysadmin": true, "params_marker_absent": true,
            "principal_sid_sha256": "11".repeat(32),
            "collation_name": "Cyrillic_General_CI_AS", "compatibility_level": 160,
            "is_read_committed_snapshot_on": false, "tables": tables})
    }
    fn refused(change: impl FnOnce(&mut Value)) {
        let mut value = fixture();
        change(&mut value);
        assert!(verify_common_module_storage_schema(&value.to_string()).is_err());
    }
    #[test]
    fn synthetic_schema_accepts_only_the_two_table_contract() {
        assert!(verify_common_module_storage_schema(&fixture().to_string()).is_ok());
        let mut primary = fixture();
        primary["tables"][0]["indexes"][0]["is_primary_key"] = json!(true);
        assert!(verify_common_module_storage_schema(&primary.to_string()).is_ok());
        refused(|v| v["tables"].as_array_mut().unwrap().swap(0, 1));
        refused(|v| v["metadata_sysadmin"] = json!(false));
        refused(|v| v["params_marker_absent"] = json!(false));
        refused(|v| v["metadata_sysadmin"] = Value::Null);
        refused(|v| v["principal_sid_sha256"] = Value::Null);
        refused(|v| v["principal_sid_sha256"] = json!("x".repeat(64)));
        refused(|v| v["principal_sid_sha256"] = json!("a".repeat(63)));
    }
    #[test]
    fn rcsi_modes_are_admitted_but_remain_typed_and_byte_hash_bound() {
        for enabled in [false, true] {
            let mut value = fixture();
            value["is_read_committed_snapshot_on"] = json!(enabled);
            assert!(verify_common_module_storage_schema(&value.to_string()).is_ok());
        }
        for invalid in [Value::Null, json!(0), json!("true")] {
            refused(|v| v["is_read_committed_snapshot_on"] = invalid);
        }
        refused(|v| {
            v.as_object_mut()
                .unwrap()
                .remove("is_read_committed_snapshot_on");
        });
        // The setting is still part of the exact observed UTF-16 JSON seal.
        // Changing it without changing that seal cannot mint a profile.
        let mut row = observation_metadata();
        let crate::sql::SqlValue::Text(json) = &mut row.values[7] else {
            panic!("metadata JSON is text");
        };
        let changed = json.replace(
            "\"is_read_committed_snapshot_on\":false",
            "\"is_read_committed_snapshot_on\":true",
        );
        assert_ne!(*json, changed);
        *json = changed;
        let mut observer = observer();
        assert!(observer.accept(row).is_err());
        assert!(observer.profile.is_none() && !observer.sealed);
    }

    #[test]
    fn duplicate_unknown_and_missing_required_nullable_fields_refuse() {
        let value = fixture().to_string();
        let duplicate = value.replacen(
            "\"encryption_type\":null",
            "\"encryption_type\":null,\"encryption_type\":null",
            1,
        );
        assert_ne!(duplicate, value);
        assert!(verify_common_module_storage_schema(&duplicate).is_err());
        refused(|v| {
            v["tables"][0]["columns"][0]
                .as_object_mut()
                .unwrap()
                .remove("encryption_type");
        });
        refused(|v| v["unexpected"] = json!(0));
        refused(|v| v["tables"][0]["columns"][0]["unexpected"] = json!(0));
    }
    #[test]
    fn altered_columns_indexes_and_features_refuse() {
        refused(|v| v["tables"][0]["columns"][1]["system_type_id"] = json!(61));
        refused(|v| v["tables"][0]["columns"][0]["user_type_id"] = json!(999));
        refused(|v| v["tables"][0]["columns"][0]["default_definition"] = json!("(0)"));
        refused(|v| {
            v["tables"][0]["columns"][1]["collation_name"] = json!("Cyrillic_General_CI_AS")
        });
        refused(|v| v["tables"][0]["indexes"][0]["columns"][1]["column_id"] = json!(6));
        refused(|v| v["tables"][0]["indexes"][0]["has_filter"] = json!(true));
        refused(|v| v["tables"][0]["indexes"][0]["is_unique"] = json!(false));
        refused(|v| v["tables"][0]["checks"] = json!([{}]));
        refused(|v| v["tables"][0]["triggers"] = json!(null));
        refused(|v| v["unsafe_storage_features"] = json!(1));
        refused(|v| v["staging_safety_version"] = json!(2));
    }
    #[test]
    fn schema_budget_counts_utf16le_bytes_before_parsing() {
        let value = fixture().to_string();
        let within = format!(
            "{value}{}",
            " ".repeat(16 * 1024 - value.encode_utf16().count())
        );
        assert!(verify_common_module_storage_schema(&within).is_ok());
        assert!(verify_common_module_storage_schema(&(within + " ")).is_err());
        assert!(verify_common_module_storage_schema(&"😀".repeat(8193)).is_err());
    }

    fn observer() -> super::ProfileObserver<'static> {
        super::ProfileObserver {
            database: "public_synthetic",
            nonce: "sealed-observation",
            limits: super::Limits::default(),
            check: std::sync::Arc::new(|| Ok(())),
            profile: None,
            sealed: false,
        }
    }
    fn observation_metadata() -> crate::sql::SqlRow {
        use crate::sql::SqlValue;
        use sha2::{Digest, Sha256};
        let json = fixture().to_string();
        let bytes: Vec<_> = json.encode_utf16().flat_map(u16::to_le_bytes).collect();
        crate::sql::SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text("recording".into()),
                SqlValue::Text("public_synthetic".into()),
                SqlValue::Text(uuid::Uuid::from_u128(1).to_string()),
                SqlValue::Text(uuid::Uuid::from_u128(2).to_string()),
                SqlValue::Text(uuid::Uuid::from_u128(3).to_string()),
                SqlValue::Text("16.0.1000.6".into()),
                SqlValue::Binary(Sha256::digest(&bytes).to_vec()),
                SqlValue::Text(json),
            ],
        }
    }
    fn observation_seal(nonce: &str) -> crate::sql::SqlRow {
        crate::sql::SqlRow {
            result_set: 1,
            values: vec![crate::sql::SqlValue::Text(nonce.into())],
        }
    }
    #[test]
    fn observation_requires_metadata_byte_hash_then_exact_rollback_seal() {
        let mut o = observer();
        o.accept(observation_metadata()).unwrap();
        assert!(!o.sealed);
        o.accept(observation_seal("sealed-observation")).unwrap();
        assert!(o.sealed);
        assert_eq!(
            o.profile.as_ref().unwrap().identity.database,
            "public_synthetic"
        );
        assert!(o.accept(observation_seal("sealed-observation")).is_err());
        for field in [1, 5, 6] {
            let mut row = observation_metadata();
            row.values[field] = match field {
                6 => crate::sql::SqlValue::Binary(vec![0; 32]),
                _ => crate::sql::SqlValue::Text("wrong".into()),
            };
            let mut o = observer();
            assert!(o.accept(row).is_err());
            assert!(o.profile.is_none() && !o.sealed);
        }
        let mut o = observer();
        assert!(o.accept(observation_seal("sealed-observation")).is_err());
        let mut o = observer();
        o.accept(observation_metadata()).unwrap();
        assert!(o.accept(observation_metadata()).is_err());
        assert!(o.accept(observation_seal("wrong")).is_err());
    }
    #[test]
    fn observation_preserves_typed_cancellation_before_returning_a_profile() {
        let mut o = observer();
        o.check = std::sync::Arc::new(|| Err(crate::cancel::Cancelled.into()));
        let error = o.accept(observation_metadata()).unwrap_err();
        assert!(error.downcast_ref::<crate::cancel::Cancelled>().is_some());
        assert!(o.profile.is_none() && !o.sealed);
    }
    #[test]
    fn profile_version_predicate_is_byte_exact_across_database_collations() {
        // A parameter and SERVERPROPERTY may carry different database/server
        // collations. The measured version is an exact UTF-16 identity, just
        // like the adjacent target identifiers, rather than linguistic text.
        let predicate = "CONVERT(varbinary(max),CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion')))<>CONVERT(varbinary(max),@P9)";
        for query in [
            include_str!("snapshot.sql").to_owned(),
            super::held_query().unwrap(),
        ] {
            assert!(
                query.contains(predicate),
                "version guard must compare binary values"
            );
            assert!(
                !query.contains("CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))<>@P9")
            );
            assert!(query.contains("IF @SchemaHash<>@P1 OR "));
            assert!(query.contains("THROW 57623, 'snapshot profile is not measured', 1;"));
        }
        // Observation precedes the measured-profile guard and must not acquire
        // a caller-provided version expectation of its own.
        assert!(!super::observation_query().unwrap().contains("@P9"));
    }

    #[test]
    fn observation_and_held_capture_share_locked_context_and_safety_producer() {
        let query = super::observation_query().unwrap();
        let canonical = include_str!("snapshot.sql");
        let prefix = canonical.split_once(" IF @SchemaHash<>@P1 OR ").unwrap().0;
        assert!(query.starts_with(prefix));
        let role = query.find("IS_SRVROLEMEMBER(N'sysadmin')").unwrap();
        let first = query
            .find("FROM dbo.Config WITH (TABLOCK,HOLDLOCK)")
            .unwrap();
        let second = query
            .find("FROM dbo.ConfigSave WITH (TABLOCK,HOLDLOCK)")
            .unwrap();
        let params = query
            .find("FROM dbo.Params WITH (TABLOCK,HOLDLOCK)")
            .unwrap();
        let ids = query.find("DECLARE @ConfigId").unwrap();
        let safety = query.find("DECLARE @UnsafeStorageFeatures").unwrap();
        assert!(role < first && first < second && second < params && params < ids && ids < safety);
        assert!(query.contains("OR referenced_object_id IN (@ConfigId,@SaveId,@ParamsId)"));
        assert!(query.contains("FROM sys.security_predicates"));
        assert!(query.contains("FROM sys.triggers\n     WHERE parent_id"));
        assert!(query.contains("@ExecutionSidHash,2) AS principal_sid_sha256"));
        assert!(!query.contains("SELECT FileName,PartNo,DataSize"));
        assert!(!query.contains("INSERT INTO") && !query.contains("DELETE "));
        let rollback = query.find(" ROLLBACK TRANSACTION;").unwrap();
        let seal = query.find(" SELECT @P7;").unwrap();
        assert!(rollback < seal);
        let held = super::held_query().unwrap();
        // RCSI affects default READ COMMITTED reads, not these explicit
        // serializable locks. Later payload/catalog reads stay under them.
        assert_eq!(query.matches("WITH (TABLOCK,HOLDLOCK)").count(), 3);
        assert_eq!(held.matches("WITH (TABLOCKX,HOLDLOCK)").count(), 3);
        assert!(!held.contains("WITH (TABLOCK,HOLDLOCK)"));
        assert!(!held.contains("NOLOCK") && !held.contains("READPAST"));
        assert!(held.contains("IF @SchemaHash<>@P1 OR "));
        for marker in [
            "staging_safety_version",
            "unsafe_storage_features",
            "metadata_sysadmin",
            "principal_sid_sha256",
            "params_marker_absent",
            "d.is_read_committed_snapshot_on",
        ] {
            assert!(query.contains(marker) && held.contains(marker));
        }
    }

    #[derive(Clone, Copy)]
    enum ObservationFault {
        None,
        MissingSeal,
        TransportAfterSeal,
        IgnoreCallback,
    }
    struct ObservationClient {
        fault: ObservationFault,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl super::SqlClient for ObservationClient {
        fn dbms(&self) -> super::Dbms {
            super::Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: crate::sql::ScriptVariables) -> anyhow::Result<()> {
            panic!("profile observation must not use scripts")
        }
        fn execute(&self, _: &str, _: &[super::SqlParam<'_>]) -> anyhow::Result<u64> {
            panic!("profile observation must not execute DML")
        }
        fn read_rows(
            &self,
            query: &str,
            params: &[super::SqlParam<'_>],
            each: &mut dyn FnMut(crate::sql::SqlRow) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(params.len(), 8);
            assert!(!query.contains("SELECT FileName,PartNo,DataSize"));
            let super::SqlParam::Text(nonce) = params[6] else {
                panic!("nonce parameter missing")
            };
            if matches!(self.fault, ObservationFault::IgnoreCallback) {
                let _ = each(observation_metadata());
                let _ = each(observation_seal(nonce));
                return Ok(());
            }
            each(observation_metadata())?;
            if matches!(self.fault, ObservationFault::MissingSeal) {
                return Ok(());
            }
            each(observation_seal(nonce))?;
            if matches!(self.fault, ObservationFault::TransportAfterSeal) {
                anyhow::bail!("recorded transport failed after rollback seal");
            }
            Ok(())
        }
        fn query_json(&self, _: &str) -> anyhow::Result<Option<String>> {
            panic!("profile observation must not use alternate queries")
        }
        fn write_rows(
            &self,
            _: &str,
            _: &[&str],
            _: &[Vec<super::SqlParam<'_>>],
        ) -> anyhow::Result<u64> {
            panic!("profile observation must not write rows")
        }
    }
    #[test]
    fn observed_profile_returns_only_after_complete_transport_and_rollback_seal() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        for (fault, succeeds) in [
            (ObservationFault::None, true),
            (ObservationFault::MissingSeal, false),
            (ObservationFault::TransportAfterSeal, false),
        ] {
            let client = ObservationClient {
                fault,
                calls: AtomicUsize::new(0),
            };
            let outcome = super::observe_common_module_profile(
                &client,
                "public_synthetic",
                super::Limits::default(),
                std::sync::Arc::new(|| Ok(())),
            );
            assert_eq!(outcome.is_ok(), succeeds);
            assert_eq!(client.calls.load(Ordering::SeqCst), 1);
        }
        let client = ObservationClient {
            fault: ObservationFault::IgnoreCallback,
            calls: AtomicUsize::new(0),
        };
        let checks = std::sync::Arc::new(AtomicUsize::new(0));
        let check: super::Check = std::sync::Arc::new(move || {
            if checks.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err(crate::cancel::Cancelled.into())
            }
        });
        let error = super::observe_common_module_profile(
            &client,
            "public_synthetic",
            super::Limits::default(),
            check,
        )
        .err()
        .expect("swallowed callback accepted");
        assert!(error.downcast_ref::<crate::cancel::Cancelled>().is_some());
        assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    }
}
