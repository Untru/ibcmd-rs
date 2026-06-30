# Parallel Agent Tasks - 2026-06-30 Round 12

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `ab20546b58b67eb3768817cc4a82aa559117bd51`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #18, AccumulationRegister Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-accumreg-v8`

Goal: reduce remaining register metadata XML parity debt after the
`InformationRegister` generated-type work.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- emit `InternalInfo` / `xr:GeneratedType` entries for
  `AccumulationRegister` metadata XML using IDs parsed from the SQL metadata
  blob;
- include the known generated type categories for accumulation registers
  without hardcoding real database GUIDs;
- preserve source XML version `2.20`/`2.21`;
- do not repeat the prior `InformationRegister` generated-type fix.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v13`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml child item or property edge not handled by previous
  rounds;
- preferred target: support creation/patching for `Pages` / `Page` form layout
  items if the surrounding packer already has enough structural hooks;
- if that proves too coupled, pick the next smallest unsupported form child
  item/property visible in the parser/formatter;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, direct
  `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`,
  `TextDocumentField/Width`, or `CheckBoxField`.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #22, Configuration/CommonAttribute XML

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v7`

Goal: reduce remaining `Configuration.xml` and `CommonAttributes` metadata XML
debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing `CommonAttribute` property or root `Configuration.xml`
  property layer that can be parsed from the metadata blob generically;
- do not hardcode real database GUIDs;
- keep `ConfigDumpInfo.xml` excluded.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Base-Free ExchangePlan Content Staging

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- focused tests in `src/mssql.rs`

Immediate target:

- remove the active `Config` fetch from `ExchangePlan/Ext/Content.xml` staging;
- use the existing source-root metadata resolver and the content packer with no
  base blob;
- update readiness audit so `exchange_plan_content_body` reports
  `current_staging_fetches_base_blob=false`;
- do not change this into empty-infobase bootstrap.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
