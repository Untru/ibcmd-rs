# S1-J: a limit on the tables the own restructuring rebuilds (#406)

Issue [#406](https://github.com/Untru/ibcmd-rs/issues/406), part of [#391](https://github.com/Untru/ibcmd-rs/issues/391);
track ui, 2026-09-30. Checkpoint 1 (the place of the check and the measurement plan) was accepted; this is checkpoint 2:
the code, the measured table, the limit derived from it, and the refusal proven on twins. The design the limit
belongs to is `docs/apply/restructuring.md`, section 12 (S1-J is row 12.7). What is measured says so.

## 1. Summary

- **What the log is.** The own restructuring rebuilds a table through its `NG` twin in the apply's one transaction. Under
  full recovery the log of the structure phase is **`2 x data + indexes`** of the rebuilt tables: the load into a heap
  writes the data once, the clustered index built over the heap once more, the other indexes once. Measured on 17 runs
  and probes (10 thousand to 4.8 million rows, four row shapes, real tables): 0.52 to 1.05 of that figure, the whole
  apply transaction 1.05 (section 4).
- **The limit** (a default the operator may change): the stage's rebuilt tables may hold **10 000 000 rows** and may
  make the rebuild write **2 GiB** (`2 x data + indexes`, summed over the stage). Above it the stage is refused, nothing
  is written, and the operator is sent to the native `config apply`, which commits in steps. The 2 GiB is half of the
  4 GiB log the budget allows; the derivation is section 5.
- **Where it plugs in** (decided at checkpoint 1): the gate side, after `decide`, over `phase.tables`, only for
  `Method::Rebuild`; one new file, `src/restructure/size_guard.rs`.
- **Raised or lowered by** `--restructure-limit-rows` / `--restructure-limit-bytes` of `mssql-config-apply` and
  `mssql-restructure`, and for every command by `IBCMD_RS_RESTRUCTURE_LIMIT_ROWS` / `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES`
  and the keys `restructure-limit-rows` / `restructure-limit-bytes` of `ibcmd-rs.toml` (section 3). The drop-in
  `ibcmd infobase config apply` takes no new flag. A raised limit still needs a backup option.
- **Proof on twins** (section 6): a table above the limit is refused with nothing written (a fingerprint of the
  database is equal before and after); a stage below it goes through and equals the native apply on checks 3, 4, 7 and 8
  of the twin protocol (12.6).

## 2. What was built

| file | what |
|---|---|
| `src/restructure/size_guard.rs` (new) | `RestructureLimit`, `LimitSetting` (the limit and where each half came from), `TableSize` (`rebuild_bytes` = 2 x data + indexes), `read_sizes` (one query on `sys.dm_db_partition_stats` over the plan's tables), `evaluate` (pure), `SizeCheck::refusal` / `to_json`, `parse_byte_size`, `resolve_limit` (flag, then the settings chain, then the default), `guard_phase` (what the gate calls) |
| `src/restructure/tests_size_guard.rs` (new) | 15 tests: at the limit passes, one above refuses, the sum over the stage, the largest table named, a missing table is 0, a name that is no plain identifier is not put into a query, the gate blocks and drops the phase, `AlterAdd` is not guarded, a server that cannot answer fails closed, units |
| `src/settings/restructure.rs` (new) | the two settings (environment, file) and their 5 tests; `settings.rs`, `settings/files.rs`, `settings/show.rs` gain the names, the two keys and two lines of `settings show` |
| `src/restructure/s1.rs` | `S1Gate` holds the limit (`size_limit(..)` next to `xml_version`) and calls `guard_phase` after `decide` |
| `src/restructure/command.rs` | `mssql-restructure`: the two flags, the guard after `make_plan` (for `--dry-run`, `--trial` and the apply), the report gets `size_check`, `--through-apply` builds the gate with the limit |
| `src/mssql_config_apply/mod.rs`, `gate.rs`, `cli.rs`, `dropin/apply.rs` | `ConfigApplyOptions.restructure_limit`; `StructurePhase.size_check` (in the report, `structure.size_check`); the flags of `mssql-config-apply`; the drop-in reads the settings chain only |
| `scripts/restructure-lab/size-limit/` (new) | the measurement kit (section 8) |

The refusal is a blocker of the S1 gate like every other reason (`S1: ...`), so the apply's existing path carries it
(`needs_native_apply`, exit 1; the drop-in words it `требуется штатный config apply: ...`). A server that cannot
answer the size query (no `VIEW DATABASE STATE`) fails the gate closed. The guard is not repeated inside the
transaction script: the apply holds an exclusive lock, the window between the gate's read and the transaction is the
gate's seconds, and the script is what checks 10-12 of the twin protocol pin.

## 3. The settings

Rows and bytes are chosen independently, highest first: the flag; the environment; the settings files (the highest
layer that sets it); the default.

| | rows | bytes to write |
|---|---|---|
| flag (`mssql-config-apply`, `mssql-restructure`) | `--restructure-limit-rows 5000000` | `--restructure-limit-bytes 4GB` |
| environment | `IBCMD_RS_RESTRUCTURE_LIMIT_ROWS=5_000_000` | `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES="4 GiB"` |
| `ibcmd-rs.toml` | `restructure-limit-rows = 5000000` | `restructure-limit-bytes = "4GB"` (a number of bytes also) |
| default | 10 000 000 | 2 GiB |

Rows: a whole number (`_` allowed). Bytes: a whole number or a number with a unit, every unit a power of 1024
(`512MB`, `4 GiB`, `1.5g`). A value of 0, a malformed value or an unknown unit fails when the command asks for the
limit, naming the flag, the variable, or the file and line. `ibcmd-rs settings show` prints both with their source;
the report of an apply prints the limit, where it came from, and what the stage rebuilds (`structure.size_check`).

The drop-in `ibcmd infobase config apply` is the platform's syntax and gets no flag: `connect` in `dropin/apply.rs`
reads the two settings from the chain and puts them into the apply's options; `restructure_gate` builds the S1 gate with
that limit (`FirstThen(ApplyCheckGate, S1Gate)`, `mssql_config_apply/mod.rs`), so the drop-in exercises it. A raised limit changes nothing else: the apply still refuses a
restructuring without `--recovery-backup` or `--i-have-a-backup` (`backup_required`); section 6 shows both.

## 4. The measurements

### 4.1 Instruments and twins

- **Tool.** The existing `ibcmd-rs mssql-restructure --database <twin> --trial` (the rebuild in one transaction, the
  read-back, `ROLLBACK`; its JSON report has the milliseconds of each phase) and `--through-apply --rehearse` (the whole
  apply script, rolled back), built from the base of the branch without the guard (`iter` profile). One committed rebuild
  (`mssql-restructure` without `--trial`).
- **Sampler.** `scripts/restructure-lab/size-limit/s1j.py poll`, its own process connected to `master` (the tool refuses a
  database another session holds; ODBC pooling is off in the kit for the same reason), every 250 ms:
  `database_transaction_log_bytes_used` and `..._reserved` of the twin's open transaction, `DBCC SQLPERF(LOGSPACE)`, file
  bytes written, tempdb allocated pages, the running statement. It stops on a stop file and at the latest after
  `--max-seconds`; `trial` checks that it is gone. No sampler process survived any run.
- **Twins.** `restore-clone.ps1` from the delta-staged and whole-image staged backups of the ddl track (read only): the
  case a2 (`Catalog._ДемоПартнеры`, main table `_Reference20`, sub-tables `_VT155`, `_VT159`) and the types case. The
  tables were grown by `s1j.py grow` (batched `INSERT ... SELECT`, in SIMPLE): **narrow** (`_Reference20`, about 1.6 KiB per row
  with its five indexes), **thin** (`_VT155`, 105 bytes, the clustered index only), **wide** (`_VT159`, 2 KiB of strings),
  **LOB** (`_VT159`, two `nvarchar(max)` values of 20 KB). SIMPLE for the growth, then `ALTER DATABASE ... SET RECOVERY FULL`
  with a full backup to `NUL` before the FULL runs and a log backup to `NUL` before each run.
- **The log figure.** The whole-image stage of a2 promotes 9 838 `Config` rows, which costs 270 MB of log by itself; the
  S1 gate refuses a whole image and accepts only a delta, so the figure that counts is the **structure phase**: the
  transaction's log up to the first statement that moves the staged rows (`structure_log_used`). The whole apply
  transaction of a delta stage was measured too (R15, R16).

### 4.2 The table (FULL recovery; `2d+i` = 2 x data + indexes, all in MB of 10^6 bytes)

| run | rows | data | indexes | 2d+i | structure log | log/(2d+i) | structure s | rollback s |
|---|---|---|---|---|---|---|---|---|
| narrow, 10 thousand rows | 10 061 | 5 | 10 | 21 | 21 | 1.02 | 0.5 | 5 |
| narrow, 100 thousand | 100 047 | 52 | 101 | 206 | 217 | 1.05 | 29 | 7 |
| narrow, 1 million | 1 000 047 | 573 | 1 127 | 2 273 | 2 071 | 0.91 | 36 | 32 |
| narrow, 3 million | 3 000 047 | 1 660 | 3 228 | 6 549 | 6 209 | 0.95 | 233 | 92 |
| thin: 4 million rows of 105 bytes | 4 000 032 | 441 | 0 | 882 | 831 | 0.94 | 15 | 30 |
| wide: + 200 thousand rows of 2 KiB | 4 200 014 | 854 | 25 | 1 734 | 1 679 | 0.97 | 22 | 29 |
| wide: + 800 thousand rows of 2 KiB | 4 800 014 | 2 094 | 147 | 4 335 | 4 235 | 0.98 | 62 | 23 |
| LOB: + 20 thousand rows of 40 KB | 4 820 014 | 2 922 | 154 | 5 999 | 5 096 | 0.85 | 72 | 35 |
| R15: committed rebuild, 700 thousand narrow rows (whole transaction, delta stage) | 700 061 | 365 | 696 | 1 426 | 1 476 | 1.04 | 69 | (committed) |
| R16: rehearsal of the whole apply script, the same twin | 700 061 | 365 | 696 | 1 426 | 1 493 | 1.05 | (rolled back) | |

Real tables of the БСП demo (`s1j.py probe`: an empty heap of the same columns, `INSERT ... WITH(TABLOCK) SELECT`,
the indexes built after the load, the log read before `ROLLBACK`; the biggest БСП table is 45 MB), log over `2d+i`:
`_InfoRg5222` 0.90, `_InfoRg5253` 0.82, `_InfoRg6498` 0.93, `_InfoRg5906` 1.02, and two LOB-heavy tables 0.52 and 0.52;
a compact 700 thousand row catalog table, 1.00. The raw rows of every run: `docs/apply/evidence/restructuring/size-limit/measured-runs.csv`.

What the table says:

1. **`2 x data + indexes` predicts the log** within 0.85 to 1.05 for the copies, and the whole apply transaction (R15,
   R16) is 1.04 and 1.05 of it. It does not depend on the row shape (narrow rows are index-bound, wide rows data-bound,
   thin rows have no index) or on the size (four orders of magnitude). The reason is in the plan: the copy loads a heap
   (`INSERT ... WITH(TABLOCK)`, the data once), then `CREATE ... CLUSTERED INDEX` rebuilds the data (once more), then the
   other indexes. LOB pages are not written twice (0.85), so the model reads high there.
2. **A row term is not needed at 105 bytes per row** (0.94 for 4 million rows). Thinner rows are unmeasured; they are
   what the rows limit is for (section 5).
3. **Recovery model.** In SIMPLE the same rebuild writes 0.5% of that: 0.5 MB, 5 MB, 10 MB and 34 MB at 10 thousand,
   100 thousand, 1 million and 3 million narrow rows (the load into a heap is minimally logged). The limit is derived from FULL,
   which is what a production base usually runs; a SIMPLE base can raise it.
4. **Time.** The structure phase takes 12 to 36 seconds per GiB written; the index phase dominates. 233 s for the 6.5 GB
   of the 3 million row case. The times vary two to three times between repeated runs on this shared machine (the
   10 thousand row index phase took 7.7 s once and 0.2 s in the next run), so they justify an order of magnitude, not a
   number.
5. **Rollback** of a trial takes a quarter to twice the structure phase (92 s after 233 s, 30 s after 15 s) and its own
   log is small (0.18 to 0.26 GB, the promotion's part); the log reserved for it is 1 to 3% of the used log.
6. **tempdb** (`SORT_IN_TEMPDB`) peaked at +1.0 GB at 3 million narrow rows, +0.9 GB at the wide case.
7. **One failure.** The first LOB run ended with error 1205 (chosen as the deadlock victim) in
   `copy _Reference20_VT159 into _Reference20_VT159NG`, rolled back cleanly (exit 1); the repeat succeeded. Seen once and
   not reproduced; by decision (2026-09-30) it is not investigated. A robustness note for the ddl track (the copy of a
   table with 20 KB `nvarchar(max)` values).
8. **The УХ confirmation of the checkpoint-1 plan could not be made on real data.** `uha_parity2_20260924.bak` holds
   the configuration only: every table has 0 rows but 844 (`_Enum4473`) or fewer. The clone was restored, surveyed and
   dropped at once (4.3 GB). The calibration on real tables was made on the БСП demo instead (six probes above), whose data is
   real but small; the model holds there too, and the guard never fires on either demo configuration (the limit is 40
   times the biggest БСП table).

## 5. The limit

The budget (yours, section 6 of checkpoint 1): a **log of 4 GiB**, and an **exclusive window under ten minutes**. Derived
from FULL recovery.

- **Log.** Log = `rebuild_bytes` x at most 1.05 (worst measured; the guard's estimate uses 1.10). A log of 4 GiB allows
  `rebuild_bytes` up to 3.6 GiB. **The default is 2 GiB**, half of that, for what the model cannot see: the rest of the
  transaction (the fold, the move of the staged rows, the caches: 0.1 to 0.3 GB), the rollback, and a source table that is
  page-compressed (the platform creates the `NG` tables without compression, so the rebuilt table is larger than
  the `used_page_count` the guard reads). At the limit the estimated log is 2.2 GiB.
- **Time.** At 12 to 36 s per GiB written, a stage at the 2 GiB limit takes about 25 to 75 s of structure phase, plus
  the gate (about 10 s) and the promotion of a delta stage (under a second). The ten-minute window would allow about
  12 GiB; the log binds first.
- **Rows.** 10 000 000. The byte limit binds first for every row of 210 bytes or more (a narrow catalog row is 2.3 KiB to
  write). The rows limit is for thin rows, whose per-row log the model reads low: even 100 unmodelled bytes of log per
  row on 10 million rows are 1 GB, which the half-budget absorbs.
- **Refused stages are the exception.** The biggest table of the БСП demo is 45 MB to write; the limit is 2 GiB. A stage
  is refused only on a base with a catalog or document of hundreds of thousands of rows.

The numbers are constants in `size_guard.rs` (`DEFAULT_LIMIT_ROWS`, `DEFAULT_LIMIT_BYTES`, `LOG_PER_REBUILD_BYTE`),
pinned by a test; the coordinator fixes them after this table.

## 6. The refusal and the pass, on twins

Twins from the types-case backup (a delta stage of 9 rows: six objects, 11 tables), `Catalog._ДемоПартнеры` grown
with narrow rows; the binary with the guard (`iter` profile).

**Above the limit** (1 200 061 rows: 672 MiB of data and 1.3 GiB of indexes, 2.6 GiB to write):

| run | result |
|---|---|
| `mssql-config-apply --allow-restructure s1 --dry-run`, default limit | refused, `needs_native_apply`, exit 1: "the stage rebuilds 11 tables with 1200812 rows, 672.0 MiB of data and 1.3 GiB of indexes; ... log would grow by about 2.9 GiB; bytes to write (the data twice and the indexes once): 2.6 GiB above the limit of 2.0 GiB (the default). The largest is _Reference20: 1200014 rows, 2.6 GiB to write. Run the native `ibcmd infobase config apply` for this stage ..." |
| the same, a real run with `--i-have-a-backup` | refused the same way |
| `mssql-restructure --trial` | refused before it writes, the same words, exit 1 |
| the limit raised by `--restructure-limit-rows 5000000 --restructure-limit-bytes 4GB`, a real run, **no backup option** | passes the gate and is refused by the backup rule: `backup_required` (Russian text naming `--recovery-backup` and `--i-have-a-backup`) |
| the limit raised by `IBCMD_RS_RESTRUCTURE_LIMIT_ROWS=5_000_000` and `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES="4 GiB"`, dry run | passes; the report says `limit rows 5000000 (IBCMD_RS_RESTRUCTURE_LIMIT_ROWS)`, `bytes 4294967296 (IBCMD_RS_RESTRUCTURE_LIMIT_BYTES)`, `within_limit: true` |
| the same environment and `--restructure-limit-bytes 1GB` | refused again, "(--restructure-limit-bytes)": the flag beats the environment |
| `ibcmd-rs.toml` in the current directory with both keys, dry run | passes; sources `...\ibcmd-rs.toml:1` and `:2`; `settings show` prints the two lines |

**Nothing written.** `s1j.py fingerprint` (row counts, sizes and checksums of `ConfigSave`, `Config`, `Params`, `Files`, the
`SchemaStorage` and `DBSchema` and `DBNames` hashes, the table count, the newest table modification, `_Reference20`'s rows
and columns) is byte-equal before and after the three refused runs that could have written (the real run with a backup
option, the trial, the raised limit without one); the dry runs write nothing by design.

**Below the limit, against the native apply** (a pair of twins from one backup: 500 061 rows in `_Reference20`, 700 MB
to write, 34% of the limit): native `config apply` on one (143 s), `mssql-config-apply --allow-restructure s1
--i-have-a-backup` on the other (50.7 s, `size_check.within_limit: true`, 11 tables, 500 812 rows). The checks of
12.6 that apply:

| check | result |
|---|---|
| 3 the data of the rebuilt tables | `EXCEPT` both ways **0 rows** in `_Reference20` (500 014 rows), its two sub-tables, `_Reference2598`, `_Document39`, `_Document39_VT970` |
| 4 `Config` | 9 841 rows on both, **0 rows on either side** (`Creation` and `Modified` included) |
| 7 native `config apply` on our twin | «Обновление конфигурации базы данных не требуется», exit 0 |
| 8 native `config export` of both, `compare_trees_fast.py` | 12 198 of 12 198 files identical (`configVersion` blanked in `ConfigDumpInfo.xml`, as in the ddl protocol) |

## 7. Open, and what is not done

- The chunked copy is 0.5 (this issue is the refusal).
- The guard does not adapt to the recovery model: a SIMPLE base writes 0.5% of the log and could rebuild far more; it
  raises the limit today. Reading `sys.databases.recovery_model_desc` is a small later change if wanted.
- The error 1205 of section 4.2 (7): seen once, not reproduced, not investigated.
- `mssql-restructure --trial` needs `--skip-session-check` (the tool's own pool holds a second session on the database and
  its check counts it).

## 8. Reproduction

```
python scripts/restructure-lab/size-limit/s1j.py presize  --db ibcmd_rs_04_ui_s1j_x --data-gb 6 --log-gb 10
python scripts/restructure-lab/size-limit/s1j.py rung     --db ibcmd_rs_04_ui_s1j_x --shape narrow --rows 1000000 --exe <ibcmd-rs.exe> --tag n1e6
python scripts/restructure-lab/size-limit/s1j.py probe    --db ibcmd_rs_04_ui_s1j_bsp --table _InfoRg5222 --tag probe1
python scripts/restructure-lab/size-limit/s1j.py fingerprint --db ibcmd_rs_04_ui_s1j_u --out before.json
python scripts/restructure-lab/size-limit/table.py [--csv file | --markdown]
pwsh scripts/restructure-lab/size-limit/native_twin.ps1 apply|export -Database ibcmd_rs_04_ui_s1j_e_nat [-Out dir]
python scripts/restructure-lab/size-limit/compare_twins.py <db A> <db B> --tables _Reference20,...
```

Every run over five minutes was one command inside `heavy-lock.ps1 acquire ui` ... `release ui`; the native commands
under the `native` lock. Twins: the writes only to `ibcmd_rs_04_ui_*` (the kit checks), every twin dropped with
`drop-lab-dbs.ps1` when its runs were done.
