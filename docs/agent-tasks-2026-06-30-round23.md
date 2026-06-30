# Parallel Agent Tasks - 2026-06-30 Round 23

Base branch: `master` after commit `1a7f8a4`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database. For base-free work,
only add narrow row generation or precise readiness diagnostics.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Completed dependency:

- Issue #23, shared V8 container layer, is already merged and closed. Round 23
  agents may use `src/v8_container.rs` for safe V8 container parsing/building
  instead of duplicating blob container logic.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v24`

Goal: reduce `form` diff debt beyond round 22 `Command/ModifiesSavedData`.

Scope:

- shared form extractor/packer code, primarily `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- no object-name or database-GUID special cases;
- do not touch metadata formatter code unless a form path resolver requires it.

Acceptance:

- inspect the latest full diff and choose one high-volume, generic `Form.xml`
  element/property gap not covered by previous rounds;
- implement extractor coverage and packer coverage when the existing pack path
  has the same section available;
- add focused tests for the chosen property/element and at least one
  regression-style assertion from a real observed shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task B - Issue #15, Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v12`

Goal: reduce metadata XML debt for one of `Catalogs`, `Documents`,
`DataProcessors`, or `Reports`.

Scope:

- primary file: `src/mssql_dump.rs`;
- avoid form/template body changes in this task;
- no hardcoded real database GUIDs or one-object exceptions.

Acceptance:

- pick one owner kind and one generic metadata layer visible in the full diff
  (`ChildObjects`, standard attributes, generated types, presentations,
  standard commands, or another repeated property);
- implement generic parsing/formatting from metadata blob contents;
- add focused unit tests from a realistic blob/text shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task C - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v9`

Goal: reduce template diff debt for one concrete subtype, preferably
`DataCompositionSchema`, MXL/SpreadsheetDocument, HTML/Text/BinaryData, or a
high-volume subtype visible in the full diff.

Scope:

- template source asset extraction/packing code in `src/mssql_dump.rs`,
  `src/module_blob.rs`, and related helpers;
- keep owner metadata child refs out of scope unless needed for selected
  template path resolution.

Acceptance:

- choose a template subtype by inspecting
  `E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`;
- implement one generic formatter/parser improvement that can affect multiple
  templates;
- add tests for `Template.xml` metadata and/or `Ext/Template.*` body content,
  depending on the chosen subtype;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task D - Issue #18, Partial Metadata / Auxiliary Assets

Worktree: `E:\ibcmd_lab\worktrees\issue-18-partial-metadata-v8`

Goal: reduce debt in one mixed partial family such as `InformationRegisters`,
`Subsystems`, `ExchangePlans`, `AccumulationRegisters`, or
`ChartsOfCharacteristicTypes`.

Scope:

- primary file: `src/mssql_dump.rs`;
- object metadata XML and directly related auxiliary source assets only;
- avoid shared form body work.

Acceptance:

- pick one family and one repeated native XML/asset mismatch from the full diff;
- implement a generic parser/formatter or source asset improvement;
- add focused tests for the chosen owner XML or `Ext/*` asset;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task E - Issue #21, Base-Free Staging Readiness

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v9`

Goal: reduce active base-blob dependencies for a narrow non-form source asset
class, or add a precise audit proving why the selected class remains unsafe.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, and related
  tests;
- avoid export-only metadata XML unless source staging needs path/type
  resolution;
- do not implement blank-database bootstrap.

Acceptance:

- pick one remaining source asset class from readiness/audit reports or source
  staging code that is not `Form.xml`;
- implement base-free row generation if safe, otherwise add a precise blocker
  reason analogous to the round 22 form-body audit;
- add targeted unit tests around readiness or row generation;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.
