# Parallel Agent Tasks - 2026-06-30 Round 14

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `62ba9c4cf7294974b47b7c7d1d447f3143cda557`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v15`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml child item or property edge not handled by previous
  rounds;
- good targets include one unsupported InputField, Table, Page, Group,
  command-bar, attribute/settings, or child-item property visible in the
  parser/formatter;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, direct
  `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`,
  `TextDocumentField/Width`, `CheckBoxField`, `Pages` / `Page`, or
  `PictureDecoration`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Subsystem/Register/Exchange Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v8`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property layer for
  `Subsystem`, `ExchangePlan`, `InformationRegister`, `AccumulationRegister`,
  `BusinessProcess`, or related partial families;
- good targets include standard commands, child object references, generated
  type edges, command interface ownership references, or one concrete property
  parsed generically from the metadata blob;
- do not repeat previous subsystem selected command interface, register
  attribute/dimension/resource child object, InformationRegister generated
  type, or AccumulationRegister generated type slices;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #14, Simple Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v7`

Goal: reduce remaining simple metadata family parity debt outside the object
families already covered by #15.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing XML child/reference/property layer for `Enum`,
  `XDTOPackage`, `HTTPService`, `WebService`, `SettingsStorage`,
  `DocumentJournal`, `Task`, or `FilterCriterion`;
- good targets include child `Form`/`Template` references, generated types,
  standard attributes, module references, or a concrete property parsed from the
  metadata blob;
- do not repeat previous done simple families or already verified selected
  types;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Base-Free Style Body Staging

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- focused tests in `src/mssql.rs`

Immediate target:

- remove the active `Config` fetch from `Style/Ext/Style.xml` staging;
- use the existing style body packer with no base blob and source-root style
  item resolver;
- update readiness audit so `style_body` reports
  `current_staging_fetches_base_blob=false`;
- do not change this into empty-infobase bootstrap.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
