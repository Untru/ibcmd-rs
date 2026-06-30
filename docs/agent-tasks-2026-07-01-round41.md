# Parallel Agent Tasks - 2026-07-01 Round 41

Base branch: `master` after commit `c05bebc`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`.

The current import target remains staging over an existing compatible
infobase. Do not redesign this round as bootstrap into a blank database.

`src/mssql_dump/mod.rs` is still very large. Avoid touching the SQL/BCP fetch
helper zone near the bottom in this round; the main agent owns the mechanical
`fetch.rs` split.

## Tasks

- [x] Task A - Issue #16, shared Form.xml parity

  Goal: close one generic Form.xml compiler/decompiler mismatch class not
  already covered by rounds 35-40.

  Ownership:
  - shared Form.xml parser/formatter/packer code in `src/mssql_dump/mod.rs`
    or `src/module_blob.rs`;
  - focused synthetic tests; real fixture probes only if already available.

  Avoid:
  - repeating `ShowCommandBar`, `Page/ScrollOnCompress`,
    `WindowOpeningMode`, dynamic-list `Settings/Field`,
    `Table/SkipOnInput=false`, or `TextDocumentField/ReadOnly`;
  - database-specific GUID/name hardcoding.

- [x] Task B - Issue #17, template/MXL/DCS body parity

  Goal: close one remaining Template/Ext/Template.xml body mismatch class.

  Ownership:
  - MXL/DCS/template body extraction or packing code in `src/mssql_dump/mod.rs`
    and adjacent packers only if needed;
  - focused tests for the selected body shape.

  Prefer a repeated MXL `document/format/*`, style, drawing, or DCS
  canonicalization residual. Do not repeat prior style refs
  `-13`, `-15`, `-21`, `-25`, `-26`, `-27`, `-34`, `-35`, `-36`, `-37`,
  `-38`, text orientation, text placement, empty format slots, or named-area
  drawing filtering.

- [x] Task C - Issue #18, partial metadata/assets

  Goal: close one register/subsystem/exchange/workflow metadata XML or
  auxiliary asset residual.

  Ownership:
  - metadata XML parser/formatter code in `src/mssql_dump/mod.rs`;
  - source asset parser/formatter code only for register/subsystem/exchange
    assets.

  Avoid Form.xml, MXL/template body work, and already completed
  InformationRegister `DataLockControlMode`.

- [x] Task D - Issue #21, existing-base staging readiness

  Goal: reduce or precisely audit one remaining base-blob dependency in the
  staging-over-existing-compatible-infobase path.

  Ownership:
  - `src/mssql.rs`, `src/module_blob.rs`, `src/source_audit.rs`, or adjacent
    staging/readiness code;
  - focused tests for readiness or row generation.

  Do not implement or claim blank-infobase bootstrap import. The accepted
  outcome may be either a base-free row-generation improvement or a more
  precise `requires_base_blob` blocker.

- [x] Task E - Issue #24, local main-agent split

  Goal: mechanically move SQL/BCP fetch helpers from `src/mssql_dump/mod.rs`
  into `src/mssql_dump/fetch.rs`.

  Ownership: main agent only. Worker agents must not edit this slice.

  Acceptance:
  - no behavior changes;
  - existing public API remains stable;
  - `cargo fmt --check`, `cargo check`, focused fetch/timing tests, and
    `git diff --check` pass.

## Integrated Results

- #16: `CheckBoxField` packs explicit `ShowInHeader` through the extended form
  layout shape.
- #17: MOXCEL number-format string tables export and pack
  `document/format/format/v8:item` references.
- #18: `BusinessProcess` and `Task` metadata XML emit `UseStandardCommands`
  from the native owner field.
- #21: `Predefined.xml` still requires an existing compatible base shape, but
  the blocker now reports exact row, nesting and editable-field details.
- #24: SQL/BCP fetch helpers moved into `src/mssql_dump/fetch.rs`.
