# Parallel Agent Tasks - 2026-06-30 Round 6

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `0dc1e2964c24c6d8468c4e28dcd345f0d6f2c5e9`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v7`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by round 5 `Table/Columns/Column`,
  such as command bar child options, remaining child item properties, column
  type/title edges, or another repeated compile/decompile property;
- keep the fix structural and generic.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task B - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v5`

Goal: reduce remaining `Template/Ext/Template.xml` parity for non-DCS template
body classes.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only for shared MXL/XML helpers
- focused tests

Immediate target:

- close one measurable MXL/SpreadsheetDocument body mismatch not handled by
  sparse print settings, for example area formatting, column/row sets, selected
  columns, or print-setting subproperties;
- preserve existing DCS behavior and source version handling.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task C - Issue #15, Object Metadata Families

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v6`

Goal: reduce metadata XML parity debt for `Catalogs`, `Documents`,
`DataProcessors`, or `Reports`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- choose one property/child/reference layer not covered by previous
  DataProcessor generated type/default-form work;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task D - Issue #14/#22, Remaining Simple Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v6`

Goal: reduce remaining simple/root metadata XML debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- pick one not-done family/property not handled by recent HTTPService or
  SettingsStorage work, for example `XDTOPackages`, `FilterCriteria`,
  `CommonAttributes`, or root `Configuration.xml` properties;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs;
- do not generate `ConfigDumpInfo.xml`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Local Task - Issue #21, Base-Free Staging Rows

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- focused tests

Immediate target:

- remove the unnecessary base fetch for `configuration_raw_body` or another
  raw/direct body kind that can be built from source bytes;
- update readiness audit with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
