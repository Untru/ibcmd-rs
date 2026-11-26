# S1-K: the chain with OUR import (#407)

Part of #391 (the minimal own restructuring, S1). The chain: an edit of the БСП 8.3.27 XML tree, then our
`ibcmd infobase config import` into an existing base, then our apply (`mssql-config-apply --allow-restructure s1
--i-have-a-backup`), compared with the native chain (native import, native `config apply`) on a twin, by the protocol of
`docs/apply/restructuring.md` 12.6. Track "ext" from 2026-09-30 (it was import's). Two stops: **checkpoint 1** (this
document): the kit, the cases, and what our import does today on `feat/0.4`; **checkpoint 2**: the real run, after the
import track's step 2 of #388 (`feat/0.4-import-override`) and the restructure-check track's wiring of the drop-in
apply to the S1 gate (`feat/0.4-dropin-s1`) are merged.

Everything below is measured on the БСП 8.3.27 corpus clone (four extensions), platform 8.3.27.2214, SQL Server 2025.

## 1. The kit

`scripts/restructure-lab/s1k/` (the ddl track's tools are used as they are: `snapshot.py`, `snapdiff.py`, `si_diff.py`,
`bracefmt.py`, `lab.py`, `db.py`, `session_job.ps1`, the session jobs in `jobs/`). The lab is `F:\ibcmd\lab\05\ext\s1k`
(`S1K_LAB`): `tree\` (a copy of the reference native export, made once with `robocopy`), `bak\base_applied.bak` (the clone
after a native baseline apply: an unchanged file staged and applied, which also folds the `_dynupdate_` rows a corpus clone
carries), `out\<case>\` (the edit, the reports, the record), `snap\` (the snapshot store; never the ddl track's).

| file | what |
|---|---|
| `cases.py` | the case list and the editors (`list`, `edit <case> <out>`); the editors are the ddl track's (`edit_cases_s1.py`, `edit_cases_s2.py`) or small ones of this kit; they write only the edited files under `<out>/stage/`, the originals under `<out>/before/` |
| `overlay.py` | puts a case's edited files over the working tree and takes them off again (`apply`, `restore`, `verify`), so one tree serves every case |
| `import_phase.ps1` | one case, the import phase: the edit, a twin of the base, OUR import (default mode), what it left in `ConfigSave` (`record.py`), OUR apply as a dry run over that stage |
| `run_phase1.ps1`, `assemble_phase1.py` | every built case in a row; the record of all of them as one text (`evidence/restructuring/s1k-checkpoint1.txt`) |
| `twin_case.ps1` | the twin protocol of one case: the native side (native `import files --partial` of the edited files, native apply), OUR apply on our twin (dry run, rehearsal, real run), checks 10, 3, 4, 5, 6, 7, 8, 9 |
| `compare_twins.py`, `check4.py`, `checks56.py`, `check8.py` | EXCEPT both ways over tables (and every table of the extension schema); `Config` row by row; `DBNames`, `DBSchema` entries and the `.si` rows; the native exports |
| `native_main.ps1` | native `ibcmd` on a lab clone: `ImportFiles`, `Apply`, `Export` (the writes under the native lock, one command per hold) |
| `digest.ps1` | the digest of a database that checks 11 and 12 compare before and after (rows of `Config` / `ConfigSave` / `Params`, `SchemaStorage`, `DBSchema`, `DBNames`, every column and index, the `*NG` tables); the ddl track's own, for `ibcmd_rs_05_ext_s1k_*` |
| `check11.ps1` | the refusal of a case (i1): OUR apply, dry run and real run, must refuse naming the objects and the adoption and leave the digest as it was; `-Native` also runs the platform apply on the same twin, for the record |
| `check12.ps1` | the injected failure: the script our real apply made (`out\<case>\script_real.sql`) on a fresh twin of the staged state (`staged.bak`, taken by `twin_case.ps1` just before the real run) with a `THROW` before `COMMIT`; the digest must be unchanged |
| `run_checkpoint2.ps1` | every built case through the whole chain (`import_phase.ps1`, `twin_case.ps1` with the case's session job, `check12.ps1`; i1: `check11.ps1`), one log per case, the summary at the end |
| `native_side.ps1`, `assemble_checkpoint2.py` | the native side of each case started as its edit appears (the native lock has a long queue), and the record of all cases as one text |
| `new_objects.py`, `skeleton_catalog.xml` | cases f1 and f2: a new catalog and a new document with attributes (the trace track's N1 and N4 shapes, from a catalog the platform made and from `_ДемоОприходованиеТоваров`); `overlay.py` puts a new file in the working tree and takes it out again |
| `check_register.py` | the change register (`_ConfigChngR`, its file lists) of the twins, without the random keys |

## 2. The cases

The edits are the ones the S1 operations were traced and twin-verified with, so a difference between the chains is the
chain's, not the edit's. Objects that an extension adopts are avoided by the cases of the built operations (S1-I refuses
them) and used by case i1.

| case | operation | edit | session job |
|---|---|---|---|
| a1 | A: add attributes | attributes of every primitive type on `КлючевыеОперации`, `Удалить_ДемоОбщиеСведения` and the document `_ДемоЗаказПокупателя` (the ddl track's types case without its three adopted catalogs) | `jobs/s1k_a1.bsl` (`types_t1.bsl` of the ddl track on those three objects) |
| b1 | B: delete attributes | the middle, the last, the first, an indexed one, the only one, in a hierarchical catalog, in a document, delete + add (6 objects); **the forms of these objects lose what binds the deleted attributes** (see below) | `jobs/s2_b1.bsl` |
| b2 | B | an attribute with the additional-order index (catalog `_ДемоПроекты`, document `_ДемоНачислениеЗарплаты`); its list form loses the binding too | `jobs/s2_b2.bsl` |
| c1 | C: widen a string | six strings in five objects (catalog, hierarchical catalog, document, indexed, up to 1024) | `jobs/s2_c1.bsl` |
| d0 | D: the index flag | one flag alone | `jobs/s2_d0.bsl` |
| d1 | D | on and off, additional order, string and number, hierarchical and flat catalog, document | `jobs/s2_d1.bsl` |
| i1 | I: an adopted object | a string attribute on `_ДемоПартнеры` and `_ДемоНоменклатура` (adopted by `_ДемоРасширение`, the second with `_Reference18X1`): S1-I must refuse | - |
| e1 | E: add a tabular section | not built in the gate yet; the editor comes with the operation | - |
| f1 | F: add a catalog / a document | not built yet; the same | - |

**The forms of b1 and b2.** The ddl track's edit deleted the attributes in the object files alone and staged only those
files natively, so the forms that bind the attributes were never part of what native saw. A whole tree is another matter: a
form whose item binds an attribute the object no longer has is a tree the platform refuses (the coordinator's finding, and
what import's b1 failure at the form pack is near to). From checkpoint 2 the editor of b1 and b2 (`cases.py`,
`with_form_unbinding`) takes out of every form of the object what binds each deleted attribute: the items whose `<DataPath>`
is `Объект.<attr>` / `Список.<attr>` with their menus, tooltips and children, the `<Field>Список.<attr></Field>` lines of a
dynamic list, the items of the list settings that name the field (`dcsset:field`, an order or a filter), and the field of
a manual query text (`<alias>.<attr>,`). For b1: ten form items, one field line and two query lines in nine forms (the
forms are added to the edited files); for b2: one form item, one order item of the list settings, one field line and one
query line in one form. The stage of the native partial import of the whole b1 set, forms included, is accepted (exit 0);
the native apply of it is in the record of checkpoint 2. What is left of the attribute's name in a form is the word in a title
(`СтраницаКомментарий`, `Комментарий`), which binds nothing. The failure of b1 in the form pack of the import stays a
separate note to the import track if it persists after the override.

## 3. The protocol for two chains that stage apart

12.6 makes the twins from one backup of the staged state, so that native and ours start from the same `ConfigSave`. Here
the point is the two stagings, so each twin is staged by its own import and two things differ by construction:

- **Check 4** (`Config`): the rows of an import carry the `Creation` / `Modified` of that import, and the `versions` row
  the generation of that stage (`root` and `version` too). `check4.py` counts the rows equal in every column, equal in
  the bytes with other dates, equal after inflate, and different in content, and names the last: on twins staged apart by
  the same native import (the harness validation below) it finds 9 837 equal, 3 with other dates and `versions`.
- **Check 8** (the native export): `ConfigDumpInfo.xml` prints a `configVersion` for every object, the generation that
  staging gave it, so it differs for exactly the changed objects. `check8.py` compares every file byte for byte and
  `ConfigDumpInfo.xml` with the `configVersion` values taken out, and lists the objects whose version differs (12 198
  files, one object on the validation).

All the rest is as 12.6: check 3 (EXCEPT both ways over the rebuilt tables and the 243 `X1` tables of the extension schema),
5 (`DBNames` text equal after inflate, `DBSchema` entries equal but `DbCopies*`), 6 (the 16 `.si` rows), 7 (a native apply
afterwards says «не требуется»), 9 (a cluster session on both), 10 (a rehearsal changes nothing), 11 (the refusals), 12 (an
injected failure inside the transaction; the ddl track's `inject_failure.ps1` on the script `twin_case.ps1` writes).

## 4. Checkpoint 1: what our import does today

`feat/0.4` 847f581a, the binary built from it, the import in its default mode (no `--base-free`, no `--no-verify`), the
tree of each case over a base that holds the same configuration. Record: `evidence/restructuring/s1k-checkpoint1.txt`.

| case | our import | in `ConfigSave` | our apply, dry run |
|---|---|---|---|
| a1 | exit -1 in 13.5 s: «Загрузка отменена: конфигурация базы после неё не совпала бы с деревом ... (файлов с расхождениями: 3 из 12197). В ConfigSave ничего не записано.» The differences listed: the new attributes are missing from the assembled configuration | 0 rows | nothing to apply |
| b1 | exit -1 in 14.5 s **before the guard**: `failed to pack Form body ...\_ДемоСтавкиНДС\Forms\ФормаЭлемента\Ext\Form.xml: failed to patch Form layout properties: cannot patch Form UseForFoldersAndItems without an existing property bag`. Bisected: the edit of `_ДемоСтавкиНДС` alone fails so; the other five objects fail on the form of `ТелефонныйЗвонок` (`failed to patch Form layout AutoCommandBar: ... direct child item entry`) | 0 rows | nothing to apply |
| b2 | exit -1 in 8.4 s: guard, 2 of 12 197 files (the attribute of `_ДемоПроекты` and of `_ДемоНачислениеЗарплаты` is still there in the assembled configuration, with its `Indexing` and uuid) | 0 rows | nothing to apply |
| c1 | exit -1 in 8.3 s: guard, 5 of 12 197 (`Length` in the tree 200 / 20 / 100 ..., in the assembled configuration 50 / 12 / 40 ...) | 0 rows | nothing to apply |
| d0 | exit -1 in 8.9 s: guard, 1 of 12 197 (`Indexing` in the tree `Index`, in the assembled configuration `DontIndex`) | 0 rows | nothing to apply |
| d1 | exit -1 in 11 s: guard, 6 of 12 197 (`Indexing`) | 0 rows | nothing to apply |
| i1 | exit -1 in 19.7 s: guard, 2 of 12 197 (the new attribute of `_ДемоНоменклатура` and `_ДемоПартнеры` is missing) | 0 rows | nothing to apply |

So today: **five cases reach the guard of #388 and are refused** with the differences named and nothing written (the guard
does its job; no case loses a change silently), **one (b1) fails earlier**, in the packing of forms of the edited objects,
with an error that has nothing to do with the deletion the case makes (the forms are not edited). Nothing stages, so our
apply has nothing to do and the chain cannot yet be compared: that is checkpoint 2, after step 2 of #388. The guard's own
words offer `--base-free` (loses what the assembly from the tree does not carry) and `--no-verify` (the listed changes
"do not reach the base"); neither is used.

The message of the guard for the built operations names exactly the changes the operations are made of (a new attribute, a
removed attribute, a longer `Length`, another `Indexing`), which is what step 2 has to assemble.

### The rest of the harness, validated on d0 (not a result about our import)

`twin_case.ps1 -Case d0 -StageOwnNative` stages our twin with the native partial import (so nothing here says anything
about our import) and runs everything after it, through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`:

- dry run and rehearsal: exit 0; check 10: the rehearsal changes nothing (`changed (0)`);
- real run: `catalog КлючевыеОперации (Reference2598): switched indexes Fld2646 = ЦелевоеВремя (DontIndex -> Index)`;
- check 3: `_Reference2598` and the 243 `X1` tables equal (EXCEPT both ways); check 5: `DBNames` equal after inflate,
  `DBSchema` equal but `DbCopies*`; check 6: 16 of 16 `.si` rows; check 7: «Обновление конфигурации базы данных не
  требуется»; check 8: 12 198 files, `ConfigDumpInfo.xml` differs in the `configVersion` of `Catalog.КлючевыеОперации` only;
  check 9: the session outputs of the two twins identical (8 lines);
- check 4: 9 837 rows equal, 3 with other dates, `versions` differs in content (the generation): the reason of section 3.

## 5. Checkpoint 2: the real chain (2026-09-30)

Our import of the edited tree (the default mode: a patch on a base that holds the configuration, the override of #388 step 2, the
guard on), then the drop-in `ibcmd infobase config apply --recovery-backup=<file>`, against the native `import files --partial` of
the edited files + native `config apply` on a twin, by the protocol of 12.6. The binary: `feat/0.4` 029b4a2b with this kit
(merge aaa97b39) for a1, b1, b2, c1, d0, d1, e5, e6, i1; `feat/0.4` eb315250 (S1-F) with this kit (merge c22d0bef) for f1 and f2.
The record: `evidence/restructuring/s1k-checkpoint2.txt` (per case: the raw lines of every check).

| case | operation | our import | our apply (structure) | 3: rebuilt + 243 `X1` tables, EXCEPT both ways | 5 | 6 | 7 | 8 (12 198 / 12 199 files) | 10 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|
| a1 | A: add attributes | exit 0, 3 objects staged | new attributes of `КлючевыеОперации`, `Удалить_ДемоОбщиеСведения`, `_ДемоЗаказПокупателя` | 252 tables, 0 differ | equal | 16 of 16 | not required | 0 differ | no change | unchanged |
| b1 | B: delete attributes (forms unbound) | exit 0, 6 objects + 9 forms | attributes removed in 6 objects, one added and one replaced beside them | 254, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| b2 | B: the additional-order index | exit 0 | `_ДемоПроекты`, `_ДемоНачислениеЗарплаты` | 250, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| c1 | C: widen a string | exit 0 | six strings in five objects | 256, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| d0 | D: the flag alone | exit 0 | `КлючевыеОперации`: switched indexes | 247, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| d1 | D | exit 0 | on and off, additional order, 6 objects | 257, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| e5 | E: new tabular sections | exit 0 | new sections in five objects | 259, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| e6 | E: attributes of sections | exit 0 | attributes of sections in three objects | 252, 0 | equal | 16 of 16 | not required | 0 | no change | unchanged |
| f1 | F: a new catalog | exit 0 (9 521 rows) | `ДемоКатФ1 (Reference11034)`: new catalog, `_Reference11034` | 247, 0 | equal but the platform's own system tables (below) | 15 of 16 (`c4629235`) | not required | 0 | no change | unchanged |
| f2 | F: a new document | exit 0 (9 521 rows) | `ДемоДокФ2 (Document11034)`: new document, its tables | 248, 0 | same | 15 of 16 (`c4629235`) | not required | 0 | no change | unchanged |
| i1 | I: adopted objects | exit 0 | refused: `S1: catalog _ДемоПартнеры is adopted by the extension _ДемоРасширение ...` | - | - | - | - | - | - | check 11 below |

- **Check 9** (a cluster session on both twins, outputs compared): a1 identical (58 lines), f1 identical (21 lines). Asked for once;
  the session jobs of b1, b2, c1, d0, d1 and e exist (jobs/) and run with -SessionCases; for those cases the applied state is proved equal by checks 3, 5, 6 and 8.
- **Check 11** (i1): the dry run, the real run of `mssql-config-apply --allow-restructure s1 --i-have-a-backup` and the drop-in
  `infobase config apply --recovery-backup` all exit 1, name `_ДемоПартнеры` and the adoption (the drop-in in the platform's
  words, `требуется штатный config apply: ...`), and the digest of the database is the same after all three. The refusal stops at
  the first adopted object of the stage. The platform's apply on the same twin accepts the stage (16 minutes: it rebuilds the
  extension's tables too), which is what the refusal hands over to.
- **Check 12**: for every applied case a `THROW` before `COMMIT` of the dry-run script on a fresh twin of the staged state (the
  backup the drop-in apply took as its way back) leaves the digest unchanged: A (a1), B (b1, b2), C (c1), D (d0, d1), E (e5,
  e6), F (f1, f2).
- **Check 4** (`Config` rows), recorded and expected: 321 rows equal in every column, 357 equal in bytes with other dates,
  about 6 095 equal after inflate, and 3 066 to 3 071 rows of different content: our import re-compiles 3 061 rows of the base
  (`other_rows_differing_from_config` of the record) and stages 9 520 rows where the native partial import stages the edited
  files; the exports of both twins are identical (check 8). It goes with #395 (stage only the changed rows).
- **Check 8**: 12 198 files (12 199 with a created object), none differs, none is missing; `ConfigDumpInfo.xml` is equal but for
  the `configVersion` of 9 514 (9 515) objects, the generation our stage gave the 9 520 rows it staged. The same cause as check 4.
- **The change register** (`_ConfigChngR`, `check_register.py`; the trace track's check of S1-F, run for every case): the rows
  (node, object) are equal in every case (20 685; 20 688 with a created object), the file lists equal in every case but one.
  Native writes `NULL` `_MessageNo` to fewer rows than ours leaves (the documented drift: "NULL in native, not in own" is 0 in all).
- **F and the platform's own tables.** The native apply of a stage that creates an object also creates four system tables of its
  build (`_DbCopiesInfoBaseUse`, `_DbCopiesUpdateStat`, `_DbCopiesUpdateTableStat`, `_WebSocketClients`), which this apply does not
  (the trace track's documented drift, `new-object.md` 8). So `DBNames` and `DBSchema` differ by exactly those entries (three names
  appended, the header count 3 higher; the entries of the four tables), and `checks56.py` tolerates them. Check 6: 15 of 16 `.si`
  rows, the one that differs is `c4629235` (same length, another order of the same text: `derived-caches.md` 4).

**Found by this run.**

1. **b1, the change register.** For four forms of the stage (`КлючевыеОперации/ФормаЭлемента`, `КлассификаторБанков/ФормаЭлемента`,
   `КлассификаторБанков/ФормаВыбора`, `ТелефонныйЗвонок/ФормаДокумента`) the file list of the registration in `_ConfigChngR_ExtProps`
   has the two rows in the other order: native position 0 = `<uuid>.1`, position 1 = `<uuid>.0`; ours `.0`, `.1`. Forty rows
   differ each way; everything else of the register is equal. No earlier case staged a form with two rows. To the apply track.
2. **b1 needs its forms.** The platform refuses the tree of the original b1 (nine forms bind the removed attributes, «Неверный путь
   к данным»); the kit's b1 and b2 take the bindings out of the forms (`cases.py`, `with_form_unbinding`), and the native
   partial import and native apply of that set are accepted.
3. **Our import's b1 failure at the form pack is gone** with the merge (the forms of the edited objects pack).
4. **Check 5 for F needs the platform's tables tolerated** (above); `checks56.py` now does.

The lab: `run_checkpoint2.ps1` (with `native_side.ps1`, which starts the native side of each case as its edit appears, and
`twin_case.ps1 -SkipNative`) ran the cases in parallel while the native lock was held by other tracks: the native apply waits
in the queue were the main cost (about five hours for eleven cases, run in parallel). Databases and backups are dropped; `s1k\bak\base_applied.bak`
and `s1k\tree` stay.
