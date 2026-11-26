# Apply-trace capture kit

Answers one question about a native 1C operation on a SQL Server infobase
(`ibcmd infobase config apply`, `infobase create`, `config load`, ...):
**what exactly did it write?**  Every claim is backed by a before/after row
diff and the SQL trace of the run.

Read-only for the inspected database: the kit never writes to it.  It only
creates and drops an Extended Events session on the server.

Needs: PowerShell 7 (or Windows PowerShell 5.1), Python 3.9+, SQL Server with
Windows authentication for the current user (`ALTER ANY EVENT SESSION` for the
trace).  No sqlcmd, no bcp, no pip packages.

| Script | What it does |
|---|---|
| `snapshot.ps1 -Database <db> -Out <dir>` | the state of the infobase: every table (rows, checksum), the schema, one line per row of the service tables |
| `diff.py --before <dir> --after <dir>` | tables created/dropped/altered, rows inserted/updated/deleted per service table, count/checksum changes of data tables |
| `trace.ps1 -Database <db> -Tag <t> -Exe <path> -ArgumentList ...` (or `-Command { ... }`) | runs the command under an XE trace and writes the report |
| `capture.ps1 -Database <db> -Tag <t> -Exe <path> -ArgumentList ...` (or `-Command { ... }`) | snapshot before, trace of the command, snapshot after, diff, one report folder |
| `trace_report.py` | the trace analysis used by `trace.ps1` (also usable on its own on `events.xml.gz` / `events.tsv.gz`) |
| `export-xel.ps1 -Pattern <.xel files> -OutDir <dir> [-Report]` | recovers a trace from the `.xel` files when a run's report step failed |
| `compare_traces.py --capture <label>=<dir> ...` | the writes of several captured runs side by side, one line per (table, operation, row-name shape): statements / rows / bytes per run, and the DDL verbs |
| `blobstore.py rebuild\|merge\|verify\|stats <dir>` | maintenance of the content-addressed blob store shared by snapshots (see "Blob store") |
| `check_capture.py <capture>` | bounded offline artifact/completeness gate; never certifies semantic acceptance |
| `tests/` | offline unit tests: `python -m unittest discover -s scripts/apply-trace/tests` |
| `lab/` | the case wrappers of the 0.4 trace track (stage, apply, path experiment, УХ plan); **not part of the kit**: they use `sqlcmd` and the lab paths `F:\ibcmd\lab\04\...`, see the last section |

## One command, everything

```powershell
$ibcmd = 'C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe'
$db    = 'ibcmd_rs_04_x_bsp_1'                      # a disposable clone
$data  = "F:\lab\ibdata\$db"
& scripts\apply-trace\capture.ps1 -Database $db -Tag apply-exclusive `
   -OutRoot F:\lab\04\x\captures -BlobStore F:\lab\04\x\blobs -TimeoutMinutes 45 `
   -Exe $ibcmd -ArgumentList @('infobase','config','apply','--dbms=MSSQLServer','--db-server=localhost',
       "--db-name=$db","--data=$data",'--user=Администратор','--force','--dynamic=disable')
```

`-Exe` and `-ArgumentList` work from `pwsh -File` too; `-Command { ... }` needs a
PowerShell session (a script block cannot be passed to `pwsh -File`) and runs in the
scope chain of the script, so do not reuse the script's parameter names (`$Database`,
`$Tag`, `$Exe`, `$Server`, `$Track`, ...) for other values.  `-TimeoutMinutes N` kills
the command and its child processes after N minutes (exit code -9).

`-BeforeCommand { ... }` / `-AfterCommand { ... }` (script blocks, also on `trace.ps1`)
run right before the command and right after it (in a `finally`, so also after a
failure or a timeout).  They are for the lab's lock discipline: the lock is then held
for the native command only, not for the snapshots around it:

```powershell
$lock = 'F:\ibcmd\lab\04\tools\heavy-lock.ps1'
& scripts\apply-trace\capture.ps1 ... -Exe $ibcmd -ArgumentList $args `
   -BeforeCommand { & pwsh -NoProfile -File $lock acquire <track> -Name native -TimeoutMin 120
                    if ($LASTEXITCODE -ne 0) { throw 'native lock not acquired' } } `
   -AfterCommand  { & pwsh -NoProfile -File $lock release <track> -Name native }
```

`-BeforeSnapshot <dir>` reuses an existing snapshot as the *before* state (the retry of
a failed command: the previous *after* is the next *before*).

Result, `<OutRoot>\<yyyyMMdd-HHmmss>-<Tag>\`:

```
report.md           headline and links
before\ after\      snapshots (see below)
trace\              summary.md write-phases.md timeline.md groups.md service-writes.tsv payloads.jsonl
                    data-writes.tsv ddl.sql transactions.tsv slowest.tsv sessions.tsv
                    events.tsv.gz trace-meta.json session.sql command.log
diff\               diff.md diff.json
```

A native ibcmd on a БСП database needs `--user=Администратор` (without it, with
stdin closed, it loops forever printing «Имя пользователя:»).

## snapshot.ps1

```powershell
pwsh -NoProfile -File scripts\apply-trace\snapshot.ps1 -Database <db> -Out <dir> `
     [-BlobStore <dir>] [-Reference <earlier snapshot>] [-DataChecksum full|counts|none]
```

Files: `meta.json`, `tables.tsv` (rows, used_kb, `CHECKSUM_AGG(BINARY_CHECKSUM(*))` and
the sum of the row checksums of every table), `columns.tsv`, `indexes.tsv`,
`constraints.tsv`, `objects.tsv`, `dbsettings.tsv`, `rows\<Table>.tsv`.

*Service tables* (`rows\`) are the file tables (`FileName` + `PartNo` + `BinaryData`:
Config, ConfigSave, ConfigCAS, ConfigCASSave, Params, Files, DepotFiles) and every
table whose name has no digit (DBSchema, SchemaStorage, IBVersion, v8users,
_SystemSettings, _ConfigChngR, ...).  Object tables (`_Reference123`, ...) have a
row count and a checksum only.

File tables: one line per file (parts are joined): size, stored bytes, sha256 of
the stored bytes, creation/modified **as stored (the platform adds a year offset,
`_YearOffset`, normally 2000: 4026-09-29 is 2026-09-29)**, `enc` (`deflate` when
the bytes are exactly one raw-deflate stream, else `raw`), decoded size and sha256,
`kind` (`v8text` for the `{...}` serialization, `text`, `container` for the 1C file
container, `binary`, `empty`), `blob` (Y when the content is kept), a preview.

The content of rows lives in a content-addressed **blob store** (`-BlobStore`,
append-only `pack-*.bin` files plus `index.tsv`; share one store between the
snapshots of a run: identical rows are stored once).  Policy: every row of
Params, Files, ConfigSave and ConfigCASSave; rows of Config/ConfigCAS that are
about to be replaced (their name is in ConfigSave/ConfigCASSave), the service
names of Config (`root`, `version`, `versions*`, `*.new`, `DynamicallyUpdated`,
...) and, with `-Reference`, every row that is new or different from the
reference snapshot.  `capture.ps1` passes the *before* snapshot as the reference of
the *after* snapshot, so both sides of every changed row are available to `diff.py`.
`-ContentMaxKB` (512) bounds one row, `-ContentBudgetMB` (1024) one snapshot.

Other service tables: `key` (primary key), `row_sha256` and a summary of the columns
(large columns are shown as length and sha; DBSchema/SchemaStorage keep up to 16 MB).

### Blob store

`pack-*.bin` files are append-only; `index.tsv` maps sha256 to (pack, offset, length).
Several snapshots (even of different databases) can run against one store at the same
time: the index is appended under a named mutex (`Global\ibcmd_rs_blobstore_<hash of the
directory>`) and packs are written under unique names.  If an index ever loses lines
(a process killed while appending, a copy of the folder taken mid-run):

```powershell
python scripts\apply-trace\blobstore.py verify <dir>    # every index line points to a valid record
python scripts\apply-trace\blobstore.py merge  <dir>    # add the records found in the packs but missing in the index
python scripts\apply-trace\blobstore.py rebuild <dir>   # index from scratch (scans the packs)
python scripts\apply-trace\blobstore.py stats  <dir>
```

Time: about 1.5 minutes on a БСП infobase (2,200 tables, 350 MB) on a busy
machine; the full checksum scan is most of it for big databases (`-DataChecksum
counts` skips it, `-ChecksumMaxMB` skips only the big tables).

## trace.ps1

```powershell
& scripts\apply-trace\trace.ps1 -Database <db> -Tag <t> -OutDir <dir> -Command { ... }
& scripts\apply-trace\trace.ps1 -Database <db> -Tag <t> -Exe <path> -ArgumentList a,b,c
& scripts\apply-trace\trace.ps1 -Database x -Tag x -Cleanup      # drop sessions left by a killed run
```

XE session `ibcmd_rs_04_<Track>_<n>` (default Track `trace`; the first free `n`),
filtered by `sqlserver.database_id`, events `rpc_completed`, `sql_batch_completed`,
`sql_transaction` (user transactions: begin/commit/rollback with the duration),
`begin/commit/rollback_tran_completed`, `error_reported` (severity 11+), optionally
`sql_statement_completed` (`-IncludeStatements`); actions: event sequence, session,
transaction id, client application, database.  Files under
`C:\temp\ibcmd_rs_04\<Track>\`, deleted after the export (`-KeepXel` keeps them).
The session is stopped and dropped even when the command fails.
`-AllDatabases` replaces the database filter by "non-system sessions" (use it when
the command also works in `master`, like `infobase create` may).

The `.xel` file has a unique name per run (`<session>_<yyyyMMddHHmmss>.xel`) and the
report only takes events after `started_utc` of `trace-meta.json` (`trace_report.py
--since`, default from `--meta`): a leftover file of an earlier run with the same
session name cannot leak into a new report (it did once: 60,000 old events).
`-Predicate '<XE predicate>'` replaces the database filter by any predicate (with
`trace_report.py --focus-db` the report keeps the statements of one database).

Limits worth knowing: `rpc_completed.statement` is cut by SQL Server at 2,000,000
characters, i.e. a binary parameter is visible only up to its first ~1,000,000
bytes (the report says `truncated by SQL Server`); `sql_batch_completed` keeps
the whole text.  Bulk-copy (`INSERT BULK`) row data is not visible in a trace: the
row diff is the evidence there.  Retention is `ALLOW_SINGLE_EVENT_LOSS` (needed for
`error_reported`); `trace-meta.json` and `summary.md` state the number of dropped events.

### Reading the report

* `summary.md` - what was done: statement counts by verb, writes per service table
  by row-name shape (`<guid>.<n>`, `<guid>_dynupdate_<guid>`, `root.new`, ...), DDL
  summary, errors, the transactions, the slowest statements.
* `write-phases.md` - **the phases of the run**: only what changes the database
  (DML on non-temporary tables, DDL), as blocks of consecutive writes to one table with
  operation x row-name shape counts, the user transactions inside the block, and the
  first and last row name.  Start reading here.
* `timeline.md` - all statements in order, consecutive equal statements collapsed
  into runs, with BEGIN / COMMIT / ROLLBACK markers (coarsened to ~400 lines).
* `groups.md` / `groups.tsv` - normalized statements (literals replaced) in order of
  first appearance with counts, total time, rows, the shapes of the string parameters.
* `service-writes.tsv` - **every** INSERT/UPDATE/DELETE on a service table in
  order: transaction, FileName, new FileName (rename), PartNo, rows affected,
  declared size, size and sha256 of the written bytes.  `payloads.jsonl` decodes the
  small written file rows.
* `ddl.sql` - CREATE/ALTER/DROP/TRUNCATE/sp_rename verbatim, in order (temporary
  tables are only counted).
* `transactions.tsv` - user transactions, from `sql_transaction` events.

## diff.py

```powershell
python scripts\apply-trace\diff.py --before <snapshot> --after <snapshot> [--out <dir>] [--blobs <store>]
```

`diff.md`: schema changes (tables created/dropped/altered with columns and indexes,
other objects, database options); per service table the rows inserted/deleted/updated
by name shape; updated file rows are classified as *only recompressed* (same decoded
bytes), *only formatting* (same `{...}` tokens or container elements: line breaks, base64
wrapping, CRLF) and *content changed*; only the last group is listed with the change
(module text of a container element, `versions` entries, `{...}` tokens); for data
tables the row count and checksum changes.  `diff.json` has the complete lists.

## Twins (two runs from identical state)

Stage once on clone X, `BACKUP DATABASE X ... WITH COPY_ONLY, COMPRESSION` to your lab
folder, restore the twin with `F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak <file>`
(see the header of that script), take the *before* snapshot on each, run each
variant under `capture.ps1`, and compare the two `after` snapshots with `diff.py`.

## Notes

* Dates in the platform's tables carry a year offset (see `year_offset` in
  `meta.json`).  Files and the text of `tables.tsv` are UTF-8, `\`, tab, CR, LF escaped.
* The kit reads only.  It is not a backup: a snapshot has no data of the object tables.

## lab/ - the case wrappers of the trace track

Recipes, kept so that a case can be repeated; they are not tested and not generic. They assume the lab of 2026-09-29:
disposable databases `ibcmd_rs_04_trace_*` made by `F:\ibcmd\lab\04\tools\restore-clone.ps1`, the named locks of
`heavy-lock.ps1` (one command per hold), the reference exports under `F:\ibcmd\lab\parity\` and `sqlcmd`. What they
document, and where the numbers are used: `docs/apply/native-apply-trace.md` section 2 (the cases), 6.4 (the path
experiment: `run_paths*.ps1`, `make_case1*_tree*.ps1`, `stage_vs_config.ps1`), 11 (8.5), 12 and `docs/apply/uh-case1-plan.md`
(`run_uh_case1.ps1`, prepared, refuses to run without `-Go`), `docs/apply/native-infobase-create.md` (`run_native_create.ps1`).

| Script | Purpose |
|---|---|
| `run_native_import_files.ps1`, `run_native_apply.ps1` | native staging (`config import files --partial`) and one traced native apply (kit capture, lock hooks) |
| `run_case3.ps1`, `run_queue.ps1`, `run_queue2.ps1`, `make_case2.py` | the БСП cases 1 - 3 of section 2 |
| `run_native_create.ps1` | traced native `infobase create` (8.3.27 and 8.5) |
| `make_case1_tree*.ps1` | hard-link trees of a reference export with a few module texts edited (no disk cost) |
| `run_paths.ps1` ... `run_paths6.ps1` | the path experiment (which staged sets make the native apply take the long path) |
| `validate_sparse_import.ps1`, `check_stage.py` | native staging from a sparse base directory; the `versions` completeness test of a stage |
| `stage_vs_config.ps1`, `dump_rows.ps1`, `row_sha.ps1`, `probe_stages.ps1`, `find_refs.ps1` | read-only row comparisons (ConfigSave against Config, decoded rows, hashes, which rows of a stage differ, which rows mention a guid) |
| `run_uh_case1.ps1` | the ERP УХ case 1 (prepared) |

## Remaining ERP УХ acceptance (#336)

See [the case 2/3 runbook](../../docs/apply/uh-remaining-traces.md). For captures that must
remain independently auditable, use `capture.ps1 -KeepXel -KeepRaw`: this retains both
the original XE files, `trace/events.xml.gz`, and the intermediate `trace/events.tsv`.
Direct `trace_report.py --xml` calls can use `--keep-intermediate` for the same retention.
The default behavior stays unchanged.
