# Parallel Agent Tasks - 2026-06-30 Round 28

Base branch: `master` after commit `42acaa8`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Round 27 performance follow-up found that a full source-layout timing run stops
on a standalone-content reference gap:

`standalone content reference not found: 0014cc2a-b5ed-427d-8ac6-116e92aaa9a4`

Treat that as a concrete blocker for issue #19/#21 planning.

## Active Batch

### Task A - Issue #19, Standalone Content Timing Blocker

Worktree: `E:\ibcmd_lab\worktrees\issue-19-standalone-content-v6`

Goal: remove or precisely isolate the standalone-content reference gap that
blocked the full source-layout timing run.

Scope:

- source asset extraction and diagnostics in `src/mssql_dump.rs`;
- performance docs under `docs/`;
- lab output under `E:\ibcmd_lab\perf`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- inspect the failed reference `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` and the
  standalone-content extraction path;
- either implement a generic resolver/fallback that preserves source semantics,
  or add a precise diagnostic that identifies the missing owner/index family and
  lets the next full timing run fail with actionable context;
- if a safe selected repro can be run against `ut_ibcmd`, run it and save
  artifacts under `E:\ibcmd_lab\perf`;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task B - Issue #16, Form.xml Residual Property

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v29`

Goal: reduce one remaining generic `Form.xml` property mismatch beyond
`AllowGettingCurrentRowURL`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no owner metadata/template body changes.

Acceptance:

- pick one generic residual from `InitialTreeView`, `RowPictureDataPath`, or
  `UserSettingsGroup`;
- implement extractor and packer coverage only when the serialized slot or
  property bag can be identified generically;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task C - Issue #18, Register Metadata Property Bodies

Worktree: `E:\ibcmd_lab\worktrees\issue-18-register-properties-v13`

Goal: reduce one register-family metadata XML mismatch beyond generated types,
default form refs, and `IncludeHelpInContents`.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on InformationRegister or AccumulationRegister owner/child XML;
- avoid shared form/template body work.

Acceptance:

- pick one generic property family such as `RegisterType`,
  `DataLockControlMode`, full-text search flags, standard attributes, or child
  resource/dimension/attribute property bodies;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task D - Issue #21, Base-Free Staging Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v14`

Goal: reduce active base-blob dependencies for one source asset class not
covered by previous audits.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task E - Issue #15, Child Metadata Property Bodies

Worktree: `E:\ibcmd_lab\worktrees\issue-15-child-properties-v19`

Goal: extend Catalog/DataProcessor child metadata beyond generic headers.

Scope:

- primary file `src/mssql_dump.rs`;
- no shared Form.xml/template body work.

Acceptance:

- pick one generic child-property layer for `Attribute`, `TabularSection`, or
  nested tabular-section attributes: type refs, fill checking, format/edit
  flags, command refs, or tabular-section-specific scalar properties;
- parse/format generically from metadata blob contents;
- no hardcoded DB GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task F - Issue #22, Configuration/CommonAttribute Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v12`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as CommonAttribute scalar/property details,
  storage refs, form refs, localized info, mobile/content blocks, interface
  refs, style/language collections, or unresolved default-role behavior;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Queued Batch

### Task G - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v16`

Goal: reduce one remaining DCS/MXL/GraphicalSchema/BinaryData template body
mismatch after round 27 TypeId readiness work.

### Task H - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v11`

Goal: reduce one remaining rights/ref/restriction mismatch after nested child
object refs, tail-aware refs, and explicit false rights.

### Task I - Issue #18, Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-subsystem-exchange-v14`

Goal: reduce one repeated `Subsystem` or `ExchangePlan` metadata/asset mismatch
beyond generated types, `UseStandardCommands`, and `Content.xml` AutoRecord.

### Task J - Issue #15, Report/DataProcessor Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v20`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
Report command child headers, DataProcessor scalars, or child headers.
