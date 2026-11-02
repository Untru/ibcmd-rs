# Reviewed source and actual bounded macOS fixed gate

Functional commit457138bbe66f4f645b342a641957c38f0d901d6e/tree26a775af55023a81750e167a586044ec7c3e2cd9 received independent SOURCE APPROVE before execution. The minimal reviewed #533/directory-witness source chain was imported without sibling CI/settings changes. All four donor blobs match the API owner's receipt; files.rs was not manually edited by the consumer.

After a separate explicit coordinator grant and an inflight-compiler check, jobs2 Rust1.95 ran exactly these sequential commands with an external target/TMP and KEEP_FIXTURES=1:

```
cargo check --locked -p ibcmd-rs --lib --no-default-features
cargo test --locked -p ibcmd-rs --lib directory_census_tests --no-default-features -- --test-threads=1
cargo test --locked -p ibcmd-rs --lib directory_witness_tests --no-default-features -- --test-threads=1
```

All three exited0. The census filter selected14 tests:14 passed/0 failed/0 ignored. The Unix witness filter selected9 tests:9 passed/0 failed/0 ignored. The original seven baseline names are among the fourteen GREEN controls, and their original3935-byte test-source prefix is unchanged. This pairs the observed baseline5 intended assertion RED+2 PASS with the fixed GREEN results. No mutation/setup/unwrap failure, skipped test, warning suppression or retry was used to obtain those results.

All phases have0 owned source/API/test warning blocks. Cargo reported98 general lib warnings and40 lib-test warnings (header counts include a separate generated-summary line). A separate pre-change lib-check warning comparison was not run; these counts do not establish that every warning is pre-existing.

Both test phases' raw Cargo stderr identifies the same root-library executable ibcmd_rs-ba7eb2a6315d773f. Actual and separately retained fixed binary:181068120 bytes, SHA2563819ead9dc551a1ce0f54b9f25982fd5b7483e78f3596b4c73da295db4541767. The separate baseline binary remains180842488 bytes/SHA25653bcdc0a2965f27b03729458296aa4ee992bcd2040a9e8c12fef86706bf0f5c7; its original receiptad5ecf58381c0ae216813fcc05f5afa2333adaf11e12280af2104b6567613cd5 is unchanged.

Fixed raw logs and original receipt are retained locally under lab/logs/issue386-directory-census-fixed-20261024T114929Z. Receipt SHA256b078c8ab407d8156be411e4c307e475369d428e852cff5b81f7399c3a74ff1c6; fixed-executable-audit.json binds the separate local copy. Output was capped at16MiB per stream per phase. Execution session30619 completedexit0 and all compiler/test/root-link resources were explicitly released before further work.

Independent read-only raw audit matched all six stream hashes, exact refs/source blobs, counts and unique test names; rehashed both actual and retained fixed binaries plus the baseline copy; verified the original seven test-prefix bytes and two isolated child PASS receipts (typed cancellation and low-FD). It inspected retained fixtures without executing compiler, harness, VM or native tools. Audit corroborates only the scoped macOS gate.

Still unproven here: broader existing source/apply/editor/MCP regressions, Windows witness7/runtime and Windows FileDbPlan integration, full root/CI gates, native apply, SQL and MCP admission. No Windows/SQL selector, protocol, database setting or policy baseline was changed. No push/PR/publication was performed in this batch.
