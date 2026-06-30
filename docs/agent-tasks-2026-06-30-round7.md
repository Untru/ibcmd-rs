# Parallel Agent Tasks - 2026-06-30 Round 7

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `3e208aaf089eaa2769644223028827a87535a031`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v8`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by previous rounds, preferably an
  extended button property beyond `LocationInCommandBar`, a remaining child
  item property, or another repeated layout field that round-trip currently
  drops;
- keep the fix structural and generic.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v6`

Goal: reduce remaining `Template/Ext/Template.xml` parity for non-DCS template
body classes.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only if a shared MXL/XML helper is truly needed
- focused tests

Immediate target:

- close one measurable MXL/SpreadsheetDocument body mismatch not handled by
  sparse print settings or `printArea/columnsID`, for example row/column set
  edges, selected columns/rows, print-setting subproperties, or area formatting;
- preserve existing DCS behavior and source version handling.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #18, Registers/Subsystems/ExchangePlans

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v6`

Goal: reduce remaining parity debt for register, subsystem, or exchange-plan
metadata and auxiliary assets.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- choose one not-done property/child/reference layer for
  `InformationRegisters`, accumulation/accounting/calculation registers,
  `Subsystems`, or `ExchangePlans`;
- do not repeat the already merged subsystem child-object reference work;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task D - Issue #15, Object Metadata Families

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v7`

Goal: reduce metadata XML parity debt for `Catalogs`, `Documents`,
`DataProcessors`, or `Reports`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- choose one property/child/reference layer not covered by prior Catalog form
  refs, DataProcessor default forms, or Report child forms;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Base-Free Staging Rows

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- focused tests

Immediate target:

- remove the unnecessary base fetch for `ScheduledJob/Ext/Schedule.xml`
  staging rows;
- update readiness audit with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
