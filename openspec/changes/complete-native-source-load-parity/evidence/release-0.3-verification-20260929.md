# 0.3 release verification (2026-09-28 .. 29)

The final check of milestone 0.3 ("Прямая замена ibcmd.exe") on the merged
integration branch `feat/0.3`: the drop-in command line, the platform
settings and the built-in SQL Server client together, against the four
reference trees.

- **Binary.** Release build of `feat/0.3` at fa4fd841 (the merge of the SQL
  track; the later commits touch only docs, scripts and the policy baseline),
  copied as `F:\ibcmd\lab\03\verify\bin\ibcmd.exe`.
- **No SQL tools.** Every run of ours had no folder of `sqlcmd.exe` or
  `bcp.exe` on PATH (`dropin_check.sh` with `NO_SQL_TOOLS=1`,
  `scripts/empty-load/run_dropin_import.ps1 -NoSqlTools`).
- **No version flag.** The platform came from an `ibcmd-rs.toml` binding the
  lab databases (`ibcmd_rs_{bsp,uha}_8327*` → 8.3.27.2214,
  `ibcmd_rs_{bsp,uha}_85*` → 8.5.1.1150) placed in the run folder.
- **Reads and writes.** The exports only read the lab source databases. The
  loads wrote new disposable databases `ibcmd_rs_03_*_import_20260929_f1`
  (files in `F:\ibcmd\lab\03\sqldata`), as Pavel allowed on 2026-09-28.

## Export: `ibcmd infobase config export`

```
ibcmd infobase config export --dbms=MSSQLServer --db-server=localhost --db-name=<db> <folder>
```

| Corpus | Database | Time | XML | Files identical to the native export |
|---|---|---|---|---|
| БСП 8.3.27 | `ibcmd_rs_bsp_8327_native_20260919` | 9 s | 2.20 | 12 198 / 12 198 |
| БСП 8.5 | `ibcmd_rs_bsp_85_src_20260922` | 10 s | 2.21 | 12 337 / 12 337 |
| ERP УХ 8.3.27 | `ibcmd_rs_uha_8327_parity2_20260920` | 163 s | 2.20 | 140 709 / 140 709 |
| ERP УХ 8.5 | `ibcmd_rs_uha_85_src_20260922` | 142 s | 2.21 | 140 709 / 140 709 |

Every run printed exactly the platform's two lines on stdout
(`[INFO] Экспорт конфигурации в XML...`,
`[INFO] Экспорт конфигурации в XML успешно завершен`), nothing on stderr, and
exited 0. The native export of ERP УХ takes 334-404 s on this machine
(0.2 measurements).

## Load: `ibcmd infobase config import`, then native apply and export

`run_dropin_import.ps1 -Tag f1 -NoSqlTools`: native `infobase create` of an
empty infobase, our import (the drop-in syntax picks the stage mode itself),
native `config apply --force --dynamic=disable`, native `config export`,
`source-diff` against the reference tree.

| Corpus | Our import | Native apply | Files identical | `ConfigDumpInfo.xml`, configVersion blanked |
|---|---|---|---|---|
| БСП 8.3.27, empty base | base-free, 9 838 rows, 49 s | SDBL "быстрой вставки" on the first run (the known 8.3.27 platform quirk), "не требуется" on the second | 12 197 / 12 198 | identical |
| БСП 8.3.27, again into the loaded base | patch, 9 517 rows, 7 s | 41 s | 12 197 / 12 198 | identical |
| БСП 8.5, empty base | base-free, 9 935 rows, 15 s | 90 s | 12 336 / 12 337 | identical |
| ERP УХ 8.3.27, empty base | base-free, 118 170 rows, 281 s | 922 s | 140 708 / 140 709 | identical, 249 143 entries |
| ERP УХ 8.5, empty base | base-free, 118 170 rows, 271 s | 1 220 s | 140 708 / 140 709 | identical, 249 143 entries |

The one differing file in every run is `ConfigDumpInfo.xml`, by its
`configVersion` values only: the 145 content-free stub rows of ERP УХ (#334,
`f44ac6b0`) are there, and the ERP УХ load now matches on that file as well.
The three loads ran at the same time.

The native apply of ERP УХ took 15-20 min here and about 4 h on 2026-09-27.
The platform's apply log is the same both times: 10 829 lines, differing
only in the order of the "Новый объект" lines. The machine was the
difference: on 2026-09-27 five track agents built and tested beside the run,
and the native export then took 22 min against 5 min now. So the shorter
apply is not ours.

## Output and exit codes against the platform (8.3.27.2214)

- **Successful import.** Measured on a throwaway file infobase: the platform
  prints `[INFO] Импорт конфигурации из XML...` and
  `[INFO] Импорт конфигурации из XML успешно завершен`, as ours does.
- **Failed import.** The platform's XML import of the БСП tree fails ("Ссылка
  на неизвестный предопределенный элемент"): `[INFO] …` on stdout, `[ERROR]`
  lines on stderr ending in `… завершен с ошибкой`, exit -1. Ours uses the
  same frame and code.
- **Exit codes:** 0 success; 2 for a malformed or incomplete command line or an
  unknown DBMS; -1 for a failed operation. Ours are the same (fab4c9df), plus 1
  for what ibcmd-rs does not serve.

## Offline cycles on the merged code (before the SQL track)

`scripts/empty-load/ve.sh`, run i03a, build 023bb86f: all four corpora
identical except `ConfigDumpInfo.xml`, which matches once configVersion is
blanked.
