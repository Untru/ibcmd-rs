# Parallel Agent Tasks - 2026-07-01 Round 35

Base branch: `master` after commit `1ab3eb2`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

Diff-mining aid:

`docs/diff-mining-2026-07-01-round35.md`

The full snapshot was generated before round 34, so do not repeat these already
merged round 34 slices:

- Catalog `<Owners>`;
- DCS `calculatedField` `d4p1` current-config type context;
- Form wrapper `55` table `RowFilter xsi:nil`;
- ExchangePlan `Content.xml` metadata-tree ordering;
- CommonAttribute property-tail `FillValue`;
- `Ext/ParentConfigurations.bin` raw-deflated staging.

## Active Batch

### Task A - Issue #17, MXL Template Format/Index Residual v22

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-format-v22`

Goal: reduce one high-volume MXL `Template.xml` format/index mismatch from the
round35 mining report.

Scope:

- template body extraction/packing in `src/mssql_dump.rs`, `src/module_blob.rs`,
  or helper modules as needed;
- focus on MXL spreadsheet `document/format/*`, `rowsItem/.../formatIndex`, or
  `columnsItem/.../formatIndex`;
- no form or metadata XML owner work.

Acceptance:

- choose one generic repeated MXL format/index class, not a fixture-specific
  patch;
- preserve or suppress native defaults based on serialized MXL data, not object
  names;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #15, Catalog Child Attribute Property Tails v29

Worktree: `E:\ibcmd_lab\worktrees\issue-15-catalog-attr-tails-v29`

Goal: reduce the high-volume Catalog child `Attribute` metadata property-tail
cluster from the mining report.

Scope:

- primary file `src/mssql_dump.rs`;
- Catalog child `Attribute` and, if naturally shared, tabular-section child
  attributes;
- avoid shared Form.xml/template body work.

Acceptance:

- reuse or generalize existing child attribute tail parsing/formatting for
  Catalog attributes;
- cover native-order property tails such as `PasswordMode`, `Format`,
  `FillValue`, `ChoiceParameters`, `Use`, `Indexing`, `FullTextSearch`, or
  `DataHistory`;
- parse/format without database-specific GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task C - Issue #16, Form Default Suppression Residual v36

Worktree: `E:\ibcmd_lab\worktrees\issue-16-form-defaults-v36`

Goal: reduce one repeated Form XML default-over-emission class from the mining
report.

Scope:

- shared form extraction/formatting/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on paths such as `Form/ShowCommandBar`, `Form/Group`,
  `Form/WindowOpeningMode`, `Form/AutoFillCheck`,
  `Table/RowSelectionMode`, `Table/EnableDrag`, or `Page/ScrollOnCompress`;
- no template or owner metadata work except tests/fixtures needed for forms.

Acceptance:

- identify a native default omission rule generically;
- suppress or conditionally emit the property only when native would emit it;
- preserve packer behavior for explicit XML values where applicable;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #18, Flowchart GraphicalSchema Residual v20

Worktree: `E:\ibcmd_lab\worktrees\issue-18-flowchart-v20`

Goal: reduce one repeated BusinessProcess/GraphicalSchema Flowchart mismatch
from the mining report.

Scope:

- source asset / metadata XML behavior in `src/mssql_dump.rs` or related helper
  modules;
- focus on `GraphicalSchema/Items/ConnectionLine/Properties/ZOrder` or `Font`;
- avoid shared Form.xml/template body work.

Acceptance:

- decode, normalize, or suppress one generic connection-line property class;
- no hardcoded database GUIDs or object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task E - Issue #13, Role Rights.xml Residual v16

Worktree: `E:\ibcmd_lab\worktrees\issue-13-roles-v16`

Goal: reduce one remaining generic Role `Rights.xml` mismatch after HTTPService
URL template method refs.

Scope:

- `src/mssql_dump.rs` role rights extraction;
- `src/module_blob.rs` role rights packing only if the selected residual affects
  import packing;
- no form/template metadata work.

Acceptance:

- pick one generic remaining rights/ref/restriction residual, such as a rights
  object family, nested child ref kind, restriction field variant, or serialized
  tail detail;
- parse/format/pack from source/native data without hardcoded database GUIDs or
  object-name exceptions;
- add focused tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task F - Issue #21, Base-Free Staging Residual v21

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v21`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: `Form.xml`, `Rights.xml`,
  `Flowchart.xml`, `Predefined.xml`, readable `CommandInterface.xml`,
  `HomePageWorkArea.xml`, metadata XML rows, `versions`,
  `FilterCriterion/Ext/ManagerModule.bsl`, `WebService/Ext/Module.bsl`,
  `.bin` V8 module bodies, `Ext/ParentConfigurations.bin`, and
  `AdditionalIndexes.xml` blocker audit.

Acceptance:

- pick one remaining asset/body family from readiness reports or staging code;
- implement safe base-free row generation if source contains enough data,
  otherwise add a precise blocker audit;
- use shared `v8_container` from #23 if the body is a V8 container;
- add targeted readiness/row-generation tests;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

## Queued Batch

### Task G - Issue #22, Configuration/CommonAttribute Residual v18

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-commonattrs-v18`

Goal: reduce one remaining root `Configuration.xml` or `CommonAttribute`
property-family mismatch after CommonAttribute `FillValue`.

### Task H - Issue #19, Diff Mining / Source-Layout Performance v4

Worktree: `E:\ibcmd_lab\worktrees\issue-19-diff-mining-v4`

Goal: turn the ad-hoc round35 XML-path mining into a reusable fast diagnostic or
timing/report path, without changing export semantics.

