//! Worker handoff is refused until complete loaded-infobase ownership is proved.
//!
//! Current connections do not enumerate idle or formerly registered loaded
//! infobases. A supplied RAS endpoint therefore cannot authorize process turn-off.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use serde::Serialize;
use uuid::Uuid;

use crate::mssql_main_activation::MainActivationMode;

const UNSUPPORTED_OWNERSHIP: &str = "worker activation is unsupported: complete loaded-infobase and process-lifetime ownership is not established; connection lists and idle SQL handles cannot authorize worker turn-off";

#[derive(Debug, Clone)]
pub struct WorkerSwitchOptions {
    pub rac: PathBuf,
    pub ras_endpoint: String,
    pub cluster_id: Uuid,
    pub infobase_id: Uuid,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerSwitchReport {
    pub old_process: String,
    pub new_process: String,
    pub switch_ms: u128,
}

#[derive(Debug, Clone)]
pub struct WorkerSwitchPlan;

/// Source orchestration calls this after read-only classification and its no-op
/// return, before staging source or publishing recovery. Standalone
/// activation calls it once the read-only plan establishes whether it is a no-op,
/// before script/recovery publication or SQL mutation. Dry runs and guarded
/// no-op stage consumption perform no worker signal and retain their behavior.
pub fn preflight_worker_execution(
    mode: MainActivationMode,
    dry_run: bool,
    no_op: bool,
) -> Result<()> {
    if mode == MainActivationMode::Worker && !dry_run && !no_op {
        bail!(UNSUPPORTED_OWNERSHIP);
    }
    Ok(())
}

pub fn prepare_dedicated_worker(_options: &WorkerSwitchOptions) -> Result<WorkerSwitchPlan> {
    bail!(UNSUPPORTED_OWNERSHIP)
}

/// Defensive boundary for a caller holding an old/prepared plan. This function
/// starts no utility and sends no server signal, even if SQL already committed.
pub fn switch_dedicated_worker(
    _options: &WorkerSwitchOptions,
    _plan: &WorkerSwitchPlan,
) -> Result<WorkerSwitchReport> {
    bail!(
        "{UNSUPPORTED_OWNERSHIP}; no worker was turned off; if activation SQL already committed, inspect retained recovery before retry"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unavailable_endpoint() -> WorkerSwitchOptions {
        WorkerSwitchOptions {
            rac: PathBuf::from("must-not-spawn-worker-rac-does-not-exist"),
            ras_endpoint: "must-not-connect".into(),
            cluster_id: Uuid::nil(),
            infobase_id: Uuid::nil(),
            timeout: Duration::ZERO,
        }
    }

    #[test]
    fn unsupported_worker_ownership_refuses_without_starting_nonexistent_rac() {
        let error = prepare_dedicated_worker(&unavailable_endpoint()).unwrap_err();
        assert_eq!(error.to_string(), UNSUPPORTED_OWNERSHIP);
        let error =
            switch_dedicated_worker(&unavailable_endpoint(), &WorkerSwitchPlan).unwrap_err();
        assert!(error.to_string().starts_with(UNSUPPORTED_OWNERSHIP));
        assert!(error.to_string().contains("no worker was turned off"));
        assert!(
            error
                .to_string()
                .contains("if activation SQL already committed")
        );
    }

    #[test]
    fn unsupported_worker_ownership_preserves_other_modes_dry_run_and_no_op() {
        for mode in [
            MainActivationMode::Exclusive,
            MainActivationMode::Online,
            MainActivationMode::Live,
            MainActivationMode::Worker,
        ] {
            for dry_run in [false, true] {
                for no_op in [false, true] {
                    let result = preflight_worker_execution(mode, dry_run, no_op);
                    assert_eq!(
                        result.is_err(),
                        mode == MainActivationMode::Worker && !dry_run && !no_op
                    );
                }
            }
        }
    }

    #[test]
    fn unsupported_worker_ownership_dispatch_precedes_staging_and_artifact_publication() {
        let source = include_str!("mssql_apply.rs");
        let apply = source.split("pub fn apply_source_change(").nth(1).unwrap();
        assert!(
            apply.find("classify_original_source_change(").unwrap()
                < apply.find("if no_op {").unwrap()
        );
        assert!(
            apply.find("if no_op {").unwrap()
                < apply.find("preflight_classified_worker_source(").unwrap()
        );
        assert!(
            !apply
                .split("let source_root")
                .next()
                .unwrap()
                .contains("prepare_dedicated_worker(")
        );
        assert!(
            apply.find("preflight_classified_worker_source(").unwrap()
                < apply
                    .find("crate::mssql::stage_source_objects")
                    .expect("main source staging must remain guarded")
        );
        let watch = source.split("pub fn watch_source_changes(").nth(1).unwrap();
        assert!(
            watch.find("preflight_worker_execution(").unwrap()
                < watch.find("verify_mssql_native_profile(").unwrap()
        );
        let activation = include_str!("mssql.rs")
            .split("pub fn activate_staged_main(")
            .nth(1)
            .unwrap();
        assert!(
            activation.find("preflight_worker_execution(").unwrap()
                < activation.find("let artifact_root").unwrap()
        );
        assert!(
            activation.find("preflight_worker_execution(").unwrap()
                < activation.find("write_new_or_identical(").unwrap()
        );
        assert!(
            activation.find("preflight_worker_execution(").unwrap()
                < activation.find("run_sql_file(").unwrap()
        );
    }
}
