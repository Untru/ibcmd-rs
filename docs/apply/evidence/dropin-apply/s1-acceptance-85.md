# The drop-in `config apply` serves S1 on the 8.5 БСП: acceptance on twins (rcheck s185, on #391)

The acceptance of `s1-acceptance.md` (cases `b1`, `c1`, `d1` on the БСП 8.3.27) repeated on the БСП **8.5.1.1150** (XML
2.21, corpus `bsp85_src_20260929.bak`, 9 948 `Config` rows, 1 GB). Debug build of `feat/0.4-s1-85` (feat/0.4 at 276d0529
plus the two changes at the end of this file). The drop-in is `ibcmd-rs infobase config apply --platform=8.5.1`; the
platform is 8.5.1.1150 with `--user="Администратор (обычное приложение)"` (plain `Администратор` asks for a password on
the 8.5 corpus); every native write under the lab's `native` lock.

## The protocol

That of `s1-acceptance.md`: one clone staged with the platform's `import files --partial`, a `BACKUP ... COPY_ONLY` of that
staged state restored as the own twin, so that both start from the same `ConfigSave`; the platform's `config apply
--dynamic=disable` on the native twin; on the own twin the drop-in, first without a backup option, then with
`--recovery-backup=<file>`. Checks against the native twin:

| # | check | how |
|---|---|---|
| 3 | the data of every rebuilt table | `compare_tables.ps1`, `EXCEPT` both ways |
| 4 | every `Config` row, `Creation` and `Modified` included | `compare_config.ps1`, both ways |
| 5 | `DBSchema` entries and order, `DBNames` text | `snapshot.py` + `dbschema_cmp.py` |
| 6 | the 16 `*.si` rows | `snapshot.py` + `si_diff.py`, and `si_canonical.py` (below) |
| 7 | the platform's `config apply` on our twin | says «Обновление конфигурации базы данных не требуется» |
| 8 | the platform's `config export` of both twins | `ibcmd-rs source-diff` |

The edits are those of the ddl track's `edit_cases_s2.py` (`b1`, `c1`, `d1`), run on the 8.5 reference export. The 8.5 БСП
is a later БСП than the 8.3.27 one, so three things are named or built differently, and the kit of this run
(`edit_cases_85.py`, in the lab folder `restructure-check/s185`) takes the same shapes from other objects:

* the catalogs `ПоставщикиУслугСервиса` and `ШаблоныЗаданийОчереди` carry the prefix `Удалить` (`УдалитьПоставщикиУслугСервиса`,
  `УдалитьШаблоныЗаданийОчереди`; same shapes: one indexed `String(50)`, seven attributes with `ИмяМетода` 255 and `Ключ`);
* `КлассификаторБанков.КоррСчет` is a data lock field of its catalog in the 8.5 БСП, and the platform's metadata check
  refuses a stage that deletes it («Указано неверное поле в полях блокировки данных»): `b1` deletes `ДеятельностьПрекращена`
  (an indexed boolean) as the first attribute of that catalog instead;
* `КлассификаторБанков.Телефоны` is an unlimited string there, which cannot be indexed («При установленном типе недопустимо
  индексирование»): the additional-order switch of `d1` takes `СВИФТБИК` (`String(11)`).

Both refusals are the platform's own, at the native apply of the stage (exit 1 after 6 s, nothing written); they are not
refusals of the drop-in.

## Result

| case | the stage | rows staged | drop-in apply (`--recovery-backup`) | rebuilt tables | check 3 | check 4 | check 5 | check 6 | check 7 | check 8 |
|---|---|---|---|---|---|---|---|---|---|---|
| `b1` attributes deleted (and two added) | 5 catalogs and a document: 11 attributes removed, 2 added | 10 (with `deleted`) | exit 0, 39.6 s, gate `s1`, backup `file` (3.9 s) | 8 (6 objects) | all equal | 0 rows differ | 1 769 tables equal, `DBNames` equal | 2 identical, 14 the same up to order | `не требуется` | 12 337 of 12 337 identical |
| `c1` variable strings widened | 3 catalogs and 2 documents: 6 strings, `50 -> 200`, `12 -> 20`, `40 -> 100`, `255 -> 1024`, `50 -> 51`, `100 -> 300` | 8 | exit 0, 44.4 s, backup 2.4 s | 10 (5 objects) | all equal | 0 | equal | 2 + 14 | `не требуется` | 12 337 of 12 337 |
| `d1` index switched | 4 catalogs and 2 documents: 12 attributes, `DontIndex <-> Index`, `DontIndex <-> IndexWithAdditionalOrder`, `IndexWithAdditionalOrder -> DontIndex` | 9 | exit 0, 47.5 s, backup 3.3 s | 11 (6 objects) | all equal | 0 | equal | 2 + 14 | `не требуется` | 12 337 of 12 337 |

The platform's own apply of the same stages took 155.8 s (`b1`), 116.6 s (`c1`) and 60.4 s (`d1`): the machine was busy with
other tracks, so these are orders of magnitude, not measurements.

**Without a backup option** (each case, before the real run): exit 1, stderr `[ERROR] Применение меняет структуру таблиц
базы данных: ... Нужна резервная копия SQL Server. Укажите `--recovery-backup <путь>` ... или `--i-have-a-backup` ...`; the
fingerprint of the database (`Config`, `ConfigSave`, `Params`, `SchemaStorage`, the tables and columns) is equal before and
after (`database untouched: True`).

## Check 6: the `*.si` rows are the same up to order

The 8.5 native apply rewrites all sixteen `*.si` rows with their records in another order (`own-apply.md`, ".si rows on
8.5"); on 8.3.27 it rewrites three (`restructuring.md`, 9.7). After a native and an own apply of the same stage, 2 rows are identical and 14 differ
as text (`si_diff.py`). `si_canonical.py` reduces every differing row to a form that ignores the order of items at every
level (brace lists: sorted children; the 29 MB XDTO-model row `ea13a2c9`: the decoded XML with sorted children) and compares:
**14 of 14 are the same up to order, in each of the three cases** (`logs/*_check6_canonical.txt`). Nothing is added, dropped or
changed inside an item; only the order the platform gives them differs. Check 7 (the platform's own apply on our twin says
«не требуется») and check 8 (the platform's export equal) hold. A cluster session on both twins (the ddl track's check 9) was not
run.

## What it took: the `root` row of 8.5

The first run on 8.5 was refused: `требуется штатный config apply: root: the service row root changes`, with `S1:
service-row-changed: Configuration: the root row differs: another configuration`. Nothing in the stage changes the
configuration; the platform's own import writes the row differently. The `root` row is `{2,<configuration uuid>,<payload>}`:

* on 8.3.27 the payload is empty, and a native import writes the row as it is;
* on 8.5.1.1150 the payload is 128 bytes (base64 in wrapped lines) encrypted in chained blocks of 16, and the platform
  **re-stamps the final block on every write**: a fresh clone, one unchanged descriptor staged by `import files --partial`, and the
  staged `root` differs from the stored one in bytes 112-127 only; after a native apply and a second import of the same
  file it differs in the last block again (probe `root_probe.ps1`). A chained block depends on the plaintext up to itself, so a
  change of the row's substance would show in the uuid or in the seven blocks before the stamp.

Two changes, each with unit tests:

* `apply_check::root_row`: the check compares the `root` row by that structure. The same bytes, or the same format tag, uuid and
  length with every payload block but the last equal, is `same` or `re-stamped` (a note in the verdict, no reason); anything
  else, a shorter or longer payload, another uuid, a change in an earlier block, a row that cannot be read, is the
  `root-row-changed` reason as before. On 8.3.27 the payload is empty and nothing changes there.
* `restructure::s1::decide`: the conservative rule compares the bytes of `root` and left its blocker standing after the plan
  answered for everything else. The blocker is now the check's to answer: a real change of `root` is a
  `service-row-changed` refusal of the classification (the plan is not made), so a `root` blocker that survives to that point
  has only the stamp behind it.

Finding 10 of `restructuring-check.md` lists the `root` row among the rows that the import track's 8.5 base-free stage of an
unchanged БСП changes. If the row that stage writes differs from the stored one in the last block only, the check no longer
refuses it for that (not measured for that writer here).

## Reproduce

`F:\ibcmd\lab\04\restructure-check\s185\`: `make_stages.ps1` (edits), `prepare_case85.ps1 -Case b1|c1|d1 -ApplyNative` (twins and the
native apply), `own_case85.ps1 -Case ... -Exe <ibcmd-rs.exe> -Snapshots` (the drop-in and the checks), `run_case85.ps1` (all of it
and the clean-up), `si_canonical.py`, `root_probe.ps1`, `logs/`.
