# Parallel Agent Tasks - 2026-06-30 Round 16

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `cc756f1`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v17`

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
  `PictureDecoration`, new `InputField/ReadOnly`, or `InputField/DataPath`;
- good next targets include a supported `LabelField`, `Button`, `Table`,
  `Group`, command-bar, attribute/settings, or child-item property visible in
  the parser/formatter;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Partial Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v10`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property layer for
  `Subsystem`, `ExchangePlan`, `InformationRegister`, `AccumulationRegister`,
  `CalculationRegister`, `AccountingRegister`, `BusinessProcess`, `Task`, or
  related partial families;
- avoid repeating previous selected command interface, subsystem child refs,
  register attribute/dimension/resource child objects,
  `ExchangePlan`/`InformationRegister`/`AccumulationRegister`/`BusinessProcess`
  generated types, `Task` form/template child refs, or round 15 generic
  form/template child refs;
- good targets include generated type edges for a not-yet-covered register
  family, standard commands, one concrete property parsed generically from the
  metadata blob, or an auxiliary asset edge;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #22, CommonAttributes / Configuration.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-22-commonattrs-config-v2`

Goal: reduce remaining `CommonAttributes` or root `Configuration.xml`
metadata XML debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing root metadata XML child/reference/property layer for
  `CommonAttribute` or `Configuration.xml`;
- avoid repeating previous nested `CommonAttribute` / `CommonModule` child
  headers and existing root header/version support;
- good targets include source-version-safe properties, child object references,
  generated type/index edges, or a concrete property parsed from the metadata
  blob;
- do not generate `ConfigDumpInfo.xml`;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, CommandInterface Base-Free Slice

Worktree: `D:\ibcmd-rs`

Goal: move one more staging row kind toward generation without a base blob while
keeping import as staging over a compatible infobase.

Scope:

- `src/module_blob.rs`
- `src/mssql.rs`
- focused tests

Immediate target:

- add a narrowly-scoped base-free `CommandInterface.xml` pack path for XML that
  already contains raw command references, without changing the existing
  base-patching path for normal staging;
- keep readiness conservative unless the row can genuinely be generated without
  base data;
- do not claim full `CommandInterface.xml` bootstrap support.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
