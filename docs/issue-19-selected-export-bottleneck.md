# Issue 19 selected-export bottleneck

Date: 2026-06-30

Scope checked: `src/mssql_dump.rs` selected source export path.

## Concrete bottleneck

The remaining selected-export bottleneck is the serial construction of broad
metadata reference indexes before row writing. In the current path this starts
after selected row headers are fetched:

1. `dump_table_source_layout` collects selected `FileName` values.
2. `selected_configuration_source_asset_index_needs` marks configuration
   command-interface rows (`.9` / `.a`) as needing `command_refs` and
   `metadata_refs`.
3. `SourceReferenceIndexNeeds::needs_broad_metadata` treats `command_refs` as
   broad, so selected `.9` export fetches every metadata row with
   `fetch_metadata_rows_bcp`.
4. `build_metadata_text_rows` inflates all fetched metadata blobs.
5. `build_command_interface_reference_index_from_texts` then scans those
   inflated metadata texts serially to map command UUIDs to readable source
   names.

This is output-preserving but expensive because a selected command-interface
body is small while the index builder walks the whole metadata set.

Latest recorded selected `Ext` source-mode run in
`docs/export-parity-status.md`:

| Stage | Time |
|---|---:|
| `prepare_indexes_ms` | 1552 ms |
| `prepare_metadata_fetch_ms` | 211 ms |
| `prepare_metadata_texts_ms` | 768 ms |
| `prepare_reference_indexes_ms` | 573 ms |
| `prepare_command_refs_ms` | 392 ms |
| `prepare_form_refs_ms` | 163 ms |
| `prepare_metadata_refs_ms` | 14 ms |

For this run the bottleneck is not row writing: it is the single-threaded
metadata text inflation plus reference-index scans needed before selected rows
can be routed and formatted.

## Why not just skip it

`parse_command_interface_blob` falls back to raw UUID text when a reference is
missing. That fallback keeps the export from crashing, but it changes source XML
semantics from readable names such as:

```text
Catalog.Products.Command.OpenList
```

to fallback names such as:

```text
0:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa
```

A selected command-interface body can contain UUIDs for nested object commands.
Those command UUIDs are not necessarily direct `Config.FileName` values; the
readable name is inside the owner metadata blob. Therefore a simple targeted
fetch by UUIDs found in the selected `.9` body is not safe for `command_refs`.

The already-safe targeted optimization is narrower: `.a` subsystem-order bodies
can use direct metadata UUID references and the current code already fetches
only those rows through `selected_configuration_direct_metadata_reference_file_names`.
The remaining `.9` command-reference case needs an owner lookup.

## Repro/report command

Keep all output under `E:\ibcmd_lab`. Use integrated authentication or an
environment variable for the password; do not put passwords on the command
line.

```powershell
$out = "E:\ibcmd_lab\perf\issue-19-selected-ext"
Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction SilentlyContinue
target\release\ibcmd-rs.exe mssql-dump-config `
  --server "<server>" `
  --database "<database>" `
  --output-dir $out `
  --overwrite `
  --no-binary-rows `
  --extract-metadata-xml `
  --file-name "<configuration-uuid>.8" `
  --file-name "<configuration-uuid>.9" `
  --file-name "<configuration-uuid>.a" `
  --file-name "<configuration-uuid>.b" `
  | Tee-Object -FilePath "E:\ibcmd_lab\perf\issue-19-selected-ext-report.json"
```

For SQL authentication, prefer:

```powershell
$env:IBCMD_DB_PSW = "<password from secure local source>"
target\release\ibcmd-rs.exe mssql-dump-config `
  --server "<server>" `
  --sql-user "<login>" `
  --sql-pwd-env IBCMD_DB_PSW `
  --database "<database>" `
  --output-dir "E:\ibcmd_lab\perf\issue-19-selected-ext" `
  --overwrite `
  --no-binary-rows `
  --extract-metadata-xml `
  --file-name "<configuration-uuid>.9"
```

Fields to compare in the JSON report:

- `timings.prepare_indexes_ms`
- `timings.prepare_metadata_fetch_ms`
- `timings.prepare_metadata_texts_ms`
- `timings.prepare_reference_indexes_ms`
- `timings.prepare_command_refs_ms`
- per-table `tables[].timings.prepare_command_refs_ms`

## Next safe optimization step

Add a command-owner reference index instead of scanning every metadata blob for
each selected `.9` run.

The safe shape is:

1. Inflate selected `.9` command-interface body early and collect command UUIDs
   that actually appear in command fields.
2. Resolve those UUIDs through a persisted or SQL-side owner index that maps
   nested command UUID -> owner metadata `FileName`.
3. Fetch only owner metadata rows plus direct metadata UUIDs needed for standard
   commands and groups.
4. Build `command_refs` from that small owner set and compare generated
   `Ext/MainSectionCommandInterface.xml` against the current broad-index output.

Acceptance criteria for the optimization:

- selected `.9` export remains byte-identical to the broad-index output on the
  known `ut_ibcmd` command-interface sample;
- missing owner rows still fall back to the current broad path, not to raw UUID
  output;
- the report shows lower `prepare_metadata_texts_ms` and
  `prepare_command_refs_ms` for selected `.9` without increasing
  `process_rows_wall_ms`.
