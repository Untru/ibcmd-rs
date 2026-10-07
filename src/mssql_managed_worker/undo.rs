//! Cold, exact-image undo of a recorded main-script LIVE publication.
//!
//! This internal API is not an artifact replay command. The borrowed ColdSession
//! must still own the original exited kernel handles, lease and exclusive journal.
//! It does not admit online/config-apply parity, Files/registration rewrites or
//! unmeasured native layouts. Five complete seven-column tables are compared;
//! only the Config/Params delta is restored and consumed ConfigSave stays empty.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{ColdSession, LifetimeBinding, OwnedRuntime};
use crate::mssql_live_continue::{DatabaseIdentity, LIVE_SQL_VERSION, LiveArtifact};
use crate::mssql_main_activation::{MainStorageRow, generation_from_versions, quote_string};
use crate::mssql_platform_profile::{MssqlNativePlatformProfile, exclusive_session_gate};
use crate::sql::{SqlClient, SqlExec, SqlRow};

const TABLES: [&str; 5] = [
    "Config",
    "ConfigSave",
    "Params",
    "ConfigCAS",
    "ConfigCASSave",
];
const MAX_ROWS: usize = 50_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 16 * 1024 * 1024;
const MAX_SQL: usize = 32 * 1024 * 1024;
type Rows = BTreeMap<(String, i32), MainStorageRow>;

// Projection and rendering share the exact same statements. The projection
// reserves hex length arithmetically, before any binary encoding/allocation.
trait SqlOutput: std::fmt::Write {
    fn binary_hex(&mut self, bytes: &[u8]) -> std::fmt::Result;
    fn push_str(&mut self, text: &str) -> Result<()> {
        self.write_str(text).context("undo SQL exceeds bound")
    }
}
#[derive(Default)]
struct SqlSize {
    bytes: usize,
}
fn bounded_sql_add(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|size| *size <= MAX_SQL)
}
impl std::fmt::Write for SqlSize {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.bytes = bounded_sql_add(self.bytes, text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
impl SqlOutput for SqlSize {
    fn binary_hex(&mut self, bytes: &[u8]) -> std::fmt::Result {
        self.add_hex_length(bytes.len())
    }
}
impl SqlSize {
    fn add_hex_length(&mut self, bytes: usize) -> std::fmt::Result {
        let hex_len = bytes.checked_mul(2).ok_or(std::fmt::Error)?;
        self.bytes = bounded_sql_add(self.bytes, hex_len).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
struct SqlText {
    text: String,
    projected: usize,
}
impl std::fmt::Write for SqlText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let size = bounded_sql_add(self.text.len(), text.len()).ok_or(std::fmt::Error)?;
        if size > self.projected {
            return Err(std::fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}
impl SqlOutput for SqlText {
    fn binary_hex(&mut self, bytes: &[u8]) -> std::fmt::Result {
        let hex_len = bytes.len().checked_mul(2).ok_or(std::fmt::Error)?;
        let size = bounded_sql_add(self.text.len(), hex_len).ok_or(std::fmt::Error)?;
        if size > self.projected {
            return Err(std::fmt::Error);
        }
        // The whole exact render size has already been reserved. Stream hex
        // directly into it, without a second full-size encoded payload.
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        for byte in bytes {
            self.text.push(char::from(DIGITS[(byte >> 4) as usize]));
            self.text.push(char::from(DIGITS[(byte & 15) as usize]));
        }
        Ok(())
    }
}

fn session_visibility_guard() -> &'static str {
    "IF ISNULL(HAS_PERMS_BY_NAME(NULL,NULL,N'VIEW SERVER PERFORMANCE STATE'),0)<>1 THROW 57573,'undo full SQL session visibility required',1;\n"
}

fn reserve_capture_budget(rows: usize, bytes: usize, incoming: usize) -> Result<(usize, usize)> {
    ensure!(incoming <= MAX_ROW_BYTES, "undo capture row exceeds bounds");
    let next_rows = rows.checked_add(1).context("undo capture row overflow")?;
    let next_bytes = bytes
        .checked_add(incoming)
        .context("undo capture byte overflow")?;
    ensure!(
        next_rows <= MAX_ROWS && next_bytes <= MAX_BYTES,
        "undo capture exceeds total bounds"
    );
    Ok((next_rows, next_bytes))
}
fn validate_row_size(bytes: usize, data_size: u64) -> Result<()> {
    ensure!(bytes <= MAX_ROW_BYTES, "undo capture row exceeds bounds");
    ensure!(
        data_size == bytes as u64,
        "unsupported undo physical row size"
    );
    Ok(())
}

/// No Deserialize or public row mutators: this snapshot comes from the built-in
/// SQL adapter, not from a user's file or a connection count.
#[derive(Clone, Debug)]
pub(crate) struct CapturedImage {
    identity: DatabaseIdentity,
    schema: String,
    ib_version: i32,
    platform_version_req: i32,
    tables: [Rows; 5],
}

impl CapturedImage {
    pub(crate) fn capture(sql: &SqlExec, identity: &DatabaseIdentity) -> Result<Self> {
        let client = sql
            .client()
            .context("guarded undo requires the built-in SQL backend")?;
        validate_identity(identity)?;
        let verified = crate::mssql_platform_profile::verify_mssql_storage_profile(
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            client,
            &identity.database,
        )?;
        let mut result = Self {
            identity: identity.clone(),
            schema: verified.storage_schema_sha256,
            ib_version: verified.ib_version,
            platform_version_req: verified.platform_version_req,
            tables: std::array::from_fn(|_| BTreeMap::new()),
        };
        let mut sql = lock_prefix(identity);
        sql.push_str(
            "BEGIN TRY\nSET TRANSACTION ISOLATION LEVEL SERIALIZABLE; BEGIN TRANSACTION;\n",
        );
        sql.push_str(&result.identity_guard()?);
        sql.push_str(
            "DECLARE @UndoCaptureRows decimal(38,0)=0,@UndoCaptureBytes decimal(38,0)=0;\n",
        );
        for table in TABLES {
            writeln!(sql, "DECLARE @Capture{table} bigint; SELECT @Capture{table}=COUNT_BIG(*) FROM dbo.[{table}] WITH(TABLOCKX,HOLDLOCK);").unwrap();
        }
        sql.push_str(&result.schema_guard());
        for table in TABLES {
            // Both aggregate and streaming budgets apply; neither observes an
            // unrelated query's headers and then adopts later binary bytes.
            writeln!(sql, "IF EXISTS(SELECT 1 FROM dbo.[{table}] WITH(UPDLOCK,HOLDLOCK) WHERE DATALENGTH(BinaryData)>{MAX_ROW_BYTES} OR DataSize<>DATALENGTH(BinaryData)) THROW 57560,'undo capture row exceeds bounds',1; SELECT @UndoCaptureRows=@UndoCaptureRows+COUNT_BIG(*),@UndoCaptureBytes=@UndoCaptureBytes+COALESCE(SUM(CONVERT(decimal(38,0),DATALENGTH(BinaryData))),0) FROM dbo.[{table}] WITH(UPDLOCK,HOLDLOCK);\n").unwrap();
        }
        writeln!(sql,"IF @UndoCaptureRows>{MAX_ROWS} OR @UndoCaptureBytes>{MAX_BYTES} THROW 57560,'undo capture exceeds total bounds',1;").unwrap();
        for (index, table) in TABLES.iter().enumerate() {
            writeln!(sql, "SELECT {index},FileName,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),CONVERT(bigint,DataSize),BinaryData FROM dbo.[{table}] WITH (UPDLOCK,HOLDLOCK) ORDER BY FileName,PartNo;").unwrap();
        }
        sql.push_str("COMMIT TRANSACTION; USE [master]; EXEC sys.sp_releaseapplock @Resource=@UndoResource,@LockOwner='Session'; END TRY BEGIN CATCH IF @@TRANCOUNT>0 ROLLBACK; USE [master]; EXEC sys.sp_releaseapplock @Resource=@UndoResource,@LockOwner='Session'; THROW; END CATCH;\n");
        let mut bytes = 0usize;
        let mut rows = 0usize;
        client.read_rows(&sql, &[], &mut |row| {
            let index = usize::try_from(row.i64(0)?)?;
            ensure!(
                index < TABLES.len() && row.values.len() == 8,
                "unexpected undo capture result"
            );
            (rows, bytes) = reserve_capture_budget(rows, bytes, row.binary(7)?.len())?;
            let value = parse_row(row)?;
            let key = (value.file_name.clone(), value.part_no);
            ensure!(
                result.tables[index].insert(key, value).is_none(),
                "duplicate undo capture key"
            );
            Ok(())
        })?;
        Ok(result)
    }

    fn identity_guard(&self) -> Result<String> {
        validate_identity(&self.identity)?;
        Ok(format!(
            "IF NOT EXISTS (SELECT 1 FROM master.sys.databases d JOIN master.sys.database_recovery_status r ON r.database_id=d.database_id WHERE d.name=N'{}' AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER' AND CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))=N'{LIVE_SQL_VERSION}' AND CONVERT(nvarchar(128),SERVERPROPERTY('ServerName'))=N'{}' AND r.database_guid='{}' AND r.family_guid='{}' AND r.recovery_fork_guid='{}') THROW 57561,'undo database identity/state changed',1;\nUSE {};\n",
            quote_string(&self.identity.database),
            quote_string(&self.identity.server),
            self.identity.database_guid,
            self.identity.family_guid,
            self.identity.recovery_fork,
            ident(&self.identity.database)
        ))
    }

    fn schema_guard(&self) -> String {
        // The same five-table contract as verify_mssql_storage_profile, checked
        // again on the actual locked transaction/session before any target write.
        let columns = [
            ("FileName", "nvarchar", 256, 0, 0),
            ("Creation", "datetime2", 6, 19, 0),
            ("Modified", "datetime2", 6, 19, 0),
            ("Attributes", "smallint", 2, 5, 0),
            ("DataSize", "bigint", 8, 19, 0),
            ("BinaryData", "varbinary", -1, 0, 0),
            ("PartNo", "int", 4, 10, 0),
        ];
        let mut sql = format!(
            "IF (SELECT COUNT_BIG(*) FROM dbo.IBVersion WITH (UPDLOCK,HOLDLOCK))<>1 OR NOT EXISTS (SELECT 1 FROM dbo.IBVersion WITH (UPDLOCK,HOLDLOCK) WHERE IBVersion={} AND PlatformVersionReq={}) THROW 57562,'undo platform identity changed',1;\nDECLARE @UndoSchema TABLE(T nvarchar(128),I int,N nvarchar(128),Y nvarchar(128),L int,P int,S int,Z int);\n",
            self.ib_version, self.platform_version_req
        );
        for table in TABLES {
            for (i, (name, ty, len, p, s)) in columns.iter().enumerate() {
                writeln!(
                    sql,
                    "INSERT @UndoSchema VALUES(N'{table}',{},N'{name}',N'{ty}',{len},{p},{s},0);",
                    i + 1
                )
                .unwrap();
            }
        }
        let actual = "SELECT t.name,c.column_id,c.name,TYPE_NAME(c.user_type_id),c.max_length,c.precision,c.scale,CONVERT(int,c.is_nullable) FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id JOIN sys.columns c ON c.object_id=t.object_id WHERE s.name=N'dbo' AND t.name IN(N'Config',N'ConfigSave',N'Params',N'ConfigCAS',N'ConfigCASSave')";
        writeln!(sql,"IF EXISTS ({actual} EXCEPT SELECT * FROM @UndoSchema) OR EXISTS(SELECT * FROM @UndoSchema EXCEPT {actual}) OR EXISTS(SELECT 1 FROM sys.triggers x JOIN sys.tables t ON t.object_id=x.parent_id WHERE t.name IN(N'Config',N'ConfigSave',N'Params',N'ConfigCAS',N'ConfigCASSave') AND x.is_disabled=0) THROW 57562,'undo schema/trigger boundary is unsupported',1;").unwrap();
        sql
    }

    fn fingerprint(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(serde_json::to_vec(&self.identity).expect("identity serialization"));
        digest.update(self.schema.as_bytes());
        for (table, rows) in TABLES.iter().zip(&self.tables) {
            digest.update(table.as_bytes());
            digest.update((rows.len() as u64).to_be_bytes());
            for row in rows.values() {
                digest.update(
                    serde_json::to_vec(&(
                        &row.file_name,
                        row.part_no,
                        &row.creation,
                        &row.modified,
                        row.attributes,
                        row.data_size,
                        row.binary_data.len(),
                    ))
                    .expect("row header serialization"),
                );
                digest.update(row.sha256());
            }
        }
        digest.finalize().into()
    }
}

#[derive(Debug)]
pub(crate) struct GuardedUndoPlan {
    artifact: LiveArtifact,
    before: CapturedImage,
    after: CapturedImage,
    desired: CapturedImage,
    witness: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UndoResult {
    Committed,
    AlreadyUndone,
}

impl GuardedUndoPlan {
    pub(crate) fn from_live(
        artifact: &LiveArtifact,
        before: CapturedImage,
        after: CapturedImage,
    ) -> Result<Self> {
        artifact.validate()?;
        ensure!(
            before.identity == artifact.identity
                && after.identity == artifact.identity
                && before
                    .schema
                    .eq_ignore_ascii_case(&artifact.storage_schema_sha256)
                && after.schema == before.schema
                && before.ib_version == after.ib_version
                && before.platform_version_req == after.platform_version_req,
            "undo identity/profile/schema capture differs from publication"
        );
        ensure!(
            artifact.recovery.retained_config_rows.is_empty(),
            "online retained rows are not a main-script LIVE undo"
        );
        let stage: Rows = artifact
            .recovery
            .staged_rows
            .iter()
            .map(|r| ((r.file_name.clone(), r.part_no), r.clone()))
            .collect();
        ensure!(
            stage.len() == artifact.recovery.staged_rows.len() && before.tables[1] == stage,
            "consumed stage is not exactly the recorded operation"
        );
        for (table, rows) in TABLES.iter().zip(&before.tables) {
            ensure!(
                rows.keys()
                    .all(|(name, _)| !name.to_ascii_lowercase().contains("_dynupdate_")),
                "{table} contains unsupported alias history"
            );
        }
        for row in &artifact.recovery.overwritten_config_rows {
            ensure!(
                before.tables[0].get(&(row.file_name.clone(), row.part_no)) == Some(row),
                "undo overwritten preimage differs from captured bytes/headers"
            );
        }
        let overwritten: std::collections::BTreeSet<_> = artifact
            .recovery
            .overwritten_config_rows
            .iter()
            .map(|r| (&r.file_name, r.part_no))
            .collect();
        ensure!(
            overwritten.len() == stage.len()
                && stage
                    .keys()
                    .all(|(name, part)| overwritten.contains(&(name, *part))),
            "undo replacement coverage differs from publication"
        );
        for (index, marker) in [
            (0, &artifact.recovery.prior_config_dynamically_updated),
            (2, &artifact.recovery.prior_params_dynamically_updated),
        ] {
            ensure!(
                before.tables[index].get(&("DynamicallyUpdated".into(), 0)) == marker.as_ref(),
                "undo marker preimage differs from publication"
            );
        }
        let generation = |image: &CapturedImage| -> Result<Uuid> {
            let row = image.tables[0]
                .get(&("versions".into(), 0))
                .context("undo versions preimage missing")?;
            Ok(generation_from_versions(&row.binary_data)?)
        };
        ensure!(
            generation(&before)? == Uuid::parse_str(&artifact.recovery.old_generation)?
                && generation(&after)? == Uuid::parse_str(&artifact.recovery.new_generation)?,
            "undo captured generations differ from operation"
        );
        let mut expected = before.clone();
        for (key, row) in &stage {
            expected.tables[0].insert(key.clone(), row.clone());
        }
        expected.tables[0].remove(&("DynamicallyUpdated".into(), 0));
        expected.tables[2].remove(&("DynamicallyUpdated".into(), 0));
        expected.tables[1].clear();
        ensure!(
            after.tables == expected.tables,
            "publication includes unexplained rows/headers/markers/ancillary changes; undo refused"
        );
        let mut desired = before.clone();
        desired.tables[1].clear(); // Undo never resurrects a consumed stage.
        let witness = hex(&Sha256::digest(serde_json::to_vec(&(
            artifact.operation.as_str(),
            artifact.recovery_token.as_str(),
            before.fingerprint(),
            after.fingerprint(),
            desired.fingerprint(),
        ))?));
        let plan = Self {
            artifact: artifact.clone(),
            before,
            after,
            desired,
            witness,
        };
        for write in [false, true] {
            let mut size = SqlSize::default();
            plan.render_into(&mut size, write)?;
        }
        Ok(plan)
    }

    fn undo_once<R: OwnedRuntime>(
        &self,
        cold: &mut ColdSession<'_, R>,
        client: &dyn SqlClient,
    ) -> Result<UndoResult> {
        let sql = self.render(true)?;
        cold.run_cold_transaction_once(|binding| {
            self.require_binding(binding)?;
            self.execute(client, &sql)
        })
    }

    /// A confirmed witness read is safe to repeat; it cannot perform an undo or
    /// retry an unknown COMMIT. An unchanged preimage alone is never success.
    fn verify_completed<R: OwnedRuntime>(
        &self,
        cold: &mut ColdSession<'_, R>,
        client: &dyn SqlClient,
    ) -> Result<UndoResult> {
        cold.require_current()?;
        self.require_binding(&cold.session.binding)?;
        let result = self.execute(client, &self.render(false)?)?;
        cold.require_current()
            .context("undo witness read confirmed but cold postcondition unproved")?;
        Ok(result)
    }

    fn require_binding(&self, b: &LifetimeBinding) -> Result<()> {
        ensure!(
            b.database == self.artifact.identity.database
                && b.cluster == Uuid::parse_str(&self.artifact.cluster_id)?
                && b.infobase == Uuid::parse_str(&self.artifact.infobase_id)?,
            "cold lifetime differs from undo database/cluster/infobase"
        );
        Ok(())
    }

    fn execute(&self, client: &dyn SqlClient, sql: &str) -> Result<UndoResult> {
        let rows = client
            .query_rows(sql, &[])
            .context("undo COMMIT or witness response unproved; retain operation")?;
        ensure!(
            rows.len() == 1
                && rows[0].result_set == 0
                && rows[0].values.len() == 2
                && rows[0].text(0)? == self.witness,
            "undo commit witness response is unproved"
        );
        match rows[0].i64(1)? {
            0 => Ok(UndoResult::AlreadyUndone),
            1 => Ok(UndoResult::Committed),
            _ => bail!("invalid undo commit outcome"),
        }
    }

    fn render(&self, write: bool) -> Result<String> {
        let mut size = SqlSize::default();
        self.render_into(&mut size, write)?;
        let mut text = String::new();
        text.try_reserve_exact(size.bytes)
            .context("undo SQL allocation failed")?;
        let mut sql = SqlText {
            text,
            projected: size.bytes,
        };
        self.render_into(&mut sql, write)?;
        ensure!(sql.text.len() == size.bytes, "undo SQL projection drift");
        Ok(sql.text)
    }

    fn render_into<S: SqlOutput>(&self, sql: &mut S, write: bool) -> Result<()> {
        sql.push_str(&lock_prefix(&self.artifact.identity))?;
        sql.push_str(
            "BEGIN TRY\nSET TRANSACTION ISOLATION LEVEL SERIALIZABLE; BEGIN TRANSACTION;\n",
        )?;
        sql.push_str(&self.after.identity_guard()?)?;
        sql.push_str("DECLARE @ActivationLock int; EXEC @ActivationLock=sys.sp_getapplock @Resource=N'ibcmd-rs:main-activation',@LockMode='Exclusive',@LockOwner='Transaction',@LockTimeout=0; IF @ActivationLock<0 THROW 57563,'undo activation lock busy',1;\n")?;
        for (i, table) in TABLES.iter().enumerate() {
            // Lock the actual schema/tables before probing trigger/column metadata.
            writeln!(
                sql,
                "DECLARE @Count{i} bigint; SELECT @Count{i}=COUNT_BIG(*) FROM dbo.[{table}] WITH(TABLOCKX,HOLDLOCK);"
            )?;
        }
        sql.push_str(&self.after.schema_guard())?;
        sql.push_str(session_visibility_guard())?;
        sql.push_str(&exclusive_session_gate(
            57564,
            "cold undo requires no other target SQL sessions",
            &[],
        ))?;
        // An unfinished/foreign log chain never becomes undo authority merely
        // because a server process was stopped. No log backup/restoration here.
        sql.push_str(&completed_header(&self.artifact))?;
        let property = format!("ibcmd-rs:undo:{}", self.artifact.operation);
        let value = format!(
            "{}:{}:{}",
            self.artifact.recovery_token,
            self.witness,
            hex(&self.before.fingerprint())
        );
        writeln!(
            sql,
            "DECLARE @Witness sql_variant=(SELECT value FROM sys.extended_properties WHERE class=0 AND name=N'{property}'); DECLARE @Executed int=0;"
        )?;
        for (i, table) in TABLES.iter().enumerate() {
            expected_rows(sql, &format!("After{i}"), &self.after.tables[i])?;
            expected_rows(sql, &format!("Desired{i}"), &self.desired.tables[i])?;
        }
        writeln!(
            sql,
            "IF @Witness IS NOT NULL BEGIN IF CONVERT(nvarchar(128),SQL_VARIANT_PROPERTY(@Witness,'BaseType'))<>N'nvarchar' OR CONVERT(varbinary(max),CONVERT(nvarchar(max),@Witness))<>CONVERT(varbinary(max),N'{value}') OR DATALENGTH(CONVERT(nvarchar(max),@Witness))<>DATALENGTH(N'{value}') THROW 57565,'foreign undo witness',1;"
        )?;
        for (i, table) in TABLES.iter().enumerate() {
            exact_assert(sql, table, &format!("Desired{i}"))?;
        }
        sql.push_str("END ELSE BEGIN\n")?;
        if write {
            for (i, table) in TABLES.iter().enumerate() {
                exact_assert(sql, table, &format!("After{i}"))?;
            }
            for index in [0, 2] {
                let old = &self.before.tables[index];
                let after = &self.after.tables[index];
                let table = TABLES[index];
                for (key, row) in old {
                    if after.get(key) == Some(row) {
                        continue;
                    }
                    writeln!(
                        sql,
                        "DELETE FROM dbo.[{table}] WHERE CONVERT(varbinary(256),FileName)=CONVERT(varbinary(256),N'{}') AND PartNo={}; IF @@ROWCOUNT<>{} THROW 57566,'undo delete accounting drift',1;",
                        quote_string(&key.0),
                        key.1,
                        usize::from(after.contains_key(key))
                    )?;
                    insert_row(sql, table, row)?;
                }
                ensure!(
                    after.keys().all(|k| old.contains_key(k)),
                    "undo has unaccounted inserted ordinary rows"
                );
            }
            for (i, table) in TABLES.iter().enumerate() {
                exact_assert(sql, table, &format!("Desired{i}"))?;
            }
            writeln!(
                sql,
                "EXEC sys.sp_addextendedproperty @name=N'{property}',@value=N'{value}'; SET @Executed=1;"
            )?;
        } else {
            sql.push_str(
                "THROW 57567,'bound undo commit witness missing; no retry authority',1;\n",
            )?;
        }
        sql.push_str("END;\n")?;
        sql.push_str(session_visibility_guard())?;
        sql.push_str(&exclusive_session_gate(
            57564,
            "cold undo SQL session raced before commit",
            &[],
        ))?;
        sql.push_str("COMMIT TRANSACTION;\n")?;
        writeln!(
            sql,
            "SELECT N'{}',@Executed; USE [master]; EXEC sys.sp_releaseapplock @Resource=@UndoResource,@LockOwner='Session'; END TRY BEGIN CATCH IF @@TRANCOUNT>0 ROLLBACK; USE [master]; EXEC sys.sp_releaseapplock @Resource=@UndoResource,@LockOwner='Session'; THROW; END CATCH;",
            self.witness
        )?;
        Ok(())
    }
}

impl ColdSession<'_, super::native::NativeRuntime> {
    /// Production backend seam. It accepts only the original creator's SQL
    /// endpoint and its borrowed current cold capability, never a user journal.
    pub(crate) fn undo_live(
        &mut self,
        plan: &GuardedUndoPlan,
        sql: &SqlExec,
    ) -> Result<UndoResult> {
        self.require_current()?;
        self.session.runtime.require_cold_sql_target(sql)?;
        let client = sql
            .client()
            .context("built-in cold undo SQL client required")?;
        plan.undo_once(self, client)
    }

    pub(crate) fn verify_live_undo(
        &mut self,
        plan: &GuardedUndoPlan,
        sql: &SqlExec,
    ) -> Result<UndoResult> {
        self.require_current()?;
        self.session.runtime.require_cold_sql_target(sql)?;
        let client = sql
            .client()
            .context("built-in cold undo SQL client required")?;
        plan.verify_completed(self, client)
    }
}

fn validate_identity(identity: &DatabaseIdentity) -> Result<()> {
    for guid in [
        &identity.database_guid,
        &identity.family_guid,
        &identity.recovery_fork,
    ] {
        ensure!(!Uuid::parse_str(guid)?.is_nil(), "nil undo identity");
    }
    ensure!(
        !identity.database.is_empty()
            && identity.database.encode_utf16().count() <= 128
            && !identity.database.chars().any(char::is_control)
            && !identity.server.is_empty()
            && !identity.server.chars().any(char::is_control),
        "invalid undo target"
    );
    Ok(())
}
fn ident(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}
fn lock_prefix(identity: &DatabaseIdentity) -> String {
    format!(
        "USE [master]; SET NOCOUNT ON; SET XACT_ABORT ON; DECLARE @UndoResource nvarchar(255)=N'ibcmd-rs:live:{}',@UndoLock int; EXEC @UndoLock=sys.sp_getapplock @Resource=@UndoResource,@LockMode='Exclusive',@LockOwner='Session',@LockTimeout=0; IF @UndoLock<0 THROW 57568,'undo LIVE lock busy',1;\n",
        Uuid::parse_str(&identity.database_guid).expect("validated identity")
    )
}
fn parse_row(row: SqlRow) -> Result<MainStorageRow> {
    let binary = row.binary(7)?;
    let data_size = u64::try_from(row.i64(6)?)?;
    validate_row_size(binary.len(), data_size)?;
    let r = MainStorageRow {
        file_name: row.text(1)?.into(),
        part_no: i32::try_from(row.i64(2)?)?,
        creation: row.text(3)?.into(),
        modified: row.text(4)?.into(),
        attributes: i32::try_from(row.i64(5)?)?,
        data_size,
        binary_data: binary.to_vec(),
    };
    ensure!(
        !r.file_name.is_empty()
            && r.file_name.encode_utf16().count() <= 128
            && !r.file_name.chars().any(char::is_control)
            && r.part_no >= 0
            && r.data_size == r.binary_data.len() as u64
            && r.binary_data.len() <= MAX_ROW_BYTES,
        "unsupported undo physical row"
    );
    for date in [&r.creation, &r.modified] {
        ensure!(
            date.len() <= 27
                && date
                    .bytes()
                    .all(|c| c.is_ascii_digit() || b"- :.".contains(&c)),
            "undo timestamp is not canonical SQL text"
        );
    }
    Ok(r)
}
fn expected_rows<S: SqlOutput>(sql: &mut S, name: &str, rows: &Rows) -> Result<()> {
    writeln!(
        sql,
        "DECLARE @{name} TABLE(FileName nvarchar(128) COLLATE Latin1_General_100_BIN2,PartNo int,Creation varchar(27),Modified varchar(27),Attributes int,DataSize bigint,ByteLength bigint,Digest binary(32),NameBytes varbinary(256),PRIMARY KEY(FileName,PartNo));"
    )?;
    for row in rows.values() {
        writeln!(
            sql,
            "INSERT @{name} VALUES(N'{}',{},'{}','{}',{},{},{},0x{},CONVERT(varbinary(256),N'{}'));",
            quote_string(&row.file_name),
            row.part_no,
            row.creation,
            row.modified,
            row.attributes,
            row.data_size,
            row.binary_data.len(),
            hex(&row.sha256()),
            quote_string(&row.file_name)
        )?;
    }
    Ok(())
}
fn exact_assert<S: SqlOutput>(sql: &mut S, table: &str, name: &str) -> Result<()> {
    let actual = format!(
        "SELECT FileName COLLATE Latin1_General_100_BIN2,PartNo,CONVERT(varchar(27),Creation,121),CONVERT(varchar(27),Modified,121),CONVERT(int,Attributes),CONVERT(bigint,DataSize),CONVERT(bigint,DATALENGTH(BinaryData)),HASHBYTES('SHA2_256',BinaryData),CONVERT(varbinary(256),FileName) FROM dbo.[{table}] WITH(TABLOCKX,HOLDLOCK)"
    );
    writeln!(
        sql,
        "IF (SELECT COUNT_BIG(*) FROM dbo.[{table}] WITH(TABLOCKX,HOLDLOCK))<>(SELECT COUNT_BIG(*) FROM @{name}) OR EXISTS({actual} EXCEPT SELECT * FROM @{name}) OR EXISTS(SELECT * FROM @{name} EXCEPT {actual}) THROW 57569,'{table} full undo snapshot changed',1;"
    )?;
    Ok(())
}
fn insert_row<S: SqlOutput>(sql: &mut S, table: &str, row: &MainStorageRow) -> Result<()> {
    write!(
        sql,
        "INSERT dbo.[{table}](FileName,PartNo,Creation,Modified,Attributes,DataSize,BinaryData) VALUES(N'{}',{},'{}','{}',{},{},0x",
        quote_string(&row.file_name),
        row.part_no,
        row.creation,
        row.modified,
        row.attributes,
        row.data_size
    )?;
    sql.binary_hex(&row.binary_data)
        .context("undo restore payload exceeds SQL bound")?;
    writeln!(
        sql,
        "); IF @@ROWCOUNT<>1 THROW 57570,'undo insert accounting drift',1;"
    )?;
    Ok(())
}
fn completed_header(artifact: &LiveArtifact) -> String {
    let mut sql = String::from("USE [master];\n");
    sql.push_str(crate::mssql_live_continue::header_table());
    writeln!(
        sql,
        "INSERT #LiveHeader EXEC(N'RESTORE HEADERONLY FROM DISK=N''{}'' WITH CHECKSUM');",
        quote_string(&quote_string(&artifact.tail))
    )
    .unwrap();
    let id = &artifact.identity;
    writeln!(sql,"IF (SELECT COUNT(*) FROM #LiveHeader)<>2 OR EXISTS(SELECT 1 FROM #LiveHeader WHERE BackupType IS NULL OR BackupType<>2 OR DatabaseName IS NULL OR DatabaseName<>N'{}' OR ServerName IS NULL OR ServerName<>N'{}' OR BindingID IS NULL OR BindingID<>'{}' OR FamilyGUID IS NULL OR FamilyGUID<>'{}' OR RecoveryForkID IS NULL OR RecoveryForkID<>'{}' OR FirstRecoveryForkID IS NULL OR FirstRecoveryForkID<>RecoveryForkID OR HasBackupChecksums IS NULL OR HasBackupChecksums<>1 OR IsDamaged IS NULL OR IsDamaged<>0 OR HasIncompleteMetaData IS NULL OR HasIncompleteMetaData<>0 OR IsCopyOnly IS NULL OR IsCopyOnly<>0 OR IsForceOffline IS NULL OR IsForceOffline<>1 OR BackupFinishDate IS NULL OR BackupSetGUID IS NULL OR FirstLSN IS NULL OR LastLSN IS NULL OR FirstLSN>=LastLSN) THROW 57571,'undo LIVE header identity/flags changed',1;",quote_string(&id.database),quote_string(&id.server),id.database_guid,id.family_guid,id.recovery_fork).unwrap();
    writeln!(sql,"IF NOT EXISTS(SELECT 1 FROM #LiveHeader a JOIN #LiveHeader b ON a.Position=1 AND b.Position=2 AND a.LastLSN=b.FirstLSN AND a.DatabaseBackupLSN=b.DatabaseBackupLSN AND a.BackupSetGUID<>b.BackupSetGUID WHERE a.BackupName=N'{}' AND b.BackupName=N'{}') OR NOT EXISTS(SELECT 1 FROM sys.database_recovery_status r JOIN #LiveHeader b ON b.Position=2 WHERE r.database_id=DB_ID(N'{}') AND r.last_log_backup_lsn=b.LastLSN) THROW 57572,'undo LIVE operation/chain changed',1;\nUSE {};",quote_string(&artifact.backup_name(1)),quote_string(&artifact.backup_name(2)),quote_string(&id.database),ident(&id.database)).unwrap();
    sql
}

#[cfg(test)]
mod tests;
