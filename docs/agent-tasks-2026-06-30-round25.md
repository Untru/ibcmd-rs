# Parallel Agent Tasks - 2026-06-30 Round 25

Base branch: `master` after commit `95aa35d`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database. For base-free work,
only add narrow row generation or precise readiness diagnostics.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Completed dependency:

- Issue #23, shared V8 container layer, is merged and closed.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v26`

Goal: reduce high-volume `Form.xml` debt beyond round 24 table
`SkipOnInput`.

Scope:

- shared form extractor/packer code, primarily `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- no object-name or database-GUID special cases;
- avoid owner metadata/template body changes.

Acceptance:

- inspect the latest full diff and choose one repeated generic form
  element/property gap, preferably a table property still listed as residual
  debt (`CommandBarLocation`, `UseAlternationRowColor`, `InitialTreeView`,
  `RowPictureDataPath`, `UpdateOnDataChange`, `UserSettingsGroup`,
  `AllowGettingCurrentRowURL`) or a cross-owner child item property;
- implement extractor coverage and packer coverage when the existing pack path
  has the same section available;
- add focused tests and at least one regression-style assertion from an observed
  real form shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task B - Issue #15, Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v14`

Goal: reduce metadata XML debt for `Catalogs`, `Documents`, `DataProcessors`,
or `Reports`.

Scope:

- primary file: `src/mssql_dump.rs`;
- avoid form/template body changes;
- no hardcoded real database GUIDs or one-object exceptions.

Acceptance:

- pick one owner kind and one generic metadata layer not already covered by
  prior rounds. Good candidates: document `UseStandardCommands`, default form
  refs, posting/numbering/presentation fields, or report/data processor command
  child refs;
- implement generic parsing/formatting from metadata blob contents;
- add focused unit tests from a realistic blob/text shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task C - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v13`

Goal: reduce template diff debt beyond round 24 DCS `TypeId` normalization.

Scope:

- template source asset extraction/packing code in `src/mssql_dump.rs`,
  `src/module_blob.rs`, and related helpers;
- keep owner metadata child refs out of scope unless needed for selected
  template path resolution.

Acceptance:

- choose one subtype/gap by inspecting
  `E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`,
  preferably DCS area-template blocks or a high-volume MXL formatting property
  not covered by prior rounds;
- implement one generic formatter/parser improvement that can affect multiple
  templates;
- add tests for `Template.xml` metadata and/or `Ext/Template.*` body content,
  depending on the selected subtype;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task D - Issue #18, Partial Metadata / Auxiliary Assets

Worktree: `E:\ibcmd_lab\worktrees\issue-18-partial-metadata-v10`

Goal: reduce debt in one mixed partial family such as `Subsystems`,
`InformationRegisters`, `ExchangePlans`, `AccumulationRegisters`, or
`ChartsOfCharacteristicTypes`.

Scope:

- primary file: `src/mssql_dump.rs`;
- object metadata XML and directly related auxiliary source assets only;
- avoid shared form body work.

Acceptance:

- pick one family and one repeated native XML/asset mismatch from the full diff;
- implement a generic parser/formatter or source asset improvement;
- add focused tests for the chosen owner XML or `Ext/*` asset;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task E - Issue #21, Base-Free Staging Readiness

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v11`

Goal: reduce active base-blob dependencies for one narrow source asset class,
or add a precise audit proving why the selected class remains unsafe.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, and related
  tests;
- avoid export-only metadata XML unless source staging needs path/type
  resolution;
- do not implement blank-database bootstrap.

Acceptance:

- pick one remaining source asset class from readiness/audit reports or source
  staging code that is not `Form.xml`, `Role/Ext/Rights.xml`, or
  `BusinessProcess/Ext/Flowchart.xml`;
- implement base-free row generation if safe, otherwise add a precise blocker
  reason analogous to the existing audits;
- add targeted unit tests around readiness or row generation;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task F - Issue #22, Configuration/CommonAttributes Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v9`

Goal: reduce remaining `Configuration.xml` / `CommonAttributes` metadata XML
debt without generating `ConfigDumpInfo.xml`.

Scope:

- primary file: `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no database-specific GUID literals.

Acceptance:

- pick one root `Configuration.xml` property family or one remaining
  `CommonAttribute` property family not covered by round 24 Content pairs;
- implement generic parsing/formatting from metadata text;
- add focused tests for both source-version and native-shaped XML where
  relevant;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task G - Issue #13, Role Rights.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v8`

Goal: reduce remaining non-order `Roles/*/Ext/Rights.xml` differences after
round 24 object UUID ordering.

Scope:

- `src/mssql_dump.rs`, `src/module_blob.rs`, and focused tests;
- do not change already-green role metadata XML envelope behavior;
- keep UUID ordering from round 24.

Acceptance:

- inspect remaining role rights diffs and choose one repeated non-order class:
  object/reference resolution gap, missing right/value mismatch,
  restriction/trailing layout case, or independent rights flag behavior;
- implement a generic rule for that mismatch;
- add unit tests around the specific rights case fixed;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.
