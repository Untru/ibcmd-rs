# Parallel Agent Tasks - 2026-07-01 Round 34

Base branch: `master` after commit `447687f`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

Overall full-snapshot readiness excluding `ConfigDumpInfo.xml`: `66.0%`
(`32758 / 49622` byte-identical files), with `16864` files still different.

Issue #23 remains closed with `status: done`. Use the shared
`src/v8_container.rs` layer for V8 container bodies; do not reintroduce private
V8 container parsing in `module_blob.rs`.

The main remaining blockers are still:

- full Form.xml compilation/decompilation parity;
- broader metadata XML coverage for partial object families;
- template body parity for DCS/MXL/SKD and related body families;
- source staging row generation or precise blockers without reading active base
  blobs.

## Active Batch

### Task A - Issue #16, Form.xml Residual v35

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v35`

Goal: reduce one remaining generic `Form.xml` mismatch after wrapper `55` table
`Period`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no owner metadata/template body work except tests/fixtures needed for forms.

Acceptance:

- pick one repeated form residual from the latest full diff, preferably under
  wrapper `55`, table service children, input fields, or a common child-item
  option;
- identify the serialized slot/property-bag key generically;
- implement extractor and packer coverage when the base shape is known;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #15, Catalog/Document Metadata Residual v28

Worktree: `E:\ibcmd_lab\worktrees\issue-15-catalog-document-v28`

Goal: reduce one repeated Catalog/Document metadata mismatch after Document
`IncludeHelpInContents` and DataProcessor child `ChoiceParameters`.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on Catalog/Document owner or child metadata XML;
- avoid shared Form.xml/template body work.

Acceptance:

- pick one generic property layer from native metadata text;
- parse/format without database-specific GUIDs or object-name exceptions;
- preserve XML ordering around existing emitted properties;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task C - Issue #18, Register/Subsystem/ExchangePlan Residual v19

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v19`

Goal: reduce one repeated register/subsystem/ExchangePlan mismatch after
ExchangePlan `ThisNode` and header-relative `UseStandardCommands`.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on generic metadata XML or source asset behavior;
- avoid shared Form.xml/template body work.

Acceptance:

- pick one repeated property or asset family, such as subsystem command
  interface refs, ExchangePlan options/content details, generated type variants,
  or remaining register owner fields;
- parse/format from source/native blob contents without hardcoded database
  GUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #22, Configuration/CommonAttribute Residual v17

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v17`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after localized root information fields.

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

### Task E - Issue #17, Template Body Residual v21

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v21`

Goal: reduce one remaining DCS/MXL/SKD/GraphicalSchema/BinaryData template body
mismatch after DCS `xsi:type` normalization.

Scope:

- template body extraction/packing in `src/mssql_dump.rs`, `src/module_blob.rs`,
  or helper modules as needed;
- no role/form metadata work.

Acceptance:

- pick one repeated template-body mismatch from latest diff or focused tests;
- implement generic parse/format/pack behavior, not a fixture-specific patch;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task F - Issue #21, Base-Free Staging Residual v20

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v20`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`, `WebService/Ext/Module.bsl`,
  `.bin` V8 module bodies, and `AdditionalIndexes.xml` blocker audit.

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

### Task G - Issue #19, Full Source-Layout Performance v3

Worktree: `E:\ibcmd_lab\worktrees\issue-19-source-layout-perf-v3`

Goal: reduce or precisely isolate the next source-layout performance bottleneck
after child-item support-index consolidation.

### Task H - Issue #13, Role Rights.xml Residual v16

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v16`

Goal: reduce one remaining generic Role `Rights.xml` mismatch after HTTPService
URL template method refs.

### Task I - Issue #15, Report/DataProcessor Metadata Residual v29

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v29`

Goal: reduce one repeated Report/DataProcessor metadata residual after child
`ChoiceParameters`.

