# Parallel Agent Tasks - 2026-06-30 Round 10

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `3b674dfb69f6e011c50e3c3d984c08f0daeb6487`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v11`

Goal: reduce the remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by previous rounds;
- prefer another structural layout item property or nested child edge that can
  be compiled from source XML without hardcoded database identifiers;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`,
  direct `SearchStringAddition/ContextMenu`, or
  `TextDocumentField/ReadOnly`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v8`

Goal: reduce remaining `Template/Ext/Template.xml` parity for
`SpreadsheetDocument`/MXL bodies.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only if a shared packer helper is required
- focused tests

Immediate target:

- close one measurable MXL/SpreadsheetDocument mismatch not handled by prior
  sparse print settings, `printArea/columnsID`, nonzero first row scanning, or
  `FieldSelectionBackColor`;
- good targets include remaining style/color mappings, selected row/column
  ranges, row/column dimensions, frozen area edges, or another printed-setting
  subproperty;
- preserve existing DCS behavior and source version handling.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #22, CommonAttributes and Configuration.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v6`

Goal: reduce remaining CommonAttribute or root `Configuration.xml` metadata
parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- choose one missing CommonAttribute property/reference layer or one deeper root
  Configuration property not already covered by header/name/synonym/comment or
  CommonAttribute `AutoUse`;
- preserve source XML version `2.20`/`2.21`;
- do not hardcode real database GUIDs;
- do not generate `ConfigDumpInfo.xml`.

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

- remove the active `Config` query from object/form `Help.xml` staging rows by
  using the deterministic help body id inferred from kind and UUID;
- keep the existing legacy-id resolver tests as extraction/readiness
  documentation, but staging should no longer need that resolver;
- update readiness audit with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
