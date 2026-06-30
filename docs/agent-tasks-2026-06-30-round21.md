# Parallel Agent Tasks - 2026-06-30 Round 21

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `14652e3`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v22`

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
  top-level `Button/DefaultButton`, new `InputField` extended options
  (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`), or new
  `UsualGroup` `Behavior` / `Representation` / `ShowTitle=false`;
- good next targets include remaining `InputField` choice/button options
  (`DropListButton`, `ClearButton`, `OpenButton`, `ChoiceButton`,
  `ChoiceListButton`, `SpinButton`, `ChoiceButtonRepresentation`),
  `TitleLocation`, `EditMode`, `AutoEditMode`, table/command-bar item
  properties, or another high-count child-item property visible in
  `docs/sfc-coverage-assessment.md`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #15, High-Volume Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v8`

Goal: reduce remaining metadata XML parity debt for high-volume object owners.

Scope:

- primary: `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML property/reference/child-object layer for
  `Catalog`, `Document`, `DataProcessor`, or `Report`;
- avoid repeating prior work for owned `Form` / `Template` child refs and
  already-covered selected Catalog scalar/presentation properties documented in
  `docs/sfc-coverage-assessment.md`;
- good targets include one source-version-safe property parsed from the
  metadata blob, standard commands for one owner family, a child-object family
  not already emitted, or a reference property that can be resolved from source
  indexes without hardcoded database-specific GUIDs;
- keep the change narrowly scoped to one owner family or one generic layer that
  is proven by tests across several families;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #21, Base-Free Source Staging Rows

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v7`

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
  bodies, ExchangePlan Content, CommonPicture/configuration picture bodies, or
  configuration `StandaloneConfigurationContent.bin`;
- good targets include another root configuration asset variant, an explicit
  suffix-specific raw/binary body route that lacks prep/readiness coverage, or
  a precise diagnostic/test for a still-base-backed boundary if the row truly
  requires base blobs;
- keep conservative boundaries for `Form.xml`, `Rights.xml`, `Predefined.xml`,
  `Flowchart.xml`, `metadata_xml`, and `versions`;
- do not alter `module_blob.rs` unless absolutely required and small.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
