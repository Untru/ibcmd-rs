//! Recoverable, idle-infobase checkpoint for the two-cycle live switch: the
//! `ibcmd-mssql-live` crate's `mssql_live_continue`, at its old path.
//!
//! The crate made a few of its crate-visible items `#[doc(hidden)] pub` for
//! the root; the names below re-export each of them crate-visibly, which
//! shadows the glob, so none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_mssql_live::mssql_live_continue::*;

pub(crate) use ibcmd_mssql_live::mssql_live_continue::{
    acquire_pending_live_for_dedicated_session, finish_verified,
};
// Read only by a root test; outside tests it still hides the item from the glob.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use ibcmd_mssql_live::mssql_live_continue::database_ident;

// Root test: the owned lab fixture reads the staged rows with the root's
// `mssql_dump`. Its fixture restates the crate's test helper.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mssql_main_activation::{
        MainActivationRecoverySnapshot, MainStorageRow, quote_string,
    };
    use crate::sql::{SqlBackend, SqlExec, SqlOptions};
    use anyhow::{Result, ensure};
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::Path;
    use uuid::Uuid;

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
}
