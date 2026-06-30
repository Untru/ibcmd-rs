# Parallel Agent Tasks - 2026-06-30 Round 9

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `2f55792f7da01de396090a13a4e604a792296107`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v10`

Goal: reduce the remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml class not handled by previous rounds;
- prefer a structural layout item property or child item edge that can be
  compiled from source XML without hardcoded database identifiers;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, or direct
  `SearchStringAddition/ContextMenu`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #19, Selected Export Performance

Worktree: `E:\ibcmd_lab\worktrees\issue-19-selected-perf-v2`

Goal: reduce selected command-interface export time without changing output.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`
- optional update to `docs/issue-19-selected-export-bottleneck.md`

Immediate target:

- implement the next safe optimization described in
  `docs/issue-19-selected-export-bottleneck.md`;
- for selected configuration `.9` command-interface exports, avoid broad
  metadata text inflation when a smaller command-owner lookup is enough;
- preserve the current broad fallback when owner rows cannot be resolved;
- output must remain byte-identical to the current broad-index path.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #15, Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v8`

Goal: reduce remaining metadata XML parity for object families.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing child-reference layer for `Catalog`, `Document`,
  `DataProcessor`, or `Report` metadata XML not covered by previous default
  form, child form, or main data composition schema work;
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

- remove an unnecessary base fetch for a staging row that already has a direct
  source packer, starting with `CommonPicture`/configuration picture bodies or
  another clearly isolated raw/binary body row;
- update readiness audit with a test-backed reason;
- do not attempt full bootstrap import into an empty database.
