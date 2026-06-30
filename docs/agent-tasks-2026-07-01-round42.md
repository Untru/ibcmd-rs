# Parallel Agent Tasks - 2026-07-01 Round 42

Base branch: `master` after commit `0a948ef`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`.

The current import target remains staging over an existing compatible
infobase. Do not redesign this round as bootstrap into a blank database.

Issue #23 is closed infrastructure. Use `src/v8_container.rs` for V8 container
work; do not reintroduce private V8 container parsing in `module_blob.rs`.

`src/mssql_dump/mod.rs` is still a high-conflict file. Keep edits inside the
owned zones below. If a task needs to cross ownership, stop and document the
reason instead of widening the patch.

## Tasks

- [ ] Task A - Issue #16, Form.xml packer parity

  Agent: `019f1af2-095b-7302-be49-76a5a574e8ee`

  Goal: close one generic Form.xml packing mismatch class in `src/module_blob.rs`
  without touching the `src/mssql_dump/mod.rs` decompiler.

  Ownership:
  - `src/module_blob.rs` Form XML parser/packer helpers and focused tests.

  Avoid:
  - changes in `src/mssql_dump/mod.rs`;
  - repeating recent fixes: `ShowCommandBar`, `Page/ScrollOnCompress`,
    `WindowOpeningMode`, dynamic-list `Settings/Field`, `Table/SkipOnInput`,
    `TextDocumentField/ReadOnly`, `CheckBoxField/ShowInHeader`;
  - database-specific GUID/name hardcoding.

  Acceptance:
  - one focused test proving XML -> native layout packing for the selected
    property or child shape;
  - `cargo test <focused-filter> --lib`, `cargo fmt --check`,
    and `git diff --check`.

- [ ] Task B - Issue #17, MXL/template body parity

  Agent: `019f1af2-7b4e-7ce3-9385-2aa3ea4878e9`

  Goal: close one remaining repeated Template/Ext/Template.xml body mismatch
  class, preferably from the known `document/format/*` or row/cell format
  signatures.

  Ownership:
  - MXL/SpreadsheetDocument extraction and packing code in
    `src/mssql_dump/mod.rs` MXL region and `src/module_blob.rs` spreadsheet
    packer region only.

  Avoid:
  - Form.xml work;
  - metadata XML work;
  - repeating recent fixes: style refs `-13`, `-15`, `-21`, `-25` through
    `-38`, text orientation, text placement, empty format slots, named-area
    drawing filtering, number-format string table.

  Acceptance:
  - focused extractor/packer or stable round-trip test for the selected MXL
    shape;
  - `cargo test moxel --lib` and/or `cargo test spreadsheet --lib`;
  - `cargo fmt --check` and `git diff --check`.

- [ ] Task C - Issue #18/#15/#22, metadata XML parity

  Agent: `019f1af2-ebee-74c3-b084-48df4f03aa06`

  Goal: close one metadata XML property-layer mismatch for a partial family:
  Catalog, Document, DataProcessor, Report, register-like families,
  Subsystem/ExchangePlan, CommonAttribute, or Configuration root.

  Ownership:
  - `src/mssql_dump/mod.rs` metadata XML parser/formatter region only.

  Avoid:
  - Form.xml, MXL/template body, source-asset body parsers;
  - hardcoded names or GUIDs from `ut_ibcmd`;
  - repeating recent fixes: Catalog `QuickChoice`/`ChoiceMode`,
    InformationRegister `DataLockControlMode`, BusinessProcess/Task
    `UseStandardCommands`, Configuration root child-family classifier.

  Acceptance:
  - focused unit test that parses native-like fields and verifies XML order;
  - `cargo test <focused-filter> --lib`, `cargo fmt --check`,
    and `git diff --check`.

- [ ] Task D - Issue #21, existing-base staging readiness

  Agent: `019f1af3-5f19-7c30-9962-e76b90e9ebf0`

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

- [ ] Task E - Issue #24, mssql_dump split follow-up

  Agent: `019f1af3-db54-7ca3-a1da-3357aee67deb`

  Goal: perform the next mechanical split after `timing.rs` and `fetch.rs`.
  Prefer extracting selected-export planning helpers into
  `src/mssql_dump/selected.rs`. If that boundary is worse than expected,
  document why and extract a smaller low-risk helper module instead.

  Ownership:
  - `src/mssql_dump/mod.rs`;
  - new `src/mssql_dump/selected.rs` or the chosen focused module.

  Avoid:
  - behavior changes;
  - parity feature work in the same patch;
  - moving tests unless necessary for visibility.

  Acceptance:
  - public API remains stable;
  - `cargo check`, focused selected-export/index-needs tests if applicable,
    `cargo fmt --check`, and `git diff --check`.
