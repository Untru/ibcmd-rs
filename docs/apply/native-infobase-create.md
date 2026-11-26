# What native `infobase create` writes (SQL Server, 8.3.27 and 8.5)

Issue #342 (0.4, track "trace"). Evidence: four runs of the native `ibcmd infobase create` on empty SQL
Server databases, each traced (Extended Events) and compared row by row before and after with the capture
kit of `scripts/apply-trace/`. Platform `ibcmd` 8.3.27.2214 (runs 1-3) and 8.5.1.1150 (run 4, section 8), SQL Server 2025
(17.0.1135), 2026-09-29.
The verbatim material a re-implementation needs is in `docs/apply/evidence/native-create-8.3.27/`:
`ddl-ru_RU.sql` (every DDL statement of run 1, in order), `ddl-en_US-differences.sql` (what run 3 adds),
`DBSchema.txt` (the 22114-byte schema text, with BOM), `DBNames.txt` (the inflated numbering, 3236 bytes) and
`initial-rows.md` (the decoded small rows of the three runs).

| Run | Command | Database before | Result |
|---|---|---|---|
| 1 | `infobase create --dbms=MSSQLServer --db-server=localhost --db-name=<db> --data=<dir> --locale=ru_RU` | empty, no table, collation Cyrillic_General_CI_AS | 72 tables, 3.8 s wall, 2.9 s of SQL |
| 2 | the same command, another empty database | the same | identical but for per-instance random values |
| 3 | `--locale=en_US` | the same | the collation of the database and of the text columns changes to Latin1_General_CI_AS |
| 4 | platform **8.5.1.1150**, `--locale=ru_RU` | the same | the same 72 tables and 141 DDL statements; only `ibparams.inf` differs (section 8) |

Nothing else needs to exist before the call: no login, no database option. `--create-database` was not used
(the database was made by `restore-clone.ps1 -Corpus empty`); `--date-offset` (default 2000) was not varied.
Not covered here (planned, not measured yet): other locales, `en_US` on 8.5.

## 1. What the platform does, in order

442 events: one working connection (414 statements) and a second one (10 statements, in the first 30 ms), 9 explicit
transactions, all of them for writing one row of `Params`/`Files`.

1. **Probes** (reads): `select count(*) from master..sysdatabases where name=...`, `SET LOCK_TIMEOUT / ARITHABORT /
   DATEFIRST / XACT_ABORT`, `DATABASEPROPERTYEX(db,'UserAccess'|'Collation')`, existence of the service tables.
   The database must exist; the platform only checks its collation.
2. **Collation adjustment** (only when the database collation is not the one the locale wants): see section 4.
3. `IF OBJECT_ID('FORMAT_NUMBER','FN') IS NULL EXEC(...)`: creates the scalar function `dbo.FORMAT_NUMBER`
   (present in every infobase; its definition hash is in the evidence).
4. **The fixed service tables**, each `create table dbo.X (...)` written by hand, in this order: `IBVersion`
   (then `UPDATE IBVersion SET IBVersion = 7, PlatformVersionReq = 80313` returning 0 rows, then `INSERT`),
   `Config`, `ConfigSave`, `Params`, `Files`, `DepotFiles`, `ConfigCAS`, `ConfigCASSave` (all seven with the
   same layout, section 3), `_YearOffset` (+ `INSERT Offset = 2000`), `DBSchema` (+ `INSERT` of the schema text).
5. **First rows of Params / Files**, written with the file-row protocol of section 5: `locale.inf`, `log.inf`,
   `evlogparams.inf`, `Files.dbcopiesparams`, `ibparams.inf`, `log.inf` again.
6. **The remaining 62 tables**: `V8CMSDPWDS`, `SchemaStorage`, `_DbSegments`, `_DbSegmentsItems`,
   `_WebSocketClients`, `_ExtensionsRestruct(NGS)`, `_ExtensionsInfo(NGS)`, `_SystemSettings`, `_CommonSettings`, ...
   `v8users`, `V8USERSMATKEYS`, `V8USERPWDPLCS`, `BinaryData`, `ExternalBinData*`, `BinaryDataStorage*` (the 49 of
   `DBSchema` and 13 more, section 2). Each is `create table dbo.X (...) ;alter table dbo.X SET
   (LOCK_ESCALATION = DISABLE);` followed by its `CREATE [UNIQUE] [CLUSTERED] INDEX ...` statements (68 index
   statements in all, `SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON`). After
   each table (or group) the platform writes the *new state of the schema* into `SchemaStorage` and `DBSchema`
   (`UPDATE SchemaStorage SET CurrentSchema = @P1, Status = ...`, `UPDATE DBSchema SET SerializedData = @P1`,
   19 times each): the schema text is a journal of the structure and doubles as crash recovery
   (`NewGenCreated` / `NewGenDropped`, `Status`, see section 6).
7. **Last rows**: `Params.DBNames.New` written, then renamed to `DBNames` (`DELETE ... EXISTS` +
   `UPDATE Params SET FileName = @P1 WHERE FileName = @P2`), `Params.DBNamesVersion-DBNames`.

## 2. Tables created (72)

* **49 schema tables**: exactly the 49 `"N"` entries of the `DBSchema` text (`DbSegments` ... `STTModelsDesc`;
  table name = `_` + entry name), created from that description.
* **23 other tables**, not in `DBSchema`: the 10 fixed ones (`IBVersion`, `Config`, `ConfigSave`, `Params`, `Files`,
  `DepotFiles`, `ConfigCAS`, `ConfigCASSave`, `_YearOffset`, `DBSchema`) and 13 more with their own DDL
  (`SchemaStorage`, `V8CMSDPWDS`, `v8users` (with the `ByEmail_V8USERS` index), `V8USERSMATKEYS`, `V8USERPWDPLCS`,
  `BinaryData`, `BinaryDataStorageContent`, `BinaryDataStorageVersion`, `ExternalBinDataStrgsList`,
  `ExternalBinDataStrgsBList`, `_STTModelsDesc_Acoustic`, `_STTModelsDesc_Descr`, `_STTModelsDesc_LangModel`).

The complete verbatim list with DDL is `ddl.sql` in the evidence folder (141 statements: 72 `CREATE TABLE`, 1 `CREATE FUNCTION`, 68
`CREATE ... INDEX`; no `DROP`, and no `ALTER DATABASE` for ru_RU).

Rows exist in six tables only (rest 0): `IBVersion` 1, `_YearOffset` 1, `DBSchema` 1, `SchemaStorage` 1,
`Params` 6, `Files` 1. `Config`, `ConfigSave`, `ConfigCAS`, `ConfigCASSave`, `DepotFiles` and `v8users` are empty:
**a created infobase has no configuration at all** (this is what our import then fills, see the case 3 notes in
`native-apply-trace.md`).

## 3. The file-table layout

`Config, ConfigSave, ConfigCAS, ConfigCASSave, Params, Files, DepotFiles` are created identically:

```sql
create table dbo.Config (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null,
  Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null,
  CONSTRAINT ByNameNo_Config PRIMARY KEY (FileName, PartNo))
```

`FileName` inherits the database collation; the PK is named `ByNameNo_<table>` (in the locale-changing path it is
dropped and re-added under an automatic name, see 4). All other constraint names are automatic
(`PK__SchemaSt__<hash>`), so **constraint names differ between two creates** and are not part of the contract.

## 4. Dependence on the locale

`--locale=ru_RU` needs the database collation `Cyrillic_General_CI_AS`; `--locale=en_US` needs
`Latin1_General_CI_AS`. The platform reads `DATABASEPROPERTYEX(db,'Collation')` and, when it differs from the
one of the locale, runs (run 3, verbatim in `ddl.sql` of the en_US run):

```sql
ALTER DATABASE <db> COLLATE Latin1_General_CI_AS
-- then the tables that were already created are repaired:
-- one batch, 21 statements, for the seven file tables that already exist (Config, ConfigCAS, ConfigCASSave,
-- ConfigSave, DepotFiles, Files, Params):
alter table [Config] drop [ByNameNo_Config]                                                          -- x7
alter table [Config] alter column [FileName] nvarchar(128) collate Latin1_General_CI_AS not null    -- x7
alter table [Config] add primary key ([FileName],[PartNo])                                            -- x7, automatic names
-- the tables created afterwards inherit the database collation
```

Everything else is the same in both locales: the 72 tables, the 68 indexes, `DBSchema` and `SchemaStorage`
are byte-identical (same sha256), `IBVersion` and `_YearOffset` are equal. Rows that differ: `locale.inf`
(`{"ru_RU",0,0,"",-1,...}` vs `{"en_US",...}`) and the three per-instance values below. The platform did not
touch any other database option (the read-committed-snapshot option stays as it was).

## 5. The rows

| Table | Row | Bytes (stored) | Content / derivation |
|---|---|---|---|
| IBVersion | (1 row) | | `IBVersion = 7`, `PlatformVersionReq = 80313` (the same in the 8.3.27 clones of the lab) |
| _YearOffset | (1 row) | | `Offset = 2000` (the `--date-offset` default; dates are stored +2000 years) |
| DBSchema | (1 row) | 22114 raw | the schema text `{0,{49,{"DbSegments","N",1,"",{3,{"SegmentId",0,{1,{"B",16,0,"",0}},"",0},...` (49 table descriptions: columns, types, indexes); UTF-8 **with BOM**, CR LF line ends |
| SchemaStorage | SchemaID 0 | | `Status = 100`, `CurrentSchema` = the same 22114 bytes as DBSchema, `NewGenCreated = NewGenDropped = EF BB BF 7B 30 2C 0D 0A 7B 30 7D 0D 0A 7D` (`{0,\r\n{0}\r\n}` with BOM) |
| Params | `locale.inf` | 112 | `{"ru_RU",0,0,"",-1,"","","","",1,0,0000..,0000..}` |
| Params | `log.inf` | 123 | `{<guid>,0,5,0000..,0000..,1,2}`: **the first GUID is random per infobase** |
| Params | `evlogparams.inf` | 6 | `{1}` |
| Params | `ibparams.inf` | 324-325 | `{20,0,0,1,"",1200,86400,0000..,0000..,-1,0,4,5,30,"",{0,"",3,8,3,30,"","","",<u64>,465,1,...},...}`: **token 50 (a 64-bit unsigned decimal) is random per infobase**; the rest is fixed |
| Params | `DBNames` | 567 stored, 3236 inflated | raw deflate of `{52,{52,{0000..,"DbSegments",1},{0000..,"DbSegmentsItems",2},...` : the numbering of the 52 schema-table names (fixed) |
| Params | `DBNamesVersion-DBNames` | 43 | `{0,<guid>}`: **random per infobase** |
| Files | `dbcopiesparams` | 8 | `{1,2}` |

The three random values are the only per-instance bytes found by comparing two creates with the same locale
(`log.inf` GUID, `DBNamesVersion-DBNames` GUID, the u64 in `ibparams.inf`); every other row is identical.
`Creation`/`Modified` of these rows are the real date and time (2026-...) **without** the +2000 years of
`_YearOffset` (the platform applies the offset only to rows it writes later; native apply writes 4026-...).

**The file-row write protocol** (identical for every `Params`, `Files`, `Config`, `ConfigSave` row the platform
writes, at create and later):

```sql
SELECT Creation,Modified,Attributes,DataSize,BinaryData FROM Params WHERE FileName = @P1 ORDER BY PartNo   -- existing?
SELECT COUNT(*) FROM Params WHERE FileName = @P1
INSERT INTO Params (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) VALUES (@P1..@P7)  -- empty BinaryData, PartNo 0
SET TRANSACTION ISOLATION LEVEL READ COMMITTED BEGIN TRANSACTION
DELETE FROM Params WHERE FileName = @P1 AND PartNo <> @P2                     -- @P2 = 0: drop the old parts
UPDATE Params SET Modified = @P1, DataSize = @P2, BinaryData = @P3 WHERE FileName = @P4 AND PartNo = @P5   -- the data of part 0
COMMIT TRANSACTION
```

A row larger than one part is written as several `PartNo` rows (parts of at most 10,000,000 bytes, `DataSize`
repeated in each part; seen in the БСП `Config` for a 22 MB module container).

## 6. Assessment: can we repeat it?

**Yes, with a small template and three generated values.** What determines the result:

* platform build (the DDL and `DBSchema` change with it: `IBVersion`/`PlatformVersionReq`, schema-generated
  tables and their columns),
* the locale (collation of the database and text columns; `locale.inf`),
* three random values, and the timestamps of six rows.

**Two ways to produce the tables**

1. *Replay the captured DDL* (`ddl.sql`, 141 statements, 40 KB) and insert the captured rows, generating the
   three random values. It is exact by construction, needs no knowledge of the schema format, and is cheap to
   verify: create with the native tool and with ours in two databases and compare the snapshots with
   `scripts/apply-trace/snapshot.ps1` + `diff.py` (constraint names and the three random values are the only
   expected differences; the snapshot tables list them). Cost: one template per platform build, refreshed by
   re-running the capture when a new build appears.
2. *Generate the DDL from `DBSchema`*: the 49 schema tables are fully described by the schema text (columns,
   types, index keys, options), the ten fixed tables are constants. This is the same generator the
   restructuring track (#341) needs for `CREATE TABLE ..NG`, so it should be reused there, not written twice.
   The order of the calls and the journaling into `SchemaStorage`/`DBSchema` after every table (section 1,
   step 6) matter only for crash recovery; a create that ends in the final state is enough for a working
   infobase.

**Recommendation.** Start with option 1 for 8.3.27 and ru_RU (all the material is in this repository),
verify by the snapshot diff, and add the en_US path (collation adjustment: `ALTER DATABASE ... COLLATE`, drop
the seven PKs, `ALTER COLUMN ... COLLATE`, re-add the PKs: the two batches of run 3, `ddl-en_US-differences.sql`). For every
other locale, run the native create once and read the collation the platform picks from the trace (the mapping
locale -> collation is not derived here beyond ru_RU and en_US). Move to option 2 when #341's generator exists
and prove it equal to the replay by the same diff. Do not try to derive the random values or the row timestamps:
the platform itself draws them fresh for each infobase.

**Open questions** (not blocking): which locale names map to which collations besides the two tested; the same
capture with `en_US` on 8.5; whether a database created with `--create-database` differs from a pre-made one (file
placement only, by the platform's documentation; not run).

## 7. What follows a create (case 3 of `native-apply-trace.md`)

A created infobase is only a shell; the configuration arrives in two more steps, both traced. Chain measured on
2026-09-29 with platform 8.3.27.2214 on the БСП demo configuration (12197 files):

1. `ibcmd infobase create --locale=ru_RU` (this document), 3.8 s.
2. Our `ibcmd-rs infobase config import` of the whole native export into the empty infobase: 37.2 s, 9838 rows
   into `ConfigSave` (an empty `Config` makes it a base-free import by itself; the bulk copy is invisible to a
   trace, the snapshot diff is the evidence).
3. Native `ibcmd infobase config apply --force --dynamic=disable` (no `--user`: a created infobase has no users):
   72 -> 1825 tables (1753 created, 27 of the 72 rebuilt to add the data-separation fields), `Config` 0 -> 9838
   rows, `ConfigSave` 9838 -> 0, `Params` 6 -> 23 rows, `Files` 1 -> 4 rows, `DBSchema` 22114 -> 949261 bytes,
   one new data row (`_DbCopiesInfoBaseUse`: a random id and `<computer name> : DefAlias`). 123.5 s.
   **It ends with exit code -1** and «Ошибка SDBL: Использование быстрой вставки недопустимо без удаления
   индексов.» on the very last statements (`DELETE FROM _Const3050 WITH(TABLOCK)`, the БСП constant
   `АккредитованныеУдостоверяющиеЦентры`), *after* the configuration was promoted and `ConfigSave` emptied. The
   same command run again exits 0 in 6.7 s and writes nothing.
4. Native `config export` from it (24 s) against the reference export: `ibcmd-rs source-diff` says 12197 files
   identical, 1 different (`ConfigDumpInfo.xml`, and only in the 9835 `configVersion` attribute values), none
   missing on either side. **Acceptance of case 3: met.**

What is missing after the failed apply compared with an apply of an existing infobase: the two `.ui` rows (no users
exist) and the three help-index files `userDocs_ru.bin`, `userVocabulary_ru.bin`, `userPostings_ru.bin` (the
step after the failure), and the row of `_Const3050` (deleted, not re-inserted). Whether these matter is an open
question of `native-apply-trace.md`.

## 8. Platform 8.5.1.1150

Run 4: `C:\Program Files\1cv8\8.5.1.1150\bin\ibcmd.exe infobase create --dbms=MSSQLServer --db-server=localhost
--db-name=<db> --data=<dir> --locale=ru_RU` on a new empty database (collation Cyrillic_General_CI_AS), traced like
the others (capture `c85-native-create`; exit 0, 24.0 s on a busy machine against 3.8 s on an idle one, 444 events,
426 statements, 9 explicit transactions). Evidence: `docs/apply/evidence/native-create-8.5/` (`diff-vs-8.3.27.md` is the
kit's diff of the two created databases, `decoded-rows.md` the differing rows).

**The 8.5 create writes what the 8.3.27 create writes.** Measured:

* the same 72 tables, the same 141 DDL statements (72 `CREATE TABLE`, 68 `CREATE INDEX`, 1 `CREATE FUNCTION`), in the same
  order, statement for statement once the automatic constraint names (`PK__...`) are masked;
* `DBSchema` (22 114 B, with BOM), `SchemaStorage`, `DBNames`, `evlogparams.inf`, `locale.inf`, `Files.dbcopiesparams`
  byte-identical; the same rows in the same six tables;
* **`IBVersion = 7`, `PlatformVersionReq = 80313` unchanged**: an 8.5 create does not require a newer platform than an
  8.3.27 one (the field is not the version of the platform that created the infobase);
* dates of the rows written by create are real years (2026), as in 8.3.27.

**Differences** (all of them): the three random values (`log.inf` guid, `DBNamesVersion-DBNames` guid, the u64 in
`ibparams.inf`), the `service_broker_guid` of the new database, and the text of **`ibparams.inf`, 293 B on 8.5 against
325 B on 8.3.27**: 8.5 writes the two nested groups without their trailing fields.

```
8.3.27: {20,0,0,1,"",1200,86400,0000..,0000..,-1,0,4,5,30,"",
         {0,"",3,8,3,30,"","","",<u64>,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},
         0,0,0,0,0,432000,0,0,2,0,0,0,
         {3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""} }
8.5   : {20,0,0,1,"",1200,86400,0000..,0000..,-1,0,4,5,30,"",
         {0,"",3,8,3,30,"","","",<u64>,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600},
         0,0,0,0,0,432000,0,0,2,0,0,0,
         {3,6,3,60,"","","",0,465,1,"","","","",600},0}
```

(8.5 omits `,0,2,"/AccessToken",""` in the first group and `,0,2,"",""` in the last one, and closes the whole value with
`,0`.) **Consequence for a re-implementation:** one template serves both platforms except `ibparams.inf`, which has to
be chosen by the target platform. The first apply that follows a create is the subject of `native-apply-trace.md`
(section 6.3, measured on 8.3.27); on 8.5 it was not traced (section 11 there covers case 1 on an existing infobase).
