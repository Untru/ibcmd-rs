# Parallel Agent Tasks - 2026-06-30 Round 26

Base branch: `master` after commit `23fe7fd`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

## Active Batch

### Task A - Issue #19, Export Performance

Worktree: `E:\ibcmd_lab\worktrees\issue-19-performance-v4`

Goal: reduce or precisely isolate the remaining full-export bottleneck after
the native `bcp` path. Focus on memory and wall time observed in
`mssql-dump-config`.

Scope:

- performance code and diagnostics in `src/mssql_dump.rs` and docs under
  `docs/`;
- no metadata parity behavior changes unless a safe micro-optimization needs
  them;
- do not reintroduce broad `sqlcmd` blob fetches.

Acceptance:

- inspect current timing counters and the latest performance notes;
- implement one safe timing/memory improvement or add a precise diagnostic that
  identifies the next dominant stage;
- add/adjust unit tests where possible;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task B - Issue #16, Form.xml Table Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v27`

Goal: reduce one remaining generic table/form property mismatch.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- avoid owner metadata/template body changes.

Acceptance:

- pick one generic residual from `InitialTreeView`, `RowPictureDataPath`,
  `UpdateOnDataChange`, `UserSettingsGroup`, or `AllowGettingCurrentRowURL`;
- implement extractor coverage and packer coverage only when the existing
  serialized slot/property bag can be identified generically;
- add focused tests around the chosen property;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task C - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v14`

Goal: reduce one high-repeat `Ext/Template.xml` body mismatch.

Scope:

- template body extraction/packing in `src/mssql_dump.rs`, `src/module_blob.rs`,
  and related helpers;
- keep owner metadata child refs out of scope.

Acceptance:

- choose one repeated generic mismatch from the full diff, preferably DCS
  area-template blocks or a MXL formatting/order field not covered by prior
  rounds;
- implement a generic parser/formatter improvement;
- add focused template body tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task D - Issue #18, Register Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-v11`

Goal: reduce one repeated register-family metadata XML mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on InformationRegister or AccumulationRegister owner XML;
- avoid shared form/template body work.

Acceptance:

- pick one generic property layer such as `EditType`, `StandardAttributes`, or
  generated type/property depth visible in the full diff;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused unit tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task E - Issue #15, Catalog Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-catalog-metadata-v15`

Goal: reduce one repeated `Catalog` metadata XML mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- no form/template body changes.

Acceptance:

- pick one generic Catalog metadata layer not already covered, preferably
  StandardAttributes detail, data history/search/input property detail, or child
  command refs;
- parse/format from metadata blob contents;
- add focused unit tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

### Task F - Issue #22, Configuration Root Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-root-v10`

Goal: reduce one root `Configuration.xml` property-family mismatch.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic root family such as default roles, storage refs, form refs,
  mobile functionality, localized info, interface/style/language refs, or
  content blocks;
- parse/format generically from root metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Queued Batch

### Task G - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v9`

Goal: reduce one remaining role object/reference or missing true-right mismatch
after explicit false rights preservation.

### Task H - Issue #21, Base-Free Staging Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v12`

Goal: choose another source asset class and either implement safe base-free row
generation or add precise blocker diagnostics.

### Task I - Issue #18, Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-subsystem-exchange-v12`

Goal: reduce one repeated `Subsystem` or `ExchangePlan` metadata/asset mismatch.

### Task J - Issue #15, Report/DataProcessor Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-metadata-v16`

Goal: reduce one repeated `Report` or `DataProcessor` owner metadata mismatch,
preferably command refs or remaining scalar/property layer.
