//! A known property loss is covered only while its surrounding widget survives.
#[path = "common/coverage_paths.rs"]
mod coverage_paths;

use std::{path::PathBuf, process::Command};

#[test]
fn observed_problem_witnesses_keep_their_exact_symptoms() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let python = if cfg!(windows) { "python" } else { "python3" };
    let output = Command::new(python)
        .arg(root.join("tools/coverage/observed_problems.py"))
        .arg("--corpus")
        .arg(coverage_paths::root())
        .arg("--ledger")
        .arg(root.join("tests/fixtures/coverage-records/observed-problems.json"))
        .arg("--binary")
        .arg(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .output()
        .expect("Python is required to validate observed coverage witnesses");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
}
