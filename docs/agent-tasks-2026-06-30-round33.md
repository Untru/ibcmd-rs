# Parallel Agent Tasks - 2026-06-30 Round 33

Base branch: `master` after commit `df5ea0f`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database, even when working
under Issue #21.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 remains closed with `status: done`. Round 33 agents may use the
shared `src/v8_container.rs` layer when Config/ConfigSave/blob inspection is
useful, but must not reopen #23 or duplicate private V8 container parsing.

Round 32 improved form source-layout CPU and several metadata/staging residuals.
The main remaining blockers are still:

- full Form.xml compilation/decompilation parity;
- broader metadata XML coverage for partial object families;
- template body parity for DCS/MXL/SKD and related body families;
- source staging row generation or precise blockers without reading active base
  blobs.

## Active Batch

### Task A - Issue #13, Role Rights.xml Residual v15

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v15`

Goal: reduce one remaining generic Role `Rights.xml` mismatch after
`Task.AddressingAttribute`.

Scope:

- `src/mssql_dump.rs` role rights extraction;
- `src/module_blob.rs` role rights packing only if the selected residual affects
  import packing;
- no unrelated metadata/form/template changes.

Acceptance:

- pick one generic remaining rights/ref/restriction residual, such as a rights
  object family, nested child ref kind, restriction field variant, or serialized
  tail detail;
- parse/format/pack from source/native data without hardcoded database GUIDs or
  object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #17, Template Body Residual v20

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v20`

Goal: reduce one remaining DCS/MXL/SKD/GraphicalSchema/BinaryData template body
mismatch after case-insensitive DCS generated type id lookup.

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

### Task C - Issue #16, Form.xml Residual v34

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v34`

Goal: reduce one remaining generic `Form.xml` mismatch after `RestoreCurrentRow`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no metadata XML/template body work except tests/fixtures needed for forms.

Acceptance:

- pick one repeated form residual, preferably under wrapper `55`, table service
  children, input fields, or a common child-item option;
- identify the serialized slot/property-bag key generically;
- implement extractor and packer coverage when the base shape is known;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #15, Report/DataProcessor Metadata Residual v27

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v27`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
child command properties, child `LinkByType`, and Document
`IncludeHelpInContents`.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on Report/DataProcessor owner or child metadata XML;
- avoid shared Form.xml/template body work.

Acceptance:

- pick one generic property layer from native metadata text;
- parse/format without database-specific GUIDs or object-name exceptions;
- preserve XML ordering around existing emitted properties;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task E - Issue #18, Register/Subsystem/ExchangePlan Residual v18

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v18`

Goal: reduce one repeated register/subsystem/ExchangePlan mismatch after
ExchangePlan child attributes.

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

### Task F - Issue #21, Base-Free Staging Residual v19

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v19`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`, `WebService/Ext/Module.bsl`,
  `AdditionalIndexes.xml` unmapped blocker audit.

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

### Task G - Issue #22, Configuration/CommonAttribute Residual v17

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v17`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after localized root information fields.

### Task H - Issue #19, Full Source-Layout Performance v3

Worktree: `E:\ibcmd_lab\worktrees\issue-19-source-layout-perf-v3`

Goal: reduce or precisely isolate the next source-layout performance bottleneck
after child-item support-index consolidation.

### Task I - Issue #15, Catalog/Document Metadata Residual v28

Worktree: `E:\ibcmd_lab\worktrees\issue-15-catalog-document-v28`

Goal: reduce one repeated Catalog/Document metadata residual not covered by
Document `IncludeHelpInContents`.

### Task J - Issue #18, Register/Subsystem/ExchangePlan Residual v19

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v19`

Goal: reduce one repeated register/subsystem/ExchangePlan mismatch after Task E.
