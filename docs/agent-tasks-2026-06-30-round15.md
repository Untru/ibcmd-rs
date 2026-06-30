# Parallel Agent Tasks - 2026-06-30 Round 15

All lab exports/imports must be written under `E:\ibcmd_lab`.

Base branch: `master` at `beb3ab8`.

Do not revert unrelated work. Each agent owns its worktree and must keep edits
inside the assigned scope. The current import path remains staging over an
existing compatible infobase. Do not implement or claim import into an empty
infobase in this round.

Do not generate or require `ConfigDumpInfo.xml`.

## Task A - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v16`

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
  `PictureDecoration`, or new `InputField/ReadOnly`;
- good next targets include another explicit `InputField`, `LabelField`,
  `Button`, `Table`, `Group`, command-bar, attribute/settings, or child-item
  property visible in the parser/formatter;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task B - Issue #18, Subsystem/Register/Exchange Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-18-registers-subsystems-v9`

Goal: reduce remaining register/subsystem/exchange-plan metadata XML and
auxiliary asset parity debt.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing metadata XML child/reference/property layer for
  `Subsystem`, `ExchangePlan`, `InformationRegister`, `AccumulationRegister`,
  `BusinessProcess`, `Task`, or related partial families;
- avoid repeating previous subsystem selected command interface, subsystem child
  refs, register attribute/dimension/resource child objects,
  `ExchangePlan`/`InformationRegister`/`AccumulationRegister`/`BusinessProcess`
  generated types, or `Task` form/template child refs;
- good targets include generated type edges for another partial family, standard
  commands, child object references, or one concrete property parsed generically
  from the metadata blob;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Task C - Issue #14, Simple Metadata XML

Worktree: `E:\ibcmd_lab\worktrees\issue-14-simple-metadata-v8`

Goal: reduce remaining simple metadata family parity debt outside the object
families already covered by #15.

Scope:

- `src/mssql_dump.rs`
- focused tests in `src/mssql_dump.rs`

Immediate target:

- close one missing XML child/reference/property layer for `Enum`,
  `XDTOPackage`, `HTTPService`, `WebService`, `SettingsStorage`,
  `DocumentJournal`, `Task`, or `FilterCriterion`;
- avoid repeating already verified done families and the round 14 `Task`
  form/template child refs;
- good targets include child `Form`/`Template` references for another family,
  generated types, standard attributes, module references, or a concrete
  property parsed from the metadata blob;
- do not hardcode real database GUIDs.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.

## Local Task - Issue #21, Base-Free HTML Template Staging

Worktree: `D:\ibcmd-rs`

Goal: prove and tighten one more staging row kind that can be generated without
a base blob while keeping the import model as staging over a compatible
infobase.

Scope:

- `src/mssql.rs`
- focused tests in `src/mssql.rs`

Immediate target:

- add explicit readiness and preparation coverage for `HTMLDocument`
  `Template.html`/`Template.xml` staging;
- ensure the path is visibly independent of active `Config` rows;
- do not change this into empty-infobase bootstrap.

Verification:

- targeted `cargo test`;
- `cargo fmt --check`;
- `git diff --check`.
