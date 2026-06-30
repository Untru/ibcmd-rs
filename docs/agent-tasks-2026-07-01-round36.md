# Parallel Agent Tasks - 2026-07-01 Round 36

Base branch: `master` after commit `effc99b`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

Diff-mining aid:

`docs/diff-mining-2026-07-01-round35.md`

The full snapshot was generated before rounds 34 and 35. Do not repeat already
merged round 34/35 slices, especially:

- Catalog `<Owners>`;
- Catalog child attribute property tails / `ChoiceParameters`;
- DCS `calculatedField` `d4p1` current-config type context;
- Form wrapper `55` table `RowFilter xsi:nil`;
- Form `ShowCommandBar=true` default suppression;
- ExchangePlan `Content.xml` metadata-tree ordering;
- Flowchart item `ZOrder`;
- CommonAttribute property-tail `FillValue`;
- MXL unknown format-bit tolerance;
- Role Rights.xml form refs;
- `Ext/ParentConfigurations.bin` raw-deflated staging;
- configuration application module base-free staging audit.

## Active Batch

### Task A - Issue #16, Form Default Suppression Residual v37

Worktree: `E:\ibcmd_lab\worktrees\issue-16-form-defaults-v37`

Goal: reduce one repeated Form XML default-over-emission class after
`ShowCommandBar=true` suppression.

Scope:

- shared form extraction/formatting/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on remaining mined paths such as `Form/Group`,
  `Form/WindowOpeningMode`, `Form/AutoFillCheck`,
  `Table/RowSelectionMode`, `Table/EnableDrag`, or
  `Page/ScrollOnCompress`;
- no template or owner metadata work except tests/fixtures needed for forms.

Acceptance:

- identify a generic native default omission rule;
- suppress or conditionally emit the property only when native would emit it;
- preserve packer behavior for explicit XML values where applicable;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #18, Flowchart Font Residual v21

Worktree: `E:\ibcmd_lab\worktrees\issue-18-flowchart-font-v21`

Goal: reduce one repeated BusinessProcess/GraphicalSchema Flowchart mismatch
after item `ZOrder`.

Scope:

- source asset / metadata XML behavior in `src/mssql_dump.rs` or related helper
  modules;
- focus on `GraphicalSchema/Items/ConnectionLine/Properties/Font` or another
  generic connection-line property from the serialized flowchart body;
- avoid shared Form.xml/template body work.

Acceptance:

- decode, normalize, or suppress one generic connection-line property class;
- no hardcoded database GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task C - Issue #17, MXL Format Property Residual v23

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-format-v23`

Goal: reduce one remaining high-volume MXL `Template.xml` format/property
mismatch after unknown format-bit tolerance.

Scope:

- template body extraction/packing in `src/mssql_dump.rs`, `src/module_blob.rs`,
  or helper modules as needed;
- focus on specific decoded MXL format properties such as `font`,
  `horizontalAlignment`, `verticalAlignment`, `textPlacement`, `fillType`,
  border/color fields, or row/cell format index preservation;
- no form or metadata XML owner work.

Acceptance:

- choose one generic repeated MXL property/index class, not a fixture-specific
  patch;
- preserve or suppress native defaults based on serialized MXL data, not object
  names;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #22, Configuration/CommonAttribute Residual v18

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v18`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after CommonAttribute `FillValue`.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source XML versions `2.20` and `2.21`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one root Configuration/CommonAttribute property family from native
  metadata text;
- parse/format without database-specific GUIDs or object-name exceptions;
- preserve XML ordering around existing emitted properties;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task E - Issue #19, Reusable Diff Mining / Source-Layout Diagnostic v4

Worktree: `E:\ibcmd_lab\worktrees\issue-19-diff-mining-v4`

Goal: turn the ad-hoc round35 XML-path mining into a reusable fast diagnostic or
report path, without changing export semantics.

Scope:

- CLI/report code in `src/cli.rs`, `src/plan.rs`, `src/source_audit.rs`, or a
  small helper module as appropriate;
- docs/tests for the diagnostic;
- no export/import behavior changes.

Acceptance:

- add a reusable way to summarize repeated diff signatures by kind/path or
  document a precise blocker if the existing architecture makes this unsafe;
- avoid loading all XML trees when a bounded/sample mode is requested;
- output should help select agent tasks by high-frequency signatures;
- add focused tests or a documented command example;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task F - Issue #21, Base-Free Staging Residual v22

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v22`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`, `WebService/Ext/Module.bsl`,
  `.bin` V8 module bodies, `Ext/ParentConfigurations.bin`, configuration
  application modules, and `AdditionalIndexes.xml` blocker audit.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

## Queued Batch

### Task G - Issue #13, Role Rights.xml Residual v17

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v17`

Goal: reduce one remaining generic Role `Rights.xml` mismatch after form refs.

### Task H - Issue #15, Object Metadata Owner Residual v30

Worktree: `E:\ibcmd_lab\worktrees\issue-15-owner-metadata-v30`

Goal: reduce one repeated owner-level metadata XML residual, such as Catalog
standard attributes, owner `QuickChoice`, `UseStandardCommands`,
`IncludeHelpInContents`, or `DataLockControlMode`.

