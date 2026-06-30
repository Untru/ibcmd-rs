# Parallel Agent Tasks - 2026-06-30 Round 17

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `cf3507d`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v18`

Goal: reduce remaining shared `Form.xml` compiler/decompiler parity debt.

Scope:

- `src/module_blob.rs`
- focused tests in `src/module_blob.rs`

Immediate target:

- close one Form.xml child item or property edge not handled by previous
  rounds;
- do not repeat prior fixes for Table/Columns/Column, DynamicList field items,
  `LocationInCommandBar`, Button `ShowTitle`, direct
  `SearchStringAddition/ContextMenu`, `TextDocumentField/ReadOnly`,
  `TextDocumentField/Width`, `CheckBoxField`, `Pages` / `Page`,
  `PictureDecoration`, new `InputField/ReadOnly`, `InputField/DataPath`, or
  `LabelField/ShowInHeader`;
- good next targets include a remaining `LabelField`, `Button`, `Group`,
  `Table`, command-bar, attribute/settings, or child-item property visible in
  the parser/formatter;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Partial Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v11`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property/generated-type/asset
  layer for `Subsystem`, `ExchangePlan`, register families, `BusinessProcess`,
  `Task`, or related partial families;
- avoid repeating previous selected command interface, subsystem child refs,
  register attribute/dimension/resource child objects, generated types already
  covered for ExchangePlan/InformationRegister/AccumulationRegister/
  AccountingRegister/CalculationRegister/BusinessProcess, Task form/template
  child refs, or generic form/template child refs;
- good targets include standard commands, owner references, one concrete
  property parsed generically from the metadata blob, or an auxiliary asset
  edge;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #17, Template / MXL / SKD Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-17-templates-v10`

Goal: reduce remaining `Template/Ext/Template.xml` and template-body parity
debt.

Scope:

- `src/module_blob.rs`
- `src/mssql_dump.rs` only if export formatting needs it
- focused tests

Immediate target:

- close one remaining SpreadsheetDocument/MXL/SKD/template body property or
  style/color mapping not handled by prior rounds;
- avoid repeating previous `style:FieldTextColor`, `style:ButtonTextColor`,
  raw template body, spreadsheet template base-free staging, or HTML template
  staging coverage;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Configuration CommandInterface Base-Free Coverage

Worktree: `D:\ibcmd-rs`

Goal: prove one more staging row kind can use the narrow raw-id
`CommandInterface.xml` base-free pack path while keeping normal staging
conservative.

Scope:

- `src/mssql.rs`
- focused tests

Immediate target:

- add readiness and preparation coverage for root `Configuration`
  `Ext/CommandInterface.xml` or `Ext/MainSectionCommandInterface.xml` when the
  XML contains raw `kind:uuid` command refs;
- keep readable command-reference XML base-backed;
- do not claim full `CommandInterface.xml` bootstrap support.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
