//! Closed selection and write policy for native MSSQL storage layouts: the
//! `ibcmd-mssql-live` crate's `mssql_platform_profile`, at its old path.
//!
//! The crate made a few of its crate-visible items `#[doc(hidden)] pub` for
//! the root; the names below re-export each of them crate-visibly, which
//! shadows the glob, so none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_mssql_live::mssql_platform_profile::*;

pub(crate) use ibcmd_mssql_live::mssql_platform_profile::refuse_unbound_live_sessions;
// Read only by the own apply, which is `ibcmd-config-apply`'s; they still hide the items from the glob.
#[expect(
    unused_imports,
    reason = "each re-export hides a widened item from the public glob"
)]
pub(crate) use ibcmd_mssql_live::mssql_platform_profile::{
    SESSION_VISIBILITY_ERROR, SESSION_VISIBILITY_PREDICATE, session_exemption,
    session_visibility_gate,
};

// The crate's test module is not built into the root's tests; the root's
// own tests run their process regressions through this copy of its helper.
#[cfg(test)]
pub(crate) mod tests {
    use std::process::Command;

    // rac_process deliberately owns one original child at a time. Run each
    // process regression in its own harness process so parallel tests cannot
    // create false failures or poison other readiness tests.
    pub(crate) fn run_isolated_profile_test(name: &str) -> bool {
        const MARKER: &str = "IBCMD_ISOLATED_PROFILE_TEST";
        if std::env::var(MARKER).ok().as_deref() == Some(name) {
            return false;
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env(MARKER, name)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "isolated profile regression failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout
                .lines()
                .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;")),
            "isolated profile regression must execute exactly one passing test: {stdout}"
        );
        true
    }
}
