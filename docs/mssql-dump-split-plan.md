# mssql_dump.rs Split Plan

`src/mssql_dump.rs` is about 42k lines and has become the main merge-conflict
surface for parity agents. The split should be mechanical first: move cohesive
blocks behind small internal modules, keep behavior unchanged, and only then
continue feature work on top of smaller files.

## Current Hot Zones

Most recent parity work touched these zones:

| Zone | Current line area | Typical agents |
|---|---:|---|
| dump orchestration, timings, SQL/BCP fetch | 1-1,550 and 26,250-27,160 | performance / selected export |
| source-reference indexes and source-asset discovery | 1,550-3,100 and 4,530-6,150 | selected export, command refs, staging readiness |
| DCS template normalization | 3,135-3,720 | DCS/SKD template parity |
| source asset writing and common asset parsers | 3,720-9,100 | Help, RoleRights, CommandInterface, Flowchart, pictures |
| Form.xml extraction/formatting | 9,100-13,800 | Form parity |
| MXL spreadsheet extraction/formatting | 13,800-15,800 | MXL/template parity |
| module/form body source paths | 15,800-16,200 | source layout / staging |
| metadata XML models, parsers, formatters | 16,200-26,200 | object metadata, registers, configuration root |
| tests | 27,160-end | all agents |

The biggest conflict source is not algorithmic coupling; it is that unrelated
format code and tests live in the same file.

## Proposed Modules

Target module layout:

```text
src/mssql_dump/
  mod.rs                  // public dump_config entrypoint and orchestration
  timing.rs               // MssqlDumpTimingReport/Summary and JSON readers
  fetch.rs                // sqlcmd/bcp fetch, native BCP parser
  selected.rs             // selected file-name expansion and index-needs logic
  indexes.rs              // object/form/template/type/subsystem/reference indexes
  source_assets/
    mod.rs                // SourceAssetKind, discovery, write_source_asset dispatch
    command_interface.rs
    dcs.rs
    flowchart.rs
    help.rs
    moxel.rs
    pictures.rs
    predefined.rs
    role_rights.rs
    schedule.rs
    standalone.rs
    style_body.rs
  form_xml.rs             // Form.xml extraction/formatting only
  metadata_xml/
    mod.rs                // extract_metadata_source_xml dispatch
    model.rs              // MetadataHeader and property structs
    common.rs             // shared XML helpers/type formatting/child tails
    configuration.rs
    object_families.rs    // Catalog/Document/DataProcessor/Report
    registers.rs
    simple.rs             // enums, constants, commands, languages, etc.
  tests/                  // optional later, only after module split settles
```

Rust path choice:

- Replace `src/mssql_dump.rs` with `src/mssql_dump/mod.rs`.
- Keep `pub fn dump_config`, report structs, and existing external API stable.
- Use `pub(crate)` between submodules; avoid making new public library API.

## Safe Order

1. Extract `timing.rs` and `fetch.rs`. Done in rounds 40 and 41.
   These are low-risk because they have clear boundaries and few dependencies
   on metadata/MXL/Form internals.

2. Extract selected-export planning helpers. Done in round 42 as
   `src/mssql_dump/selected.rs`.

3. Extract `source_assets::dcs`, `source_assets::moxel`, and `form_xml.rs`.
   These are the main parity-agent conflict zones. Moving them early lets future
   agents own separate files.

4. Extract `metadata_xml` in two passes:
   first shared model/helpers, then family-specific parsers/formatters.
   Do not split individual object families before shared child-tail helpers are
   isolated, otherwise imports will become circular.

5. Extract `indexes.rs`.
   These depend on metadata/source asset models, so doing them after the model
   split is cleaner.

6. Move tests only after code modules compile cleanly.
   Initially keep tests in `mssql_dump/mod.rs` so mechanical moves do not also
   fight test fixture visibility.

## Guardrails

- First PR/agent round should be pure moves plus import fixes: no behavior
  changes, no parity fixes in the same commits.
- After each move run:
  - `cargo fmt --check`
  - `cargo check`
  - focused existing tests for the moved area
  - `git diff --check`
- Use `git diff --color-moved` during review to confirm the patch is mostly
  relocation.
- Keep one agent per module-move round. Parallelizing the first split will
  create worse conflicts than the current file.

## Completed First Split Task

Implemented layout:

```text
src/mssql_dump/
  mod.rs
  timing.rs
  fetch.rs
```

Moved:

- `MssqlDumpTimingReport`, `MssqlDumpTimingSummary`,
  `MssqlDumpTableTimingSummary`;
- timing helper methods and JSON readers;
- SQL/BCP row fetch structs and functions from the bottom of the file.

This reduced the central file and gives a template for later format-specific
moves without touching parity behavior. The next practical split is
`form_xml.rs`, followed by MXL/source-asset modules.

## Completed Selected Split

Round 42 added:

```text
src/mssql_dump/
  selected.rs
```

Moved selected-export planning helpers, selected source-reference index needs,
direct selected metadata reference scanning, and selected command-interface
reference helpers. This was mechanical and kept tests in `mod.rs`.
