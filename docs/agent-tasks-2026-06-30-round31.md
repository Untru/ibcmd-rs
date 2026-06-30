# Parallel Agent Tasks - 2026-06-30 Round 31

Base branch: `master` after commit `5b256e4`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Issue #23 remains closed with `status: done`. Round 31 agents may use the
shared `src/v8_container.rs` layer from #23 when blob/container inspection is
useful, but must not reopen #23 or duplicate the old private V8 container code
from `module_blob.rs`.

Round 30 cleared the ExchangePlan `Content.xml` extraction blocker. The full
release source-layout run completed under:

`E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source-report.json`

Important timing evidence from that run:

| Metric | Value |
|---|---:|
| `prepare_indexes_ms` | 17,461 |
| `prepare_object_refs_ms` | 10,621 |
| `fetch_rows_ms` | 5,043 |
| `process_rows_wall_ms` | 15,598 |
| `source_asset_cpu_ms` | 191,976 |
| `source_asset_form_cpu_ms` | 124,212 |
| `source_asset_form_xml_cpu_ms` | 118,809 |
| `source_asset_form_child_items_cpu_ms` | 70,750 |

Treat form source-asset CPU as the first performance target for issue #19.

## Active Batch

### Task A - Issue #19/#16, Form Source-Asset CPU Hotspot

Worktree: `E:\ibcmd_lab\worktrees\issue-19-form-cpu-v1`

Goal: reduce or precisely isolate the full source-layout form extraction CPU
hotspot without changing source semantics.

Scope:

- form source-asset extraction in `src/mssql_dump.rs`;
- form body parsing helpers in `src/module_blob.rs` only if needed;
- performance docs under `docs/`;
- lab output under `E:\ibcmd_lab\perf`;
- no `ConfigDumpInfo.xml`.

Acceptance:

- inspect timing fields from
  `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source-report.json`;
- identify the largest generic avoidable cost under form extraction, especially
  `source_asset_form_xml_cpu_ms` / `source_asset_form_child_items_cpu_ms`;
- implement a low-risk optimization or add finer timing diagnostics that make
  the next optimization obvious;
- if the change is safe, run a focused selected form/source-asset repro and
  save artifacts under `E:\ibcmd_lab\perf`;
- add focused tests when behavior changes;
- update `docs/issue-19-selected-export-bottleneck.md` with the new evidence;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task B - Issue #16, Form.xml Residual Property

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v32`

Goal: reduce one remaining generic `Form.xml` mismatch after
`RowPictureDataPath`.

Scope:

- shared form extraction/packing in `src/mssql_dump.rs` and `src/module_blob.rs`;
- no owner metadata/template body changes.

Acceptance:

- pick one generic residual from `InitialTreeView` or another repeated form
  property identified from the latest diff;
- implement extractor and packer coverage only when the serialized slot or
  property bag can be identified generically;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task C - Issue #15, Object Metadata Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-properties-v24`

Goal: reduce Catalog/Document/DataProcessor/Report metadata XML mismatches after
round 30 child attribute scalar tails and child command property tails.

Scope:

- primary file `src/mssql_dump.rs`;
- no shared Form.xml/template body work.

Acceptance:

- pick one generic owner or child property layer not covered by round 30, such
  as tabular-section properties, nested tabular-section attribute gaps, owner
  scalar settings, default refs, numbering/presentation flags, or command
  property variants;
- parse/format generically from metadata blob contents;
- no hardcoded DB GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task D - Issue #18, Register/Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-metadata-assets-v16`

Goal: reduce one register, subsystem, or ExchangePlan metadata/asset mismatch
not covered by register standard attributes, `Turnovers`, and ExchangePlan
content refs.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on generic metadata XML or source asset behavior;
- avoid shared form/template body work.

Acceptance:

- pick one repeated property family from the latest diff, such as register
  resource/dimension/attribute property bodies, subsystem command-interface
  refs, ExchangePlan content options, or remaining generated-type/internal-info
  variants;
- parse/format from metadata blob contents without database-specific UUIDs;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task E - Issue #21, Base-Free Staging Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v17`

Goal: reduce active base-blob dependencies for one remaining source asset or
body family not covered by previous audits.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

### Task F - Issue #22, Configuration/CommonAttribute Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v15`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after CommonAttribute separation tails.

Scope:

- primary file `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no `ConfigDumpInfo.xml`.

Acceptance:

- pick one generic family such as root storage refs, root form refs, localized
  info, mobile/content blocks, interface refs, style/language collections, or
  unresolved root metadata refs;
- parse/format generically from metadata text;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`.

## Queued Batch

### Task G - Issue #17, Template Body Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v19`

Goal: reduce one remaining DCS/MXL/GraphicalSchema/BinaryData template body
mismatch after DCS current-config prefix normalization.

### Task H - Issue #13, Role Rights.xml Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v14`

Goal: reduce one remaining rights/ref/restriction mismatch after source-ref
based object mapping for import packing.

### Task I - Issue #15, Report/DataProcessor Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-15-report-dp-v25`

Goal: reduce one repeated Report/DataProcessor metadata mismatch not covered by
round 30 child command property tails.

### Task J - Issue #18, Subsystem/ExchangePlan Residual

Worktree: `E:\ibcmd_lab\worktrees\issue-18-subsystem-exchange-v17`

Goal: reduce one repeated Subsystem or ExchangePlan metadata/asset mismatch
after Task D, without touching performance plumbing.
