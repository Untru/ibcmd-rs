# Parallel Agent Tasks - 2026-06-30 Round 4

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `b4ff8328ad7c9c3d70987daa82d94dd6a8deefb1`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v5`

Goal: reduce the remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`
- only touch `src/mssql_dump.rs` if the selected fix cannot be covered in the
  shared form body XML compiler/decompiler layer

Immediate target:

- close one recurring Form.xml class such as DynamicList `Settings/Field`,
  `Columns/Column`, column titles/types, or another repeated child/property
  class visible in existing tests/comments;
- keep the change structural and generic, not object-name or GUID specific.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- no large runtime regression in form formatting/parsing paths.

## Task B - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v4`

Goal: reduce remaining `Template/Ext/Template.xml` parity, preferably for
SpreadsheetDocument/MXL or another non-DCS body class.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only for shared MXL/XML pack/unpack helpers
- focused tests

Immediate target:

- close one measurable template body mismatch class, such as MXL print settings,
  column/area formatting, binary/text body routing, or a remaining SKD edge case;
- preserve existing DCS behavior and source version handling.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task C - Issue #13, Role Rights.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v4`

Goal: reduce remaining `Roles/*/Ext/Rights.xml` differences.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only if import packing must understand the same XML
  shape
- focused tests

Immediate target:

- identify and fix one generic role rights ordering, object mapping, header
  flag, or restriction shape class;
- do not revive the earlier reverted two-object ordering experiment unless a
  broader rule justifies it.

Verification:

- `cargo test role_rights --lib` or a narrower focused test;
- `cargo fmt --check`.

## Task D - Issue #14, Remaining Simple Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v4`

Goal: reduce remaining simple/partial metadata XML debt outside groups already
marked done in `docs/export-parity-status.md`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`
- docs/status only if selected verification evidence changes

Immediate target:

- pick one not-done family from the status table, for example
  `XDTOPackages`, `SettingsStorages`, `HTTPServices`, `FilterCriteria`, or a
  remaining `CommonAttributes` property not covered by Issue #22 round 3;
- preserve source XML version behavior `2.20`/`2.21`;
- do not hardcode GUIDs from a real database.

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

- pick one row kind currently reported as `can_generate_without_base_blob` but
  still `current_staging_fetches_base_blob`;
- either remove the unnecessary base fetch in the guarded path, or make the
  audit/report more precise with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
