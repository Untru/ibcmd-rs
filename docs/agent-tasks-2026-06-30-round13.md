# Parallel Agent Tasks - 2026-06-30 Round 13

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `19f04725ed9e45d516ea85146d18c6aceadce0d9`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v14`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml child item or property edge not handled by previous
  rounds;
- good targets include one unsupported `InputField`, `Table`, `Page`, `Group`,
  command-bar, attribute/settings, or child-item property visible in the
  parser/formatter;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, direct
  `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`,
  `TextDocumentField/Width`, `CheckBoxField`, or `Pages` / `Page` properties
  added in round 12;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #15, Object Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-15-object-metadata-v8`

Goal: reduce remaining metadata XML debt for object families such as Catalogs,
Documents, DataProcessors, and Reports.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property layer for `Catalog`,
  `Document`, `DataProcessor`, or `Report`;
- good targets include generic child object headers, form/template references,
  standard attributes, generated types, or a concrete object property parsed
  from the metadata blob;
- do not repeat previous DataProcessor owned template child refs or already
  covered Catalog/Document/Report generated-type slices;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #17, Template/MXL/SKD Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v9`

Goal: reduce remaining `Template/Ext/Template.xml` and MXL/SKD/template body
parity debt.

Scope:

- `src/module_blob.rs` and/or `src/mssql_dump.rs`
- focused tests in the touched file(s)

Immediate target:

- close one missing MXL/SKD/template formatter or packer edge;
- good targets include one unhandled SpreadsheetDocument XML style, font, line,
  merge/area/print-setting, drawing/picture, SKD normalization, or template
  metadata type edge;
- do not repeat the previous `style:FieldTextColor` fix;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Base-Free Spreadsheet Template Staging

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping the import model as staging over a compatible infobase.

Scope:

- `src/mssql.rs`
- focused tests in `src/mssql.rs`

Immediate target:

- remove the active `Config` fetch from `SpreadsheetDocument`
  `Ext/Template.xml` staging;
- use the MOXCEL packer output directly from source XML;
- update readiness audit so `template_spreadsheet_body` reports
  `current_staging_fetches_base_blob=false`;
- do not change this into empty-infobase bootstrap.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
