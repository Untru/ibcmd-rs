# Parallel Agent Tasks - 2026-07-01 Round 38

Base branch: `master` after commit `8dcdcb8`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It remains an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

Latest bounded signature-mining report:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\source-diff-signatures-round36.json`

The full snapshot was generated before rounds 34-37. Treat it as a
prioritization aid, not proof that an already-merged slice is still missing.
Do not repeat already merged slices, especially:

- Form `ShowCommandBar=true`, `Page/ScrollOnCompress=false`, and dynamic-list
  `Settings/Field` support;
- Flowchart `ZOrder` and connection-line `Font`;
- MXL unknown format-bit tolerance, `textOrientation`, `textPlacement=Cut`,
  and raw non-1-based column `formatIndex` preservation;
- Catalog/DataProcessor child attribute `ChoiceParameters` and `ChoiceForm`
  tails;
- CommonAttribute `FillValue` and `FillChecking`;
- Role Rights form refs;
- root `Configuration.xml` `CommonPicture` child objects;
- base-free staging for `CommonCommand/Ext/CommandInterface.xml`,
  `Ext/ParentConfigurations.bin`, configuration application modules, and
  `Ext/MainSectionPicture.xml`.

Issue #23 is closed and done. Use `src/v8_container.rs` if a V8 container is
needed; do not reintroduce private V8 container parsing into `module_blob.rs`.

## Active Batch

### Task A - Issue #17, MXL Format Table Residual v26

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-format-v26`

Goal: reduce one remaining high-volume MXL `document/format/*` residual after
the round37 `textPlacement=Cut` fix.

Scope:

- template body extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on one generic format property class such as `width`, `height`, `font`,
  `horizontalAlignment`, `verticalAlignment`, `fillType`, or border fields;
- no Form.xml or metadata owner work.

Acceptance:

- inspect current master first and choose one concrete missing or
  mis-normalized MXL format property;
- implement export and import packing if the XML property is source-editable;
- preserve defaults by serialized shape, not by object names or database GUIDs;
- add focused extractor and packer tests where applicable;
- run targeted MXL/spreadsheet tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task B - Issue #17, MXL Named Area Residual v27

Worktree: `E:\ibcmd_lab\worktrees\issue-17-mxl-named-area-v27`

Goal: reduce the repeated MXL `document/namedItem/area/*` residual.

Scope:

- template body extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- focus on named area fields such as `beginRow`, `beginColumn`, `endRow`,
  `endColumn`, `type`, or native area ordering;
- avoid broad rewrites of the MXL parser and do not repeat the column
  `formatIndex` work from round37.

Acceptance:

- identify one concrete named-area residual using the signature report examples
  or a focused fixture;
- implement a generic fix that applies across templates;
- add focused synthetic MOXCEL/XML tests;
- run targeted MXL/spreadsheet tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task C - Issue #16, Form Default / Structural Residual v39

Worktree: `E:\ibcmd_lab\worktrees\issue-16-form-defaults-v39`

Goal: reduce one repeated Form.xml default-over-emission or structural residual
after round37 dynamic-list `Settings/Field`.

Scope:

- shared Form.xml extraction/packing in `src/mssql_dump.rs` and
  `src/module_blob.rs`;
- prefer one remaining default class from the mined report such as
  `UseInInterfaceCompatibilityMode`, `WindowOpeningMode`, `AutoFillCheck`,
  `Table/SkipOnInput`, or `Table/EnableDrag`;
- no template, role, or metadata owner work.

Acceptance:

- inspect current form property extraction before editing;
- implement one generic native-default suppression or packer/extractor fix;
- preserve explicit non-default XML behavior;
- add focused extractor and packer tests;
- run targeted form tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task D - Issue #15, Object Metadata Attribute Tail Residual v31

Worktree: `E:\ibcmd_lab\worktrees\issue-15-attribute-tails-v31`

Goal: reduce one shared child `Attribute` property-tail residual for
Catalog/Document/DataProcessor/Report after `ChoiceForm`.

Scope:

- primary file `src/mssql_dump.rs`;
- focus on a generic property family such as `ChoiceParameterLinks`,
  `CreateOnInput`, `QuickChoice`, `ChoiceFoldersAndItems`, `Use`,
  `Indexing`, `FullTextSearch`, or `DataHistory`;
- no Form.xml or template body work.

Acceptance:

- inspect current shared child attribute property-tail parser/formatter;
- implement a generic tail fix shared across owners when safe;
- preserve native XML ordering and unresolved references without guessing;
- add focused metadata XML tests;
- run targeted child metadata tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task E - Issue #18, Register / Subsystem / Exchange Residual v22

Worktree: `E:\ibcmd_lab\worktrees\issue-18-metadata-assets-v22`

Goal: reduce one remaining partial metadata or auxiliary asset residual in
register/subsystem/exchange/workflow families.

Scope:

- primary files `src/mssql_dump.rs` and, only if needed, `src/mssql.rs`;
- prefer one generic layer not covered by prior rounds: Subsystem scalar refs,
  InformationRegister owner scalars, ExchangePlan metadata tails, or a remaining
  Flowchart/manual segment/order property;
- no Form.xml or template body work.

Acceptance:

- choose one concrete residual class after checking current master and issue
  history;
- implement a structural fix without database-specific GUIDs or object names;
- add focused tests for source XML versions where applicable;
- run targeted metadata/asset tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

### Task F - Issue #21, Base-Free Staging Residual v24

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v24`

Goal: reduce one remaining active base-blob dependency or make its blocker
precise, while staying within staging over an existing compatible infobase.

Scope:

- `src/mssql.rs`, `src/source_audit.rs`, `src/module_blob.rs`,
  `src/v8_container.rs`, related tests;
- no blank DB bootstrap redesign;
- do not repeat already audited classes: Form.xml, Rights.xml, Flowchart.xml,
  Predefined.xml blockers, readable CommandInterface.xml blockers, raw
  root/CommonCommand CommandInterface.xml, HomePageWorkArea.xml, metadata XML
  blockers, `versions`, FilterCriterion manager modules, WebService modules,
  `.bin` V8 module bodies, `Ext/ParentConfigurations.bin`, configuration
  application modules, and `Ext/MainSectionPicture.xml`.

Acceptance:

- identify one source path family that can be generated from source bytes
  without reading the active Config blob, or add a precise readiness blocker if
  the suffix/body shape is not known;
- if staging is implemented, add row-generation tests proving no active base
  blob is fetched;
- if only an audit/blocker is safe, make the blocker precise enough to plan the
  next implementation;
- run targeted staging/readiness tests, `cargo fmt --check`, `cargo check`, and
  `git diff --check`;
- commit changes on the assigned branch.

## Queued / Do Not Start In This Batch

- Issue #13 Role Rights.xml residuals.
- Issue #19 performance retiming or full diff regeneration.
- Issue #22 additional Configuration.xml/CommonAttribute residuals.
- Issue #23 V8 container layer; it is already closed/done.
