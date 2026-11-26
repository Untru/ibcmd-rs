# The drop-in `config apply` serves S1 on the ERP УХ 8.3.27: acceptance on twins (rcheck suha, on #391)

The acceptance of `s1-acceptance.md` (cases `b1`, `c1`, `d1` on the БСП 8.3.27) and `s1-acceptance-85.md` (the БСП 8.5) repeated
on the **ERP УХ 8.3.27**: two cases, `b1` (attributes deleted, one added) and `c1` (variable strings widened). Corpus
`uha_parity2_20260924.bak` (the УХ configuration, 118 377 `Config` rows, 16 857 tables in `DBSchema`, 21 187 tables in the
database). The drop-in is `ibcmd-rs infobase config apply --platform=8.3.27`; the platform is 8.3.27.2214; the УХ lab databases have
no users, so there is no `--user`. Every native command ran under the lab's `heavy` lock and then its `native` lock, the drop-in
apply under `heavy`, one УХ pair at a time.

**What this proves and what it does not.** The УХ backup holds the configuration only: the tables of the database are empty
(every rebuilt table has 0 rows on both twins, and so has the change register). The run therefore proves the structure path on
УХ's metadata scale (thousands of descriptors to read and compare, 16 857 tables in the schema, the caches of a big
configuration), and the equality of everything the apply writes but rows of user data. It does not prove the copy of rows of the
rebuilt tables, the log and the time that a rebuild takes on a table with millions of rows, or the size limit (S1-J) at scale.
Checks 3 and 9' (below) compare empty tables and say so.

## The protocol

That of `s1-acceptance.md`: a clone staged with the platform's `import files --partial`, a `BACKUP ... COPY_ONLY` of that staged
state restored as the own twin (both start from the same `ConfigSave`), the platform's `config apply --dynamic=disable` on the
native twin, on the own twin the drop-in first without a backup option, then with `--recovery-backup=<file>`. Checks against the
native twin: 2 tables, columns and indexes (`snapshot.py` + `snapdiff.py`); 3 the data of every rebuilt table (`EXCEPT` both ways);
4 every `Config` row, `Creation` and `Modified` included; 5 `DBSchema` entries and `DBNames` text; 6 the 16 `*.si` rows; 7 the
platform's `config apply` on our twin; 8 the platform's `config export` of both twins with `ibcmd-rs source-diff` (one export per
twin); 9' `_ConfigChngR` and `_ConfigChngR_ExtProps` without their key column, which the native apply rebuilds with new keys.

The edits are the shapes of `edit_cases_s2.py` (`b1`, `c1`) on the УХ reference export, on objects of the УХ (none is referenced
elsewhere in the tree, no catalog has predefined items, no extension adopts anything; `edit_cases_uha.py` in the lab folder).

## Result

| case | the stage | rows staged | native apply | drop-in apply (`--recovery-backup`) | rebuilt tables | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9' |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `b1` | 2 flat catalogs, 2 hierarchical ones (one with a tabular section) and 2 documents: 8 attributes removed (strings, booleans, an unlimited string), 1 string added | 10 (with `deleted`) | 394.9 s | exit 0, 381.9 s (debug build), backup 6.8 s | 10 (6 objects) | equal | 0 rows, equal | 0 rows differ | entries equal, `DBNames` equal, order differs | 16 identical | `не требуется` | 140 709 of 140 709 identical | 0 rows |
| `c1` | 5 catalogs (one hierarchical, with an indexed string) and 2 documents: 7 strings widened (`20->60`, `100->300`, `256->400`, `256->512`, `255->300` indexed, `100->200`, `50->100`) | 10 | 178.9 s | exit 0, 148.9 s (release build), backup 11.2 s | 13 (7 objects) | equal | 0 rows, equal | 0 rows differ | entries equal, `DBNames` equal, order differs | 16 identical | `не требуется` | 140 709 of 140 709 identical | 0 rows |

Without a backup option (each case, before the real run): exit 1, stderr `[ERROR] Применение меняет структуру таблиц базы
данных: ... Нужна резервная копия SQL Server. Укажите `--recovery-backup <путь>` ...`; the fingerprint of the database is equal
before and after (`database untouched: True`).

### Timings, ours against the native apply

| | native | ours | of that: gate (reading and comparing 118 377 rows' descriptors, the S1 plan) | the transaction (`sql`) | the backup |
|---|---|---|---|---|---|
| `b1` (debug build) | 394.9 s | 380.3 s | 326.7 s | 29.4 s | 6.8 s |
| `c1` (release build) | 178.9 s | 146.2 s | 56.4 s (+ 58.5 s reading the inventory) | 14.1 s | 11.2 s |

On a stage of ten rows the time is about the platform's (4 % and 18 % less), unlike the apply without a restructuring (300 modules in
34.8 s against 639.8 s), and the numbers are orders of magnitude: the machine was busy with the other tracks (the inventory read, 2.8 s in the `b1` run,
took 58.5 s in the `c1` run), and the two builds differ. What an S1 apply of a stage of ten rows costs on the УХ is the gate: the
restructure check reads the descriptors of the active `Config` and the S1 plan reads the schema; the rebuild itself is 14-29 s
on the empty tables. On a table with data the platform's own rebuild (the `..NG` copy) and this apply's (one transaction, the size guard
S1-J) are the ones to measure; not done here.

Around them: the native `import files --partial` of a stage took 107.2 s (`b1`) and 99.7 s (`c1`); the platform's apply on our twin
(check 7) 33.9 s and 134.6 s; one native export of the УХ 738-1 175 s (140 709 files; 862-1 879 s with the wait for the locks); the
snapshots of a twin (21 187 tables) 8-20 minutes each.

## What differs: the order of the tables in `DBSchema`

Check 5 finds every `DBSchema` entry equal (16 857 of 16 857) and `DBNames` equal, but the **order** of the table list differs. The
platform's apply takes each rebuilt table out of its place and appends it at the **end** of the list, in the order it rebuilt
them (`b1`: positions 16 851-16 856 of 16 857, after `ExtensionsInfoNGS`). Ours puts them before `ConfigChngR`
(`plan.rs`: `insert_before("ConfigChngR", ...)`, the rule of the БСП, where `ConfigChngR` is the last table: the platform
rebuilds it in the same apply and it stays last). On the УХ `ConfigChngR` sits at position 4 809 (4 803 in the platform's list) and the platform does not
rebuild it (presumably because the register has no rows), so it stays where it is
and the rebuilt tables go to the end; ours go to positions 4 803-4 808. Without the named tables the two orders are equal (`schema_order.py`).

It changes nothing that was checked: the tables, columns and indexes are equal (check 2), the platform's own apply on our twin
says «не требуется» (check 7), and its export is equal (check 8). It is a difference from what the platform writes, and the
twin comparison of `restructuring.md` 12.6 (check 5) is written to see it. The rule that reproduces both cases is "the rebuilt tables
at the end of the list, ahead of `ConfigChngR` only when it is the last table (the platform then rebuilds it after them)".

## The other differences of the `Params` rows (`snapdiff.py`)

Two `.ui` rows exist on the native twin only, `ibparams.inf`, `locale.inf`, `MobileVersions.dat`, `siVersions`, `DBNamesVersion`,
the XDTO model row and `gc.mrk` differ in the platform's own bookkeeping: the drift list of `restructuring.md` 12.6 and
`own-apply.md` ("Which per-apply writes are required"). The 16 `*.si` rows come back identical after both applies (on 8.5 they are
the same up to order).

## What stays refused

A subordinate catalog is refused, in the first attempt of `c1` (which widened `ГрафыБухОтчетности.КодГрафы`): exit 1 in 40.8 s
(release build), `требуется штатный config apply: <the descriptors that differ>; S1: catalog ГрафыБухОтчетности is subordinate
to owners: its owner field is not covered`, the fingerprint equal. It is the typed refusal of the plan (`restructuring.md`, 12), working on
УХ's catalogs; the case was staged again without that catalog.

## Reproduce

`F:\ibcmd\lab\04\restructure-check\suha\`: `census_uha.py`, `selection.py`, `edit_cases_uha.py`, `make_stages.ps1` (edits),
`prepare_case_uha.ps1 -Case b1|c1 -ApplyNative`, `own_case_uha.ps1 -Case ... -Exe <ibcmd-rs.exe> -Snapshots`, `run_case_uha.ps1`,
`compare_chng.ps1`, `schema_order.py`, `si_canonical.py`, `logs/`.
