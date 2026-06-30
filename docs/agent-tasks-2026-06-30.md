# Parallel Agent Tasks - 2026-06-30

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope.

## Task A - Issue #16, Forms

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms`

Goal: move `Ext/Form.xml` closer to full export/import parity.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs`
- focused tests in the same files

Immediate target:

- fix one real, reproducible form round-trip/export gap;
- prefer a small proven parser/formatter extension over broad refactors;
- keep existing partial form compiler behavior compatible.

Verification:

- targeted `cargo test` for the changed form tests;
- no lab files outside `E:\ibcmd_lab`.

## Task B - Issue #15, Object Metadata Families

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata`

Goal: improve metadata XML parity for object families still partial.

Scope:

- `src/mssql_dump.rs`
- tests in `src/mssql_dump.rs`

Immediate target:

- use EDT XML-layer notes to add one missing root-level or child-object block
  for Catalog/Document/DataProcessor/Report metadata;
- avoid database-specific GUID literals;
- preserve source version behavior `2.20`/`2.21`.

Verification:

- targeted `cargo test` for the changed family;
- any source-diff/lab data must use `E:\ibcmd_lab`.

## Task C - Issue #18, Registers/Subsystems/Exchange Plans

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems`

Goal: improve parity for register/subsystem/exchange-plan metadata and assets.

Scope:

- `src/mssql_dump.rs`
- tests in `src/mssql_dump.rs`

Immediate target:

- close one measurable mismatch in Subsystems, ExchangePlans or register
  metadata/body assets;
- prefer deterministic source layout/order fixes derived from EDT XML-layer
  behavior.

Verification:

- targeted `cargo test`;
- lab data only under `E:\ibcmd_lab`.

## Task D - Issue #21, Bootstrap Import Architecture

Worktree: `E:\ibcmd_lab\worktrees\issue-21-bootstrap-import`

Goal: start the path from staging-over-existing-base toward import into an empty
infobase.

Scope:

- `src/mssql.rs`
- `src/module_blob.rs`
- `src/cli.rs`
- docs/tests as needed

Immediate target:

- add an explicit dry-run/audit surface that reports which selected source
  objects require existing base blobs and which can be generated without them;
- do not claim full bootstrap import yet;
- make the report actionable for the next compiler work.

Verification:

- targeted `cargo test` for new audit/report code;
- no SQL writes unless explicitly guarded and using lab DBs only.

## Task E - Issue #22, CommonAttributes and Configuration.xml Export

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config`

Goal: start the currently not-started export parity rows.

Scope:

- `src/mssql_dump.rs`
- tests in `src/mssql_dump.rs`
- README/status docs if the ready percentage changes with evidence

Immediate target:

- implement or improve `CommonAttributes` metadata XML export, or add a
  minimal but valid `Configuration.xml` formatter from existing root metadata;
- no ConfigDumpInfo generation.

Verification:

- targeted unit tests;
- optional selected export/source-diff under `E:\ibcmd_lab`.
