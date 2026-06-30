# Parallel Agent Tasks - 2026-06-30 Round 2

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `bc49b24`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path is staging over an existing
compatible infobase; do not redesign it as bootstrap import into an empty base
in this round.

## Task A - Issue #13, Roles

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v3`

Goal: reduce remaining `Roles/Ext/Rights.xml` export/import mismatches.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`
- docs/status updates only if evidence changes

Immediate target:

- fix one real Rights.xml pack/unpack gap without relying on database-specific
  GUIDs;
- keep the existing base-blob staging model compatible.

Verification:

- targeted `cargo test` for role rights changes;
- `cargo fmt --check`;
- no lab data outside `E:\ibcmd_lab`.

## Task B - Issue #14, Simple Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v3`

Goal: improve metadata XML parity for simple families that remain partial
(`Constants`, `FunctionalOptions`, `DefinedTypes`, `XDTO`, `Commands`,
`Events`).

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs` only if import packing is needed
- focused tests in the same files

Immediate target:

- add one missing property block or child/reference block for a simple metadata
  family;
- preserve source version behavior `2.20`/`2.21`.

Verification:

- targeted `cargo test`;
- no hardcoded real-base GUIDs.

## Task C - Issue #17, Templates

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v3`

Goal: reduce remaining `Template/Ext/Template.xml` and template body parity
mismatches.

Scope:

- `src/mssql_dump.rs`
- `src/module_blob.rs`
- focused template/MXL/SKD tests

Immediate target:

- close one measured template export/import mismatch;
- avoid broad refactors unless they directly remove duplicated template body
  handling.

Verification:

- targeted `cargo test` for the changed template path;
- `cargo fmt --check`.

## Task D - Issue #19, Performance

Worktree: `E:\ibcmd_lab\worktrees\issue-19-performance-v3`

Goal: reduce or explain the current export-time bottleneck without changing
output semantics.

Scope:

- profiling/reporting code in `src/mssql_dump.rs`, `src/mssql.rs`, or docs
- tests where behavior changes

Immediate target:

- identify one concrete serial bottleneck from the current selected-export path
  and either implement a low-risk improvement or add a benchmark/profiling
  report that pinpoints the next safe optimization.

Verification:

- targeted tests for code changes, or a documented profiling command/result;
- lab/profiling output only under `E:\ibcmd_lab`.

## Local Task - Issue #16, Forms

Worktree: `D:\ibcmd-rs`

Goal: continue reducing full `Ext/Form.xml` parity debt.

Immediate target:

- fix the current non-fixture full-test failure
  `mssql_dump::tests::extracts_regular_form_attribute_types_from_body_tail`
  if feasible;
- keep changes scoped to form type extraction/formatting.
