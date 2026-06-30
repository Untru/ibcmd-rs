# Parallel Agent Tasks - 2026-06-30 Round 5

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `859f1c7a89e84064d844692c02eddf206ea5be3e`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v6`

Goal: reduce the remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`
- touch `src/mssql_dump.rs` only if the selected fix cannot be covered in the
  shared form body XML compiler/decompiler layer

Immediate target:

- close a Form.xml class not handled in round 4, such as child item
  `Columns/Column`, column titles/types, command bar child options, or another
  repeated child/property class;
- keep the change structural and generic, not object-name or GUID specific.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task B - Issue #15, Object Metadata Families

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v5`

Goal: reduce XML parity debt for `Catalogs`, `Documents`, `DataProcessors`, or
`Reports`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- add one missing root-level scalar, generated type, InternalInfo piece,
  child-object block, reference block, or default form/list property for one
  object family;
- preserve source XML version behavior `2.20`/`2.21`;
- do not hardcode GUIDs from a real database.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task C - Issue #18, Registers/Subsystems/Exchange Plans

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v5`

Goal: reduce parity debt for `Subsystems`, `ExchangePlans`, or register-family
metadata/assets.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one XML/property/asset mismatch not already covered by prior generated
  type fixes;
- keep output deterministic and source-version aware.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`.

## Task D - Issue #14/#22, Remaining Simple Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v5`

Goal: reduce remaining simple/partial metadata XML debt outside groups already
marked done in `docs/export-parity-status.md`.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- pick one not-done family/property not handled in round 4, for example
  `XDTOPackages`, `SettingsStorages`, `FilterCriteria`, `CommonAttributes`, or
  `Configuration.xml` root properties;
- preserve source XML version behavior `2.20`/`2.21`;
- do not hardcode GUIDs from a real database;
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

- pick one `can_generate_without_base_blob` row kind still reported as
  `current_staging_fetches_base_blob`;
- remove an unnecessary base fetch in a guarded path, or make the audit/report
  more precise with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
