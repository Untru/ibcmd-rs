//! Recoverable, idle-infobase checkpoint for the two-cycle live switch.
//! Active-session readiness is deliberately not inferred from SQL handles.

#[cfg(test)]
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use clap::Args;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::mssql_main_activation::{MainActivationRecoverySnapshot, quote_string};
use crate::sql::{SqlBackend, SqlClient, SqlExec, SqlOptions};

pub const LIVE_SQL_VERSION: &str = "17.0.1135.8";
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Args)]
pub struct LiveContinueArgs {
    #[arg(long)]
    pub artifact: PathBuf,
    #[arg(long)]
    pub database: String,
    #[arg(long, default_value = "localhost")]
    pub server: String,
    #[arg(long)]
    pub sql_user: Option<String>,
    #[arg(long)]
    pub sql_pwd: Option<String>,
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Accept rollback of SQL work which starts before the second recovery.
    #[arg(long)]
    pub interrupt_sessions: bool,
    #[arg(long, default_value = "rac")]
    pub rac: PathBuf,
    #[arg(long, default_value = "localhost:1545")]
    pub ras_endpoint: String,
    #[arg(long)]
    pub infobase_user: Option<String>,
    #[arg(long)]
    pub infobase_pwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseIdentity {
    pub server: String,
    pub database: String,
    pub database_guid: String,
    pub family_guid: String,
    pub recovery_fork: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveArtifact {
    pub format: u32,
    pub sql_engine_version: String,
    pub verified_platform_profile: String,
    pub storage_schema_sha256: String,
    pub operation: String,
    pub identity: DatabaseIdentity,
    pub tail: String,
    pub recovery_token: String,
    pub cluster_id: String,
    pub infobase_id: String,
    pub recovery: MainActivationRecoverySnapshot,
}

#[derive(Debug, Serialize)]
pub struct LiveContinueReport {
    pub artifact: PathBuf,
    pub state: String,
    pub cycle_2_executed: Option<bool>,
    /// Warm clients need a restart to refresh client-side code; no zero-error promise.
    pub client_restart_required: bool,
    pub detail: String,
}

/// Refuse an artifact the standalone reader cannot recover, before cycle 1.
pub fn serialize_artifact(artifact: &LiveArtifact) -> Result<Vec<u8>> {
    serialize_artifact_with_limit(artifact, MAX_ARTIFACT_BYTES as usize)
}

fn serialize_artifact_with_limit(artifact: &LiveArtifact, limit: usize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(artifact)?;
    ensure!(
        bytes.len() <= limit,
        "serialized live artifact exceeds the continuation reader limit; promotion/cycle 1 was not started"
    );
    Ok(bytes)
}

pub fn pending_operation_query(database: &str) -> String {
    format!(
        "SELECT TOP (1) a.name FROM msdb.dbo.backupset a JOIN sys.database_recovery_status r ON a.database_guid=r.database_guid WHERE r.database_id=DB_ID(N'{}') AND a.type='L' AND a.name LIKE N'ibcmd-rs:live:%:1' AND NOT EXISTS (SELECT 1 FROM msdb.dbo.backupset b WHERE b.database_guid=a.database_guid AND b.type='L' AND b.name=LEFT(a.name,LEN(a.name)-1)+N'2') ORDER BY a.backup_set_id DESC",
        quote_string(database)
    )
}

/// Serialize the source-stage mutation/default LIVE activation against a split operation.
/// The check and the complete supplied SQL body execute on one session in master.
/// No target database write precedes the lock or pending-history check.
pub fn guard_pending_live(database: &str, body: &str) -> String {
    let db = quote_string(database);
    let pending = pending_operation_query(database);
    format!(
        "USE [master]; DECLARE @LiveGuardResource nvarchar(255), @LiveGuardLock int; SELECT @LiveGuardResource=N'ibcmd-rs:live:'+LOWER(CONVERT(nvarchar(36),database_guid)) FROM sys.database_recovery_status WHERE database_id=DB_ID(N'{db}'); IF @LiveGuardResource IS NULL THROW 57262,'live guard database identity is unavailable',1; EXEC @LiveGuardLock=sys.sp_getapplock @Resource=@LiveGuardResource,@LockMode='Exclusive',@LockOwner='Session',@LockTimeout=0; IF @LiveGuardLock<0 THROW 57260,'live switch is already running',1; BEGIN TRY IF EXISTS ({pending}) THROW 57269,'an earlier live cycle 1 is unfinished; staging/activation refused before any target write',1;\n{body}\nUSE [master]; EXEC sys.sp_releaseapplock @Resource=@LiveGuardResource,@LockOwner='Session'; END TRY BEGIN CATCH IF @@TRANCOUNT>0 ROLLBACK; USE [master]; EXEC sys.sp_releaseapplock @Resource=@LiveGuardResource,@LockOwner='Session'; THROW; END CATCH;"
    )
}

pub fn preflight_backend(sql: &SqlExec, database: &str) -> Result<()> {
    let SqlBackend::Client(client) = sql.backend() else {
        bail!(
            "live phase splitting requires the built-in SQL client; --sqlcmd is refused before staging"
        );
    };
    let version = client.query_scalar(
        "SELECT CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))",
        &[],
    )?;
    ensure!(
        version.as_ref().and_then(|v| v.as_str()) == Some(LIVE_SQL_VERSION),
        "live checkpoint is measured on SQL Server 17.0.1135.8 only; nothing was staged or promoted"
    );
    ensure!(
        client
            .query_scalar(&pending_operation_query(database), &[])?
            .is_none(),
        "an earlier live cycle 1 is unfinished; use its saved mssql-live-continue artifact before another live activation"
    );
    Ok(())
}

pub fn identity(client: &dyn SqlClient, database: &str) -> Result<DatabaseIdentity> {
    let rows = client.query_rows(&format!("SELECT CONVERT(nvarchar(128),SERVERPROPERTY('ServerName')), d.name, CONVERT(varchar(36),r.database_guid), CONVERT(varchar(36),r.family_guid), CONVERT(varchar(36),r.recovery_fork_guid) FROM sys.databases d JOIN sys.database_recovery_status r ON r.database_id=d.database_id WHERE d.name=N'{}' AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER'", quote_string(database)), &[])?;
    ensure!(
        rows.len() == 1,
        "live continuation requires an ONLINE/MULTI_USER database"
    );
    let row = &rows[0];
    let result = DatabaseIdentity {
        server: row.text(0)?.to_owned(),
        database: row.text(1)?.to_owned(),
        database_guid: row.text(2)?.to_owned(),
        family_guid: row.text(3)?.to_owned(),
        recovery_fork: row.text(4)?.to_owned(),
    };
    for value in [
        &result.database_guid,
        &result.family_guid,
        &result.recovery_fork,
    ] {
        Uuid::parse_str(value)?;
    }
    Ok(result)
}

impl LiveArtifact {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.format == 1, "unsupported live artifact format");
        ensure!(
            self.sql_engine_version == LIVE_SQL_VERSION,
            "unsupported saved live SQL engine version"
        );
        ensure!(
            self.verified_platform_profile == "platform-8.3.27.2214",
            "unsupported saved live platform profile"
        );
        ensure!(
            self.storage_schema_sha256.len() == 64
                && self
                    .storage_schema_sha256
                    .bytes()
                    .all(|ch| ch.is_ascii_hexdigit()),
            "invalid saved storage profile digest"
        );
        Uuid::parse_str(&self.operation)?;
        Uuid::parse_str(&self.cluster_id)?;
        Uuid::parse_str(&self.infobase_id)?;
        crate::mssql_main_activation::validate_tail_log_output(&self.tail)?;
        ensure!(
            Path::new(&self.tail).is_absolute(),
            "live artifact tail must be absolute"
        );
        ensure!(
            self.recovery_token.len() == 64
                && self.recovery_token.bytes().all(|ch| ch.is_ascii_hexdigit()),
            "invalid saved live recovery token"
        );
        let token = format!("{:x}", Sha256::digest(serde_json::to_vec(&self.recovery)?));
        // The production renderer uses uppercase hex. Preserve its spelling for
        // backup names, while comparing the exact digest rather than hex case.
        ensure!(
            token.eq_ignore_ascii_case(&self.recovery_token),
            "live recovery token does not match its snapshot"
        );
        ensure!(
            !self.recovery.staged_rows.is_empty(),
            "live artifact has no staged rows"
        );
        ensure!(
            self.recovery.old_generation != self.recovery.new_generation,
            "live artifact describes a no-op"
        );
        Uuid::parse_str(&self.recovery.new_generation)?;
        for value in [
            &self.identity.database_guid,
            &self.identity.family_guid,
            &self.identity.recovery_fork,
        ] {
            Uuid::parse_str(value)?;
        }
        Ok(())
    }

    pub fn backup_name(&self, cycle: u8) -> String {
        format!("ibcmd-rs:live:{}:{cycle}", self.recovery_token)
    }
}

/// All 59 HEADERONLY columns measured on SQL Server 17.0.1135.8. Other builds refuse.
/// Source: Microsoft RESTORE HEADERONLY result-set contract; see the evidence document.
pub(crate) fn header_table() -> &'static str {
    "DROP TABLE IF EXISTS #LiveHeader; CREATE TABLE #LiveHeader (BackupName nvarchar(128), BackupDescription nvarchar(255), BackupType tinyint, ExpirationDate datetime, Compressed tinyint, Position smallint, DeviceType tinyint, UserName nvarchar(128), ServerName nvarchar(128), DatabaseName nvarchar(128), DatabaseVersion int, DatabaseCreationDate datetime, BackupSize bigint, FirstLSN numeric(25,0), LastLSN numeric(25,0), CheckpointLSN numeric(25,0), DatabaseBackupLSN numeric(25,0), BackupStartDate datetime, BackupFinishDate datetime, SortOrder smallint, CodePage smallint, UnicodeLocaleId int, UnicodeComparisonStyle int, CompatibilityLevel tinyint, SoftwareVendorId int, SoftwareVersionMajor int, SoftwareVersionMinor int, SoftwareVersionBuild int, MachineName nvarchar(128), Flags int, BindingID uniqueidentifier, RecoveryForkID uniqueidentifier, Collation nvarchar(128), FamilyGUID uniqueidentifier, HasBulkLoggedData bit, IsSnapshot bit, IsReadOnly bit, IsSingleUser bit, HasBackupChecksums bit, IsDamaged bit, BeginsLogChain bit, HasIncompleteMetaData bit, IsForceOffline bit, IsCopyOnly bit, FirstRecoveryForkID uniqueidentifier, ForkPointLSN numeric(25,0), RecoveryModel nvarchar(60), DifferentialBaseLSN numeric(25,0), DifferentialBaseGUID uniqueidentifier, BackupTypeDescription nvarchar(60), BackupSetGUID uniqueidentifier, CompressedBackupSize bigint, Containment tinyint, KeyAlgorithm nvarchar(32), EncryptorThumbprint varbinary(20), EncryptorType nvarchar(32), LastValidRestoreTime datetime, TimeZone smallint, CompressionAlgorithm nvarchar(32));\n"
}

fn database_ident(database: &str) -> Result<String> {
    ensure!(
        !database.is_empty()
            && database.encode_utf16().count() <= 128
            && !database.chars().any(char::is_control),
        "invalid database identifier"
    );
    Ok(format!("[{}]", database.replace(']', "]]")))
}

/// The header check is re-run inside the same master session/application lock as the append.
/// Two genuine operation-owned sets return without another interruption; arbitrary two sets refuse.
pub fn render_continue(artifact: &LiveArtifact, interrupt: bool, execute: bool) -> Result<String> {
    artifact.validate()?;
    let db = database_ident(&artifact.identity.database)?;
    let name = quote_string(&artifact.identity.database);
    let tail = quote_string(&artifact.tail);
    let resource = format!(
        "ibcmd-rs:live:{}",
        Uuid::parse_str(&artifact.identity.database_guid)?
    );
    let mut sql = format!(
        "USE [master]; SET NOCOUNT ON; SET XACT_ABORT ON;\nDECLARE @LiveLock int; EXEC @LiveLock=sys.sp_getapplock @Resource=N'{resource}', @LockMode='Exclusive', @LockOwner='Session', @LockTimeout=0; IF @LiveLock<0 THROW 57260, 'live continuation is already running',1;\nDECLARE @LiveAccessChanged bit=0, @LiveExecuted bit=0;\nBEGIN TRY\nIF CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))<>N'17.0.1135.8' THROW 57261,'live continuation HEADERONLY schema is measured on SQL Server 17.0.1135.8 only',1;\nIF NOT EXISTS (SELECT 1 FROM sys.databases d JOIN sys.database_recovery_status r ON r.database_id=d.database_id WHERE d.name=N'{name}' AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER' AND d.recovery_model IN (1,2) AND r.last_log_backup_lsn IS NOT NULL AND CONVERT(nvarchar(128),SERVERPROPERTY('ServerName'))=N'{}' AND r.database_guid='{}' AND r.family_guid='{}' AND r.recovery_fork_guid='{}') THROW 57262,'live continuation database identity/state changed',1;\n",
        quote_string(&artifact.identity.server),
        artifact.identity.database_guid,
        artifact.identity.family_guid,
        artifact.identity.recovery_fork
    );
    sql.push_str(header_table());
    sql.push_str(&format!(
        "INSERT #LiveHeader EXEC(N'RESTORE HEADERONLY FROM DISK=N''{}'' WITH CHECKSUM');\n",
        quote_string(&tail)
    ));
    sql.push_str(&format!("IF (SELECT COUNT(*) FROM #LiveHeader) NOT IN (1,2) OR EXISTS (SELECT 1 FROM #LiveHeader WHERE BackupType<>2 OR BackupType IS NULL OR DatabaseName<>N'{name}' OR DatabaseName IS NULL OR ServerName<>N'{}' OR ServerName IS NULL OR BindingID<>'{}' OR BindingID IS NULL OR FamilyGUID<>'{}' OR FamilyGUID IS NULL OR RecoveryForkID<>'{}' OR RecoveryForkID IS NULL OR FirstRecoveryForkID<>RecoveryForkID OR FirstRecoveryForkID IS NULL OR HasBackupChecksums<>1 OR HasBackupChecksums IS NULL OR IsDamaged<>0 OR IsDamaged IS NULL OR HasIncompleteMetaData<>0 OR HasIncompleteMetaData IS NULL OR IsCopyOnly<>0 OR IsCopyOnly IS NULL OR IsForceOffline<>1 OR IsForceOffline IS NULL OR BackupFinishDate IS NULL OR BackupSetGUID IS NULL OR FirstLSN IS NULL OR LastLSN IS NULL OR FirstLSN>=LastLSN) THROW 57263,'ambiguous or foreign live log artifact',1;\nIF (SELECT COUNT(*) FROM #LiveHeader WHERE Position=1 AND BackupName=N'{}')<>1 OR EXISTS (SELECT 1 FROM #LiveHeader WHERE Position NOT IN (1,2) OR (Position=2 AND (BackupName<>N'{}' OR BackupName IS NULL))) THROW 57264,'log sets are not the recorded live cycles',1;\n", quote_string(&artifact.identity.server), artifact.identity.database_guid, artifact.identity.family_guid, artifact.identity.recovery_fork, artifact.backup_name(1), artifact.backup_name(2)));
    // A tail belonging to the operation does not permit continuing a later configuration or dirty stage.
    sql.push_str(&format!("USE {db};\nIF EXISTS (SELECT 1 FROM dbo.ConfigSave) OR EXISTS (SELECT 1 FROM dbo.Config WHERE FileName=N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%') OR EXISTS (SELECT 1 FROM dbo.Params WHERE FileName=N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%') THROW 57265,'live continuation stage/markers changed',1;\n"));
    for row in &artifact.recovery.staged_rows {
        sql.push_str(&format!("IF (SELECT COUNT_BIG(*) FROM dbo.Config WHERE FileName=N'{}' AND PartNo={} AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{:x})<>1 THROW 57266,'live continuation published generation/rows changed',1;\n", quote_string(&row.file_name), row.part_no, row.data_size, Sha256::digest(&row.binary_data)));
    }
    sql.push_str("USE [master];\nIF (SELECT COUNT(*) FROM #LiveHeader)=2 BEGIN\nIF NOT EXISTS (SELECT 1 FROM #LiveHeader a JOIN #LiveHeader b ON a.Position=1 AND b.Position=2 WHERE a.LastLSN=b.FirstLSN AND a.DatabaseBackupLSN=b.DatabaseBackupLSN AND a.BackupSetGUID<>b.BackupSetGUID) THROW 57267,'live backup chain is not contiguous',1;\nEND\n");
    if execute {
        sql.push_str("ELSE BEGIN\n");
        sql.push_str(&render_continuation_sessions_check(
            &artifact.identity.database,
            interrupt,
        ));
        // Refuse an intervening log backup: merely matching two set counts is not chain evidence.
        sql.push_str(&format!("IF NOT EXISTS (SELECT 1 FROM sys.database_recovery_status r JOIN #LiveHeader h ON h.Position=1 WHERE r.database_id=DB_ID(N'{name}') AND r.last_log_backup_lsn=h.LastLSN) THROW 57268,'live log chain advanced after cycle 1',1;\nALTER DATABASE {db} SET SINGLE_USER WITH ROLLBACK IMMEDIATE;\nSET @LiveAccessChanged=1;\nBACKUP LOG {db} TO DISK=N'{tail}' WITH NORECOVERY, NOINIT, COMPRESSION, CHECKSUM, NAME=N'{}';\nRESTORE DATABASE {db} WITH RECOVERY;\nALTER DATABASE {db} SET MULTI_USER;\nSET @LiveAccessChanged=0; SET @LiveExecuted=1;\nEND\n", artifact.backup_name(2)));
    }
    sql.push_str(&format!("SELECT CONVERT(int,(SELECT COUNT(*) FROM #LiveHeader)),CONVERT(int,@LiveExecuted);\nEXEC sys.sp_releaseapplock @Resource=N'{resource}',@LockOwner='Session';\nEND TRY BEGIN CATCH\nIF @LiveAccessChanged=1 AND DB_ID(N'{name}') IS NOT NULL AND DATABASEPROPERTYEX(N'{name}','Status')<>N'RESTORING' ALTER DATABASE {db} SET MULTI_USER;\nUSE [master];\nEXEC sys.sp_releaseapplock @Resource=N'{resource}',@LockOwner='Session';\nTHROW; END CATCH;\n"));
    Ok(sql)
}

/// Empty RAS user-session inventory is the only readiness case accepted by this checkpoint.
/// Empty SQL connection inventory is never used as evidence.
pub fn require_idle_ras(
    rac: &Path,
    endpoint: &str,
    artifact: &LiveArtifact,
    user: Option<&str>,
    pwd: Option<&str>,
) -> Result<()> {
    let version = rac_bounded(rac, &["agent", "version", endpoint], Duration::from_secs(5))?;
    let build = crate::mssql_platform_profile::parse_rac_agent_build(&version)?;
    require_saved_build(&artifact.verified_platform_profile, &build)?;
    let cluster = format!("--cluster={}", artifact.cluster_id);
    let ib = format!("--infobase={}", artifact.infobase_id);
    let mut args = vec!["infobase", "info", cluster.as_str(), ib.as_str()];
    let userarg = user.map(|user| format!("--infobase-user={user}"));
    let pwdarg = user.map(|_| format!("--infobase-pwd={}", pwd.unwrap_or_default()));
    if let Some(user) = &userarg {
        args.push(user);
    }
    if let Some(pwd) = &pwdarg {
        args.push(pwd);
    }
    args.push(endpoint);
    let binding = rac_bounded(rac, &args, Duration::from_secs(5))?;
    validate_ras_binding(&binding, artifact)?;
    let text = rac_bounded_bytes(
        rac,
        &[
            "session",
            "list",
            &format!("--cluster={}", artifact.cluster_id),
            &format!("--infobase={}", artifact.infobase_id),
            endpoint,
        ],
        Duration::from_secs(5),
    )?;
    idle_session_inventory(&text)
}

fn require_saved_build(profile: &str, build: &str) -> Result<()> {
    ensure!(
        profile == "platform-8.3.27.2214" && build == "8.3.27.2214",
        "live continuation platform changed or is unsupported; cycle 2 was not started"
    );
    Ok(())
}

fn idle_session_inventory(text: &[u8]) -> Result<()> {
    ensure!(
        text.iter().all(u8::is_ascii_whitespace),
        "warm/session readiness is not yet measured: RAS lists sessions; cycle 1 is retained, cycle 2 was not started. End the owned sessions and run mssql-live-continue"
    );
    Ok(())
}

fn rac_bounded(rac: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    let stdout = rac_bounded_bytes(rac, args, timeout)?;
    Ok(String::from_utf8(stdout).context("RAS readiness output is not UTF-8")?)
}

/// RAS session presence needs no decoding of OEM user names. Only an empty,
/// successful, bounded response admits cycle 2; identity/version stay strict UTF-8.
fn rac_bounded_bytes(rac: &Path, args: &[&str], timeout: Duration) -> Result<Vec<u8>> {
    let mut child = Command::new(rac)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    fn drain(mut stream: impl Read) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            ensure!(bytes.len() + n <= 1024 * 1024, "rac output exceeded bound");
            bytes.extend_from_slice(&buffer[..n]);
        }
        Ok(bytes)
    }
    let out = std::thread::spawn(move || drain(stdout));
    let err = std::thread::spawn(move || drain(stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out.join();
            let _ = err.join();
            bail!("RAS session readiness timed out; cycle 2 was not started");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("rac stdout reader panicked"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("rac stderr reader panicked"))??;
    ensure!(
        status.success() && stderr.is_empty(),
        "RAS session readiness failed: {}",
        String::from_utf8_lossy(&stderr)
    );
    Ok(stdout)
}

fn render_continuation_sessions_check(database: &str, interrupt: bool) -> String {
    if interrupt {
        return String::new();
    }
    let condition = crate::mssql_live_gate::active_work_condition(&quote_string(database));
    format!(
        "IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions s WHERE {condition}) THROW 57239, 'live continuation refused: sessions of the database started work; cycle 2 was not started; committed promotion/cycle 1 is retained. Retry when sessions are idle', 1;\n"
    )
}

pub fn finish(
    client: &dyn SqlClient,
    artifact: &LiveArtifact,
    path: &Path,
    rac: &Path,
    endpoint: &str,
    user: Option<&str>,
    pwd: Option<&str>,
    interrupt: bool,
) -> Result<LiveContinueReport> {
    finish_with_readiness(client, artifact, path, interrupt, || {
        let storage = crate::mssql_platform_profile::verify_mssql_storage_profile(
            crate::mssql_platform_profile::MssqlNativePlatformProfile::Platform8_3_27_2214,
            client,
            &artifact.identity.database,
        )?;
        ensure!(
            storage.storage_schema_sha256 == artifact.storage_schema_sha256,
            "live continuation storage profile changed; cycle 2 was not started"
        );
        require_idle_ras(rac, endpoint, artifact, user, pwd)
    })
}

fn finish_with_readiness(
    client: &dyn SqlClient,
    artifact: &LiveArtifact,
    path: &Path,
    interrupt: bool,
    mut idle: impl FnMut() -> Result<()>,
) -> Result<LiveContinueReport> {
    let state = validated_state(client, artifact, interrupt, false)?;
    if state.0 == 2 {
        return Ok(report(path, "already_complete", false));
    }
    idle()?;
    // Recheck the real RAS inventory immediately before the atomic header/append guard.
    idle()?;
    let state = validated_state(client, artifact, interrupt, true)?;
    let final_state = validated_state(client, artifact, interrupt, false)?;
    ensure!(
        final_state.0 == 2,
        "cycle 2 completion is ambiguous; inspect retained artifact"
    );
    Ok(report(
        path,
        if state.1 {
            "complete"
        } else {
            "already_complete"
        },
        state.1,
    ))
}

fn validated_state(
    client: &dyn SqlClient,
    artifact: &LiveArtifact,
    interrupt: bool,
    execute: bool,
) -> Result<(i64, bool)> {
    let rows = client.query_rows(&render_continue(artifact, interrupt, execute)?, &[])?;
    ensure!(
        rows.len() == 1,
        "live state query returned ambiguous results"
    );
    let count = rows[0].i64(0)?;
    let executed = rows[0].i64(1)?;
    ensure!(
        matches!(count, 1 | 2) && matches!(executed, 0 | 1),
        "live state result is invalid"
    );
    Ok((count, executed == 1))
}

fn validate_ras_binding(text: &str, artifact: &LiveArtifact) -> Result<()> {
    let mut values = std::collections::BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (key, value) = line
            .split_once(':')
            .context("unrecognized RAS binding output")?;
        ensure!(
            values
                .insert(key.trim(), value.trim().trim_matches('"'))
                .is_none(),
            "ambiguous RAS registration"
        );
    }
    fn server(s: &str) -> String {
        let s = s.trim().to_ascii_lowercase();
        let host = std::env::var("COMPUTERNAME")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(s.as_str(), "localhost" | "." | "(local)" | "127.0.0.1")
            || (!host.is_empty() && s == host)
        {
            "local".to_owned()
        } else {
            s
        }
    }
    ensure!(
        values.get("infobase") == Some(&artifact.infobase_id.as_str())
            && values.get("dbms") == Some(&"MSSQLServer")
            && values.get("db-name") == Some(&artifact.identity.database.as_str())
            && values
                .get("db-server")
                .is_some_and(|v| server(v) == server(&artifact.identity.server)),
        "RAS registration does not bind to the recorded SQL database"
    );
    Ok(())
}

fn report(path: &Path, state: &str, executed: bool) -> LiveContinueReport {
    LiveContinueReport { artifact: path.to_owned(), state: state.to_owned(), cycle_2_executed: Some(executed), client_restart_required: true, detail: "Empty RAS user-session readiness only; active/warm cohort readiness and zero-error switching are not established".to_owned() }
}

pub fn unresolved(path: &Path, error: &anyhow::Error) -> LiveContinueReport {
    LiveContinueReport {
        artifact: path.to_owned(),
        state: "continuation_required".to_owned(),
        cycle_2_executed: None,
        client_restart_required: true,
        detail: format!(
            "completion not established; inspect retained artifact before retry: {error:#}"
        ),
    }
}

pub fn run(args: &LiveContinueArgs) -> Result<LiveContinueReport> {
    ensure!(
        args.allow_non_lab,
        "--allow-non-lab acknowledgement is required"
    );
    let artifact = crate::mssql_live_artifact::read(&args.artifact)?;
    ensure!(
        artifact.identity.database == args.database,
        "live artifact belongs to another database"
    );
    let password = args
        .sql_pwd
        .clone()
        .or_else(|| std::env::var(&args.sql_pwd_env).ok());
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: true,
    })?;
    let SqlBackend::Client(client) = sql.backend() else {
        bail!("live continuation requires the built-in SQL client");
    };
    ensure!(
        identity(client, &args.database)? == artifact.identity,
        "live artifact database/server identity changed"
    );
    finish(
        client,
        &artifact,
        &args.artifact,
        &args.rac,
        &args.ras_endpoint,
        args.infobase_user.as_deref(),
        args.infobase_pwd.as_deref(),
        args.interrupt_sessions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_main_activation::MainStorageRow;
    use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow, SqlValue};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture() -> LiveArtifact {
        let recovery = MainActivationRecoverySnapshot {
            old_generation: Uuid::nil().to_string(),
            new_generation: Uuid::from_u128(1).to_string(),
            overwritten_config_rows: vec![],
            retained_config_rows: vec![],
            prior_config_dynamically_updated: None,
            prior_params_dynamically_updated: None,
            staged_rows: vec![MainStorageRow {
                file_name: "versions".into(),
                part_no: 0,
                creation: "".into(),
                modified: "".into(),
                attributes: 0,
                data_size: 3,
                binary_data: vec![1, 2, 3],
            }],
        };
        LiveArtifact {
            format: 1,
            sql_engine_version: LIVE_SQL_VERSION.into(),
            verified_platform_profile: "platform-8.3.27.2214".into(),
            storage_schema_sha256: "a".repeat(64),
            operation: Uuid::from_u128(2).to_string(),
            identity: DatabaseIdentity {
                server: "localhost".into(),
                database: "lab]db".into(),
                database_guid: Uuid::from_u128(3).to_string(),
                family_guid: Uuid::from_u128(4).to_string(),
                recovery_fork: Uuid::from_u128(5).to_string(),
            },
            tail: std::env::temp_dir()
                .join("tail's.trn")
                .to_string_lossy()
                .into_owned(),
            recovery_token: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&recovery).unwrap())
            ),
            cluster_id: Uuid::from_u128(6).to_string(),
            infobase_id: Uuid::from_u128(7).to_string(),
            recovery,
        }
    }

    #[test]
    fn recovery_token_rejects_wrong_digest_nonhex_and_wrong_length() {
        for token in ["0".repeat(64), "z".repeat(64), "a".repeat(63)] {
            let mut artifact = fixture();
            artifact.recovery_token = token;
            assert!(artifact.validate().is_err());
        }
    }

    #[test]
    fn real_checkpoint_renderer_token_roundtrips_without_changing_backup_names() {
        use crate::mssql_main_activation::{
            MainActivationMode, MainActivationSnapshot, prepare_main_activation,
            render_main_activation_checkpoint,
        };
        use flate2::{Compression, write::DeflateEncoder};
        use std::io::Write;

        let body = Uuid::from_u128(9).to_string();
        let rows = |generation: Uuid, text: &[u8]| {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
            write!(encoder, "{{1,4,\"\",{generation},\"root\",x}}").unwrap();
            [
                (body.clone(), text.to_vec()),
                ("root".into(), vec![1]),
                ("version".into(), vec![2]),
                ("versions".into(), encoder.finish().unwrap()),
            ]
            .into_iter()
            .map(|(file_name, binary_data)| MainStorageRow {
                file_name,
                part_no: 0,
                creation: String::new(),
                modified: String::new(),
                attributes: 0,
                data_size: binary_data.len() as u64,
                binary_data,
            })
            .collect()
        };
        let plan = prepare_main_activation(
            MainActivationMode::Live,
            rows(Uuid::from_u128(11), b"new body"),
            MainActivationSnapshot {
                config_rows: rows(Uuid::from_u128(10), b"old body"),
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[body],
            true,
        )
        .unwrap();
        let mut artifact = fixture();
        let script =
            render_main_activation_checkpoint("lab]db", &plan, Some(&artifact.tail), false)
                .unwrap();
        artifact.recovery = script.recovery;
        artifact.recovery_token = script.report.recovery_token;
        artifact.validate().unwrap();
        let saved: LiveArtifact =
            serde_json::from_slice(&serialize_artifact(&artifact).unwrap()).unwrap();
        saved.validate().unwrap();
        assert_eq!(saved.recovery_token, artifact.recovery_token);
        assert!(script.sql.contains(&quote_string(&saved.backup_name(1))));
        assert!(
            render_continue(&saved, false, true)
                .unwrap()
                .contains(&quote_string(&saved.backup_name(2)))
        );
    }

    #[test]
    fn serialized_artifact_boundary_matches_the_continuation_reader_limit() {
        let artifact = fixture();
        let bytes = serialize_artifact(&artifact).unwrap();
        assert_eq!(
            serialize_artifact_with_limit(&artifact, bytes.len()).unwrap(),
            bytes
        );
        assert!(
            serialize_artifact_with_limit(&artifact, bytes.len() - 1)
                .unwrap_err()
                .to_string()
                .contains("cycle 1 was not started")
        );
        assert!(bytes.len() as u64 <= MAX_ARTIFACT_BYTES);
    }

    #[test]
    fn delayed_runtime_upgrade_refuses_and_the_saved_build_is_mandatory() {
        assert!(require_saved_build("platform-8.3.27.2214", "8.3.27.2214").is_ok());
        for (saved, actual) in [
            ("platform-8.3.27.2214", "8.5.1.1150"),
            ("platform-8.5.1.1150", "8.3.27.2214"),
            ("", "8.3.27.2214"),
        ] {
            assert!(
                require_saved_build(saved, actual)
                    .unwrap_err()
                    .to_string()
                    .contains("cycle 2 was not started")
            );
        }
        let mut saved = fixture();
        saved.verified_platform_profile.clear();
        assert!(saved.validate().is_err());
        let mut saved = fixture();
        saved.sql_engine_version = "16.0.1000.6".into();
        assert!(
            saved
                .validate()
                .unwrap_err()
                .to_string()
                .contains("SQL engine")
        );
    }

    #[test]
    fn lock_resource_uses_canonical_uuid_spelling() {
        let mut artifact = fixture();
        artifact.identity.database_guid = "12345678-ABCD-1234-ABCD-123456789ABC".into();
        assert!(
            render_continue(&artifact, false, false)
                .unwrap()
                .contains("@Resource=N'ibcmd-rs:live:12345678-abcd-1234-abcd-123456789abc'")
        );
        assert!(
            guard_pending_live("lab", "SELECT 1;")
                .contains("LOWER(CONVERT(nvarchar(36),database_guid))")
        );
    }

    /// Creates bounded SQL-only engine fixtures on the one disposable clone created by this track.
    /// Does not write the database, call RAS, or claim a platform/session activation proof.
    #[test]
    #[ignore = "owned LIVE lab clone only; produces SQL engine fixtures on F"]
    fn write_owned_native_sql_fixture() -> Result<()> {
        let chosen = std::env::var("IBCMD_RS_LIVE_FIXTURE_DB")
            .unwrap_or_else(|_| "ibcmd_rs_05_live_idle_20261001".into());
        ensure!(
            matches!(
                chosen.as_str(),
                "ibcmd_rs_05_live_idle_20261001" | "ibcmd_rs_05_live_sql17_20261001"
            ),
            "fixture accepts only this track's two owned disposable clones"
        );
        let database = chosen.as_str();
        let out = Path::new(if database.contains("sql17") {
            r"F:\ibcmd\lab\05\wave1\live\sql17"
        } else {
            r"F:\ibcmd\lab\05\wave1\live\sql-guard"
        });
        fs::create_dir_all(out)?;
        let sql = SqlExec::from_options(SqlOptions::integrated("localhost", None))?;
        let SqlBackend::Client(client) = sql.backend() else {
            unreachable!()
        };
        let mut artifact = fixture();
        artifact.identity = identity(client, database)?;
        artifact.storage_schema_sha256 =
            crate::mssql_platform_profile::verify_mssql_storage_profile(
                crate::mssql_platform_profile::MssqlNativePlatformProfile::Platform8_3_27_2214,
                client,
                database,
            )?
            .storage_schema_sha256;
        artifact.recovery.staged_rows = crate::mssql_dump::fetch_main_activation_rows(
            &sql,
            database,
            "Config",
            &std::collections::BTreeSet::from(["versions".to_owned()]),
        )?;
        artifact.recovery_token = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&artifact.recovery)?)
        );
        artifact.tail = out.join("cycle.trn").to_string_lossy().to_string();
        artifact.validate()?;
        fs::write(
            out.join("synthetic-artifact.json"),
            serde_json::to_vec_pretty(&artifact)?,
        )?;
        let db = database_ident(database)?;
        let full = quote_string(&out.join("full.bak").to_string_lossy());
        let tail = quote_string(&artifact.tail);
        fs::write(
            out.join("cycle1.sql"),
            format!(
                "USE [master]; IF EXISTS(SELECT 1 FROM sys.dm_os_file_exists(N'{tail}') WHERE file_exists=1) THROW 57500,'owned fixture output already exists',1; ALTER DATABASE {db} SET RECOVERY FULL; BACKUP DATABASE {db} TO DISK=N'{full}' WITH INIT,COMPRESSION,CHECKSUM; ALTER DATABASE {db} SET SINGLE_USER WITH ROLLBACK IMMEDIATE; BACKUP LOG {db} TO DISK=N'{tail}' WITH NORECOVERY,INIT,COMPRESSION,CHECKSUM,NAME=N'{}'; RESTORE DATABASE {db} WITH RECOVERY; ALTER DATABASE {db} SET MULTI_USER;",
                artifact.backup_name(1)
            ),
        )?;
        fs::write(
            out.join("validate.sql"),
            render_continue(&artifact, false, false)?,
        )?;
        fs::write(
            out.join("continue.sql"),
            render_continue(&artifact, false, true)?,
        )?;
        fs::write(
            out.join("guard.sql"),
            guard_pending_live(database, "THROW 57501,'target body reached',1;"),
        )?;
        fs::write(out.join("pending.sql"), pending_operation_query(database))?;
        println!(
            "SQL-only fixtures saved to {}; RAS/session proof is not measured",
            out.display()
        );
        Ok(())
    }

    #[test]
    fn snapshot_tampering_and_noop_artifacts_refuse_before_sql() {
        let mut artifact = fixture();
        artifact.validate().unwrap();
        artifact.recovery.staged_rows[0].binary_data[0] = 99;
        assert!(
            render_continue(&artifact, false, true)
                .unwrap_err()
                .to_string()
                .contains("token")
        );
        let mut artifact = fixture();
        artifact.recovery.new_generation = artifact.recovery.old_generation.clone();
        artifact.recovery_token = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&artifact.recovery).unwrap())
        );
        assert!(
            artifact
                .validate()
                .unwrap_err()
                .to_string()
                .contains("no-op")
        );
    }

    #[test]
    fn idle_inventory_refuses_every_nonempty_or_unrecognized_ras_response() {
        idle_session_inventory(b"\r\n \t").unwrap();
        for response in [
            "session : 123\napp-id : 1CV8\nlast-active-at : 2026-10-01T12:00:00",
            "ERROR",
            "app-id : RAS",
        ] {
            assert!(idle_session_inventory(response.as_bytes()).is_err());
        }
    }

    #[test]
    fn actual_oem_user_name_and_unknown_bytes_never_admit_cycle_two() {
        // Exact Администратор bytes from successful native RAC stdout on the
        // wave2 service83 fixture; decoding this CP866 row as UTF-8 fails.
        let oem = b"user-name : \x80\xA4\xAC\xA8\xAD\xA8\xE1\xE2\xE0\xA0\xE2\xAE\xE0\r\n".to_vec();
        assert!(std::str::from_utf8(&oem).is_err());
        for response in [oem.as_slice(), b"\0", b"\xFF", b"\xC2\xA0"] {
            assert!(
                idle_session_inventory(response)
                    .unwrap_err()
                    .to_string()
                    .contains("cycle 2 was not started")
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn failed_rac_or_stderr_cannot_become_an_empty_session_inventory() {
        // This tests the protocol, not pwsh startup speed on a busy Windows CI host.
        let call = |command| {
            rac_bounded_bytes(
                Path::new("pwsh"),
                &["-NoProfile", "-Command", command],
                Duration::from_secs(20),
            )
        };
        idle_session_inventory(&call("exit 0").unwrap()).unwrap();
        for command in [
            "exit 7",
            "[Console]::Error.Write('fixture failure'); exit 0",
        ] {
            assert!(
                call(command)
                    .unwrap_err()
                    .to_string()
                    .contains("RAS session readiness failed")
            );
        }
        assert!(
            rac_bounded(
                Path::new("pwsh"),
                &[
                    "-NoProfile",
                    "-Command",
                    "[Console]::OpenStandardOutput().WriteByte(128)"
                ],
                Duration::from_secs(20)
            )
            .unwrap_err()
            .to_string()
            .contains("not UTF-8")
        );
    }

    #[test]
    fn ras_binding_rejects_foreign_database_server_and_ambiguous_registrations() {
        let artifact = fixture();
        let good = format!(
            "infobase : {}\ndbms : MSSQLServer\ndb-name : lab]db\ndb-server : localhost\n",
            artifact.infobase_id
        );
        validate_ras_binding(&good, &artifact).unwrap();
        for bad in [
            good.replace("lab]db", "foreign"),
            good.replace("localhost", "foreign"),
            good.replace("MSSQLServer", "PostgreSQL"),
            good.replace("dbms : MSSQLServer\n", ""),
            format!("{good}\n{good}"),
        ] {
            assert!(validate_ras_binding(&bad, &artifact).is_err());
        }
    }

    #[test]
    fn continuation_guards_identity_headers_generation_and_chain_before_append() {
        let sql = render_continue(&fixture(), false, true).unwrap();
        let append = sql.find("BACKUP LOG [lab]]db]").unwrap();
        for guard in [
            "@LockOwner='Session'",
            "BindingID",
            "FamilyGUID",
            "RecoveryForkID",
            "BackupName",
            "ConfigSave",
            "HASHBYTES",
            "last_log_backup_lsn=h.LastLSN",
            "cycle 2 was not started",
        ] {
            assert!(sql.find(guard).unwrap() < append, "{guard}");
        }
        assert_eq!(sql.matches("BACKUP LOG [lab]]db]").count(), 1);
        assert!(!sql.contains("NORECOVERY, INIT"));
        assert!(sql.contains("IF @LiveAccessChanged=1"));
        assert!(sql.contains("a.LastLSN=b.FirstLSN"));
        assert!(sql.contains("tail''''s.trn"));
    }

    #[test]
    fn continuation_work_refusal_retains_committed_promotion_in_its_message() {
        let sql = render_continuation_sessions_check("lab", false);
        assert!(sql.contains("cycle 2 was not started; committed promotion/cycle 1 is retained"));
        assert!(!sql.contains("promotion is rolled back"));
        assert!(sql.contains("open_transaction_count>0"));
        assert!(render_continuation_sessions_check("lab", true).is_empty());
    }

    #[test]
    fn target_generation_refusal_releases_the_lock_in_its_owning_database() {
        for execute in [false, true] {
            let sql = render_continue(&fixture(), false, execute).unwrap();
            // A stale row throws while USE still names the target; app locks are
            // database-scoped, so target-context release would hide 57266 with 1223.
            let target = sql.find("USE [lab]]db];").unwrap();
            let refusal = sql.find("THROW 57266").unwrap();
            assert!(target < refusal);
            assert!(!sql[target..refusal].contains("USE [master]"));
            let catch = sql.split("END TRY BEGIN CATCH").nth(1).unwrap();
            let release = catch.find("EXEC sys.sp_releaseapplock").unwrap();
            let master = catch.find("USE [master];").unwrap();
            assert!(master < release);
            assert!(!catch[master..release].contains("ALTER DATABASE"));
            assert!(catch[release..].contains("THROW;"));
        }
    }

    #[test]
    fn pending_history_guard_precedes_target_writes_and_survives_foreign_backups() {
        let sql = guard_pending_live("lab]db", "USE [lab]]db]; DELETE FROM ConfigSave;");
        let write = sql.find("DELETE FROM ConfigSave").unwrap();
        assert!(sql.find("sp_getapplock").unwrap() < write);
        assert!(sql.find("IF EXISTS (SELECT TOP (1) a.name").unwrap() < write);
        assert!(sql.contains("b.name=LEFT(a.name,LEN(a.name)-1)+N'2'"));
        // Matching only the specific second-cycle name cannot let an unrelated later backup clear the pending operation.
        assert!(!sql.contains("MAX(backup_set_id)"));
        assert!(sql.contains("IF @@TRANCOUNT>0 ROLLBACK"));
    }

    struct Complete {
        calls: AtomicUsize,
        malformed: bool,
        post_append_failure: bool,
    }
    impl SqlClient for Complete {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("unvalidated script execution")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("unexpected write")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if self.post_append_failure {
                if call == 2 {
                    bail!("post-append verification transport failed");
                }
                assert_eq!(query.contains("BACKUP LOG"), call == 1);
                return each(SqlRow {
                    result_set: 0,
                    values: vec![SqlValue::Int(1), SqlValue::Int(i64::from(call == 1))],
                });
            }
            assert!(
                query.contains("#LiveHeader")
                    && query.contains("BindingID")
                    && query.contains("@LockOwner='Session'")
            );
            assert!(!query.contains("BACKUP LOG"));
            each(SqlRow {
                result_set: 0,
                values: vec![
                    SqlValue::Int(if self.malformed { 3 } else { 2 }),
                    SqlValue::Int(0),
                ],
            })
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            unreachable!()
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("unexpected rows")
        }
    }

    #[test]
    fn already_complete_uses_atomic_validated_state_without_rac_or_a_third_cycle() {
        let client = Complete {
            calls: AtomicUsize::new(0),
            malformed: false,
            post_append_failure: false,
        };
        for _ in 0..2 {
            let report = finish(
                &client,
                &fixture(),
                Path::new("saved.json"),
                Path::new("not-an-executable"),
                "",
                None,
                None,
                false,
            )
            .unwrap();
            assert_eq!(report.state, "already_complete");
            assert_eq!(report.cycle_2_executed, Some(false));
        }
        assert_eq!(client.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn invalid_state_never_reaches_rac_or_a_write() {
        let client = Complete {
            calls: AtomicUsize::new(0),
            malformed: true,
            post_append_failure: false,
        };
        assert!(
            finish(
                &client,
                &fixture(),
                Path::new("saved.json"),
                Path::new("not-an-executable"),
                "",
                None,
                None,
                false
            )
            .unwrap_err()
            .to_string()
            .contains("invalid")
        );
        assert_eq!(client.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn post_append_failure_reports_unknown_execution_and_never_asserts_false() {
        let client = Complete {
            calls: AtomicUsize::new(0),
            malformed: false,
            post_append_failure: true,
        };
        let mut checks = 0;
        let error =
            finish_with_readiness(&client, &fixture(), Path::new("saved.json"), false, || {
                checks += 1;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(checks, 2);
        assert_eq!(client.calls.load(Ordering::SeqCst), 3);
        assert!(error.to_string().contains("post-append verification"));
        let report = unresolved(Path::new("saved.json"), &error);
        assert_eq!(report.cycle_2_executed, None);
        assert!(serde_json::to_value(&report).unwrap()["cycle_2_executed"].is_null());
    }

    #[cfg(windows)]
    #[test]
    fn a_hung_rac_is_killed_at_the_readiness_deadline() {
        let started = Instant::now();
        let error = rac_bounded(
            Path::new("pwsh"),
            &["-NoProfile", "-Command", "Start-Sleep -Seconds 10"],
            Duration::from_millis(150),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
