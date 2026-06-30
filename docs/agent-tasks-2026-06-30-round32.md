# Parallel Agent Tasks - 2026-06-30 Round 32

Base branch: `master` after commit `9090c6f`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database, even when working
under Issue #21.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 remains closed with `status: done`. Round 32 agents may use the
shared `src/v8_container.rs` layer when Config/ConfigSave/blob inspection is
useful, but must not reopen #23 or duplicate private V8 container parsing.

Round 31 reduced several residuals but the main remaining blockers are still:

- full Form.xml compilation/decompilation parity;
- broader metadata XML coverage for partial object families;
- source staging row generation or precise blockers without reading active base
  blobs;
- performance around full source-layout source asset generation.

## Active Batch

### Task A - Issue #16, Form.xml Residual v33

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v33`

Goal: reduce one remaining generic Form.xml mismatch after
`ChoiceFoldersAndItems`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no metadata XML/template body changes except tests/fixtures needed for forms.

Acceptance:

- pick one repeated form residual from the latest full diff or existing unit
  gaps, preferably a property under wrapper `55` or common child item options;
- identify the serialized slot/property-bag key generically;
- implement both extractor and packer coverage when the base shape is known;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task B - Issue #15, Object Metadata Residual v26

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-properties-v26`

Goal: reduce one Catalog/Document/DataProcessor/Report metadata XML mismatch
not covered by tabular-section tails and child `LinkByType`.

Scope:

- primary file `src/mssql_dump.rs`;
- no shared Form.xml/template body work.

Acceptance:

- pick one generic owner or child property layer from the latest diff;
- parse/format from metadata blob contents, without database-specific GUIDs or
  object-name exceptions;
- preserve XML ordering around already emitted properties;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task C - Issue #18, Register/Subsystem/ExchangePlan Residual v17

Worktree: `E:\ibcmd_lab\worktrees\issue-18-subsystem-exchange-v17`

Goal: reduce one register, subsystem, or ExchangePlan metadata/asset mismatch
after generic register child property tails.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on generic metadata XML or source asset behavior;
- avoid shared Form.xml/template body work.

Acceptance:

- pick one repeated property or asset family, such as subsystem command
  interface refs, ExchangePlan content options, generated-type/internal-info
  variants, or remaining register owner fields;
- parse/format from source/native blob contents without hardcoded database
  GUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task D - Issue #21, Base-Free Staging Residual v18

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v18`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`, `WebService/Ext/Module.bsl`.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task E - Issue #22, Configuration/CommonAttribute Residual v16

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v16`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after settings-storage refs.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as root form refs, localized info,
  mobile/content blocks, interface refs, style/language collections, or
  unresolved root metadata refs;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task F - Issue #19, Full Source-Layout Performance v2

Worktree: `E:\ibcmd_lab\worktrees\issue-19-source-layout-perf-v2`

Goal: reduce or precisely isolate the next full source-layout performance
bottleneck after the round 31 `InputField` option-bag cache.

Scope:

- source asset extraction and timing in `src/mssql_dump.rs`;
- form body helpers in `src/module_blob.rs` only if needed;
- performance notes under `docs/`;
- lab output under `E:\ibcmd_lab\perf`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- use the before/after timing reports under `E:\ibcmd_lab\perf` to choose the
  next largest avoidable cost, especially `source_asset_form_xml_cpu_ms` and
  `source_asset_form_child_items_cpu_ms`;
- implement a low-risk optimization or add finer timing diagnostics that make
  the next optimization obvious;
- if running a real repro, keep output under `E:\ibcmd_lab\perf` and verify no
  `ConfigDumpInfo.xml` is generated;
- add focused tests when behavior changes;
- update `docs/issue-19-selected-export-bottleneck.md` with the new evidence;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

## Queued Batch

### Task G - Issue #13, Role Rights.xml Residual v15

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v15`

Goal: reduce one remaining generic Role Rights.xml mismatch after
`Task.AddressingAttribute`.

### Task H - Issue #17, Template Body Residual v20

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v20`

Goal: reduce one remaining DCS/MXL/GraphicalSchema/BinaryData template body
mismatch after case-insensitive DCS type id lookup.

### Task I - Issue #15, Report/DataProcessor Residual v27

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v27`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
child command properties and child `LinkByType`.

### Task J - Issue #18, Register/Subsystem/ExchangePlan Residual v18

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v18`

Goal: reduce one repeated register/subsystem/ExchangePlan mismatch after Task C
without touching performance plumbing.
