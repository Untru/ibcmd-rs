# Parallel Agent Tasks - 2026-06-30 Round 27

Base branch: `master` after commit `7048c77`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 status was rechecked before this plan. It remains closed and verified.
Use the shared `src/v8_container.rs` layer from #23 as infrastructure when it
helps inspect or prove blob/body behavior, but do not reopen #23 or duplicate
its extraction work.

## Active Batch

### Task A - Issue #16, Form.xml Residual Property

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v28`

Goal: reduce one remaining generic `Form.xml` property mismatch beyond
`UpdateOnDataChange=Auto`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no owner metadata/template body changes.

Acceptance:

- pick one generic residual from `InitialTreeView`, `RowPictureDataPath`,
  `UserSettingsGroup`, or `AllowGettingCurrentRowURL`;
- implement extractor coverage and packer coverage only when the serialized
  slot/property bag can be identified generically;
- add focused tests around the selected property;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task B - Issue #18, BusinessProcess/Task Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-workflow-metadata-v13`

Goal: reduce one repeated `BusinessProcess` or `Task` metadata XML mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- owner metadata XML and directly related source asset refs only;
- avoid shared form/template body work.

Acceptance:

- choose one generic mismatch from the full diff, preferably standard
  attributes, generated type details, scalar owner properties, or default form
  refs not already covered;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused unit tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task C - Issue #21, Base-Free Staging with V8 Container Follow-Up

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v13`

Goal: reduce active base-blob dependencies for one additional source asset
class, using the shared #23 `v8_container` module where useful.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, and related tests;
- do not implement blank-database bootstrap;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if the source contains enough data,
  otherwise add a precise blocker audit;
- if V8 container inspection is useful, use the shared #23 module rather than
  reimplementing container parsing;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task D - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v15`

Goal: reduce one repeated `Ext/Template.xml` or non-MXL template body mismatch.

Scope:

- template body extraction/packing in `src/mssql_dump.rs`, `src/module_blob.rs`,
  and related helpers;
- `src/v8_container.rs` may be used only if a template body is a V8 container;
- keep owner metadata child refs out of scope.

Acceptance:

- choose one repeated generic mismatch from the full diff, preferably remaining
  DCS settings/area-template shape, GraphicalSchema, BinaryData/AddIn route
  diagnostics, or MXL format/order field not covered by prior rounds;
- implement a generic parser/formatter or readiness improvement;
- add focused template body tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task E - Issue #15, Report Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-metadata-v17`

Goal: reduce one repeated `Report` owner metadata XML mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- no form/template body changes.

Acceptance:

- pick one generic Report metadata layer not already covered, preferably
  generated object type, `UseStandardCommands`, default form refs,
  `IncludeHelpInContents`, presentations/explanations, or command child refs;
- parse/format from metadata blob contents without hardcoded DB GUIDs;
- add focused unit tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task F - Issue #22, CommonAttribute or Configuration Root Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v11`

Goal: reduce one remaining `CommonAttribute` or root `Configuration.xml`
property-family mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as CommonAttribute scalar/property details,
  default roles, storage refs, form refs, localized info, mobile/content blocks,
  interface refs, or language/style collections;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Queued Batch

### Task G - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v10`

Goal: reduce one remaining role rights mismatch after tail-aware standard
attribute refs. Focus on unhandled tail-coded object classes, missing true
rights, generated extra rights, or restriction/field edge cases.

### Task H - Issue #19, Real Export Timing Follow-Up

Worktree: `E:\ibcmd_lab\worktrees\issue-19-performance-v5`

Goal: run or prepare a real lab timing follow-up for the 256 MiB bcp batch
change. If running the full export is too expensive, add a focused timing report
parser/doc that makes the next run comparable and actionable.

### Task I - Issue #18, Register Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-v12`

Goal: reduce one remaining register-family metadata XML mismatch beyond
generated type and default form refs, preferably standard attributes, scalar
register properties, or child object property bodies.

### Task J - Issue #15, DataProcessor/Catalog Child Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-child-metadata-v18`

Goal: reduce one repeated child metadata mismatch for Catalog or DataProcessor,
preferably `Attribute`, `TabularSection`, command child refs, or another
generic child-object layer.
