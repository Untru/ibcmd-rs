# Restructuring on `config apply`: what native does, and what it takes to do it ourselves

Issue [#341](https://github.com/Untru/ibcmd-rs/issues/341), track "ddl", 0.4 "Своё применение конфигурации".
2026-09-29. Checkpoint 1 (sections 1-8, 10-11): traces, formats, mapping. Checkpoint 2 (section 9): the
prototype for the simplest case -- a new attribute in a catalog -- in `src/restructure/`, run on a twin of
the native apply and compared with it. Issue [#391](https://github.com/Untru/ibcmd-rs/issues/391) (the minimal own
restructuring, S1), checkpoint 1 (section 12): how the own apply hands the structural changes to the own restructure
inside its transaction, which caches to write, the split into sub-issues, and the first step -- attributes of every
primitive type in catalogs and documents, equal to native down to the export.

Scope of the measurements: platform 8.3.27.2214, Microsoft SQL Server 2025, exclusive apply
(`ibcmd infobase config apply --force --dynamic=disable --user=Администратор`), the БСП demo configuration
(1761 main tables, 4 extensions) restored from the lab corpus. Case a was traced once on the 8.5.1.1150
БСП as well (section 9.7); the other cases only on 8.3.27. Everything below is **measured** unless it says
**hypothesis**. Evidence files are listed in section 11; the scripts that produced them are in
`scripts/restructure-lab/`.

## 1. Summary

1. **Native apply never uses `ALTER TABLE`.** Whenever the structure entry of a table changes (a column
   added, widened or removed, an index added or removed, a sub-table added), the platform rebuilds the
   whole object through "new generation" tables: it creates `_XNG` for the table and each of its
   sub-tables, loads the data with `INSERT ... SELECT` (defaults for new columns; a type change is converted
   row by row inside the 1C engine and bulk-inserted), drops the old tables and renames the new ones. New
   objects take the same path (create `NG`, rename). Progress is recorded in
   `SchemaStorage`: `Status` 100 -> 200 -> 400 -> 500 -> 100, and `NewGenCreated` holds the definitions of the
   tables created so far (section 3).
2. **The structure is a text, and it is decoded.** `DBSchema` (== `SchemaStorage.CurrentSchema` of
   `SchemaID = 0`) is 1C brace text; `Params.DBNames` (raw deflate of brace text) numbers the objects. A
   byte-exact writer and a model "DBSchema entry -> `CREATE TABLE` / `CREATE INDEX`" reproduce **all 1937
   tables of the 8.3.27 БСП and all 1922 tables of the 8.5 БСП** (columns, types, nullability, indexes,
   implicit primary keys); no mismatch is left (section 4).
3. **Metadata -> fields is regular.** For 94 of 94 catalogs and documents (owner-subordinate catalogs and
   attributes of defined types excluded: 16 + 29 objects) the field list of the main table is reproduced
   exactly from the XML tree plus `DBNames` (section 5). The rules that had to be found are few: the
   standard fields per object kind, the type table, the nullable rule, common-attribute fields.
4. **Adding an attribute needs four things**: a new number (global counter over the main *and all
   extension* `DBNames`), a new field entry in the right place, the rebuild of the object's tables
   with a type-dependent default for the existing rows, and the promotion of the staged `Config` rows
   (section 6).
5. **Estimate** (section 8): the "add/remove attribute or tabular section, add a plain catalog or document,
   index flag" subset is a matter of weeks; registers with indexes and totals, common attributes,
   subordination, predefined data, type changes with data conversion, extensions and exchange plans make
   the full job a matter of months. A **round-trip gate** (the generator must reproduce the *old* schema of
   an object family from the *old* metadata before it may change it) turns coverage into an automatic
   safety check.
6. **Findings other tracks need** (section 10): our import in patch mode silently ignores added
   attributes; native `config import` failed twice under load with an unknown-predefined-element error;
   a native apply interrupted at "Принятие изменений" left a database that refuses every `ibcmd config`
   command including `config repair`.
7. **The prototype works, in one transaction** (section 9). `ibcmd-rs mssql-restructure` plans and runs
   "new attributes in one catalog" on a lab database: the platform's new-generation protocol
   (`NG` tables, copy, indexes, drop, rename) plus the publication of `DBSchema`, `SchemaStorage`,
   `DBNames` and the promotion of the staged `Config` rows are **one SQL Server transaction**, read back
   (row counts, columns, indexes) before `COMMIT`; `--trial` runs everything and rolls it back, and the
   database is exactly as it was. On the twin of case a2 the result equals the native apply where it matters:
   the rebuilt tables and their data, `Config` (9841 rows, metadata included), `DBSchema` and `DBNames` (but
   the two system tables the platform upgraded on its own), the native export (12 198 files identical to
   the native twin's), a session (also in a real 1C server cluster) reads and writes the new attribute,
   native `config apply` says "не требуется", and when the same catalog is staged again native computes the
   same structure and runs no DDL at all.
8. **What a restructure must also write into `Params`** (section 9.6): `DBNames` and `DBNamesVersion-DBNames`
   (numbers); the XDTO model cache (one `*.si` row) -- left stale, XDTO serialization of the changed object
   fails ("Свойство ... не обнаружено") while queries and writes work; the prototype inserts the one
   `<property>` line native inserts (text equal to native's). The other two changed `*.si` rows (an object
   registry, a permutation) and `siVersions` are left as they are: no effect in any check, and the next native
   apply that touches the object recomputes them all from the metadata. The `*.ui` rows are the platform's
   licensing records: not touched.
9. **`ALTER TABLE ... ADD` is acceptable to the platform** for everyday work (section 9.5): the column lands
   after the separator column instead of in the schema's place and sessions, queries and writes do not care.
   **The estimate changes** (section 8): the prototype was quick, the classification of a staged image and
   the derived caches are the long poles.

## 2. Method

- **Clones.** `restore-clone.ps1` from the БСП 8.3.27 corpus; one cumulative clone per lineage
  (`ibcmd_rs_04_ddl_bsp8327_a`, `_m`, `_c2`), a COPY_ONLY backup before every apply
  (`lab/04/restructure/bak/*_staged.bak`) is the twin source.
- **Staging.** Changes were made in a copy of the reference native export and staged with **native**
  `ibcmd infobase config import` (our patch-mode import cannot stage structural edits, section 10). When the
  native import was unusable (cases h and c) the tree was staged with **our import in `--base-free` mode**.
- **Trace.** An Extended Events session per apply (`sql_batch_completed`, `rpc_completed`,
  `sql_statement_completed`, `object_created/altered/deleted`), filtered to the lab database, 50-60 thousand
  events per apply. `xe_read.py` turns it into JSONL, `xe_shapes.py` into a story of normalised statements.
- **Snapshots.** Before/after: every table's columns and indexes, row count and `CHECKSUM_AGG(BINARY_CHECKSUM(*))`,
  and the service tables (`DBSchema`, `SchemaStorage`, `Params`, `Config`, `ConfigSave`, `Files`, `ConfigCAS`)
  row by row with content hashes (`snapshot.py`, `snapdiff.py`).
- **Twins (checkpoint 2).** Two databases restored from the same backup of the staged case a2
  (`ibcmd_rs_04_ddl_bsp8327_a_a2_staged.bak`): the native apply runs on one (`a2_nat`), the prototype on the other
  (`a2_own` for its first run, `a2_fin` for the final run of the finished prototype, the one the tables of section 9
  report); everything is compared with the native twin. A second native apply of case a2 on a fresh twin gave
  a byte-identical `DBSchema` and `Config` -- the native result is deterministic but for random guids
  (`DBNamesVersion-DBNames`, `siVersions`, `*.ui`, the PK names of `_ConfigChngR`, `MobileVersions.dat`).
- **Staging of checkpoint 2.** Case a was staged before by a full native import; the follow-up experiments
  use the native **`import files --partial`**, which stages a delta of four rows: the changed descriptor and
  `root`, `version`, `versions` (section 3.4).
- **Sessions.** The 1C server cluster runs as LocalSystem, which has no SQL login (the apply track hit
  error 18456), so a session on a lab database is opened with a stand-alone server (`ibsrv`) started as the
  caller's Windows account (integrated SQL login) and the 8.3.27 thin client over HTTP (`1cv8c /WS`) running
  one generic processing (`probe/ddl_probe.epf`, built with the Designer from XML) that executes a BSL job
  file on the server and writes its result (`scripts/restructure-lab/srv.ps1`, `session_job.ps1`). From 15:33
  the coordinator's `register-ib.ps1` registers a lab database in the local cluster (the cluster connects with the
  SQL login), and the same job runs in a **real cluster session** (`session_job.ps1 -Database <db>`).
- **Cases** (the changed tables hold data: 8-716 rows, the totals table 3424; case c creates an empty table):

| label | change (all in the БСП demo objects) | table(s) that changed | rows |
|---|---|---|---|
| baseline | nothing (staged unchanged tree) | `_ConfigChngR` only | 20685 |
| a2 | String(50) attribute in `Catalog._ДемоПартнеры` | `_Reference20` (+2 sub-tables) | 14 |
| b | Number(15,2) attribute in `Document._ДемоЗаказПокупателя` | `_Document39` (+3) | 8 |
| c | new catalog `ДемоНовыйСправочник` | new `_Reference11036` | 0 |
| d (`dm`) | String(10) dimension in `InformationRegister._ДемоЦеныНоменклатуры`, Number(15,2) resource in `AccumulationRegister._ДемоОстаткиТоваровВМестахХранения` | `_InfoRg7272`, `_AccumRg505`, `_AccumRgT510` | 13, 50, 3424 |
| e + g (`g`) | that attribute widened 50 -> 100 and its index flag switched on (one apply) | `_Reference20` | 14 |
| f | that attribute (indexed) deleted | `_Reference20` | 14 |
| h | 12 attributes of every basic type on a flat catalog; 1 attribute on a catalog with common attributes; 1 attribute in a tabular section; 1 new tabular section | `_Reference2598`, `_Reference27`, `_Reference20_VT159`, new `_Reference15_VT11039` | 716, 71, 18 |
| k | string String(0) -> String(5), Number(10,0) -> Number(5,0), Boolean -> String(10) on rows that hold longer/larger values | `_Reference2598` | 716 (5 seeded) |

Case e was merged into g (two failed native imports; section 10); the string widening and the index
creation are still separable in the trace. Checkpoint 2 added: a85 (case a on the 8.5 БСП, section 9.7), the
identical re-stage of case a on the twin, and a second and a third attribute for the `ALTER TABLE`
experiment (section 9.5). No trace exists for: case e alone, deleting a whole object, type changes of
references, `--dynamic=force`, ERP УХ.

## 3. What native apply does

### 3.1 Timeline (case d, clean run, 113 s under a loaded machine)

`lab/04/restructure/xe/dm`, `docs/apply/evidence/restructuring/timeline-dm.txt`:

| phase | seconds | what |
|---|---|---|
| metadata check | 1.7 - 26 | "Проверка корректности метаданных": thousands of single-row `SELECT ... FROM ConfigSave/Config WHERE FileName = @P1` |
| stage copy | 26 - 60 | for **every** staged row (9839 statements): `INSERT Config SELECT '<name>.new', ... FROM ConfigSave WHERE FileName = '<name>'` |
| structure | 63 - 64 | `create table ...NG`, `CREATE INDEX ...NG`, `drop index`, `INSERT INTO ...NG SELECT`, `insert bulk` |
| switch | 75.5 - 76.6 | `Status 400`, `drop table`, `Status 500`, 28 x `sp_rename`, `Status 100`, `UPDATE DBSchema` |
| promotion | 76.7 - 108 | `Params`: `*.sinew` -> `*.si`; `Config`: 9838 x (`DELETE` old + `UPDATE ... SET FileName` from `.new`); `DELETE FROM ConfigSave` |
| help index | 112.8 - 113 | `Files`: `userDocs_ru`, `userVocabulary_ru`, `userPostings_ru`, `MobileVersions.dat` renamed from `*.new` |

Apply wall times (s): a2 63, b 222 (machine busy), c 167, d 115, g 75, f 96, h 122, baseline 139. The
structure DDL is about **1-2 s of the 75-220**; the rest is row-by-row staging and promotion.

### 3.2 The "new generation" protocol and `SchemaStorage`

`SchemaStorage(SchemaID int, Status int, CurrentSchema, NewGenCreated, NewGenDropped varbinary(max))`;
`SchemaID = 0` is the main configuration, `1` the extensions (their tables carry the `X1` suffix). Sequence
of statements of case a2 (`docs/apply/evidence/restructuring/a2-structure-statements.sql` has them
verbatim):

1. `create table dbo._Reference20NG (...) ;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);`
   for the table and every sub-table, then all their indexes (`CREATE [UNIQUE] [CLUSTERED] INDEX
   <name>NG ON dbo.<table>NG (...) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF,
   ALLOW_ROW_LOCKS = ON) [ON [PRIMARY]]`; the clustered index has no `ON [PRIMARY]`).
2. `UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0` after each group of created
   tables. `NewGenCreated` is a **mini DBSchema** (`{0,{<n>,<table entry>...}}`, same grammar as `DBSchema`,
   the new definitions of the NG tables created so far; 2.6 KB after the first group, 4.3 KB after the fourth;
   see `newgen-created-a2-*.brace.txt`).
3. Indexes are dropped again (`drop index <name>NG on ...`) so the load runs into a heap, the data is copied
   (`INSERT INTO dbo._Reference20NG WITH(TABLOCK) (cols) SELECT ... FROM dbo._Reference20 T1 WITH(NOLOCK)`),
   the indexes are created again.
4. `UPDATE SchemaStorage SET Status = 400`; `drop table` of every old table; `Status = 500`; `sp_rename
   '<t>NG', '<t>', 'object'` for every table, `sp_rename '<t>.<i>NG', '<i>', 'index'` for every index (via
   `sp_prepexec`); `ALTER INDEX <pk> ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)`.
5. `UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE
   SchemaID = 0` with the full new schema and `NewGenCreated = NewGenDropped = EF BB BF 7B 30 2C 0D 0A 7B 30
   7D 0D 0A 7D` (`{0,{0}}`), then `UPDATE DBSchema SET SerializedData = @P1` (the same text).

`NewGenDropped` was never written in these cases (**hypothesis**: it is for the online/high-load path of
milestone 0.5, together with `--dynamic=force`). The statuses make the run resumable; **measured**: an apply
that ended with exit code -1 at "Принятие изменений" (after step 5, before the promotion) left the database
refusing `apply`, `import`, and `config repair --commit|--rollback|--fix-metadata` with "Обнаружена
незавершенная операция сохранения конфигурации" (section 10). An apply that failed earlier was simply
continued by the next apply.

### 3.3 What triggers a rebuild, what is copied

- A table is rebuilt when its **entry in the new DBSchema differs from the old one**. The whole object goes
  with it: main table and all sub-tables (case a2: `_Reference20`, `_Reference20_VT155`, `_Reference20_VT159`
  for one new column in the main table). Companion tables: the change-registration table of an exchange
  plan (`_ReferenceChngR1033`, 10 rows) was **not** rebuilt in case a2; the predefined-data table
  (`_RefSInf<n>`: `PDInitialized` and the separator) was not rebuilt in a2 either (`_RefSInf4877`, no rows)
  but was rebuilt with its catalog in case h (`_RefSInf3901` with `_Reference27`, one row copied). The
  difference is predefined data: 14 of the 75 `RefSInf` tables hold rows, exactly the 14 catalogs that have
  a `Predefined.xml`. (Checkpoint 1 said "companion tables follow"; that was too general.)
  Index-only changes rebuild the table too (case g). New tables use the same NG path without data.
- `_ConfigChngR` and `_ConfigChngR_ExtProps` are rebuilt **on every apply**, even with no change (the
  primary key gets a new auto-generated name each time; the row count stayed 20685 in a2, b, g, f, h and
  grew by 3 in c and d, so the platform also registers something there - not decoded). Their copy goes
  through 1C temp tables and `insert bulk` (the log line "Обработка данных Реструктуризация Таблица
  регистрации изменений конфигурации 1000..20000" is this copy).
- Platform-build drift: the databases of the corpus were last touched by an older build. The first
  structural apply also upgraded system tables (`_DbCopies` +`_StorageVariant`, `_DbCopiesUpdates`
  +3 columns in a2; created `_DbCopiesInfoBaseUse`, `_DbCopiesUpdateStat`, `_DbCopiesUpdateTableStat`,
  `_WebSocketClients` in c). Their names take numbers from the same `DBNames` counter (nil uuid).
- **Data copy is `INSERT ... SELECT` with column mapping and defaults** (the defaults: section 5): unchanged
  columns are copied by name, a widened column is `CAST(T1._Fld11034 AS NVARCHAR(100))`, a new one is a typed
  parameter; `(T1._TrNum + 0.0)` is how the query compiler widens a numeric.
- **A type change is converted by the platform, not by SQL** (case k, `conversion-k.md`): the old table is read
  in pages of 100 rows, the values are converted in the 1C engine and written with `insert bulk` into the NG
  table. Measured results with no warning: string truncated (`abcdefghij` -> `abcde`), number saturated
  (`123456` -> `99999`), boolean to string -> `Да`/`Нет`.
- The log messages of the platform name the actions: "Новый объект: Справочник.X", "Объект изменен:
  РегистрСведений.Y", "Создана таблица: ...", "Изменена структура таблиц базы данных", and for registers
  "Реструктуризация ... пересчет итогов" (totals of the accumulation register are copied, not recalculated:
  `_AccumRgT510` 3424 rows go through `INSERT ... SELECT`).

### 3.4 Promotion of the configuration rows

- The staged rows go from `ConfigSave` to `Config` under `.new` names, and after the structure is switched
  they are renamed (`DELETE FROM config WHERE FileName = '<n>' AND EXISTS(... '<n>.new')`, then
  `UPDATE config SET FileName = '<n>' WHERE FileName = '<n>.new'`); `Params` rows `*.si` go through `*.sinew`,
  `Files` rows through `*.new`/`*.datNEW`. **All staged rows are moved, changed or not** (9839 of them).
- `root`, `version` and `versions` are taken **as staged**: the generation printed by native
  ("Создано поколение конфигурации: 56569e24...") is the header uuid of the staged `versions` row
  (`{1,9834,"",<uuid>,...}`); the row is identical before and after the apply.
- The staged `deleted` row (raw deflate of `0`: the list of deleted objects, empty here) is consumed by the
  apply and not copied: `Config` after the apply equals the staged `ConfigSave` minus `deleted` **row for
  row, `Creation`/`Modified`/`Attributes`/`DataSize` included** (9841 of 9842 rows, case a2).
- A native `config import files --partial` stages a **delta**: the changed descriptor plus `root`, `version`
  and `versions` (4 rows for case a, 8.3.27 and 8.5), no `deleted` row; the apply promotes those and leaves
  every other `Config` row alone.
- **The rows of a full stage are not comparable as text.** A native import writes every descriptor in the
  record versions of the configuration's compatibility mode: in the staged image of case a2, 9166 of the 9838
  `Config` rows differ from the stored ones as bytes, 3639 of them still after inflate (catalog tag 56 -> 57,
  attribute wrapper `{5,...}` -> `{6,...,0,{1,<nil>}}`, standard-attribute records 13 -> 14 elements ...)
  without any structural change; the other 5527 differ in the deflate stream alone. A classifier has to read
  *facts* (section 9.2), not diff rows.
- Other writes of an apply, none derived from the structure change alone: `Params` `DBNames` and
  `DBNamesVersion-DBNames` (only when names were allocated), the `*.si` cache rows and `siVersions`
  (rewritten, with new version guids, on every apply that promotes something; a no-change apply leaves them), two `*.ui` rows, `_ExtensionsRestruct(NGS)` rows
  (state of the extensions' restructure), `Files` help-index rows, and on the first apply of a lineage the
  garbage collection of `ConfigCAS`/`Files` (12797 -> 636 rows, 353 -> 44 rows).

## 4. Formats

### 4.1 Brace text and its layout

Both `DBSchema` and `DBNames` use the platform's brace text. The layout is **byte-exact reproducible** with
two rules: every nested list starts on a new line (`\r\n`), and a list whose last element is itself a list
closes on a new line (`scripts/restructure-lab/bracefmt.py`, `dumps`); `parse -> dumps` gives back the same
bytes for `DBSchema` (977 749 characters + BOM) and `DBNames` of 8.3.27 and 8.5. `DBSchema` and
`NewGenCreated` start with a UTF-8 BOM; the inflated `DBNames` too. Strings are `"..."` with `""` escapes;
unquoted tokens are numbers, uuids, one-letter codes.

### 4.2 `DBSchema`

```
{0,{<count>,<table>,...}}
table    = {"<Name>","N",<number>,"",<fields>,<subtables>,<indexes>,1,"R"|"S",<sepA>,<sepB>,"",0,0}
fields   = {<n>,<field>...}         field = {"<name>",<nullable 0|1>,{<n>,<type>...},"",0}
subtable = {"VT<n>","I",0,"<Owner>",<fields>,{0},<indexes>,1,"S",{0},{0},"",0,0}
index    = {"<name>",<unique 0|1>,{<n>,"<field>"...},<clustered>,<flag>,0,{0},0,0}
sepA/sepB = {0} | {1,{{<n>,"<field>"...}}}
```

The SQL table name is `_` + `<Name>` (the number is already in the name for objects: `Reference20`); sub-tables
are `_<Owner>_<VTn>`. Type entries `{tag,a,b,ref,k[,six]}`:

| tag | SQL | notes |
|---|---|---|
| `B` a | `binary(a)`; a = `0x80000000` -> `varbinary(max)` | `{"B",16}` = uuid, `{"B",2147483648}` = ValueStorage |
| `L` | `binary(1)` | boolean |
| `N` p,s | `numeric(p,s)` | sixth element 1 with s = 0: `int`; 2: `int IDENTITY` |
| `T` | `datetime2(0)` | dates are stored +2000 years (`_YearOffset`) |
| `S` a | `nvarchar(a & 0x7fffffff)` if bit 31 is set, else `nchar(a)`; length 0 -> `nvarchar(max)` | |
| `R` ref,k | typed (`ref` = a table name): `_<F>RRef binary(16)`; untyped: `_<F>TRef binary(4)` + `_<F>RRef binary(16)` | k: 2 = own `ID`, 3 = other reference, 4 = untyped |
| `V` | `timestamp` | row version |
| `E` | `_<F>_TYPE binary(1)` | first entry of a composite type |

A field with several type entries (composite type) becomes several columns: `_<F>_TYPE`, `_<F>_L`, `_<F>_N`,
`_<F>_T`, `_<F>_S`, `_<F>_RTRef`, `_<F>_RRRef` (with a single typed reference only `_RRRef`).
`nullable = 1` -> `NULL`, else `NOT NULL`.

- **Sub-tables** get `_<Owner>_IDRRef binary(16)`, then the **sepB** columns of the owner, then
  `_KeyField binary(4)`, then their fields; a clustered `_<Owner>_<VTn>_SK (sepB, _<Owner>_IDRRef, _KeyField)`
  is implicit; the declared indexes are `_<Owner>_<VTn>_<i>` with sepB in front and the owner key at the
  end (field `ID` means `_<Owner>_IDRRef`).
- **Indexes** of a main table are named `_<Name>_<i>` in list order; **sepA** columns are prepended to each.
  An index with more than 16 key columns is cut to 16 and is not unique (SQL Server limit; found in
  `_InfoRg6849`).
- **Implicit keys** (fields contain `ID` = `{"R",0,0,"<self>",2}`): with sepA -> clustered unique
  `_<Name>_S_HPK (sepA, _IDRRef)`, plus non-clustered unique `_<Name>_S_PK (_IDRRef)` when sepB is empty;
  without sepA -> inline `primary key` on `_IDRRef` (auto-named `PK___...`, hence the new name after
  every rebuild).
- **Table order matters for byte equality**: a rebuilt table is removed from its place and appended at the
  end of the list, in the order of creation of the NG tables; `ConfigChngR` (rebuilt every time) is always
  last. New tables are appended before it. Header count is the number of tables.

Conformance (`dbschema_check.py`; `docs/apply/evidence/restructuring/dbschema-conformance-*.txt`): 8.3.27
БСП 1937/1937 tables (1761 main + 176 sub-tables) columns and indexes, 1761/1761 implicit keys; 8.5 БСП
1922/1922 and 1769/1769. The extension schema (`SchemaID = 1`, 243 entries, SQL names get the suffix
`X1`, e.g. `_Reference10523X1`, `_Document10069_VT10186X1`): all 277 physical tables match (columns; indexes not
checked). Same grammar, same 14-element table entry, same type tags in 8.5; both databases report
`IBVersion 7`, `PlatformVersionReq 80313`.

### 4.3 `DBNames` and `DBNamesVersion`

`Params.DBNames`: raw deflate of `{<max number>,{<count>,{<uuid>,"<kind>",<number>},...}}`. `<kind>` is the
family of the SQL name (`Reference`, `Fld`, `VT`, `LineNo`, `InfoRg`, `ByDims`, ...), `<number>` the
suffix. Entries are sorted by number; **numbers are never reused and entries never removed** (case f: the
deleted attribute stays). `Params.DBNamesVersion-DBNames` is `{0,<uuid>}` with a **new random guid** at
every change (`DBNamesVersion` itself did not change). Each extension has `DBNames-Ext-<id>` and
`DBNamesVersion-DBNames-Ext-<id>`.

**Allocation** (measured in a2, b, c, d, h): the next number is `max` over the headers of the main **and
all extension** `DBNames` plus one (a2: main header 10824, extension b3fa0ef0 11033 -> 11034). New entries
are appended in the order the platform walks the changed objects: in metadata (configuration) order, i.e.
by kind first (case d: the information register got 11036-11037 before the accumulation register got 11038,
although its table number 7272 is larger than 505), inside a kind in the traced cases both name order and
table number order agree (case h: `Reference15`, `20`, `27`, `2598`); within an object a new tabular section
(`VT`, `LineNo`, its attributes) and attributes in metadata order. The header takes the last number and
the count grows. Built-in tables that the platform adds take numbers from the same counter with the nil
uuid. **The header may run ahead of the largest entry by one** (case c: entries up to 11039, header 11040;
the trace track saw 11039 over 11038 after a new catalog): a number was reserved and no entry written (a
companion name of the new catalog, **hypothesis**). The counter is the *header*, not the largest number, and
the rule "max over the headers + 1" is the one the prototype uses (case a on 8.3.27: extension header
11033 -> 11034; on 8.5: extension header 11261 -> 11262).

Which kinds a metadata class allocates (`docs/apply/evidence/restructuring/dbnames-kinds.txt`, all of
БСП): catalog `Reference` + `ReferenceChngR` (+ `RefSInf` for 74 of 114); attribute `Fld`; tabular
section `VT` + `LineNo`; document `Document` + `DocumentChngR`; constant `Const` + `ConstChngR` + `Fld`;
information register `InfoRg` + `InfoRgChngR` (+ `InfoRgOpt`); dimension `Fld` (+ `ByDims` for 111);
resource `Fld` (+ `ByResource`); register attribute `Fld` (+ `ByProperty`); document/BP/task attribute
`Fld` (+ `ByField` when indexed); enum `Enum`; exchange plan `Node`; scheduled job `ScheduledJobs`. A
new catalog without exchange-plan membership allocated **only** `Reference` (case c: no `ChngR`, no `RefSInf`).

## 5. Metadata -> tables

Verified on the whole БСП by `gen_check.py`, `md_types_check.py` (files in the evidence folder).

**Fields of a catalog main table**: `ID`, `Version`, `Marked`, `PredefinedID`, (`OwnerID` for owned
catalogs: not verified), (`ParentID`, and `Folder` only for `HierarchyFoldersAndItems`), `Code` (if
`CodeLength > 0`: `S` variable/fixed or `N` by `CodeType`), `Description` (if `DescriptionLength > 0`),
attributes in metadata order (`Fld<n>`), then the fields of **common attributes** in their configuration
order (the data separator `Fld2683` is one of them, so is the multilingual `НаименованиеЯзык1`). **Document**:
`ID`, `Version`, `Marked`, `Date_Time`, (`NumberPrefix` `T` when `NumberPeriodicity != Nonperiodical`),
`Number`, `Posted`, attributes, common attributes. A new attribute goes **after the last own attribute and
before the common-attribute fields** (case h2: before `Fld7857`).

**Type -> type entries** (231 distinct XML descriptors in the БСП, one mapping each; 4 are
`TypeSet`/defined types whose mapping depends on the resolved set):

| XML | entries | new-column default in the copy |
|---|---|---|
| `xs:boolean` | `L` | `0x00` |
| `xs:string`, `Length=n`, `Variable` | `S` `0x80000000|n` | `N''` (`@P nvarchar`) |
| `xs:string`, `Fixed` | `S` n | `CAST(@P AS NCHAR(n))` of n blanks |
| `xs:string`, `Length=0` | `S` `0x80000000` | `N''` |
| `xs:decimal` (p,s) | `N` p,s | `CAST(@P AS NUMERIC(p, s))`, @P = 0 |
| `xs:dateTime` (Date or DateTime) | `T` | `'2001-01-01 00:00:00'` (the empty date + 2000 years) |
| `cfg:CatalogRef.X`, `EnumRef`, `DocumentRef` | `R` `<Table>`,3 | 16 zero bytes |
| composite (several types) | `E` + `L` `N` `T` `S` (fixed order) + `R` | `_TYPE = 0x01`, `_N = 0`, `_S = N''`, refs zero |
| `v8:ValueStorage` | `B` `0x80000000` | `0x01010800000000000000EFBBBF7B2255227D` |
| `v8:UUID` | `B` 16 | 16 zero bytes |

The defaults are what the copy `INSERT ... SELECT` of case h used (`types-h-statements.sql`).

**Nullable flag**: 1 exactly when the catalog is `HierarchyFoldersAndItems` **and** the attribute has
`Use = ForItem`: 229 of 229 such attributes; every other attribute (719 + 116 + 36 + 8 catalog attributes,
271 document attributes, all register fields) is 0. For nullable fields the copy is
`CAST(CASE WHEN T1._Folder = 0x01 THEN @P1 END AS NVARCHAR(50))`: **NULL for folders, the default for items**
(`_Folder = 0x01` marks an item).

**Tabular sections**: `VT<n>` with first field `LineNo<m>` `N(5,0)` (`LineNumberLength`), attribute fields
`Fld<k>`, no common attributes; the SQL table has the extra columns above. An indexed attribute makes
`ByFieldFld<k>` `{2,"Fld<k>","ID"}` (non-unique).

**Indexes from the `Indexing` flag**: an attribute of a hierarchical catalog (case g) -> two **unique**
indexes, `ByParentFieldFld<n> {4,"ParentID","Folder","Fld<n>","ID"}` and `ByFieldFld<n>
{2,"Fld<n>","ID"}` (flat catalogs: not traced); a document attribute: two naming forms coexist in the corpus
(`Document39` has `ByField5561` on `Fld2909` and `ByFieldFld3821` on `Fld3821`; only 3 of 271 document
attributes have a `ByField` number in `DBNames`; **hypothesis**: numbered names are older allocations);
information register -> the `ByDims`/`ByPeriod` family (below).

**Registers** (case d): a new dimension is inserted after the last dimension and before the resources;
every index over dimensions is rewritten (`ByPeriod (Period, D1)` -> `(Period, D1, D2)`,
`ByDims (D1, Period)` clustered -> `(D1, D2, Period)`), and a **third** index `ByDims<number>
(D1, Period, D2)` appears with the number allocated from `DBNames` (kind `ByDims`, keyed by the uuid of the
first dimension). A new accumulation register resource is inserted after the last resource and appears in
the totals table as well (`_AccumRgT510`: `numeric(21,2)` for a `numeric(15,2)` resource - totals have
higher precision). The derivation of register indexes from `Master`, `MainFilter`, `Indexing` and register
kind is **not** reproduced (259 information registers, 8 kinds of index sets in the corpus).

**Not reproduced yet** (skipped or untested): owner-subordinated catalogs (`OwnerID`), attributes of
defined types and characteristics (type sets), constants, enums, exchange plans, charts, business processes,
tasks, document journals, accounting/calculation registers, predefined data tables, `ChngR`/`SInf`/`Opt`
companions.

## 6. The algorithm for the simplest case (a new attribute)

Implemented in `src/restructure/` (section 9). Given the staged `ConfigSave` (a whole image or a delta) and
the stored `Config`:

1. **Classify** (fail closed, `plan.rs`): no new file (a new object) and an empty `deleted` marker; across the
   attribute bodies of every descriptor that differs, no attribute is removed and none changes its type or
   its indexing (or `Use`); the new attributes -- uuids with no field yet -- all sit in the own attributes of
   **one catalog**; that catalog is otherwise the same as far as its table goes (hierarchy, code and
   description lengths, owners, data history, tabular sections); the catalog has no predefined data; every
   stored attribute maps to the field the stored `DBSchema` entry has.
2. **Number**: `n = max(header of the main DBNames, headers of every DBNames-Ext-*) + 1` per new attribute in
   metadata order; append `{<attr uuid>,"Fld",<n>}`; new guid in `DBNamesVersion-DBNames`; deflate.
3. **Schema**: insert the field entry `{"Fld<n>",<nullable>,{1,<type>},"",0}` right after the field of the
   attribute before it (the first attribute: after the standard fields); nullable when the catalog has the
   `Folder` field and the attribute is `ForItem`; the entry moves to the end of the table list, before
   `ConfigChngR`; the header count is unchanged.
4. **Rebuild** in one transaction: create the `NG` tables of the main table and its sub-tables, copy with the
   default of the type (`0x00`, `N''` / `CAST(CASE WHEN T1._Folder = 0x01 THEN N'' END AS NVARCHAR(n))`,
   `CAST(0 AS NUMERIC(p, s))`, the empty date, ...), create the indexes, drop the old tables, `sp_rename`
   the tables and the indexes; read the result back and compare it with the model.
5. **Publish** in the same transaction: `SchemaStorage` (`Status 100`, the new `CurrentSchema`, empty
   generations), `DBSchema`, `Params` `DBNames` and `DBNamesVersion-DBNames`, the XDTO model row (one
   `<property>` line), the staged `Config` rows (the parts that differ replace the stored ones), `ConfigSave`
   emptied. Not written: `_ConfigChngR` (rebuilt by native on every apply), the two other changed `*.si` rows,
   `siVersions`, `*.ui`, the help index in `Files`, `_ExtensionsRestruct*` (section 9.6).

## 7. Risks and unknowns

- **Flakiness of native staging** (section 10) makes traces expensive; own staging (base-free) worked, and the
  native `import files --partial` (a delta of four rows, a minute) is the cheap way to stage one changed file.
- **The classification of a staged image** is not solved by comparing rows (section 3.4); the prototype reads
  facts for attributes and catalogs, the other kinds need their own readers or the restructuring check (#338).
- **Derived state of an apply** (`*.si` caches, `siVersions`, exchange-plan registration, help index): the
  platform does not repair a stale cache on its own when nothing new is staged (section 9.6).
- **Extensions** (4 in the corpus): `SchemaStorage(1)` and `_ExtensionsRestruct*` are updated by the main
  apply (`_ExtensionsRestructNGS` 0 -> 3 rows in the baseline); an extension that adopts a changed object
  may need its own tables rebuilt. Not traced.
- **Common attributes / data separation** append fields to hundreds of tables; a change there rebuilds
  most of the database.
- **Exchange plans**: `ChngR` tables exist only for objects in exchange plans; `_ConfigChngR` registration.
- **Type sets** (defined types, characteristics) need the resolved set to know the composite columns.
- **Data conversion** of type changes runs inside the 1C engine (case k): string truncation, number
  saturation and the text of a boolean (`Да`/`Нет`, probably language dependent) were seen; the rules for other
  type pairs (string -> number, date <-> string, reference type change, composite <-> simple) and the
  conditions under which the platform refuses or asks for `--force` are unknown.
- **Predefined data** (tables, `Predefined.xml`) and **subordination** are outside the traced cases.
- **Platform-build drift**: a database from an older build needs the built-in tables of the current build
  (about 40 system tables, 4 upgraded or created in our traces); a reference list per build is required.
- **Recovery**: a native apply that stopped after the switch left the database unusable for `ibcmd`;
  an own implementation must be atomic or resumable -- one transaction is atomic (section 9.3). **Size**: the
  NG copy is O(rows) with a log proportional to the table; a single transaction on a 100 GB table is
  impractical (the native path commits in steps).
- **8.5**: formats are identical (section 4.2) and the protocol of case a is the same (section 9.7); 2.21 metadata
  rows and new-in-8.5 objects may add families; the prototype was not run there. The own apply's S1 (12) is proven on
  the 8.5 БСП: `b1`, `c1`, `d1` of 12.6 against the native twins, `evidence/dropin-apply/s1-acceptance-85.md`.

## 8. Estimate

Assumptions: one engineer who knows the repo, Rust, MSSQL, native as an oracle on lab clones, the research
tools of this track available. Numbers are ranges of working weeks; confidence is stated. **Status** is what the
prototype of section 9 has done.

| work package | weeks | confidence | status / note |
|---|---|---|---|
| W1 formats in Rust: brace parser/writer (byte-exact), `DBSchema`, `NewGenCreated`, `DBNames`, `SchemaStorage` | 1.5-2 | high | **done** (`src/restructure/{names,schema,storage}.rs` on `metadata_model::brace`; byte-exact on both corpora) |
| W2 `DBSchema` -> DDL/DML generator, NG driver, publish, conformance tests on the corpora | 2-3 | high | **done for one object family** (tables, indexes, copy with defaults, one-transaction driver with read-back); registers add index derivation, identity columns, in-place copy variants |
| W3 metadata -> schema for catalogs, documents and their tabular sections (all attribute kinds, indexes, common attributes) | 3-4 | medium | started: a new String/Boolean/Number/Date attribute of a catalog; types by reference and composite types need the type-id -> table map; indexes, sub-tables, owners, type sets open |
| W4 registers (information, accumulation: dims, resources, indexes, totals, aggregates) | 4-6 | low-medium | index derivation is not decoded |
| W5 other families (constants, enums, charts, BP, tasks, exchange plans, journals, accounting/calculation registers) | 5-8 | low | 106 table families in the corpus |
| W6 data conversion for type changes, deletions of objects and their references | 3-5 | low | done by the 1C engine row by row (case k); rules per type pair unknown |
| W7 extensions and platform-build drift | 3-4 | low | `_ExtensionsRestruct*`, `X1` tables; the 8.5 drift is 240 `ALTER INDEX` statements on primary keys |
| W8 verification harness (native twin per case, session read, parity export) | 2 | medium | **done as a kit**: twin restore, snapshot/diff, XE trace, native export + `source-diff`, a session on a lab database without the cluster |
| W9 8.5 differences | 2-3 | low | one trace: same protocol for the object (section 9.7) |
| **W10 derived state of an apply** (new): the XDTO model and the other `*.si` caches, `siVersions`, `_ConfigChngR` registration, `_ExtensionsRestruct*`, help index, `MobileVersions.dat` | 2-3 | low-medium | the XDTO row for a simple attribute is done (text equal to native's); `1a621f0f.si` (object registry: counter + one entry) and `c77bc206.si` (permutation) are not decoded |
| **W11 classification of a staged image** (new, shared with the restructuring check #338) | 2-4 | medium | rows of a native full stage differ from the stored ones in the record versions (section 3.4): facts per kind have to be read from both; the prototype does it for attributes of all kinds and for catalogs |

- **Minimal useful subset for 0.4** ("S1"): W1 + W2 + the catalog/document part of W3 restricted to
  **append/delete an attribute of a simple type, add/delete a tabular section, index flag, add a plain
  catalog or document, widen a string** + W8 + W10/W11 for these: about **6-9 weeks** with the fail-closed
  classifier and the round-trip gate (refuse anything else, use native apply for it). Each step is
  independently verifiable on a twin. The prototype covers the first item of that list ("append an attribute
  of a simple type to a catalog") and took one working day of agent time on top of checkpoint 1 (about 5 000 lines
  with the tests; a human engineer should count a week) -- the formats were the work of checkpoint 1; what took
  the time were the twins, the classification and the caches.
- **Full own restructuring** (W1-W11): the packages add up to 29-44 weeks; with a contingency for the
  untraced areas (a third; 29.5-44 weeks x 1.33 = 39-59 weeks) **9-13 months** of one engineer (checkpoint 1 said 7-11), with a long tail; do not promise it for 0.4 or
  0.5.
- **Round-trip gate (recommended)**: before an object family is allowed to change, the generator must
  rebuild the *old* `DBSchema` entries of that family from the *old* `Config` rows and match the stored
  text exactly (the checks of `gen_check.py`/`dbschema_check.py` run at apply time on the affected
  objects). A family the generator cannot reproduce is refused; coverage then grows without risking
  silent damage. The prototype applies it to the fields of the changed catalog (`check_stored_fields`).
- **Not worth building first**: the online/high-load path (0.5) shares the NG protocol but adds
  `NewGenDropped`, sessions and switching under load; it needs its own traces.

## 9. The prototype for case (a): a new attribute in a catalog

Code: `src/restructure/` (about 4 200 lines and 830 lines of tests), command `ibcmd-rs mssql-restructure`;
lab kit: `scripts/restructure-lab/`; fixtures: `tests/fixtures/native-evidence/restructure/`.

### 9.1 What was built

| file | what |
|---|---|
| `names.rs` | `DBNames` (raw deflate of brace text): parse, byte-exact write, the shared counter, `DBNamesVersion` |
| `schema.rs` | `DBSchema` / `NewGenCreated` on `metadata_model::brace` (not a second brace implementation): views over the tree, field insertion, the table -> SQL model (columns, types, indexes, implicit keys, `create table`, `CREATE INDEX` in the platform's text and order) |
| `storage.rs` | `SchemaStorage` states 100/200/400/500 and the empty-generation marker `{0,{0}}` |
| `catalog.rs` | a catalog row read as *facts* through the layout of `metadata_model::objects` (no XML context needed); an attribute's type pattern -> field type entries |
| `xdto.rs` | the XDTO model cache row: insert one `<property>` line |
| `plan.rs` | the checks (9.2), the plan (new schema, names, model row) and its statements |
| `reader.rs`, `exec.rs` | the database side; the plan run in one transaction with read-back checks |
| `command.rs` | `mssql-restructure --database <db> [--dry-run] [--trial] [--alter-add] [--skip-xdto] [--dump-plan <dir>] [--report <file>]` |

`--dry-run` plans and prints (6-24 s on the БСП twin, depending on the load of the machine); `--trial` runs everything and rolls it back; writing to a database
whose name is not `ibcmd_rs_04_*` / `ibcmd_rs_05_*` needs `--allow-non-lab`; other sessions in the database stop it.
Tests: 30 in the crate's lib (formats, DBSchema against the statements of the native trace, plan against the
native result, refusals, XDTO); three of them read the lab corpora and skip themselves without the lab:
byte-exact round trip of `DBSchema` and `DBNames` of the 8.3.27 and the 8.5 БСП, the model against the
columns and indexes of **every** table of both databases (1937 and 1922, `schema.txt` of a snapshot), and the plan of
case a2 made from the staged snapshot against the native result (9.4).

### 9.2 What it checks, fail closed

Everything else is refused with the reason (`plan.rs`, tested in `tests_plan.rs`):

1. the staged image adds no file (no new object) and its `deleted` marker is empty; the image may be a whole
   configuration or a delta (`import files --partial`);
2. over the attribute bodies (`{27,{2,...}}`) of every descriptor that differs -- **all kinds**, not only
   catalogs -- no attribute is removed, none changes its type, indexing or `Use`, and the new ones (uuids
   without a field) all sit in the own attributes of **one catalog**;
3. that catalog keeps its hierarchy, code and description lengths, owners, data history and tabular sections; has
   no subordination, no predefined data (a non-empty `RefSInf`, section 3.3), no other companion than the change
   registration table;
4. every stored attribute of it maps to the field the stored `DBSchema` entry has (type entries and
   nullability) -- the round-trip gate on the one object that is going to change;
5. a new attribute has a boolean, string, number or date type, is not indexed, is a nullable field only if it is a
   string (`ForItem` in a hierarchical catalog: NULL for folders, the default for items).

Not checked -- the restructuring check's job (#338), run it first: a changed property of a non-catalog object that
changes its table (a document's number length, a register's dimension order), and attributes of other kinds that
change in a way that does not touch their type and indexing. Types by reference, composite types and defined
types are refused until the type-id -> table map exists.

### 9.3 One transaction: yes

`SET XACT_ABORT ON; SET LOCK_TIMEOUT 60000; BEGIN TRANSACTION`, then the platform's statements, then the
read-back, then `COMMIT`. Inside the transaction SQL Server 2025 accepted `CREATE TABLE`, `ALTER TABLE ... SET
(LOCK_ESCALATION)`, `CREATE [UNIQUE] [CLUSTERED] INDEX ... WITH (SORT_IN_TEMPDB, MAXDOP, ALLOW_PAGE_LOCKS)`,
`INSERT ... WITH(TABLOCK) ... SELECT`, `DROP TABLE`, `sp_rename` (objects and indexes), and the updates of
`SchemaStorage`, `DBSchema`, `Params`, `Config`. Measured on the БСП twin of the final run (case a2, `a2_fin`;
`prototype-apply-report-a2.json`, `prototype-trial-report-a2.json`):

| run | result |
|---|---|
| `--trial` | 33.5 s in all; 3 `NG` tables created, 14 + 29 + 18 rows copied, 10 indexes, the old tables dropped, 13 renames, then the publication (the XDTO row included); every check passed; `ROLLBACK`. Afterwards the database is what it was: `ConfigSave` 9842 rows, `Config` 9838, `Status 100`, the same `CurrentSchema` hash, no `NG` table, `_Reference20` with 15 columns |
| apply | 9.0 s in all, 7.3 s inside the transaction, `COMMIT` |
| by phase (apply) | create 17 ms, copy 8 ms, indexes 95 ms, drop 5 ms, renames 22 ms, publication 7.0 s of which the promotion of a **whole** stage (9 842 rows) 5.9 s and emptying `ConfigSave` 0.9 s; a **delta** stage promotes in 0.07 s. The first run on a loaded machine (`a2_own`, before the XDTO row): apply 65 s, of which the index phase 46 s and the promotion 17 s; trial 40.8 s. The structure work itself is a fraction of a second for a catalog of 14 rows |

What atomicity gives: no `Status` 200/400/500 is ever visible and `NewGenCreated` is never written, so there
is nothing to resume and no "незавершенная операция" state (section 10, finding 3); a failed check rolls
back. What it costs: the log of the whole copy in one transaction (kilobytes for a catalog; for a table of 100 GB it is
not acceptable, the native path commits in steps: cap the size of what is rebuilt this way or chunk the copy);
`Sch-M` locks on the object's tables for the duration (exclusive apply anyway). Differences from the native
sequence, all harmless in the result: the indexes are created once, after the load (native creates them, drops
them, loads a heap and creates them again); `_ConfigChngR` is not rebuilt.

### 9.4 Verification on the twin of case a2

Both twins come from the same backup of the staged state; native applied one (`a2_nat`), the prototype the other
(`a2_fin`, the final run; the first run on `a2_own` gave the same tables, `DBSchema`, `DBNames` and `Config`, without the XDTO row).
Full output: `twin-compare-a2.txt`, `sessions-a2.md`.

| check | result |
|---|---|
| plan made offline from the staged snapshot vs the native result (`tests_corpus.rs`) | `DBNames` text **equal**; every `DBSchema` entry **equal** but `DbCopies` and `DbCopiesUpdates` (the platform upgraded those two system tables on its own, a build drift; the native table order differs only by where they sit); the new `Reference20` entry equal byte for byte; the XDTO model row text equal to native's (25 173 491 characters) |
| snapshot of both twins (`twin-compare-a2.txt`, section 1) | 2234 tables in both; structure identical but the auto-named primary key of `_ConfigChngR` (random) and the two upgraded system tables |
| data of the rebuilt tables | `_Reference20` (all columns but the row version), `_VT155`, `_VT159`: `EXCEPT` both ways **0 rows**; the new column: 1 folder NULL, 13 items `''`, as native |
| `Config` after | 9841 rows, **`EXCEPT` both ways 0 rows -- `Creation`/`Modified`/`Attributes`/`DataSize`/`BinaryData` included**; `ConfigSave` 0, `ConfigCAS` identical (`EXCEPT` 0), `SchemaStorage` both rows `Status 100` and empty generations |
| the XDTO model row `Params ea13a2c9-....si`, inflated | **equal to native's** (25 173 494 bytes, the same sha256); the deflated rows differ in size (2 998 675 native, 2 977 849 ours: another deflate implementation) |
| native `config apply` on our twin | "Обновление конфигурации базы данных не требуется", exit 0 |
| the **same catalog staged again** on our twin (native `import files --partial`, 4 rows) and a native apply, traced | 1611 events, **no `create table`, no `drop`, no `sp_rename`**: the platform's own recomputation of the structure from the staged metadata equals the `DBSchema` we wrote. (No DDL at all; it registered the change for exchange plans and rewrote the `*.ui` rows) |
| native export (of both twins, `source-diff`) | 12 198 files, **identical** between the native twin and ours, `ConfigDumpInfo.xml` included. Against the reference tree (the БСП exported before the edit): exactly `Catalogs/_ДемоПартнеры.xml` (**byte-identical to the case-a edit**) and `ConfigDumpInfo.xml` differ; with `configVersion` blanked the latter has the new attribute and lacks two empty module entries of `Catalog.НастройкиАвторизацииИнтернетСервисов` that the native import dropped (the same in the native twin) |
| session on our twin: a stand-alone server + thin client, and a **real session in the 1C server cluster** (the twin registered with `register-ib.ps1`, `Srvr="localhost:2541"`) | the catalog reads with the new attribute empty in all 14 rows (13 items `''`, the folder NULL) -- as on the native twin; an item written with a value is stored, found by a query on the attribute and changed; the object is serialized through XDTO **with** the attribute (only when the model cache is right, 9.6: a stale row makes the same job fail) |

Where the twins still differ, all of it derived state (9.6): `_ConfigChngR` (native rebuilds it on every apply
with new keys and registers 782 more objects as changed for the nodes: `_MessageNo` NULL 17 327 vs 16 545 rows;
only matters with exchange plans), `Params` (`*.ui`, `siVersions`, two `*.si`, random guids; and the session-state row `ecsreg_*` that the first server start
removes from ours as it did from native's), `Files`
(`MobileVersions.dat`, `gc.mrk`, the help index: native leaves 2 bytes in `userDocs_ru` and friends, ours keeps
what was there), `_DbCopies*`.

### 9.5 `ALTER TABLE ... ADD`: accepted by the platform

Question: may an own apply add the column in place instead of rebuilding the object (much cheaper for a big table)?
The column lands at the end of the physical table, after the separator column `_Fld2683` instead of before it.
Experiment (`--alter-add` on the copy of our twin, a second attribute `ДемоВторойРеквизит` String(20) staged by the
native partial import): the plan is `ALTER TABLE dbo._Reference20 ADD _Fld11035 nvarchar(20) NULL` plus `UPDATE ... SET
_Fld11035 = N'' WHERE _Folder = 0x01`, the publication as before; 0.3 s in all. The physical order is now
`..., _Fld11034, _Fld2683, _Fld11035`.

| probe | result |
|---|---|
| session (stand-alone server + thin client) | the metadata knows the attribute; a new item with both attributes is written, read back, found by `WHERE ДемоВторойРеквизит = &Значение`; an existing item is changed and written; XDTO serialization contains the attribute (after the model cache was dropped, 9.6) |
| a third attribute staged (native partial import) and a **native apply with a trace** on the ALTER state | native rebuilt the catalog's three tables ("Изменена структура таблиц базы данных", `alter-native-structure-statements.sql`): `_Reference20NG` is created with `..., _Fld11034, _Fld11035, _Fld11036, _Fld2683`, the copy reads the ALTER-added `T1._Fld11035` **by name**, and afterwards `sys.columns` has the platform's order -- **the next native rebuild normalizes the column order**. The data survived (both earlier attributes, the item changed in the session) |

A native `config apply` with nothing staged directly after the ALTER was not run: the stored `DBSchema` is the same text a rebuild
would have written and the platform decides from `DBSchema` (the re-stage on a rebuilt twin ran no DDL, section 9.4), so "не требуется" is
expected -- an inference, not a measurement.

Verdict: the platform reads and writes by column *name*; column order is not part of the contract as far as sessions
and its own restructure are concerned. **Hypotheses not tested**: a table of `_InfoRg`/`_AccumRg` family (the aggregates and
totals read positionally?), extensions, the online path. Two costs remain: the physical order then differs from what
the platform would produce, so a byte-for-byte comparison of the table structures with a native twin stops working (which
is how the prototype proves itself), and the size of the row grows in place (page splits) where a rebuild packs it. The prototype
keeps the rebuild as the default and `--alter-add` as the research switch.

### 9.6 Which `Params` rows a restructure needs

Native writes these on the apply of case a2 (twin comparison; each `*.si` is a derived cache):

| row | native | prototype | needed |
|---|---|---|---|
| `DBNames` | +1 entry, header 11034 | same text (deflate differs) | yes, by construction: the numbering of the new field |
| `DBNamesVersion-DBNames` | new random guid | new random guid | yes, by construction (version of the numbering) |
| `ea13a2c9-...si` (XDTO model of the configuration: `{2,1,{{#base64:<model xml>}}}`, 64-character lines, CR CR LF) | one line inserted: `<property name="ДемоНовыйРеквизит" type="xs:string" lowerBound="0"/>` in `CatalogObject._ДемоПартнеры`, after the property of the attribute before | the same line, same text | **for XDTO**: with the row stale, `СериализаторXDTO.ЗаписатьXML(object)` fails "Свойство 'ДемоНовыйРеквизит' не обнаружено"; queries and writes work. The row **deleted** -> the platform rebuilds the model in memory, serialization works and the row is not written back. **All** `*.si` rows deleted -> the stand-alone server exits at start. A native apply with nothing new to promote does not refresh the rows |
| `1a621f0f-...si` (object registry, 32 422 lines: a pre-order list of the metadata objects, `}<flag>,<flag>,<uuid>,<owner uuid>,<kind>,"<name>",{1,1,{"ru","<synonym>"}}`, kind 36 = attribute, 39 = tabular section; the counter is the number of entries) | counter `{10807,` -> `{10808,` and one entry, placed after the last attribute of the owner catalog and before its first tabular section (`xdto-model-a2.txt`) | left stale | no effect seen in any check; the rule for a new attribute of a catalog is known, several attributes are inserted in metadata order (native, third attribute), not implemented |
| `c77bc206-...si` (a list of uuids with zeros, 3 lines) | the same list in another order -- the **same text** after the apply of one attribute and after the apply of three | left | no effect seen; the order rule is not decoded (it does not depend on the number of attributes) |
| the other 13 `*.si`; `siVersions` (plain text `{0,16,"<row>.si",<guid>,...}`) | the 13 rows keep their content; `siVersions` gets a new random guid for **every** row on every apply and the pairs are reordered | left | no effect seen in fresh sessions. A server that is **already running** and keys its cache by these guids would need a new guid for a row that changed -- not tested (needs a server across the restructure; 0.5) |
| `*.ui` x2 | rewritten (encrypted) | left | not needed: these are the platform's configuration-licensing records (track ui, coordinator's note); an own apply does not create or change them |

So a restructure needs the numbering rows and, to be complete, the model cache row(s); the prototype writes the XDTO row
(`--skip-xdto` leaves it stale). The alternative that costs no code: **delete** the stale XDTO row and let the platform
recompute it (first session pays for the 25 MB model). A stale set of the other two `*.si` rows is the open item.

Native computes all these caches **from the metadata**, it does not patch the stored rows: the apply of a third attribute on a twin whose caches our
runs had left stale wrote all three properties into the XDTO model and all three entries into the registry (`alter-experiment.md`). A stale cache
therefore heals at the next native apply that touches the object; until then only XDTO serialization of the changed object suffers. Two real cluster
sessions on the final twin rewrote none of the `*.si` rows and none of `DBNames` (`sessions-a2.md`): the platform accepts the rows the prototype
leaves.

### 9.7 The 8.5 trace

Case a on the 8.5.1.1150 БСП (`ibcmd_rs_04_ddl_bsp85_a`, native 8.5 import of the same catalog edited in the 2.21
dialect with `import files --partial`, 4 rows staged in 61 s, then a traced native apply; the user is
`Администратор (обычное приложение)`, plain `Администратор` asks for a password there).

- **The protocol is the same.** The 39 statements of `Reference20` -- three `NG` tables, ten indexes, the copy, the
  renames -- are those of 8.3.27 with the new field number (`Fld11262`: extension header 11261 + 1), the same
  index order, the same `Status` 200/400/500 updates around `NewGenCreated`, `_ConfigChngR` rebuilt through
  `insert bulk`, four `INSERT` into `_ExtensionsRestructNGS`. `IBVersion 7`, `PlatformVersionReq 80313` both.
- **Differences, all outside the object:** 8.5 does not upgrade `_DbCopies*` (the 8.5 clone is on the current build) but
  runs **240 `ALTER INDEX ... SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)`** on the primary keys of older tables; it
  rewrites all 16 `*.si` rows (three in 8.3.27), `siVersions` and both `*.ui`; the first apply of the lineage collects
  garbage (`ConfigCAS` 19 359 -> 730 rows, `Files` 369 -> 42); `_ExtensionsRestruct` 80 -> 82 and
  `_ExtensionsRestructNGS` 0 -> 3 rows (three extensions); the descriptor row is 4745 -> 4802 bytes, as in 8.3.27.
- The prototype was not run on 8.5 (the statements of the object are the same, the record layouts of 2.21 differ
  in `Since8_5_1` slots that the catalog layout does not have); `catalog.rs` reads the 8.5 catalog row through the
  same layout code and the model reproduces all 1922 tables of the 8.5 БСП.

### 9.8 Not done, open

(Section 12 takes these up: the set S1 of operations, their evidence and the split into sub-issues.)

- **Case e alone** (widening a string), **deleting an object**, **changing the type of a reference**: not traced;
  the prototype refuses type changes and deletions (the data conversion of case k is done by the 1C engine).
- **Indexes** (the `Indexing` flag creates `ByField...` entries), **tabular sections**, new objects, registers,
  documents (their attribute wrapper differs and their `Use` does not exist): the model already reads them, the plan does not
  change them.
- **Types by reference and composite types**: need the map of type ids to table names (`Reference569`, `Enum2894`) built from
  the generated types of the rows; then `R`/`E` fields.
- **Predefined data**: the platform rebuilds `RefSInf` with the catalog (case h); the prototype refuses such a catalog.
- **Extensions**: `SchemaStorage(1)`, the `X1` tables and `_ExtensionsRestruct*` (native inserts four rows into
  `_ExtensionsRestructNGS` on every apply and removes them again); an extension that adopts the changed catalog is
  not looked at.
- **`--dynamic=force`**, ERP УХ, online (0.5): not traced by this track.
- **Derived state**: the object registry and the permutation `*.si` (the entry rule for one attribute is known,
  the permutation's is not), the version guids of `siVersions` (needed by a server that runs across the restructure), the `_ConfigChngR`
  registration for exchange plans, the help index in `Files`; big tables (chunked copy, section 9.3).
- **Not run**: a native `config apply` with nothing staged right after an `ALTER TABLE ... ADD` (expected "не требуется", section 9.5);
  the prototype itself on 8.5 (the object's statements are the same, 9.7).

## 10. Findings other tracks need

1. **Our patch-mode import silently drops a new attribute.** Case a: 9517 rows staged, every row identical
   to `Config` except `versions`; the native apply then ran to the end (92 s) and changed nothing
   structural. The import must refuse or stage the attribute. `--base-free` staged
   the whole edited tree (9838 rows; 3 multi-part rows `5189beb9..0[1..2]`, `7e3283df..0[1]` are missing
   compared with native) and **native apply accepted it on an existing database** for 12 attributes of
   every type, a new tabular section, and a new catalog (cases h, c).
2. **Native `config import` failed twice under load; the row count is not the completeness test.** The first
   import after an apply failed with "Ссылка на неизвестный предопределенный элемент -
   ChartOfCharacteristicTypes.ОбъектыАдресацииЗадач.ВсеОбъектыАдресации" (the reference changed between
   attempts and disappeared on retry; the import uses several writer connections; **hypothesis**: a race
   between the load of predefined items and the objects that refer to them at 90-99% CPU), and two
   applies of a stage failed at once with "Нарушена целостность структуры конфигурации" (cases c, e). Checkpoint 1
   called the stages of 9597 / 9615 / 9635 rows *partial* because a stage of this configuration was 9842 rows
   in the runs before; the trace track measured that a **complete** native stage of the БСП holds about 9 618
   rows. Those counts were probably complete stages, so the diagnosis "partial" is withdrawn. The test is:
   `ConfigSave` has a `versions` row, no `commit` and no `*.new` rows, and every staged `versions` entry that
   differs from `Config`'s has its row. The two errors above remain real; their cause is not established (a
   second import worked each time). `--threads` is not accepted by import.
3. **A native apply can die at "Принятие изменений"** (exit -1, no message; case d on clone `a`, while a
   second ibcmd of this track - an import and an apply on another clone - ran at the same time; the clean
   re-run on a fresh clone passed while four ibcmd processes of other tracks were running, so concurrency
   alone is not proven to be the cause). The database was left with
   `DBSchema` new, `Config` with `*.new` rows and `ConfigSave` full, and **every** `ibcmd config` command,
   including `config repair --commit`, `--rollback` and `--fix-metadata`, answered with the same "Обнаружена
   незавершенная операция сохранения конфигурации". Do not use such a clone.
4. `ibcmd infobase config export info|status` (ConfigDumpInfo of the database, differences against a
   given ConfigDumpInfo) and `config import files --partial` exist natively and may help the apply track.
5. The native apply moves **all** staged rows through `.new` even when unchanged (9839 rows, about 65 s of
   the 113 s); an own apply that promotes only changed rows in one transaction should be much faster.
6. `ConfigDumpInfo.xml` did not need an entry for a new object once the stage was complete (the case c
   failure was a partial stage); not tested in isolation.
7. **Detecting a structural change (for an own apply that must refuse it).** The platform recomputes the whole
   `DBSchema` from the metadata and rebuilds exactly the tables whose entry differs. An apply without
   restructuring can therefore be guarded the same way: build the expected entries of the changed
   objects from the staged rows and compare them with the stored `DBSchema`; any difference (or an object
   family the generator does not know) means "structural, use native apply". As a cheaper first cut, a
   changed object row of a table-owning kind (catalog, document, register, constant, enum, chart, ...) that
   differs from `Config` in anything but names, synonyms, comments, forms, commands, templates and
   module flags is structural.
8. **XE capture recipe** that worked: `sql_batch_completed`, `rpc_completed`, `sql_statement_completed`,
   `object_created|altered|deleted`, action `database_name` predicate, `EVENT_RETENTION_MODE = NO_EVENT_LOSS`
   (then `error_reported` cannot be added), `event_file` target. `sp_statement_completed` adds 200 000
   events per apply and no information beyond `rpc_completed`; `object_*` events list every DDL with its
   text, including the temp tables of the platform.

9. **The staged image is not comparable to the stored one as text** (section 3.4): 9166 of 9838 `Config` rows
   differ as bytes after a native import of the tree exported from the same database, 3639 after inflate, in
   record versions only. An apply or a check that diffs rows will call everything changed; read facts.
10. **`import files --partial` stages a delta** (`<descriptor>`, `root`, `version`, `versions`; 4 rows, about a
    minute, 8.3.27 and 8.5) and the native apply of it restructures as usual. The `Администратор` of the 8.5
    БСП corpus has a password: use `Администратор (обычное приложение)`.
11. **A native apply that has nothing new to promote does not rewrite the `*.si` caches** (it rewrote the `*.ui`
    licensing rows and registered the change for exchange plans): a cache that a direct write left stale is not
    repaired by the next no-change apply. An apply that changes the object **recomputes** the caches from the metadata
    (the third attribute on a twin with stale caches wrote all three properties / registry entries). The XDTO model
    cache decides whether a changed object can be serialized (section 9.6); a session can run without any stale-cache
    error otherwise.
12. **The stage of a `Config` promotion can be set-based:** `DELETE`/`INSERT ... SELECT` of the parts that differ
    (18-30 s for a whole 80 MB image in a loaded lab, 0.07 s for a delta) instead of the native 9 839
    row-by-row `.new` renames; the `deleted` marker row is not copied.
13. **A native `ibcmd` that asks for a password holds the lock and waits for ever** (a plain `Администратор` on the
    8.5 corpus held the native lock for 23 minutes until it was killed). The kit's `Invoke-NativeCommand`
    closes stdin, sets a timeout and caps the output.
14. **`Params` holds a session-state row, `ecsreg_<id>`** (306 bytes in the БСП corpus): the first server start (a session) removes it; whether a
    native `ibcmd` command does too was not tested. A comparison of `Params` between two databases must ignore it (it is why two snapshots
    taken before and after a session differ by one row).
15. **The platform builds derived state from the metadata on every apply that changes something**: `*.si` caches, the change registration
    `_ConfigChngR` (rebuilt with new keys, 782 more objects registered than in an own restructure) and `_ExtensionsRestruct*`. An own apply
    that skips them is right for the database (sessions accept it) and stale for XDTO, until the next native apply.

## 11. Evidence

Repo (`docs/apply/evidence/restructuring/`): `a2-structure-statements.sql` (the NG protocol verbatim),
`newgen-created-a2-first.brace.txt`, `newgen-created-a2-last.brace.txt`, `timeline-dm.txt`,
`types-h-statements.sql`, `dbschema-conformance-8327.txt`, `dbschema-conformance-85.txt`,
`gen-check-catalogs-documents.txt`, `md-types-vs-dbschema.txt`, `dbnames-kinds.txt`, `roundtrip.txt`,
`conversion-k.md`, `cases.md`. Checkpoint 2: `prototype-statements-a2.sql` (the statements of the plan of case a2
without the binary parameters), `prototype-apply-report-a2.json` and `prototype-trial-report-a2.json` (the reports of the real
run and of the trial: steps, times, read-back checks), `twin-compare-a2.txt` (native twin against ours: snapshot diff,
`EXCEPT` of the data, exports), `xdto-model-a2.txt` (the change of the XDTO model row and the other `*.si` rows, `siVersions`),
`sessions-a2.md` (the jobs and their output), `a85-structure-statements.sql` (the 8.5 trace, the `ALTER INDEX` list cut),
`alter-experiment.md` and `alter-native-structure-statements.sql` (the ALTER experiment and the native rebuild that followed).
Tests: `src/restructure/tests_*.rs`, fixtures `tests/fixtures/native-evidence/restructure/`.
S1 (issue #391, section 12): the evidence files are listed in 12.10.

Lab (`F:\ibcmd\lab\04\restructure`): `xe/<case>/events.jsonl` (the raw `.xel` files are deleted), `out/diff_<case>.txt`,
`snap/<db>/<label>/{schema.txt,tables.tsv,svc.json}` with blobs in `blobs/` (kept: a2_staged, a2_after, the two
twins, the 8.5 БСП), `bak/*_a2_staged.bak` (twin source), `logs/`, `tree/patches/<case>/{before,after}` (the exact
XML edits), `probe/` (the session processing and jobs), `STATUS.md`. Scripts: `scripts/restructure-lab/README.md`.

## 12. S1: the own apply hands structural changes to the own restructure (#391)

Design of checkpoint 1 of #391 (2026-09-29), with a working spike. What is measured says so; a proposal or an
inference is marked. Code: `src/restructure/` (`s1.rs` the gate, `script.rs` the T-SQL of the plan, `plan.rs`,
`object.rs`, `registry.rs`, `xdto.rs`), the seam in `src/mssql_config_apply/` (12.4). Evidence: 12.10. S1 is the minimal
set of restructurings the own apply does by itself: attribute add and delete, tabular section add, the index flag,
string widening, a plain new catalog or document -- catalogs and documents only.

### 12.1 The decisions in short

| question | answer |
|---|---|
| what is in S1 | on **catalogs and documents**: add / delete an attribute of a primitive type, add a tabular section (and attributes of one that was there), switch the index of an attribute, widen a variable string, add a plain object. **Built**: adding attributes of every primitive type (12.8), deleting an attribute and widening a string (12.11), switching the index of an attribute (12.13), a new tabular section and new attributes of the sections an object had (12.14), and every combination of them in one stage on different objects, and all but one on the same object (12.15). Everything else is a refusal that goes to the platform's own apply: other kinds (registers, charts, ...), other properties, types by reference and composite types, predefined data, subordination, data history, an extension that adopts the object, 8.5 |
| where the structure work runs | **inside the apply's transaction**: the apply's locks and assertions, then the *structure phase*, then the fold of dynamic generations, the move of the staged rows, the resets and the caches, the postconditions, `COMMIT`. One SERIALIZABLE script; any failed assertion rolls the rebuilt tables back with everything else; no `Status 200/400/500` is ever visible and there is nothing to resume |
| how the apply asks | the gate it already has (`StructuralGate::check`) plus `take_structure()` and `ScriptInputs.structure_sql`: about 100 lines in `mssql_config_apply` (half of them comments and a test), and no dependency of it on `restructure` (12.4) |
| which caches | attribute operations change two `Params` rows -- the XDTO model and the object registry -- and `siVersions`; a new object needs three more. **Deleting a stale row is not a cheap route**: with `1a621f0f` or `a07b62f0` absent the server does not start (12.5) |
| the split | eleven sub-issues (S1-A ... S1-K) in waves, each with a twin-against-native acceptance (12.6, 12.7) |
| open | 12.9 |

### 12.2 From reasons to operations

The apply's plan runs the conservative gate first; it refuses every descriptor whose text differs, which is where a
structural change shows up. The S1 gate (`restructure::s1::S1Gate`) starts from that refusal and tries to explain it:

```
ConservativeGate verdict            blockers = the descriptors that differ
  -> check_staged (rcheck)          reasons {class, object, row, property, change}
  -> apply_check::s1::classify      typed reason -> S1 operation | refusal                (12.2.1)
  -> plan (restructure::plan)       reads the same descriptors itself: new DBSchema, DBNames, caches, statements
  -> agreement                      the planned objects and attributes == the check's       (12.2.2)
  -> the planned descriptors' blockers are withdrawn; any other blocker left = refusal
  -> StructurePhase                 {sql, params_rewrites, tables, objects, caches}         (12.4)
```

Every link fails closed: an error of the plan or of the check is a blocker (`S1: <reason>`), and the apply refuses with
its existing `StructuralRefusal` and exit path. A stage the conservative gate accepts (bodies, layout-only descriptors)
never reaches the S1 gate's logic: it is a plain apply, unchanged.

#### 12.2.1 The reasons that name an operation

The strings are what `mssql-apply-check` prints for the cases of the traces (`--tree` on `tree/patches/<case>/after`, or
the ConfigSave of the twin); the code matches the *property path* and the first word of the *change*, not the rule text.

| reason (`[structure] <object>: <property>: <change>`) | S1 operation | built |
|---|---|---|
| `Catalog.X`, `Document.X`: `ChildObjects/Attribute[A]`: `added (a column is added or dropped)` | add an attribute | **yes** |
| `...`: `ChildObjects/Attribute[A]`: `removed (a column is added or dropped)` | delete an attribute | **yes** |
| `...`: `ChildObjects/Attribute[A]/Properties/Type/StringQualifiers/Length`: `50 -> 100 (a property of an attribute no rule covers)` | widen a string (the plan judges the direction: a shorter limit is refused) | **yes** |
| `...`: `ChildObjects/Attribute[A]/Properties/Indexing`: `DontIndex -> Index (a property of an attribute no rule covers)`, and `DontIndex <-> IndexWithAdditionalOrder` | switch the index (`Index <-> IndexWithAdditionalOrder` is refused: not traced) | **yes** |
| `...`: `ChildObjects/TabularSection[T]`: `added (a tabular section is added, dropped or moved)` | add a tabular section (its attributes are not separate reasons of a new section) | **yes** |
| `...`: `ChildObjects/TabularSection[T]/ChildObjects/Attribute[A]`: `added (a column of a tabular section is added, dropped or moved)` | add an attribute to a tabular section that was there (`AddSectionAttribute`; dropped or moved: refused) | **yes** |
| `Catalog.X`: (no property): `added (Catalog; an object that owns tables or stored data is added or dropped)` **and** `Configuration`: `ChildObjects/Catalog[X]`: `added (...)` | add an object (the pair) | no |

Refused, each with a message that names the reason: a `data` reason (predefined items, the content of an exchange plan) or
an `unknown` one (a row the check cannot read, a row that "differs but both sides decode to the same XML"); any kind but
catalog and document (case d: a register's dimension and resource); a property no operation covers (`CodeLength`,
`HierarchyType`, the precision of a number, a column of an **existing** tabular section that is dropped, moved or changed); an operation of the
table that is not built ("designed (12.3) but not built in this version"). One refused reason refuses the stage.

#### 12.2.2 Two decoders must agree

The check reads descriptors with rcheck's decoder; the plan reads them through `metadata_model::objects::layout`. The gate
lets a stage through only when both name the same objects and, for each, the same attributes (`decide` in `s1.rs`): a
change one of them does not see is a disagreement and a refusal. The plan checks besides (fail closed, `plan.rs`): the
images add no file, the `deleted` marker names removed attributes and nothing else, no attribute changes its indexing or
`Use`, a retyped attribute is a variable string whose limit grows (a fixed or unlimited string, a shorter limit, any other
type change and every difference of the type text but the limit are refused), the object keeps its
shape (hierarchy, code and description lengths, number length, owners, data history), the object has no predefined data
(a non-empty `RefSInf`), no companion table but its change-registration table, and **every stored attribute maps to the
field the stored `DBSchema` entry has** -- the generator reproduces the stored entry before it is trusted to extend it.

#### 12.2.3 The shapes of a staged image (the classification, W11)

| shape | measured | S1 |
|---|---|---|
| **delta stage**: the changed descriptors plus `root`, `version`, `versions` (a native `import files --partial`; our patch-mode import) | 4 rows (case a2), 9 rows (the types case: 6 descriptors); the check's reasons are the real changes only | accepted |
| **whole image** of a native import (`import files` of the tree exported from the same database) | the a2 image restored as `s1_full`, 9 839 rows: **400 reasons** = 1 real (`structure`: the attribute) + 396 `unknown` "the row differs (a -> b bytes) but both sides decode to the same XML" (record version 56 -> 57) + 2 for the `Configuration` `{68}` row ("fields 26 and 43 hold 80324 and 80327") + 1 for the `deleted` row | refused: the noise is `unknown`, so the stage is |
| a stage with a `deleted` row | a whole native import writes `0`; a partial import that removes attributes writes `<n>,"<attribute id>",1,...` (one id per removed attribute, flag 1; measured, case b1: 10th row of the stage, 266 bytes). The apply refuses any `deleted` row before it asks the gate, unless the gate says it judges them (`StructuralGate::judges_deleted_row`) | the S1 gate judges it: every id must be an attribute the stage removes (flag 1) and nothing else; the structure phase then deletes the row itself under its content hash (`consumed_staged_rows`, the apply's counts are adjusted). Any other id, an object or a file: refused |

Lifting the whole-image refusal is a classification job (sub-issue S1-H): the source-tree route the check track advises
(`mssql-apply-check --tree` on the import's input) is the reliable input meanwhile; treating "same XML" rows as noise needs
the check's decoders to be total, which is theirs to state.

### 12.3 The operations: what each one changes

What the platform does for each operation, from the native traces of checkpoint 2 and of this phase (`cases.md`,
`types-h-statements.sql`, the twin comparisons). "Rebuild" is the new-generation (NG) copy of section 3.2; the platform
rebuilds the tables whose `DBSchema` entry differs, and nothing else.

| operation | `DBSchema` | `DBNames` | tables | copy | caches | evidence | state |
|---|---|---|---|---|---|---|---|
| **add an attribute** (catalog, document) | the field entry `{"Fld<n>",<nullable>,{<type entries>}}` inserted after the field of the attribute before it (the first after the standard fields) | `Fld` entry appended, `n` = max over the main header and every `DBNames-Ext-*` header + 1, in the objects' order of the configuration's lists and the attributes' metadata order | the object's **main table and all its sub-tables** rebuilt (a2: 3 tables; the types case: 11 tables of 6 objects) | by column name; the new column gets the default of its type: string `''`, fixed string spaces, number `0`, date/time `2001-01-01 00:00:00`, boolean `0x00`, uuid 16 zero bytes, value storage `0x0101...`; nullable (`NULL` for the rows the attribute does not apply to) only for an attribute `Use ForItem` / `ForFolder` of a hierarchical catalog | XDTO model (one `<property>` line), object registry (one record: kind 36 catalog, 41 document), `siVersions` | a2, b, h, types case (T1) | **built** |
| **delete an attribute** | the field entry removed; the indexes of the attribute go with it (`ByFieldFld<n>`, `ByOwnerFieldFld<n>`, `ByParentFieldFld<n>`); in a document with an additional-order attribute the date index `ByDocDate` **stays** and loses the field from its list (b2); the indexes that stay are **renumbered** (`_Document1564_5` -> `_4`) | unchanged: the entries of the removed attributes stay (b1: the header and the count as they were, plus the new numbers of the attributes the same stage adds) | the main table and **all its sub-tables** rebuilt without the column and its indexes (b1: 8 tables of 6 objects; b2: 4 tables of 2) | by column name; the dropped column's data is not carried | XDTO lines removed, registry records removed and the count decremented, `siVersions` | f, b1, b2 | **built** (S1-B). The stage's `deleted` row lists the removed attributes (12.2.3) and is consumed by the phase |
| **widen a string** (variable length, `b > a`) | the type entry of the field `{"S",0x80000000\|a}` -> `\|b`; the field keeps its place, the declared indexes stay as they are | unchanged (g, c1) | the object's **main table and all its sub-tables rebuilt** (c1: 5 objects, 10 tables -- the platform rebuilds, it does not `ALTER COLUMN`); the column `nvarchar(a)` -> `nvarchar(b)` | the column copied as it is | **none**: the text of all 16 `.si` rows is the same before and after the platform's apply (c1, measured) | g, c1 | **built** (S1-C). Narrowing, a fixed string, an unlimited one (`nvarchar(max)` is another column type), a change between them and every other type change is data conversion (case k, done by the 1C engine): refuse |
| **switch the index** (`DontIndex` <-> `Index`, `DontIndex` <-> `IndexWithAdditionalOrder`) | the declared indexes of the entry only, the fields are the same: `ByFieldFld<n> {2,"Fld<n>","ID"}` (a hierarchical catalog first gets `ByParentFieldFld<n> {4,"ParentID","Folder","Fld<n>","ID"}`), with the additional order the key goes on with `Description,ID,Marked` (a catalog) or `Date_Time,ID,Marked` (a document, whose date index `ByDocDate` also lists the field last); the entries sit among the attribute indexes in the order of their fields in the table, the physical indexes are renumbered; off is the reverse | unchanged: **no `ByField` entry** for a switch (d0, d1) | the object's **main table and all its sub-tables rebuilt** (d0: `_Reference2598`; d1: 6 objects, 11 tables), the columns as they were | as it is | **none** (the text of all 16 `.si` rows is the same before and after, d0 and d1) | g, d0, d1 | **built** (S1-D). `Index` <-> `IndexWithAdditionalOrder` and several additional-order attributes of one document are not traced: refused |
| **add a tabular section** (with attributes) | a new sub-table entry appended to the object's sub-table list: `{"VT<n>","I",0,"<Owner>",{k+1,{"LineNo<n+1>",0,{1,{"N",5,0,"",0}},"",0},<one field per attribute>},{0},{<declared indexes>},1,"S",{0},{0},"",0,0}`; an indexed attribute (`Index`) gets `ByFieldFld<m> {2,"Fld<m>","ID"}`, **not unique** (the 72 sub-tables of the БСП that have indexes: all `ByField*`, none unique) | per object, in the objects' order of the configuration's lists (changed objects before new ones, catalogs before documents): the object's own new attributes first, then each section in the order of the descriptor -- a section that was there one `Fld` per new attribute, a new one `VT` and `LineNo` (both keyed by the section's uuid, `LineNo` = `VT` + 1) and one `Fld` per attribute (e1: 11034 ... 11053; e4: 11034 ... 11047) | the object's **main table and all its old sub-tables are rebuilt** (the NG story of every rebuild: create, indexes, copy, drop the indexes, rename); the new sub-table is **created empty in the same generation** (`create table ...NG`, its unique clustered index, no `INSERT`, no `drop`) and renamed with the others. Case h looked as if only the new table were created: `snapdiff` does not show the rebuild of a table whose key is not an auto-named one (`_S_HPK`); the XE trace of e1, e3, e4, e5 and e6 shows it | by column name for the old tables (the new column gets the default of its type); nothing for the new one | registry (a record of kind 39 (catalog) / 43 (document) after the subtree of the owner's last section, or of its last attribute, and a record of kind 14 for each attribute), `2203278d` (the tabular-section group refilled in the hash order of *all* its keys, however many sections are new), XDTO (the row type `...TabularSectionRow.<Owner>.<Section>` and the property of the section in the object type), `siVersions` | h, e1, e4, e5 | **built** (S1-E, 12.14). Not built, refused: a section removed, moved, renamed or put before the old ones, a length of the line number other than 5, a section with no attribute, a `Use` of a catalog's section other than `ForItem` (a new section, or an old one that gets an attribute), an attribute of a section that is removed, moved, retyped or re-indexed, the additional order in a section attribute, a stage that also removes an attribute |
| **add an attribute to a tabular section** (that was there) | the field inserted into the section's sub-table entry after the field of the attribute before it (the first one after the line number field); an indexed attribute adds `ByFieldFld<m>` among the section's other `ByField` entries in the order of the fields | one `Fld` per new attribute, after the object's own new attributes and before the numbers of the new sections (e3: 11034 ... 11039) | the object's **main table and all its sub-tables rebuilt** (e3: 4 objects, 9 tables), not "that sub-table alone" (case h) | by column name, the new column gets the default of its type (a section attribute is never nullable) | registry (kind 14 records), XDTO (a property line in the row type of the section), `siVersions`; **not** `2203278d` | h, e3, e4, e6 | **built** (S1-E, 12.14) |
| **add an object** (plain catalog or document) | a new table entry before `ConfigChngR` (c: `_Reference11036` with its four indexes) | a `Reference` entry (c: 11036; no `ReferenceChngR` until an exchange plan lists the catalog; a `RefSInf` when the catalog has predefined data: refuse) numbered from the counter | a new table, empty | none | XDTO (three object types), registry, `2203278d`, `a07b62f0` and the rest of the derived rows; the `Config` rows of the object and its registration for the exchange-plan nodes (the apply's own new-object registration) | c | designed. The largest: 12.5 |

Independent of the operation: the platform rebuilds the derived state it always rebuilds (`_ConfigChngR`, `.ui`, the
garbage collection of `ConfigCAS` and `Files`, `_ExtensionsRestruct*`); an own apply does not, and the twin comparison
tolerates exactly that list (12.6).

### 12.4 One transaction and the seam

#### 12.4.1 The order inside the apply's script

```
SET XACT_ABORT ON; SET LOCK_TIMEOUT 30000; USE [db]; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE
BEGIN TRY  BEGIN TRANSACTION
  application lock; TABLOCKX on Config, ConfigSave, Params, Files (and _ConfigChngR)
  exclusive access (no other user session), no unfinished-operation markers, SchemaStorage Status 100
  fingerprints of ConfigSave, the Config rows to replace, the dynamic-update rows, the Params marker
  DECLARE @offset, @now
  -- structure phase (new)                                              <- StructurePhase.sql
       guards: SchemaStorage idle and the planned CurrentSchema hash; DBNames row hash; no *NG table left
       create every <table>NG; copy (INSERT ... SELECT by column name); row counts equal
       indexes; the columns and indexes of every NG table are the model's (THROW 57404)
       drop the old tables; sp_rename tables and indexes; the same assertions on the final names
       SchemaStorage (Status 100, new CurrentSchema, empty generations), DBSchema, DBNames, DBNamesVersion
  fold of the dynamic generations; delete the DynamicallyUpdated markers
  move the staged rows into Config; ConfigChngR reset and new registrations
  Files.MobileVersions.dat; Params rewrites (the apply's search info and StructurePhase.params_rewrites: the caches)
  postconditions; DELETE FROM ConfigSave
COMMIT   (or ROLLBACK for a rehearsal)   END TRY   BEGIN CATCH  ROLLBACK; THROW  END CATCH
```

The structure phase comes before the move only because its work is the longest and its assertions the most likely to fail:
nothing in it reads `Config`, and nothing after it reads the rebuilt tables. **Measured** (12.8): the rebuild of eleven
tables (a catalog of 716 rows among them) with its `Sch-M` locks, `INSERT ... SELECT`, `DROP TABLE`, `sp_rename` and the
publication runs inside the apply's SERIALIZABLE transaction, together with the fold and the move; the rehearsal (the
whole 9.3 MB script, rolled back) left the database as it was.

#### 12.4.2 The seam (a proposal; the apply track owns `mssql_config_apply`, the patch is a spike on this branch)

The seam is the apply track's (S1-A, #397): `StructurePhase`, `StructuralGate::take_structure`, `ScriptInputs.structure_sql`,
the merge of the cache rewrites (a clash on the same row is a refusal), `--allow-restructure s1`, the backup policy
(`BackupPolicy`: `--recovery-backup <path>` takes a COPY_ONLY backup first, `--i-have-a-backup` acknowledges one, neither
refuses) and the apply's own handling of a consumed `deleted` row (`consumed_names`, `consumed_row_count`: the row is not
moved into `Config`, is not an unfinished-operation marker, and goes with the rest of `ConfigSave`). What the S1 work adds
on top of it is small and additive (the port of wave 1, S1-B):

| file | change |
|---|---|
| `gate.rs` | `StructuralGate::judges_deleted_row(&self) -> bool`, default `false`: the stage's `deleted` row lists the removed attributes (ids, flag 1); the apply consumes an empty list and a list of the rows of a dynamic update itself and refuses every other, unless the gate says it judges the list. `StructurePhase::consumed_staged_rows`: the staged rows the phase answers for |
| `mod.rs` | a `deleted` list the gate judges is consumed like an empty one -- but only when, after the gate, the phase answers for it (`consumed_staged_rows > 0`); otherwise the apply refuses it after all. `restructure_gate` builds `restructure::s1::S1Gate` for `--allow-restructure s1` |
| `restructure/s1.rs` | `S1Gate`: conservative verdict -> `check_staged` (the reasons that name a consumed row are dropped, as the apply's own check gate does) -> `apply_check::s1::classify` -> plan -> `decide`; `take_structure`; `judges_deleted_row` is `true`. `AddAttribute`, `DeleteAttribute`, `WidenString`, `SwitchIndex`, `AddTabularSection` and `AddSectionAttribute` are built; `AddObject` is refused as designed but not built |
| `restructure/script.rs` | `Plan::phase_sql(now)`: the plan as T-SQL with `THROW` assertions (57400..57405) |
| `restructure/reader.rs` | `RowSource`: the plan's input read through the apply's client (`ClientSource`) as well as through a dedicated connection |
| CLI | `ibcmd-rs mssql-config-apply --allow-restructure s1 (--recovery-backup <path> \| --i-have-a-backup) [--dry-run \| --rehearse]`; `mssql-restructure --through-apply` is the same run driven from the research command (it takes the plan options of the kit: fixed `DBNamesVersion`, skipped caches) |

Direction of the dependencies: `restructure` uses `mssql_config_apply::{gate, sqlgen, model, si}`; the apply knows the
gate only as a trait object. The unit of change in the apply is the structure phase as an opaque, self-contained T-SQL
text with its own assertions, so the apply track reviews an interface, not the restructure.

#### 12.4.3 Why not the standalone order (restructure, then apply)

Two commits would leave a window with the new schema and the old `Config` (the database is then inconsistent for any
session and the apply's fingerprints are asserted against a database that changed under them). It also loses what the
apply's script does for the rows: **measured** on the types case, the standalone restructure (`mssql-restructure`,
which promotes `Config` with its own statements) left the `_dynupdate_` alias rows and the `DynamicallyUpdated` marker
that the БСП clone carries from an earlier dynamic update; the native apply folds them, the apply's script folds them,
the standalone promotion does not -- and the native export of the two twins then differs in `configVersion` of the six
restructured objects (12 197 other files identical). **Confirmed through the apply** (12.8): the same stage through the
apply's script gives a `Config` table identical to native's row by row, and a native export identical in all 12 198
files. The standalone command stays a research tool; the promotion belongs to the apply.

#### 12.4.4 Failure and recovery

Any failed assertion or error rolls the transaction back: the database is as it was, `SchemaStorage` never leaves
`Status 100`, no `*NG` table survives (they are created inside the transaction). SQL Server rolls a crashed session back
at recovery. **Measured** with a `THROW` injected as the last statement before `COMMIT` of the generated script (run with
`sqlcmd` on a fresh twin, `s1_t1_fail`): the structure phase, the fold and the move ran for 20 s and the `CATCH` block took
all of it back -- `ConfigSave` 9 rows, `Config` 9847, no `NG` table, no new column, `SchemaStorage` `Status 100` with the
hash it had, `DBNames` and the `DynamicallyUpdated` marker as they were. What a committed restructuring cannot give is its own undo: the old tables are dropped in the transaction.
The apply's recovery artifact keeps the `Config` rows and the cache rows; for the tables the answer is a SQL Server
backup taken before (`BACKUP DATABASE ... WITH COPY_ONLY`), which is also how the lab makes twins. **Decided** (the
coordinator, after Pavel): a structural apply **refuses unless** `--recovery-backup <path>` (the apply takes a COPY_ONLY
backup first; recommended) **or** `--i-have-a-backup` is given; a dry run and a rehearsal write nothing and need neither.
Track apply implemented it in S1-A (#397); the S1 gate does not look at it. The log of the whole copy is
in one transaction: fine for a catalog, not for 100 GB, so the gate refuses when the rebuilt tables exceed a limit
(S1-J) and points to the native apply.

#### 12.4.5 What the transaction does not include

The derived state of section 10.15 stays as it is for an own apply: `_ConfigChngR` is reset for the staged objects by the
apply (not rebuilt as native does), `.ui` rows, the help index in `Files`, the garbage collection of the CAS tables,
`_ExtensionsRestruct*`. A native `config apply` afterwards prints "Обновление конфигурации базы данных не требуется"
(12.8): the platform decides from `DBSchema`, and ours is the platform's.

### 12.5 The derived caches (W10): what to write, what may be left

A native apply rewrites all 16 `Params` `*.si` rows and gives each a new version in `siVersions`; the text of most of them
does not change. Compared after inflate, native against native before (the changes of section 9.6 and this phase):

| row | role (decoded so far) | attribute add / delete | widen, index flag | tabular section | new object |
|---|---|---|---|---|---|
| `1a621f0f` | object registry: pre-order records `uuid,parent,kind,"name",{1,1,{"ru","synonym"}},flag,flag`, the count in the header (kinds: 35 catalog, 36 catalog attribute, 41 document attribute, 39 tabular section, 14 its attribute) | +/- one record, count +/- 1 | none (widening: measured, c1; the index flag alone: d0) | +records | +records |
| `ea13a2c9` | XDTO model of the configuration: `{2,1,{{#base64:<XML>}}}`, 64-character lines, CR CR LF | +/- one `<property>` line in `CatalogObject.X` / `DocumentObject.X` | none (inferred) | + a row object type and the section's property | + three object / value types |
| `2203278d` | per-class index of object ids `{29,<class>,<count>,<ids...>` | none | none | the tabular-section group | count + 1 and the id |
| `a07b62f0` | names -> tables list `{1089,"Constant.X","Константа.X",<uuid>,1,0,"Const3892",3892,...` | none | none | none | entry, count + 1 |
| `42ed49cc` | a second per-class id list `{114,...` | none | none | none | count + 1, the id |
| `c4629235` | per-object descriptors with help references `v8config://v8cfgHelp/mdobject/id...` | none | none | none | the object's records |
| `facbfffe` | synonyms by object uuid | none | none | none | one entry |
| `fe8acd6a` | a list of `{"#",<uuid>}` items | none | none | none | three lines |
| `c77bc206` | a list of uuids with zeros (3 lines); the same text after 1 or 3 attributes | none | none | ? | ? |
| the other 7 | unchanged in every case measured | none | none | none | none |

For the types case the native apply changed the text of **2 of 16** rows (registry, XDTO); the other 14 were rewritten
with the same text. For the new catalog of case c it changed 8 (`si_diff_pristine_vs_c2.txt`).

What the platform needs of each row when a **new catalog** exists, measured on the native state of case c
(`ibcmd_rs_04_ddl_s1_cache`: one row at a time deleted, then one at a time put back in its state from before the object; a
stand-alone `ibsrv` on the database and a thin client running the probe `probe/jobs/newcat.bsl`: metadata, a query, a
write, the type of the reference, XDTO of the reference and of the object, a query by the reference; logs
`logs/cache_bisect.log`, `logs/cache_bisect2.log`, `out/cache_*.txt`):

| row | absent | stale (without the new object) |
|---|---|---|
| `1a621f0f` object registry | **the server exits at start** | the client fails: "Тип не определен" |
| `a07b62f0` names -> tables | **the server exits at start** | the client **hangs** (348 s, no result) |
| `2203278d` per-class index | the client fails: "Тип не определен" | the client fails: "Тип не определен" |
| `ea13a2c9` XDTO model | rebuilt in memory, everything works, the row is not written back | XDTO serialization of the object fails: "Несоответствие типов" (queries and the reference's XDTO work) |
| `42ed49cc`, `c4629235`, `facbfffe`, `fe8acd6a` | tolerated in the probe | tolerated in the probe |
| `c77bc206` and the rest | not probed (unchanged by the attribute operations; the text of `c77bc206` is the same in every case) | |

**Decision.**

1. **Attribute operations** (add, delete; the rest change nothing in the caches): write the XDTO model row and the object
   registry row, and give both new versions in `siVersions`. Built and equal to native after inflate for 23 attributes of
   six objects and for a2 (12.8). The XDTO row is required (a stale one breaks the serialization of the changed object);
   the registry row is not required by any probe for an attribute, but it is what the platform rewrites, and once the
   placement rule (after the last attribute of the owner, before its first tabular section; metadata order for several)
   is coded it costs nothing.
2. **Removing a stale row is not a route** for the registry, the names -> tables list or the per-class index: with the
   first two absent the server does not start. For the XDTO model alone it works (the model is rebuilt in memory at every
   start and never written back: 25 MB of XML per session start), which makes it an emergency switch (`--skip-xdto` leaves the row
   stale; an operator deleting it is a stop-gap, not the design).
3. **A new object** (S1-F, S1-G) needs the registry, `a07b62f0` and `2203278d` written, and the XDTO model written or removed.
   The four tolerated rows can be left stale in a first cut and are written when their placement rules are decoded (S1-G).
4. `siVersions` is plain text `{0,16,"<row>.si",<guid>,...}`: only the rewritten rows get a new guid (the platform gives
   all sixteen new ones every apply; a server that keeps its cache across the restructure is 0.5, not tested).

The rewrites go through the apply's guarded rewrite (`ParamsRewrite`: the row's size and SHA-256 asserted under the lock,
parts beyond the first deleted, `Modified` and, for `*.si`, `Creation` set to the apply's `@now`).

### 12.6 The acceptance of a case: the twin against native

One protocol for every case of S1 (a2, b, f, g, h, c and the types case are the first uses). Setup: the staged image once
(native `import files --partial` of the edited tree for the lab; our own import once it stages the change), a
`BACKUP DATABASE ... WITH COPY_ONLY`, two twins from it with `restore-clone.ps1 -Corpus bak`; native
`apply_only.ps1 -Database <nat>` (under the `native` lock, optionally traced) on one; ours on the other
(`mssql-restructure --through-apply`, later `mssql-config-apply`).

| # | check | tool (`scripts/restructure-lab`) | expected |
|---|---|---|---|
| 1 | the plan made offline from the staged snapshot equals the native result | a test in `tests_corpus.rs` per case | `DBNames` text equal; every `DBSchema` entry equal but the drift list; the cache rows equal after inflate |
| 2 | tables, columns, indexes | `snapshot.py`, `snapdiff.py` | identical but the drift list |
| 3 | the data of every rebuilt table | `compare_tables.ps1` | `EXCEPT` both ways: 0 rows (every column but the row version) |
| 4 | the `Config` rows, `Creation` / `Modified` included | `compare_config.ps1` | equal |
| 5 | `DBSchema` entries and `DBNames` text of the databases | `dbschema_cmp.py` | equal but `DbCopies`, `DbCopiesUpdates` |
| 6 | the 16 `.si` rows | `si_diff.py` | "16 of 16 have the same text" |
| 7 | a native `config apply` on our twin | `apply_only.ps1` | «Обновление конфигурации базы данных не требуется» |
| 8 | native `config export` of both | `export_tree.ps1`, `ibcmd-rs source-diff` | no differing file |
| 9 | a session in the cluster on both | `register-ib.ps1`, `session_job.ps1 -Database <db> -Job <case>.bsl`, `unregister` | identical output (read, defaults, XDTO, write, query) |
| 10 | a rehearsal changes nothing | `--rehearse`, then `snapshot.py` | no difference |
| 11 | the refusals of the case | tests of `decide` + a dry run on a stage with one change more than S1 | refused with the reason, nothing written |
| 12 | a failure inside the transaction takes everything back (once per operation kind that adds a statement) | a `THROW` injected before `COMMIT` of the `--script-output` script, run with `sqlcmd` on a fresh twin | the database as it was: row counts, `SchemaStorage` hash, no `NG` table, no new column |

The drift list -- differences that are the platform's own derived state, tolerated in every case: `_DbCopies` and
`_DbCopiesUpdates` (the platform upgrades those system tables on its own; new ones appear in another build: case c), the
auto-named primary keys of rebuilt tables (`PK___...`, random), `_ConfigChngR` and `_ConfigChngR_ExtProps` (rebuilt with
new keys by native), `_ExtensionsRestructNGS` (three scratch rows natively), `Params` `.ui` rows and the guids of
`siVersions`, `ecsreg_*`, the garbage collection of `ConfigCAS` and `Files` (`userDocs_ru*`), `DBNamesVersion`'s guid.

### 12.7 The split into sub-issues

Waves: **0** = this phase (the framework and adding attributes); **1** = the seam, and the operations that need only the
machinery of wave 0; **2** = the operations that need the wave-1 pieces; **3** = end to end. Sizes: S ~ a day, M ~ two
to three days, L ~ a week of an agent.

| id | title | scope | depends on | acceptance (per case, 12.6) | size, owner |
|---|---|---|---|---|---|
| **S1-A** | The seam in the apply | `take_structure`, `structure_sql`, the merge of `params_rewrites`, the report, `--allow-restructure s1` in `mssql-config-apply`, the backup policy of 12.4.4; the patch of 12.4.2 is the starting point | wave 0 | script tests (the phase between the assertions and the move; a clash refused); the types case through `mssql-config-apply` equals native in checks 1-10, `Config` and the export included | S, apply |
| **S1-B** | Delete an attribute | the reverse of adding: field and its indexes out of the entry, XDTO line and registry record out, siVersions | S1-A | case f (indexed attribute, last / middle / only attribute of the object, a catalog and a document, several deletions in one stage, delete + add in one stage) | M, ddl |
| **S1-C** | Widen a string | `Length a -> b` of a variable string, `b > a`; fixed / unlimited / narrowing refused | S1-A | case e / g without the index flag; catalog and document; an indexed attribute; the boundary `b` = 1024, 4000 | S, ddl |
| **S1-D** | Switch the index | `Indexing` `DontIndex` <-> `Index` (and the additional-order value); first trace of the flag alone, then the plan | S1-A | case g alone; string and number attribute; catalog with and without hierarchy, document; on and off | M, ddl |
| **S1-E** | Add a tabular section | a new sub-table (created empty, the object's other tables rebuilt with it); then the new attributes of a section that was there (the object rebuilt) | S1-A, the section rows of S1-G | cases e1, e3, e4 (12.14): catalogs (flat, hierarchical, subordinate) and documents; nested numbering | M, ddl. **Done (12.14)** |
| **S1-F** | Add a plain catalog or document | tables, `DBNames`, `DBSchema` before `ConfigChngR`, the object's `Config` rows and their registration for exchange-plan nodes, the pair of reasons | S1-A, S1-G | case c (catalog), a document; a new object with attributes; a new object and an attribute of an old one in one stage | L, ddl |
| **S1-G** | The derived caches of a new object (W10) | decode and write `2203278d`, `a07b62f0`, `42ed49cc`, `c4629235`, `facbfffe`, `fe8acd6a`, `c77bc206`; the XDTO types of a new object; a tabular section's rows | none (decoders); S1-E / S1-F use it | each row equal to native's after inflate for cases c, h and the types case; the cache-necessity table of 12.5 repeated on the result | M-L, ddl or a second agent |
| **S1-H** | The classification of a staged image (W11) and the refusal matrix | the noise of a whole native image (396 "same XML" rows, the `{68}` Configuration row, `deleted`); a corpus test: every rcheck probe case (`p1`-`p12`, `restructuring-check.md`) is expected S1 or refused with a named reason | S1-A | the a2 whole image is accepted only with a proof of the noise, or refused with the reasons; the matrix passes; no case of the rcheck corpus that is not S1 is let through | M, rcheck (+ ddl) |
| **S1-I** | Extensions | `DBNames-Ext-*` numbering (done), `SchemaStorage(1)` and the `X1` tables, `_ExtensionsRestruct*`; refuse when an extension adopts a changed object, or prove it harmless | wave 0 | БСП twin with 4 extensions: an object adopted by none passes; one adopted by an extension is refused; the extension's tables and the platform's later native apply are unchanged | M, ext |
| **S1-J** | Size guard, chunked copy | a limit on the rows / bytes of the rebuilt tables, the refusal that points to the native apply; the chunked copy is 0.5 | wave 0 | a synthetic table above the limit is refused; the limit is measured (log growth, time) | S, ddl |
| **S1-K** | End to end with our import | tree edit -> our `infobase config import` -> `mssql-config-apply` -> compare with native import + native apply; every operation; cluster session; ERP УХ with the coordinator's OK | S1-A, and the import track's fix of the silently dropped attribute (10.1) | the twin protocol with our import as the stager, БСП 8.3.27; then УХ | L, import + ddl |

Where an operation plugs in (all in `src/restructure/`): `plan.rs::find_changes` (detects new, removed and retyped
attributes; it refuses re-indexed ones), `check_object` (what else may differ), `plan_object` (fields -> the new entry -> the
tables -> the copy), `xdto_update` / `registry_update` (the caches), `s1.rs::decide` (a built operation is one more arm of the
match of `S1Operation` and one more set in the agreement of the two decoders; `apply_check::s1::classify` names it), `script.rs` (nothing to change unless a new kind of statement
appears). Each sub-issue starts with the corpus test of its case (`tests_corpus.rs`: the plan made offline from the staged
snapshot against the native result), which is the quick loop; the twin run of 12.6 closes it.

Not in S1 and not filed: types by reference and composite types (need the map from type ids to tables), predefined data, registers.

### 12.8 The first step: attributes of every primitive type on catalogs and documents

The prototype of section 9 planned one `String` attribute of one catalog. The plan (`plan.rs`) now takes any number of new
attributes of **boolean, string (variable, fixed, unlimited), number (integer, fractional, non-negative), date, date and
time, time, value storage and uuid**, in any number of catalogs and documents in one stage (a reference, a composite or a
defined type is refused: the map from type ids to tables does not exist).

**The types case (T1)**, `edit_cases_s1.py` on the pristine БСП 8.3.27 clone, staged by the native `import files --partial`
(9 rows: six descriptors, `root`, `version`, `versions`), 23 attributes:

| object | rows | new attributes |
|---|---|---|
| `Catalog._ДемоГруппыДоступаПартнеров` (hierarchy) | 7 | boolean |
| `Catalog._ДемоМестаХранения` | 5 | date, number |
| `Catalog._ДемоПартнеры` (hierarchy; 2 sub-tables) | 14 | variable string, time, number `ForFolder` (nullable: folders only) |
| `Catalog.КлючевыеОперации` | 716 | boolean; variable, fixed(20) and unlimited strings; integer, fractional and non-negative numbers; date; date and time |
| `Catalog.Удалить_ДемоОбщиеСведения` | 3 | string |
| `Document._ДемоЗаказПокупателя` (3 sub-tables) | 8 | number (between the old attributes), boolean, string, number, date, date and time, unlimited string |

What the native side taught (all in the code, pinned by tests):

* **Order**: the platform walks the objects in the order of the configuration's own lists (catalogs, then documents), not
  by table number or name, and numbers the fields in that order (`Fld11034` ... `Fld11056`).
* **Nullable** exactly for an attribute of a hierarchical catalog with folders whose `Use` is `ForItem` or `ForFolder`
  (`ForFolderAndItem`, flat catalogs and documents are `NOT NULL`); the default goes to the rows the attribute applies to
  (`CASE WHEN _Folder = 0x01 THEN <v> END` for items, `= 0x00` for folders), the others get `NULL`; the XDTO property has
  `lowerBound="0"` iff the field is nullable.
* Date, date and time and time are one column type, `datetime2(0)`; the default is `2001-01-01 00:00:00`.

Results against the native apply on twins of one staged backup (`s1_t1_nat`), for the two ways of running the plan: the
standalone command `mssql-restructure` (`s1_t1_own`; it promotes `Config` with its own statements) and **through the
apply** (`s1_t1_seam`; `mssql-restructure --through-apply`: the S1 gate, the structure phase inside the apply's script).
The check numbers are those of 12.6.

| check | standalone | through the apply |
|---|---|---|
| 1. the plan made offline from the staged snapshot | `DBNames` text **equal**; every `DBSchema` entry **equal** but `DbCopies`, `DbCopiesUpdates`; XDTO and registry rows **equal after inflate** (`corpus_plan_of_the_types_case_equals_the_native_result`; the same test for a2) | the same plan |
| run | trial 13.8 s and rolled back; apply 7.8 s in the transaction, 16 s with the planning: create 0.3 s (11 tables), copy 0.4 s, 40 indexes 1.9 s, drop 0.5 s, 51 renames 1.6 s, publication 0.6 s | dry run 13 s (gate 10 s: the check and the plan); **rehearsal** 46 s (the whole 9.3 MB script run and rolled back: `ConfigSave` 9, `Config` 9847, no `NG` table, `Status 100` afterwards); apply 30 s, of which 21 s in the transaction (the structure phase, the fold, the move, the resets, the caches) |
| 2. tables, columns, indexes | 2234 tables in both; identical but the auto-named primary keys of two rebuilt tables (and of `_ConfigChngR`, which native rebuilds) and the two upgraded system tables | the same |
| 3. data of the 11 rebuilt tables | `EXCEPT` both ways **0 rows** in all | **0 rows** in all |
| 4. `Config` | every row equal, `Creation` / `Modified` included, but the six rows of the dynamic history that native folds (12.4.3) | **0 rows on either side: the whole table is byte-identical to native's**, 9841 rows |
| 5. `DBSchema`, `DBNames` | 1761 entries, the two that differ are `DbCopies`, `DbCopiesUpdates`; `DBNames` text equal (348 389 bytes) | the same |
| 6. the `*.si` rows | **16 of 16 have the same text** after inflate (registry and XDTO are ours, the other 14 as before) | **16 of 16** |
| 7. native `config apply` afterwards | «Обновление конфигурации базы данных не требуется», exit 0 | «Обновление конфигурации базы данных не требуется», exit 0 |
| 8. native export of both, `source-diff` | 12 198 files: 12 197 identical; `ConfigDumpInfo.xml` differs in `configVersion` of the six restructured objects (12.4.3) | **12 198 of 12 198 identical**, `ConfigDumpInfo.xml` included |
| 9. cluster session, 93 lines of output | identical to native's: types, the values of the existing rows per attribute (`NULL` for the folders of a `ForItem` attribute, the default for items), XDTO serialization, write, read back and query by each attribute on all six objects | identical |

Check 11, the refusals, on the real database with `--through-apply --dry-run`: the same stage with the `root` row changed by
one byte is refused with **one** blocker, `root: the service row root changes` -- the six descriptors' blockers are
withdrawn because the plan answers for them, the one the gate does not cover stays; the whole native image of a2
(`s1_full`) is refused by the apply itself for its `deleted` row before the gate is asked; neither wrote anything. The unit
tests of `decide` (`tests_s1.rs`) cover the other refusals: a `data` / `unknown` reason, another kind, an operation not
built, the check and the plan disagreeing, a plan that refuses, blockers beyond the listed ones.

The only rows the through-the-apply twin still differs in are the drift list of 12.6: the two upgraded system tables, the
auto-named primary keys, `_ConfigChngR`, `_ExtensionsRestructNGS`, `.ui`, `DBNamesVersion` and `siVersions` guids, and the
garbage collection of `ConfigCAS` (12 797 rows against native's 636) and `Files` (353 against 44).

### 12.9 Open questions and risks

1. **Backup policy of a structural apply** (12.4.4): decided. A committed restructuring is taken back from a SQL Server
   backup only, so a structural apply refuses unless `--recovery-backup <path>` (COPY_ONLY, taken first) or
   `--i-have-a-backup` is given.
2. **The search-information row clash** (12.4.2): a stage that adds an attribute *and* a form to the same catalog rewrites
   `1a621f0f` twice; the gate refuses it (the apply's edit and ours are both insertions and commute, but the merge is not
   built). Split the stage, or build the chain in S1-A.
3. **Exchange plans**: the apply resets `_ConfigChngR` for the staged objects; native rebuilds the table with new keys and
   registers 782 more objects (10.15). Untested on a base with real exchange-plan nodes.
4. **Size** (S1-J): a rebuilt table is copied in one transaction. The log of a 100 GB table is not acceptable. Done: the
   stage is refused above a limit (10 million rows, 2 GiB to write), measured and proven on twins in
   `docs/apply/restructure-size-limit.md`; the chunked copy is 0.5.
5. **Types by reference, composite types, defined types**: need the map from type ids to tables (`Reference569`,
   `Enum2894`) built from the generated types of the rows, and the `RRef` / `_TYPE` / `_RRRef` columns (case h has them).
6. **Predefined data**: the platform rebuilds `RefSInf` with the catalog (14 of 75 catalogs of the БСП); refused.
7. **Extensions** (S1-I): `SchemaStorage(1)`, `X1` tables, `_ExtensionsRestruct*`; an extension that adopts the changed
   object is not looked at yet -- the gate must refuse or prove it harmless.
8. **8.5**: served, measured on the БСП (s185: `b1`, `c1`, `d1`, checks 3, 4, 5, 7, 8 equal to native). The storage differs
   from 8.3.27 in the `ALTER INDEX` lists (native only, not needed for the result), all sixteen `*.si` rows rewritten in
   another order (the same up to order after an own apply), 2.21 records, and the `root` row whose payload the platform
   re-stamps on every write (`apply_check::root_row`). ERP УХ 8.5 is not measured.
9. **ERP УХ** (big tables, predefined data everywhere): 8.3.27 measured on two cases with empty tables (s-uha: `b1`, `c1`,
   checks 2, 3, 4, 5, 7, 8 equal to native, `evidence/dropin-apply/s1-acceptance-uha.md`); the platform appends the rebuilt
   tables at the end of `DBSchema`; the plan first put them before `ConfigChngR`, which is equal only where that table is last, as
   on the БСП (the platform rebuilds it after the others), and not on the УХ, where the register is empty, the platform does not
   rebuild `ConfigChngR` and it stays in the middle of the list. Since the merge of 12.15 the plan places a rebuilt table at the
   end, ahead of `ConfigChngR` only when that is the last table (`DbSchema::insert_rebuilt`); the created tables of S1-F come
   first, in the same place. Proved by unit tests for both shapes and a fresh ERP UH clone on 2026-10-01:
   `uha_order_proof.ps1` stages rcheck's b1 files with native import, then runs our drop-in apply. All six rebuilt tables
   occupy the native twin's positions 16 851-16 856 in the same order, after ExtensionsInfoNGS, and every other table keeps
   its stored relative order. ConfigChngR stays in the middle (position 4 809 before, 4 803 after). Evidence:
   `s1-mix-release-20261001.txt`. Rebuilding UH tables with data and UH 8.5: not measured; the size guard decides most of it.
10. **The dynamic history**: the apply folds it; a restructure over an active dynamic update (`Status` not 100) is refused.
11. **A running server across the restructure** (0.5): the guids of `siVersions` of a cache row that changed.
12. **The alter method** (9.5): kept as a research switch of the direct command; it is not offered to the apply (the physical
    order then differs from native's, and the byte-level twin comparison stops working).
13. **`ByField` numbers**: an attribute created *with* the index flag gets an extra `DBNames` entry of kind `ByField`
    (`dbnames-kinds.txt`); switching the flag later does not (g, and d0, d1 on the БСП: `DBNames` did not change). The
    planner builds new indexed attributes, including their extra `ByField` entries: case m7 of 12.15 covers seven of them
    with both import routes and an offline comparison against the native result.

### 12.10 Evidence

Repo (`docs/apply/evidence/restructuring/`, this phase): `s1-reasons.txt` (what `mssql-apply-check` prints for the cases: the
types case from the ConfigSave, the other cases from `--tree`, and the whole native image), `s1-t1-twin-compare.txt` (the
checks of 12.6 on the types case, with the numbers of 12.8), `s1-t1-session.txt` (the job output of the cluster session, native and
ours), `s1-cache-necessity.txt` (the cache experiment of 12.5), `s1-seam-script.sql` (the generated transaction with the
binary values shortened), and for wave 1 (12.11) `s2-wave1-twin-compare.txt` (the checks of 12.6 for b1, b2, c1) and
`s2-b1-session.txt`, `s2-b2-session.txt`, `s2-c1-session.txt` (the cluster session outputs, native's and ours are equal), for S1-D (12.13) `s2-wave2-d-twin-compare.txt`, `s2-d0-session.txt`, `s2-d1-session.txt`, for S1-E (12.14) `s2-wave2-e-twin-compare.txt` (e4, e5, e6), `s2-e1-e3-twin-compare.txt`, `s2-e1-e3-extension-refusals.txt`, `s2-e{1,3,4,5,6}-session.txt`, `s2-e4-dropin-route.txt`, the native statements `e{1,3,4,5,6}-structure-statements.sql` and what the native apply changed in the entries (`e{1,3,4,5,6}-entries-diff.txt`). `s1-port-acceptance.txt` (12.12: the four cases through `mssql-config-apply --allow-restructure s1`). Lab (`F:\ibcmd\lab\04\restructure`): `out/diff_s1_t1_native.txt`, `out/diff_s1_t1_nat_vs_own.txt`,
`out/except_*`, `out/si_diff_*`, `out/dbschema_cmp_*`, `out/config_cmp_*`, `out/export_diff_*`, `out/session_t1_*`, `logs/cache_bisect*.log`,
`xe/s1_t1/`, `snap/ibcmd_rs_04_ddl_s1_*`. Tools: `scripts/restructure-lab/` (`compare_tables.ps1`, `compare_config.ps1`,
`dbschema_cmp.py`, `params_row.ps1`, `cache_variant.ps1`, `edit_cases_s1.py`, `jobs/types_t1.bsl`). Tests: `tests_s1.rs` (the gate),
`tests_plan.rs` (`the_phase_text_is_the_statements_with_assertions`), `tests_corpus.rs` (the types case).

For the combination matrix (12.15): `s1-mix-matrix.txt` (m1-m7, f1, f3 and the eight refusals, both import routes),
`s1-mix-check12-m6.txt` (rollback of the largest stage), and `s1-mix-release-20261001.txt` (the final quick gates,
the completed f1 and r7 runs, and the ERP UH DBSchema order proof). The matrix records historical measurements;
the release evidence distinguishes the fresh merged-code runs from those earlier results.

### 12.11 Wave 1: delete an attribute (S1-B #398) and widen a string (S1-C #399)

Cases on the **pristine** БСП 8.3.27 (every row is record version 56; the restructuring check cannot yet read a row a native
import has rewritten as version 57 -- S1-H), made by `edit_cases_s2.py` and staged by the native `import files --partial`
(`stage_case.ps1`); twins from one COPY_ONLY backup; ours through the apply (`mssql-restructure --through-apply
--i-have-a-backup`). The checks are those of 12.6; the numbers are in `s2-wave1-twin-compare.txt`.

| case | stage | native | ours |
|---|---|---|---|
| **b1** | 11 attributes deleted in 6 objects (the middle, the last, the first, indexed ones of a flat catalog, a hierarchical catalog and a document; the only attribute of an object; a nullable field of a hierarchical catalog; an unlimited string) and 2 replaced (deleted and added in one stage) | 8 tables rebuilt, indexes renumbered, `DBNames` grows by the two new numbers only | apply 51 s (rehearsal 97 s with the planning) |
| **b2** | an additional-order attribute deleted from a catalog and from a document | the catalog loses `ByFieldFld179`; `ByDocDate` of the document **stays and loses the field** (`{4,...,"Fld4062"}` -> `{3,...}`), its own index goes; 4 tables | apply 4 s |
| **c1** | 6 variable strings widened in 5 objects (a catalog up to the limit of 1024; a hierarchical catalog, two attributes, one indexed; a document, one indexed and one by a single character) | 10 tables rebuilt (main tables and all sub-tables), **no cache row changes**, `DBNames` unchanged | apply 13 s (rehearsal 44 s) |

| check | b1 | b2 | c1 |
|---|---|---|---|
| 1. the plan offline from the staged snapshot equals native's | `corpus_plan_of_the_deletion_case_equals_the_native_result`: `DBNames` text, every `DBSchema` entry but `DbCopies*`, XDTO and registry rows after inflate | `corpus_plan_of_the_additional_order_deletion_case_...` | `corpus_plan_of_the_widening_case_...`: also that the plan writes no cache and that all 16 `.si` rows are the same text before and after native's apply |
| 2. tables, columns, indexes | identical but the drift list | the same | the same |
| 3. data of the rebuilt tables, `EXCEPT` both ways | 0 rows (8 tables) | 0 rows (4 tables) | 0 rows (10 tables) |
| 4. `Config`, whole table | **0 rows on either side** | the same | the same |
| 5. `DBSchema`, `DBNames` | 1761 entries, only `DbCopies*` differ; `DBNames` text equal | the same | the same |
| 6. the `.si` rows | 16 of 16 | 16 of 16 | 16 of 16 |
| 7. native `config apply` on our twin | «не требуется» | «не требуется» | «не требуется» |
| 8. native export of both | 12 198 of 12 198 identical | 12 198 of 12 198 | 12 198 of 12 198 |
| 9. cluster session | identical, 49 lines | identical | identical, 48 lines |
| 10. a rehearsal changes nothing | `snapdiff` empty | empty | empty |
| 11. refusals (dry run on a real stage that has one change more) | the `deleted` row names an id that is no removed attribute: `S1: the staged image deletes aaaaaaaa-..., which is not an attribute it removes` | (unit tests: an index of another kind that names the field) | a limit made shorter: `S1: attribute ИмяХеш changes its type (the limit is shorter)`; `root` changed: `the service row root changes` |
| 12. `THROW` before `COMMIT` on a fresh twin | digest of the whole database unchanged (`Config`, `Params`, schema and names hashes, every column and index, no `NG` table) | unchanged | unchanged |

What wave 1 taught:

* **The stage of a deletion carries a `deleted` row** (`<n>,"<attribute id>",1,...`, one id per removed attribute). The apply
  refused every such row before it asked the gate and its script refused it as an unfinished-operation marker. The seam
  grew `StructuralGate::judges_deleted_row` and `StructurePhase::consumed_staged_rows` (12.4.2); the gate accepts only ids of
  attributes the same stage removes, and the apply then consumes the row like an empty list (on the first, spike form of the
  seam the phase deleted the row itself under its content hash; the apply's own consumed-rows machinery replaced that).
* **Indexes are renumbered** after a deletion (`_Reference22_4`, ...): the physical names are `_<Table>_<ordinal>`; the model
  recomputes them from the entry, which is why the byte-level comparison works.
* **`ByDocDate` is not an attribute index**: it lists the additional-order attribute last, and the platform keeps it, one
  field shorter. Any other index that names the field is refused.
* **The base64 of the XDTO model row**: when its length is a multiple of 64 the platform writes the line separator after
  the last (full) line too (case b2 hit it: 25 173 135 bytes against our 25 173 132). Found by the offline comparison, fixed
  and pinned by a test.
* **Widening rebuilds the object** (main table and all sub-tables) although only a column type changes, and writes no cache.
* **Code that still refers to a deleted attribute**: the object module of `_ДемоНачислениеЗарплаты` uses `ПериодРегистрации`;
  reading that document as an object in a session hangs on native's result and on ours alike (queries work), so the b2
  session job reads the document by queries only (including an ordering by date, which uses `ByDocDate`). The gate cannot
  see code: deleting an attribute breaks the modules that use it, whichever route deleted it.
* The plan judges the *direction* of a limit change, the restructuring check names both directions the same way
  (`Length: a -> b`); the two decoders agree on which attribute, the plan refuses a shorter limit, a fixed or unlimited string.

### 12.12 The port onto feat/0.4: the apply's seam and rcheck's classification

Wave 1 was built on the spike form of the seam (12.4.2). `feat/0.4` has the apply track's seam (S1-A, #397) and rcheck's typed
classification (`apply_check::s1::classify`, #404); the S1 work was ported onto it (branch `feat/0.4-s1-port`):

* `src/mssql_config_apply` is the apply's version plus two additive pieces for the `deleted` row of a deletion
  (`StructuralGate::judges_deleted_row`, `StructurePhase::consumed_staged_rows`) and the wiring of `--allow-restructure s1`
  (`restructure_gate` builds `S1Gate`). The apply's own machinery consumes the row (`consumed_names`, `consumed_row_count`),
  so the structure phase no longer deletes it and the script's `@@ROWCOUNT` assertions need no change; the apply refuses the
  list after all when no phase answers for it.
* `s1.rs` maps `S1Operation` (`AddAttribute`, `DeleteAttribute`, `WidenString` built; `SwitchIndex`, `AddTabularSection`,
  `AddObject` refused as designed but not built) instead of the wording of the reasons, drops the reasons that name the
  consumed `deleted` row (as the apply's own check gate does) and has no backup logic of its own: the apply refuses a
  structural apply without `--recovery-backup` / `--i-have-a-backup` and takes the COPY_ONLY backup itself.
* Acceptance, `mssql-config-apply --allow-restructure s1 --i-have-a-backup` on twins of the staged backups against the native
  `config apply` (`s1-port-acceptance.txt`): the types case T1 (11 tables), b1 (8), c1 (10) and b2 (4). Checks 3-8 of 12.6 pass
  in all four: `EXCEPT` both ways 0 rows in every rebuilt table, `Config` 0 rows on either side, `DBSchema` equal but
  `DbCopies*`, `DBNames` text equal, 16 of 16 `.si` rows, a native `config apply` afterwards «не требуется», native export
  12 198 of 12 198 files identical. Without a backup flag the apply refuses with its own message; with `--recovery-backup` it
  takes the backup (1.5 s for the БСП clone) and applies.

### 12.13 S1-D: the index flag of an attribute (#400)

The trace of one flag alone (case d0: `ЦелевоеВремя` of `КлючевыеОперации`, a number of a flat catalog, `DontIndex` -> `Index`)
first, then the plan (`add_field_indexes` in `schema.rs`, `IndexSwitch` in `plan.rs`, `SwitchIndex` in the S1 gate). What the
platform does (`entries_diff.py` on the snapshots of the staged state and of the native result, the XE trace):

* it **rebuilds the object** (d0: `_Reference2598`, the create / index / copy / drop / rename story of every other rebuild); the
  columns are the same; the new index sits among the others in the order of the fields, so the physical names are renumbered
  (`_Reference2598_4` is now the new one);
* the `DBSchema` entry gets or loses the declared indexes of the attribute and nothing else; `DBNames` does not change (no
  `ByField` number for a switch); **no cache row changes** (all 16 `.si` rows have the same text after the native apply).

Case d1 (six objects, 12 switches: on -- numbers of two flat catalogs, a string of a hierarchical catalog, two attributes of
a document; with the additional order an attribute of the hierarchical catalog and one of the document; off -- indexed
attributes of a flat catalog, of a document and the additional order of a flat catalog and of a document): the index entries are what 12.3 says; `ByDocDate` of a document keeps its place and lists the attribute
last while the additional order is on (removed again when it goes off, the same code as b2).

The twin protocol (12.6) through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`, ours against the native apply
of the same stage (`s2-wave2-d-twin-compare.txt`, `s2-d0-session.txt`, `s2-d1-session.txt`):

| check | d0 | d1 |
|---|---|---|
| 1. the plan offline from the staged snapshot equals native's | `corpus_plan_of_the_index_flag_alone_equals_the_native_result` | `corpus_plan_of_the_index_flags_on_and_off_equals_the_native_result` (12 switches in 6 objects; no cache; the 16 `.si` rows unchanged natively) |
| 2. tables, columns, indexes | identical but the drift list | the same |
| 3. data of the rebuilt tables, `EXCEPT` both ways | 0 rows (1 table) | 0 rows (11 tables) |
| 4. `Config` | 0 rows on either side | the same |
| 5. `DBSchema`, `DBNames` | equal but `DbCopies*`; the `DBNames` text equal | the same |
| 6. the `.si` rows | 16 of 16 | 16 of 16 |
| 7. native `config apply` afterwards | «не требуется» | «не требуется» |
| 8. native export of both, `source-diff` | 12 198 of 12 198 identical | 12 198 of 12 198 |
| 9. cluster session (the indexing the metadata reports, every switched attribute read in the order of its own index, rows with a digest, XDTO, write and read back) | identical, 8 lines | identical, 60 lines |
| 10. a rehearsal changes nothing | `snapdiff` empty | empty |
| 11. refusals on a real stage | (unit tests) | `Index` -> `IndexWithAdditionalOrder` of a stored `Index`: `S1: index-mode-outside-s1` (the classification's); the `root` row changed: `the service row root changes` |
| 12. `THROW` before `COMMIT` on a fresh twin | -- | the digest of the whole database unchanged |

Not traced, therefore refused: `Index` <-> `IndexWithAdditionalOrder` (the classification refuses it; the plan refuses it too),
a second additional-order attribute of one document (the date index would list two), an index of an attribute of a subordinate
catalog (`ByOwnerField...`), a hierarchical catalog without its `ParentDescr` index, an additional order of a catalog with no
description. The gate takes `DontIndex` <-> `IndexWithAdditionalOrder` since `apply_check::s1` was widened by one variant for it
(a separate commit).

### 12.14 S1-E: a new tabular section and new attributes of the sections an object had (#401)

Five cases, made on the pristine БСП 8.3.27 by `edit_cases_s3.py` and traced natively first (`entries_diff.py` on the snapshots of
the staged state and of the native result, the XE trace, `si_diff.py`):

* **e1**: five new sections in one stage -- three catalogs (`_ДемоСтавкиНДС` flat, `_ДемоМестаХранения` hierarchical,
  `_ДемоКонтактныеЛицаПартнеров` subordinate to owners) and two documents; attributes of every primitive type, two of them indexed;
* **e3**: new attributes of sections that were there -- first, in the middle and last, indexed and not, in a catalog of each shape
  and a document (`Товары`: one attribute first, one indexed last);
* **e4**: a catalog and a document that each get a new own attribute, a new attribute of the section they had and new sections (the
  document two of them) at once: the numbering of one object with all three;
* **e5**, **e6**: the shapes of e1 and of e3 on objects **no extension adopts**. The clone has four extensions and S1-I (`restructuring-extensions.md`)
  refuses an object one of them adopts: `_ДемоМестаХранения` and `_ДемоСписаниеТоваров` of e1 and `_ДемоПартнеры` and `_ДемоОрганизации`
  of e3 are adopted, so the gate that has S1-I refuses those two stages (`S1: catalog _ДемоПартнеры is adopted by the extension
  _ДемоРасширение ...`, `s2-e1-e3-extension-refusals.txt`). e4 never touched an adopted object. e5: a flat catalog, a hierarchical one
  that has a section already, a subordinate one and two documents (one has a section already), five new sections; e6: a hierarchical
  catalog, a subordinate one whose section has indexed attributes already (a new indexed attribute first, another in the middle,
  a plain one last) and a document.

What the platform does (it corrects two claims of the first phase: the row "add a tabular section" of 12.3 said the object's main
table is not rebuilt, and case h's reading was that an attribute of a section rebuilds "that sub-table alone"; both were the blind
spot of `snapdiff`, which does not show a rebuild of a table whose primary key is not an auto-named one):

* **any structural change of the object rebuilds all its tables** -- the main table and every sub-table the object had -- in the one
  new generation; a new sub-table is created in the same generation, empty, and renamed with the rest (e4, the catalog: `create` of
  `_Reference21NG`, `_Reference21_VT960NG` and the new `_Reference21_VT11036NG`, the indexes of all three, the copy of the first two,
  the `drop` of the indexes and the old tables, the renames);
* `DBSchema`: the field goes into the section's sub-table entry (after the field of the attribute before it; the first attribute of a
  section after the line number field), a new section is a new sub-table entry appended to the object's list, with its own line
  number field `LineNo<n+1>` (`numeric(5,0)`), its fields, and the declared indexes of the indexed attributes, **not unique**, among
  the section's other attribute indexes **in the order of the fields** (e6, `_Reference14_VT67`: the new `Fld11034` first, then
  the two stored ones, then `Fld11035`);
* `DBNames`: the object's own new attributes first, then each section in the order of the descriptor: the new attributes of a
  section that was there, or -- for a new section -- `VT`, `LineNo` (the section's uuid) and the attributes. The objects come in the
  configuration's order, catalogs first, and the numbers go on from the shared counter;
* the caches: a new section changes three rows (`1a621f0f`, `2203278d`, `ea13a2c9`), a new attribute of an old section two (`1a621f0f`,
  `ea13a2c9`), and `siVersions`; the rows are made by `caches::change::rewrite` (S1-G), which now takes any number of new sections of
  a kind in one refill of `2203278d` (`derived-caches.md`, 3).

The plan (`src/restructure/`):

* `catalog.rs`: a section's facts have its synonyms, its `Use` and its `LineNumberLength` (absent from the older record version, where
  every section has 5), and the `Indexing` of its attributes;
* `plan.rs::check_object` refuses what is not built (12.3) with the reason; `plan_sections` numbers, edits the sub-table entries
  (`schema.rs`: `subtable_mut`, `add_subtable_field_index`, `subtable_entry`, `push_subtable`) and returns a `SectionPlan` for each
  section that is new or grew; `plan_object` rebuilds every stored table and makes the new ones with `TablePlan::created`;
  `section_cache_updates` asks `caches::change::rewrite`;
* `reader.rs` also reads the stored descriptors of the catalogs and documents the configuration lists (the caches traverse the
  sections of every object of the kind; a partial stage holds only the ones it changes; found on the first real run of e4);
* the gate (`s1.rs`) takes `AddTabularSection` and `AddSectionAttribute` (`apply_check::s1`, a separate commit) and needs the plan to
  name the same sections and the same `Section.Attribute` pairs; `command.rs` reports the sections and the created tables.

The twin protocol (12.6) through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`, ours against the native apply of the
same stage, with the binary of the branch after the merge of feat/0.4 (`s2-wave2-e-twin-compare.txt`, `s2-e4-session.txt`,
`s2-e5-session.txt`, `s2-e6-session.txt`, the native statements `e5-structure-statements.sql`, ...):

| check | e4 | e5 | e6 |
|---|---|---|---|
| 1. the plan offline from the staged snapshot equals native's | `corpus_plan_of_new_sections_in_a_catalog_and_a_document_equals_the_native_result`: the `DBNames` text, every `DBSchema` entry, three cache rows; `case_e4_...every_row_is_native`: all sixteen rows | `corpus_plan_of_new_sections_on_objects_no_extension_adopts_...` | `corpus_plan_of_section_attributes_on_objects_no_extension_adopts_...` |
| 2. tables, columns, indexes | identical but the drift list | the same | the same |
| 3. data of the rebuilt tables, `EXCEPT` both ways | 0 rows (7 tables) | 0 rows (13 tables) | 0 rows (6 tables) |
| 4. `Config` | 0 rows on either side | the same | the same |
| 5. `DBSchema`, `DBNames` | equal but `DbCopies*`; the `DBNames` text equal | the same | the same |
| 6. the `.si` rows | 16 of 16 | 16 of 16 | 16 of 16 |
| 7. native `config apply` afterwards | «не требуется» | «не требуется» | «не требуется» |
| 8. native export of both, `source-diff` | 12 198 of 12 198 identical | the same | the same |
| 9. cluster session (the sections and their attributes with the indexing, the rows of each section with a digest, the XDTO serialization of a new object with a row in every section, write, read back, query through each section, the indexed attributes read in the order of their index) | identical, 80 lines | identical, 90 lines | identical, 71 lines |
| 10. a rehearsal changes nothing | `snapdiff` empty | empty | empty |
| 11. refusals on a real stage (a fresh twin of e4, `refusals_e.ps1`) | a stored section renamed: `S1: tabular-section-outside-s1` (its generated types change); a composite type in the attribute of a new section: `S1: attribute ДемоДата of tabular section ДемоТЧ2: a composite type (2 items) is not mapped to fields yet`; the precision of a stored section's attribute changed: `S1: tabular-section-outside-s1 ... Attribute[КоличествоДней]/Properties/Type/NumberQualifiers/Digits: 3 -> 4` | -- | -- |
| 12. `THROW` before `COMMIT` on a fresh twin | the digest of the whole database (rows of `Config`, `ConfigSave`, `Params`, the schema storage, `DBSchema`, `DBNames`, the checksums of all columns and indexes, 2 234 tables, no `*NG`) unchanged | -- | -- |

Case e5 was run once more with the binary of the last commit of the branch (feat/0.4 at 5e146f00 merged, `s2-e5-final-binary-run.txt`):
the same result in every check (16 of 16 rows, `DBNames` text equal, «не требуется», 12 198 of 12 198 files, the session identical, 90 lines).

Cases e1 and e3 ran the same twelve checks first, with the binary of commit `ad58c54d` (before S1-I was merged; `s2-e1-e3-twin-compare.txt`):
e1 12 rebuilt tables, session 90 lines, e3 9 tables, session 69 lines, all checks equal; their plans equal native's in
the offline tests of the merged code as well.

The drop-in route once (`s2-e4-dropin-route.txt`, `dropin_run.ps1`): `ibcmd-rs infobase config apply` on a fresh twin of e4 with the
merged binary -- without a backup option it is refused (exit 1, `BackupRequired`, the number of staged rows unchanged), with
`--recovery-backup=<file>` it exits 0 (gate `s1`, a 269 MB backup), and the checks 2-8 come out as above: the data of the seven
rebuilt tables equal, `Config` 0 rows, `DBSchema` and `DBNames` equal, 16 of 16 `.si` rows, «не требуется» afterwards, 12 198 of
12 198 files of the native exports identical.

The other refusals of the plan have unit tests on the fixtures of case a2 (`tests_sections.rs`): a section removed, moved, put before
the old ones, renamed; an attribute of an old section removed, moved, retyped or switched off; a new section with a reference type,
with no attribute, with a line number of 6, with an attribute the stored image has, with the additional order in an attribute; a stage
that removes an attribute and adds a section *in the same object* (in different objects of one stage it is built, 12.15); a stored sub-table the schema does not have. The owner field of a subordinate catalog
(`_ДемоКонтактныеЛицаПартнеров`) is not covered for *attributes* of the object, so it stays a refusal for them; a stage that only adds
sections or section attributes to it is let through (e1, e5, e6). A stage that creates an object (S1-F) and also adds sections to existing
ones is refused: both rewrite the same cache rows.

Not traced, therefore refused: the removal of a section or of an attribute of one (the caches of a removal are not composed with an
addition), a section in the middle of the list (the order of the sub-tables of the entry is not known), `Use` of a section other than
`ForItem`, the additional order in an attribute of a section, two new objects of a kind in one stage (S1-F).

### 12.15 S1 combinations: the built operations in one stage (#391, before 0.4.0)

Users do not stage one operation at a time. The operations of S1 were each traced and verified alone (12.8 A add an attribute, 12.11 B delete one
and C widen a string, 12.13 D switch the index, 12.14 E add a tabular section or an attribute of one); this section is the matrix of their
combinations, run through the drop-in route users run, `ibcmd-rs infobase config apply --recovery-backup=<file>` (gate `s1`), on objects no
extension adopts (S1-I). With S1-F merged (`docs/apply/new-object.md`) the matrix has its rows too: a new catalog or document next to other operations.
The stages are made by `edit_cases_s4.py <out> <case>` (m*, r1-r6) and `edit_cases_s5.py <out> <case>` (f*, r7, r8) on the БСП 8.3.27 and are described in
the table below.

**What the combinations needed** (`plan.rs`, `schema.rs`; the first run of the matrix found the first two, rcheck's run on the ERP УХ the third):

* **One refusal was wider than what was not traced.** `plan()` refused any stage that removes an attribute *and* adds a section or an attribute
  of one, because the object registry (`1a621f0f`) and the XDTO model (`ea13a2c9`) have two writers -- `caches::change::rewrite` for the objects
  with sections and the removal/addition editors (`xdto_update`, `registry_update`) for the objects that only change their attribute list --
  and the two were never run over the same rows. Native rewrites each row once for the whole stage, so the plan now does the same: the objects
  with sections go through `rewrite` first, then the editors run on the rows that are left (`RowText`, `edited()`, `put_row()`), and each row
  is deflated once (`CacheUpdate`). The refusal stays for what was never traced, **one object that removes an attribute and adds a section (or
  an attribute of one)**: `S1: catalog <name> removes an attribute and adds a tabular section (or attributes of one) in one stage: not built`.
  A stage that creates an object (S1-F) *and* adds sections to existing objects, or removes their attributes, stays refused (r7, r8, below).
* **A new attribute that comes with the index flag** (`Index`, `IndexWithAdditionalOrder`) was refused as "an index is a separate case". It
  is not: the entries are those of a switch of the index (12.13), added to the entry after the fields are inserted (`add_field_indexes`:
  `ByFieldFld<n>`, and `ByParentFieldFld<n>` for a hierarchical catalog, the additional order with the key on `Description,ID,Marked` /
  `Date_Time,ID,Marked`). Native traced it on m7 (all seven attributes) and the plan equals it: `DBNames`, every `DBSchema` entry, both cache rows.

* **The place of the rebuilt tables in `DBSchema`.** The platform appends every table it rebuilds to the **end** of the list, in the order it rebuilds
  them; the plan put them before `ConfigChngR`, which is the same only where that table is the last one (the БСП: the platform rebuilds it after
  the others). On the ERP УХ (empty change register) the platform does not rebuild it, it stays in the middle, and rcheck's twins showed the six
  rebuilt tables of `b1` at positions 16 851-16 856 natively and 4 803-4 808 in ours, every entry equal. `DbSchema::insert_rebuilt` puts a table at
  the end and ahead of `ConfigChngR` only when that is the last table; the tables S1-F creates come first, in the same place, so the placements
  compose. Nothing changes on the БСП (every corpus test, every twin of this section).

Nothing else changed in the planner, the gate or the seam: a stage with several operations on several objects was already one plan, one
rebuild of every table of every changed object (12.3), one transaction.

**The stages.** Operations: A add an attribute, B delete one, C widen a string, D switch the index, E+ a new tabular section, E~ a new attribute of
a section the object had. Every stage is checked first offline: the plan made from the staged snapshot equals the platform's result in `DBNames`
(text), every `DBSchema` entry, every cache row the plan writes, and the `.si` rows the plan does not write are the ones the platform left alone
(`tests_corpus.rs`, `corpus_mix`).

| case | the stage | objects | A | B | C | D | E+ | E~ | tables rebuilt or created |
|---|---|---|---|---|---|---|---|---|---|
| **m1** | **B + C** in the same objects: `КлассификаторБанков` (hierarchical) 2 deleted and 3 widened, `ШаблоныЗаданийОчереди` 1 and 2, `КлючевыеОперации` 1 and 1 | 3 | 0 | 4 | 6 | 0 | 0 | 0 | 3 |
| **m2** | **C + D** on the same attribute (widened and its index switched on, off, to the additional order): `КлассификаторБанков` 3 attributes, `_ДемоЗаказПокупателя` 2, `КлючевыеОперации` 1 | 3 | 0 | 0 | 6 | 6 | 0 | 0 | 6 |
| **m3** | **E + C + A** in the same object: `КлючевыеОперации` a new section and a widened attribute; `_ДемоЗаказПокупателя` a new section, a new attribute of the old section, a widened attribute and a new own attribute; `ТелефонныйЗвонок` a new section and a new own attribute | 3 | 2 | 0 | 2 | 0 | 3 | 1 | 11 |
| **m4** | **B in some objects, E in others**: `КлассификаторБанков` 2 and `ШаблоныЗаданийОчереди` 1 attributes deleted; `КлючевыеОперации` and `_ДемоСчетФактураПолученный` a new section each, `_ДемоОприходованиеТоваров` two new attributes of its section | 5 | 0 | 3 | 0 | 0 | 2 | 2 | 8 |
| **m5** | **A, B, C, D and E at once** on six objects: `КлассификаторБанков` B+C+D, `ШаблоныЗаданийОчереди` B+A, `КлючевыеОперации` C+D+E, `_ДемоЗаказПокупателя` A+D+E+E~, `ТомаХраненияФайлов` D, `_ДемоПодразделения` (hierarchical) A+E~ | 6 | 3 | 2 | 2 | 4 | 2 | 2 | 12 |
| **m6** | **one large stage**: the six objects of m5 and eleven more (catalogs and documents, flat, hierarchical and subordinate, with and without sections; every operation at least four times) | 17 | 4 | 4 | 6 | 8 | 4 | 4 | 30 |
| **m7** | **A + D** on the same new attribute: seven new attributes that come indexed (`Index`, `IndexWithAdditionalOrder`, a string, a number, a date) in `КлассификаторБанков` (hierarchical, nullable ones too), `КлючевыеОперации`, `ТомаХраненияФайлов`, `ШаблоныЗаданийОчереди` and `_ДемоЗаказПокупателя` | 5 | 7 | 0 | 0 | 0 | 0 | 0 | 8 |
| **f1** | **F + C + D**: a new flat catalog `ДемоКатН1` (S1-F, case N1: attributes of every primitive type, a reference, indexed ones) next to two widened strings (`КлючевыеОперации`, `_ДемоСписаниеБезналичныхДенежныхСредств`) and three switched indexes (`КлассификаторБанков`, `ТомаХраненияФайлов`, the same document) | 5 | 0 | 0 | 2 | 3 | 0 | 0 | 5 |
| **f3** | **F + A + C + D**: a new catalog `ДемоКатН5` and a new document `ДемоДокН5` (N5) next to two new own attributes (`ШаблоныЗаданийОчереди`, `_ДемоРасходныйКассовыйОрдер`), a widened string (`КлючевыеОперации`) and two switched indexes (`ТомаХраненияФайлов`, `_ДемоНачислениеЗарплаты`) | 7 | 2 | 0 | 1 | 2 | 0 | 0 | 10 |

**The twin protocol through the drop-in route** (12.6; the platform stages the descriptors of the case with `import files --partial`, the
platform's apply is the native twin, ours is `ibcmd-rs infobase config apply --recovery-backup=<file>` on the other): checks 2-8 for every case,
with the binary of this branch after the merge of feat/0.4 (`029b4a2b`), except that m2-m5 ran first with the binary before that merge (their
plans are unchanged by it; their offline tests and the snapshot checks of the drop-in twin were repeated with the final binary, see the route
through our import below).

| case | drop-in | 3. `EXCEPT` both ways | 4. `Config` | 6. `.si` rows | 7. the platform's apply afterwards | 8. native exports, `source-diff` |
|---|---|---|---|---|---|---|
| **m1** | 48.5 s | 0 rows (3 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m2** | 8.5 s | 0 rows (6 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m3** | 41.6 s | 0 rows (11 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m4** | 37.3 s | 0 rows (8 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m5** | 27.5 s | 0 rows (12 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m6** | 66.6 s | 0 rows (30 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **m7** | 63.7 s | 0 rows (8 tables) | 0 rows on either side | 16 of 16 | «не требуется» | 12 198 of 12 198 identical |
| **f1** | 65.1 s | 0 rows (5 tables) | 0 rows on either side | 15 of 16 | «не требуется» | 12 200 of 12 200 identical |
| **f3** | 285.8 s | 0 rows (10 tables) | 0 rows on either side | 15 of 16 | «не требуется» | 12 201 of 12 201 identical |

Checks 2 (tables, columns, indexes: the same but the drift list of 12.6) and 5 (`DBSchema` entries equal but `DbCopies*`, on the base of the F rows
equal) held in every case; 6 is "16 of 16 `.si` rows have the same text", or "15 of 16" where a new object is created (`c4629235` differs in the order
of some entries only, `derived-caches.md` 4; the offline test accepts that row by its length). The check 12 (a `THROW` before `COMMIT` of the generated script on a fresh twin) was run once, on
the largest stage, m6 (17 objects, 30 tables, 5 676 rows rebuilt, three cache rows and `siVersions`): the script ran for 46.7 s, the error was raised, and the digest
of the whole database -- rows and checksums of `Config`, `ConfigSave` and `Params`, the schema storage, `DBSchema`, `DBNames`, the checksums of all
columns and indexes, 2 234 tables, no `*NG` table -- was the one before the run (`s1-mix-check12-m6.txt`).

**The F rows (f1, f3).** The platform stages a new object on a base that has had a native apply already (`new-object.md` 8), so these two run on the backup of
case c after its native apply (`bsp8327_c2_native_after`, `MIX_BASE_BAK`) and use its native export as the tree (`MIX_BASE_TREE`); the new objects are the ones
of the S1-F cases N1 (a flat catalog) and N5 (a catalog and a document), copied as they are with `Configuration.xml`. With `Configuration.xml` in the
stage the platform's partial import also wants the forms and templates of every edited object in the base directory, listed or not (it stops with
`Файл объекта не существует .../Forms/ФормаЭлемента.xml` otherwise, and `stage_case.ps1` now stops on an empty `ConfigSave` instead of going on with
nothing staged). The tables of a new object are created empty and come first in `DBSchema`, the rebuilt ones after them (12.15, above).

**The route through our import.** The same stages, staged by `ibcmd-rs infobase config import <the whole tree>` (the base tree, the edited
descriptors, and the forms of the objects that lose attributes) into a pristine clone, then the drop-in apply. The reference is the drop-in twin of
the platform route, which the platform route showed equal to the platform's own result in checks 2-8 (0 rows apart in 3, 4, 8), and, for the
checks that read snapshots (2, 5, 6), the platform's own snapshot (`nat_after`). The native lock is one queue for all tracks, so this pass was
run with one native write per case (`mix_ours.ps1`, the check 7).

| case | our import | drop-in | 2, 5, 6 against the platform's snapshot | 3. `EXCEPT` against the drop-in twin | 4. `Config` rows that differ in text | 7. the platform's apply afterwards | 8. exports against the drop-in twin |
|---|---|---|---|---|---|---|---|
| **m1** | 43 s | 206.2 s | equal | 0 rows | 3 068 of 9 841 | «не требуется» | 4 file(s) differ: 3 edited forms and `ConfigDumpInfo.xml` |
| **m2** | 87.7 s | 242.1 s | equal | 0 rows | 3 068 of 9 841 | «не требуется» | 1 file(s) differ: `ConfigDumpInfo.xml` only |
| **m3** | 37.1 s | 170.9 s | equal | 0 rows | 3 068 of 9 841 | «не требуется» | 1 file(s) differ: `ConfigDumpInfo.xml` only |
| **m4** | 42.8 s | 223.2 s | equal | 0 rows | 3 070 of 9 841 | «не требуется» | 4 file(s) differ: 3 edited forms and `ConfigDumpInfo.xml` |
| **m5** | 56.1 s | 299.5 s | equal | 0 rows | 3 071 of 9 841 | «не требуется» | 4 file(s) differ: 3 edited forms and `ConfigDumpInfo.xml` |
| **m6** | 105.2 s | 546.9 s | equal | 0 rows | 3 082 of 9 841 | «не требуется» | 6 file(s) differ: 5 edited forms and `ConfigDumpInfo.xml` |
| **m7** | 44.8 s | 445.5 s | equal | 0 rows | 3 070 of 9 841 | «не требуется» | 1 file(s) differ: `ConfigDumpInfo.xml` only |
| **f1** (2026-10-01) | 15.3 s (9 staged rows) | 8.3 s | equal (15 of 16) | 0 rows (5 tables) | 27 of 9 840 | «не требуется» | 1 file differs: `ConfigDumpInfo.xml` only; 12 199 equal |
| **f3** | 23.1 s (160.3 s in the first run) | 62.2 s (252 s) | equal (15 of 16) | 0 rows (10 tables) | 30 of 9 841 | «не требуется» | 1 file differs: `ConfigDumpInfo.xml` only |

The m1-m7 and f3 measurements are the historical runs of 2026-09-30. The completed f1 run of 2026-10-01 uses
the merged integration code and its delta importer: unchanged rows stay byte-identical (9 792 of 9 840); only 9 rows
are staged, instead of recompiling the full configuration. The 27 text differences against the platform-staged twin
are confined to object descriptors, object parts and `versions`; checks 2, 3, 5, 6, 7 and 8 pass as shown.

* The **check 3** (`EXCEPT` both ways of every rebuilt table) and the checks 2, 5, 6 are the ones that matter for S1: the structure phase and
  the data of the rebuilt tables do not depend on how the stage got into `ConfigSave`.
* The **check 4** differs by construction until #395: our import re-compiles rows. Counted after inflate, 3 068-3 082 of the 9 841 `Config` rows
  differ in text from the platform's (2 275 of them the `.0` rows, 554 `.2`, 108 `.1`, 103 `.3`, and the descriptors of the changed objects: 3
  in m2, 6 in m5, 17 in m6), 6 081-6 095 differ only in the compression of the same text, 678 are equal bytes. The descriptors of the changed
  objects are written in the older record layout (`{56,` and no `1,0,{1,00000000-...}` tail of each attribute where the platform writes `{57,`
  with the tail); the platform reads both (checks 7 and 8), and the S1 result does not depend on it.
* The **check 8** differs in `ConfigDumpInfo.xml` (its per-object versions are hashes of the rows, and the rows differ as above) and, for a stage
  that removes attributes, in the forms our import was given: the twin the platform staged with the descriptors only keeps the original forms,
  ours has the forms without the elements bound to the removed attributes. `diff_paths.py` verifies that the differing files are **exactly** the
  edited forms and `ConfigDumpInfo.xml`, and that each edited form comes back from the export of our twin byte for byte.
* **The base decides how many rows differ.** The figures above are for the pristine БСП, whose rows were compiled by another build than the one that
  compiles ours; on the base that had a native apply (f3) 30 of 9 841 rows differ in text: the descriptors of the changed and the new objects,
  their `.0` rows, `versions`, and three further parts of rows of the new objects (our import splits a row into parts where the platform keeps one).
* **A hand-made file is not an export.** The catalog files of the S1-F cases were written by a script and lack `<Use>` in the attributes (the exporter always
  writes it, the platform's import defaults it); our import refuses them (`Catalogs/ДемоКатН1.xml: ... descriptor: no <Use>`). The route through our import
  is given the same files with `<Use>ForItem</Use>` (`stage_full/`).
* **A tree that removes an attribute must be a configuration.** The platform's partial import takes the descriptors alone and does not look at
  the forms; our import of a whole tree (like the platform's) refuses a tree whose forms still bind fields to a removed attribute
  (`Form/ChildItems/.../DataPath: "Список.КоррСчет" in the tree, "~Список.КоррСчет" in the configuration that would result`), and a list form
  that keeps a manual query column of it (`Таблица.Версия КАК Версия` in `ВнешниеКомпоненты`: every field of the list turned into `~Список.X`).
  What a user does is what `form_bindings.py` does: the elements bound to `Объект.<attribute>` / `Список.<attribute>` and the query columns are
  taken out of the forms (m1 3 forms, m4 3, m5 3, m6 5).

**The refusals** (`mix_case.ps1 -Refused`): the drop-in must exit 1 and leave the database as it was (the digest of check 12 before and after).
For r7 the copied forms of `КлассификаторБанков` now lose their bindings to `КоррСчет` through `form_bindings.py`, so the
platform stages the Configuration.xml case successfully. The gate then refuses the intended F + B combination, before a backup or write.

| case | the stage | the reason the gate gives | exit | database |
|---|---|---|---|---|
| **r1** | **B + E+ on the same object** (`КлючевыеОперации`: an attribute deleted, a section added), with a widening and a section in other objects | `S1: catalog КлючевыеОперации removes an attribute and adds a tabular section (or attributes of one) in one stage: not built (the caches of a removal are not composed with the ones of a section of the same object)` | 1 | unchanged (digest of check 12) |
| **r2** | **B + E~ on the same object** (`_ДемоЗаказПокупателя`: an attribute deleted, an attribute added to its section), with an index switch elsewhere | `S1: document _ДемоЗаказПокупателя removes an attribute and adds a tabular section (or attributes of one) in one stage: not built (the caches of a removal are not composed with the ones of a section of the same object)` | 1 | unchanged (digest of check 12) |
| **r3** | valid operations on three objects, and a change of `_ДемоПартнеры`, an object the extension `_ДемоРасширение` adopts (S1-I) | `S1: catalog _ДемоПартнеры is adopted by the extension _ДемоРасширение (object _ДемоПартнеры 3014d9c1-cb00-49fb-81b3-e8ced354975f, active image); the extension keeps no table of its own for it: the own restructure does not change an object an extension adopts, the platform's apply does` | 1 | unchanged (digest of check 12) |
| **r4** | valid operations on three objects, and a string that gets shorter (`ОчередьЗаданий.ИмяПользователя` 32 -> 16) | `S1: length-not-widened: Catalog.ОчередьЗаданий: ChildObjects/Attribute[ИмяПользователя]/Properties/Type/StringQualifiers/Length: 32 -> 16` | 1 | unchanged (digest of check 12) |
| **r5** | valid operations on three objects, and an attribute that changes its type (`ОчередьЗаданий.ИмяПользователя` string -> number) | `S1: attribute-property-outside-s1: Catalog.ОчередьЗаданий: ChildObjects/Attribute[ИмяПользователя]/Properties/Type/Type: xs:string -> xs:decimal` | 1 | unchanged (digest of check 12) |
| **r6** | valid operations on three objects, and a subordinate catalog's own attributes (`_ДемоБанковскиеСчета`: one deleted, one widened) | `S1: catalog _ДемоБанковскиеСчета is subordinate to owners: its owner field is not covered` | 1 | unchanged (digest of check 12) |
| **r7** | **F + B**: a new document `ДемоДокН4` (S1-F, N4) and an attribute deleted from a catalog (`КлассификаторБанков.КоррСчет`), with a widening elsewhere | `S1: a new object together with removed attributes of existing objects is not built: the caches of the new object are chained on the additions only` | 1 (dry run and drop-in) | unchanged (digest of check 12); backup not made |
| **r8** | **F + E**: a new catalog `ДемоКатН1` and new tabular sections of existing objects (a section of `КлючевыеОперации`, an attribute of the section of `_ДемоОприходованиеТоваров`), with a widening elsewhere | `S1: a new object together with new tabular sections (or attributes of them) of existing objects is not built: both rewrite the registry, the index of the generated types and the XDTO model` | 1 | unchanged (digest of check 12) |

Each refused stage also contains valid operations on other objects (r3-r6 three of them on other objects, r1 and r2 one or two), and none of them is
half-applied: the gate refuses the whole stage, with the reason, before anything is written.

The rows of the matrix that are not built, and why: one object that removes an attribute and adds a section or an attribute of one (r1, r2:
the caches of the two are not composed for one object; it is the *same-object* case only); a stage that creates an object (S1-F) and removes an
attribute of another (r7: the caches of a new object are chained on additions only, N3, N6 and N7 trace additions) or adds sections to another (r8: both
rewrite the registry, the index of the generated types and the XDTO model); a subordinate catalog's own attributes (the owner field, r6; its sections
are built); and, as before, everything of 12.14 "not traced" and the refusals of 12.9.

Evidence (`docs/apply/evidence/restructuring/`): `s1-mix-matrix.txt` (the summaries of all cases, both routes), `s1-mix-check12-m6.txt`,
`s1-mix-release-20261001.txt` (the final code's gates and completed f1, r7 and ERP UH order proof);
tools `scripts/restructure-lab/`: `edit_cases_s4.py`, `edit_cases_s5.py`, `mix_case.ps1`, `mix_ours.ps1`, `form_bindings.py`, `mix_forms.py`,
`config_compare.py`, `diff_paths.py`, `assemble_mix_evidence.py`, `uha_order_proof.ps1`, `schema_order_dump.py`. Tests: `tests_corpus.rs` (`corpus_mix_*`, nine
cases), `tests_sections.rs` (`a_new_attribute_comes_indexed_with_the_entries_of_a_switch`), `tests_schema.rs` (the place of the rebuilt tables).

**Completed for release (2026-10-01).** The earlier matrix proves m1-m7, f1 (platform route), f3 (both routes), r1-r6 and r8,
and rollback check 12 on m6. The final merged code passes quick fmt, policy guard, layer clippy and root library tests
(3 586 passed, 9 ignored). Fresh runs complete f1 through our import, the r7 refusal after a valid native stage, and the
ERP UH order proof described in 12.9. The UH proof compares the entire unchanged relative order and the six moved-table
positions against the saved native twin result; it does not claim a new full UH twin data/export comparison. The known
`c4629235` ordering approximation of S1-F remains documented, as do the out-of-scope/refused compositions above.
