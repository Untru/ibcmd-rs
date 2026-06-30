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

Round 28 removed this standalone-content blocker for source-asset-only dumps
(`--no-binary-rows` without `--extract-metadata-xml`). The standalone content
row now collects the UUIDs from `Ext/StandaloneConfigurationContent.bin` and
builds the required metadata reference names even when metadata XML extraction
is disabled.

Focused real repro:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-standalone-content-v6-standalone-repro-targeted `
  --overwrite `
  --no-binary-rows `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.0 `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.5 `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.6 `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.7 `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.8 `
  --file-name c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.f `
  | Tee-Object -FilePath E:\ibcmd_lab\perf\issue-19-standalone-content-v6-standalone-repro-targeted.json
```

Result: 6 selected rows / 40,421 bytes / 2 source assets. The run completed
successfully and wrote `Ext/StandaloneConfigurationContent.bin`; the previous
missing reference `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` resolved as
`Role.РазделОтчетыИМониторингЦелевыеПоказатели`. Release targeted timings were
`prepare_indexes_ms=2013`, `prepare_object_refs_ms=155`,
`prepare_standalone_refs_ms=109`, `fetch_rows_ms=67`, and
`source_asset_standalone_content_cpu_ms=2`.

A full source-layout timing run was retried under:

```text
E:\ibcmd_lab\perf\issue-19-standalone-content-v6-full-source
```

It passed the standalone-content reference and then stopped at a later,
separate source asset:

```text
failed to extract exchange plan content from source asset ExchangePlans\Полный\Ext\Content.xml
```

The partial run wrote 1,295 files / 130,826,195 bytes and still did not
generate `ConfigDumpInfo.xml`.

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

## Round 29 ExchangePlan Content.xml blocker isolation

The full source-layout stop after Round 28 was reproduced through a selected
ExchangePlan content run for the real `ut_ibcmd` row:

```powershell
E:\ibcmd_lab\worktrees\issue-19-exchange-content-v1\target\debug\ibcmd-rs.exe `
  mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-exchange-content-v1-polny-selected-after `
  --overwrite `
  --no-binary-rows `
  --file-name-list E:\ibcmd_lab\perf\issue-19-exchange-content-v1-polny-selected-file-names.txt
```

The selected file-name list contains only:

```text
00fa91bf-4a38-4fe7-830d-2f265c9e251d.1
```

That is `ExchangePlans\Полный\Ext\Content.xml`. The raw content body was saved
for inspection at:

```text
E:\ibcmd_lab\perf\issue-19-exchange-content-v1-polny-content-inflated.txt
```

The body shape is a supported flat ExchangePlan content list:

```text
{2,1699,<metadata-id>,<auto-record>,...,0}
```

The generic source-layout path was updated to build metadata object/type
reference indexes when source assets are being extracted without metadata XML
output, and the ExchangePlan content parser now reports the exact unsupported
content item instead of only the asset path. The selected repro now stops with:

```text
failed to extract exchange plan content from source asset ExchangePlans\Полный\Ext\Content.xml

Caused by:
    ExchangePlanContent item 0 references unsupported metadata id ff76e85a-6d29-41d3-a83e-f4a34139c6b2
```

The referenced metadata id is a direct `Config.FileName` row with metadata code
`16` and name `ВыгружатьВнутренниеШтрихкодыШтучныхТоваров`, so the remaining
blocker is not the ExchangePlan content framing. The next step is to determine
why this real Constant row is not present in the source-layout object reference
index during the selected/full extraction path, or to add a targeted reference
index path for ExchangePlan content ids.

## Round 30 ExchangePlan Constant reference index

Root cause: selected ExchangePlan owner metadata was classified as needing
form/template/type reference indexes, but not `object_refs`. When a selected
`ExchangePlans\Полный\Ext\Content.xml` export includes the owner metadata row,
that selected needs profile overrides the full source-layout reference profile.
The `.1` ExchangePlan content body can reference direct metadata object UUIDs,
including Constants, so direct object references must be prepared for selected
ExchangePlan source-asset extraction even when metadata XML is disabled.

The real Constant row exists in `ut_ibcmd.dbo.Config` and is included by the
broad metadata predicate:

```text
FileName: ff76e85a-6d29-41d3-a83e-f4a34139c6b2
PartNo: 0
DataSize: 404
```

Its inflated direct metadata shape is a code `16` Constant named
`ВыгружатьВнутренниеШтрихкодыШтучныхТоваров`. The selected needs rule now sets
`object_refs` for `ExchangePlan`, and the focused unit expectation was updated
accordingly.

Selected repro after the fix:

```powershell
E:\ibcmd_lab\worktrees\issue-19-exchange-constant-ref-v2\target\debug\ibcmd-rs.exe `
  mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-polny-selected-after-fix `
  --overwrite `
  --no-binary-rows `
  --file-name-list E:\ibcmd_lab\perf\issue-19-exchange-content-v1-polny-selected-file-names.txt
```

Result: completed successfully, wrote 2 files, did not generate
`ConfigDumpInfo.xml`, and the first content entry resolved as:

```xml
<Metadata>Constant.ВыгружатьВнутренниеШтрихкодыШтучныхТоваров</Metadata>
```

Selected timing evidence:

| Metric | Value |
|---|---:|
| `rows` | 2 |
| `source_asset_rows` | 1 |
| `prepare_indexes_ms` | 122,332 |
| `prepare_metadata_texts_ms` | 7,830 |
| `prepare_reference_indexes_ms` | 113,950 |
| `prepare_object_refs_ms` | 111,000 |
| `source_asset_exchange_plan_cpu_ms` | 10 |

Artifacts:

- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-polny-selected-after-fix-report.json`
- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-polny-selected-after-fix.log`
- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-polny-selected-after-fix`
- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-constant-inflated.txt`

Because the selected repro passed, a full release source-layout run was retried:

```powershell
E:\ibcmd_lab\worktrees\issue-19-exchange-constant-ref-v2\target\release\ibcmd-rs.exe `
  mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source `
  --overwrite `
  --no-binary-rows
```

Result: completed successfully, wrote 13,428 files from 40,576 rows, emitted
10,277 source assets, and did not generate `ConfigDumpInfo.xml`.

Full-run timing evidence:

| Metric | Value |
|---|---:|
| `total_rows` | 40,576 |
| `total_binary_bytes` | 927,826,268 |
| `total_source_asset_rows` | 10,277 |
| `prepare_indexes_ms` | 17,461 |
| `prepare_metadata_texts_ms` | 1,570 |
| `prepare_reference_indexes_ms` | 15,520 |
| `prepare_object_refs_ms` | 10,621 |
| `fetch_rows_ms` | 5,043 |
| `process_rows_wall_ms` | 15,598 |
| `source_asset_cpu_ms` | 191,976 |
| `source_asset_exchange_plan_cpu_ms` | 96 |

Artifacts:

- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source-report.json`
- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source.log`
- `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source`

Follow-up: the selected one-row ExchangePlan content repro is now correct but
expensive because it must build broad object refs. A future performance pass can
target direct metadata UUIDs from the selected ExchangePlan content body instead
of building the full object reference index.

## Round 31 Form source-asset CPU pass

The full source-layout report from Round 30 made form extraction the largest
remaining source-asset CPU target:

| Metric | Round 30 value |
|---|---:|
| `source_asset_cpu_ms` | 191,976 |
| `source_asset_form_cpu_ms` | 124,212 |
| `source_asset_form_xml_cpu_ms` | 118,809 |
| `source_asset_form_child_items_cpu_ms` | 70,750 |

The largest generic avoidable cost found in this pass was repeated parsing of
the same extended `InputField` option bag while extracting child items. Each
extended input field previously called `form_input_field_extended_options` for
every individual option-backed property, so one child item could resplit the
same nested `{38,...}` option block more than twenty times. The extractor now
splits that option block once per input field and passes the cached field slice
to the private option-property readers. This is source-preserving: the same
focused property tests still assert the generated XML for width/height,
stretching, buttons, quick choice, mark-required, and related options.

An attempted selected `.0` form-body repro used the largest distinct
`Config.FileName LIKE '%.0'` rows from `ut_ibcmd`, but selected source-only mode
does not currently build `form_refs` for raw selected form body rows. That run
therefore fetched 24 rows but wrote `source_asset_rows=0`, so it is recorded as
a selected-repro caveat rather than performance evidence for form extraction.

Full source-layout repro after the change:

```powershell
E:\ibcmd_lab\worktrees\issue-19-form-cpu-v1\target\release\ibcmd-rs.exe `
  mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-form-cpu-v1-full-source-after `
  --overwrite `
  --no-binary-rows
```

Result: completed successfully, wrote 10,277 source assets from 40,576 rows,
and did not generate `ConfigDumpInfo.xml`.

| Metric | Before | After |
|---|---:|---:|
| `process_rows_wall_ms` | 15,598 | 16,298 |
| `source_asset_cpu_ms` | 191,976 | 198,884 |
| `source_asset_form_cpu_ms` | 124,212 | 123,507 |
| `source_asset_form_xml_cpu_ms` | 118,809 | 118,673 |
| `source_asset_form_child_items_cpu_ms` | 70,750 | 68,886 |
| `source_asset_form_format_cpu_ms` | 1,836 | 1,693 |

Interpretation: the low-risk cache removes a repeated child-item parse and
reduced the child-item counter by about 1.9 seconds in this full run. The
overall wall time is within run-to-run noise and other source asset classes were
slower in this after run, so the next form optimization should still focus on
`extract_form_child_items`. The remaining larger generic cost appears to be
recursive child-item discovery repeatedly splitting nested item bodies and
scanning table/input service children.

Artifacts:

- `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-selected-form-file-names.txt`
- `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-selected-forms-after-report.json`
- `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-selected-forms-after`
- `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-full-source-after-report.json`
- `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-full-source-after`

## Round 32 Form child-item pre-pass consolidation

Round 31 left `extract_form_child_items` as the largest avoidable Form.xml
sub-counter:

| Metric | Round 31 after |
|---|---:|
| `source_asset_form_xml_cpu_ms` | 118,673 |
| `source_asset_form_child_items_cpu_ms` | 68,886 |
| `source_asset_form_properties_cpu_ms` | 5,110 |
| `source_asset_form_items_cpu_ms` | 3,187 |

The next low-risk cost was not another property parser, but repeated recursive
pre-passes over the same form layout tree before child-item parsing. The old
path separately walked and split nested child bodies to build table names,
table column names, child item names, and table user-settings groups, then
walked the tree again for actual child extraction.

The extractor now builds those support indexes through one combined
child-item-index traversal:

- table id -> table name;
- table id -> column id -> column name;
- child item id -> child item name;
- table id -> resolved `UserSettingsGroup` name.

The output path remains unchanged: `parse_form_child_item_pairs` still owns the
actual child-item structure and formatting. Existing focused child-item tests
cover the affected behavior, including table service children and wrapper `55`
user-settings groups.

Full source-layout repro after the change:

```powershell
E:\ibcmd_lab\worktrees\issue-19-source-layout-perf-v2\target\release\ibcmd-rs.exe `
  mssql-dump-config `
  --server localhost `
  --database ut_ibcmd `
  --output-dir E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after `
  --overwrite `
  --no-binary-rows
```

Result: completed successfully, wrote 10,277 source assets from 40,576 rows,
and did not generate `ConfigDumpInfo.xml`.

| Metric | Round 31 after | Round 32 after |
|---|---:|---:|
| `process_rows_wall_ms` | 16,298 | 16,055 |
| `source_asset_cpu_ms` | 198,884 | 187,772 |
| `source_asset_form_cpu_ms` | 123,507 | 106,971 |
| `source_asset_form_xml_cpu_ms` | 118,673 | 102,296 |
| `source_asset_form_child_items_cpu_ms` | 68,886 | 40,598 |
| `source_asset_form_format_cpu_ms` | 1,693 | 2,149 |
| `source_asset_inflated_cpu_ms` | 37,151 | 40,128 |
| `source_asset_moxel_cpu_ms` | 20,253 | 21,260 |

Interpretation: consolidating the pre-passes removed about 28.3 seconds from
the child-item CPU counter in this run. `source_asset_form_child_items_cpu_ms`
is still the largest Form.xml sub-counter, but it is now roughly tied with
inflated source assets. The next form-specific optimization should focus on the
remaining recursive child discovery path itself, especially
`parse_form_child_item_pairs` candidate scanning and the table/input service
child scans that can still parse nested bodies more than once.

Artifacts:

- `E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after-report.json`
- `E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after.log`
- `E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after`
