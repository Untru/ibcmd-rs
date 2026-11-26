# What native `config apply` writes: traces on the БСП demo, platform 8.3.27

Issue [#336](https://github.com/Untru/ibcmd-rs/issues/336) ("Трасса штатного `config apply`: что пишется кроме
Config"), track "trace", 0.4 «Своё применение конфигурации». Companion of `native-infobase-create.md` (#342) and
of `restructuring.md` (#341, track "ddl": the DDL and the `SchemaStorage` protocol, **not repeated here**).

Scope of the measurements: platform `ibcmd` 8.3.27.2214, Microsoft SQL Server 2025 (17.0.1135.8), the БСП demo
configuration with its extensions (2 234 tables in the database) restored from the lab corpus, 2026-09-29. Every number is
**measured** unless the text says **hypothesis**. Platform 8.5.1.1150 is measured for case 1 and for `infobase
create` (section 11). Not measured: multi-part rows (> 10 MB), extensions being changed by the apply, `--dynamic=auto`.

**Acceptance correction (2026-10-23):** #336 is open. The original criterion also requires complete ERP УХ
attribute and new-database traces. Neither is measured here. End-to-end 0.3 parity proves the resulting
configuration, not the native write set. See [the remaining capture runbook](uh-remaining-traces.md).

Historical narrowed scope (agreed with the coordinator on 2026-09-29): the БСП cases 1-3 of this document, the
ERP УХ case 1 (modules only; measured, section 12) and the ERP УХ end-to-end runs of ibcmd-rs 0.3 (native create, our
import, native apply, native export identical) as the new-database evidence on УХ. The УХ cases 2 (attribute) and 3
(new database) are not traced.

The traces were made with the capture kit `scripts/apply-trace/` (commits `b3f2bb11` .. `41dc713e` of this branch):
an Extended Events session on the database plus a row-by-row snapshot of every service table before and after.
The full captures (traces of 17 000 - 110 000 statements, snapshots, diffs) are in the lab folder
`F:\ibcmd\lab\04\trace\captures\` (not in the repository); the condensed evidence is in
`docs/apply/evidence/native-apply-8.3.27/` (section 10).

## 1. Summary

1. **An apply writes far more than `Config`.** For a change of five module texts (38 staged rows) the first apply
   of a clone made 12 833 writes to service tables, of which 12 160 are the garbage collection of `ConfigCAS`
   (first apply of a lineage only); a repeated apply of the same kind makes 351. All writes go through the file-row
   protocol of section 3.1; there is no bulk load except the change register.
2. **Two paths.** The apply runs either the LONG path of section 3.2 (prepare, name tables, bookkeeping and rebuild of the change
   register through "new generation" tables, the `commit` marker and the switch, promotion, tail) or a SHORT one (no structure
   phase, no register rebuild, no name tables, no CAS garbage collection: 2 322 statements against 17 126 on the БСП). Measured
   on 37 applies (section 6.4): the long path is taken when a **staged descriptor row of a catalog or document differs in content** from `Config`
   (a changed synonym or attribute; a native `import files` of a catalog or document module, which re-saves the object in the current
   format; not a data processor's descriptor, which stays short) **or when more than 20 rows are staged**, otherwise the short path. The kind of the module, the state of the database
   and the staging tool as such do not matter. What changes with the change is only the *content* of a few rows (section 4) and,
   for structure changes, the DDL of `restructuring.md`.
3. **Rows rewritten on every apply of an existing infobase, whatever the change (both paths)**: `Params <guid>.ui` (3 rows),
   `Files MobileVersions.dat` (a 1000-GUID ring, one new random GUID), the 16 `Params <guid>.si` rows and `siVersions` (new
   random guids). **The long path also rewrites** `Files extd_props_cached/gc.mrk`, the name tables `Params DBNames*`, `DBSchema`
   and `SchemaStorage` (identical content when the structure is unchanged) and rebuilds the change register `_ConfigChngR`
   (+ `_ExtProps`) with **new row ids** through `..NG` tables even when nothing changed; the short path updates `_MessageNo` of
   the changed objects in place. The help index `userDocs_ru.bin` / `userVocabulary_ru.bin` / `userPostings_ru.bin` is rebuilt
   when a help row is staged or more than 20 rows are staged (6.4).
4. **The crash protocol is a marker row**: `Config` row `commit` (empty) is written after everything is prepared and
   deleted after everything is promoted; the promotion of the staged rows runs in autocommit, row by row, **outside
   any transaction** (19 230 statements, 45 s for 9 615 staged rows). `dynamicCommit` (exclusive: absent)
   holds the list of names in a dynamic apply.
5. **Exclusive and dynamic (`--dynamic=force`) differ only in naming and two rows** (section 5): staged rows go to
   `<name>_dynupdate_<generation>` instead of replacing `<name>`; a new overlay of 16 `.si` rows is added; the
   change register gets `_MessageNo = NULL` for the changed objects; `dynamicCommit`, `dbStruFinal` and
   `DynamicallyUpdated` are written. The structure phase and the register rebuild are the same in both.
6. **Case 3 works end to end.** Native `infobase create` -> our import of the whole native export -> native `apply`
   -> native `export` gives an export identical to the reference (12 197 files identical, `ConfigDumpInfo.xml`
   differs only in the 9 835 `configVersion` values). The first apply of such an infobase ends with exit code
   -1 and «Использование быстрой вставки недопустимо без удаления индексов» on the last statements, after
   everything was committed; a second apply is a no-op with exit 0 (section 6.3).
7. **Dates.** Native writes `Creation`/`Modified` with the `_YearOffset` (+2000 years: `4026-09-29`); the rows our
   import stages carry `2026-...`. `infobase create` itself writes real years. (Section 8.)
8. **ERP УХ (21 187 tables, 118 k `Config` rows; section 12).** The same kind of apply (10 module bodies, 23 staged rows) took 522 s and
   28 701 statements, 99 % of them reads (52 k descriptors in batches, 20 k bodies one by one, 6 k `_DataHistoryMetadata` probes); the
   writes were 228 statements in 28 s. No CAS garbage collection (`ConfigCAS` is empty), no register rebuild (the register is empty), no
   `SchemaStorage` walk: the structure line is in the log, the write set is the short path's plus `DBNames.New` passes, `_DbCopies*`
   housekeeping and, because no native apply had completed on that base, `ibparams.inf`, `locale.inf` and two new `.ui` rows.
9. Findings for other tracks and the open questions are in sections 8 and 9.

## 2. Method and cases

Every case = a clone (`ibcmd_rs_04_trace_<suffix>`, the suffix is in the table) restored from the БСП 8.3.27
corpus; staging by **our** tool or by
**native** `ibcmd infobase config import files --partial`; one native `config apply --force` under the kit
(`capture.ps1`: snapshot, XE trace, snapshot, diff). The native apply always ran with `--user=Администратор` except
on the created infobase (no users). The wrappers took the lab locks (`heavy`, and since 11:30 `native`).

| id | case | staged by | apply | database | wall | statements | capture (lab) |
|---|---|---|---|---|---|---|---|
| E01 | baseline (pilot): no-op import of the whole tree (patch mode, 9 517 rows) | ours | `--dynamic=disable` | `_noop` | 122.6 s (trace window; exit not recorded, the log ends with success) | 60 172 | `20260929-101302-e01-noop-import-apply` |
| 1x | **case 1**, five module texts (common module, catalog object module, document manager module, managed form module, common form module), first apply | ours (`mssql-stage-source-objects`, 38 rows) | `--dynamic=disable` | `_c1_disable` | 108.8 s | 17 126 | `20260929-105417-c1-disable-exclusive` |
| 1d | case 1, the twin restored from the same backup | ours, byte-identical rows | `--dynamic=force` | `_c1_force` | 73.1 s | 17 163 | `20260929-110125-c1-force-dynamic` |
| 1x' | case 1, second edit of the same modules on the database after 1x (steady state) | ours | `--dynamic=disable` | `_c1_disable` | 30.2 s | 4 627 | `20260929-115633-c1b-second-exclusive` |
| 1d' | the same on the twin after 1d | ours | `--dynamic=force` | `_c1_force` | 33.0 s | 4 700 | `20260929-115935-c1b-second-dynamic` |
| 2s | **case 2**, String(25) attribute in `Catalog._ДемоВидыНоменклатуры`: staging | native, one file (9 rows) | - | `_c2_attr` | 6.0 s | 477 | `20260929-111517-c2a1-native-stage-attribute` |
| 2a | case 2, apply of that stage | native | `--dynamic=disable` | `_c2_attr` | 110.2 s | 16 912 | `20260929-111719-c2a2-native-apply-attribute` |
| 2n | case 2, a stage from our patch-mode import that carried **no** attribute (it drops it) | ours (9 rows) | `--dynamic=disable` | `_c2_attr` | 45.1 s | 2 322 | `20260929-110735-c2a-attribute-added` |
| 2ns | case 2, new catalog `ТрассаНовыйСправочник` (no attributes) + `Configuration.xml`: staging | native (9 615 rows) | - | `_c2_attr` | 362.6 s | 89 712 | `20260929-112108-c2b1-native-stage-new-catalog` |
| 2c | case 2, apply of that stage | native | `--dynamic=disable` | `_c2_attr` | 157.3 s | 52 077 | `20260929-113311-c2b2-native-apply-new-catalog` |
| 3 | **case 3**, the whole tree into a created infobase: our import 37.2 s, apply, retry, export | ours (base-free by itself) | `--dynamic=disable` | `_c3_new` | 123.5 s (exit -1) + 6.7 s | 87 128 + 416 | `20260929-114021-c3-ours-import`, `-114151-c3-native-apply`, `-115512-c3-native-apply-retry` |

Staging mode of case 2: the attribute and the new catalog were staged with the **native**
partial import, because our **patch mode drops a new attribute silently** (2n: all 9 staged rows identical to
`Config` but for `versions`; the apply saw no structure change and took the short path of section 6.4). The
`--base-free` mode of our import was not run for case 2 (track ddl staged its structural cases that way and native
accepted it); case 3 is the base-free path by construction (an empty `Config`).

Two side effects of the lab worth knowing when reading the diffs:

* **First apply on a clone of the corpus** (1x, 1d, 2a): the clones were last touched by an older platform build. The
  apply also upgrades service tables (`_DbCopies` +`_StorageVariant`, `_DbCopiesUpdates` +3 columns), sets
  `ALLOW_PAGE_LOCKS = OFF` on the primary keys of eight extension tables (`ALTER INDEX ... _Reference10877X1`, ...)
  and collects the garbage of `ConfigCAS`/`Files` (12 797 -> 636 rows, 353 -> 44 rows). Later applies (1x', 1d', 2c)
  do none of this.
* **2c re-imported the whole tree**: with `Configuration.xml` in `--partial`, native staged 9 615 rows (every
  object gets a new version guid; the 517 role-rights rows gained an entry for the new object) and, because the tree
  `c2b` has no attribute, the apply **dropped** the `_Fld11034` column that 2a had added (`_Reference12` rebuilt). That
  is noise for the new-catalog case; the new catalog itself is the creation of `_Reference11035`.

## 3. The exclusive apply (`--dynamic=disable`), phase by phase

### 3.1 The file-row write protocol

Every row of `Config`, `ConfigSave`, `Params`, `Files` (and `ConfigCAS`) is written the same way (verbatim in the
create document, section 5):

```
SELECT COUNT(*) FROM <T> WHERE FileName = @P1                                   -- exists?
INSERT INTO <T> (FileName,Creation,Modified,Attributes,DataSize,BinaryData,PartNo) VALUES (@P1..@P7)   -- empty part 0, autocommit
BEGIN TRANSACTION
DELETE FROM <T> WHERE FileName = @P1 AND PartNo <> 0                            -- drop old extra parts
UPDATE <T> SET Modified=@P1, DataSize=@P2, BinaryData=@P3 WHERE FileName=@P4 AND PartNo=0   -- data (UPDATE ... PartNo=n for more parts)
COMMIT
```

`Modified` (and `Creation` of new rows) is `now + _YearOffset` (2000 years) with ten fractional digits in the
parameter (`4026-09-29 10:57:21.1640765552`). Parts hold at most 10 000 000 bytes. The help-index writer (E2) adds
three autocommit statements after the data transaction, `UPDATE Files SET Attributes = @P1 WHERE FileName = @P2`,
then the same for `Creation` and for `Modified` (explicit file info); the other writers do not. Renames are
`UPDATE <T> SET FileName = @new WHERE FileName = @old`; a promotion is `DELETE FROM <T> WHERE FileName = @P1
AND EXISTS(SELECT 1 FROM <T> WHERE FileName = @P2)` followed by that `UPDATE` (both autocommit).
Tables are read with `SELECT Creation,Modified,Attributes,DataSize,BinaryData FROM <T> WHERE FileName = @P1 ORDER BY PartNo`.

### 3.2 The phases

These are the phases of the **long path** (which path an apply takes: section 6.4). The short path has A0 - A4, the `.sinew`
rows of B4, C1 and D1 - E2, with an in-place `_MessageNo` update instead of B1 - B3 and C2; its blocks are in
`evidence/native-apply-8.3.27/write-phases-no-change-short-path.md`.
Times are from 1x (108.8 s) with the steady state 1x' (30.2 s) in brackets. The command opens about a dozen
connections, most of which only run `SET` statements; the writes of phase A come from one connection, everything
from B on from another (the session id changes at 11.4 s).
`write-phases.md` of each capture (in the evidence) lists the same blocks in order.

| # | phase | writes (table: row name, size, format) | tx |
|---|---|---|---|
| A0 | **check** (0 - 9.4 s; 0 - 9.5): "Проверка корректности метаданных", thousands of single-row `SELECT ... FROM Config/ConfigSave WHERE FileName = @P1` | none | - |
| A1 | first row | `Params <guid>.ui` (`54ba7662-...ui`, 9 bytes, unchanged content: only `Modified` moves) | 1 tx |
| A2 | clean-up | `DELETE FROM ConfigSave WHERE FileName LIKE '%.new'`, `DELETE FROM Config WHERE FileName LIKE '%.new'` (leftovers of an interrupted apply) | autocommit |
| A3 | **copy of the staged rows** (9.56 - 9.70 s) | for every `ConfigSave` name: `DELETE Config '<n>.new' (if it exists)` + `INSERT Config SELECT '<n>.new', Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM ConfigSave WHERE FileName = '<n>'`; in FileName order, each object as `<guid>` then `<guid>.<n>` ascending, then `root`, `version`, `versions` (38 rows: 14 `<guid>`, 21 `<guid>.<n>`, 3) | autocommit |
| A4 | mobile-client ring | `Files MobileVersions.datNEW`: INSERT empty, then UPDATE 37 009 B (section 4.3) | 1 tx |
| A5 | name tables, pass 1 | `Params DBNames.New` (143 619 B stored), `DBNames-Ext-1.New` (20), `DBNames-Ext-347eea02-....New` (954), `DBNames-Ext-b3fa0ef0-....New` (17 817), each INSERT + DELETE + UPDATE; then `DELETE FROM Params WHERE FileName = 'DBNames.New' AND FileName <> 'Params/DBNamesVersion-DBNames'` (and the three others): pass 1 is thrown away | 4 tx |
| B0 | connection 2: `SELECT SchemaID, Status FROM SchemaStorage WHERE SchemaID <> 0`, `_DbCopies` read, `_ExtensionsRestructNGS` DELETE (0 - 3 rows) | | |
| B1 | **rebuild of the change register** (11.5 - 18.0 s [14.2 - 16.1]) | `CREATE TABLE _ConfigChngRNG`, `_ConfigChngR_ExtPropsNG` (+ 3 indexes each, dropped and created again around the load), `UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200`, `insert bulk` of all rows (20 685 + 21 357 rows in batches of 10 000; the source is read through 1C temp tables `#tt`), `_ExtensionsRestruct` DELETE + INSERT ×2 | 2 tx |
| B2 | extension bookkeeping (18.0 - 22.6 s) | `Files MobileVersions.datNEW` rewritten twice (new head GUID each time), `_ExtensionsRestructNGS` DELETE + INSERT (2 tx) twice, `Files extd_props_cached/gc.mrk` (9 B, section 4.3) | 1 tx per write |
| B3 | **ConfigCAS garbage collection** (22.9 - 96.8 s; **first apply of a lineage only**) | `DELETE FROM ConfigCAS WHERE FileName = @P1` ×12 160 (sha1 names), `DELETE dbStruFinal<guid>` (1); then in **one** transaction 309 `DELETE FROM Files` (`userDocs_ru_<sha1>.bin`, `userPostings_ru_<sha1>.bin`, `userVocabulary_ru_<sha1>.bin`, 103 each) and `Files CAS_GC_Info` rewritten (`{0,20260929105718}`) | 1 tx |
| B4 | name tables, pass 2, and the `.sinew` cache (98.2 - 103.5 s [18.5 - 25.1]) | `Params DBNames*.New` again (same protocol), then `Params <guid>.sinew` ×16 (16 names, 443 B .. 2.9 MB, 4.06 MB in all), one tx per row; `DBNames*.New` deleted if equal to the current row (they are, for module edits) | 20 tx |
| C1 | **commit marker** (103.7 s [25.3]) | `Config commit` INSERT empty, DELETE, UPDATE (0 bytes) | 1 tx |
| C2 | switch of the register (103.7 - 105.5 s [25.3 - 26.5]) | `UPDATE SchemaStorage SET Status = ...`, `drop table _ConfigChngR`, `drop table _ConfigChngR_ExtProps`, `UPDATE SchemaStorage`, 5 × `exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object'` (2 tables, 3 indexes; each via `sp_prepexec` in its own tx), `ALTER INDEX <pk> ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)` (first apply of a clone: + 8 on the `X1` tables), `UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3` (`@P1` = the 977 476-byte schema text, the trace keeps the first 524 186), `UPDATE DBSchema SET SerializedData = @P1` (same text) | 5 tx |
| D1 | **promotion in `Params`** (105.6 - 106.2 s) | `DELETE <guid>.si` ×16 + `UPDATE <guid>.sinew SET FileName = '<guid>.si'` ×16; `siVersions` DELETE + UPDATE (1 273 B); `DELETE DynamicallyUpdated`; when the names changed: `DELETE DBNames` + `UPDATE DBNames.New -> DBNames`, `DBNamesVersion-DBNames` DELETE + UPDATE (43 B, `{0,<guid>}`) | 1 tx |
| D2 | **promotion in `Config`** (106.2 - 106.5 s; 45 s for 9 615 rows) | (1) first the rows of pending overlays of earlier dynamic updates: `DELETE '<n>'` + `UPDATE '<n>_dynupdate_<g>' -> '<n>'` (the merge), `DELETE versions_dynupdate_<g>`, `DELETE deleted_dynupdate_<g>`; (2) for every staged object, the `<guid>.<n>` rows ascending and the `<guid>` descriptor **last**: `DELETE FROM Config WHERE FileName = '<n>' AND EXISTS(<n>.new)` then `UPDATE Config SET FileName = '<n>' WHERE FileName = '<n>.new'`; (3) `root`, `version`, `versions`; (4) `DELETE Config DynamicallyUpdated`. `deleted.new` is deleted without being promoted | **none** (autocommit) |
| D3 | clean-up | `DELETE FROM ConfigSave WHERE FileName LIKE '%'`; `Files MobileVersions.dat` DELETE + `UPDATE ... datNEW -> dat`; `DELETE Config commit`, `dynamicCommit`, `dbStruFinal` (three autocommit deletes, absent rows included) | autocommit |
| E1 | user rows (106.6 s) | `Params 789702c6-....ui` (24 301 B) and `97f2c291-....ui` (94 B) rewritten, one tx each | 2 tx |
| E2 | **help index** (107.0 - 107.4 s) | `Files userDocs_ru.new` (46 KB), `userVocabulary_ru.new` (103 KB), `userPostings_ru.new` (512 KB), each: `DELETE FROM Files WHERE FileName = 'userX_ru.new'`, INSERT empty, (tx) DELETE + UPDATE data, then the three file-info UPDATEs of 3.1; afterwards, for the three: `DELETE userX_ru.bin` + `UPDATE userX_ru.new SET FileName = 'userX_ru.bin'` | 3 tx |

The whole run has 48 user transactions (43 `BEGIN TRANSACTION` batches and the 5 `sp_rename` calls); every
explicit transaction wraps one file-row write (the `DELETE ... PartNo <> 0` + `UPDATE` pair), the `.si` promotion,
one `sp_rename` or the CAS clean-up. Nothing spans phases: an interrupted run is resumed from the markers, not
rolled back (section 3.3).

### 3.3 Markers and states

| row / value | meaning (measured) |
|---|---|
| `Config` `<n>.new` | a staged row copied to `Config`, waiting for the switch |
| `Config` `commit` (0 bytes) | everything is prepared and the register is being switched; present from C1 to D3 |
| `Config` `dynamicCommit` | dynamic apply only: `{N,"<name>",...}` all N staged names (38 in 1d), present C1 to D3 |
| `Config` `dbStruFinal` (0 bytes) | dynamic apply only, present between the `.si` overlay and D3; deleted at the end of every apply |
| `Config` / `Params` `DynamicallyUpdated` | the list of overlay generations of dynamic updates (section 5) |
| `SchemaStorage.Status` 100 -> 200 -> 400 -> 500 -> 100 | the structure protocol of `restructuring.md` section 3.2 |
| `ConfigSave` rows | the input; the last statement of D3 empties the table |

Track ddl measured that a run interrupted after the switch of the schema and before the promotion leaves a database
that refuses every `ibcmd config` command (their section 10): the `commit` marker and the `.new` rows are what the
platform resumes from, and native does not resume when they are inconsistent.

## 4. What is written, table by table

Case columns: **fixed** = the same in every apply of an existing infobase; **content** = depends on the change.
The complete matrix (statements / rows / bytes per family and case) is `write-families.md` in the evidence.

### 4.1 `Config`, `ConfigSave`, `ConfigCAS`

* **Copy and promotion** (A3, D2): every staged row, changed or not; 38 rows in case 1, 9 615 in 2c. New content in
  `Config` = the staged bytes (`INSERT ... SELECT`, `RENAME`): the apply does not touch the payload. Rows of
  `Config` keep their `Creation`/`Modified` of the staging (native: `4026-...`, ours: `2026-...`).
* **`versions`**: staged; the header uuid (`{1,<count>,"",<generation>,...}`) is the "Создано поколение
  конфигурации" line of the log. The apply does not modify it (1x: `848a0a59-...` -> `bd7d351c-...`, the value the
  staging wrote).
* **Deleted rows**: none in a plain apply. 1x deleted six rows only because the clone carried a pending overlay of an
  earlier dynamic update (`<guid>_dynupdate_06cb0442-...` ×4, `versions_dynupdate_...`, `DynamicallyUpdated`).
* **`ConfigSave`** ends empty (D3). An apply after a *partial* native import (track ddl saw a `ConfigSave` of 9 597 -
  9 635 rows and a native apply that died at «Принятие изменений») is a real risk, but **the row count alone does
  not tell**: the complete native stage of 2ns has 9 618 rows against 9 841 in `Config` (a stage holds only the rows
  whose version guid changed, and here 9 615 of the 9 840 entries of `versions` did). The check used here, all
  measured on 2ns: no `commit` row and no `%.new` rows in `ConfigSave`, `root`, `version` and `versions` present,
  and every entry of the staged `versions` whose guid differs from `Config`'s has a `ConfigSave` row (9 615 of 9 615).
* **`ConfigCAS`** (B3): garbage collection of the entries no extension version refers to (12 797 -> 636 rows, sha1
  names, one statement per row, 74 s). Not repeated on a collected database. What "unreferenced" means is not decoded.
* **`Config` `deleted`** (`ConfigSave deleted`, 45 B text): written by the native import when objects disappear from
  the tree; `1,"4e84d26f-8ca9-45dd-b1f3-8037eb78f6bd",1` in 2ns is the attribute that 2a had added and the tree
  `c2b` lacks. It is copied as `deleted.new` (A3) and then **deleted, not promoted** at the start of D2; it drives
  the removal of the column (`_Reference12` rebuilt without `_Fld11034`), and `DBNames` keeps the entry.

### 4.2 `Params`

| row | fixed / content | size | format and derivation |
|---|---|---|---|
| `<guid>.ui` ×3 | fixed | 9 B, 24 301 B, 94 B | `54ba7662-...` is rewritten with the same 9 bytes (`Modified` only). `789702c6-...` and `97f2c291-...` are Base64 text (lines of 64 characters) whose content changes on every apply with the same size (**hypothesis**: re-encrypted with a fresh IV); decoding belongs to track ui |
| `DBNames`, `DBNames-Ext-1`, `DBNames-Ext-<guid>` ×2 | content | stored 143 619 B, 20 B, 954 B, 17 817 B (inflated 347 167 B, 18 B, 2 309 B, 40 882 B) | raw deflate of `{<max>,{<count>,{<guid>,"<kind>",<number>},...}}`: the numbering of metadata objects and fields (`Fld`) and of platform tables (nil guid: `WebSocketClients`, `DbCopies...`). **Append-only**; the number counter is global over the main and all extension `DBNames`: 2a: `{4e84d26f-...,"Fld",11034}` (11034 = max over `DBNames-Ext-b3fa0ef0-...` (11033) + 1; the main header said 10824), 2c: `a51bda0b-...` "Reference" 11035 (the new table `_Reference11035`) and the three new platform tables 11036 - 11038 (header 11039). Rewritten only when an entry was added (module edits: identical, `.New` thrown away) |
| `DBNamesVersion-DBNames` | content | 43 B | `{0,<guid>}`, a new random (v4) guid whenever `DBNames` changed; the `-Ext-` rows never changed |
| `<guid>.sinew` -> `<guid>.si` ×16 | fixed | 295 B .. 3.0 MB | raw deflate of `{2,{1,<guid>,5,...}}` / `{0,{27,...}}`: indexes of the metadata (type patterns `{"Pattern",{"#",<type guid>}}`; `1a621f0f-....si` is the list of all metadata objects, entries `<guid> <parent guid> <kind code> "<name>" {1,1,{"ru","<synonym>"}} 0 0` with the entry count in the header: the added attribute made `10807 -> 10808` and one entry `4e84d26f-... eeaa0c4f-... 36 "ТрассаРеквизит" ...`), 16 rows with fixed names (the same 16 in the created infobase). **Content is identical to the previous apply when only modules change** (1x: 16 of 16 "same content, other metadata"); 2 of 16 changed for the attribute (2a; `1a621f0f-....si` grew by 41 B) and 9 of 16 for the new catalog with the removed attribute (2c). The rows are rebuilt and written on every apply |
| `siVersions` | fixed | 1 273 B | `{0,16,"<guid>.si",<guid>,...}`: **a new random (v4) guid for every one of the 16 entries on every apply**, entries reordered |
| `DynamicallyUpdated` | fixed (deleted; written in a dynamic apply) | 82 B .. 119 B | section 5 |
| `evlogparams.inf`, `ibparams.inf`, `locale.inf`, `log.inf` | not touched | | written by `infobase create` only |

### 4.3 `Files`

| row | fixed / content | size | format and derivation |
|---|---|---|---|
| `MobileVersions.dat` (`.datNEW` -> `.dat`) | fixed | 37 009 B (1000 guids) | `{<n>,<guid>,<guid>,...}` a **ring of at most 1000 version guids**; each apply puts one **fresh random (v4) guid** in front and drops the last (`after[1:] == before[:-1]` in 4 of 4 applies). `.datNEW` is written three times (a new head guid each time; the third is the result). The created infobase starts with `{1,<guid>}` (43 B) |
| `extd_props_cached/gc.mrk` | fixed (long path) | 9 B | `0x11` + a little-endian 64-bit **time in 1/10 000 s since 0001-01-01**, real local time of the apply (`11 20 12 d3 f5 67 45 02 00` = 2026-09-29 10:56:02). Decoded from eight samples of five applies, all equal to the `Modified` of the row minus 2000 years |
| `CAS_GC_Info` | with the CAS GC | 21 B | `{0,20260929105718}` local time `YYYYMMDDhhmmss` of the GC |
| `userDocs_ru.bin` / `userVocabulary_ru.bin` / `userPostings_ru.bin` | when a help row or more than 20 rows are staged (6.4); content: see below | 46 KB / 103 KB / 512 KB stored (229 KB / 516 KB / 1.0 MB inflated) | raw deflate of UTF-16LE text: documents (`244192<TAB>22<TAB>6ba07682...`), a vocabulary (`0 187 0`, `00 70 187`, ...), postings. The log line is «Построение индекса справки». Stored size 46.2 / 103.1 / 511.9 KB in 1x, 1d, 1x', 1d' (46.4 KB for the first file in 2n and 2a); **2.2 / 8.6 / 4.9 KB in E01 and 2 bytes each in 2c** (the two applies that moved ~9 600 staged rows), and absent after the failed first apply of the created infobase. **Derivation open** (section 9) |
| `userDocs_ru_<sha1>.bin` ... | with the CAS GC | 2 B each | the per-content parts of the same index, deleted by B3 (103 of each) |
| `dbcopiesparams` | not touched | | written by create |

### 4.4 Data tables and structure

* **`_ConfigChngR` (+ `_ConfigChngR_ExtProps`)**: rebuilt **on every apply of the long path**, also with no structure change
  and also in a dynamic apply (B1, C2); the short path keeps the rows and their ids and sets `_MessageNo = NULL` for the changed
  objects in place (2n: one `UPDATE`, 7 rows). `_IDRRef` of all rows is new after a rebuild (a sequential run from a fresh base value, e.g.
  `8F5B00E04C68009311F1BBDB32A05000`, `...05001`, ...); `(_NodeTRef, _NodeRRef, _MDObjID)` is the key and does not
  change; the `..NG` DDL is `restructuring.md` section 3.3. **`_MessageNo` after the rebuild is NULL exactly for the
  objects whose descriptor row `<guid>` was in the staged set of this apply, and 0 for all others** (measured in
  six applies, the counts match to the row: the register has five nodes; per large node 1x has 14 NULL = the 14
  staged descriptors, 2a 2 NULL, 2c 4 930 NULL = the 4 930 staged descriptors; the two small nodes hold the subset
  of those objects they register: 3, 0, 1 270; in the steady state 1x'/1d' nothing flips because the same objects
  are staged again). The clone's registers came with NULL for 17 279 objects, so the first apply shows `NULL -> 0`
  for them (1x, 1d, 2a: 17 279 / 17 279 / 17 321 rows) and 2c, which staged nearly everything, `0 -> NULL`
  (17 321 rows). A **new object** is registered by the apply at three of the five nodes with `_MessageNo = NULL`
  (2c: `0BDA1BA5-1C1B-3149-80D4-6AB10522DE81` = `a51bda0b-...`, three new rows in `_ConfigChngR`, none in
  `_ExtProps`); which objects and nodes a register holds beyond that is not decoded. The dynamic apply also
  runs `UPDATE T2 SET _MessageNo = CAST(NULL AS NUMERIC(38,8)) FROM _ConfigChngR T2 WHERE _MDObjID IN (<the 14
  changed objects>) AND _IDRRef IN (SELECT RS_FIELD FROM #tt6)` (49 rows), which the rebuild rule already implies.
* **`_ExtensionsRestruct`, `_ExtensionsRestructNGS`**: bookkeeping of the extensions' restructure state: DELETE and
  INSERT of 1 - 3 rows per step (`_ExtDataID` = the extension, `_RestructData` a blob of 444 - 13 890 B); after the
  apply `_ExtensionsRestructNGS` holds three rows.
* **`SchemaStorage`, `DBSchema`**: rewritten with the same schema text when the structure did not change (1x, 1d:
  the rows hash equal); with the new text when it did (2a: 977 751 B, 2c: 979 610 B). `restructuring.md` section 3.2.
* **New platform tables** on the first apply of an older clone: `_DbCopiesInfoBaseUse`, `_DbCopiesUpdateStat`,
  `_DbCopiesUpdateTableStat`, `_WebSocketClients` (2c), `_DbCopies`/`_DbCopiesUpdates` altered (2a). In a created
  infobase `_DbCopiesInfoBaseUse` gets one data row `{_Id = random guid, _Description = "<computer> : DefAlias"}`.

## 5. The dynamic apply (`--dynamic=force`)

Same phases A - E with these differences (1d against 1x, 1d' against 1x'):

| step | exclusive | dynamic |
|---|---|---|
| A3 copy | `<n>.new` | the same |
| C1 marker | `commit` | `commit` and `dynamicCommit` (`{38,"<n>",...}`) |
| D1 `.si` | `.sinew` -> `<guid>.si` (old rows replaced) | `.sinew` -> `<guid>_dynupdate_<gen>.si`: **16 more rows** (Params 39 -> 55); the old `.si` and `siVersions` stay valid; `siVersions` is still rewritten |
| after D1 | | `Config dbStruFinal` (empty), `Params DynamicallyUpdated` `{0,3,<base gen>,<previous overlay>,<gen>}` (119 B), `UPDATE _ConfigChngR SET _MessageNo = NULL` for the changed objects |
| D2 promotion | `<n>.new` -> `<n>` | `<n>.new` -> `<n>_dynupdate_<gen>`; `versions.new` -> `versions_dynupdate_<gen>`; `root`, `version` as in exclusive; `Config DynamicallyUpdated` `{1,2,<previous overlay>,<gen>}` (82 B) |
| result | 9 844 -> 9 838 rows | 9 844 -> 9 880 rows (+36 overlay rows: 21 + 14 + `versions`), then +36 again per further dynamic apply (9 916 after 1d'); Params +16 per apply |
| the register rebuild, `sp_rename`, help index, `.ui`, ring | as exclusive | as exclusive |

`<gen>` is the header guid of the staged `versions` row (the same in 1x and 1d, since the twin restored the same
staged rows). Overlay rows accumulate until an **exclusive apply merges them** (rename to the base name, or delete
when a newer staged row replaces them; 1x did that for the clone's old overlay). Timing: 73.1 s against 108.8 s
(first apply; both include the CAS GC, the load of the machine differed between the runs), 33.0 s against 30.2 s
(steady state): no measurable difference in the work itself.

## 6. The cases

### 6.1 Case 1: modules only (1x, 1d, 1x', 1d')

* Staged: 38 `ConfigSave` rows for five modules (14 descriptors `<guid>`, 21 `<guid>.<n>`, `root`, `version`,
  `versions`). The five modules: common module `06427cd3-....0`, document manager module `08c4ceb3-....2`,
  catalog object module `28e59c50-....0`, managed form module `a12cfe30-....0`, common form `323997ca-....0`. In the
  steady state (1x') **exactly** these five containers (`container element text changed`) and `versions` differ from
  `Config` (6 rows); the other 32 staged rows are identical.
* On the first apply (1x) 11 staged rows differed in content from `Config`: the five module rows, `versions`, and
  five managed-form rows (`04332cbc-....0`, `1f671d47-....0`, `3b52d3f8-....0`, `73338f66-....0`, `81b9c9c4-....0`)
  where **our writer produces other properties than the platform stored** (e.g. added `15 {"U"}` and `19 {"S" ""}`,
  counters `11 -> 13`); a further 16 rows differed only in the deflate stream and 8 only in line breaks. Once
  these rows are in the database (1x') the diff is clean: findings for the import track, section 8.
* What is written: everything of section 3.2; 12 833 service writes in 1x (12 160 of them CAS GC, 309 `Files` GC),
  351 in 1x'. Statements: 17 126 (4 627), user transactions 48 (47), DDL statements 27 (19) of which 5 renames:
  the register only, plus in 1x the eight primary-key lock options.
* Time, from the blocks of `write-phases.md`: 108.8 s = 9.4 s check + 2.0 s prepare + 11.2 s register and
  bookkeeping + 73.9 s CAS GC + 1.4 s `Files` clean-up + 5.3 s names and `.sinew` + 3.6 s switch, promotion and
  tail. Steady state 30.2 s = 9.5 s check + 1.3 s prepare + 4.4 s register and bookkeeping (behind a 3.3 s gap
  while the second connection starts) + 6.6 s names and `.sinew` + 3.5 s switch, promotion and tail.
* Twin comparison (1x against 1d, restored from one backup, measured on the `after` snapshots): the row sets
  written are the same except section 5; all 36 overlay rows of 1d, its `versions_dynupdate_...` (344 223 B) and its
  16 overlay `.si` rows equal, decoded, the rows 1x wrote under the base names.

### 6.2 Case 2: an attribute (2s, 2a), a new catalog (2ns, 2c)

**Staging by native import** (for reference; the import track will want it): one file, 9 rows in `ConfigSave`
(2s, 6.0 s): `<guid>.<n>.new` ×4, `<guid>.new` ×2, `root.new`, `version.new`, `versions.new` (340 595 B, the whole
row), `commit`, each through INSERT + DELETE + UPDATE in its own tx, then renamed to the final names and `commit`
deleted; 10 tx. Our patch-mode stage of the same change has the same 9 rows but the catalog descriptor lacks the
attribute (2n).

**Apply of the attribute (2a)**: everything of 3.2, plus the DDL of `restructuring.md` for `_Reference12`
(`_Reference12NG` rebuilt with the new column `_Fld11034 nvarchar(25)`: 9 rows copied, 4 indexes, 4 renames, the
old table dropped), `_DbCopies` and `_DbCopiesUpdates` rebuilt for their new columns, `DBNames.New` promoted to
`DBNames` (one entry), `DBNamesVersion-DBNames` new, two of the 16 `.si` rows with new content, `DBSchema`
977 476 -> 977 751 B. Log: «Объект изменен: Справочник._ДемоВидыНоменклатуры»,
«Изменена структура таблиц базы данных». 110.2 s, 56 DDL statements (of which 14 renames).

**Apply of the new catalog (2c)**: the same plus `_Reference11035` (+ 3 indexes) and the three platform tables
created through `..NG`, 19 `sp_rename`, `DBNames` +4 entries, `_ConfigChngR` +3 rows; the staged 9 615 rows make the
copy (53 s, 19 231 statements) and the promotion (45 s, 19 230 statements) the dominant costs. Log: «Новый объект:
Справочник.ТрассаНовыйСправочник». 157.3 s.

### 6.3 Case 3: a new infobase

See `native-infobase-create.md` section 7 for the numbers. The apply of the whole tree into the created infobase:
the file-row protocol as above; the structure of 1 753 tables is created through `..NG` (5 507 `sp_rename`),
27 of the 72 tables of the create are rebuilt for the data-separation fields; **no CAS GC, no `.ui` rows, no
`DBNames-Ext`**; `MobileVersions.dat` is created as `{1,<guid>}`; `CAS_GC_Info` is written. The error at the end:

```
SELECT T1._Fld3051, T1._RecordKey FROM dbo._Const3050 T1 WITH(NOLOCK)                      -- 1 row
SELECT T1._Fld3051 FROM dbo._Const3050 T1 WITH(TABLOCK) WHERE DATALENGTH(T1._Fld3051) < 128
DELETE FROM T1 FROM dbo._Const3050 T1 WITH(TABLOCK)                                          -- 1 row deleted
-> «Ошибка SDBL: Использование быстрой вставки недопустимо без удаления индексов.»  exit code -1
```

(`_Const3050` = the constant `АккредитованныеУдостоверяющиеЦентры`, `varbinary(max)`, the platform re-inserts its
value with a fast insert into a table that has an index.) All promotions, the emptying of `ConfigSave` and the
deletion of the markers had been executed before. The retry logged 416 events and changed no table (exit 0, 6.7 s).
The export of the result matches the reference (section 1, item 6).

### 6.4 Which path an exclusive apply takes

Not every exclusive apply runs the phases of section 3.2. The platform's own log tells the path. The LONG path prints
«Обработка структуры базы данных…» and «Реструктуризация  Таблица регистрации изменений конфигурации …» and rebuilds the
change register (a new `_IDRRef` for every row); the SHORT path prints «Принятие изменений…» and «Регистрация изменений в
планах обмена…» twice and leaves the register ids alone. «Построение индекса справки» (in the short path together with «Сбор
служебной информации») is a separate matter: it appears when a help row (`<guid>.1`) is among the staged rows (X0, X1, X4, Y1,
Y2, Y5 - Y7, Y13, Y15) and also in the long applies of more than 20 staged rows without a help row (Y16 - Y19); it does not
appear in the applies of 20 rows or less without a help row (X2, Y3, Y4, Y9, Y12, Y14, Y20) nor in the long applies of 7 rows
(X3, Y8, Y10, Y11).

**The experiment.** 25 untraced applies on 8 clones of the БСП corpus (`ibcmd_rs_04_trace_c4_*` and `_c5_*`,
`--dynamic=disable`, log lines, staged row count and register ids as the evidence), each with one chosen staged set; staged by our
`mssql-stage-source-objects` (patch mode, tree of case 1) or by the native `import files --partial` from a sparse base
directory (only the edited `.bsl` files; the platform stages the object descriptor and the changed rows). With the traced
applies of this document that is 37 applies, one line each in `docs/apply/evidence/native-apply-8.3.27/path-experiment.tsv`;
the scripts are `scripts/apply-trace/lab/run_paths*.ps1`.

| id | clone (nth apply on it) | staged by | what was staged | rows | descriptor differs | path | time |
|---|---|---|---|---|---|---|---|
| X0 | c4_paths (1) | ours | catalog _ДемоВидыНоменклатуры, patch-mode stage (the attribute is dropped: nothing effective) | 9 | no | **short** | 20 s |
| X1 | c4_paths (2) | ours | catalog _ДемоКонтрагенты: object module, 5 forms, help | 18 | no | **short** | 91.7 s |
| X2 | c4_x2 (1) | ours | common module _ДемоЛокализацияКлиентСервер | 5 | no | **short** | 41.9 s |
| X3 | c4_paths (3) | native | catalog _ДемоКонтрагенты object module + module of the form ФормаЭлемента of catalog _ДемоГруппыДоступаНоменклатуры | 7 | yes | **long** | 149 s |
| X4 | c4_x4 (1) | ours | catalog _ДемоКонтрагенты: synonym changed, forms, help | 18 | yes | **long** | 231.3 s |
| Y1 | c4_x4 (2) | ours | document _ДемоПоручениеЭкспедитору: manager module, 2 forms, help | 11 | no | **short** | 18.7 s |
| Y2 | c4_paths (4) | ours | common module + catalog _ДемоКонтрагенты object module (its forms already equal to Config) | 20 | no | **short** | 15.6 s |
| Y3 | c4_paths (5) | ours | common module (again) | 5 | no | **short** | 12.8 s |
| Y4 | c4_x4 (3) | ours | common module | 5 | no | **short** | 14.3 s |
| Y5 | c5_union (1) | ours | common module + catalog _ДемоКонтрагенты + document _ДемоПоручениеЭкспедитору with forms and help (the stage of c1c) | 28 | no | **long** | 243.9 s |
| Y6 | c5_doc (1) | ours | document _ДемоПоручениеЭкспедитору: manager module, 2 forms, help | 11 | no | **short** | 17.2 s |
| Y7 | c5_pair (1) | ours | document + catalog object module, with forms and help | 26 | no | **long** | 77.5 s |
| Y8 | c5_doc (2) | native | catalog _ДемоКонтрагенты object module + document manager module (no form rows) | 7 | yes | **long** | 62.9 s |
| Y9 | c5_union (2) | native | module of one catalog form (ФормаЭлемента of _ДемоГруппыДоступаНоменклатуры) | 5 | no | **short** | 8.6 s |
| Y10 | c5_pair (2) | native | common module + document manager module | 7 | yes | **long** | 52.6 s |
| Y11 | c4_x4 (4) | native | object modules of two catalogs (_ДемоКонтрагенты, _ДемоОрганизации) | 7 | yes | **long** | 61.8 s |
| Y12 | c4_paths (6) | native | two common modules (_ДемоЛокализацияКлиентСервер, _ДемоЛокализация) | 7 | no | **short** | 28 s |
| Y13 | c5_pair (3) | ours | common module + document manager module, forms, help (our tool; Y10 staged the same edits natively) | 13 | no | **short** | 27.4 s |
| Y14 | c5_union (3) | native | common module + catalog object module requested; native staged the common module only | 5 | no | **short** | 53.9 s |
| Y15 | c5_probe (1) | ours | common module + catalog _ДемоКонтрагенты with forms and help | 20 | no | **short** | 19.8 s |
| Y16 | c5_cms (1) | ours | twelve small common modules | 27 | no | **long** | 454 s |
| Y17 | c5_union (4) | ours | nine small common modules | 21 | no | **long** | 23.7 s |
| Y18 | c5_pair (4) | ours | ten small common modules | 23 | no | **long** | 22.2 s |
| Y19 | c5_doc (3) | ours | eleven small common modules | 25 | no | **long** | 37.8 s |
| Y20 | c4_x4 (5) | ours | eight small common modules | 19 | no | **short** | 12.5 s |

The traced applies (sections 2, 6.1 - 6.3, 11) fit the same rule: 2n (9 rows, no change) short; 1x, 1d, 1x', 1d' (38 rows),
c1c (28), the 8.5 pair (41), E01, 2a, 2c and case 3 long.

Seven stages of track apply (S1 - S5C, `docs/apply/own-apply.md` on feat/0.4, their native logs in
`F:\ibcmd\lab\04\apply\logs\native_*.out`) were staged by that track (with its own tools or the native import) and applied by the native `config apply`; this track did not
run them and uses them only to test the rule: S1, S2, S3 (30 rows) and S5B (4 rows, the descriptor of a **catalog** gains a form reference) long; S4 (16 rows), S5A (15 rows) and
S5C (2 rows) short, S4 and S5A although they stage changed descriptors of a **data processor** (rule 1 below). The evidence
file lists six of them (S1 has no row count in that document).

**The rule that fits all 37 applies, qualified by those seven stages.**

1. **A staged descriptor row (`<guid>`, no suffix) of a catalog or a document (an object that owns tables) whose content differs
   from the row in `Config` gives the long path.** Changed synonym (X4), added attribute (2a), new catalog (2c), a new infobase (3),
   a form reference added to the descriptor of a catalog (track apply, S5B: 4 rows, long); and every native `import files` of
   a catalog or document module (X3, Y8, Y10, Y11), because the platform **re-saves such an object in the current
   serialization**: on the corpus the document descriptor grows from 14 983 to 15 947 B and the catalog descriptor from
   42 266 to 44 219 B (field counts `13 -> 14`, `24 -> 25`, `5 -> 6`, a block `3b10624f-... {"#", 502b7765-...}` and a `0 {1
   00000000-...}` added, the header count `56 -> 57`). A form or a common module is not re-saved that way: their descriptors
   stay byte-identical after a native import (Y9, Y12, Y14; compared with a pristine clone) and the apply is short. Our patch-mode
   stage never changes the descriptor of an object whose descriptor did not change (all our arms but X4).
   **The rule does not hold for a data processor.** Track apply's S4 (16 rows) and S5A (15 rows) staged changed descriptors of a data
   processor (its active text and the references to the new forms and templates) together with the descriptors of the new forms
   and templates, and both stayed on the short path; only the row limit of rule 2 sends a data-processor stage to the long path.
   So rule 1 is proven for **catalogs and documents** (every arm of this document with a changed descriptor is of that kind, plus
   S5B), refuted for data processors, and **untested for every other kind** of object (registers, enumerations, common modules,
   reports, ...) and for the descriptors of forms and templates other than as new objects.
2. **With unchanged descriptors the number of staged rows decides: up to 20 rows short, from 21 rows long.**
   The last short ones have 19 rows (Y20: eight common modules) and 20 rows (Y2, Y15: a common module and a catalog with its
   forms); the first long ones 21 (Y17: nine common modules), 23 and 25 (Y18, Y19). The kind of the rows does not matter: a common
   module alone (X2, Y3, Y4) or eight of them are short, **nine to twelve small common modules without any form or data object**
   are long (Y16 - Y19), and the second edit of case 1 (1x': 38 staged rows, only 6 of them different from `Config`) is long.
   Rows and objects were not separated (a common module is two rows): the limit is "more than 20 staged rows" or "nine or more
   staged objects" (the catalog set has eight: the catalog, its five forms and one more object).

**What does not decide**, each measured: the kind of the edited module (container or the text inside a form row: Y9 alone is
short); which objects are edited (catalog + document ours: Y7 long, but common module + document ours: Y13 short with 13 rows,
the same edits staged natively: Y10 long, because the native stage carries a changed document descriptor); the state of
the database (the same edit short on a fresh clone, after a short apply and after a long apply: Y6, Y3, Y4; long sets long on fresh and
used clones: Y5, Y7 against c1c, X3).

**Not known.** Why the platform switches at that number of rows: the statements of the two paths are the same up to the first
write and the check phase reads the same rows, so the decision is taken in memory. Also not known: whether the limit counts rows or
objects; the descriptor rule for kinds of objects other than catalogs and documents (a data processor's does not count: S4, S5A;
registers, enumerations, common modules, ... untested); whether the limit is the same on 8.5 (there only the long case was
measured, section 11); Y14 asked for a common module and a catalog module, but the native import staged the common module only
(5 rows; the reason is not established), so it counts as a one-object set.

Short path, traced (2n; 2 322 statements, 45.1 s): `.ui`, `.new` copy, `MobileVersions.datNEW` once, `.sinew` x16,
`commit`, `.si` promotion, `dbStruFinal`, `UPDATE _ConfigChngR SET _MessageNo = NULL` (7 rows), promotion (with the
merge of a pending overlay), clean-up, `.ui`, help index. It has no `..NG` table, no `DBNames`, no CAS garbage
collection, one `DELETE` on `_ExtensionsRestructNGS` (0 rows) and no other `_ExtensionsRestruct*` write, and leaves
the register ids alone. Times (indicative; other tracks were working): the short applies took 9 - 28 s when the machine was quiet, the long ones without garbage collection 22 - 63 s.

**For the own apply.** The long path does everything the short one does and more, so it is safe for any set; every database of these
experiments, after either path, took the next native apply. The rule above is the selector if the short path is wanted as an
optimization: a changed descriptor of a catalog or a document, or more than 20 staged rows, means long (a data processor's
descriptor does not). The rule describes what native does; a
database produced by our apply has to be accepted by native whichever path we take. The ERP УХ apply (section 12) confirms rule 2 at
twelve times the size (23 staged rows: the structure line appeared) and shows that "long" names a set of phases, not a fixed write set:
with an empty change register and no extensions it wrote the short path's rows plus a few small items.

## 7. Which of it is required (classification, mostly hypothesis)

| kind | rows | evidence / hypothesis |
|---|---|---|
| the configuration | `Config` promotion (`root`, `version`, `versions`, staged rows), `DBSchema`/`SchemaStorage`, `DBNames` (+ version) | measured: what the platform reads back; the export of case 3 depends on it |
| protocol | `commit`, `dynamicCommit`, `dbStruFinal`, `.new` names, `Status` | measured: resume markers; without them a stopped run cannot be continued (track ddl) |
| derived caches | 16 `.si` + `siVersions`, the help index, `_ConfigChngR` ids | **hypothesis**: rebuilt by the platform when absent or stale; content unchanged for module edits. Not tested |
| bookkeeping | `MobileVersions.dat`, `gc.mrk`, `CAS_GC_Info`, `_ExtensionsRestruct*`, `.ui` | **hypothesis**: not needed for a server to open the configuration; the first three carry only times and random guids |
| garbage collection | `ConfigCAS`, `Files` clean-up | measured: unrelated to the change; first apply of a lineage |

The proposal in section 9 turns the hypotheses into a test: apply with the minimal subset and open the result with
native `config export` and `infobase config check`.

## 8. Findings other tracks need

* **Import.** (a) The rows our tools stage carry `Creation`/`Modified` in `2026`, the platform's `4026`
  (`_YearOffset`); after the apply `Config` holds a mix. (b) Patch mode stages 9 517 rows for a no-op (E01: 6 100
  differ only in compression, 2 661 only in formatting, **399 in content**) where native stages 9 rows for one
  attribute (2s); 1x showed five form rows with extra properties. (c) Native staging of `Configuration.xml` is a
  full re-import: 9 615 rows, 362.6 s with 50 sessions, 225 rollbacks, 9 841 transactions (2ns). (d) Adding a
  metadata object adds an entry to the rights rows of every role (517 rows in 2ns).
* **Apply.** Everything in section 3; the promotion must run outside a transaction row by row like native, or in
  a transaction if we accept a different recovery story; the `commit` marker before the switch is the contract with
  native tools.
* **Everyone who checks a native stage** (lab rule of 2026-09-29 11:30, "check the `ConfigSave` row count"): a complete stage can hold fewer rows than
  `Config` (2ns: 9 618 of 9 841, complete by the `versions` test of section 4.1), so a count near 9.6 thousand, as
  track ddl reported for its partial stages, is not proof of a partial stage by itself; use the `versions` test.
* **Track ddl.** (a) The number counter of `DBNames` is global over the main and the extensions' `DBNames`;
  the main header may lag (10824 while the extension counter is at 11033); the header after 2c (11039) exceeds the
  largest entry (11038) by one, cause unknown. (b) `sp_rename` runs through `sp_prepexec` (the kit reads it now).
  (c) The register is rebuilt in the long path (exclusive and dynamic alike) and updated in place in the short one (6.4). (d) A created infobase needs 27 service tables
  rebuilt for the data-separation fields.
* **Track ui.** Three `.ui` rows per apply: one touched, two re-encrypted with the same size (section 4.2).
* **Create (#342).** The first apply on a created infobase fails on `_Const3050` after committing everything
  (section 6.3); anyone replaying `infobase create` + our apply must not copy that failing statement.

## 9. Open questions

1. **What decides the long or the short path** - answered empirically in section 6.4: a staged descriptor row of a catalog or document that differs from
   `Config` (also the native re-save of a catalog or document in the current format), or more than 20 staged rows. A data processor's
   descriptor does not count (track apply, S4 and S5A). Left open: why the platform switches at that number of rows (rows or
   objects), the descriptor rule for kinds of objects other than catalogs, documents and data processors, and on 8.5.
2. **`_ConfigChngR`**: the `_MessageNo` rule of section 4.4 is exact on six applies (1x, 1d, 1x', 1d', 2a, 2c); open: which objects and nodes a
   register holds at all (20 685 rows, five nodes; a new object went to three nodes), and the generator of the
   row ids.
3. **`DBNames` header** after 2c (11039 with entries up to 11038); the allocation order of the three platform tables.
4. **`.si`**: which metadata property changes which of the 16 rows; whether a stale `.si` set is tolerated.
5. **Help index**: when it is rebuilt is answered in 6.4 (a help row is staged, or more than 20 rows are staged); what feeds
   its content is open. Its size is the same in four applies that moved 38 rows, but tiny (E01) or empty (2c)
   in the two that moved ~9 600 rows, although the help pages are in `Config` in all of them. Whether the
   created infobase gets it at its next apply.
6. **`ConfigCAS` GC**: the reference rule; does it run when nothing is unreferenced?
7. **The failed first apply** (6.3): is the `_Const3050` row needed (the constant's default value)? Does a second
   apply with a real change write the missing `.ui`/help index rows?
8. **Multi-part rows** and **extension changes** in an apply, `--dynamic=auto`, **ERP УХ cases 2 and 3** (case 1 is measured, section
   12): not traced. Also open from the УХ run: a new object on an empty register, the bare 13-row stage, a steady-state apply, a base with
   extensions or users (12.7).

**Proposed next step for the apply track:** implement the exclusive apply in two layers: first the SHORT path (the phases of
2n: small and fully known), then the LONG extras (name tables, register rebuild through `..NG` tables, garbage collection: section
3.2 and `restructuring.md`). Take the long path when a staged descriptor of a catalog or document differs from `Config` or when more than 20 rows are
staged (section 6.4; a data processor's descriptor does not count), or always: it is a superset and safe. Use `compare_traces.py` to check each layer against a native oracle on a
twin (the diff of both `after` snapshots must show only random guids, timestamps and the ids of `_ConfigChngR`); test the
hypotheses of section 7 by dropping the cache writes one at a time.

## 10. Evidence and how to repeat

* Condensed evidence: `docs/apply/evidence/native-apply-8.3.27/write-families.md` (the matrix: every family of
  writes, per case; the `Config` copies `<n>.new` are left out with `--skip-config-copies`; columns E01,
  modules-excl = 1x, modules-dyn = 1d, modules-excl-2 = 1x', modules-dyn-2 = 1d', no-change = 2n, attribute = 2a,
  new-catalog = 2c, new-infobase = the first apply of 3; a cell is "statements / rows affected / payload bytes"),
  `write-phases-*.md` (the blocks of consecutive writes per case, in order).
* Captures (lab, not in the repository): `F:\ibcmd\lab\04\trace\captures\<id>\` with `trace\{summary,write-phases,
  timeline,groups}.md`, `service-writes.tsv` (every write in order), `payloads.jsonl`, `ddl.sql`, `diff\diff.md`,
  `before\`, `after\`; blobs in `F:\ibcmd\lab\04\trace\blobs`.
* Repeat one case: restore a clone (`F:\ibcmd\lab\04\tools\restore-clone.ps1`), stage, then
  `pwsh -NoProfile -File F:\ibcmd\lab\04\trace\scripts\run_native_apply.ps1 -Database <db> -Tag <tag> -Dynamic disable`
  (heavy and native locks, kit capture); compare runs with
  `python scripts/apply-trace/compare_traces.py --capture <label>=<capture dir> ...`.
* The case scripts (copies in `scripts/apply-trace/lab/`, originals in `F:\ibcmd\lab\04\trace\scripts\`):
  `run_native_import_files.ps1` (native staging), `run_native_apply.ps1`, `run_case3.ps1`, `run_queue*.ps1` (the cases of
  section 2), `run_paths*.ps1` and `make_case1*_tree*.ps1` (the path experiment of 6.4), `run_native_create.ps1`,
  `make_case1_tree85.ps1`, `validate_sparse_import.ps1`, `check_stage.py` (the `versions` completeness test of 4.1),
  `stage_vs_config.ps1`, `dump_rows.ps1`, `row_sha.ps1`, `run_uh_case1.ps1` (section 12).
* Evidence of the path experiment: `docs/apply/evidence/native-apply-8.3.27/path-experiment.tsv` (one line per apply of 6.4:
  staged rows, time, phases seen in the log, whether the register ids changed).
* Evidence of the ERP УХ case (section 12): `docs/apply/evidence/native-apply-uh/` - `trace-summary.md` and `write-phases.md` (the kit's
  reports), `matrix-uh-vs-bsp.md` (`compare_traces.py` against the БСП long, short and 8.5 applies), `diff-condensed.md` (the
  snapshot diff without the module texts), `reads-profile.md` (the statements by class and by 30 s), `params-before-after.md`
  (`ibparams.inf`, `locale.inf`, `log.inf`, the ring). The full capture stays in the lab as
  `captures\20260929-171426-uh-case1-forms-exclusive` (123 MB with the two snapshots).
* Evidence of platform 8.5.1.1150 (section 11): `docs/apply/evidence/native-apply-8.5/` - `matrix-first-apply-8.3.27-vs-8.5.md`
  and `matrix-steady-8.3.27-vs-8.5.md` (`compare_traces.py` output, `--skip-config-copies`) and the `write-phases-*.md` of both
  8.5 applies; for `infobase create` `docs/apply/evidence/native-create-8.5/`.

## 11. Platform 8.5.1.1150 (case 1) and the differences from 8.3.27

Measured on 2026-09-29 with `ibcmd` 8.5.1.1150 on one clone, `ibcmd_rs_04_trace_c85_mod`, restored from the БСП 8.5 corpus
(2 325 tables; 9 948 `Config` rows after the apply; the change register 21 219 + 21 863 rows). Case 1 is the case of 6.1:
the five module texts of the 8.5 demo tree (common module, catalog object module, document manager module, managed form
module, common form module), staged by **our** `mssql-stage-source-objects --platform 8.5.1.1150` (41 rows: 15 descriptors,
23 `<guid>.<n>`, `root`, `version`, `versions`), applied with `--dynamic=disable` (exclusive only, as agreed). The corpus
names its administrator «Администратор (обычное приложение)»: `--user=Администратор` ends at authentication with exit code -1.
Captures `c85-modules-exclusive-2` (the first apply of the clone) and `c85-modules-exclusive-steady` (the same five modules
edited again, like 1x'). `infobase create` on 8.5 is in `native-infobase-create.md` section 8.

### 11.1 Numbers

| | 8.3.27 first (1x) | 8.3.27 steady (1x') | 8.5 first | 8.5 steady |
|---|---|---|---|---|
| command | 108.8 s | 30.2 s | 293.0 s | 52.9 s |
| first write of the apply (end of the check phase) | 9.2 s | 9.5 s | 19.1 s | 12.8 s |
| statements | 17 126 | 4 627 | 23 956 | 4 768 |
| user transactions | 48 | 47 | 49 | 48 |
| DDL statements | 27 | 19 | 270 | 21 |
| `ConfigCAS` garbage collection | 12 160 deletes, 74 s | none | 18 627 deletes, 219 s | none |
| staged rows | 38 | 38 | 41 | 41 |
| register rebuilt (`_ConfigChngRNG` + ExtProps) | 20 685 + 21 357 | same | 21 219 + 21 863 | same |

The machine ran several other tracks during these applies, so the times are indicative (the CAS deletes cost 6.1 ms each
on 8.3.27 and 11.8 ms on 8.5 in these runs); the statement counts and the row sets are not affected.

### 11.2 What is the same

The phases A - E of section 3.2 and every family of 4.1 - 4.3 in the same order (`matrix-first-apply-8.3.27-vs-8.5.md`
has one row per family): the `.ui` rows, the `.new` copies, `MobileVersions.datNEW` three times, the name tables in two
passes, the register rebuild through `..NG` tables with `sp_rename`, `gc.mrk`, the CAS garbage collection with the
`Files` clean-up and `CAS_GC_Info`, the 16 `.sinew` -> `.si` and `siVersions`, the `commit` marker, the promotion outside
any transaction, the two `.ui` rows of the user, the help index. The write set of the steady state is the same family by
family (`matrix-steady-8.3.27-vs-8.5.md`). The `_MessageNo` rule of 4.4 holds: on the first apply 17 365 rows of the
register go `NULL -> 0` (the clone's register came with NULL, as on 8.3.27 with 17 279), none in the steady state, and
`_IDRRef` is new for 21 219 of 21 219 rows in both. Sizes differ only with the corpus (`DBNames` 269 KB against 281 KB,
the extension `DBNames` 49 KB against 37 KB, the help index 47 / 104 / 512 KB against 46 / 103 / 512 KB).

### 11.3 What differs

1. **`_DbCopies*` housekeeping on every apply of a base that has the `_DbCopies*` tables** (the 8.5 БСП corpus; absent from the 8.3.27
   БСП apply, whose corpus predates `_DbCopiesInfoBaseUse`, but **not a version feature**: the 8.3.27 apply of the УХ base does the same,
   section 12.4): right after the first pass of the name tables and before the `CREATE TABLE _ConfigChngRNG`: `UPDATE _DbCopiesInfoBaseUse` (1 row), `DELETE FROM
   _DbCopiesInitialLast`, `DELETE FROM _DbCopiesUpdates` and `TRUNCATE TABLE` of `_DbCopiesTrChObj` and `_DbCopiesTrChanges`;
   the tables are empty in the corpus. It is 2 more DDL statements in the steady state (21 against 19).
2. **Lock options, first apply only** (not in the steady state, measured on the second apply): after the register switch
   (`sp_rename` block) and before the promotion, `ALTER TABLE <t> SET (LOCK_ESCALATION = DISABLE)` on the ten service
   tables (`IBVersion`, `_YearOffset`, `DBSchema`, `Config`, `ConfigCASSave`, `ConfigCAS`, `ConfigSave`, `DepotFiles`,
   `Files`, `Params`) and 240 `ALTER INDEX <pk> ON <t> SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)` on the primary
   keys of the data tables (`Enum`, `Reference`, `Document`, `Node`, `_STTModels*`, `v8users`): 26 s from the start of
   the `sp_rename` block to the promotion (8.3.27: 2 s), of which the `ALTER INDEX` statements take 9 s and 10.6 s are a gap
   without traced writes. The 8.3.27 first apply did the same for eight extension tables (9 `ALTER
   INDEX` in all) and no `ALTER TABLE`. **Hypothesis**: a one-time upgrade of the physical options of a clone that an
   older build created; a database created by 8.5 needs none of it.
3. **`version` and `root` rows** (not touched by the apply, but the platform writes its own): `version` is `{{216,0,{80324,0}}}`
   on 8.3.27 and `{{217,0,{80501,1,{<guid>}}}}` on 8.5; the third token of `root` is empty on 8.3.27 and a Base64 text
   (lines of 64 characters) on 8.5. Our writer has to produce the 8.5 form when it stages for 8.5.
4. **Time**: the check phase and the garbage collection are slower in these runs (19.1 s against 9.2 s; 11.8 ms against
   6.1 ms per delete); the load of the machine differed, so this is not a platform figure.
5. `infobase create`: identical DDL and `DBSchema`; `ibparams.inf` is 293 B on 8.5 against 325 B (`native-infobase-create.md`
   section 8).

Consequence for the own apply: one implementation serves both platforms, with the 8.5 additions of items 1 and 3; item 2
is an upgrade step that a correct apply on a corpus clone does not need to reproduce (hypothesis, not tested: a database
without the options works).

## 12. ERP УХ 8.3.27, case 1 (measured)

Measured on 2026-09-29 as planned in `uh-case1-plan.md`, with the go and the OK for one clone of the coordinator. This is the
size class the own apply has to handle: 21 187 tables and 118 377 `Config` rows, twelve times the БСП. Together with the БСП
cases 1 - 3 and the end-to-end runs of ibcmd-rs 0.3 it is the acceptance evidence of #336 for УХ; the УХ cases 2 and 3 are not
traced.

### 12.1 Set-up

* **Clone** `ibcmd_rs_04_trace_uha_c1`, restored from `uha_parity2_20260924.bak` (37 s; data file 4 296 MB, log shrunk to 72 MB).
  The corpus looks like a base on which no native apply has completed (its service rows are what `infobase create` writes, see
  12.4): `ConfigCAS` **0 rows**, the change register
  `_ConfigChngR` and `_ConfigChngR_ExtProps` **0 rows**, `v8users` 0 rows (so the commands need no `--user`), no extensions
  (`Params` has no `DBNames-Ext-*`), `Files` 14 rows (the help index in Russian and English, `MobileVersions.dat` of 43 B,
  `CAS_GC_Info`, `gc.mrk`, `ib.pfl`, ...), `Params` 23 rows (16 `.si` rows of 25.5 MB in all, `DBNames` 3.0 MB stored, no `.ui`),
  `SchemaStorage` with a 14.9 MB schema text, `ibparams.inf` of 292 B in the 8.5 form and `locale.inf` with zero GUIDs.
* **Stage**: the five modules of case 1 (common module, catalog object module, document manager module, managed form module, common
  form module) and five padding common modules, one comment line appended to each; staged by the **native** `import files
  --base-dir <sparse> --partial` from a sparse directory of 10 files (457 KB; 40 s), 23 `ConfigSave` rows (10 descriptors, 10
  bodies, `root`, `version`, `versions`). `check_stage.py`: COMPLETE (118 170 `versions` entries, 23 differ). The ten descriptors are
  **identical** to `Config` (native did not re-save them in another serialization, as it did on the БСП corpus), so rule 1 of 6.4 does not apply and rule 2
  (23 rows, more than 20) predicted the long path.
* **Apply**: `ibcmd infobase config apply --force --dynamic=disable`, platform 8.3.27.2214, `heavy` then `native` held from 17:24:42
  to 17:33:25 (8.7 min), released the moment the command returned; kit capture with `-DataChecksum counts -MaxStatementKB 32
  -ContentMaxKB 256 -ContentBudgetMB 512`. Capture `20260929-171426-uh-case1-forms-exclusive`; evidence in
  `docs/apply/evidence/native-apply-uh/`. The runner: `scripts/apply-trace/lab/run_uh_case1.ps1 -Go -SkipRestore -Database
  ibcmd_rs_04_trace_uha_c1`.

### 12.2 Numbers

| | estimate in the plan | measured | БСП 8.3.27 steady (1x') |
|---|---|---|---|
| command | 6 - 15 min (up to 60 with the CAS GC) | **522.1 s** (8.7 min), exit 0 | 30.2 s |
| statements | 50 000 - 270 000 | **28 701** (28 326 SELECT) | 4 627 |
| writes to service tables | about 350 + 8 per staged row | **228** in 20 blocks, 26 user transactions | 351 |
| DDL | register rebuild expected | **2** (`TRUNCATE` of two `_DbCopies*` tables) | 19 |
| CAS garbage collection | none to 220 000 deletes | **0** (`ConfigCAS` is empty) | 0 (done in the first apply) |
| trace | 27 min for 500 k events | 28 753 events, 0 dropped, XE file 66.7 MB; **export of the events to XML 7.5 min** (64 events/s, the slow step of the kit), report 8 s | - |
| snapshots | 15 - 45 min each | 613 s before, 1 077 s after (60 MB each); diff 80 KB | - |
| disk | peak 10 - 12 GB | clone 4.3 GB (the data file did not grow; log 136 MB), `ibdata` 0.77 GB, capture 123 MB; peak about 5.5 GB | - |

The estimate of the number of statements was too high twice over: there is no garbage collection here, and the reads are 0.24 per
`Config` row against 0.40 on the БСП. The time and the lock hold were inside the estimate; the disk was well below it.

**Where the time goes** (`evidence/native-apply-uh/reads-profile.md`, times relative to the first statement of the trace):
99 % of the statements are reads, and 95 % of the 518 s pass before the first write or between write groups; the write groups take about 28 s,
11.5 s of them for the `.sinew` rows. The platform reads every descriptor (52 154 distinct `<guid>` rows in 115 `IN`-list statements)
and then **20 376 body rows one by one, each exactly once** (17 390 of them `<guid>.<n>`): 6.4 k of them in the first 245 s
(«Проверка корректности метаданных»), 13.9 k more between 275 and 500 s. Their statement time is 129 s (6.3 ms each, against 0.84 ms on
the БСП: the buffer cache of the fresh clone was cold); the rest is the platform's own processing between the statements. In order:

| time | what |
|---|---|
| 0 - 245 s | check: 6.4 k single-row `Config` reads, then 30 s (245 - 275) without statements |
| 275.5 - 281 s | first writes: the `.new` copy of the 23 staged rows (3.8 s, `versions` is 4.0 MB stored), the ring `MobileVersions.datNEW` (once), `DBNames.New` (3.0 MB, first pass) |
| 281 - 344 s | reads: 1 492 `_ExtensionsRestruct` reads (on a base without extensions), 5 974 `_DataHistoryMetadata` probes (659 on the БСП), 111 aggregate-mode probes of accumulation registers, more `Config` bodies |
| 344 - 346 s | `_DbCopies*` housekeeping, `_ExtensionsRestructNGS` `DELETE` (0 rows), `DBNames.New` second pass |
| 346 - 492 s | 146 s of reads only: about 12 k more `Config` bodies («Сбор служебной информации») |
| 492.5 - 504 s | 16 x `.sinew` (25.5 MB written, 11.5 s) |
| 504.6 - 509 s | `commit`, promotion of `.si` and `siVersions`, promotion of the 23 rows (0.8 s), clean-up |
| 510 - 513 s | `.ui`, `ibparams.inf`, `locale.inf`; the help index promotion (all no-ops) |

### 12.3 Which path

The log has the long path's line «Обработка структуры базы данных…», and the rule of 6.4 predicted it (23 rows), but what the
apply **wrote** is the short path's set plus a few items (12.4): no `..NG` table, no `sp_rename`, no `SchemaStorage` walk
(`SchemaStorage` and `DBSchema` are only read), no `gc.mrk`, the ring written once, the register untouched. The register has
no rows on this base, so there is nothing to rebuild or to update: `UPDATE _ConfigChngR SET _MessageNo = NULL` is not even
issued (the БСП short path issues it). So "long" and "short" name the phases the platform runs; what they write depends on the
base: the register rebuild and the extension bookkeeping of the БСП long path need a register with rows and extensions.

### 12.4 Families beyond the БСП's, and the ones missing

`evidence/native-apply-uh/matrix-uh-vs-bsp.md` has one row per family against the БСП long, short and 8.5 applies.

**New on УХ** (in no БСП trace):

1. `Params ibparams.inf` rewritten (292 -> 324 B): the platform's own (8.3.27) long form, in place of the 8.5 short form of the
   corpus, and the two zero GUIDs of the outer group replaced by two new GUIDs; `Params locale.inf` (112 B): its two zero GUIDs replaced by
   **the same two GUIDs** (`150607a8-...`, `58a6e802-...`). The rows are written by `infobase create` with zeros
   (`native-infobase-create.md`); consistent with the first native apply on a base whose rows are still in the state `create` left them (`params-before-after.md`).
2. `Params <guid>.ui`: **two rows inserted** (94 B and 24.7 KB Base64 text), not updated; the БСП bases had them.
3. The **English** help index (`userDocs_en`, `userVocabulary_en`, `userPostings_en`) is handled next to the Russian one, but the
   promotions are no-ops on both (`DELETE ... AND EXISTS(<n>.new)` and the rename affect 0 rows): the help build wrote no `.new` rows, and the old
   index stays.
4. `_DbCopiesInfoBaseUse` replaced (`_Id` new, `_Description` `localhost:3541 : uha` -> `DESKTOP-SMI5N4O : DefAlias`, the computer name
   and the alias), two `DELETE`s and two `TRUNCATE`s on `_DbCopies*`: the same steps as on the 8.5 БСП. They are therefore **not an 8.5
   feature** (section 11.3, item 1 is corrected): they run on every apply of a base that has the `_DbCopies*` tables; the БСП 8.3.27 corpus is older and has
   no `_DbCopiesInfoBaseUse`.
5. Reads only: 111 aggregate-mode probes (`SELECT MIN(_AGGMODE), MAX(_AGGMODE), COUNT_BIG(*)`). The other reads exist on the БСП too but
   scale: 5 974 `_DataHistoryMetadata` probes (659), 1 492 `_ExtensionsRestruct` reads (268, here on a base without extensions), 115
   `IN`-list statements for 52 154 descriptors (23 for 4 916).

**In the БСП long path and missing here**: the register rebuild through `..NG` tables and its DDL (`_ConfigChngR` is empty), the
`SchemaStorage`/`DBSchema` walk and rewrite, `gc.mrk`, `CAS_GC_Info` and the ConfigCAS garbage collection, the `_ExtensionsRestruct` /
`_ExtensionsRestructNGS` inserts, the two extra rewrites of the ring, `DBNames-Ext-*`, the content of the help index (see 3), the `_MessageNo` update.

**The same as on the БСП**: the `.new` copy and the promotion in `Config`, `commit`, the 16 `.sinew` -> `.si` rows and `siVersions` (new
random guids; the 16 rows keep their decoded content), `DBNames.New` in two passes that end in `DELETE` (nothing changed), the ring
`MobileVersions.dat` (`{1,g0}` -> `{2,g1,g0}`), `DELETE DynamicallyUpdated` (no overlay here). End state (`diff-condensed.md`): no schema
change, 10 body rows and `versions` changed in `Config`, the 10 descriptors and `root`, `version` rewritten with the same content (only
`Creation`/`Modified` move), 23 `ConfigSave` rows deleted, the ring, `siVersions`, the 16 `.si` rows (same content), `ibparams.inf`, `locale.inf`, two new `.ui` rows, one
`_DbCopiesInfoBaseUse` row.

### 12.5 How big the CAS garbage collection is

**None on this base**: `ConfigCAS` has 0 rows, so the apply issues no `DELETE FROM ConfigCAS` and no `Files` clean-up, and leaves
`CAS_GC_Info` and `gc.mrk` alone. The collections measured on the БСП (12 160 and 18 627 single-row deletes, 74 s and 219 s) are the
content store of the **extensions**: a base without extensions has nothing to collect. The size for a УХ base with extensions is not
measured.

### 12.6 Recommendation for the apply track: what the own apply must write on УХ

In the terms of the table "Which per-apply writes are required" of `docs/apply/own-apply.md`:

| Native write on УХ | Verdict |
|---|---|
| `Config`: the 10 bodies and `versions` (4.0 MB stored, 9.1 MB decoded: one part, below the limit of 10 000 000 B), the 10 descriptors and `root`/`version` promoted with the same content | **required** (the rows); the promotion is 0.8 s for 23 rows |
| `_ConfigChngR._MessageNo := NULL`, registrations of new objects | **required only where the register has rows**: here 0 rows, native issues no update and inserts nothing. Update the existing rows of the owners; never insert registrations for existing objects; a new object on an empty register is **untested** (native probably registers at the nodes the register knows: none) |
| `Params` 16 `.si` + `siVersions`: 25.5 MB written, 11.5 s, content unchanged | **not required** (write only for new objects, as now): the largest avoidable cost of a native apply on УХ |
| `MobileVersions.dat` ring: `{2,g1,g0}` | keep, as now (80 B) |
| `DBNames.New` two passes over 3.0 MB, both discarded | **not required**: `DBNames` is unchanged for a body edit |
| `SchemaStorage`/`DBSchema` walk, `..NG` rebuild | **not run by native either** on this base |
| `gc.mrk`, `CAS_GC_Info`, ConfigCAS collection, `_ExtensionsRestruct*` | **not written by native**; nothing to do |
| help index | native wrote nothing; **not required** |
| `Params <guid>.ui` (two rows, created) | as in the БСП verdict: not written. The end state then differs from native's by **two rows that exist**; measure `config check`, a cold `ibsrv` session and an export on an own-applied twin before relying on it |
| `ibparams.inf`, `locale.inf` (identity GUID pair, own-format text) | **hypothesis: not required** (written by the first completed native apply of a base whose rows the platform did not write). Never copy the GUIDs from another base. Test on the twin like the `.ui` rows |
| `_DbCopies*` housekeeping, `_ExtensionsRestructNGS` `DELETE` | scratch / registration of the copy; **hypothesis: not required** |

**Answer: on УХ the own apply must write nothing beyond the minimal set it already has** (the promotion of the staged rows, the fold of
overlays when the base has any, the ring, the `_MessageNo` update of existing register rows - none here). Everything else native wrote
is in the rows marked "not required" or "hypothesis: not required", and the hypotheses need one test on a twin. Two things it must
**not** do: insert registrations for existing objects, and copy the identity GUIDs of `ibparams.inf` / `locale.inf` from another base.
What matters at this scale is not the write set (228 statements natively, a few dozen for ours) but the **check**: native spends
95 % of its 518 s before the first write or between write groups, reading 72 k of the 118 k rows (52 k descriptors in batches, 20 k bodies one by one); the own gate should not do
20 k single-row round trips (read the changed rows and their dependents set-wise), and a 4 MB `versions` row is copied and promoted
(the `.new` copy of the 23 rows, `versions` being 4 MB of it, took 3.8 s natively). The limit of 20 staged rows of 6.4 held at this scale (23 rows: the structure
line appeared); what happens below 20 on УХ (the bare variant stages 13 rows) was not run.

### 12.7 What the run does not show

Cases 2 and 3 on УХ (an attribute, a new object: the DDL of a 21 187-table base, a new object on an empty register); a second
(steady-state) apply, which would show the warm-cache time; a base with extensions or users; the bare 13-row stage; the 8.5 platform on УХ.
