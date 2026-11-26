# MSSQL direct export architecture plan

## Current pressure points

- `src/mssql_dump.rs` mixes SQL fetching, metadata indexing, source layout routing, object-kind XML formatting, asset extraction, and diagnostics.
- `src/module_blob.rs` mixes form/XML packing, unpacking, and low-level V8 container helpers.
- Performance work now depends on per-stage timings and byte-level diffs; both should stay first-class diagnostics.

## Proposed module split

- `mssql_dump/db.rs`: `sqlcmd`/`bcp` process execution, native BCP row decoding, password/env handling boundaries.
- `mssql_dump/export.rs`: table orchestration, direct source export mode, output directory policy, timing aggregation.
- `mssql_dump/indexes.rs`: metadata text row cache and reference indexes for objects, forms, templates, fields, commands, help, standalone content.
- `mssql_dump/routing.rs`: mapping metadata/body rows to source paths and `SourceAssetKind`.
- `mssql_dump/metadata/`: one formatter/parser module per high-churn metadata family, for example `common_picture`, `catalog`, `constant`, `form`, `style_item`.
- `mssql_dump/assets/`: source asset extractors and formatters, for example `ext_picture`, `form`, `help`, `moxel`, `role_rights`, `schedule`, `standalone_content`.
- `module_blob/container.rs`: low-level V8 container read/write primitives.
- `module_blob/form.rs`: form body parsing and packing.

## Diagnostic contract

- Keep `dump_timings` in the export JSON and continue reporting both wall time and CPU-style accumulated stage time.
- Keep `source-diff` grouping by `kind` and `object_hint`; it is the fastest way to choose the next compatibility target.
- Add focused regression tests per fixed source family before broad refactors.

## Next compatibility targets

- `metadata_xml`: largest remaining group after CommonPictures.
- `form`: biggest CPU consumer and the second-largest diff group.
- `template`: isolated enough to improve after metadata/form patterns are stable.
