# The drop-in `config apply` serves S1: acceptance on БСП 8.3.27 twins (rcheck dn, on #391)

Debug build of `feat/0.4-dropin-s1` (feat/0.4 at 847f581a merged: S1 waves 1, the index flag, the S1 gate on the apply's
seam, the extension refusal). `b1`, `c1` and the controls ran on the build before the last merge (ee651602), `d1` again on the
final build with the same result. The drop-in is `ibcmd-rs infobase config apply` (the program under the platform's command line). Platform
8.3.27.2214, `--user=Администратор`, SQL Server on localhost, every native write under the lab's `native` lock.

## The protocol

The twin protocol of the ddl track (`docs/apply/restructuring.md`, 12.6), with the drop-in as the apply of our twin.

1. A fresh clone of `bsp_native_20260923.bak` (`..._dn_<case>_nat`); the platform's `infobase config import files --partial`
   stages the case (the edited descriptors plus `root`, `version`, `versions`, and a `deleted` row when attributes go).
   The edits are those of the ddl track's `edit_cases_s2.py` (`b1`, `c1`, `d1`), run on the reference tree, which is only read.
2. `BACKUP DATABASE ... COPY_ONLY` of that staged state, restored as the own twin (`..._dn_<case>_own`): both twins start from
   the very same `ConfigSave`.
3. The platform's `infobase config apply --dynamic=disable` on the native twin.
4. On the own twin, the drop-in: first **without** a backup option, then with `--recovery-backup=<file>`.
5. Checks against the native twin: 3 (data of every rebuilt table, `EXCEPT` both ways), 4 (every `Config` row, `Creation`
   and `Modified` included, both ways), 7 (the platform's `config apply` on our twin), 8 (the platform's `config export`
   of both, `ibcmd-rs source-diff`).

## Result

| case | the stage | rows staged | drop-in apply (`--recovery-backup`) | rebuilt tables | check 3 | check 4 | check 7 | check 8 |
|---|---|---|---|---|---|---|---|---|
| `b1` attributes deleted (and one added) | 5 catalogs and a document: 11 attributes removed, 2 added | 10 (with `deleted`) | exit 0, 18.6 s, gate `s1`, backup `file` (282 MB) | 8 (6 objects) | all equal | 0 rows differ | `не требуется` | 12 198 files identical |
| `c1` variable strings widened | 3 catalogs and 2 documents: 6 strings, `50 -> 200`, `12 -> 20`, `40 -> 100`, `255 -> 1024`, `50 -> 51`, `100 -> 300` | 8 | exit 0, 18.1 s | 10 | all equal | 0 | `не требуется` | 12 198 identical |
| `d1` index switched | 4 catalogs and 2 documents: 12 attributes, `DontIndex <-> Index`, `DontIndex <-> IndexWithAdditionalOrder`, `IndexWithAdditionalOrder -> DontIndex` | 9 | exit 0, 21.2 s | 11 | all equal | 0 | `не требуется` | 12 198 identical |

The platform's own apply of the same stages took 34-195 s (it checks the metadata first; the first native apply of a
clone is the slowest).

**Without a backup option** (each case, before the real run): exit 1, stderr `[ERROR] Применение меняет структуру таблиц
базы данных: ... Нужна резервная копия SQL Server. Укажите `--recovery-backup <путь>` ... или `--i-have-a-backup` ...` (the
apply's `BackupRequired`, in Russian, naming both options). The database fingerprint (`Config`, `ConfigSave`, `Params`,
`SchemaStorage`: row counts and checksums of names, sizes and times; the tables and the columns of `sys.tables`) is equal
before and after.

## What goes as before

| run | result |
|---|---|
| a harmless descriptor change (a synonym of a catalog), staged by the platform on a twin pair, drop-in apply with **no** backup option | exit 0, gate `apply-check`, no `structure`, no backup; `Config` rows equal to the native twin (check 4: 0), `не требуется` afterwards |
| three module texts, staged by the drop-in's own `config import` (patch mode, 9 517 rows, 54 s), drop-in apply | exit 0, gate `apply-check`; the platform's apply afterwards `не требуется` |

The first row is the reason for `gate::FirstThen`. With the S1 gate alone (which wraps the conservative rule, refusing every
changed descriptor) the same stage was refused: `[ERROR] требуется штатный config apply: 5422610d-...: the descriptor's text
differs from the active one: a metadata change, possibly structural; S1: the conservative gate refuses descriptors, the
restructuring check names no change in them`, exit 1; the default gate (`mssql-config-apply --dry-run`) passes it.
The composed gate asks the restructure check first and hands to S1 only what the check refuses.

## What stays refused (exit 1, nothing written, fingerprint equal), with `--i-have-a-backup`

| stage | the drop-in says |
|---|---|
| an attribute added (S1) and `CodeLength` 9 -> 12 (outside S1) in one stage | `требуется штатный config apply: 5eab8a1b-...: the descriptor's text differs ...; S1: property-outside-s1: Catalog._ДемоПартнеры: Properties/CodeLength: 9 -> 12 (a property no rule covers) [Properties/CodeLength]` |
| a new tabular section (the ddl track's case h4) | `требуется штатный config apply: 28e59c50-...: ...; 28e59c50-...: S1: Catalog._ДемоКонтрагенты: the S1 operation "add-tabular-section" is designed (docs/apply/restructuring.md, 12.3) but not built in this version` |
| a backup file that exists already (`--recovery-backup`) | exit -1 before anything is written: `the backup file ... exists already: the apply does not overwrite a backup; name another file` |

## The program's own import does not stage a restructuring yet

The acceptance stages with the platform's import. Asked to do it with `ibcmd infobase config import` (ours) on the same
edits, on a fresh clone:

* **patch mode** (the default): `Загрузка ... переносит из дерева имя, синоним и комментарий объектов, модули, формы,
  макеты, права ролей и командные интерфейсы; реквизиты, табличные части, ... в базе остаются прежними`. It refuses the edited
  descriptor (`--verify`: `MetaDataObject/Catalog/ChildObjects/Attribute[3]/@uuid` differs), and stages nothing.
* **`--base-free`** with the `b1` edits: `a base-free stage needs every row, and 4 could not be produced`: the four forms
  bind `Объект.<deleted attribute>` (`АбонентКакСвязаться`, `КоррСчет`, `Ставка`, `ЦелевоеВремя`), which the writer cannot
  place (the native import accepts a form that names a removed attribute; a user's tree would have the field removed too).
* **`--base-free`** with the `c1` edits (22 s, 9 838 rows): the stage is complete and the apply refuses it with `--i-have-a-backup`:
  `root: the service row root changes; <five descriptors>: the descriptor's text differs ...; dad11c2e-....7: a BusinessProcess
  body with suffix .7 that the source-asset registry does not name; S1: data-change: BusinessProcess.Задание: Flowchart:
  content changed (8805 -> 8805 bytes)`. The business process flowchart row (a counter the compiler cannot reproduce,
  restructuring-check.md, finding 5) is a `data` change and the `root` row is rewritten: S1 takes neither.

So the chain "our import, then the drop-in apply" of a descriptor edit waits for the import track; the apply is proven on the
platform's stage, which is the state a stage of the platform's import is in.
