# restructure-lab: research kit of the "ddl" track (issue #341)

Tools used to trace the native `ibcmd infobase config apply` on lab clones and to decode the structure of
the database (`DBSchema`, `DBNames`, `SchemaStorage`). Findings: `docs/apply/restructuring.md`; extracts:
`docs/apply/evidence/restructuring/`.

**Lab only.** Everything here writes only to databases named `ibcmd_rs_04_ddl_*` (the scripts check the
prefix) and reads other databases with `SELECT`. The paths default to the lab of the 0.4 tracks; set
`DDL_LAB` (lab folder), `DDL_DB` (default database), `DDL_TREE` (working XML tree) and `DDL_NATIVE_TREE`
(native export used as reference metadata) to use another location. Python 3.13 with `pyodbc` and the
"ODBC Driver 18 for SQL Server"; PowerShell 7; SQL Server with Windows authentication; a native 8.3.27.2214
`ibcmd.exe`.

## Capture

| script | what |
|---|---|
| `xe.ps1` | Extended Events helpers: `Start-DdlXe` / `Stop-DdlXe` (session `ibcmd_rs_04_ddl_<n>`, filtered to one database, files in `C:\temp\ibcmd_rs_04\ddl`), `Invoke-NativeApply` (native apply, notes other ibcmd processes seen while it ran) |
| `native_lock.ps1` | `Invoke-NativeLocked { ... }`: runs a native `config import` / `config apply` / `infobase create` under the lab `native` lock (`heavy-lock.ps1 acquire ddl -Name native` ... `release`), held for that command only; every script here that starts a native write uses it |
| `run_case.ps1` | one traced apply: light snapshot of the staged state, COPY_ONLY backup (twin source), XE session, native `config apply --force --dynamic=disable`, XE read, full snapshot, diff |
| `run_series.ps1` | cumulative cases: edit the tree, native import until the stage is complete (see below), `run_case` |
| `try_native.ps1` | fast loop: native import (+ apply) of a tree on a debug clone |
| `import_files.ps1` | native `config import files --partial` of some files of a tree: stages a delta of four rows (about a minute) |
| `apply_only.ps1` | native `config apply` of what is staged, under the lock; `-Trace <label>` adds an Extended Events trace (`xe/<label>/events.jsonl`) |
| `export_tree.ps1` | native `config export` of a database into a folder (read-only, no lock); compare trees with `ibcmd-rs source-diff` |
| `snapshot.py` | columns, indexes, row counts and checksums of all tables + service tables with content hashes into a snapshot folder; blobs go to a content-addressed store |
| `snapdiff.py`, `schemadiff.py`, `dbnames_diff.py` | diffs of two snapshots: tables/columns/indexes, service rows, `DBSchema` entries, `DBNames` |
| `xe_read.py`, `xe_shapes.py`, `timeline.py`, `extract_ddl.py` | XE file -> JSONL, normalised statement story per session, phase timeline, the structure statements as a readable SQL log |
| `si_diff.py` | the `Params` `*.si` cache rows between two snapshots, after inflate (the XDTO model row is shown decoded from its base64 block) |
| `compare_tables.ps1`, `compare_config.ps1`, `dbschema_cmp.py` | twin comparison (S1): the rows of tables of two databases with `EXCEPT` both ways (every column but the row version); the `Config` rows one twin has and the other has not, by name; `DBSchema` entries and `DBNames` text of two snapshots of two databases |
| `edit_cases_s2.py`, `stage_case.ps1` | S1 wave 1 (issues #398-#400): `edit_cases_s2.py <out> b1\|b2\|c1\|d0\|d1` makes the edited files of a case on the pristine БСП (delete attributes, delete an additional-order attribute, widen strings, the index flag alone, the index flag on and off); `stage_case.ps1 -Case <c>` restores a pristine clone, stages the case with the native partial import, snapshots and backs up the staged state and restores the two twins (`_nat`, `_own`) from the backup |
| `twin_run.ps1`, `entries_diff.py`, `assemble_evidence_run.py` | S1 wave 2 (S1-D and after): `twin_run.ps1` runs the whole twin protocol of 12.6 for a case whose native twin is applied: dry run, rehearsal (+ snapshot: nothing changed), the real run through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`, `twin_check.ps1`, a native apply on our twin, native exports + `source-diff`, a session job on both; `entries_diff.py` prints what a native apply changed in the `DBSchema` entries (fields and declared indexes) and in `DBNames` between two snapshots of two databases (the first thing to run on a new case); `assemble_evidence_run.py` makes the evidence file from the out files |
| `twin_check.ps1`, `inject_failure.ps1`, `tamper_stage.py` | the twin protocol of 12.6 for a case whose twins are both applied: `twin_check.ps1` = checks 2-6 in one run (snapshot, tables / columns / indexes, `EXCEPT` of every rebuilt table taken from the report, `Config`, `DBSchema` / `DBNames`, the `.si` rows); `inject_failure.ps1` = check 12 (the generated script with a `THROW` before `COMMIT` on a fresh twin, a digest of the whole database before and after); `tamper_stage.py` changes one staged row (inflate, edit, deflate) to make the refusals of check 11 testable |
| `edit_cases_s3.py`, `jobs/s2_e.bsl`, `refusals_e.ps1`, `dropin_run.ps1` | S1-E (#401): `edit_cases_s3.py <out> e1|e3|e4|e5|e6` makes the edited files of the cases on the pristine БСП (e1: five new tabular sections of three catalogs and two documents; e3: new attributes of tabular sections that were there, first / middle / last, indexed; e4: a catalog and a document with a new own attribute, a new attribute of an old section and new sections at once; e5, e6: the shapes of e1 and e3 on objects no extension adopts, which S1-I would refuse); `stage_case.ps1` stages them (the tree is `tree_s2\<case>`), `jobs/s2_e.bsl` is the cluster session job of all of them (sections and their attributes with the indexing, rows with a digest, XDTO of a new object with a row in every section, write, read back, query through each section); `refusals_e.ps1` = check 11 on a fresh twin of e4: three edits of the staged descriptor, each with a dry run and the S1 blocker it gives; `dropin_run.ps1` = the drop-in route once: `ibcmd-rs infobase config apply` without and with `--recovery-backup`, then the checks 2-8 against the native twin |
| `edit_cases_s4.py`, `mix_case.ps1`, `mix_ours.ps1`, `form_bindings.py`, `mix_forms.py`, `config_compare.py`, `assemble_mix_evidence.py` | S1 combinations (12.15): `edit_cases_s4.py <out> m1..m7|r1..r6` makes the stages that combine the built operations across objects no extension adopts (m1 B+C, m2 C+D on one attribute, m3 E+C+A, m4 B and E in different objects, m5 all five on six objects, m6 17 objects, m7 new indexed attributes; r1..r6 must be refused); `mix_case.ps1 -Case <c> [-Refused] [-Ours]` runs a case through the drop-in route `ibcmd-rs infobase config apply --recovery-backup` (stage with the platform, the platform apply on the native twin, dry run, drop-in, checks 2-8, or for a refused case exit 1 and the same digest of the database) and, with `-Ours`, through our import of the whole tree; `mix_ours.ps1` is the route through our import alone with one native write per case (`-Phase prepare|finish`; the drop-in twin and the platform's snapshot are the reference); `mix_forms.py` + `form_bindings.py` write the forms without the fields (and manual query columns) bound to removed attributes, what a whole tree needs; `config_compare.py` compares the `Config` rows of two databases by bytes and after inflate; `assemble_mix_evidence.py` makes the evidence file |
| `edit_cases_s5.py`, `uha_order_proof.ps1`, `schema_order_dump.py`, `diff_paths.py` | S1 combinations with S1-F (12.15): `edit_cases_s5.py <out> f1|f3|r7|r8` stages a new catalog / document (the files of the trace track's N1, N4, N5 cases, `newobj\`) next to other operations on the base that had a native apply (`MIX_BASE_BAK`, `MIX_BASE_TREE` for `mix_case.ps1` / `mix_ours.ps1`); `uha_order_proof.ps1` = the order of `DBSchema` on the ERP УХ (a fresh clone, rcheck's stage, the native import, our drop-in apply, `schema_order_dump.py --compare` against rcheck's positions); `diff_paths.py` = the files a `source-diff` lists against the edited forms |
| `params_row.ps1`, `cache_variant.ps1` | S1 cache experiments: read / replace / delete one `Params` `*.si` row of a lab database; start a server on a variant (a row absent or stale) and run a probe job (what the platform needs of each cache row, section 12.5) |

## Decode and check

| script | what |
|---|---|
| `bracefmt.py` | parser and writer of the platform's brace text; `dumps(parse(x)) == x` byte for byte (`rt_check.py`) |
| `dbschema.py` | model `DBSchema` -> SQL columns, indexes, implicit primary keys, DDL text |
| `dbschema_check.py` | the model against a real database dump (`schema.txt` of a snapshot): 1937/1937 tables on the 8.3.27 БСП, 1922/1922 on the 8.5 БСП |
| `md_types_check.py`, `gen_check.py` | XML type descriptors -> type entries; the field list of catalogs and documents generated from the XML tree and `DBNames`, compared with the real entries (94/94) |
| `names.py`, `lab.py`, `db.py`, `dbnames_kinds.py`, `make_cases_md.py` | `DBNames` parsing, snapshot store access, pyodbc helpers, the metadata class -> `DBNames` kinds table, the per-case evidence file |

## Sessions on a lab database

| script | what |
|---|---|
| `srv.ps1` | `start` / `stop` a stand-alone server (`ibsrv`) on one lab database under the caller's Windows account (the cluster service has no SQL login) |
| `session_job.ps1` | runs a piece of BSL (`-Job file.bsl`, the variable `Результат` is printed) in a thin-client session over HTTP (`-Port`) or, with `-Database`, in the local 1C cluster where the database was registered with the lab's `register-ib.ps1`; the client is a generic processing built from `probe/src` with the Designer |

## Edit the tree

`edit_tree.py`, `edit_cases_more.py`, `edit_cases_h.py`: the exact XML edits of the cases (a: attribute,
b: document attribute, c: new catalog, d: dimension + resource, e/g/f: widen / index / delete, h: attribute
types and tabular sections). Each edit keeps the original and the edited file under `tree/patches/<case>/`.
`edit85.py` makes case a on the 8.5 (2.21) dialect, `edit_second.py <tree> <catalog> <name> ...` adds a second / third attribute
(the `ALTER TABLE` experiment). `edit_cases_s1.py` makes the types case of S1 (T1): attributes of every primitive type
(boolean, strings of every kind, integer / fractional / non-negative numbers, dates, time) in five catalogs and a document,
one stage; `jobs/types_t1.bsl` is the session job that reads, serializes, writes and queries them. `jobs/s2_b1.bsl` and `jobs/s2_c1.bsl` are the session jobs of the deletion and widening cases (rows and a digest of the kept values, XDTO, write and read back, query by the widened attribute).

## Typical run

```powershell
$env:DDL_LAB = 'F:\ibcmd\lab\04\restructure'
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp8327 -Name ibcmd_rs_04_ddl_bsp8327_x -Track ddl -Purpose "..."
pwsh -NoProfile -File scripts\restructure-lab\run_series.ps1 -Cases a -Prev <snapshot label> -Database ibcmd_rs_04_ddl_bsp8327_x
python scripts\restructure-lab\snapdiff.py <db> <before-label> <db> <after-label>
```

Every native write goes through the lab `native` lock (`native_lock.ps1`), one command per hold. A native `config import` is complete when
`ConfigSave` has a `versions` row, no `commit` and no `*.new` rows, and every staged `versions` entry that differs from `Config`'s has its
row (a complete stage of the БСП is about 9 618 rows in a full import, 4 rows with `--partial`; the row count alone is not the test).

## S1: the same run through the apply (issue #391)

```powershell
# the S1 gate lets the stage through and the apply runs ONE transaction: the structure phase, then the promotion
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_X --through-apply --dry-run                 # plan and check, write nothing
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_X --through-apply --rehearse               # run everything, roll it back
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_X --through-apply --report r.json --script-output t.sql --recovery-dir rec
# compare with the native twin (docs/apply/restructuring.md, 12.6)
pwsh -NoProfile -File scripts/restructure-lab/compare_tables.ps1 -A <nat> -B <ours> -Tables _Reference20,_Document39
pwsh -NoProfile -File scripts/restructure-lab/compare_config.ps1 -A <nat> -B <ours>
python scripts/restructure-lab/dbschema_cmp.py <nat> <label> <ours> <label>
python scripts/restructure-lab/si_diff.py <nat> <label> <ours> <label>
```

## The prototype's twin run (checkpoint 2)

```powershell
# 1. the staged image once (native import), a COPY_ONLY backup of it, two twins from the backup
sqlcmd -S localhost -E -C -b -Q "BACKUP DATABASE [X] TO DISK = N'<lab>\bak\X_a2_staged.bak' WITH COPY_ONLY, COMPRESSION, INIT"
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak <lab>\bak\X_a2_staged.bak -Name ibcmd_rs_04_ddl_a2_nat -Track ddl -Purpose "..."
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bak -Bak <lab>\bak\X_a2_staged.bak -Name ibcmd_rs_04_ddl_a2_fin -Track ddl -Purpose "..."
# 2. native on one twin (apply_only.ps1), the prototype on the other
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_a2_fin --dry-run --dump-plan <dir>
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_a2_fin --trial      # runs and rolls back
ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_a2_fin --report <file.json>
# 3. compare: snapshots, data with EXCEPT, the *.si rows, exports, a native no-op apply, sessions
python snapshot.py <db> <label>; python snapdiff.py <native db> <label> <our db> <label>
python si_diff.py <native db> <label> <our db> <label>
pwsh export_tree.ps1 -Database <db> -Out <dir>;  ibcmd-rs source-diff <native tree> <our tree>
pwsh apply_only.ps1 -Database <our db>            # "Обновление конфигурации базы данных не требуется"
pwsh F:\ibcmd\lab\04\tools\register-ib.ps1 register -Database <our db> -Platform 8.3.27 -Track ddl
pwsh session_job.ps1 -Job <file.bsl> -Database <our db>
pwsh F:\ibcmd\lab\04\tools\register-ib.ps1 unregister -Database <our db> -Platform 8.3.27 -Track ddl
```
