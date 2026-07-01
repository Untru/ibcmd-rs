# Parallel Agent Tasks - 2026-07-01 Round 43

Base branch: `master` after commit `d168663`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`.

The current import target remains staging over an existing compatible
infobase. Do not redesign this round as bootstrap into a blank database.

Issue #23 is closed infrastructure. Use `src/v8_container.rs` for V8 container
work; do not reintroduce private V8 container parsing in `module_blob.rs`.

Use `docs/form-diff-matrix.md` for Form.xml mapping work. Prefer controlled
small XML/blob differences over manual inspection of large forms.

## Tasks

- [ ] Task A - Issue #16/#15/#18/#22, generic diff-matrix corpus tooling

  Goal: generalize the controlled-diff matrix approach beyond one Form pair.
  The first implementation may keep the existing Form layout analyzer, but the
  input/report model should be generic enough to also describe metadata object
  property cases such as "object without attribute" versus "object with one
  attribute/property".

  Ownership:
  - `src/form_matrix.rs`;
  - `src/cli.rs`;
  - `src/main.rs`;
  - `docs/form-diff-matrix.md`.

  Suggested implementation:
  - add an optional repeated or file-based input format for multiple cases, or
    a `--case-root` layout if that fits the current CLI better;
  - include a `kind`/`case_type` field so the report can distinguish Form layout
    cases from future metadata/object property cases;
  - produce one JSON report containing per-case XML differences, blob/layout
    differences and candidate mappings;
  - keep the existing one-pair CLI working.

  Avoid:
  - touching `src/module_blob.rs` Form packer behavior;
  - invoking ibcmd or requiring a live database;
  - generating `ConfigDumpInfo.xml`.

  Acceptance:
  - focused unit tests for multi-case matrix reporting;
  - CLI parser test for the new option;
  - `cargo test form_matrix --lib` or focused filters;
  - `cargo fmt --check` and `git diff --check`.

- [ ] Task B - Issue #16, Form.xml packer parity from matrixable property

  Goal: close one generic Form.xml XML -> layout packing mismatch class using a
  small controlled test. Prefer a property that can later be represented as a
  `form-diff-candidates` matrix case.

  Ownership:
  - `src/module_blob.rs` Form XML parser/packer helpers and focused tests only.

  Avoid:
  - touching `src/mssql_dump/mod.rs`;
  - repeating recent fixes: ShowCommandBar, Page/ScrollOnCompress,
    WindowOpeningMode, dynamic-list Settings/Field, Table/SkipOnInput,
    TextDocumentField/ReadOnly, CheckBoxField/ShowInHeader, Table/DataPath;
  - database-specific GUID/name hardcoding.

  Acceptance:
  - one focused test proving XML -> native layout packing for the selected
    property or child shape;
  - `cargo test <focused-filter> --lib`, `cargo fmt --check`,
    and `git diff --check`.

- [ ] Task C - Issue #17, MXL/template format parity

  Goal: close one remaining repeated MXL `document/format/*`, row/cell
  `formatIndex`, or namedItem/area mismatch class from the current signature
  list.

  Ownership:
  - MXL/SpreadsheetDocument extraction and packing code in
    `src/mssql_dump/mod.rs` MXL region and `src/module_blob.rs` spreadsheet
    packer region only.

  Avoid:
  - Form.xml work;
  - metadata XML work;
  - repeating recent fixes: style refs `-13`, `-15`, `-21`, `-25` through
    `-38`, text orientation, text placement, empty format slots, named-area
    drawing filtering, number-format string table, verticalAlignment `Bottom`.

  Acceptance:
  - focused extractor/packer or stable round-trip test for the selected MXL
    shape;
  - `cargo test moxel --lib` and/or `cargo test spreadsheet --lib`;
  - `cargo fmt --check` and `git diff --check`.

- [ ] Task D - Issue #15/#18/#22, metadata XML property-layer parity

  Goal: close one metadata XML property-layer mismatch for a partial family.
  Prefer high-volume child attribute tails (`Use`, `ToolTip`, `QuickChoice`,
  `PasswordMode`, `MultiLine`, `MinValue`, `MaxValue`, `Mask`,
  `MarkNegatives`, `Indexing`) or a root Configuration/CommonAttribute layer if
  that is more isolated.

  Ownership:
  - `src/mssql_dump/mod.rs` metadata XML parser/formatter region only.

  Avoid:
  - Form.xml, MXL/template body, source-asset body parsers;
  - hardcoded names or GUIDs from `ut_ibcmd`;
  - repeating recent fixes: Catalog `QuickChoice`/`ChoiceMode`, Catalog
    create/history tail, InformationRegister `DataLockControlMode`,
    BusinessProcess/Task `UseStandardCommands`, Configuration root
    child-family classifier.

  Acceptance:
  - focused unit test that parses native-like fields and verifies XML order;
  - `cargo test <focused-filter> --lib`, `cargo fmt --check`,
    and `git diff --check`.

- [ ] Task E - Issue #21, existing-base staging readiness

  Goal: reduce or precisely audit one remaining base-blob dependency in the
  staging-over-existing-compatible-infobase path.

  Ownership:
  - `src/mssql.rs`, `src/source_audit.rs`, and only the smallest necessary
    helper in `src/module_blob.rs` if a blocker message must be refined.

  Avoid:
  - claiming blank-infobase bootstrap support;
  - broad metadata/form/template parser changes;
  - weakening existing base-dependency diagnostics.

  Acceptance:
  - either a base-free row-generation improvement or a more precise
    `requires_base_blob` blocker with counts/shape evidence;
  - focused staging/readiness tests;
  - `cargo fmt --check` and `git diff --check`.
