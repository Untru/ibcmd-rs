//! Worker handoff is refused until complete loaded-infobase ownership is
//! proved: the `ibcmd-mssql-live` crate's `mssql_worker_switch`, at its old
//! path.

pub use ibcmd_mssql_live::mssql_worker_switch::*;

// Root test: it reads the root's `mssql_apply.rs` and `mssql.rs`.
#[cfg(test)]
mod tests {
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
