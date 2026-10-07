//! Opt-in compact LIVE envelopes. Legacy format-1 bytes and tokens are unchanged.
//! This layer verifies files before the continuation's existing ownership/SQL gates.

use std::path::Path;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::mssql_live_continue::{DatabaseIdentity, LiveArtifact, MAX_ARTIFACT_BYTES};
use crate::mssql_main_activation::MainActivationMode;
use crate::mssql_recovery_artifact::{read_bounded, sibling};

const MAX_COMPACT_MANIFEST: usize = 2 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    integrity_sha256: String,
    payload: Envelope,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    sql_engine_version: String,
    verified_platform_profile: String,
    storage_schema_sha256: String,
    operation: String,
    #[serde(deserialize_with = "deserialize_identity")]
    identity: DatabaseIdentity,
    tail: String,
    recovery_token: String,
    cluster_id: String,
    infobase_id: String,
    recovery_file: String,
    recovery_file_sha256: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn deserialize_identity<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<DatabaseIdentity, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Identity {
        server: String,
        database: String,
        database_guid: String,
        family_guid: String,
        recovery_fork: String,
    }
    let value = Identity::deserialize(deserializer)?;
    Ok(DatabaseIdentity {
        server: value.server,
        database: value.database,
        database_guid: value.database_guid,
        family_guid: value.family_guid,
        recovery_fork: value.recovery_fork,
    })
}

fn recovery_name(token: &str) -> Result<String> {
    ensure!(
        token.len() == 64 && token.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid compact LIVE token"
    );
    Ok(format!(
        "ibcmd-live-{}.recovery.json",
        token.to_ascii_lowercase()
    ))
}

fn encode(payload: Envelope) -> Result<Vec<u8>> {
    let integrity_sha256 = digest(&serde_json::to_vec(&payload)?);
    let bytes = serde_json::to_vec_pretty(&Manifest {
        format: 2,
        integrity_sha256,
        payload,
    })?;
    ensure!(
        bytes.len() <= MAX_COMPACT_MANIFEST,
        "compact LIVE manifest exceeds byte budget"
    );
    Ok(bytes)
}

fn envelope(artifact: &LiveArtifact, recovery_file_sha256: String) -> Result<Envelope> {
    Ok(Envelope {
        sql_engine_version: artifact.sql_engine_version.clone(),
        verified_platform_profile: artifact.verified_platform_profile.clone(),
        storage_schema_sha256: artifact.storage_schema_sha256.clone(),
        operation: artifact.operation.clone(),
        identity: artifact.identity.clone(),
        tail: artifact.tail.clone(),
        recovery_token: artifact.recovery_token.clone(),
        cluster_id: artifact.cluster_id.clone(),
        infobase_id: artifact.infobase_id.clone(),
        recovery_file: recovery_name(&artifact.recovery_token)?,
        recovery_file_sha256,
    })
}

/// Publishes complete pack, recovery sidecar, then LIVE manifest; no SQL is run.
/// An interrupted publication can leave complete unreferenced sidecars.
pub(crate) fn write(path: &Path, artifact: &LiveArtifact) -> Result<()> {
    artifact.validate()?;
    // Check envelope size/path collisions before any file publication.
    let candidate = envelope(artifact, "0".repeat(64))?;
    let sidecar = sibling(path, &candidate.recovery_file)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    ensure!(
        !name.eq_ignore_ascii_case(&candidate.recovery_file)
            && !(name.to_ascii_lowercase().starts_with("ibcmd-recovery-")
                && name.to_ascii_lowercase().ends_with(".pack")),
        "LIVE manifest cannot use a recovery sidecar or pack filename"
    );
    encode(candidate)?;
    crate::mssql_recovery_artifact::write(
        &sidecar,
        &artifact.identity.database,
        MainActivationMode::Live,
        &artifact.recovery,
        &artifact.recovery_token,
    )?;
    let saved = read_bounded(&sidecar, MAX_COMPACT_MANIFEST)?;
    let bytes = encode(envelope(artifact, digest(&saved))?)?;
    crate::mssql_artifact::write_new_or_identical(path, &bytes)?;
    let decoded = read(path)?;
    ensure!(
        serde_json::to_vec(&decoded)? == serde_json::to_vec(artifact)?,
        "compact LIVE publication readback differs"
    );
    Ok(())
}

/// Supports legacy embedded format 1 and compact format 2 without changing tokens.
/// All file integrity checks precede any SQL connection or administrative command.
pub(crate) fn read(path: &Path) -> Result<LiveArtifact> {
    let bytes = read_bounded(path, MAX_ARTIFACT_BYTES as usize)?;
    #[derive(Deserialize)]
    struct Version {
        format: u32,
    }
    let version: Version = serde_json::from_slice(&bytes)?;
    if version.format == 1 {
        let artifact: LiveArtifact = serde_json::from_slice(&bytes)?;
        artifact.validate()?;
        return Ok(artifact);
    }
    ensure!(version.format == 2, "unsupported live artifact format");
    ensure!(
        bytes.len() <= MAX_COMPACT_MANIFEST,
        "compact LIVE manifest exceeds byte budget"
    );
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    let p = manifest.payload;
    ensure!(
        digest(&serde_json::to_vec(&p)?) == manifest.integrity_sha256,
        "compact LIVE envelope integrity mismatch"
    );
    ensure!(
        p.recovery_file == recovery_name(&p.recovery_token)?,
        "compact LIVE recovery basename differs"
    );
    let sidecar = sibling(path, &p.recovery_file)?;
    let saved = read_bounded(&sidecar, MAX_COMPACT_MANIFEST)?;
    ensure!(
        digest(&saved) == p.recovery_file_sha256,
        "compact LIVE recovery sidecar digest differs"
    );
    let (database, mode, recovery) = crate::mssql_recovery_artifact::read(&sidecar)?;
    ensure!(
        database == p.identity.database && mode == MainActivationMode::Live,
        "compact LIVE recovery database/mode differs"
    );
    let artifact = LiveArtifact {
        format: 1,
        sql_engine_version: p.sql_engine_version,
        verified_platform_profile: p.verified_platform_profile,
        storage_schema_sha256: p.storage_schema_sha256,
        operation: p.operation,
        identity: p.identity,
        tail: p.tail,
        recovery_token: p.recovery_token,
        cluster_id: p.cluster_id,
        infobase_id: p.infobase_id,
        recovery,
    };
    artifact.validate()?;
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_main_activation::{MainActivationRecoverySnapshot, MainStorageRow};
    use std::fs;

    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("ibcmd-live-pack-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("operation.live.json")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let target = fs::canonicalize(&self.0).unwrap();
            let root = fs::canonicalize(std::env::temp_dir()).unwrap();
            assert_eq!(target.parent(), Some(root.as_path()));
            assert!(
                target
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("ibcmd-live-pack-test-")
            );
            fs::remove_dir_all(target).unwrap();
        }
    }
    fn fixture() -> LiveArtifact {
        let row = |name: &str, bytes: &[u8]| MainStorageRow {
            file_name: name.into(),
            part_no: 0,
            creation: "4026-10-01 12:00:00.003".into(),
            modified: "4026-10-01 12:00:01.007".into(),
            attributes: 3,
            data_size: bytes.len() as u64,
            binary_data: bytes.to_vec(),
        };
        let recovery = MainActivationRecoverySnapshot {
            old_generation: uuid::Uuid::from_u128(1).to_string(),
            new_generation: uuid::Uuid::from_u128(2).to_string(),
            overwritten_config_rows: vec![row("root", &[0, 255, 42])],
            retained_config_rows: vec![],
            prior_config_dynamically_updated: None,
            prior_params_dynamically_updated: None,
            staged_rows: vec![row("root", &[1, 255, 43])],
        };
        LiveArtifact {
            format: 1,
            sql_engine_version: crate::mssql_live_continue::LIVE_SQL_VERSION.into(),
            verified_platform_profile: "platform-8.3.27.2214".into(),
            storage_schema_sha256: "a".repeat(64),
            operation: uuid::Uuid::from_u128(3).to_string(),
            identity: DatabaseIdentity {
                server: "localhost".into(),
                database: "owned_lab".into(),
                database_guid: uuid::Uuid::from_u128(4).to_string(),
                family_guid: uuid::Uuid::from_u128(5).to_string(),
                recovery_fork: uuid::Uuid::from_u128(6).to_string(),
            },
            tail: std::env::temp_dir()
                .join("live-test.trn")
                .to_str()
                .unwrap()
                .into(),
            recovery_token: digest(&serde_json::to_vec(&recovery).unwrap()).to_ascii_uppercase(),
            cluster_id: uuid::Uuid::from_u128(7).to_string(),
            infobase_id: uuid::Uuid::from_u128(8).to_string(),
            recovery,
        }
    }
    #[test]
    fn compact_live_roundtrip_preserves_legacy_bytes_token_headers_and_sql() {
        let directory = Directory::new();
        let artifact = fixture();
        let legacy = crate::mssql_live_continue::serialize_artifact(&artifact).unwrap();
        let legacy_path = directory.0.join("legacy.live.json");
        fs::write(&legacy_path, &legacy).unwrap();
        write(&directory.path(), &artifact).unwrap();
        write(&directory.path(), &artifact).unwrap();
        for decoded in [
            read(&legacy_path).unwrap(),
            read(&directory.path()).unwrap(),
        ] {
            assert_eq!(
                serde_json::to_vec(&decoded).unwrap(),
                serde_json::to_vec(&artifact).unwrap()
            );
            assert_eq!(decoded.backup_name(1), artifact.backup_name(1));
            assert_eq!(
                crate::mssql_live_continue::render_continue(&decoded, true, false).unwrap(),
                crate::mssql_live_continue::render_continue(&artifact, true, false).unwrap()
            );
        }
        assert_eq!(fs::read(legacy_path).unwrap(), legacy);
        assert!(
            !String::from_utf8(fs::read(directory.path()).unwrap())
                .unwrap()
                .contains("binary_data")
        );
    }
    #[test]
    fn compact_live_rejects_missing_changed_and_foreign_sidecars_before_sql() {
        let directory = Directory::new();
        let artifact = fixture();
        write(&directory.path(), &artifact).unwrap();
        let original = fs::read(directory.path()).unwrap();
        let mut manifest: Manifest = serde_json::from_slice(&original).unwrap();
        let sidecar = directory.0.join(&manifest.payload.recovery_file);
        let saved = fs::read(&sidecar).unwrap();
        fs::remove_file(&sidecar).unwrap();
        assert!(read(&directory.path()).is_err());
        fs::write(&sidecar, b"different").unwrap();
        assert!(read(&directory.path()).is_err());
        fs::write(&sidecar, &saved).unwrap();
        for foreign in ["../outside.json", "F:outside.json", "other.recovery.json"] {
            manifest.payload.recovery_file = foreign.into();
            manifest.integrity_sha256 = digest(&serde_json::to_vec(&manifest.payload).unwrap());
            fs::write(directory.path(), serde_json::to_vec(&manifest).unwrap()).unwrap();
            assert!(read(&directory.path()).is_err());
        }
        manifest = serde_json::from_slice(&original).unwrap();
        let mut unknown: serde_json::Value = serde_json::from_slice(&original).unwrap();
        unknown["payload"]["identity"]["unexpected"] = serde_json::json!(true);
        fs::write(directory.path(), serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert!(read(&directory.path()).is_err());
        manifest.payload.identity.database = "foreign_lab".into();
        manifest.integrity_sha256 = digest(&serde_json::to_vec(&manifest.payload).unwrap());
        fs::write(directory.path(), serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(read(&directory.path()).is_err());
        fs::write(directory.path(), &original).unwrap();
        crate::mssql_recovery_artifact::write(
            &directory.0.join("other.json"),
            &artifact.identity.database,
            MainActivationMode::Online,
            &artifact.recovery,
            &artifact.recovery_token,
        )
        .unwrap();
        let other = fs::read(directory.0.join("other.json")).unwrap();
        fs::write(&sidecar, &other).unwrap();
        manifest = serde_json::from_slice(&original).unwrap();
        manifest.payload.recovery_file_sha256 = digest(&other);
        manifest.integrity_sha256 = digest(&serde_json::to_vec(&manifest.payload).unwrap());
        fs::write(directory.path(), serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(read(&directory.path()).is_err());
        fs::write(&sidecar, &saved).unwrap();
        fs::write(directory.path(), &original).unwrap();
        let recovery: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        let pack = directory
            .0
            .join(recovery["payload"]["pack_file"].as_str().unwrap());
        fs::write(pack, b"truncated").unwrap();
        assert!(read(&directory.path()).is_err());
        let args = crate::mssql_live_continue::LiveContinueArgs {
            artifact: directory.path(),
            database: artifact.identity.database.clone(),
            server: "must-not-connect".into(),
            sql_user: None,
            sql_pwd: None,
            sql_pwd_env: "MUST_NOT_READ_COMPACT_FAILURE_PASSWORD".into(),
            allow_non_lab: true,
            interrupt_sessions: false,
            rac: "must-not-spawn-rac".into(),
            ras_endpoint: "must-not-contact".into(),
            infobase_user: None,
            infobase_pwd: None,
        };
        let error = crate::mssql_live_continue::run(&args).unwrap_err();
        assert_eq!(
            error.to_string(),
            read(&args.artifact).unwrap_err().to_string()
        );
    }
    #[test]
    fn compact_live_missing_pack_names_the_local_file_before_sql() {
        let directory = Directory::new();
        let artifact = fixture();
        write(&directory.path(), &artifact).unwrap();
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(directory.path()).unwrap()).unwrap();
        let sidecar = directory.0.join(&manifest.payload.recovery_file);
        let recovery: serde_json::Value =
            serde_json::from_slice(&fs::read(sidecar).unwrap()).unwrap();
        let pack = directory
            .0
            .join(recovery["payload"]["pack_file"].as_str().unwrap());
        fs::remove_file(&pack).unwrap();
        let args = crate::mssql_live_continue::LiveContinueArgs {
            artifact: directory.path(),
            database: artifact.identity.database.clone(),
            server: "must-not-connect".into(),
            sql_user: None,
            sql_pwd: None,
            sql_pwd_env: "MUST_NOT_READ_MISSING_PACK_PASSWORD".into(),
            allow_non_lab: true,
            interrupt_sessions: false,
            rac: "must-not-spawn-rac".into(),
            ras_endpoint: "must-not-contact".into(),
            infobase_user: None,
            infobase_pwd: None,
        };
        let error = crate::mssql_live_continue::run(&args).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("read compact recovery pack {}", pack.display())
        );
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
    }
    #[test]
    fn compact_live_refuses_invalid_inputs_and_collisions_without_overwrite() {
        let directory = Directory::new();
        let mut artifact = fixture();
        artifact.recovery_token = "0".repeat(64);
        assert!(write(&directory.path(), &artifact).is_err());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
        artifact = fixture();
        let collision = directory
            .0
            .join(recovery_name(&artifact.recovery_token).unwrap());
        assert!(write(&collision, &artifact).is_err());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
        fs::write(directory.path(), b"retained foreign output").unwrap();
        assert!(write(&directory.path(), &artifact).is_err());
        assert_eq!(
            fs::read(directory.path()).unwrap(),
            b"retained foreign output"
        );
    }
    #[test]
    fn compact_live_keeps_large_binary_payloads_out_of_the_envelope() {
        let directory = Directory::new();
        let mut artifact = fixture();
        artifact.recovery.staged_rows[0].binary_data = vec![255; 1024 * 1024];
        artifact.recovery.staged_rows[0].data_size = 1024 * 1024;
        artifact.recovery_token = digest(&serde_json::to_vec(&artifact.recovery).unwrap());
        let legacy = crate::mssql_live_continue::serialize_artifact(&artifact).unwrap();
        write(&directory.path(), &artifact).unwrap();
        assert!(fs::metadata(directory.path()).unwrap().len() < 4096);
        assert!(legacy.len() > 4 * 1024 * 1024);
        assert_eq!(
            read(&directory.path()).unwrap().recovery.staged_rows[0]
                .binary_data
                .len(),
            1024 * 1024
        );
    }
}
