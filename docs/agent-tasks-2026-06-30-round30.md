# Parallel Agent Tasks - 2026-06-30 Round 30

Base branch: `master` after commit `ffb78d2`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 was rechecked before this plan. It is closed with `status: done`.
Round 30 agents may use the shared `src/v8_container.rs` layer from #23 when
blob/container inspection is useful, but must not reopen #23 or duplicate the
old private V8 container code from `module_blob.rs`.

Round 29 isolated the current full source-layout timing blocker. A selected
repro for `ExchangePlans\Полный\Ext\Content.xml` now stops at:

`ExchangePlanContent item 0 references unsupported metadata id ff76e85a-6d29-41d3-a83e-f4a34139c6b2`

That id is a direct Constant metadata row named
`ВыгружатьВнутренниеШтрихкодыШтучныхТоваров`. Treat this as the first blocker
for issue #19/#18 planning.

## Active Batch

### Task A - Issue #19/#18, ExchangePlan Constant Ref Index

Worktree: `E:\ibcmd_lab\worktrees\issue-19-exchange-constant-ref-v2`

Goal: remove or precisely isolate the Constant reference-index gap that blocks
ExchangePlan `Ext/Content.xml` source extraction without metadata XML.

Scope:

- source-layout reference index preparation in `src/mssql_dump.rs`;
- ExchangePlan `Content.xml` extraction diagnostics and tests;
- performance docs under `docs/`;
- lab output under `E:\ibcmd_lab\perf`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- reproduce or inspect the selected repro for
  `ExchangePlans\Полный\Ext\Content.xml`;
- determine why direct Constant metadata row
  `ff76e85a-6d29-41d3-a83e-f4a34139c6b2` is absent from the source-layout
  object reference index;
- implement a generic source-reference fix, or add a more precise diagnostic if
  another unsupported shape is discovered;
- rerun the smallest selected ExchangePlan repro and save artifacts under
  `E:\ibcmd_lab\perf`;
- if the selected repro passes, retry the full source-layout timing run only
  until the next blocker or completion;
- add focused tests;
- update `docs/issue-19-selected-export-bottleneck.md` with the new evidence;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task B - Issue #16, Form.xml Residual Property

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v31`

Goal: reduce one remaining generic `Form.xml` mismatch after table
`DefaultItem`.

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

### Task C - Issue #15, Object Child Metadata Properties

Worktree: `E:\ibcmd_lab\worktrees\issue-15-child-properties-v22`

Goal: reduce Catalog/Document/DataProcessor/Report metadata XML mismatches after
child `Attribute` property tails.

Scope:

- primary file `src/mssql_dump.rs`;
- no shared Form.xml/template body work.

Acceptance:

- pick one generic owner or child property layer not covered in round 29, such
  as `TabularSection` scalar properties, nested tabular-section attribute
  tails, command property details, default refs, fill checking, format/edit
  flags, or object scalar settings;
- parse/format generically from metadata blob contents;
- no hardcoded DB GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task D - Issue #18, Register/Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-metadata-assets-v15`

Goal: reduce one register, subsystem, or ExchangePlan metadata/asset mismatch
not covered by round 29 `DataLockControlMode` / `FullTextSearch` and the
ExchangePlan content diagnostics.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on generic metadata XML or source asset behavior;
- avoid shared form/template body work.

Acceptance:

- pick one repeated property family from the latest diff, such as register
  standard attributes, resource/dimension/attribute property bodies, subsystem
  command-interface refs, ExchangePlan content options, or remaining
  generated-type/internal-info variants;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task E - Issue #21, Base-Free Staging Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v16`

Goal: reduce active base-blob dependencies for one remaining source asset or
body family not covered by previous audits.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task F - Issue #22, Configuration/CommonAttribute Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v14`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after native property detail defaults.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as CommonAttribute storage refs, form refs,
  localized info, mobile/content blocks, interface refs, style/language
  collections, or unresolved root metadata refs;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

## Queued Batch

### Task G - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v18`

Goal: reduce one remaining DCS/MXL/GraphicalSchema/BinaryData template body
mismatch after default-only spreadsheet `printSettings` suppression.

### Task H - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v13`

Goal: reduce one remaining rights/ref/restriction mismatch after serialized
object order preservation.

### Task I - Issue #15, Report/DataProcessor Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v23`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
child attribute property tails, Report command child headers, DataProcessor
scalars, or child headers.

### Task J - Issue #18, Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-subsystem-exchange-v16`

Goal: reduce one repeated Subsystem or ExchangePlan metadata/asset mismatch
after Task A/D, without touching performance plumbing.
