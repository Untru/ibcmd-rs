# Native version status API (#362, reused by #380)

The editor `StoredConfiguration::status` compares source bytes. Native status
compares ConfigDumpInfo versions; equal XML bytes do not imply equal versions.
Native import can restamp Configuration and Language without changing XML.

- `StoredConfiguration::native_config_dump_info(&mut self) -> Result<Vec<u8>>`
  reads the main configuration through the shared export inventory writer and
  the existing SQL handle (or own rows directory), without writing files or
  converting the object-body batches. A completed ConfigSave is included.
- `mssql_dump::read_config_dump_info(&MssqlDumpConfigArgs)` is the same reader
  for direct filedb/SQL/rows-dir callers. Set `main_configuration` for native
  main-state semantics. Selected rows, base/sync, or partial inventory are refused.
- `native_export_status::ConfigDumpInfoSnapshot::parse(&[u8])` reads the XML.
  `current.compare(&reference)` returns `NativeExportStatus` with `fullRequired`
  and changes (`name`, `id`, `kind`, `configVersion`, `referenceVersion`).
  A removed ID or renamed reference requires a full export. Each ordinary
  changed row retains its exact current and reference versions.
- `NativeExportStatus::text(short)` supplies the measured native stdout form;
  RPC should use the typed fields, retain reference state explicitly, and
  surface `fullRequired` rather than converting it to an unchanged preview.

Measured on native 8.5.1.1592: unchanged status is empty; individual rows are
`added` / `modified`; removed/renamed reference reports `modified: all`.
A reference-only XML header change 2.20 -> 2.21 does not count as a native
version change. This differs from the custom export dialect migration policy.

`ConfigDumpInfoSnapshot::owners()` returns `byId: rowId -> { object, root, nativeObject }`
using canonical metadata-reference grammar and root descriptors from the same native inventory. Forms and
templates retain their immediate full name; `root` permits top-level roll-up.
Inline Command modules map to the object whose XML contains their descriptor;
`nativeObject` retains the exact Command reference (and every other owner).
Inline members such as owned Commands need not have a standalone descriptor version entry; storage row IDs must not be treated universally as metadata UUIDs. Canonical nested references are retained as written. This is name routing, not an independent proof that an editor tree node still exists; RPC must check its source/freshness identity. No search climbs past an unknown nested kind to a known ancestor. The native Configuration.<name> owner is normalized to the editor root
`Configuration`. `unmappedIds` is explicit and must prevent an all-green result
for unsupported/future metadata. Metadata bytes never participate in this map.

Private ERP read-only evidence on native 8.5.1.1592: 105,253 top-level versioned
entries (214,747 Metadata elements including nested parts), all 105,253 mapped
by these descriptor/owner rules, zero unmapped. XML bytes and database remain
private; only aggregate counts are published.


For CLI `info --out` and `status --out`, explicitly selected parent directory
aliases are resolved before publication; missing status parent directories are
created below that resolved parent. A linked final file is refused. Use a plain
file path for status: parent traversal (`..`) and terminal directory syntax
(a separator or `/.`) are refused rather than rewritten as a file target.
Object export keeps its strict linked-component checks. These path checks do
not claim safety against hostile concurrent replacement of parent directories.

An explicitly empty `--out` is refused before resolving the database. For
`objects`, the selected output root and every existing ancestor must themselves
be plain directories; resolving aliases before checking them would bypass the
object publication guard. Failed queries with `--report` write `ok: false` and
the original diagnostic, including when the report replaces an older success.
A report publication failure is a secondary diagnostic.

Native incremental export validates removed/renamed rows and nested parts against
the inventory used by the actual export, before preparing its output directory
or starting writers. Body-row roles use current typed source-asset routing,
never the reference's role spelling. This supplements the CLI preflight; it does
not claim transactional consistency across concurrent SQL changes. The custom
export command retains its documented full-export fallback and pruning behavior.
