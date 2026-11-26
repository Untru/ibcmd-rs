//! Isolate unit tests that exercise the process-owned export adapter state.
//! The CLI/MCP owns that state per process; private row-adapter tests use their
//! own locks, so the public engine mutex cannot synchronize this unit harness.

pub(crate) fn isolated_engine_test(name: &str) -> bool {
    const CHILD: &str = "IBCMD_EXPORT_ENGINE_TEST_CHILD";
    if std::env::var(CHILD).as_deref() == Ok(name) {
        return false;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--color=never"])
        .env(CHILD, name)
        .output()
        .expect("start isolated export-engine test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{name} failed:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(name)
            && stdout.contains("running 1 test")
            && stdout.contains("test result: ok. 1 passed; 0 failed;"),
        "isolated child must execute exactly the requested test {name}:\n{stdout}\n{stderr}"
    );
    true
}
