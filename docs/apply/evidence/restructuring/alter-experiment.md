# ALTER TABLE ... ADD instead of the rebuild: does the platform accept it? (checkpoint 2)

Question of the coordinator: may an own apply add the column of a new attribute with `ALTER TABLE ... ADD` (cheap for a big table) instead of
the NG rebuild of the object's tables? The column then lands at the **end** of the physical table, after the separator column `_Fld2683`, and
the platform lists its fields in the `DBSchema` order (`..., _Fld11034, _Fld2683`).

## Setup

- Twin `ibcmd_rs_04_ddl_a2_own2`: a copy of our applied twin of case a2 (bak `..._a2_own_after.bak`), which has the first new attribute
  `ДемоНовыйРеквизит` = `_Fld11034` (a rebuild, column before `_Fld2683`).
- A second attribute `ДемоВторойРеквизит` String(20) was staged with the native `import files --partial` (4 rows;
  `scripts/restructure-lab/edit_second.py` makes the edit).
- `ibcmd-rs mssql-restructure --database ibcmd_rs_04_ddl_a2_own2 --alter-add --skip-xdto` (the model cache row is not touched by this run).

## The run (report: the lab's work/alter_apply_report.json, 311 ms in all, one transaction)

```
ALTER TABLE dbo._Reference20 ADD _Fld11035 nvarchar(20) NULL;
UPDATE dbo._Reference20 SET _Fld11035 = N'' WHERE _Folder = 0x01;
-- then the publication as in the rebuild: SchemaStorage, DBSchema, Params DBNames / DBNamesVersion, Config promotion, ConfigSave emptied
```

Read-back of the run: "`_Reference20`: columns as the model (set), physical order differs from the model: the new column is last".

## What a session does with such a table (stand-alone server; sessions-a2.md)

Job `alter_check.bsl`: the metadata knows the attribute; a new item with both attributes is written and read back; a query with
`ГДЕ ДемоВторойРеквизит = &Значение` finds it; an existing item is changed and written; XDTO serialization carries the attribute (the
model cache row had been deleted, so the platform rebuilt the model in memory).

## What the platform's own restructure does with such a table (native apply, traced: xe/alter_native)

A third attribute (`ДемоТретийРеквизит`, String(30), `_Fld11036`) was staged with the native `import files --partial` (4 rows) and the native
`config apply` ran on the same twin: exit 0, 308 s in a loaded lab, "Объект изменен: Справочник._ДемоПартнеры / Изменена структура таблиц
базы данных / Создано поколение конфигурации".

- It **rebuilt** the catalog's three tables with the NG protocol (`alter-native-structure-statements.sql` has the statements):

```
create table dbo._Reference20NG (
... _Fld6357RRef binary(16),
_Fld11034 nvarchar(50),
_Fld11035 nvarchar(20),      <- the column that our ALTER put last: now before the separator
_Fld11036 nvarchar(30),
_Fld2683 numeric(7, 0) not null
)
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, ..., _Fld11034, _Fld11035, _Fld11036, _Fld2683) SELECT
T1._IDRRef, ..., T1._Fld11034, T1._Fld11035, CAST(CASE WHEN T1._Folder = 0x01 THEN @P1 END AS NVARCHAR(30)), T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);
```

  The copy reads the ALTER-added column **by name** and the new table is in the platform's order: **the next native rebuild normalizes the
  order**. Afterwards `sys.columns` of `_Reference20` = `..., _Fld6357RRef, _Fld11034, _Fld11035, _Fld11036, _Fld2683`; the data survived:
  the folder NULL in all three, 12 items with `''`, the item changed in the session with `'изменено'` in the second attribute, the item
  written by the check with `'первый'` and `'второй'`, the third attribute `''` in all items.
- The same apply also rebuilt `_DbCopies`, `_DbCopiesUpdates` (the build drift of the source backup), `_ConfigChngR` and
  `_ConfigChngR_ExtProps` (its own change registration), as in case a2.
- It **recomputed the derived caches from the metadata, not from the stale rows**: the XDTO model row now carries all three properties and
  the object registry `1a621f0f-....si` got three entries (counter 10807 -> 10810), including the two attributes that our runs had never
  added (`out`: si_diff of `a2_own/own_after` -> `a2_own2/alter_native_after`). `c77bc206-....si` was rewritten to the same text as after the
  native apply of case a2. A stale cache left by an own apply therefore heals at the next native apply that touches the object.

## Verdict

- **Sessions accept the ALTER-added column**: reads, writes, queries by the column and XDTO serialization work; the platform addresses columns
  by name.
- **The native restructure accepts it too** and puts the column in its place while rebuilding.
- Not run: a native `config apply` with nothing staged directly after the ALTER (the stored `DBSchema` is the same text a rebuild would have
  written, and the earlier no-change / re-stage applies on rebuilt twins prove that the platform decides from `DBSchema`, not from
  `sys.columns`, so the outcome is expected to be "не требуется" -- an inference, not a measurement); an ALTER on tables of registers
  (aggregates and totals may read positionally), extensions, the online path.
- Costs: the physical order differs from the platform's, so a byte comparison of the table structures with a native twin stops working
  (it is how the prototype proves itself); rows grow in place (page splits) where a rebuild packs them. **The prototype keeps the rebuild as
  the default and `--alter-add` as a research switch.**
