# Actual macOS source-only baseline

After the coordinator's explicit chart/root-link FULL RELEASE, one granted jobs1 compile and seven exact harness invocations ran on 2026-10-24. This is baseline behavior, not a fixed acceptance result.

- Commit: c623f6536eaa046ecfeb4ba28887756396a5fc57; tree a4ffaa1905286c8ea6f67405ddedc9496c448c1f, clean throughout. Production behavior is still base6ca; only a cfg(test) declaration and the new controls were added.
- Toolchain: rustc1.95.0, aarch64-apple-darwin; explicit toolchain PATH/RUSTC, CARGO_BUILD_JOBS=1, external target build/issue-386-directory-census and a fresh retained external TMP root, KEEP_FIXTURES=1.
- Command: cargo test --locked -p ibcmd-rs --lib directory_census_tests --no-default-features --no-run --message-format=json; exit0. Cargo JSON identified exactly one root-library test executable.
- Harness: ibcmd_rs-ba7eb2a6315d773f; SHA256 53bcdc0a2965f27b03729458296aa4ee992bcd2040a9e8c12fef86706bf0f5c7. The actual local executable was present when this receipt was recorded; this is not a remotely archived binary claim.
- Each invocation used the same executable, full test name, --exact --nocapture --test-threads=1 --color never; each selected1 test with0 ignored.

| Control | Actual exit | Actual outcome |
|---|---:|---|
| unchanged_directory_census_is_accepted | 0 | PASS |
| empty_directory_addition_invalidates_held_census | 101 | Intended assertion RED |
| empty_directory_membership_removal_invalidates_held_census | 101 | Intended assertion RED |
| empty_directory_rename_invalidates_held_census | 101 | Intended assertion RED |
| nested_empty_directory_rename_invalidates_held_census | 101 | Intended assertion RED |
| case_only_empty_directory_rename_invalidates_held_census | 101 | Intended assertion RED |
| directory_replaced_by_file_is_not_unchanged | 0 | PASS (existing file-count refusal) |

All five failures reached the exact assertion `directory membership changed below traversal limits but revalidation accepted it` in directory_census_tests.rs. Intended filesystem mutations succeeded; no compile, mutation-unwrap, unrelated failure, timeout or output-cap failure was accepted as RED. The runner exited0 only after separately classifying the expected5 RED and2 PASS. Compile output was capped at64MiB stdout/8MiB stderr, each test at256KiB per stream. Cargo JSON contained40 warning records and no warning block naming either owned source path; no separate pre-change warning comparison was run.

Raw stdout/stderr, source hashes, command arguments, exits and environment selectors are retained locally under lab/logs/issue386-directory-census-baseline-20261024T112123Z. The receipt SHA256 is ad5ecf58381c0ae216813fcc05f5afa2333adaf11e12280af2104b6567613cd5. All stream hashes were checked in a read-only self-audit; an independent raw audit was requested. Fixtures were retained without cleanup. The compiler/test/root-link slot was explicitly FULL RELEASEd after process completion.

No directory-witness donor, production fix, full root suite, Windows runtime, native platform, SQL, VM, MCP route or CI execution was included. Fixed revalidation and combined regression acceptance remain open.
