//! Fail-closed main-configuration publication plans for MSSQL 8.3.27.
//!
//! This module only models and renders the row transition evidenced for the
//! `Config`/`ConfigSave`/`Params` storage boundary.  It deliberately does not
//! claim that an online publication invalidates already-running server caches.

use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read;
use uuid::Uuid;

const MAX_ROW_BYTES: usize = 16 * 1024 * 1024;
const MAX_PLAN_BYTES: usize = 32 * 1024 * 1024;
const MAX_ROWS: usize = 128;
const MAX_INFLATED_VERSIONS_BYTES: usize = 16 * 1024 * 1024;
const SERVICE_NAMES: [&str; 3] = ["root", "version", "versions"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainActivationMode {
    Exclusive,
    /// Publish while sessions remain connected. Existing sessions retain their
    /// loaded generation; sessions opened afterwards use the new generation.
    Online,
    /// Publish ordinary rows, then force a lossless SQL recovery cycle so the
    /// already-connected 1C session reloads the new generation.
    Live,
    /// Publish ordinary rows while sessions are connected. The caller must
    /// hand the dedicated 1C worker process off after the SQL commit.
    Worker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainStorageRow {
    pub file_name: String,
    pub part_no: i32,
    pub creation: String,
    pub modified: String,
    pub attributes: i32,
    pub data_size: u64,
    pub binary_data: Vec<u8>,
}

impl MainStorageRow {
    pub fn sha256(&self) -> [u8; 32] {
        Sha256::digest(&self.binary_data).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationSnapshot {
    /// Exact ordinary Config rows corresponding to every staged FileName.
    pub config_rows: Vec<MainStorageRow>,
    /// Existing Config.DynamicallyUpdated row, if present.
    pub config_dynamically_updated: Option<MainStorageRow>,
    /// Existing Params.DynamicallyUpdated row, if present.
    pub params_dynamically_updated: Option<MainStorageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationRecoverySnapshot {
    pub old_generation: String,
    pub new_generation: String,
    pub overwritten_config_rows: Vec<MainStorageRow>,
    pub prior_config_dynamically_updated: Option<MainStorageRow>,
    pub prior_params_dynamically_updated: Option<MainStorageRow>,
    pub staged_rows: Vec<MainStorageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivationDryRunReport {
    pub mode: MainActivationMode,
    pub old_generation: String,
    pub new_generation: String,
    pub changed_targets: Vec<String>,
    pub touched_tables: Vec<String>,
    pub staged_rows: usize,
    pub staged_bytes: usize,
    pub no_op: bool,
    pub online_protocol_verified: bool,
    pub existing_sessions_retain_generation: bool,
    pub live_session_switch_expected: bool,
    pub requires_tail_log_artifact: bool,
    pub recovery_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainActivationPlan {
    mode: MainActivationMode,
    ordinary_generation: Uuid,
    old_generation: Uuid,
    new_generation: Uuid,
    dynamic_history: Vec<Uuid>,
    staged_rows: Vec<MainStorageRow>,
    active_rows: Vec<MainStorageRow>,
    changed_targets: Vec<String>,
    config_marker: Option<MainStorageRow>,
    params_marker: Option<MainStorageRow>,
    no_op: bool,
    recovery: MainActivationRecoverySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainActivationScript {
    pub sql: String,
    pub report: MainActivationDryRunReport,
    pub recovery: MainActivationRecoverySnapshot,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MainActivationError {
    SafetyGate(String),
    InvalidRow(String),
    StructuralTarget(String),
    Versions(String),
    Limit(String),
}

impl std::fmt::Display for MainActivationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, detail) = match self {
            Self::SafetyGate(detail) => ("write safety gate rejected activation", detail),
            Self::InvalidRow(detail) => ("invalid activation row", detail),
            Self::StructuralTarget(detail) => ("unsupported or structural staged target", detail),
            Self::Versions(detail) => ("invalid versions blob", detail),
            Self::Limit(detail) => ("activation input exceeds a safety limit", detail),
        };
        write!(formatter, "{prefix}: {detail}")
    }
}

impl std::error::Error for MainActivationError {}

impl MainActivationPlan {
    pub fn mode(&self) -> MainActivationMode {
        self.mode
    }

    pub fn old_generation(&self) -> Uuid {
        self.old_generation
    }

    pub fn new_generation(&self) -> Uuid {
        self.new_generation
    }

    pub fn is_no_op(&self) -> bool {
        self.no_op
    }

    pub fn recovery(&self) -> &MainActivationRecoverySnapshot {
        &self.recovery
    }

    pub fn dry_run_report(&self) -> MainActivationDryRunReport {
        let recovery_json = serde_json::to_vec(&self.recovery)
            .expect("serializing a bounded recovery snapshot cannot fail");
        MainActivationDryRunReport {
            mode: self.mode,
            old_generation: self.old_generation.hyphenated().to_string(),
            new_generation: self.new_generation.hyphenated().to_string(),
            changed_targets: self.changed_targets.clone(),
            touched_tables: if self.no_op {
                vec!["ConfigSave"]
            } else {
                match self.mode {
                    MainActivationMode::Exclusive
                    | MainActivationMode::Live
                    | MainActivationMode::Worker => {
                        vec!["Config", "ConfigSave", "Params"]
                    }
                    MainActivationMode::Online => {
                        vec!["Config", "ConfigSave", "Params"]
                    }
                }
            }
            .into_iter()
            .map(str::to_owned)
            .collect(),
            staged_rows: self.staged_rows.len(),
            staged_bytes: self
                .staged_rows
                .iter()
                .map(|row| row.binary_data.len())
                .sum(),
            no_op: self.no_op,
            online_protocol_verified: self.mode == MainActivationMode::Online,
            existing_sessions_retain_generation: self.mode == MainActivationMode::Online,
            live_session_switch_expected: matches!(
                self.mode,
                MainActivationMode::Live | MainActivationMode::Worker
            ),
            requires_tail_log_artifact: self.mode == MainActivationMode::Live && !self.no_op,
            recovery_token: hex(&Sha256::digest(recovery_json)),
        }
    }
}

/// Builds an immutable plan only from an exact ConfigSave image and its
/// matching ordinary Config/Params snapshot.
pub fn prepare_main_activation(
    mode: MainActivationMode,
    staged_rows: Vec<MainStorageRow>,
    snapshot: MainActivationSnapshot,
    allowed_non_structural_targets: &[String],
    allow_non_lab: bool,
) -> Result<MainActivationPlan, MainActivationError> {
    if !allow_non_lab {
        return Err(MainActivationError::SafetyGate(
            "--allow-non-lab acknowledgement is required".to_owned(),
        ));
    }
    validate_rows("ConfigSave", &staged_rows, false)?;
    validate_rows("Config", &snapshot.config_rows, false)?;
    if staged_rows.len() > MAX_ROWS {
        return Err(MainActivationError::Limit(format!(
            "{} staged rows exceeds {MAX_ROWS}",
            staged_rows.len()
        )));
    }
    let staged_bytes = staged_rows.iter().try_fold(0usize, |total, row| {
        total
            .checked_add(row.binary_data.len())
            .ok_or_else(|| MainActivationError::Limit("staged byte count overflow".to_owned()))
    })?;
    if staged_bytes > MAX_PLAN_BYTES {
        return Err(MainActivationError::Limit(format!(
            "{staged_bytes} staged bytes exceeds {MAX_PLAN_BYTES}"
        )));
    }

    validate_optional_marker("Config", snapshot.config_dynamically_updated.as_ref())?;
    validate_optional_marker("Params", snapshot.params_dynamically_updated.as_ref())?;

    let staged = index_rows(&staged_rows)?;
    let active = index_rows(&snapshot.config_rows)?;
    for service in SERVICE_NAMES {
        require_single_part(&staged, service, "ConfigSave")?;
        require_single_part(&active, service, "Config")?;
    }
    if staged.len() != active.len() || staged.keys().ne(active.keys()) {
        return Err(MainActivationError::InvalidRow(
            "Config snapshot must contain exactly the ordinary rows named by ConfigSave".to_owned(),
        ));
    }

    let allowed = allowed_non_structural_targets
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut changed_targets = Vec::new();
    for row in &staged_rows {
        if SERVICE_NAMES.contains(&row.file_name.as_str()) {
            continue;
        }
        if !is_existing_body_target(&row.file_name) || !allowed.contains(row.file_name.as_str()) {
            return Err(MainActivationError::StructuralTarget(row.file_name.clone()));
        }
        changed_targets.push(row.file_name.clone());
    }
    changed_targets.sort();
    changed_targets.dedup();
    if changed_targets.is_empty() {
        return Err(MainActivationError::StructuralTarget(
            "the stage contains no admitted existing module/form body rows".to_owned(),
        ));
    }

    let ordinary_generation =
        generation_from_versions(&active[&("versions".to_owned(), 0)].binary_data)?;
    let (old_generation, dynamic_history) = active_generation_from_markers(
        ordinary_generation,
        snapshot.config_dynamically_updated.as_ref(),
        snapshot.params_dynamically_updated.as_ref(),
    )?;
    let new_generation =
        generation_from_versions(&staged[&("versions".to_owned(), 0)].binary_data)?;
    let no_op = staged.iter().all(|(key, row)| {
        active
            .get(key)
            .is_some_and(|current| current.binary_data == row.binary_data)
    });
    if !no_op && old_generation == new_generation {
        return Err(MainActivationError::Versions(
            "changed payload reuses the active generation UUID".to_owned(),
        ));
    }

    let recovery = MainActivationRecoverySnapshot {
        old_generation: old_generation.hyphenated().to_string(),
        new_generation: new_generation.hyphenated().to_string(),
        overwritten_config_rows: snapshot.config_rows.clone(),
        prior_config_dynamically_updated: snapshot.config_dynamically_updated.clone(),
        prior_params_dynamically_updated: snapshot.params_dynamically_updated.clone(),
        staged_rows: staged_rows.clone(),
    };
    Ok(MainActivationPlan {
        mode,
        ordinary_generation,
        old_generation,
        new_generation,
        dynamic_history,
        staged_rows,
        active_rows: snapshot.config_rows,
        changed_targets,
        config_marker: snapshot.config_dynamically_updated,
        params_marker: snapshot.params_dynamically_updated,
        no_op,
        recovery,
    })
}

pub fn render_main_activation_sql(
    database: &str,
    plan: &MainActivationPlan,
    tail_log_output: Option<&str>,
) -> Result<MainActivationScript, MainActivationError> {
    let database_name = database;
    let database_literal = quote_string(database_name);
    let database = quote_ident(database_name)?;
    let live_tail = match (plan.mode, plan.no_op, tail_log_output) {
        (MainActivationMode::Live, false, Some(path)) => Some(validate_tail_log_output(path)?),
        (MainActivationMode::Live, false, None) => {
            return Err(MainActivationError::SafetyGate(
                "--tail-log-output is required for live activation".to_owned(),
            ));
        }
        (MainActivationMode::Live, true, _) => None,
        (_, _, Some(_)) => {
            return Err(MainActivationError::SafetyGate(
                "--tail-log-output is only valid for live activation".to_owned(),
            ));
        }
        (_, _, None) => None,
    };
    let mut sql = String::new();
    writeln!(sql, "SET NOCOUNT ON;").unwrap();
    writeln!(sql, "SET XACT_ABORT ON;").unwrap();
    if let Some(tail) = live_tail.as_deref() {
        writeln!(sql, "USE [master];").unwrap();
        writeln!(sql, "IF DB_ID(N'{database_literal}') IS NULL THROW 57230, 'live activation database does not exist', 1;").unwrap();
        writeln!(sql, "IF (SELECT recovery_model FROM sys.databases WHERE name=N'{database_literal}') NOT IN (1,2) THROW 57231, 'live activation requires FULL or BULK_LOGGED recovery', 1;").unwrap();
        writeln!(sql, "IF (SELECT state FROM sys.databases WHERE name=N'{database_literal}') <> 0 THROW 57232, 'live activation requires an ONLINE database', 1;").unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM sys.dm_os_file_exists(N'{}') WHERE file_exists=1 OR file_is_a_directory=1) THROW 57233, 'tail-log output already exists or is a directory', 1;", quote_string(tail)).unwrap();
    }
    writeln!(sql, "USE {database};").unwrap();
    writeln!(sql, "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    writeln!(sql, "BEGIN TRANSACTION;").unwrap();
    writeln!(sql, "DECLARE @LockResult int;").unwrap();
    writeln!(sql, "EXEC @LockResult = sys.sp_getapplock @Resource=N'ibcmd-rs:main-activation', @LockMode='Exclusive', @LockOwner='Transaction', @LockTimeout=0;").unwrap();
    writeln!(
        sql,
        "IF @LockResult < 0 THROW 57200, 'main activation application lock is busy', 1;"
    )
    .unwrap();

    render_expected_table(&mut sql, "ExpectedStage", &plan.staged_rows);
    render_expected_table(&mut sql, "ExpectedActive", &plan.active_rows);
    render_exact_set_assertion(&mut sql, "ConfigSave", "ExpectedStage", 57201);
    render_selected_assertion(&mut sql, "Config", "ExpectedActive", 57202);
    render_marker_assertion(&mut sql, "Config", plan.config_marker.as_ref(), 57203);
    render_marker_assertion(&mut sql, "Params", plan.params_marker.as_ref(), 57204);

    if plan.no_op {
        writeln!(sql, "DELETE FROM dbo.ConfigSave;").unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> {} THROW 57205, 'ConfigSave cleanup drifted', 1;",
            plan.staged_rows.len()
        )
        .unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigSave) THROW 57206, 'ConfigSave postcondition failed', 1;").unwrap();
        writeln!(sql, "COMMIT TRANSACTION;").unwrap();
        render_catch(&mut sql, None);
        return Ok(MainActivationScript {
            sql,
            report: plan.dry_run_report(),
            recovery: plan.recovery.clone(),
        });
    }

    match plan.mode {
        MainActivationMode::Exclusive => render_ordinary_transition(&mut sql, plan, true),
        MainActivationMode::Online => render_online_transition(&mut sql, plan),
        MainActivationMode::Live | MainActivationMode::Worker => {
            render_ordinary_transition(&mut sql, plan, false)
        }
    }
    writeln!(sql, "DELETE FROM dbo.ConfigSave;").unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> {} THROW 57220, 'ConfigSave cleanup drifted', 1;",
        plan.staged_rows.len()
    )
    .unwrap();
    render_postconditions(&mut sql, plan);
    writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigSave) THROW 57221, 'ConfigSave postcondition failed', 1;").unwrap();
    writeln!(sql, "COMMIT TRANSACTION;").unwrap();
    render_catch(&mut sql, None);
    if let Some(tail) = live_tail.as_deref() {
        render_live_recovery(&mut sql, &database, &database_literal, tail);
    }

    if sql.len() > MAX_PLAN_BYTES {
        return Err(MainActivationError::Limit(format!(
            "rendered SQL uses {} bytes; maximum is {MAX_PLAN_BYTES}",
            sql.len()
        )));
    }
    Ok(MainActivationScript {
        sql,
        report: plan.dry_run_report(),
        recovery: plan.recovery.clone(),
    })
}

fn render_ordinary_transition(
    sql: &mut String,
    plan: &MainActivationPlan,
    require_no_sessions: bool,
) {
    if require_no_sessions {
        writeln!(sql, "IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process=1 AND session_id<>@@SPID AND database_id=DB_ID()) THROW 57209, 'exclusive activation requires no other database sessions', 1;").unwrap();
    }
    for row in &plan.staged_rows {
        let name = quote_string(&row.file_name);
        writeln!(
            sql,
            "DELETE FROM dbo.Config WHERE FileName=N'{name}' AND PartNo={};",
            row.part_no
        )
        .unwrap();
        writeln!(sql, "INSERT dbo.Config (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo FROM dbo.ConfigSave WHERE FileName=N'{name}' AND PartNo={};", row.part_no).unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> 1 THROW 57210, 'exclusive promotion source drifted', 1;"
        )
        .unwrap();
    }
    render_marker_delete(sql, "Config", plan.config_marker.is_some(), 57215);
    render_marker_delete(sql, "Params", plan.params_marker.is_some(), 57216);
}

fn render_online_transition(sql: &mut String, plan: &MainActivationPlan) {
    let generation = plan.new_generation.hyphenated();
    for row in &plan.staged_rows {
        let source = quote_string(&row.file_name);
        let destination = match row.file_name.as_str() {
            "root" | "version" => row.file_name.clone(),
            "versions" => format!("versions_dynupdate_{generation}"),
            _ => dynamic_alias(&row.file_name, &generation.to_string()),
        };
        let destination = quote_string(&destination);
        if matches!(row.file_name.as_str(), "root" | "version") {
            writeln!(
                sql,
                "DELETE FROM dbo.Config WHERE FileName=N'{destination}' AND PartNo={};",
                row.part_no
            )
            .unwrap();
        } else {
            writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Config WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{destination}' AND PartNo={}) THROW 57211, 'dynamic alias already exists', 1;", row.part_no).unwrap();
        }
        writeln!(sql, "INSERT dbo.Config (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) SELECT N'{destination}',Creation,Modified,Attributes,DataSize,BinaryData,PartNo FROM dbo.ConfigSave WHERE FileName=N'{source}' AND PartNo={};", row.part_no).unwrap();
        writeln!(
            sql,
            "IF @@ROWCOUNT <> 1 THROW 57212, 'online promotion source drifted', 1;"
        )
        .unwrap();
    }
    let (config_payload, params_payload) = next_dynamic_marker_payloads(plan);
    render_marker_upsert(sql, "Config", &config_payload, 57213);
    render_marker_upsert(sql, "Params", &params_payload, 57214);
}

fn render_expected_table(sql: &mut String, variable: &str, rows: &[MainStorageRow]) {
    writeln!(sql, "DECLARE @{variable} TABLE (FileName nvarchar(4000) NOT NULL, PartNo int NOT NULL, DataSize bigint NOT NULL, Digest varbinary(32) NOT NULL, PRIMARY KEY(FileName,PartNo));").unwrap();
    for row in rows {
        writeln!(
            sql,
            "INSERT @{variable} VALUES (N'{}',{}, {},0x{});",
            quote_string(&row.file_name),
            row.part_no,
            row.data_size,
            hex(&row.sha256())
        )
        .unwrap();
    }
}

fn render_exact_set_assertion(sql: &mut String, table: &str, expected: &str, code: u32) {
    writeln!(sql, "IF EXISTS (SELECT FileName,PartNo,CONVERT(bigint,DataSize),HASHBYTES('SHA2_256',BinaryData) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) EXCEPT SELECT FileName,PartNo,DataSize,Digest FROM @{expected}) OR EXISTS (SELECT FileName,PartNo,DataSize,Digest FROM @{expected} EXCEPT SELECT FileName,PartNo,CONVERT(bigint,DataSize),HASHBYTES('SHA2_256',BinaryData) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK)) THROW {code}, '{table} exact snapshot drifted', 1;").unwrap();
}

fn render_selected_assertion(sql: &mut String, table: &str, expected: &str, code: u32) {
    writeln!(sql, "IF EXISTS (SELECT E.FileName,E.PartNo FROM @{expected} E LEFT JOIN dbo.{table} T WITH (UPDLOCK,HOLDLOCK) ON T.FileName=E.FileName AND T.PartNo=E.PartNo AND CONVERT(bigint,T.DataSize)=E.DataSize AND HASHBYTES('SHA2_256',T.BinaryData)=E.Digest WHERE T.FileName IS NULL) THROW {code}, '{table} selected snapshot drifted', 1;").unwrap();
}

fn render_marker_assertion(
    sql: &mut String,
    table: &str,
    marker: Option<&MainStorageRow>,
    code: u32,
) {
    match marker {
        Some(row) => writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated') <> 1 OR (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW {code}, '{table}.DynamicallyUpdated drifted', 1;", row.data_size, hex(&row.sha256())).unwrap(),
        None => writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.{table} WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'DynamicallyUpdated') THROW {code}, 'unexpected {table}.DynamicallyUpdated row', 1;").unwrap(),
    }
}

fn render_marker_upsert(sql: &mut String, table: &str, payload: &[u8], code: u32) {
    let data = hex(payload);
    writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.{table} WHERE FileName=N'DynamicallyUpdated' AND PartNo=0) UPDATE dbo.{table} SET Modified=SYSUTCDATETIME(),DataSize={},BinaryData=0x{} WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 ELSE INSERT dbo.{table} (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) VALUES (N'DynamicallyUpdated',SYSUTCDATETIME(),SYSUTCDATETIME(),0,{},0x{},0);", payload.len(), data, payload.len(), data).unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> 1 THROW {code}, '{table}.DynamicallyUpdated upsert failed', 1;"
    )
    .unwrap();
}

fn render_marker_delete(sql: &mut String, table: &str, expected: bool, code: u32) {
    writeln!(
        sql,
        "DELETE FROM dbo.{table} WHERE FileName=N'DynamicallyUpdated';"
    )
    .unwrap();
    writeln!(
        sql,
        "IF @@ROWCOUNT <> {} THROW {code}, '{table}.DynamicallyUpdated cleanup drifted', 1;",
        if expected { 1 } else { 0 }
    )
    .unwrap();
}

fn render_postconditions(sql: &mut String, plan: &MainActivationPlan) {
    let generation = plan.new_generation.hyphenated().to_string();
    let published = plan
        .staged_rows
        .iter()
        .cloned()
        .map(|mut row| {
            if plan.mode == MainActivationMode::Online {
                row.file_name = match row.file_name.as_str() {
                    "root" | "version" => row.file_name,
                    "versions" => format!("versions_dynupdate_{generation}"),
                    _ => dynamic_alias(&row.file_name, &generation),
                };
            }
            row
        })
        .collect::<Vec<_>>();
    render_expected_table(sql, "ExpectedPublished", &published);
    render_selected_assertion(sql, "Config", "ExpectedPublished", 57222);
    if plan.mode == MainActivationMode::Online {
        let (config_payload, params_payload) = next_dynamic_marker_payloads(plan);
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.Config WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW 57223, 'Config.DynamicallyUpdated postcondition failed', 1;", config_payload.len(), hex(&Sha256::digest(&config_payload))).unwrap();
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName=N'DynamicallyUpdated' AND PartNo=0 AND CONVERT(bigint,DataSize)={} AND HASHBYTES('SHA2_256',BinaryData)=0x{}) <> 1 THROW 57224, 'Params.DynamicallyUpdated postcondition failed', 1;", params_payload.len(), hex(&Sha256::digest(&params_payload))).unwrap();
    } else {
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName=N'DynamicallyUpdated') THROW 57225, 'Config.DynamicallyUpdated cleanup postcondition failed', 1;").unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Params WHERE FileName=N'DynamicallyUpdated') THROW 57226, 'Params.DynamicallyUpdated cleanup postcondition failed', 1;").unwrap();
    }
}

fn render_catch(sql: &mut String, live_database: Option<(&str, &str)>) {
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF XACT_STATE() <> 0 ROLLBACK TRANSACTION;").unwrap();
    if let Some((database, database_name)) = live_database {
        writeln!(sql, "USE [master];").unwrap();
        writeln!(sql, "IF DB_ID(N'{}') IS NOT NULL AND DATABASEPROPERTYEX(N'{}','Status') <> N'RESTORING' ALTER DATABASE {database} SET MULTI_USER;", quote_string(database_name), quote_string(database_name)).unwrap();
    }
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
}

fn render_live_recovery(
    sql: &mut String,
    database: &str,
    database_literal: &str,
    tail_log_output: &str,
) {
    let tail = quote_string(tail_log_output);
    writeln!(sql, "DECLARE @LiveExpected1cConnections int=(SELECT COUNT(*) FROM sys.dm_exec_sessions WHERE is_user_process=1 AND database_id=DB_ID(N'{database_literal}') AND program_name=N'1CV83 Server');").unwrap();
    writeln!(sql, "CHECKPOINT;").unwrap();
    writeln!(sql, "USE [master];").unwrap();
    writeln!(sql, "BEGIN TRY").unwrap();
    // The first recovery makes rphost observe the newly committed ordinary
    // generation. The second recovery advances already open 1C sessions to
    // that prepared generation. Both log backup sets are retained in one
    // operator-owned artifact.
    writeln!(
        sql,
        "ALTER DATABASE {database} SET SINGLE_USER WITH ROLLBACK IMMEDIATE;"
    )
    .unwrap();
    writeln!(
        sql,
        "BACKUP LOG {database} TO DISK=N'{tail}' WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM;"
    )
    .unwrap();
    writeln!(sql, "RESTORE DATABASE {database} WITH RECOVERY;").unwrap();
    writeln!(sql, "ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "DECLARE @LiveReconnectDeadline datetime2(3)=DATEADD(millisecond,4000,SYSUTCDATETIME()), @LiveStableSince datetime2(3)=NULL, @LiveObserved1cConnections int=0;").unwrap();
    writeln!(sql, "WHILE @LiveExpected1cConnections > 0 AND SYSUTCDATETIME() < @LiveReconnectDeadline BEGIN SELECT @LiveObserved1cConnections=COUNT(*) FROM sys.dm_exec_sessions WHERE is_user_process=1 AND database_id=DB_ID(N'{database_literal}') AND program_name=N'1CV83 Server'; IF @LiveObserved1cConnections >= @LiveExpected1cConnections BEGIN IF @LiveStableSince IS NULL SET @LiveStableSince=SYSUTCDATETIME(); IF DATEDIFF(millisecond,@LiveStableSince,SYSUTCDATETIME()) >= 500 BREAK; END ELSE SET @LiveStableSince=NULL; WAITFOR DELAY '00:00:00.100'; END;").unwrap();
    writeln!(sql, "IF @LiveExpected1cConnections > 0 AND (@LiveObserved1cConnections < @LiveExpected1cConnections OR @LiveStableSince IS NULL OR DATEDIFF(millisecond,@LiveStableSince,SYSUTCDATETIME()) < 500) THROW 57234, '1C SQL connections did not recover before the live activation deadline; database is online and the second recovery was not started', 1;").unwrap();
    writeln!(
        sql,
        "ALTER DATABASE {database} SET SINGLE_USER WITH ROLLBACK IMMEDIATE;"
    )
    .unwrap();
    writeln!(
        sql,
        "BACKUP LOG {database} TO DISK=N'{tail}' WITH NORECOVERY, NOINIT, COMPRESSION, CHECKSUM;"
    )
    .unwrap();
    writeln!(sql, "RESTORE DATABASE {database} WITH RECOVERY;").unwrap();
    writeln!(sql, "ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "END TRY").unwrap();
    writeln!(sql, "BEGIN CATCH").unwrap();
    writeln!(sql, "IF DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') <> N'RESTORING' ALTER DATABASE {database} SET MULTI_USER;").unwrap();
    writeln!(sql, "IF DB_ID(N'{database_literal}') IS NOT NULL AND DATABASEPROPERTYEX(N'{database_literal}','Status') = N'RESTORING' BEGIN DECLARE @LiveRecoveryMessage nvarchar(2048)=N'live activation left database restoring; run RESTORE DATABASE {database} WITH RECOVERY; original error: '+ERROR_MESSAGE(); THROW 57250,@LiveRecoveryMessage,1; END;").unwrap();
    writeln!(sql, "THROW;").unwrap();
    writeln!(sql, "END CATCH;").unwrap();
}

fn validate_tail_log_output(path: &str) -> Result<String, MainActivationError> {
    if path.is_empty()
        || path.encode_utf16().count() > 2048
        || path.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        return Err(MainActivationError::SafetyGate(
            "tail-log output path is empty, contains control characters, or exceeds 2048 UTF-16 code units"
                .to_owned(),
        ));
    }
    Ok(path.to_owned())
}

fn validate_rows(
    table: &str,
    rows: &[MainStorageRow],
    allow_empty: bool,
) -> Result<(), MainActivationError> {
    if !allow_empty && rows.is_empty() {
        return Err(MainActivationError::InvalidRow(format!(
            "{table} snapshot is empty"
        )));
    }
    for row in rows {
        if row.file_name.is_empty()
            || row.file_name.encode_utf16().count() > 128
            || row
                .file_name
                .chars()
                .any(|ch| ch == '\0' || ch.is_control())
        {
            return Err(MainActivationError::InvalidRow(format!(
                "{table} has an invalid FileName"
            )));
        }
        if row.part_no != 0 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table}.{} uses unsupported PartNo {}",
                row.file_name, row.part_no
            )));
        }
        if row.binary_data.len() > MAX_ROW_BYTES || row.data_size != row.binary_data.len() as u64 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table}.{} DataSize/binary length is invalid or exceeds {MAX_ROW_BYTES}",
                row.file_name
            )));
        }
    }
    index_rows(rows)?;
    Ok(())
}

fn validate_optional_marker(
    table: &str,
    marker: Option<&MainStorageRow>,
) -> Result<(), MainActivationError> {
    if let Some(row) = marker {
        validate_rows(table, std::slice::from_ref(row), false)?;
        if row.file_name != "DynamicallyUpdated" || row.part_no != 0 {
            return Err(MainActivationError::InvalidRow(format!(
                "{table} marker is not DynamicallyUpdated part 0"
            )));
        }
    }
    Ok(())
}

fn index_rows(
    rows: &[MainStorageRow],
) -> Result<BTreeMap<(String, i32), &MainStorageRow>, MainActivationError> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        if indexed
            .insert((row.file_name.clone(), row.part_no), row)
            .is_some()
        {
            return Err(MainActivationError::InvalidRow(format!(
                "duplicate row {} part {}",
                row.file_name, row.part_no
            )));
        }
    }
    Ok(indexed)
}

fn require_single_part(
    rows: &BTreeMap<(String, i32), &MainStorageRow>,
    name: &str,
    table: &str,
) -> Result<(), MainActivationError> {
    if !rows.contains_key(&(name.to_owned(), 0)) {
        return Err(MainActivationError::InvalidRow(format!(
            "{table} is missing {name} part 0"
        )));
    }
    Ok(())
}

fn is_existing_body_target(name: &str) -> bool {
    let (uuid, suffix) = if name.len() == 36 {
        (name, "")
    } else if name.len() > 37 && name.as_bytes().get(36) == Some(&b'.') {
        (&name[..36], &name[37..])
    } else {
        return false;
    };
    Uuid::parse_str(uuid).is_ok()
        && (suffix.is_empty() || suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn generation_from_versions(blob: &[u8]) -> Result<Uuid, MainActivationError> {
    let mut decoder = DeflateDecoder::new(blob).take((MAX_INFLATED_VERSIONS_BYTES + 1) as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(|error| MainActivationError::Versions(format!("raw deflate failed: {error}")))?;
    if plain.len() > MAX_INFLATED_VERSIONS_BYTES {
        return Err(MainActivationError::Limit(format!(
            "inflated versions exceeds {MAX_INFLATED_VERSIONS_BYTES} bytes"
        )));
    }
    let text = std::str::from_utf8(plain.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&plain))
        .map_err(|_| MainActivationError::Versions("header is not UTF-8".to_owned()))?;
    let mut fields = text
        .strip_prefix('{')
        .ok_or_else(|| MainActivationError::Versions("header does not start with '{'".to_owned()))?
        .splitn(5, ',');
    if fields.next().map(str::trim) != Some("1") {
        return Err(MainActivationError::Versions(
            "unsupported versions header tag".to_owned(),
        ));
    }
    let count = fields
        .next()
        .ok_or_else(|| MainActivationError::Versions("missing row count".to_owned()))?
        .trim();
    if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MainActivationError::Versions(
            "invalid versions row count".to_owned(),
        ));
    }
    if fields.next().map(str::trim) != Some("\"\"") {
        return Err(MainActivationError::Versions(
            "unsupported versions header shape".to_owned(),
        ));
    }
    let generation = Uuid::parse_str(
        fields
            .next()
            .ok_or_else(|| MainActivationError::Versions("missing generation UUID".to_owned()))?
            .trim(),
    )
    .map_err(|_| MainActivationError::Versions("invalid generation UUID".to_owned()))?;
    if fields.next().is_none() {
        return Err(MainActivationError::Versions(
            "versions header has no mapping payload".to_owned(),
        ));
    }
    Ok(generation)
}

fn active_generation_from_markers(
    ordinary: Uuid,
    config_marker: Option<&MainStorageRow>,
    params_marker: Option<&MainStorageRow>,
) -> Result<(Uuid, Vec<Uuid>), MainActivationError> {
    match (config_marker, params_marker) {
        (None, None) => Ok((ordinary, Vec::new())),
        (Some(config), Some(params)) => {
            let config = marker_fields(&config.binary_data)?;
            let params = marker_fields(&params.binary_data)?;
            let config_count = marker_count(&config, "Config")?;
            let params_count = marker_count(&params, "Params")?;
            if config[0] != "1" || config_count == 0 || config.len() != config_count + 2 {
                return Err(MainActivationError::Versions(
                    "unsupported Config.DynamicallyUpdated marker".to_owned(),
                ));
            }
            if params[0] != "0"
                || params_count != config_count + 1
                || params.len() != params_count + 2
            {
                return Err(MainActivationError::Versions(
                    "unsupported Params.DynamicallyUpdated marker".to_owned(),
                ));
            }
            if config_count > 4096 {
                return Err(MainActivationError::Limit(
                    "dynamic generation history exceeds 4096 entries".to_owned(),
                ));
            }
            let params_ordinary = Uuid::parse_str(params[2]).map_err(|_| {
                MainActivationError::Versions(
                    "Params.DynamicallyUpdated has an invalid ordinary generation".to_owned(),
                )
            })?;
            if params_ordinary != ordinary {
                return Err(MainActivationError::Versions(
                    "Params dynamic ordinary generation disagrees with versions".to_owned(),
                ));
            }
            let history = config[2..]
                .iter()
                .map(|value| {
                    Uuid::parse_str(value).map_err(|_| {
                        MainActivationError::Versions(
                            "Config.DynamicallyUpdated has an invalid generation".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let params_history = params[3..]
                .iter()
                .map(|value| {
                    Uuid::parse_str(value).map_err(|_| {
                        MainActivationError::Versions(
                            "Params.DynamicallyUpdated has an invalid generation".to_owned(),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if history != params_history {
                return Err(MainActivationError::Versions(
                    "Config/Params dynamic generation histories disagree".to_owned(),
                ));
            }
            Ok((*history.last().expect("non-empty history"), history))
        }
        _ => Err(MainActivationError::Versions(
            "Config and Params dynamic markers must both be present or both absent".to_owned(),
        )),
    }
}

fn marker_count(fields: &[&str], table: &str) -> Result<usize, MainActivationError> {
    fields
        .get(1)
        .ok_or_else(|| MainActivationError::Versions(format!("{table} marker has no count")))?
        .parse::<usize>()
        .map_err(|_| MainActivationError::Versions(format!("{table} marker count is invalid")))
}

fn next_dynamic_marker_payloads(plan: &MainActivationPlan) -> (Vec<u8>, Vec<u8>) {
    let mut history = plan.dynamic_history.clone();
    history.push(plan.new_generation);
    let generations = history
        .iter()
        .map(|value| value.hyphenated().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let config = utf8_bom(&format!("{{1,{},{generations}}}", history.len()));
    let params = utf8_bom(&format!(
        "{{0,{},{},{generations}}}",
        history.len() + 1,
        plan.ordinary_generation.hyphenated()
    ));
    (config, params)
}

fn marker_fields(blob: &[u8]) -> Result<Vec<&str>, MainActivationError> {
    let text = std::str::from_utf8(blob.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(blob))
        .map_err(|_| MainActivationError::Versions("dynamic marker is not UTF-8".to_owned()))?;
    let inner = text
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| MainActivationError::Versions("dynamic marker is not braced".to_owned()))?;
    Ok(inner.split(',').map(str::trim).collect())
}

fn dynamic_alias(name: &str, generation: &str) -> String {
    match name.split_once('.') {
        Some((base, suffix)) => format!("{base}_dynupdate_{generation}.{suffix}"),
        None => format!("{name}_dynupdate_{generation}"),
    }
}

fn utf8_bom(value: &str) -> Vec<u8> {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(value.as_bytes());
    bytes
}

fn quote_ident(value: &str) -> Result<String, MainActivationError> {
    if value.is_empty()
        || value.encode_utf16().count() > 128
        || value.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        return Err(MainActivationError::InvalidRow(
            "invalid database identifier".to_owned(),
        ));
    }
    Ok(format!("[{}]", value.replace(']', "]]")))
}

fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02X}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write;

    const OLD: &str = "719baa18-69ed-439a-8962-1de53d98e05e";
    const NEW: &str = "d66867e4-febb-4c6e-9ef1-230dac3e25fa";
    const BODY: &str = "b27aebc8-f190-4658-a81d-fd1406905f39";

    fn deflate(text: &str) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn row(name: &str, data: Vec<u8>) -> MainStorageRow {
        MainStorageRow {
            file_name: name.to_owned(),
            part_no: 0,
            creation: "2026-08-29T00:00:00Z".to_owned(),
            modified: "2026-08-29T00:00:00Z".to_owned(),
            attributes: 0,
            data_size: data.len() as u64,
            binary_data: data,
        }
    }

    fn versions(generation: &str) -> Vec<u8> {
        deflate(&format!("{{1,4,\"\",{generation},\"root\",x}}"))
    }

    fn fixture(mode: MainActivationMode) -> MainActivationPlan {
        let staged = vec![
            row(BODY, b"new descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"new body".to_vec()),
            row("root", b"new root".to_vec()),
            row("version", b"new version".to_vec()),
            row("versions", versions(NEW)),
        ];
        let active = vec![
            row(BODY, b"old descriptor".to_vec()),
            row(&format!("{BODY}.0"), b"old body".to_vec()),
            row("root", b"old root".to_vec()),
            row("version", b"old version".to_vec()),
            row("versions", versions(OLD)),
        ];
        prepare_main_activation(
            mode,
            staged,
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap()
    }

    #[test]
    fn derives_generations_from_raw_deflated_versions_header() {
        let plan = fixture(MainActivationMode::Exclusive);
        assert_eq!(plan.old_generation().to_string(), OLD);
        assert_eq!(plan.new_generation().to_string(), NEW);
    }

    #[test]
    fn exclusive_replaces_only_exact_staged_ordinary_rows() {
        let script =
            render_main_activation_sql("lab]db", &fixture(MainActivationMode::Exclusive), None)
                .unwrap();
        assert!(script.sql.contains("USE [lab]]db]"));
        assert!(
            script
                .sql
                .contains("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        );
        assert!(script.sql.contains("sp_getapplock"));
        assert!(
            script
                .sql
                .contains("exclusive activation requires no other database sessions")
        );
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Config WHERE FileName=N'root'")
        );
        assert!(!script.sql.contains("_dynupdate_"));
        assert!(!script.sql.contains("Files.MobileVersions"));
        assert!(!script.sql.contains("_ConfigChngR"));
        assert!(!script.sql.contains(".ui"));
    }

    #[test]
    fn online_preserves_ordinary_body_and_creates_evidenced_aliases() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Online), None).unwrap();
        assert!(script.sql.contains(&format!("{BODY}_dynupdate_{NEW}")));
        assert!(script.sql.contains(&format!("{BODY}_dynupdate_{NEW}.0")));
        assert!(script.sql.contains(&format!("versions_dynupdate_{NEW}")));
        assert!(
            !script
                .sql
                .contains(&format!("DELETE FROM dbo.Config WHERE FileName=N'{BODY}'"))
        );
        assert!(script.sql.contains("EFBBBF7B312C312C"));
        assert!(script.sql.contains("EFBBBF7B302C322C"));
        assert!(script.report.online_protocol_verified);
        assert!(script.report.existing_sessions_retain_generation);
    }

    #[test]
    fn live_promotes_ordinary_rows_then_runs_guarded_tail_recovery() {
        let script = render_main_activation_sql(
            "lab]db",
            &fixture(MainActivationMode::Live),
            Some(r"C:\tail's\generation.trn"),
        )
        .unwrap();
        let promotion = script
            .sql
            .find("DELETE FROM dbo.Config WHERE FileName=N'root'")
            .unwrap();
        let single_users = script
            .sql
            .match_indices("SET SINGLE_USER")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let backups = script
            .sql
            .match_indices("BACKUP LOG [lab]]db]")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let recoveries = script
            .sql
            .match_indices("RESTORE DATABASE [lab]]db] WITH RECOVERY")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(single_users.len(), 2);
        assert_eq!(backups.len(), 2);
        assert_eq!(recoveries.len(), 3);
        assert!(promotion < single_users[0]);
        assert!(single_users[0] < backups[0] && backups[0] < recoveries[0]);
        assert!(recoveries[0] < single_users[1]);
        assert!(single_users[1] < backups[1] && backups[1] < recoveries[1]);
        assert!(script.sql.contains("program_name=N'1CV83 Server'"));
        assert!(script.sql.contains("DATEADD(millisecond,4000"));
        assert!(script.sql.contains("WAITFOR DELAY '00:00:00.100'"));
        assert!(
            script
                .sql
                .contains("did not recover before the live activation deadline")
        );
        assert!(!script.sql.contains("WAITFOR DELAY '00:00:05'"));
        assert!(script.sql.contains("sys.dm_os_file_exists"));
        assert!(script.sql.contains("file_is_a_directory=1"));
        assert!(script.sql.contains("C:\\tail''s\\generation.trn"));
        assert!(
            script
                .sql
                .contains("WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM")
        );
        assert!(
            script
                .sql
                .contains("WITH NORECOVERY, NOINIT, COMPRESSION, CHECKSUM")
        );
        assert!(!script.sql.contains("COPY_ONLY"));
        assert!(script.sql.contains("SET MULTI_USER"));
        assert!(
            script
                .sql
                .contains("live activation left database restoring")
        );
        assert!(script.report.live_session_switch_expected);
        assert!(script.report.requires_tail_log_artifact);
        assert!(!script.report.existing_sessions_retain_generation);
        assert_eq!(
            script.report.touched_tables,
            ["Config", "ConfigSave", "Params"]
        );
    }

    #[test]
    fn live_requires_a_bounded_tail_path_and_other_modes_reject_it() {
        assert!(matches!(
            render_main_activation_sql("lab", &fixture(MainActivationMode::Live), None),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            render_main_activation_sql(
                "lab",
                &fixture(MainActivationMode::Exclusive),
                Some(r"C:\tail.trn")
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            render_main_activation_sql(
                "lab",
                &fixture(MainActivationMode::Live),
                Some("bad\npath")
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
    }

    #[test]
    fn worker_promotes_ordinary_rows_without_database_recovery() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Worker), None).unwrap();
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Config WHERE FileName=N'root'")
        );
        assert!(
            !script
                .sql
                .contains("exclusive activation requires no other database sessions")
        );
        assert!(!script.sql.contains("SET SINGLE_USER"));
        assert!(!script.sql.contains("BACKUP LOG"));
        assert!(!script.sql.contains("RESTORE DATABASE"));
        assert!(script.report.live_session_switch_expected);
        assert!(!script.report.requires_tail_log_artifact);
    }

    #[test]
    fn exact_sha_and_row_set_guards_precede_mutations() {
        let script =
            render_main_activation_sql("lab", &fixture(MainActivationMode::Exclusive), None)
                .unwrap();
        let guard = script.sql.find("SHA2_256").unwrap();
        let mutation = script.sql.find("DELETE FROM dbo.Config WHERE").unwrap();
        assert!(guard < mutation);
        assert!(
            script
                .sql
                .contains("EXCEPT SELECT FileName,PartNo,DataSize,Digest")
        );
        assert!(script.sql.contains("ConfigSave exact snapshot drifted"));
    }

    #[test]
    fn fails_closed_without_acknowledgement_or_for_unknown_target() {
        let plan = fixture(MainActivationMode::Exclusive);
        let snapshot = MainActivationSnapshot {
            config_rows: plan.active_rows.clone(),
            config_dynamically_updated: None,
            params_dynamically_updated: None,
        };
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                plan.staged_rows.clone(),
                snapshot.clone(),
                &[BODY.to_owned(), format!("{BODY}.0")],
                false
            ),
            Err(MainActivationError::SafetyGate(_))
        ));
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                plan.staged_rows,
                snapshot,
                &[],
                true
            ),
            Err(MainActivationError::StructuralTarget(_))
        ));
    }

    #[test]
    fn rejects_missing_service_rows_duplicates_parts_and_reused_generation() {
        let plan = fixture(MainActivationMode::Exclusive);
        let mut missing = plan.staged_rows.clone();
        missing.retain(|row| row.file_name != "root");
        assert!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                missing,
                MainActivationSnapshot {
                    config_rows: plan.active_rows.clone(),
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            )
            .is_err()
        );

        let mut multipart = plan.staged_rows.clone();
        multipart[0].part_no = 1;
        assert!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                multipart,
                MainActivationSnapshot {
                    config_rows: plan.active_rows.clone(),
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            )
            .is_err()
        );

        let mut reused = plan.staged_rows;
        let versions_row = reused
            .iter_mut()
            .find(|row| row.file_name == "versions")
            .unwrap();
        versions_row.binary_data = versions(OLD);
        versions_row.data_size = versions_row.binary_data.len() as u64;
        assert!(matches!(
            prepare_main_activation(
                MainActivationMode::Exclusive,
                reused,
                MainActivationSnapshot {
                    config_rows: plan.active_rows,
                    config_dynamically_updated: None,
                    params_dynamically_updated: None
                },
                &[BODY.to_owned(), format!("{BODY}.0")],
                true
            ),
            Err(MainActivationError::Versions(_))
        ));
    }

    #[test]
    fn unchanged_payload_is_noop_that_only_validates_and_clears_stage() {
        let active = fixture(MainActivationMode::Exclusive).active_rows;
        let plan = prepare_main_activation(
            MainActivationMode::Exclusive,
            active.clone(),
            MainActivationSnapshot {
                config_rows: active,
                config_dynamically_updated: None,
                params_dynamically_updated: None,
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert!(plan.is_no_op());
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        assert!(!script.sql.contains("exclusive promotion source drifted"));
        assert!(script.sql.contains("DELETE FROM dbo.ConfigSave"));
    }

    #[test]
    fn recovery_token_covers_prior_rows_and_markers() {
        let plan = fixture(MainActivationMode::Online);
        let report = plan.dry_run_report();
        assert_eq!(report.recovery_token.len(), 64);
        assert_eq!(plan.recovery().overwritten_config_rows.len(), 5);
        assert_eq!(report.touched_tables, ["Config", "ConfigSave", "Params"]);
    }

    #[test]
    fn prior_dynamic_marker_overrides_stale_ordinary_versions_generation() {
        let base = fixture(MainActivationMode::Online);
        let current = "a05f2e61-a8a0-4d85-999b-663afc575ced";
        let config_marker = row(
            "DynamicallyUpdated",
            utf8_bom(&format!("{{1,1,{current}}}")),
        );
        let params_marker = row(
            "DynamicallyUpdated",
            utf8_bom(&format!("{{0,2,{OLD},{current}}}")),
        );
        let plan = prepare_main_activation(
            MainActivationMode::Online,
            base.staged_rows,
            MainActivationSnapshot {
                config_rows: base.active_rows,
                config_dynamically_updated: Some(config_marker),
                params_dynamically_updated: Some(params_marker),
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        assert_eq!(plan.old_generation().to_string(), current);
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        let expected_config = hex(&utf8_bom(&format!("{{1,2,{current},{NEW}}}")));
        let expected_params = hex(&utf8_bom(&format!("{{0,3,{OLD},{current},{NEW}}}")));
        assert!(script.sql.contains(&expected_config));
        assert!(script.sql.contains(&expected_params));
    }

    #[test]
    fn exclusive_normalizes_both_dynamic_markers_with_exact_counts() {
        let base = fixture(MainActivationMode::Exclusive);
        let current = "a05f2e61-a8a0-4d85-999b-663afc575ced";
        let plan = prepare_main_activation(
            MainActivationMode::Exclusive,
            base.staged_rows,
            MainActivationSnapshot {
                config_rows: base.active_rows,
                config_dynamically_updated: Some(row(
                    "DynamicallyUpdated",
                    utf8_bom(&format!("{{1,1,{current}}}")),
                )),
                params_dynamically_updated: Some(row(
                    "DynamicallyUpdated",
                    utf8_bom(&format!("{{0,2,{OLD},{current}}}")),
                )),
            },
            &[BODY.to_owned(), format!("{BODY}.0")],
            true,
        )
        .unwrap();
        let script = render_main_activation_sql("lab", &plan, None).unwrap();
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Config WHERE FileName=N'DynamicallyUpdated'")
        );
        assert!(
            script
                .sql
                .contains("DELETE FROM dbo.Params WHERE FileName=N'DynamicallyUpdated'")
        );
        assert!(script.sql.contains(
            "IF @@ROWCOUNT <> 1 THROW 57215, 'Config.DynamicallyUpdated cleanup drifted'"
        ));
        assert!(script.sql.contains(
            "IF @@ROWCOUNT <> 1 THROW 57216, 'Params.DynamicallyUpdated cleanup drifted'"
        ));
    }
}
