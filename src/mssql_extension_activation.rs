//! Fail-closed direct MSSQL publication for one verified extension.
//!
//! Only transactions observed in independent exact-build native controls are
//! rendered here. An independently validated schema phase shares the same
//! transaction as CAS and registry publication.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::io::Read;

use flate2::read::DeflateDecoder;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::mssql_extension_stage::{
    ExtensionRegistrySnapshot, ExtensionStagePlan, extension_namespace_prefix,
    validate_configinfo_manifest,
};
use crate::mssql_extensions::{DecodedExtensionZippedInfo, decode_extension_zipped_info};

const MAX_REGISTRY_BYTES: usize = 4_000;
const MAX_ROWS: usize = 100_000;
const MAX_PACKED_BYTES: usize = 512 * 1024 * 1024;
const MAX_SCRIPT_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionActivationMode {
    Exclusive,
    /// Existing sessions keep their loaded generation. New sessions observe
    /// the published generation after the 8.3.27 server polling interval.
    Online,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionServiceMarkerSnapshot {
    /// Whether the exact empty `dbStruFinal<uuid>` row existed at snapshot time.
    pub present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionActivationRecovery {
    pub namespace_prefix: String,
    pub extension_id: Vec<u8>,
    pub prior_registry_version: Vec<u8>,
    pub prior_registry_blob: Vec<u8>,
    pub published_registry_blob: Vec<u8>,
    pub prior_root_sha1: String,
    pub published_root_sha1: String,
    pub service_marker_previously_present: bool,
    #[serde(default = "legacy_service_marker")]
    pub service_marker_managed: bool,
    /// Registry-only recovery would detach a committed schema from its metadata.
    #[serde(default)]
    pub schema_changed: bool,
}

fn legacy_service_marker() -> bool {
    true
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionActivationDryRun {
    pub mode: ExtensionActivationMode,
    pub namespace_prefix: String,
    pub prior_root_sha1: String,
    pub published_root_sha1: String,
    pub staged_rows: usize,
    pub staged_bytes: u64,
    pub immutable_cas_rows: usize,
    pub touched_tables: Vec<String>,
    pub no_op: bool,
    pub live_cache_invalidation_verified: bool,
    pub existing_sessions_retain_generation: bool,
    pub recovery_token: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub restructured_objects: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CasRow {
    file_name: String,
    binary_data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaPublication {
    sql: String,
    tables: Vec<String>,
    objects: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionActivationPlan {
    mode: ExtensionActivationMode,
    namespace_prefix: String,
    snapshot: ExtensionRegistrySnapshot,
    registry_after: Vec<u8>,
    old_root: [u8; 20],
    new_root: [u8; 20],
    staged_rows: Vec<(String, i16, Vec<u8>, i32)>,
    cas_rows: Vec<CasRow>,
    staged_bytes: u64,
    storage_format: u32,
    service_marker: ExtensionServiceMarkerSnapshot,
    no_op: bool,
    own_ras_processes: Vec<crate::mssql_platform_profile::OwnRasProcess>,
    manage_service_marker: bool,
    schema: Option<SchemaPublication>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionActivationScript {
    sql: String,
    pub report: ExtensionActivationDryRun,
    pub recovery: ExtensionActivationRecovery,
}

impl ExtensionActivationScript {
    pub fn sql(&self) -> &str {
        &self.sql
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionActivationError {
    SafetyGate(String),
    UnsupportedPlatform(String),
    InvalidStage(String),
    Limit(String),
    Recovery(String),
}

impl Display for ExtensionActivationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (prefix, detail) = match self {
            Self::SafetyGate(v) => ("extension activation safety gate", v),
            Self::UnsupportedPlatform(v) => ("unsupported extension registry protocol", v),
            Self::InvalidStage(v) => ("invalid extension activation stage", v),
            Self::Limit(v) => ("extension activation limit exceeded", v),
            Self::Recovery(v) => ("invalid extension recovery snapshot", v),
        };
        write!(f, "{prefix}: {detail}")
    }
}

impl Error for ExtensionActivationError {}

impl ExtensionActivationPlan {
    pub(crate) fn storage_format(&self) -> u32 {
        self.storage_format
    }

    pub fn is_no_op(&self) -> bool {
        self.no_op
    }

    /// The plan with the worker processes whose idle SQL sessions the
    /// `exclusive` session gate leaves out: the sessions the tool's own RAS
    /// verification made the cluster open (#409 F-3).
    pub fn with_own_ras_processes(
        mut self,
        own: Vec<crate::mssql_platform_profile::OwnRasProcess>,
    ) -> Self {
        self.own_ras_processes = own;
        self
    }

    /// Attach only a complete schema plan validated against the active image
    /// and the SQL snapshot. DDL shares the publication transaction.
    pub(crate) fn with_restructuring(
        mut self,
        schema: &crate::mssql_extension_restructure::ExtensionRestructurePlan,
    ) -> Result<Self, ExtensionActivationError> {
        if self.mode != ExtensionActivationMode::Exclusive {
            return Err(ExtensionActivationError::SafetyGate(
                "extension restructuring requires exclusive activation".into(),
            ));
        }
        if self.no_op || schema.phase_sql().trim().is_empty() {
            return Err(ExtensionActivationError::InvalidStage(
                "a schema transition must belong to a changed extension image".into(),
            ));
        }
        self.schema = Some(SchemaPublication {
            sql: schema.phase_sql().to_owned(),
            tables: schema.touched_tables(),
            objects: schema.objects(),
        });
        Ok(self)
    }

    pub fn dry_run(&self) -> ExtensionActivationDryRun {
        ExtensionActivationDryRun {
            mode: self.mode,
            namespace_prefix: self.namespace_prefix.clone(),
            prior_root_sha1: hex(&self.old_root),
            published_root_sha1: hex(&self.new_root),
            staged_rows: self.staged_rows.len(),
            staged_bytes: self.staged_bytes,
            immutable_cas_rows: self.cas_rows.len(),
            touched_tables: if self.no_op {
                vec!["ConfigCASSave".to_owned()]
            } else {
                let mut tables = vec!["ConfigCAS", "ConfigCASSave", "_ExtensionsInfo"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if let Some(schema) = &self.schema {
                    tables.extend(schema.tables.iter().cloned());
                    tables.sort();
                    tables.dedup();
                }
                tables
            },
            no_op: self.no_op,
            live_cache_invalidation_verified: self.mode == ExtensionActivationMode::Online,
            existing_sessions_retain_generation: self.mode == ExtensionActivationMode::Online,
            recovery_token: String::new(),
            restructured_objects: self
                .schema
                .as_ref()
                .map(|s| s.objects.clone())
                .unwrap_or_default(),
        }
    }

    pub fn recovery(&self) -> ExtensionActivationRecovery {
        ExtensionActivationRecovery {
            namespace_prefix: self.namespace_prefix.clone(),
            extension_id: self.snapshot.extension_id.to_vec(),
            prior_registry_version: self.snapshot.version.to_vec(),
            prior_registry_blob: self.snapshot.zipped_info.clone(),
            published_registry_blob: self.registry_after.clone(),
            prior_root_sha1: hex(&self.old_root),
            published_root_sha1: hex(&self.new_root),
            service_marker_previously_present: self.service_marker.present,
            service_marker_managed: self.manage_service_marker,
            schema_changed: self.schema.is_some(),
        }
    }
}

/// Validates the registry envelope that the direct publisher must rewrite.
///
/// Callers run it as a read-only precheck before staging, so a refusal cannot
/// leave a staged `ConfigCASSave` namespace behind. It performs no I/O.
pub fn require_supported_registry_transition(
    zipped_info: &[u8],
) -> Result<DecodedExtensionZippedInfo, ExtensionActivationError> {
    if zipped_info.len() > MAX_REGISTRY_BYTES {
        return Err(ExtensionActivationError::Limit(format!(
            "registry blob has {} bytes, maximum is {MAX_REGISTRY_BYTES}",
            zipped_info.len()
        )));
    }
    let decoded = decode_extension_zipped_info(zipped_info)
        .map_err(|e| ExtensionActivationError::UnsupportedPlatform(e.to_string()))?;
    if zipped_info.get(30) != Some(&0) {
        return Err(ExtensionActivationError::UnsupportedPlatform(
            "only the clean published-to-published 8.3.27 transition (byte 30 = 0) is enabled"
                .to_owned(),
        ));
    }
    Ok(decoded)
}

/// Validates a complete selected-extension stage and prepares the evidenced
/// 8.3.27 registry transition. This function performs no I/O.
pub fn prepare_extension_activation(
    mode: ExtensionActivationMode,
    snapshot: ExtensionRegistrySnapshot,
    stage: &ExtensionStagePlan,
    service_marker: ExtensionServiceMarkerSnapshot,
    allow_non_lab: bool,
) -> Result<ExtensionActivationPlan, ExtensionActivationError> {
    prepare_extension_activation_impl(
        mode,
        snapshot,
        stage,
        service_marker,
        allow_non_lab,
        false,
        false,
    )
}

pub(crate) fn prepare_extension_activation_for_profile(
    profile: crate::mssql_platform_profile::MssqlNativePlatformProfile,
    mode: ExtensionActivationMode,
    snapshot: ExtensionRegistrySnapshot,
    stage: &ExtensionStagePlan,
    service_marker: ExtensionServiceMarkerSnapshot,
    allow_non_lab: bool,
) -> Result<ExtensionActivationPlan, ExtensionActivationError> {
    let v85 =
        profile == crate::mssql_platform_profile::MssqlNativePlatformProfile::Platform8_5_1_1150;
    if v85 && mode != ExtensionActivationMode::Exclusive {
        return Err(ExtensionActivationError::UnsupportedPlatform(
            "8.5.1.1150 extension publication is measured only in exclusive mode".into(),
        ));
    }
    let configinfo = stage
        .rows()
        .iter()
        .find(|r| r.logical_name == "configinfo")
        .ok_or_else(|| {
            ExtensionActivationError::InvalidStage("configinfo row is missing".into())
        })?;
    let manifest = validate_configinfo_manifest(&inflate_raw_bounded(
        &configinfo.binary_data,
        MAX_PACKED_BYTES,
    )?)
    .map_err(|e| ExtensionActivationError::InvalidStage(e.to_string()))?;
    profile
        .require_extension_storage_format(manifest.storage_format)
        .map_err(|e| ExtensionActivationError::UnsupportedPlatform(e.to_string()))?;
    prepare_extension_activation_impl(
        mode,
        snapshot,
        stage,
        service_marker,
        allow_non_lab,
        v85,
        true,
    )
}

fn prepare_extension_activation_impl(
    mode: ExtensionActivationMode,
    snapshot: ExtensionRegistrySnapshot,
    stage: &ExtensionStagePlan,
    service_marker: ExtensionServiceMarkerSnapshot,
    allow_non_lab: bool,
    v85: bool,
    synchronize_version: bool,
) -> Result<ExtensionActivationPlan, ExtensionActivationError> {
    if !allow_non_lab {
        return Err(ExtensionActivationError::SafetyGate(
            "--allow-non-lab acknowledgement is required".to_owned(),
        ));
    }
    let decoded = require_supported_registry_transition(&snapshot.zipped_info)?;
    let old_root = parse_sha1(&decoded.active_cas_root)?;
    let namespace = extension_namespace_prefix(snapshot.extension_id);
    if stage.namespace_prefix() != namespace {
        return Err(ExtensionActivationError::InvalidStage(format!(
            "stage namespace {} does not match selected extension {namespace}",
            stage.namespace_prefix()
        )));
    }
    if stage.rows().len() > MAX_ROWS {
        return Err(ExtensionActivationError::Limit(format!(
            "{} rows exceeds {MAX_ROWS}",
            stage.rows().len()
        )));
    }

    let mut by_name = BTreeMap::new();
    let mut staged_rows = Vec::with_capacity(stage.rows().len());
    let mut staged_bytes = 0_u64;
    for row in stage.rows() {
        if row.attributes != 0 || row.part_no != 0 {
            return Err(ExtensionActivationError::InvalidStage(format!(
                "{} must have Attributes=0 and PartNo=0",
                row.logical_name
            )));
        }
        if by_name
            .insert(row.logical_name.clone(), row.binary_data.as_slice())
            .is_some()
        {
            return Err(ExtensionActivationError::InvalidStage(format!(
                "duplicate logical name {}",
                row.logical_name
            )));
        }
        staged_bytes = staged_bytes
            .checked_add(row.binary_data.len() as u64)
            .ok_or_else(|| {
                ExtensionActivationError::Limit("packed byte count overflow".to_owned())
            })?;
        staged_rows.push((
            row.logical_name.clone(),
            row.attributes,
            row.binary_data.clone(),
            row.part_no,
        ));
    }
    if staged_bytes > MAX_PACKED_BYTES as u64 {
        return Err(ExtensionActivationError::Limit(format!(
            "{staged_bytes} packed bytes exceeds {MAX_PACKED_BYTES}"
        )));
    }
    let configinfo = by_name.get("configinfo").ok_or_else(|| {
        ExtensionActivationError::InvalidStage("configinfo row is missing".to_owned())
    })?;
    let manifest_plain = inflate_raw_bounded(configinfo, MAX_PACKED_BYTES)?;
    let manifest = validate_configinfo_manifest(&manifest_plain)
        .map_err(|e| ExtensionActivationError::InvalidStage(e.to_string()))?;
    if manifest.entries.len() + 1 != by_name.len() {
        return Err(ExtensionActivationError::InvalidStage(
            "stage must contain exactly configinfo and every mapped content row".to_owned(),
        ));
    }
    let mut mapped = BTreeSet::new();
    for entry in &manifest.entries {
        let packed = by_name.get(&entry.logical_name).ok_or_else(|| {
            ExtensionActivationError::InvalidStage(format!(
                "configinfo maps missing row {}",
                entry.logical_name
            ))
        })?;
        let actual: [u8; 20] = Sha1::digest(packed).into();
        if actual != entry.packed_sha1 {
            return Err(ExtensionActivationError::InvalidStage(format!(
                "configinfo digest differs for {}",
                entry.logical_name
            )));
        }
        mapped.insert(entry.logical_name.clone());
    }
    if by_name
        .keys()
        .any(|name| name != "configinfo" && !mapped.contains(name))
    {
        return Err(ExtensionActivationError::InvalidStage(
            "stage contains a row absent from configinfo".to_owned(),
        ));
    }

    let new_root: [u8; 20] = Sha1::digest(configinfo).into();
    let mut registry_after = snapshot.zipped_info.clone();
    if registry_after.len() < 35 {
        return Err(ExtensionActivationError::UnsupportedPlatform(
            "registry envelope is too short for evidenced transition".to_owned(),
        ));
    }
    registry_after[4..24].copy_from_slice(&new_root);
    registry_after[30] = 0;
    registry_after[31..35].copy_from_slice(&snapshot.version[4..8]);
    if synchronize_version {
        let key = manifest.identity.configuration_id.hyphenated().to_string();
        let packed = by_name.get(&key).ok_or_else(|| {
            ExtensionActivationError::InvalidStage("staged extension root is missing".into())
        })?;
        let version = crate::mssql_extension_tree_load::stored_root_version(packed, &key)
            .map_err(|e| ExtensionActivationError::InvalidStage(e.to_string()))?;
        registry_after =
            crate::mssql_extensions::replace_extension_registry_version(&registry_after, &version)
                .map_err(|e| ExtensionActivationError::UnsupportedPlatform(e.to_string()))?;
    }
    let after = decode_extension_zipped_info(&registry_after)
        .map_err(|e| ExtensionActivationError::UnsupportedPlatform(e.to_string()))?;
    if after.active_cas_root != hex(&new_root) || !after.active {
        return Err(ExtensionActivationError::UnsupportedPlatform(
            "patched registry envelope failed its full decoder".to_owned(),
        ));
    }

    let mut cas_by_hash = BTreeMap::<String, Vec<u8>>::new();
    for (_, _, bytes, _) in &staged_rows {
        let file_name = hex(&Sha1::digest(bytes));
        if let Some(existing) = cas_by_hash.get(&file_name) {
            if existing != bytes {
                return Err(ExtensionActivationError::InvalidStage(format!(
                    "SHA-1 collision for packed ConfigCAS key {file_name}"
                )));
            }
        } else {
            cas_by_hash.insert(file_name, bytes.clone());
        }
    }
    let cas_rows = cas_by_hash
        .into_iter()
        .map(|(file_name, binary_data)| CasRow {
            file_name,
            binary_data,
        })
        .collect();
    Ok(ExtensionActivationPlan {
        mode,
        namespace_prefix: namespace,
        snapshot,
        registry_after,
        old_root,
        new_root,
        staged_rows,
        cas_rows,
        staged_bytes,
        storage_format: manifest.storage_format,
        service_marker,
        no_op: old_root == new_root,
        own_ras_processes: Vec::new(),
        // 8.5 refreshes an existing legacy marker, but does not create one.
        manage_service_marker: !v85 || service_marker.present,
        schema: None,
    })
}

/// Renders one SERIALIZABLE transaction. Binary payloads are embedded and the
/// result must be sent as a script file/stdin, never through a command line.
pub fn render_extension_activation_sql(
    database: &str,
    plan: &ExtensionActivationPlan,
) -> Result<ExtensionActivationScript, ExtensionActivationError> {
    let mut report = plan.dry_run();
    let recovery = plan.recovery();
    let recovery_json = serde_json::to_vec(&recovery).map_err(|error| {
        ExtensionActivationError::Recovery(format!(
            "failed to serialize extension recovery token: {error}"
        ))
    })?;
    report.recovery_token = hex(&sha2::Sha256::digest(&recovery_json));
    let db = quote_ident(database)?;
    let pattern = format!("{}~_~_%", plan.namespace_prefix);
    let marker = format!("dbStruFinal{}", plan.namespace_prefix);
    let mut sql = String::new();
    writeln!(sql, "SET NOCOUNT ON;\nSET XACT_ABORT ON;\nSET TRANSACTION ISOLATION LEVEL SERIALIZABLE;\nUSE {db};\nBEGIN TRY\nBEGIN TRANSACTION;").unwrap();
    writeln!(sql, "DECLARE @LockResult int; EXEC @LockResult = sys.sp_getapplock @Resource=N'{}', @LockMode='Exclusive', @LockOwner='Transaction', @LockTimeout=0; IF @LockResult < 0 THROW 57200, 'Extension activation lock unavailable', 1;", quote_string(&format!("ibcmd-rs:extension-activation:{}", plan.namespace_prefix))).unwrap();
    if plan.mode == ExtensionActivationMode::Exclusive {
        let gate = crate::mssql_platform_profile::exclusive_session_gate(
            57212,
            "exclusive extension activation requires no other database sessions",
            &plan.own_ras_processes,
        );
        sql.push_str(&gate);
        sql.push('\n');
    }
    writeln!(sql, "DECLARE @ExtensionId binary(16)=0x{}; DECLARE @ExpectedVersion binary(8)=0x{}; DECLARE @Before varbinary(max)=0x{}; DECLARE @After varbinary(max)=0x{};", hex(&plan.snapshot.extension_id), hex(&plan.snapshot.version), hex(&plan.snapshot.zipped_info), hex(&plan.registry_after)).unwrap();
    sql.push_str("IF (SELECT COUNT_BIG(*) FROM dbo._ExtensionsInfo WITH (UPDLOCK,HOLDLOCK) WHERE _IDRRef=@ExtensionId AND _Version=@ExpectedVersion AND DATALENGTH(_ExtensionZippedInfo)=DATALENGTH(@Before) AND _ExtensionZippedInfo=@Before) <> 1 THROW 57201, 'Extension registry optimistic predicate failed', 1;\n");
    writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.ConfigCASSave WITH (UPDLOCK,HOLDLOCK) WHERE FileName LIKE N'{}' ESCAPE N'~') <> {} THROW 57202, 'Selected ConfigCASSave prefix row count changed', 1;", quote_string(&pattern), plan.staged_rows.len()).unwrap();
    for (name, attributes, bytes, part) in &plan.staged_rows {
        let full_name = format!("{}__{}", plan.namespace_prefix, name);
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.ConfigCASSave WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}' AND Attributes={} AND DataSize={} AND DATALENGTH(BinaryData)={} AND BinaryData=0x{} AND PartNo={}) <> 1 THROW 57203, 'Selected ConfigCASSave row changed', 1;", quote_string(&full_name), attributes, bytes.len(), bytes.len(), hex(bytes), part).unwrap();
    }
    if plan.no_op {
        writeln!(sql, "DELETE dbo.ConfigCASSave WHERE FileName LIKE N'{}' ESCAPE N'~'; IF @@ROWCOUNT<>{} THROW 57209, 'Selected ConfigCASSave cleanup count changed', 1;", quote_string(&pattern), plan.staged_rows.len()).unwrap();
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigCASSave WHERE FileName LIKE N'{}' ESCAPE N'~') THROW 57211, 'Selected staging prefix remains', 1;", quote_string(&pattern)).unwrap();
        finish_publication_transaction(&mut sql);
        return Ok(ExtensionActivationScript {
            sql,
            report,
            recovery,
        });
    }
    if let Some(schema) = &plan.schema {
        sql.push_str(&schema.sql);
        sql.push('\n');
    }
    for row in &plan.cas_rows {
        writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}') BEGIN IF (SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}') <> 1 OR (SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}' AND Attributes=0 AND DataSize={} AND DATALENGTH(BinaryData)={} AND BinaryData=0x{} AND PartNo=0) <> 1 THROW 57204, 'Immutable ConfigCAS collision or drift', 1; END ELSE BEGIN INSERT dbo.ConfigCAS (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) VALUES (N'{}',DATEADD(year,2000,SYSUTCDATETIME()),DATEADD(year,2000,SYSUTCDATETIME()),0,{},0x{},0); IF @@ROWCOUNT<>1 THROW 57205, 'ConfigCAS insert failed', 1; END;", row.file_name, row.file_name, row.file_name, row.binary_data.len(), row.binary_data.len(), hex(&row.binary_data), row.file_name, row.binary_data.len(), hex(&row.binary_data)).unwrap();
    }
    if plan.manage_service_marker {
        writeln!(sql, "DECLARE @Marker nvarchar(255)=N'{}'; DECLARE @MarkerCount bigint=(SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=@Marker);", quote_string(&marker)).unwrap();
        if plan.service_marker.present {
            sql.push_str("IF @MarkerCount<>1 OR (SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WHERE FileName=@Marker AND Attributes=0 AND DataSize=0 AND DATALENGTH(BinaryData)=0 AND PartNo=0)<>1 THROW 57206, 'dbStruFinal optimistic predicate failed', 1; UPDATE dbo.ConfigCAS SET Modified=DATEADD(year,2000,SYSUTCDATETIME()) WHERE FileName=@Marker AND Attributes=0 AND DataSize=0 AND DATALENGTH(BinaryData)=0 AND PartNo=0; IF @@ROWCOUNT<>1 THROW 57207, 'dbStruFinal refresh failed', 1;\n");
        } else {
            sql.push_str("IF @MarkerCount<>0 THROW 57206, 'dbStruFinal appeared after snapshot', 1; INSERT dbo.ConfigCAS (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) VALUES (@Marker,DATEADD(year,2000,SYSUTCDATETIME()),DATEADD(year,2000,SYSUTCDATETIME()),0,0,0x,0); IF @@ROWCOUNT<>1 THROW 57207, 'dbStruFinal insert failed', 1;\n");
        }
    }
    sql.push_str("UPDATE dbo._ExtensionsInfo SET _UpdateTime=DATEADD(year,2000,SYSUTCDATETIME()), _ExtensionZippedInfo=@After WHERE _IDRRef=@ExtensionId AND _Version=@ExpectedVersion AND DATALENGTH(_ExtensionZippedInfo)=DATALENGTH(@Before) AND _ExtensionZippedInfo=@Before; IF @@ROWCOUNT<>1 THROW 57208, 'Extension registry update lost race', 1;\n");
    writeln!(sql, "DELETE dbo.ConfigCASSave WHERE FileName LIKE N'{}' ESCAPE N'~'; IF @@ROWCOUNT<>{} THROW 57209, 'Selected ConfigCASSave cleanup count changed', 1;", quote_string(&pattern), plan.staged_rows.len()).unwrap();
    sql.push_str("IF (SELECT COUNT_BIG(*) FROM dbo._ExtensionsInfo WHERE _IDRRef=@ExtensionId AND _Version<>@ExpectedVersion AND DATALENGTH(_ExtensionZippedInfo)=DATALENGTH(@After) AND _ExtensionZippedInfo=@After)<>1 THROW 57210, 'Registry postcondition failed', 1;\n");
    writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.ConfigCASSave WHERE FileName LIKE N'{}' ESCAPE N'~') THROW 57211, 'Selected staging prefix remains', 1;", quote_string(&pattern)).unwrap();
    finish_publication_transaction(&mut sql);
    if sql.len() > MAX_SCRIPT_BYTES {
        return Err(ExtensionActivationError::Limit(format!(
            "SQL script has {} bytes, maximum is {MAX_SCRIPT_BYTES}",
            sql.len()
        )));
    }
    Ok(ExtensionActivationScript {
        sql,
        report,
        recovery,
    })
}

/// Roll back errors that SQL Server does not abort automatically as well.
fn finish_publication_transaction(sql: &mut String) {
    sql.push_str(
        "COMMIT TRANSACTION;\nEND TRY\nBEGIN CATCH\nIF @@TRANCOUNT > 0 ROLLBACK TRANSACTION;\nTHROW;\nEND CATCH;\n",
    );
}

/// Binds a recovery transaction to the exact post-publication rowversion/blob.
/// New immutable CAS rows are intentionally retained; they are content-addressed.
pub fn render_extension_recovery_sql(
    database: &str,
    recovery: &ExtensionActivationRecovery,
    current_version: [u8; 8],
    current_blob: &[u8],
    allow_non_lab: bool,
) -> Result<String, ExtensionActivationError> {
    if !allow_non_lab {
        return Err(ExtensionActivationError::SafetyGate(
            "--allow-non-lab acknowledgement is required".to_owned(),
        ));
    }
    if recovery.schema_changed {
        return Err(ExtensionActivationError::Recovery(
            "this publication changed the extension schema; registry-only recovery would detach stored data from its metadata; restore the verified database backup instead".into(),
        ));
    }
    if current_blob != recovery.published_registry_blob {
        return Err(ExtensionActivationError::Recovery(
            "current registry blob is not the publication recorded by this recovery artifact"
                .to_owned(),
        ));
    }
    decode_extension_zipped_info(current_blob)
        .map_err(|e| ExtensionActivationError::Recovery(e.to_string()))?;
    let id: [u8; 16] = recovery.extension_id.as_slice().try_into().map_err(|_| {
        ExtensionActivationError::Recovery("extension id is not 16 bytes".to_owned())
    })?;
    if extension_namespace_prefix(id) != recovery.namespace_prefix {
        return Err(ExtensionActivationError::Recovery(
            "extension namespace does not match id".to_owned(),
        ));
    }
    let old_root = parse_sha1(&recovery.prior_root_sha1)?;
    let mut after = current_blob.to_vec();
    after[4..24].copy_from_slice(&old_root);
    after[30] = 0;
    after[31..35].copy_from_slice(&current_version[4..8]);
    let after = crate::mssql_extensions::restore_extension_registry_version(
        &after,
        &recovery.prior_registry_blob,
    )
    .map_err(|e| ExtensionActivationError::Recovery(e.to_string()))?;
    decode_extension_zipped_info(&after)
        .map_err(|e| ExtensionActivationError::Recovery(e.to_string()))?;
    let marker = format!("dbStruFinal{}", recovery.namespace_prefix);
    let mut sql = format!(
        "SET NOCOUNT ON;\nSET XACT_ABORT ON;\nSET TRANSACTION ISOLATION LEVEL SERIALIZABLE;\nUSE {};\nBEGIN TRANSACTION;\n",
        quote_ident(database)?
    );
    writeln!(sql, "DECLARE @LockResult int; EXEC @LockResult=sys.sp_getapplock @Resource=N'{}',@LockMode='Exclusive',@LockOwner='Transaction',@LockTimeout=0; IF @LockResult<0 THROW 57220,'Extension recovery lock unavailable',1;", quote_string(&format!("ibcmd-rs:extension-activation:{}", recovery.namespace_prefix))).unwrap();
    writeln!(sql, "DECLARE @Id binary(16)=0x{}; DECLARE @Version binary(8)=0x{}; DECLARE @Before varbinary(max)=0x{}; DECLARE @After varbinary(max)=0x{};", hex(&id), hex(&current_version), hex(current_blob), hex(&after)).unwrap();
    sql.push_str("UPDATE dbo._ExtensionsInfo WITH (UPDLOCK,HOLDLOCK) SET _UpdateTime=DATEADD(year,2000,SYSUTCDATETIME()),_ExtensionZippedInfo=@After WHERE _IDRRef=@Id AND _Version=@Version AND DATALENGTH(_ExtensionZippedInfo)=DATALENGTH(@Before) AND _ExtensionZippedInfo=@Before; IF @@ROWCOUNT<>1 THROW 57221,'Extension recovery optimistic predicate failed',1;\n");
    if recovery.service_marker_managed {
        if recovery.service_marker_previously_present {
            writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}')<>1 OR (SELECT COUNT_BIG(*) FROM dbo.ConfigCAS WITH (UPDLOCK,HOLDLOCK) WHERE FileName=N'{}' AND Attributes=0 AND DataSize=0 AND DATALENGTH(BinaryData)=0 AND PartNo=0)<>1 THROW 57222,'dbStruFinal recovery predicate failed',1; UPDATE dbo.ConfigCAS SET Modified=DATEADD(year,2000,SYSUTCDATETIME()) WHERE FileName=N'{}' AND Attributes=0 AND DataSize=0 AND DATALENGTH(BinaryData)=0 AND PartNo=0;", quote_string(&marker), quote_string(&marker), quote_string(&marker)).unwrap();
        } else {
            writeln!(sql, "DELETE dbo.ConfigCAS WHERE FileName=N'{}' AND Attributes=0 AND DataSize=0 AND DATALENGTH(BinaryData)=0 AND PartNo=0; IF @@ROWCOUNT<>1 THROW 57222,'dbStruFinal recovery predicate failed',1;", quote_string(&marker)).unwrap();
        }
    }
    sql.push_str("COMMIT TRANSACTION;\n");
    Ok(sql)
}

fn inflate_raw_bounded(bytes: &[u8], maximum: usize) -> Result<Vec<u8>, ExtensionActivationError> {
    let decoder = DeflateDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .take((maximum as u64) + 1)
        .read_to_end(&mut output)
        .map_err(|e| {
            ExtensionActivationError::InvalidStage(format!("cannot inflate configinfo: {e}"))
        })?;
    if output.len() > maximum {
        return Err(ExtensionActivationError::Limit(format!(
            "inflated configinfo exceeds {maximum} bytes"
        )));
    }
    Ok(output)
}

fn parse_sha1(value: &str) -> Result<[u8; 20], ExtensionActivationError> {
    if value.len() != 40 {
        return Err(ExtensionActivationError::UnsupportedPlatform(
            "active root is not SHA-1".to_owned(),
        ));
    }
    let mut out = [0_u8; 20];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Ok(out)
}

fn digit(value: u8) -> Result<u8, ExtensionActivationError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(ExtensionActivationError::UnsupportedPlatform(
            "active root contains non-hex data".to_owned(),
        )),
    }
}

fn quote_ident(value: &str) -> Result<String, ExtensionActivationError> {
    if value.is_empty() || value.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
        return Err(ExtensionActivationError::SafetyGate(
            "invalid database identifier".to_owned(),
        ));
    }
    Ok(format!("[{}]", value.replace(']', "]]")))
}

fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_extension_stage::{
        ConfigInfoIdentity, ExtensionStageRow, prepare_extension_stage,
    };
    use uuid::Uuid;

    const REGISTRY: &str = "43c29a1435571124d08b1ad5dbf127247fd18626c615134ba19a0800000001000000018181974f7b002200230022002c00380037003000320034003700330038002d0066006300320061002d0034003400330036002d0061006400610031002d006400660037003900640033003900350063003400320034002c000a007b0031002c0022007200750022002c002200140435043c043e043a0020001f044304410442043e043504200040043004410448043804400435043d043804350422007d000a007d009a07312e302e312e31828120";

    fn decode(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|p| (digit(p[0]).unwrap() << 4) | digit(p[1]).unwrap())
            .collect()
    }
    fn fixture() -> (ExtensionRegistrySnapshot, ExtensionStagePlan) {
        let id = [
            0x87, 0x80, 0x10, 0x7b, 0x44, 0xa2, 0x85, 0x8a, 0x11, 0xe8, 0x07, 0x73, 0x34, 0x7e,
            0xea, 0x02,
        ];
        let root = Uuid::parse_str("cd9aecaa-e480-481c-8c77-ff6600837681").unwrap();
        let plan = prepare_extension_stage(
            id,
            &ConfigInfoIdentity {
                storage_format: 80321,
                configuration_id: root,
                descriptor: vec![1, 2, 3],
            },
            vec![
                ExtensionStageRow::new(root.to_string(), vec![1, 2, 3]),
                ExtensionStageRow::new(format!("{root}.0"), vec![4, 5, 6]),
            ],
        )
        .unwrap();
        let mut zipped_info = decode(REGISTRY);
        zipped_info[30] = 0;
        zipped_info[31..35].copy_from_slice(&[0, 0, 0x18, 0x5f]);
        (
            ExtensionRegistrySnapshot {
                extension_id: id,
                version: [0, 0, 0, 0, 0, 1, 0x7e, 0xd2],
                zipped_info,
            },
            plan,
        )
    }

    #[test]
    fn schema_phase_shares_the_registry_transaction_and_cannot_use_registry_only_recovery() {
        let (snapshot, stage) = fixture();
        let mut plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        plan.schema = Some(SchemaPublication {
            sql: "CREATE TABLE dbo.[_Reference900X1] ([_IDRRef] binary(16) NOT NULL);".into(),
            tables: vec!["_Reference900X1".into(), "SchemaStorage".into()],
            objects: vec!["Catalog.Example".into()],
        });
        let script = render_extension_activation_sql("db", &plan).unwrap();
        let text = script.sql();
        let begin = text.find("BEGIN TRANSACTION").unwrap();
        let ddl = text.find("CREATE TABLE dbo.[_Reference900X1]").unwrap();
        let registry = text.find("UPDATE dbo._ExtensionsInfo SET").unwrap();
        let commit = text.find("COMMIT TRANSACTION").unwrap();
        assert!(begin < ddl && ddl < registry && registry < commit);
        assert!(
            script
                .report
                .touched_tables
                .contains(&"SchemaStorage".to_owned())
        );
        assert_eq!(script.report.restructured_objects, vec!["Catalog.Example"]);
        assert!(script.recovery.schema_changed);
        let error = render_extension_recovery_sql(
            "db",
            &script.recovery,
            [0; 8],
            &script.recovery.published_registry_blob,
            true,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("registry-only recovery would detach stored data")
        );
    }

    #[test]
    fn publication_catches_errors_and_explicitly_rolls_back() {
        let (snapshot, stage) = fixture();
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        let script = render_extension_activation_sql("db", &plan).unwrap();
        let text = script.sql();
        let begin = text
            .find("BEGIN TRY")
            .expect("publication must catch failures");
        let write = text.find("INSERT dbo.ConfigCAS").unwrap();
        let commit = text.find("COMMIT TRANSACTION").unwrap();
        let catch = text
            .find("BEGIN CATCH")
            .expect("publication must roll back");
        assert!(begin < write && write < commit && commit < catch);
        assert!(text[catch..].contains("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION"));
        assert!(text[catch..].contains("THROW;"));
    }

    #[test]
    fn patches_only_evidenced_registry_fields() {
        let (snapshot, stage) = fixture();
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot.clone(),
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        for i in 0..snapshot.zipped_info.len() {
            if !(4..24).contains(&i) && i != 30 && !(31..35).contains(&i) {
                assert_eq!(plan.registry_after[i], snapshot.zipped_info[i], "byte {i}");
            }
        }
        assert_eq!(&plan.registry_after[31..35], &snapshot.version[4..8]);
        assert_eq!(plan.registry_after[30], 0);
    }

    #[test]
    fn sql_is_selected_transactional_and_does_not_touch_unproven_caches() {
        let (snapshot, stage) = fixture();
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        let script = render_extension_activation_sql("db]name", &plan).unwrap();
        assert!(script.sql().contains("SERIALIZABLE"));
        assert!(script.sql().contains("sp_getapplock"));
        assert!(script.sql().contains("sys.dm_exec_sessions"));
        assert!(script.sql().contains("USE [db]]name]"));
        assert!(script.sql().contains("ConfigCASSave"));
        assert!(script.sql().contains("ESCAPE N'~'"));
        assert!(
            script
                .sql()
                .contains("dbStruFinal347eea02-0773-11e8-8780-107b44a2858a")
        );
        for forbidden in [
            "dbo.Params",
            "dbo.Files",
            "_ExtensionsRestruct",
            "_ExtensionsInfoNGS",
            "ibcmd.exe",
        ] {
            assert!(!script.sql().contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn the_exclusive_gate_leaves_out_only_the_idle_sessions_of_the_own_ras_processes() {
        let (snapshot, stage) = fixture();
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        let counted = render_extension_activation_sql("db", &plan).unwrap();
        assert!(counted.sql().contains(
            "database_id=DB_ID()) THROW 57212, 'exclusive extension activation requires no other database sessions', 1;"
        ));
        let plan =
            plan.with_own_ras_processes(vec![crate::mssql_platform_profile::OwnRasProcess {
                host: "LAB-HOST".to_owned(),
                pid: 22608,
            }]);
        let exempt = render_extension_activation_sql("db", &plan).unwrap();
        assert!(exempt.sql().contains(
            "AND status=N'sleeping' AND open_transaction_count=0 AND ((ISNULL(host_name,N'')=N'LAB-HOST' AND ISNULL(host_process_id,-1) IN (22608))))"
        ));
        assert!(exempt.sql().contains("THROW 57212"));
    }

    #[test]
    fn online_is_evidenced_and_unacknowledged_writes_fail_closed() {
        let (snapshot, stage) = fixture();
        let online = prepare_extension_activation(
            ExtensionActivationMode::Online,
            snapshot.clone(),
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        assert!(online.dry_run().live_cache_invalidation_verified);
        assert!(online.dry_run().existing_sessions_retain_generation);
        let online_sql = render_extension_activation_sql("db", &online).unwrap();
        assert!(!online_sql.sql().contains("sys.dm_exec_sessions"));
        assert_eq!(online_sql.report.recovery_token.len(), 64);
        assert!(
            prepare_extension_activation(
                ExtensionActivationMode::Exclusive,
                snapshot,
                &stage,
                ExtensionServiceMarkerSnapshot { present: true },
                false
            )
            .is_err()
        );
    }

    #[test]
    fn recovery_requires_exact_published_blob_and_version_binds_generation() {
        let (snapshot, stage) = fixture();
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: false },
            true,
        )
        .unwrap();
        let token = plan.recovery();
        assert!(
            render_extension_recovery_sql("db", &token, [0, 0, 0, 0, 0, 1, 2, 3], b"wrong", true)
                .is_err()
        );
        let sql = render_extension_recovery_sql(
            "db",
            &token,
            [0, 0, 0, 0, 0, 1, 2, 3],
            &token.published_registry_blob,
            true,
        )
        .unwrap();
        assert!(sql.contains("DELETE dbo.ConfigCAS"));
        assert!(sql.contains("0000000000010203"));
    }

    #[test]
    fn unchanged_extension_only_validates_and_clears_its_stage() {
        let (snapshot, stage) = fixture();
        let first = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot.clone(),
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        let snapshot = ExtensionRegistrySnapshot {
            zipped_info: first.recovery().published_registry_blob,
            ..snapshot
        };
        let plan = prepare_extension_activation(
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        assert!(plan.is_no_op());
        let script = render_extension_activation_sql("db", &plan).unwrap();
        assert!(script.sql().contains("DELETE dbo.ConfigCASSave"));
        assert!(!script.sql().contains("UPDATE dbo._ExtensionsInfo SET"));
        assert_eq!(script.report.touched_tables, vec!["ConfigCASSave"]);
    }
}

#[cfg(test)]
mod platform85_tests {
    use super::*;
    use crate::mssql_extension_stage::ExtensionStageRow;
    use crate::mssql_platform_profile::MssqlNativePlatformProfile;

    fn native_fixture() -> (ExtensionRegistrySnapshot, ExtensionStagePlan, Vec<u8>) {
        native_fixture_parts(
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-851150/registry-snapshot.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-851150/registry-before.hex"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-851150/native-stage.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-851150/registry-applied.hex"
            ),
        )
    }

    fn native_fixture_parts(
        metadata: &str,
        before: &str,
        rows: &str,
        after: &str,
    ) -> (ExtensionRegistrySnapshot, ExtensionStagePlan, Vec<u8>) {
        let metadata: serde_json::Value = serde_json::from_str(metadata).unwrap();
        let array = |name: &str| {
            metadata[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u8)
                .collect::<Vec<_>>()
        };
        let id: [u8; 16] = array("extension_id").try_into().unwrap();
        let decode = |text: &str| {
            text.trim()
                .as_bytes()
                .chunks_exact(2)
                .map(|p| (digit(p[0]).unwrap() << 4) | digit(p[1]).unwrap())
                .collect::<Vec<_>>()
        };
        let snapshot = ExtensionRegistrySnapshot {
            extension_id: id,
            version: array("version").try_into().unwrap(),
            zipped_info: decode(before),
        };
        let rows: Vec<serde_json::Value> = serde_json::from_str(rows).unwrap();
        let rows = rows
            .iter()
            .map(|r| {
                ExtensionStageRow::new(
                    r["name"].as_str().unwrap(),
                    crate::module_blob::decode_base64_mime(r["packed"].as_str().unwrap()).unwrap(),
                )
            })
            .collect();
        (
            snapshot,
            ExtensionStagePlan::from_complete_rows(id, rows).unwrap(),
            decode(after),
        )
    }

    #[test]
    fn review_579_recovery_restores_prior_version_and_envelope_on_both_builds() {
        let native27 = native_fixture_parts(
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-snapshot.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-before.hex"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/native-stage.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-applied.hex"
            ),
        );
        for (profile, (snapshot, stage, _)) in [
            (
                MssqlNativePlatformProfile::Platform8_5_1_1150,
                native_fixture(),
            ),
            (MssqlNativePlatformProfile::Platform8_3_27_2214, native27),
        ] {
            let prior_blob = snapshot.zipped_info.clone();
            let prior = decode_extension_zipped_info(&prior_blob).unwrap();
            let plan = prepare_extension_activation_for_profile(
                profile,
                ExtensionActivationMode::Exclusive,
                snapshot,
                &stage,
                ExtensionServiceMarkerSnapshot { present: true },
                true,
            )
            .unwrap();
            let recovery = plan.recovery();
            assert_ne!(
                decode_extension_zipped_info(&recovery.published_registry_blob)
                    .unwrap()
                    .version,
                prior.version
            );
            let current_version = [0, 0, 0, 0, 0, 1, 2, 3];
            let sql = render_extension_recovery_sql(
                "synthetic",
                &recovery,
                current_version,
                &recovery.published_registry_blob,
                true,
            )
            .unwrap();
            let after_hex = sql
                .split("DECLARE @After varbinary(max)=0x")
                .nth(1)
                .unwrap()
                .split(';')
                .next()
                .unwrap();
            let after = after_hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| (digit(pair[0]).unwrap() << 4) | digit(pair[1]).unwrap())
                .collect::<Vec<_>>();
            let decoded = decode_extension_zipped_info(&after).unwrap();
            assert_eq!(decoded.version, prior.version, "{}", profile.id());
            assert_eq!(decoded.active_cas_root, prior.active_cas_root);
            let mut expected = prior_blob;
            expected[30] = 0;
            expected[31..35].copy_from_slice(&current_version[4..8]);
            assert_eq!(after, expected, "{}", profile.id());
        }
    }

    #[test]
    fn native_83272214_storage_and_version_transition_are_exact() {
        let (snapshot, stage, expected) = native_fixture_parts(
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-snapshot.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-before.hex"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/native-stage.json"
            ),
            include_str!(
                "../tests/fixtures/native-evidence/extension-apply-83272214/registry-applied.hex"
            ),
        );
        let plan = prepare_extension_activation_for_profile(
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        assert_eq!(plan.registry_after, expected);
        assert!(plan.recovery().service_marker_managed);
    }

    #[test]
    fn native_851150_registry_transition_and_marker_policy_are_exact() {
        let (snapshot, stage, expected) = native_fixture();
        let plan = prepare_extension_activation_for_profile(
            MssqlNativePlatformProfile::Platform8_5_1_1150,
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: false },
            true,
        )
        .unwrap();
        assert_eq!(plan.registry_after, expected);
        assert!(!plan.recovery().service_marker_managed);
        let script = render_extension_activation_sql("synthetic", &plan).unwrap();
        assert!(!script.sql().contains("dbStruFinal"));
        assert!(!script.sql().contains("dbo.Params"));
        assert!(script.sql().contains("optimistic predicate failed"));
        let recovery = render_extension_recovery_sql(
            "synthetic",
            &script.recovery,
            [0; 8],
            &script.recovery.published_registry_blob,
            true,
        )
        .unwrap();
        assert!(!recovery.contains("dbStruFinal"));
    }

    #[test]
    fn native_851150_refreshes_existing_legacy_marker() {
        let (snapshot, stage, _) = native_fixture();
        let plan = prepare_extension_activation_for_profile(
            MssqlNativePlatformProfile::Platform8_5_1_1150,
            ExtensionActivationMode::Exclusive,
            snapshot,
            &stage,
            ExtensionServiceMarkerSnapshot { present: true },
            true,
        )
        .unwrap();
        assert!(plan.recovery().service_marker_managed);
        let script = render_extension_activation_sql("synthetic", &plan).unwrap();
        assert!(script.sql().contains("dbStruFinal refresh failed"));
        assert!(!script.sql().contains("dbStruFinal insert failed"));
    }

    #[test]
    fn native_851150_stage_rejects_drift_online_and_cross_profile() {
        let (snapshot, stage, _) = native_fixture();
        assert!(
            prepare_extension_activation_for_profile(
                MssqlNativePlatformProfile::Platform8_5_1_1150,
                ExtensionActivationMode::Online,
                snapshot.clone(),
                &stage,
                ExtensionServiceMarkerSnapshot { present: false },
                true
            )
            .is_err()
        );
        assert!(
            prepare_extension_activation_for_profile(
                MssqlNativePlatformProfile::Platform8_3_27_2214,
                ExtensionActivationMode::Exclusive,
                snapshot.clone(),
                &stage,
                ExtensionServiceMarkerSnapshot { present: false },
                true
            )
            .is_err()
        );
        let mut rows = stage.rows().to_vec();
        rows.iter_mut()
            .find(|r| r.logical_name != "configinfo")
            .unwrap()
            .binary_data
            .push(0);
        let broken = ExtensionStagePlan::from_complete_rows(snapshot.extension_id, rows).unwrap();
        assert!(
            prepare_extension_activation_for_profile(
                MssqlNativePlatformProfile::Platform8_5_1_1150,
                ExtensionActivationMode::Exclusive,
                snapshot,
                &broken,
                ExtensionServiceMarkerSnapshot { present: false },
                true
            )
            .is_err()
        );
    }
}
