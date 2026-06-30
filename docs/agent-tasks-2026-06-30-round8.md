# Parallel Agent Tasks - 2026-06-30 Round 8

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `c0b0ad2b0aec9ffc86a3357428d54f9b6e5dfd16`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v9`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by previous rounds, preferably another
  extended button/input/table child property or another repeated layout field
  currently dropped by compile/decompile;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, or Button `ShowTitle`;
- keep the fix structural and generic.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v7`

Goal: reduce remaining `Template/Ext/Template.xml` parity for non-DCS template
body classes.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only if a shared MXL/XML helper is truly needed
- focused tests

Immediate target:

- close one measurable MXL/SpreadsheetDocument body mismatch not handled by
  sparse print settings, `printArea/columnsID`, or nonzero first row scanning;
- good targets include row/column set edges, selected rows/columns,
  print-setting subproperties, cell/area formatting, or picture/detail edges;
- preserve existing DCS behavior and source version handling.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #22, CommonAttributes and Configuration.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v5`

Goal: reduce remaining CommonAttribute or root `Configuration.xml` metadata
parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- choose one missing CommonAttribute property/reference layer or one deeper root
  Configuration property not already covered by header/name/synonym/comment;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs;
- do not generate `ConfigDumpInfo.xml`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task D - Issue #13, Role Rights.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v5`

Goal: reduce remaining `Roles/*/Ext/Rights.xml` parity debt.

Scope:

- `src/module_blob.rs`
- `src/mssql_dump.rs` only if extraction formatting needs a matching adjustment
- focused tests

Immediate target:

- close one remaining Rights.xml mismatch not handled by previous header flag
  preservation work, such as one permission flag, ordering, RLS/template edge,
  or another repeated rights-table field;
- preserve existing base-backed role table shape assumptions;
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

- remove the unnecessary base fetch for `CommonTemplate`/`Template`
  `AddIn`/`BinaryData` `Template.bin` staging rows;
- update readiness audit with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
