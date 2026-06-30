# Parallel Agent Tasks - 2026-06-30 Round 19

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `1a5bd43`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v20`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml child item or property edge not handled by previous
  rounds;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, direct
  `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`,
  `TextDocumentField/Width`, `CheckBoxField`, `Pages` / `Page`,
  `PictureDecoration`, `InputField/ReadOnly`, `InputField/DataPath`,
  `LabelField/ShowInHeader`, `InputField/SkipOnInput`, or top-level
  `Button/DefaultButton`;
- good next targets include remaining `Group`, `Button`, `Table`,
  command-bar, input-field button/choice properties, attribute/settings, or
  child-item property edges visible in the parser/formatter;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Partial Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v13`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property/generated-type/asset
  layer for `Subsystem`, `ExchangePlan`, register families, `BusinessProcess`,
  `Task`, or related partial families;
- avoid repeating generated types already covered for ExchangePlan,
  InformationRegister, AccumulationRegister, AccountingRegister,
  CalculationRegister, BusinessProcess, and Task;
- avoid prior child refs, standard form/template refs, Task generated types,
  and ExchangePlan `UseStandardCommands`;
- good targets include `Subsystem` scalar properties/order edges, standard
  commands, owner references, child object refs for register-like families, or
  one concrete property parsed generically from the metadata blob;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #21, Base-Free Source Staging Rows

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v5`

Goal: move one more source staging row toward generation without fetching a
base blob while keeping the import model as staging over an existing compatible
infobase.

Scope:

- primary: `src/mssql.rs`
- focused tests in `src/mssql.rs`

Immediate target:

- close one remaining staging-readiness/preparation gap for a body row that can
  be generated from source bytes and synthetic metadata properties without
  reading active `Config` blobs;
- prefer a body family that currently has weaker or no explicit prep/readiness
  coverage, such as `WSReference` definition, configuration raw/binary asset
  variants, or another suffix-specific row visible in `source_bootstrap_readiness_report`;
- if a candidate still truly requires a base blob (`Form.xml`, `Rights.xml`,
  `Predefined.xml`, `Flowchart.xml`, `metadata_xml`, `versions`), keep the code
  conservative and add only precise diagnostics/tests that document that
  boundary;
- do not alter `module_blob.rs` in this task unless the change is absolutely
  required and small; avoid conflicting with Task A.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
