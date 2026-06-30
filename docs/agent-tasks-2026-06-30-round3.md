# Parallel Agent Tasks - 2026-06-30 Round 3

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `510c8c5`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

## Task A - Issue #15, Object Metadata Families

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v4`

Goal: reduce XML parity debt for `Catalogs`, `Documents`, `DataProcessors`, or
`Reports`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`
- docs/status only if evidence changes

Immediate target:

- add one missing root-level scalar, InternalInfo, child-object or reference
  block for one object family;
- preserve source XML version behavior `2.20`/`2.21`;
- do not hardcode GUIDs from a real database.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task B - Issue #18, Registers/Subsystems/Exchange Plans

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v4`

Goal: reduce parity debt for `Subsystems`, `ExchangePlans`, or register-family
metadata/assets.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one measurable XML/property/asset mismatch;
- keep existing source layout decisions deterministic and source-version aware.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task C - Issue #21, Base-Free Row Generation Readiness

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v4`

Goal: move selected staging rows closer to generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- `src/module_blob.rs`
- `src/cli.rs` only if a CLI/report field changes
- focused tests

Immediate target:

- pick one row kind currently reported as `can_generate_without_base_blob` but
  still `current_staging_fetches_base_blob`, and either remove that unnecessary
  base fetch in a guarded path or make the audit/report more precise with a
  test-backed reason;
- do not attempt full bootstrap import into an empty database.

Verification:

- targeted `cargo test`;
- no SQL writes unless explicitly guarded and lab-only.

## Task D - Issue #22, CommonAttributes and Configuration.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v4`

Goal: reduce remaining CommonAttributes/Configuration.xml XML parity debt.

Scope:

- `src/mssql_dump.rs`
- tests in `src/mssql_dump.rs`
- docs/status if evidence changes

Immediate target:

- add one missing `CommonAttribute` property, root configuration property, or
  reference block;
- no `ConfigDumpInfo` generation.

Verification:

- targeted `cargo test`;
- source version `2.20`/`2.21` must remain correct.

## Local Task - Issue #19, Performance

Worktree: `D:\ibcmd-rs`

Goal: implement one low-risk optimization or measurement for the selected
command-interface export bottleneck described in
`docs/issue-19-selected-export-bottleneck.md`.

Immediate target:

- make `command_refs` reference-index construction cheaper without changing
  output semantics, or add a focused test/measurement that safely narrows the
  next code change.
