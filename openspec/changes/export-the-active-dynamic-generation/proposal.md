# Export the active dynamic generation

## Why

An online (dynamic) configuration update does not rewrite the rows it changes.
It writes the new bodies under an alias — `<base>_dynupdate_<generation>` — and
records the generation in the `Config` row `DynamicallyUpdated`; the plain rows
keep the previous content. The infobase, and the native `ibcmd`, read the
aliased rows as the configuration. This export reads the plain ones, so on any
database left holding an active dynamic generation it publishes the *previous*
configuration for every object that update touched.

Measured on 21.09.2026 against the BSP demo database
`ibcmd_rs_bsp_8327_native_20260919`, which an earlier load test left with the
active generation `06cb0442-0c47-4fad-986a-f08f28287c1b`:

| | files |
|---|---|
| identical to a native export of the same database | 12 194 |
| different | 4 |

The four are exactly the objects that generation carries —
`CommonModules/_ДемоЗаметки/Ext/Module.bsl` and
`CommonForms/_ДемоПримечание` with its form module, each missing the line the
update added — plus `ConfigDumpInfo.xml`, which is built from the `versions`
record and whose active copy is `versions_dynupdate_<generation>`.

The same database with no active generation exported 12 198 of 12 198.

ERP УХ 3.3.3.3 carries 156 rows under two *superseded* generations and no
`DynamicallyUpdated` record at all; its plain rows are the configuration, which
is what `is_superseded_generation_owner` already encodes. That rule is correct
for a committed update and silent about an active one.

## What changes

- The export resolves the active dynamic generation from the storage table's
  own `DynamicallyUpdated` record, once per run.
- Every row fetch and every file-name inventory reads the configuration
  *after* that generation is applied: a row named `<base>_dynupdate_<active>`
  is published under `<base>` and hides the plain row of that name, a row
  named `<base>_dynupdate_<other>` is a superseded generation and is ignored,
  and `versions_dynupdate_<active>` is the `versions` record.
- A database with no `DynamicallyUpdated` record keeps exactly today's
  behaviour.

## Impact

- `src/mssql_dump/fetch.rs` (the storage-table expression every query builds
  on), `src/mssql_dump/mod.rs` (the file-name inventory),
  `src/mssql_dump/source_assets.rs` (`is_superseded_generation_owner`, which
  becomes "not of the active generation").
- No change for a database without an active generation, which is every
  parity corpus this project measures except the one above.
