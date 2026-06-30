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

## Batch timing summary reports

Round 27 added a focused parser for saved `mssql-dump-config` JSON reports:

```powershell
target\release\ibcmd-rs.exe mssql-dump-timing-summary `
  E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs-report.json `
  -o E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs-summary.json
```

The summary keeps the fields needed to compare the 256 MiB BCP batch change:

- `batch_cap_binary_bytes`
- `fetch_row_batches`
- `fetch_row_batch_max_rows`
- `fetch_row_batch_max_binary_bytes`
- `fetch_row_batch_max_binary_mib`
- `fetch_rows_ms`
- `fetch_rows_bcp_ms`
- `fetch_rows_ms_per_gib`
- `process_rows_wall_ms`
- `process_rows_wall_ms_per_gib`

It accepts one or more report paths and emits a JSON array, so before/after runs
can be summarized with the same command.

## Round 27 real timing follow-up

Environment check on 2026-06-30:

- `sqlcmd` was available locally.
- `localhost` SQL Server accepted integrated authentication.
- `ut_ibcmd` contained 40,625 `Config` rows and about 927,826,268 bytes of
  `BinaryData`; `ConfigSave` was empty.

A full native source-layout run was attempted under:

```text
E:\ibcmd_lab\perf\issue-19-performance-v5-full-fetch
```

Command:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-performance-v5-full-fetch `
  --overwrite `
  --no-binary-rows
```

That run did not produce a timing report because export processing stopped on
an existing source-layout gap:

```text
failed to extract standalone content from source asset Ext/StandaloneConfigurationContent.bin
standalone content reference not found: 0014cc2a-b5ed-427d-8ac6-116e92aaa9a4
```

No `ConfigDumpInfo.xml` file was generated by the failed attempt.

To still exercise the 256 MiB BCP batch path on real data, a selected blob run
used the 36 largest blob-bearing `Config.FileName` values from `ut_ibcmd`. The
file-name list is:

```text
E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blob-file-names.txt
```

Command:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs `
  --overwrite `
  --no-binary-rows `
  --file-name-list E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blob-file-names.txt `
  | Tee-Object -FilePath E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs-report.json
```

Result:

| Metric | Value |
|---|---:|
| Selected rows | 72 |
| Selected binary bytes | 663,776,134 |
| `fetch_row_batches` | 3 |
| `fetch_row_batch_max_rows` | 29 |
| `fetch_row_batch_max_binary_bytes` | 268,302,217 |
| `fetch_row_batch_max_binary_mib` | 256 |
| `fetch_rows_ms` / `fetch_rows_bcp_ms` | 3,532 / 3,532 |
| `fetch_rows_ms_per_gib` | 5,713 |
| `process_rows_wall_ms` | 1 |

Artifacts:

- `E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs-report.json`
- `E:\ibcmd_lab\perf\issue-19-performance-v5-selected-blobs-summary.json`

Interpretation: the real selected run crossed the batch cap and split row fetch
into three native BCP batches. The largest observed batch stayed effectively at
the 256 MiB cap, confirming that the batch limiter is active. Because this run
disabled binary writes and source extraction, it isolates BCP fetch timing; it
does not replace a full source-layout performance run.

## Implemented optimization

Previously implemented first low-risk step in this area: the broad
`build_command_interface_reference_index_from_texts` scan now parses metadata
rows in parallel and then applies entries in the original row order. This keeps
the previous overwrite semantics for duplicate keys, while reducing the CPU wall
time reported in `prepare_command_refs_ms` on large selected `.9` exports.

Round 9 implemented the next step: selected configuration `.9`
command-interface exports no longer force broad metadata text indexing just
because `command_refs` are needed.

The current shape is:

1. Inflate selected `.9` command-interface body early and collect command UUIDs
   that actually appear in command fields.
2. Resolve direct metadata refs first.
3. Resolve missing nested command refs through owner metadata rows.
4. Fetch only owner metadata rows plus direct metadata UUIDs when resolution is
   complete.
5. Fall back to the previous broad metadata path when owner resolution is
   incomplete, preserving readable names instead of emitting raw UUID fallbacks.

Covered by focused tests:

- `targeted_command_owner_rows_match_broad_command_interface_output`
- `selected_command_interface_refs_collect_command_fields_only`

Remaining performance follow-up:

- run the selected `.9` export command against the lab database and record fresh
  `prepare_metadata_texts_ms` and `prepare_command_refs_ms` timings under
  `E:\ibcmd_lab\perf`;
- keep issue #19 open until the measured selected export confirms the expected
  wall-time improvement on the real `ut_ibcmd` sample.
- rerun the full source-layout export after the standalone-content reference gap
  is fixed, then summarize the resulting report with `mssql-dump-timing-summary`.
