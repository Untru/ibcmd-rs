# Parallel Agent Tasks - 2026-06-30 Round 24

Base branch: `master` after commit `821d5e3`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database. For base-free work,
only add narrow row generation or precise readiness diagnostics.

Latest full source-only diff remains:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

The full snapshot percentage in README/status docs must not be changed until a
new full diff is generated after this round.

Completed dependency:

- Issue #23, shared V8 container layer, is merged and closed. Agents may use
  `src/v8_container.rs` instead of duplicating V8 container logic.

## Task A - Issue #13, Role Rights.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v7`

Goal: reduce remaining `Roles/*/Ext/Rights.xml` differences.

Scope:

- `src/mssql_dump.rs`, `src/module_blob.rs`, and focused tests;
- do not change already-green role metadata XML envelope behavior;
- do not reintroduce the previous two-object ordering experiment unless a
  broader rule from multiple examples proves it.

Acceptance:

- inspect several remaining role rights diffs and choose one repeated ordering
  or flag mismatch;
- implement a generic rule for that mismatch;
- add unit tests around the specific rights ordering/flag case;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task B - Issue #22, Configuration/CommonAttributes Metadata

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v8`

Goal: reduce remaining `Configuration.xml` / `CommonAttributes` metadata XML
debt without generating `ConfigDumpInfo.xml`.

Scope:

- primary file: `src/mssql_dump.rs`;
- preserve source-version behavior for 2.20/2.21;
- no database-specific GUID literals.

Acceptance:

- pick one root `Configuration.xml` property family or one remaining
  `CommonAttribute` property family from the latest full diff;
- implement generic parsing/formatting from metadata text;
- add focused tests for both source-version and native-shaped XML where
  relevant;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task C - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v25`

Goal: reduce high-volume `Form.xml` debt beyond round 23 wrapper `55`
`AutoRefresh` / `AutoRefreshPeriod`.

Scope:

- shared form extractor/packer code, primarily `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- no object-name or database-GUID special cases;
- avoid owner metadata/template body changes.

Acceptance:

- inspect the latest full diff and choose one repeated generic form
  element/property gap, preferably another wrapper `55` table property or a
  cross-owner child item property;
- implement extractor coverage and packer coverage when the existing pack path
  has the same section available;
- add focused tests and at least one regression-style assertion from an observed
  real form shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task D - Issue #15, Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v13`

Goal: reduce metadata XML debt for `Catalogs`, `Documents`, `DataProcessors`,
or `Reports`.

Scope:

- primary file: `src/mssql_dump.rs`;
- avoid form/template body changes;
- no hardcoded real database GUIDs or one-object exceptions.

Acceptance:

- pick one owner kind and one generic metadata layer not already covered by
  prior rounds (`Command` child refs, standard attributes, default form refs,
  presentations, standard commands, or another repeated property);
- implement generic parsing/formatting from metadata blob contents;
- add focused unit tests from a realistic blob/text shape;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task E - Issue #17, Template Body Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v12`

Goal: reduce template diff debt for one concrete subtype not covered by round
23 `SpreadsheetDocument verticalUnmerge`.

Scope:

- template source asset extraction/packing code in `src/mssql_dump.rs`,
  `src/module_blob.rs`, and related helpers;
- keep owner metadata child refs out of scope unless needed for selected
  template path resolution.

Acceptance:

- choose one subtype by inspecting
  `E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`,
  preferably `DataCompositionSchema` or another high-volume MXL property;
- implement one generic formatter/parser improvement that can affect multiple
  templates;
- add tests for `Template.xml` metadata and/or `Ext/Template.*` body content,
  depending on the selected subtype;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.

## Task F - Issue #18, Partial Metadata / Auxiliary Assets

Worktree: `E:\ibcmd_lab\worktrees\issue-18-partial-metadata-v9`

Goal: reduce debt in one mixed partial family such as `Subsystems`,
`InformationRegisters`, `ExchangePlans`, `AccumulationRegisters`, or
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

## Task G - Issue #21, Base-Free Staging Readiness

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v10`

Goal: reduce active base-blob dependencies for one narrow non-form source asset
class, or add a precise audit proving why the selected class remains unsafe.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, and related
  tests;
- avoid export-only metadata XML unless source staging needs path/type
  resolution;
- do not implement blank-database bootstrap.

Acceptance:

- pick one remaining source asset class from readiness/audit reports or source
  staging code that is not `Form.xml` and not `Role/Ext/Rights.xml`;
- implement base-free row generation if safe, otherwise add a precise blocker
  reason analogous to the form-body and role-rights audits;
- add targeted unit tests around readiness or row generation;
- run targeted tests, `cargo fmt --check`, `cargo check`, and `git diff --check`.
