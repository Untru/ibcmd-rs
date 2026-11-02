//! `ibcmd-rs mssql-config-apply`: the command line of the own apply
//! (`crate::mssql_config_apply`). It reads the CLI's arguments, the settings
//! and the root's readers (the extensions' adoptions, the stored activation
//! rows), so it stays in the root; `mssql_config_apply::run_command` keeps
//! its path.

use anyhow::{Context, Result, bail};

use crate::mssql_config_apply::{
    AllowRestructure, BackupPolicy, ConfigApplyOptions, Exclusivity, GateChoice, RecoveryBlobs,
    apply_staged_configuration, refusal_report, write_needs_acknowledgement,
};
use crate::sql::SqlExec;

/// `ibcmd-rs mssql-config-apply`: build the SQL handle, run, print the report.
pub fn run_command(args: &crate::cli::MssqlConfigApplyArgs) -> Result<()> {
    use crate::cli::{MssqlConfigApplyExclusivityArg, MssqlConfigApplyRecoveryArg};
    use crate::sql::SqlOptions;

    if write_needs_acknowledgement(&args.database, args.dry_run) && !args.allow_non_lab {
        bail!(
            "--allow-non-lab acknowledgement is required for a database write outside the lab (a database named ibcmd_rs_04_* or ibcmd_rs_05_*) or use --dry-run"
        );
    }
    let password = args.sql_user.as_deref().and_then(|_| {
        args.sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok())
    });
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &args.server,
        user: args.sql_user.as_deref(),
        password: password.as_deref(),
        password_env: &args.sql_pwd_env,
        trust_server_certificate: true,
    })?;
    let mut options = ConfigApplyOptions::new(
        args.database.clone(),
        args.platform_profile,
        crate::restructure::extensions::read_adoptions,
        &crate::mssql_dump::StoredActivationRows,
    );
    options.dry_run = args.dry_run;
    options.rehearse = args.rehearse;
    options.exclusivity = match args.exclusivity {
        MssqlConfigApplyExclusivityArg::Sql => Exclusivity::SqlSessions,
        MssqlConfigApplyExclusivityArg::Assumed => Exclusivity::Assumed,
    };
    options.recovery_dir = args.recovery_dir.clone();
    options.recovery_keep = args.recovery_keep;
    options.recovery_blobs = match args.recovery_blobs {
        MssqlConfigApplyRecoveryArg::Changed => RecoveryBlobs::Changed,
        MssqlConfigApplyRecoveryArg::None => RecoveryBlobs::None,
    };
    options.script_output = args.script_output.clone();
    options.allow_restructure = args
        .allow_restructure
        .map(|crate::cli::MssqlConfigApplyRestructureArg::S1| AllowRestructure::S1);
    options.backup = match (&args.recovery_backup, args.i_have_a_backup) {
        (Some(file), _) => BackupPolicy::File(file.clone()),
        (None, true) => BackupPolicy::Acknowledged,
        (None, false) => BackupPolicy::None,
    };
    options.gate = match args.gate {
        crate::cli::MssqlConfigApplyGateArg::ApplyCheck => GateChoice::ApplyCheck,
        crate::cli::MssqlConfigApplyGateArg::Conservative => GateChoice::Conservative,
    };
    if args.admit_unverified_roles
        && (options.gate != GateChoice::Conservative || options.allow_restructure.is_some())
    {
        bail!(
            "--admit-unverified-roles is a switch of the conservative gate: add --gate conservative"
        );
    }
    options.admit_unverified_roles = args.admit_unverified_roles;
    options.restructure_limit = crate::restructure::size_guard::resolve_limit(
        &crate::settings::Settings::load(None)?,
        args.restructure_limit_rows,
        args.restructure_limit_bytes.as_deref(),
    )?;
    match apply_staged_configuration(&sql, &options) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(path) = &args.report {
                std::fs::write(path, &json)
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }
            println!("{json}");
            Ok(())
        }
        Err(error) => {
            if let Some(refusal) = refusal_report(&error, &args.database) {
                let json = serde_json::to_string_pretty(&refusal)?;
                if let Some(path) = &args.report {
                    std::fs::write(path, &json)
                        .with_context(|| format!("failed to write {}", path.display()))?;
                }
                println!("{json}");
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_dropin_entry_never_asks_for_a_lab_acknowledgement() {
        // The drop-in reaches the apply through apply_staged_configuration, which knows nothing about the lab;
        // only the developer command (run_command) checks the name. The drop-in source must not mention the flag.
        let dropin = include_str!("dropin/apply.rs");
        assert!(
            !dropin.contains(concat!("allow_", "non_lab")),
            "the drop-in apply asks for an acknowledgement"
        );
        assert!(
            !dropin.contains(concat!("allow-", "non-lab")),
            "the drop-in apply asks for an acknowledgement"
        );
        assert!(!dropin.contains("write_needs_acknowledgement"));
    }
}
