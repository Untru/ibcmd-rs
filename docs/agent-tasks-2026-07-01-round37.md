# Parallel Agent Tasks - 2026-07-01 Round 37

Base branch: `master` after commit `34e6583`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

Round 36 signature-mining report:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\source-diff-signatures-round36.json`

Issue #23 is closed and done. Use `src/v8_container.rs` if a V8 container is
needed; do not reintroduce private V8 container parsing into `module_blob.rs`.

The full snapshot was generated before rounds 34-36. Treat the signature report
as a prioritization aid, not proof that an already-merged slice is still
missing. Do not repeat already merged slices, especially:

- Form `ShowCommandBar=true` and `Page/ScrollOnCompress=false` default
  suppression;
- Flowchart `ZOrder` and connection-line `Font`;
- MXL unknown format-bit tolerance and `textOrientation`;
- Catalog owner refs and known child attribute choice-parameter tail handling;
- CommonAttribute `FillValue` and `FillChecking`;
- Role Rights form refs;
- `CommonCommand/Ext/CommandInterface.xml` raw base-free staging;
- `Ext/ParentConfigurations.bin` and configuration application module
  base-free staging.

## Active Batch

### Task A - Issue #17, MXL Format Table Residual v24

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-format-v24`

Goal: reduce one high-volume MXL `document/format/*` mismatch from the round36
signature report after `textOrientation`.

Scope:

- template body extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on one generic format property class such as `font`, `width`, `height`,
  `horizontalAlignment`, `verticalAlignment`, `textPlacement`, `fillType`, or
  border-related fields;
- no Form.xml or metadata XML owner work.

Acceptance:

- inspect current master first and choose one missing or mis-normalized generic
  MXL format property/index class;
- implement export and import packing if the XML property is source-editable;
- preserve native defaults by data shape, not by object names or database GUIDs;
- add focused extractor and packer tests where applicable;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #17, MXL Format Index / Named Area Residual v25

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-index-area-v25`

Goal: reduce one repeated MXL `formatIndex`, `columnsItem`, or `namedItem/area`
signature without touching unrelated format property decoding.

Scope:

- template body extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on column/row/cell `formatIndex` preservation, column item emission, or
  named area fields (`beginRow`, `beginColumn`, `endRow`, `endColumn`, `type`);
- avoid broad rewrites of the MXL parser.

Acceptance:

- identify one concrete residual class from the signature report examples;
- implement a generic fix that applies across templates;
- add focused tests using synthetic MOXCEL/XML fixtures;
- run targeted tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task C - Issue #16, Form Attribute Settings Field Residual v38

Worktree: `E:\ibcmd_lab\worktrees\issue-16-form-attribute-settings-v38`

Goal: reduce the mined Form.xml `Form/Attributes/Attribute/Settings/Field/*`
left-only residual.

Scope:

- shared Form.xml extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on attribute settings field data path / field list extraction and XML
  packing;
- no template, role, or metadata owner work.

Acceptance:

- inspect current attribute settings parser/formatter;
- add support for one generic `Settings/Field` representation from serialized
  form metadata;
- preserve explicit XML packer behavior;
- add focused tests;
- run targeted form tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #22, Configuration ChildObjects Residual v19

Worktree: `E:\ibcmd_lab\worktrees\issue-22-config-childobjects-v19`

Goal: reduce root `Configuration.xml` child-object residuals for high-count
families such as `CommonModule` or `CommonPicture`.

Scope:

- primary file `src/mssql_dump.rs`;
- source XML versions `2.20` and `2.21` must remain preserved;
- no `ConfigDumpInfo.xml`;
- no broad metadata compiler redesign.

Acceptance:

- inspect current root child-object extraction and the round36
  `configuration_root` signatures;
- add or correct one root child-object family or ordering rule generically;
- no database-specific GUIDs or object-name exceptions;
- add focused tests;
- run targeted `configuration_xml` tests, `cargo fmt --check`, `cargo check`,
  and `git diff --check`;
- commit changes on the assigned branch.

### Task E - Issue #15, Object Metadata Attribute Tail Residual v30

Worktree: `E:\ibcmd_lab\worktrees\issue-15-attribute-tails-v30`

Goal: reduce one high-volume object metadata attribute-tail residual after the
round35 Catalog child attribute work.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on Catalog/Document/DataProcessor/Report child `Attribute` property tail
  parsing/formatting and ordering;
- do not repeat already covered `ChoiceParameters` object-ref resolution unless
  a current regression proves it is still incomplete.

Acceptance:

- inspect current master and choose a concrete residual property family such as
  `ChoiceForm`, `ChoiceParameterLinks`, `CreateOnInput`, `FillValue`,
  `FillChecking`, `FullTextSearch`, `DataHistory`, or ordering around these
  fields;
- implement a generic property-tail fix shared across owners when safe;
- add focused tests;
- run targeted metadata XML tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task F - Issue #21, Base-Free Staging Residual v23

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v23`

Goal: reduce active base-blob dependencies for one remaining source asset/body
family, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/module_blob.rs`, `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already covered classes: Form.xml, Rights.xml, Flowchart.xml,
  Predefined.xml blocker audit, readable CommandInterface.xml blocker audit,
  raw root/CommonCommand CommandInterface.xml, HomePageWorkArea.xml, metadata
  XML rows, `versions`, FilterCriterion manager modules, WebService modules,
  `.bin` V8 module bodies, `Ext/ParentConfigurations.bin`, and configuration
  application modules.

Acceptance:

- identify one source path family that can be generated from source bytes
  without reading the active Config blob, or add a precise readiness blocker if
  the suffix/body shape is not known;
- if staging is implemented, add row-generation tests proving no active base
  blob is fetched;
- if only an audit/blocker is safe, make the blocker precise enough to plan the
  next implementation;
- run targeted staging tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

## Queued / Do Not Start In This Batch

- Issue #13 Role Rights.xml residuals.
- Issue #18 register/subsystem/exchange-plan metadata residuals.
- Issue #19 performance retiming or full diff regeneration.
- Issue #23 V8 container layer; it is already closed/done.

