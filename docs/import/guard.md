# The import guard (#388, checkpoint 2)

> Step 2 (`docs/import/override.md`) made the stage carry most of what this document lists as refused: the
> refusals of section 5 (attribute, tabular section, new catalog, removed form, ...) are cases of the acceptance
> at checkpoint 2, and now load. The guard is unchanged and checks every case; what it still refuses is the
> removal of a single file of an object that stays (a module, a picture).

`ibcmd infobase config import` into a database that holds the configuration stages in patch mode, and patch
mode carries only part of a tree (`docs/import/patch-mode.md`). The guard makes the rest impossible to lose
silently: before anything is written, the configuration the stage would leave is exported and compared with the
tree, and any difference refuses the import.

## 1. What it does

1. The stage builds its rows in memory, as before (patch mode: `mssql::stage_source_objects`).
2. **The state an apply would leave** is built in memory (`OfflineRows::with_staged`): the database's Config rows
   as the storage publishes them - the aliases of an active dynamic generation (`<id>_dynupdate_<id>`) under their
   plain names, its history row (`DynamicallyUpdated`) left out - with each staged row in place of the stored row
   of its name. Nothing is read from ConfigSave and nothing is written.
3. The **model export** runs on that state exactly as `infobase config export` runs on a database
   (`export_staged_state`), with a sink instead of the disk: each file goes to a comparer on the writer threads.
4. The comparer checks every file against the tree (below). `ConfigDumpInfo.xml` is not compared (it holds
   generation ids no stage keeps).
5. A difference refuses the import: exit -1, a Russian message, and the transaction that replaces ConfigSave is
   never reached, so **ConfigSave holds exactly what it held** (checked by fingerprint in the acceptance run).

The check needs the target's Config in memory once (БСП: 1.7 s) and one export (2.3 s); the numbers of every
corpus measured are in section 6.

## 2. What is compared

A file of the export against the tree's file of the same relative path:

| kind | how |
|---|---|
| identical bytes (the tree scan's sha256) | passes at once |
| `.xml`, `.xsd`, `.wsdl` | leaf by leaf: element path (`Name[i]` among siblings), attributes and trimmed text, as `source-diff-explain` does; formatting, attribute order and comments do not count |
| `.bsl`, `.txt`, `.html`, `.htm`, `.css`, `.js` | text after the byte order mark is dropped and CRLF becomes LF |
| anything else | bytes |

A file the export writes and the tree lacks (something the tree removed that the stage leaves in) and a file of the
configuration the tree has and the export does not write (something the stage does not carry) are differences
too. "A file of the configuration" is `Configuration.xml`, the metadata collections' folders and the
configuration's own `Ext`; a readme beside the tree is not one. With `--path-prefix` only the paths asked for are
compared.

The message names, per file, the first three differing leaves; an element only one side has is one line with its
`Name`:

```
Загрузка отменена: конфигурация базы после неё не совпала бы с деревом F:\...\tree (файлов с расхождениями: 1 из 12197). В ConfigSave ничего не записано.
  Catalogs/_ДемоПартнеры.xml: в дереве есть элемент MetaDataObject/Catalog/ChildObjects/Attribute[7] («ДемоНовыйРеквизит»), в собранной конфигурации его нет
Загрузка в базу, где конфигурация уже есть, переносит из дерева имя, синоним и комментарий объектов, модули, формы, макеты, права ролей и командные интерфейсы; ...
Что делать: загрузите эту конфигурацию штатным ibcmd (или Конфигуратором), либо в пустую базу. Ключ --base-free ... применяйте его только к базе, где это допустимо.
Чтобы всё равно загрузить остальное, добавьте --no-verify (перечисленное выше в базу не попадёт).
```

## 3. Defaults and switches

| | |
|---|---|
| patch stage (`infobase config import`, the target holds the configuration) | on |
| base-free stage (an empty infobase, or `--base-free`) | on |
| `--no-verify` | off for any stage |
| `--verify` | on (the default; for scripts that want it written down) |
| `IBCMD_RS_STAGE_VERIFY=0\|1` | off / on for any stage, over the command line (for scripts) |
| `IBCMD_RS_STAGE_ALL_ROWS=1` | a patch stage against a database stages every row of the tree, as before #395 (`docs/import/delta.md`) |
| `mssql-stage-source-objects` (the research command) | off; `--verify` turns it on |

Every stage is checked: the project fails closed, and a stage compiled from the tree can carry a slip of the
compiler as well as a patch stage can drop a change. The base-free check costs the export and nothing else. The
first version scanned the tree after the stage (a full read and hash of the tree's files: 152 s on ERP УХ, of a
stage of 121 s); a second read each file when the export produced it and was worse (263 s, the export's writer
threads wait on the disk). The version in use hashes the tree's files on the file-bound pool beside the export,
which spends its time in the model: 94 s (section 6).

A guard that cannot run (the generation history row unreadable, the export fails) refuses with the reason and the
way to skip it (`--no-verify`); it does not let an unchecked stage through.

## 4. The patch refusals

(At checkpoint 2. Since step 2 the first two are built from the tree; a reference to an object the tree has no file
of and an object no writer can build still refuse.)

A patch stage cannot build an object the database does not hold, predefined data with items the stored row lacks,
or a file whose reference names an object the tree has no file of. The stage collects **every** such object and one
refusal names them all, in Russian, before anything is written:

- a new object (`справочника нет в базе (в таблице Config нет строки <uuid>)`): use the platform's ibcmd, or
  `--base-free` where losing what the XML does not carry (business process flowchart counters, the always-used
  flag of constants) is acceptable;
- new predefined items: the same, with the items' names read from the tree;
- a reference to an object the tree has no file of (an exchange plan's content, a role's rights, ...): the tree is
  incomplete - `--base-free` refuses it too - so the message says to restore the file or remove the reference.

## 5. Acceptance on БСП 8.3.27 (`scripts/import-lab/run_guard_acceptance.ps1`)

The edits are those of `docs/import/patch-mode.md` section 3 (`scripts/import-lab/edits.py`), applied to an
exported БСП 8.3.27 tree and loaded through the drop-in `infobase config import` with its default flags into a
staging clone that is never applied. Before and after every run ConfigSave is fingerprinted (row count, a checksum
of every row's bytes, the last `Modified`); a refusal must leave the fingerprint as it was.

**Patch mode alone loses these silently; the guard refuses them (11 of 11).** Each is refused with exit -1 and an
unchanged ConfigSave.

| edit | change to the tree | files named |
|---|---|---|
| `attr` | a String attribute added to a catalog | 1: `Attribute[7] («…»)` is in the tree, not in the built configuration |
| `ts` | a tabular section added to a catalog | 1 |
| `prop` | `QuickChoice` flipped on a catalog | 1: `Properties/QuickChoice: в дереве «true», в собранной конфигурации «false»` |
| `attrprop` | `FillChecking` of an attribute set to `ShowError` | 1 |
| `attrdel` | the first attribute of a catalog removed | 1: the built configuration has it, the tree does not |
| `enumval` | an enumeration value added | 1 |
| `subsys` | a catalog added to a subsystem's content | 1: `Content/Item «Catalog.…»` |
| `nestsub` | the same in a nested subsystem | 1 |
| `confver` | `Configuration` `<Version>` bumped | 1: `Properties/Version: в дереве «3.1.11.467», в собранной конфигурации «3.1.11.466»` |
| `predefdel` | a predefined item removed | 1: `Ext/Predefined.xml`, `Item[3] («…»)` |
| `formdel` | a form removed (its files and the `<Form>` entry) | 6: the entry of the owner, and the five files the stage leaves in the database |

**Patch mode refuses these itself; the refusal is now Russian, complete and says what to do (6 of 6).** Every
problem of the tree is listed, not the first one.

| edit | change to the tree | what the message says |
|---|---|---|
| `newcat` | a new catalog | the catalog is not in the database (no Config row of its uuid); new objects are not loaded into a database with a configuration yet |
| `newform` | a new form of a catalog | the same for the form |
| `newtpl` | a new template | the same for the template |
| `predef` | a predefined item added | the predefined data has items the database lacks (the item is named) |
| `catdel` | a catalog removed (file and `Configuration.xml` entry) | 4 files still name it: 2 exchange plans' content, 2 roles' rights; restore the file or remove the reference |
| `catfile` | the file removed, the `Configuration.xml` entry kept | the same 4 |

**Patch mode carries these; the guard lets them through (5 of 5) and the unchanged tree too.** Exit 0, ConfigSave
staged, every file of the tree accounted for.

| edit | change to the tree | files identical | guard |
|---|---|---|---|
| `noop` | none (control) | 12 197 of 12 197 | 15.4 s (first run, cold) |
| `syn` | the synonym of a document | 12 197 | 4.4 s |
| `rights` | a right switched off in a role | 12 197 | 4.2 s |
| `ci` | the visibility of a command in a subsystem's command interface | 12 197 | 3.9 s |
| `module` | a comment line appended to a common module | 12 197 | 8.1 s |
| `predefedit` | the description of a predefined item | 12 197 | 7.4 s |

Runs: `acc2` (all 23 cases; the binary before the last two commits, which changed only the wording of the
messages) and `acc4` (the 17 refusals again with the binary of the last code commit; a refusal writes nothing, so
the 17 fingerprints are the same before and after, and the six carried cases need no second run). Results in
`out\guard-acceptance\<tag>.json`, the full refusal texts in `out\import-<tag>-<edit>.json`. An unchanged БСП 8.5
native tree (12 336 files) also passes, with a guard of 8.5 s.

## 6. Cost

| corpus, stage | files | guard | the stage it is added to | peak memory |
|---|---|---|---|---|
| БСП 8.3.27, patch, a real database (drop-in import) | 12 197 | 3.9-8.1 s (15.4 s on a cold first run): Config read into memory 1.7 s, export 2.3 s, comparison | the whole `config import` takes 5-80 s on the shared machine | not measured apart |
| БСП 8.5, patch, unchanged tree | 12 336 | 8.5 s | | |
| ERP УХ 8.3.27, patch, offline (`--script-only`, Config rows from a folder) | 140 708 | 98.5 s: rows read 5.1 s, export 93.1 s (+9 % of the stage) | 1 049.6 s wall, 1 411 s CPU, of which the guard 98.8 s | 6.7 GB |
| ERP УХ 8.3.27, base-free, offline | 140 708 | 94.3 s: export 92.7 s, the tree hashed beside it (an earlier version that scanned the tree after the stage: 152 s) | stage 120.6 s; the process 286 s wall, 1 103 s CPU | 8.3 GB |

Offline means no database is touched: `scripts/import-lab/run_uha_guard.ps1` (inside the heavy lock) runs
`mssql-stage-source-objects --script-only --verify`. The database-backed patch stage of УХ holds all Config rows
in memory for the guard, which the offline run does not (it reads them from the folder as it goes): its peak has not
been measured. The wall figures come from a machine shared with other work; the CPU figures are the steadier ones.

The default follows from this: on БСП the guard is a few seconds and on ERP УХ a tenth of a patch stage and
94 s on a base-free stage of 121 s, so every stage is checked.

## 7. Limits

- The guard compares with what the model export writes. Where the export is not at parity with the platform for a
  construct, an unchanged native tree is refused for that file (measured on unchanged native trees: БСП 8.3.27, 0 of
  12 197 files differ; БСП 8.5, 0 of 12 336; ERP УХ 8.3.27, 0 of 140 708, in both stage modes). `--no-verify` is the
  way out, and the message says so.
- A stage into a database holds all of its Config rows in memory while the guard runs (the offline УХ run reads
  them from a folder instead): on ERP УХ that is the figure of section 6 plus the rows, not measured yet.
- It sees only what the XML carries. State the XML does not (flowchart counters, always-used flags) is not
  compared; `docs/import/patch-mode.md` section 4.1 lists it.
- It refuses; it does not make patch mode carry the change. That is step 2 of the proposal (descriptor override,
  new objects, `deleted`).
