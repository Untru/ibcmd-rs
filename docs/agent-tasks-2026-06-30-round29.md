# Parallel Agent Tasks - 2026-06-30 Round 29

Base branch: `master` after commit `23216a2`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 was rechecked before this plan. It is closed with `status: done`.
Round 29 agents may use the shared `src/v8_container.rs` layer from #23 when
blob/container inspection is useful, but must not reopen #23 or duplicate the
old private V8 container code from `module_blob.rs`.

Round 28 removed the previous standalone-content blocker for source-only dumps:

`standalone content reference not found: 0014cc2a-b5ed-427d-8ac6-116e92aaa9a4`

The next full source-layout timing run now stops later at:

`failed to extract exchange plan content from source asset ExchangePlans\Полный\Ext\Content.xml`

Treat that as the first blocker for issue #19/#18 planning.

## Active Batch

### Task A - Issue #19/#18, ExchangePlan Content Timing Blocker

Worktree: `E:\ibcmd_lab\worktrees\issue-19-exchange-content-v1`

Goal: remove or precisely isolate the ExchangePlan `Ext/Content.xml` source
asset failure that currently blocks a full source-layout timing report.

Scope:

- ExchangePlan source asset extraction in `src/mssql_dump.rs` and helpers;
- related content XML parsing/formatting tests;
- performance docs under `docs/`;
- lab output under `E:\ibcmd_lab\perf`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- reproduce or inspect the failure for `ExchangePlans\Полный\Ext\Content.xml`;
- either implement a generic extractor fix that preserves source semantics, or
  add a precise diagnostic naming the unsupported content shape;
- if the fix is safe, rerun the smallest selected ExchangePlan repro that
  exercises `Ext/Content.xml` and save artifacts under `E:\ibcmd_lab\perf`;
- if practical, retry the full source-layout timing run only until the next
  blocker or completion;
- add focused tests;
- update `docs/issue-19-selected-export-bottleneck.md` with the new evidence;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task B - Issue #16, Form.xml Residual Property

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v30`

Goal: reduce one remaining generic `Form.xml` property mismatch after wrapper
`55` table `UserSettingsGroup`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no owner metadata/template body changes.

Acceptance:

- pick one generic residual from `InitialTreeView`, `RowPictureDataPath`, or
  another repeated form property identified from the latest diff;
- implement extractor and packer coverage only when the serialized slot or
  property bag can be identified generically;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task C - Issue #18, Register/Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-metadata-assets-v14`

Goal: reduce one register, subsystem, or ExchangePlan metadata/asset mismatch
not covered by generated types, `UseStandardCommands`, `RegisterType`,
`IncludeHelpInContents`, and `Content.xml` file framing.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on generic metadata XML or source asset behavior;
- avoid shared form/template body work.

Acceptance:

- pick one repeated property family from the latest diff, such as register
  `DataLockControlMode`, full-text search flags, subsystem command interface
  refs, ExchangePlan content options, or child resource/dimension/attribute
  property bodies;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task D - Issue #21, Base-Free Staging Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v15`

Goal: reduce active base-blob dependencies for one remaining source asset or
body family not covered by previous audits.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task E - Issue #15, Object Metadata Property Bodies

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-properties-v21`

Goal: reduce Catalog/Document/DataProcessor/Report metadata XML mismatches after
child headers, child attribute type refs, standard attributes, and Report
auxiliary-variant placeholder behavior.

Scope:

- primary file `src/mssql_dump.rs`;
- no shared Form.xml/template body work.

Acceptance:

- pick one generic owner or child property layer for `Attribute`,
  `TabularSection`, nested tabular-section attributes, commands, default refs,
  fill checking, format/edit flags, or object scalar settings;
- parse/format generically from metadata blob contents;
- no hardcoded DB GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task F - Issue #22, Configuration/CommonAttribute Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v13`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after conditional-separation refs.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as CommonAttribute scalar/property details,
  storage refs, form refs, localized info, mobile/content blocks, interface
  refs, style/language collections, or unresolved root metadata refs;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

## Queued Batch

### Task G - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v17`

Goal: reduce one remaining DCS/MXL/GraphicalSchema/BinaryData template body
mismatch after selected owner `type_index` readiness.

### Task H - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v12`

Goal: reduce one remaining rights/ref/restriction mismatch after string-backed
restriction fields.

### Task I - Issue #18, ExchangePlan Content Follow-Up

Worktree: `E:\ibcmd_lab\worktrees\issue-18-exchange-content-v15`

Goal: if Task A only isolates the timing blocker, continue generic
ExchangePlan `Content.xml` parity work without touching performance plumbing.

### Task J - Issue #15, Report/DataProcessor Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v22`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
Report command child headers, DataProcessor scalars, or child headers.
