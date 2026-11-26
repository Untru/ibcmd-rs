# ERP УХ, case 1 (modules only): plan for the traced native apply

Part of [#336](https://github.com/Untru/ibcmd-rs/issues/336); the БСП measurements this plan scales from are in
`native-apply-trace.md`. **Status: run on 2026-09-29 (17:14 - 17:59); the result is section 12 of `native-apply-trace.md`**, this
file keeps the plan and, at the end, what the run showed against it. The run held the `native` lock for 8.7 min. Every number
below is the estimate made before the run: either **measured offline** (the reference export, the backup header) or **scaled from
the БСП traces**.

## 1. Purpose and scope

Acceptance of #336 (agreed with the coordinator on 2026-09-29): the БСП cases 1-3 of `native-apply-trace.md`, this case,
and the ERP УХ end-to-end runs of ibcmd-rs 0.3 (native `infobase create`, our import, native apply, native export
identical) as the new-database evidence on УХ. The УХ cases 2 (attribute) and 3 (new database) are **not** traced.

What the case adds to the БСП traces: the write set and the cost of a text-only change (five module texts, like case 1)
on a base twelve times larger, that is which costs scale with the size of the base (the reads of the check phase, the
change register, the CAS garbage collection, the caches) and which do not (the writes per staged row, the fixed
families of section 3.2 of `native-apply-trace.md`).

## 2. Size of the base (measured offline)

| | БСП 8.3.27 | ERP УХ 8.3.27 |
|---|---|---|
| reference export (`native` folder of the parity runs) | 12 198 files | 140 709 files, 9 138 MB (76 s to walk) |
| `ConfigDumpInfo.xml` entries with `configVersion` | 9 835 | 118 167 (file 52.6 MB; `Configuration.xml` 2.4 MB) |
| `Config` rows | 9 841 | about 118 200 (БСП: entries + 6) |
| backup used for a clone | `bsp_native_20260923.bak`, 0.75 GB | `F:\ibcmd\lab\dbbak\uha_parity2_20260924.bak`, 1.72 GB; data file 4.20 GB, log 3.57 GB (`restore-clone.ps1` shrinks the log) |

The export lives on `E:\`, which is read-only, and a hard-link tree cannot cross volumes; a copy of 9 GB is not
allowed. The staging therefore uses a **sparse base directory** with the edited files only (section 4).

## 3. Steps

Locks are FIFO and one command is held at a time; only step 5 holds `heavy` and `native` together. The runner is
`scripts/apply-trace/lab/run_uh_case1.ps1` (lab copy in `F:\ibcmd\lab\04\trace\scripts\`); it refuses to run without
`-Go`.

| # | step | lock | estimate | check / stop |
|---|---|---|---|---|
| 0 | guards: the coordinator's go and OK for the clone; free space on `F:` at least 40 GB (250 GB at the time of writing); the database must not exist | - | seconds | the runner throws otherwise |
| 1 | `restore-clone.ps1 -Corpus uha8327 -Name ibcmd_rs_04_trace_uh_mod -Track trace ...` | - | about a minute (the БСП restore of 0.77 GB took 5 s at 160 MB/s; 4.2 GB is 30 s plus the log shrink) | the database is listed in `databases.tsv` by the tool |
| 2 | sparse base directory: the five edited `.bsl` files (446 KB) and the padding modules (5 files, 4 KB), a comment line appended (BOM and CRLF kept) | - | seconds | the source files were checked to exist (section 4) |
| 3 | native staging: `ibcmd infobase config import files --base-dir <sparse> --partial <files>` | `native`, one command | БСП: 29.6 s for two files; УХ: **unknown**, guess 1 - 3 min (the platform reads the `versions` row of 118 k entries, about 9 MB decoded) | `check_stage.py` says COMPLETE (no `commit`, no `*.new`, `root`/`version`/`versions` present, every changed guid of `versions` has a `ConfigSave` row; the row count alone is not a test) and the runner prints the predicted path (section 4) |
| 4 | `capture.ps1`: before snapshot with `-DataChecksum counts` | - | БСП: 448 s for 2 325 tables at a busy machine; УХ: the table count is known after the restore (expect several times more), row dumps of the service tables up to `-RowDumpMaxRows` (300 000: the change register of about 250 k rows is dumped, which keeps the `_MessageNo` rule checkable) - **guess 15 - 45 min** | none |
| 5 | XE trace and `ibcmd infobase config apply --force --dynamic=disable --user=Администратор` | `heavy`, then `native`, acquired by the `-BeforeCommand` hook after the snapshot, released by `-AfterCommand` the moment the command ends | see section 5: 6 - 15 min without the CAS garbage collection, up to about 60 min with it | `-TimeoutMinutes 90`: the kit kills the command, the hook releases the locks |
| 6 | after snapshot, `diff.py`, `trace_report.py` | - | 30 - 60 min (section 6) | |
| 7 | clean-up: sparse directory, `ibdata\<db>` (768 MB in every БСП run), `events.xml.gz`; STATUS.md; the database goes to "Databases no longer needed" | - | | |

Optional second run on the same clone (steady state, one more hold of about the steady-state time of section 5): it
separates the one-time garbage collection from the recurring cost. Not planned unless the coordinator asks.

## 4. The edits

Five files of the reference export, each with one comment line appended, exactly like case 1 on the БСП:

| kind | path under the export root | size |
|---|---|---|
| common module | `CommonModules\СтроковыеФункцииКлиентСервер\Ext\Module.bsl` | 103 514 B |
| catalog object module | `Catalogs\ВидыКонтактнойИнформации\Ext\ObjectModule.bsl` | 6 230 B |
| document manager module | `Documents\ЗаказПоставщику\Ext\ManagerModule.bsl` | 311 864 B |
| managed form module | `Catalogs\Валюты\Forms\ФормаЭлемента\Ext\Form\Module.bsl` | 23 063 B |
| common form module | `CommonForms\ВопросОбУстановкеВнешнейКомпоненты\Ext\Form\Module.bsl` | 1 427 B |

(All five paths exist; checked read-only on 2026-09-29.) Variant `forms` (default) stages all five, variant `containers` the
first three. **Padding**: `-Pad N` (default 5) adds N small common modules of the БСП library that the УХ configuration
contains too (`ОплатаСервисаКлиентПереопределяемый`, `СообщенияВМоделиСервисаПовтИсп`, `ЭлектроннаяПодписьВМоделиСервисаПереопределяемый`,
`ОповещениеПользователейБТСПереопределяемый`, `ТарификацияВызовСервера`, ... 500 - 1 300 B each, all present, checked), a comment
line appended to each. The reason is the path rule of `native-apply-trace.md` section 6.4: the long path is taken when a staged
descriptor differs from `Config` **or when more than 20 rows are staged** (a common module is two rows, and `root`,
`version`, `versions` are three). The five edits alone stage 13 rows (short unless the native import re-saves a catalog or
document descriptor, which it did on the БСП: Y8, Y10, Y11); with the default padding the stage has 23 rows, so the apply is
long by the row rule whatever the descriptors do, and it shows every phase of section 3.2 at УХ scale. `-Pad 0` gives the bare
case. The runner prints the predicted path after the staging (a staged descriptor that differs from `Config`, or the row count)
and keeps the comparison in `uh-stage-vs-config.txt`.

The staging is **native** (`import files`), not ours: our `mssql-stage-source-objects` needs the whole dependent tree
(`CommonCommands` for the forms) and would need the 9 GB tree; the sparse import was validated on the БСП (two edited
files, 29.6 s, 7 `ConfigSave` rows: the descriptors, the two module rows, `root`, `version`, `versions`). One thing to watch: a
native import of two files staged only one of them once (Y14 of section 6.4, reason not established); `check_stage.py` counts the
rows, and the runner prints them, so an unexpected count is visible before the apply.

## 5. What to expect (estimates and their derivation)

**Statements.** The БСП steady state (1x', 4 627 statements) is 3 979 SELECTs and about 650 writes. The reads scale with
the base: 2 281 single-row `Config` reads, 659 `_DataHistoryMetadata` probes (one per metadata object that has a history
table), 328 `ConfigCAS` reads, 268 `_ExtensionsRestruct` reads and 23 IN-list reads of 214 descriptors each: 0.40 reads
per `Config` row. For УХ that is about **47 000 reads** (`_DataHistoryMetadata` may scale with the number of objects
instead of rows). The writes are the fixed families (about 350 statements) plus 8 per staged row plus the register
(`INSERT BULK`, batches of 10 000 rows: about 50 statements for 2 x 250 k rows). **Without the CAS garbage
collection: about 50 000 statements.**

**CAS garbage collection** (first apply of a lineage only): one `DELETE FROM ConfigCAS` per unreferenced row: 12 160 on the
БСП 8.3.27 (74 s, 6.1 ms each), 18 627 on the БСП 8.5 (219 s, 11.8 ms each), 1.2 - 1.9 rows per `Config` row. The
number of `ConfigCAS` rows of the УХ backup is **unknown** (the backup is not opened before the go). At the same ratio:
140 000 - 220 000 deletes, 14 - 45 min. If the УХ lineage was collected by an earlier native apply: none. **Total: 50 000
to 270 000 statements.**

**Time.** Steady state 30 s (8.3.27) - 53 s (8.5) on 9.8 k rows, of which 9.5 - 12.8 s is the check; times twelve: 6 - 10
min; register rebuild (БСП 6.5 s for 42 k rows) about 80 s; the 16 `.sinew` rows (4 MB written on the БСП) may reach 50 MB. The lock hold is 6 - 15 min
without the garbage collection, up to about 60 min with it.

**Long or short path.** With the default padding: predicted long (23 staged rows, section 4). The statement counts above are
the long path; the short path (2n: 2 322 statements on the БСП) has no register rebuild, no `DBNames`, no garbage collection: the
reads remain. The rule was found on the БСП; whether the limit of 20 rows scales with the size of the base is not known, which is one
more thing the УХ run shows (the runner prints the prediction, the log of the apply gives the outcome).

## 6. Capture settings for 100 k+ statements

```
capture.ps1 -Database ibcmd_rs_04_trace_uh_mod -Tag uh-case1-forms-exclusive -Track trace
    -DataChecksum counts -MaxStatementKB 32 -ContentMaxKB 256 -ContentBudgetMB 512 -TimeoutMinutes 90 ...
```

| setting | БСП default | УХ | why |
|---|---|---|---|
| `-DataChecksum` | `full` | `counts` | the checksum scan reads every data table (4.2 GB); the diff needs the service tables, whose rows are dumped in any case |
| `-MaxStatementKB` | 1024 | 32 | statement text is kept up to this size; the register switch ships the schema text (0.98 MB on the БСП, about 12 MB at УХ scale) in one statement, and the XML export of the events is the memory hog of the report |
| `-ContentMaxKB` / `-ContentBudgetMB` | 512 / 1024 | 256 / 512 | rows larger than 256 KB stored (`versions` at УХ scale is about 1 MB stored, 9 MB decoded) are not kept in the blob store; the budget bounds the store per snapshot |
| `-RowDumpMaxRows` | 300 000 | default | the change register (about 250 k rows) and its `ExtProps` table stay under it |
| `-TimeoutMinutes` | 90 | 90 | the hold of the `native` lock is bounded |
| `-IncludeStatements` | off | off | it would double the events |

Cost of the report (measured on the trace of the 9 517-row tree import, `memtest`): 109 619 events -> 319 s and 443 MB
peak working set. Linear extrapolation: 50 k events 2.5 min / 0.2 GB; 270 k events 13 min / 1.1 GB (an earlier, more
pessimistic estimate was 27 min and 2.2 GB for 500 k events). `timeline.md` is coarsened to about 400 lines and
`write-phases.md` collapses consecutive writes to one table, so the size of the *reports* does not grow with the number
of statements (the CAS garbage collection is one block of `write-phases.md`); `service-writes.tsv` has one line per write
(200 k lines of the garbage collection are about 30 MB).

## 7. Disk use (estimate)

| what | size | remark |
|---|---|---|
| clone, data file | 4.2 GB, plus growth during the apply (the two `..NG` register tables, the rewritten caches): up to +1 GB | stays until Pavel drops the database; listed in STATUS.md |
| clone, log | shrunk by `restore-clone.ps1`; simple recovery; the largest transaction is the register bulk load: estimate below 2 GB | |
| `--data` directory of the apply | 768 MB (every БСП run) | deleted after the run |
| sparse base directory | 0.45 MB | deleted |
| XE files (`C:\temp\ibcmd_rs_04\trace`) | 1.5 KB per event: 0.1 - 0.4 GB | deleted by the kit after the export |
| transient XML export of the events | up to about 3 GB | deleted by the kit |
| snapshots `before` / `after` | `Config` rows as TSV (118 k rows), the change register (250 k rows, about 25 MB each), the columns/indexes of every table (the БСП: 17 - 22 MB per snapshot for 2 325 tables) | about 0.2 GB for both |
| blob store growth | at most the budget, 512 MB per snapshot; the БСП: 0.3 MB | |
| retained reports | `service-writes.tsv`, `groups`, `timeline`, `ddl.sql`, `diff` | **about 0.6 GB** |
| **peak on `F:`** | clone 5 - 6 GB + transient 4 - 5 GB | **about 10 - 12 GB, retained about 0.6 GB + the clone** |

## 8. Stop conditions and risks

* The command runs longer than the timeout: the kit kills it and the hook releases both locks; the clone is then in an
  interrupted state (`commit`, `*.new` rows), which is a result too, but the case is not complete.
* `import files` fails or the stage is not COMPLETE: nothing is applied; report to the coordinator.
* Free space on `F:` below 25 GB at any point: stop and tell the coordinator (the disk rules of 2026-09-29).
* The XE session loses events (`dropped_events` in `trace-meta.json` is not 0): the statement counts are then lower
  bounds; the row diff is unaffected.
* Another track needs `native` while the hold lasts: FIFO makes it wait; the hold cannot be shortened after the command
  has started. This is the reason for the go.

## 9. What the run showed against the plan (2026-09-29)

| | plan | measured |
|---|---|---|
| statements | 50 000 - 270 000 | 28 701 (no garbage collection: `ConfigCAS` of the corpus is empty; 0.24 reads per `Config` row against the assumed 0.40) |
| CAS garbage collection | none to 220 000 deletes | none |
| lock hold (`heavy` + `native`) | 6 - 15 min, up to 60 | 8.7 min (522.1 s), released when the command returned |
| stage | 23 rows with `-Pad 5`, predicted long | 23 rows, native `import files` 40 s, COMPLETE; the structure line appeared |
| disk peak | 10 - 12 GB | about 5.5 GB (clone 4.3 GB, `ibdata` 0.77 GB, capture 123 MB) |
| snapshots | 15 - 45 min each | 10 and 18 min |
| export of the trace | (not estimated separately) | 7.5 min for 28 753 events: the slow step of the kit (64 events/s) |
| `--user` | needed on БСП | not needed: `v8users` of the corpus is empty |

Two things the plan did not foresee: the corpus has no register rows, no extensions, no CAS rows and no users, so the apply wrote
less than the БСП long path although the structure line appeared; and it wrote a few rows the БСП applies do not
(`ibparams.inf`, `locale.inf`, two `.ui` rows). Both are in `native-apply-trace.md` sections 12.3 and 12.4. The runner grew
`-SkipRestore` (the clone was restored by hand first, to look at it) and needs no `--user` here.
