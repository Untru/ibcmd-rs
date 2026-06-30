# Parallel Agent Tasks - 2026-06-30 Round 11

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `3bb0394ff4dd409a4696383344d80848ad26d2e1`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v12`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by previous rounds;
- prefer a structural layout item property, report-form property, or nested
  child edge that can be compiled from source XML without hardcoded database
  identifiers;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`,
  direct `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`, or
  `TextDocumentField/Width`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #13, Role Rights.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v6`

Goal: reduce remaining `Roles/*/Ext/Rights.xml` parity debt.

Scope:

- `src/module_blob.rs`
- `src/mssql_dump.rs` only if extraction formatting needs a matching adjustment
- focused tests

Immediate target:

- close one remaining Rights.xml mismatch not handled by previous header flag
  or restriction-template work;
- good targets include one repeated rights-table flag, ordering edge, object
  name/reference formatting edge, or another RLS/template detail;
- preserve existing base-backed role table shape assumptions;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #18, Register/Subsystem/Exchange Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v7`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property layer for
  `InformationRegister`, `AccumulationRegister`, `ExchangePlan`, or
  `Subsystem`;
- avoid repeating previous work on subsystem child object references,
  subsystem command interface selected export, or register
  Attribute/Dimension/Resource child objects;
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

- remove an unnecessary active `Config` fetch from object/common-module module
  body staging where `pack_module_blob_bytes` can synthesize default module info
  without a base blob;
- keep the staging model as compatible-base staging, not bootstrap into an
  empty infobase;
- update readiness audit with a test-backed reason.
