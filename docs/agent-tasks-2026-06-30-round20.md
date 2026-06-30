# Parallel Agent Tasks - 2026-06-30 Round 20

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `88fd0e8`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v21`

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
  `LabelField/ShowInHeader`, `InputField/SkipOnInput`,
  top-level `Button/DefaultButton`, or new `InputField` extended options
  (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`);
- good next targets include `UsualGroup` `Representation` / `Behavior` /
  `ShowTitle`, remaining `InputField` choice/button options, `Table` or
  command-bar item properties, or other high-count child-item properties
  visible in `docs/sfc-coverage-assessment.md`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Partial Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v14`

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
  ExchangePlan `UseStandardCommands`, and Subsystem `UseStandardCommands`;
- good targets include `Subsystem` scalar/order edges other than
  `UseStandardCommands`, standard commands for one partial family, owner
  references, register child object refs, or one concrete property parsed from
  the metadata blob;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #21, Base-Free Source Staging Rows

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v6`

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
- do not repeat WSReference definition `.0`, configuration raw-id
  `CommandInterface.xml` / `MainSectionCommandInterface.xml`, raw template
  bodies, HTML/spreadsheet/binary templates, AdditionalIndexes for Document and
  AccumulationRegister, module bodies, help bodies, style bodies, schedule
  bodies, ExchangePlan Content, or CommonPicture/configuration picture bodies;
- good targets include a configuration-level raw/binary asset variant without
  explicit prep/readiness coverage, a suffix-specific raw body visible in
  `source_bootstrap_readiness_report`, or a precise diagnostic/test for a
  still-base-backed boundary if the row truly requires base blobs;
- keep conservative boundaries for `Form.xml`, `Rights.xml`, `Predefined.xml`,
  `Flowchart.xml`, `metadata_xml`, and `versions`;
- do not alter `module_blob.rs` unless absolutely required and small.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
