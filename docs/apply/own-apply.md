# The own `config apply` (milestone 0.4, issues #337 and #339)

`ibcmd-rs mssql-config-apply` moves what `ibcmd-rs infobase config import`
staged in `ConfigSave` into `Config`, the way the platform's exclusive
`ibcmd infobase config apply --force --dynamic=disable` does, without the
platform. It applies a configuration that needs **no restructuring**: changed
modules, forms, templates, pictures and help pages of any object, and a **new
form or template of an existing object**, from a full stage or a delta stage (a
few objects and `versions`). Anything that changes the database's structure is
refused with the list of the rows that would (use the native apply for it), unless the
caller asks for the restructuring seam (`--allow-restructure s1`, see
[Restructuring inside the apply](#restructuring-inside-the-apply-s1-a-397); the gate that
fills the seam comes with track ddl).

**Which stages it takes: those of this repository's importers only.** The stage
of the platform's own `ibcmd infobase config import` is refused: it carries a
`deleted` row (a list of removals, written to every stage), about 600 descriptors
rewritten in another record shape (format 56 to 57, a `{68}` Configuration row),
new version guids for every name and values above 10 MB cut into parts. The plan
stops at a `deleted` row that names anything but the rows of a dynamic update and of the forms and templates it can
account for name by name (since #393; before, anything but the dynamic rows), with
a message that says so; the conservative gate would stop at the descriptors. The
restructure check of track rcheck (#338, `apply_check::check_staged`, the default gate
since checkpoint 2) refuses such a stage as unknown, too. Measured on the stages the
import and rcheck tracks left in the lab: `ibcmd_rs_04_import_bsp_nat` and
`ibcmd_rs_04_rcheck_nat_a2` (9 846 and 9 842 rows, each with a `deleted` row) are
refused at once, read-only.

Platform: Microsoft SQL Server; 8.3.27 (profile `platform-8.3.27.2214`) and, since
checkpoint 2, 8.5 (profile `platform-8.5.1.1150`, capability `mssql.config.apply`;
[measured](#85-392) on the БСП 8.5 corpus; new forms, templates and bodies stay 8.3.27
only). The `Params` `.ui` rows
(the platform's configuration-licensing records, track ui #340) are **never
written** by this apply; see [known differences](#known-differences-from-the-native-apply).

**Checkpoint 2 (#392, #393, #397) in short.**

- The restructure check of track rcheck is the **default gate** (`--gate apply-check`);
  the conservative rule is the explicit option ([the default gate](#the-default-gate)).
- A staged `deleted` row that is **empty** or names **all** the dynamic-update rows of the
  base is consumed as the native apply consumes it ([removals](#removals-the-stages-deleted-row));
  since #393 a list that names the rows of removed forms and templates is executed too, and any list the apply
  cannot account for name by name is refused whole.
- **Typed refusals** for callers that sort them ([refusals](#refusals-a-caller-can-sort)).
- The recovery artifact is **one file** and the newest five per database are kept
  ([recovery](#recovery-artifact-and-its-retention)).
- A **restructuring seam** in the apply's own transaction and the **backup policy** of
  structural applies ([S1-A](#restructuring-inside-the-apply-s1-a-397)).
- Measured on the **ERP УХ clone** (118 377 rows, 4.3 GB; [scale](#erp-uh-8327-at-scale-392)):
  300 edited modules 34.8 s against 639.8 s natively, Config equal row for row; a stage of the
  whole configuration (1.6 GB) 699 s with a log peak of 9.2 GB.
- **8.5** admitted ([8.5](#85-392)).

**#412 (exchange plans of distributed infobases).** The native apply registers a changed object at **every
node of the plans that register changes**, inserting the row where the node has none (a node with an initial
image has none); this apply only updated the rows that existed, so the message for such a node carried none
of the change. Fixed and proved against the native messages ([exchange plans](#exchange-plans-of-distributed-infobases-412)).

Contents: [what the native apply does](#what-the-native-apply-does-measured) -
[what this apply does](#what-this-apply-does) -
[what it does not write](#what-it-does-not-write) -
[known differences](#known-differences-from-the-native-apply) -
[which writes are required](#which-per-apply-writes-are-required) -
[safety](#safety) - [the structural gate](#the-structural-gate) -
[the default gate](#the-default-gate) - [refusals](#refusals-a-caller-can-sort) -
[recovery](#recovery-artifact-and-its-retention) -
[restructuring seam and backup policy](#restructuring-inside-the-apply-s1-a-397) -
[exchange plans](#exchange-plans-of-distributed-infobases-412) -
[command line](#command-line) - [verification](#verification) -
[ERP UH at scale](#erp-uh-8327-at-scale-392) - [8.5](#85-392) -
[limits and open points](#limits-and-open-points).

## What the native apply does (measured)

Measured on 8.3.27.2214 with SQL Server Extended Events (`rpc_completed`,
`sql_batch_completed`, `sql_transaction`) and full before/after row-level
snapshots of every service table, on twins of one БСП clone that differ only in
who applied (`ibcmd_rs_04_apply_bsp8327_*`, lab `F:\ibcmd\lab\04\apply`).
Scenarios: S1 a base-free stage with a new form and a new template; S2 a
patch-mode stage of 227 module/form/template edits (9 517 rows); S3 a delta
stage of three objects (30 rows); S2b a second apply with nothing staged; S4 a
delta stage without `root` and `version` (16 rows: a new form, a new template
and their owner, ten module edits, `versions`); S5 delta stages of the other
shapes of new objects (see below).

Order of the native apply (S2/S3, session numbers omitted):

1. Reads: `commit`, `DynamicallyUpdated`, `convertPhase`, `deleted`,
   `erase_save`, `versions_dynupdate_*`, `root`, `version`, the whole
   `ConfigSave` listing, the user list. Nothing else: **no application lock,
   no session check, no single-user switch** (only `SET LOCK_TIMEOUT 20000`).
   A standalone `ibcmd` does not enforce exclusive access at the SQL level.
2. `Params.<uuid>.ui` (two rows): `DELETE ... PartNo <> 0`, `UPDATE ... BinaryData`
   in one user transaction. The value is a base64 text (encrypted); the same
   size, all bytes different.
3. `DELETE FROM ConfigSave/Config WHERE FileName LIKE '%.new'`, then for every
   staged row `F`, two autocommit statements: `DELETE FROM Config WHERE
   FileName = 'F.new' AND EXISTS (SELECT 1 FROM ConfigSave WHERE FileName = 'F')`
   and `INSERT Config SELECT 'F.new', Creation, Modified, Attributes, DataSize,
   BinaryData, PartNo FROM ConfigSave WHERE FileName = 'F'` (all parts). The
   data never leaves the server. ~20 000 statements for 9 517 rows.
4. `MobileVersions.datNEW` in `Files`; `DBNames.New` / `DBNames-Ext-*.New` in
   `Params` (rewritten with unchanged content).
5. The restructuring framework, on the **long path** (S1, S2, S3; the trace track's cases 1x, 2a, 2c): even
   when nothing is structural it
   rebuilds `_ConfigChngR` and `_ConfigChngR_ExtProps` as `..NG` tables (new
   `_IDRRef` for every row, the staged objects' `_MessageNo` set to NULL), fills
   `_ExtensionsRestructNGS` when extensions exist, and only in S1 restructured
   the route-point table of a business process. Around it `SchemaStorage`
   (SchemaID 0) walks `Status` 100 -> 200 (`UPDATE ... SET NewGenCreated = <1 KB
   of table text>, Status = 200`) -> 400 -> 500 -> 100 (`UPDATE ... SET Status = 100,
   CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3`, 977 KB) and
   `UPDATE DBSchema SET SerializedData = @P1` writes the schema blob again: **both
   are written in every apply, but with the bytes they already had** (S2, S3:
   `DBSchema` and `SchemaStorage` hashes equal before and after; they change only when
   a table does, as in S1).
6. The extension CAS garbage collection (`ConfigCAS` 12 797 -> 636 rows,
   `Files.CAS_GC_Info`, `extd_props_cached/gc.mrk`) and the help index
   (`Files.userDocs_ru*`, `userPostings_ru*`, `userVocabulary_ru*`).
7. Recovery marker: one row `commit` in `Config`.
8. `Params.*.si` service-information rows (16 on БСП) written as `.sinew` and
   renamed, `siVersions` with new version guids.
9. `DELETE FROM Params WHERE FileName = 'DynamicallyUpdated'`; a dynamic
   generation from an earlier dynamic update is **folded into the ordinary
   rows**: for every alias `X_dynupdate_G[.n]`, the ordinary `X[.n]` is
   deleted and the alias renamed to it; `versions_dynupdate_G` and
   `deleted_dynupdate_G` are deleted.
10. For every staged row `F`: delete `Config.F`, rename `F.new` to `F`
    (autocommit again, ~10 000 statements, the crash-safe part is the `commit`
    marker). `root`, `version`, `versions` last. `Config.DynamicallyUpdated`
    deleted.
11. `DELETE FROM ConfigSave` (all rows), `MobileVersions.datNEW` renamed over
    `MobileVersions.dat`, the markers `commit`, `dynamicCommit`, `dbStruFinal`
    deleted (the last two are never written by an exclusive apply), the help
    index files renamed.

Consequences measured on the end state:

- a multi-part `Config` row (10 MB parts) becomes one `PartNo = 0` row: the
  staged row is copied as it is;
- the staged rows' `Creation`/`Modified` are kept, `Attributes` too;
- a `ConfigSave` that is empty makes the native apply print "Обновление
  конфигурации базы данных не требуется" and change nothing (5.6 s);
- the new generation the native apply prints is the header GUID of the staged
  `versions` row (byte-swapped) followed by `00000000`.

### The short path (S4; the trace track's case 2n)

Not every stage takes the long path above. The S4 stage (traced with the kit of track trace, 195 write
statements, 24 user transactions, 75 s, `docs/apply/evidence/own-apply/s4-write-families.md`) ran no
structure phase, no `..NG` tables (no DDL at all), no `DBNames` passes, no ConfigCAS garbage collection:
`.ui` (3 rows), the `.new` copy of the staged rows, `MobileVersions.datNEW`, the 16 `.sinew` rows, the `commit`
marker, the promotion of `.si`, `dbStruFinal`, `UPDATE _ConfigChngR SET _MessageNo = NULL` (40 rows) with six
`INSERT`s into `_ConfigChngR` and six into `_ConfigChngR_ExtProps` (the two new objects, three nodes each),
the promotion of the staged rows, the clean-up and the help index. `_IDRRef`, `DBSchema` and `SchemaStorage`
kept their bytes. What decides the path is in the trace track's section 6.4 (`docs/apply/native-apply-trace.md`,
37 native applies): a staged descriptor row whose content differs from `Config`'s gives the long path, and so
do more than 20 staged rows. Seven stages of this track agree with the second part (S1, S2 and S3 have more than
20 rows and are long; S4, S5A and S5C have fewer and are short) and with the first part for a **catalog**
(S5B: 4 rows, the catalog owner's descriptor gains a form reference, long). They do not agree with the first part
as written for a **data processor**: S4 (16 rows) and S5A (15 rows) stage owner descriptors that differ from
`Config`'s -- the active text plus the references to the new forms and templates, checked by the gate --
together with the descriptors of the new objects, and both stayed on the short path. So the rule holds for
objects that own tables (catalogs and documents, which the native `import files` also re-saves in another
serialization; every experiment of the trace track with a changed descriptor is of that kind), not for a
data processor's, form's or template's descriptor. This is for the trace track (question 1). This apply
does not care: it never renumbers ids and writes the same end state for both paths (S3 on the long path,
S4 on the short one, both equal).

A **third path** was seen for a stage of `versions` and one body row (S5C, a module added to a data
processor, 25 s): no «Сбор служебной информации», no help-index build, `Params.DynamicallyUpdated` and the
`.si` rows left alone; `.ui` (3 rows), `MobileVersions.dat`, `Config` (with the fold of the dynamic
overlay) and the change registrations are written. This apply reproduces it by writing `.si` only for new
objects and the `Params` marker deletion only for stages with a descriptor row. The same paths, by owner
kind, in S5: a data processor's or a form's descriptor (short path), a catalog's (S5B: the long path, the
register rebuilt).

### What a new form or template adds (measured: S1, S4, S5)

Besides the staged rows themselves (the two descriptors, their `.0` bodies and the
owner's descriptor, which lists the new uuid in its forms or templates group with
the group's count raised):

- **`_ConfigChngR`**: one row per ordinary exchange-plan node and new object, `_MDObjID` =
  the uuid in the platform's byte order, `_MessageNo` NULL, a fresh `_IDRRef` (the platform
  takes them from a clock-based sequence; any unique 16 bytes do). The nodes are those of the
  exchange plans the table already holds rows for, **except the plan's own node**
  (`_PredefinedID <> 0` in the plan's node table `_Node<n>`, `n` = the `_NodeTRef` as a
  number): in the БСП demo three of five registered nodes (two DIB nodes and a DIB node with
  a filter); the plan's own node and the offline-work plan's own node get no row.
- **`_ConfigChngR_ExtProps`**: the new object's file list, one row per node row:
  `(_KeyField 0, '<uuid>.0')` (a form's help page `.1` follows as key 1). The list of an
  *existing* object is historical, not derived: one form in the demo lists `.1` before `.0`
  although only `.0` exists. A body row an existing object gains is therefore *appended*
  (key = last key + 1).
- **`_MessageNo` reset**: every object that owns a staged row, not only those with a staged
  descriptor: the object whose uuid starts the staged name, or the object whose file list
  names it (the configuration's own module `f389d417-...0` resets the configuration
  object `66193438-...`). S4: 11 objects, 39 rows (two of them the plan's own nodes).
- **`Params` search information** (16 `.si` rows written as `<uuid>.si`, plus `siVersions`):
  every row is rewritten with the same content and a new version guid; the **main row**
  (`1a621f0f-...si`, 2 MB inflated, raw deflate, BOM, CRLF) lists every metadata object as a
  record `uuid, parent, kind, "name", {1,n, {"lang","synonym"}...}, flag, flag`, depth
  first: the owner, then the groups of its attributes, tabular sections, forms, templates and
  commands. *kind* is the index of the object's class id in the row's own first list
  (a catalog's forms are `fdf816d2-...` = 37, a data processor's `d5b0e5ed-...` = 60, every
  template `3daea016-...` = 16), so it is the descriptor's group class id. The new object gets
  a record next to its sibling of the same group (behind the previous one in the owner's
  list, in the descriptor's order); the count in `{10809,` rises. The two flags: the first is
  the form descriptor's flag behind its header (0 for a managed form, 1 for an ordinary one:
  383 of 383 forms agree), the second 0; templates 0/0. Names and synonyms come from the
  descriptor's header (`{1,"ru","Форма"}` becomes `{1,1,{"ru","Форма"}}`; several languages
  sorted by code, one per line). `siVersions` is `{0,16,"<name>.si",<guid>,...}`: only the
  edited row's guid needs to change.
- **What does not change**: `DBSchema` and `SchemaStorage` (a form or a template has no
  table), `IBVersion`, `Config` rows other than the staged ones and the folded aliases.

### The state an interrupted native apply leaves

The native apply is ~30 000 autocommit statements around the `commit` marker. When it dies
part-way (ddl track, 2026-09-29: killed at «Принятие изменений» while several native runs
competed for the CPU) the database answers every `ibcmd config` command with «Обнаружена
незавершенная операция сохранения конфигурации». By the trace that state is: `SchemaStorage`
(SchemaID 0) at `Status` 200, 400 or 500 (never 100), a non-empty `NewGenCreated`, possibly
`DBSchema` already rewritten, `*.new` rows in `Config` (and a `commit` row once the renames
began), `ConfigSave` still full. The own apply **refuses** that state before it writes anything
(the `*.new` and marker names, and `SchemaStorage.Status <> 100`, are checked in the plan and
again under the locks) and points at the native `config repair`. It never produces the state
itself: the whole move is one transaction.

### What the importer stages (findings for the import track)

- Our import stages the **whole** configuration (patch mode 9 517 rows, base-free 9 842), not a
  delta, so a native apply of it moves every row through `.new`, unchanged ones too.
- ddl track: patch mode silently drops descriptor changes (a new catalog attribute never reached
  `ConfigSave`; every staged row equalled `Config` but `versions`). The apply can only judge what
  is staged: **a change that is missing from `ConfigSave` is invisible to every gate here**.
- Patch mode cannot stage a new form or template (`Config row not found`); base-free can, but it
  recompiles a business process's route-point flowchart differently, which makes the native apply
  restructure that table (S1).
- The help index (`Files.userDocs_ru*`) built by the native apply from our staged help rows is
  nearly empty (2 KB and 2 bytes against 47 KB in the base), so our packed help pages are not
  what the platform's indexer reads.

## What this apply does

One serializable transaction, data moves inside the server only:

1. application lock `ibcmd-rs:config-apply`, exclusive table locks on
   `Config`, `ConfigSave`, `Params`, `Files` (and `_ConfigChngR`);
2. exclusive access: no other user session on the database (`sys.dm_exec_sessions`,
   needs `VIEW SERVER STATE`; our own process is excluded by pid), no unfinished
   operation (`commit`, `dynamicCommit`, `dbStruFinal`, `convertPhase`,
   `erase_save`, `deleted`, `*.new`);
3. the plan's view must still hold: aggregate fingerprints of `ConfigSave`, of
   the `Config` rows it replaces, of the dynamic-update rows and of the `Params`
   marker (row count, byte total and three sums of SHA-256 slices, computed by
   the server) equal what the read-only planning saw;
4. the dynamic generations are folded oldest first (see step 9 above), then
   the `Config` `DynamicallyUpdated` row is deleted, and the `Params` one too when the
   stage carries a descriptor row (native leaves it after a stage of body rows alone, S5C);
5. every `Config` row named by a staged row is deleted (all parts) and the staged
   rows are inserted as they are;
6. `_ConfigChngR._MessageNo := NULL` for every object that owns a staged row, all
   nodes: by the uuid a staged name starts with, or through `_ConfigChngR_ExtProps` (a file
   list that names a staged row). Measured: 4 703 objects x nodes in S2, 13 objects x 3
   nodes in S3, 11 objects / 39 rows in S4; unstaged objects keep their value;
7. new forms and templates: registered in `_ConfigChngR` for every ordinary node
   (ids continue the table's sequence: the greatest `_IDRRef` plus one), their files listed in
   `_ConfigChngR_ExtProps`; a body row an existing object gains is appended to its list;
8. `Params`: the main search-information row gets a record per new object and updates the
   localized synonyms of existing records whose descriptor headers changed, for both the
   nonstructural gate and S1. These edits compose with the stage's other cache edits; its
   `siVersions` entry gets a new version (both rows guarded by the digest the plan saw).
   `Creation` and `Modified` use the platform's form. Twin and session evidence:
   [`evidence/dropin-apply/synonyms.md`](evidence/dropin-apply/synonyms.md);
9. `Files.MobileVersions.dat` gets a fresh random GUID at the head (list capped at
   1 000), timestamps in the platform's form (local time plus the year offset);
10. postconditions inside the transaction: every staged row is in `Config`
    byte for byte (name, part, size, attributes, timestamps, `BinaryData`), no other
    part of it is, the moved row count equals the staged count, no alias or marker is
    left, no change registration of a staged object keeps a message number, every new
    object is registered for every node with its files;
11. `ConfigSave` is emptied and the transaction commits. A failed step, a lost
    connection or a crash rolls everything back: nothing is left for
    `ibcmd infobase config repair` to finish.

A rehearsal (`--rehearse`) runs the same script and ends it with `ROLLBACK`.

Also in the script since checkpoint 2: a staged `deleted` row that the plan consumed is
left out of step 5 (it is not moved into `Config`), out of the moved-row count and of the
postconditions, and the in-transaction check for unfinished operations does not count it as
one; the dynamic rows a consumed list names are deleted instead of folded in step 4
([removals](#removals-the-stages-deleted-row)); the rows of a removed form or template are deleted by name between
steps 4 and 5, under a fingerprint of their own (#393); and a structure phase from a gate that lets a
restructuring through runs between the assertions and step 4, in the same transaction
([S1-A](#restructuring-inside-the-apply-s1-a-397)).

## What it does not write

Derived state the native apply rewrites and this one leaves alone (each is
listed in the report as `not_written`):

| What | Why it is safe to skip |
|---|---|
| `Params.<uuid>.ui` (two rows) | The platform's configuration-licensing records (track ui, #340), re-encrypted by every native apply. This apply never creates or alters licensing data. See [known differences](#known-differences-from-the-native-apply). |
| Unchanged `Params.*.si` rows and their `siVersions` entries | Service-information caches. Their content is a function of the object set and names (same text before and after S2/S3; only a new object, a rename or a synonym change alters it). The native apply rewrites all 16 rows with unchanged content and new version guids; this one rewrites only caches whose content changes (new or removed objects, forms and templates, restructuring, or changed synonyms), with their `siVersions` entries. |
| Help index (`Files.userDocs_ru*` ...) | Rebuilt by the native apply from help pages; a body-only change leaves the pages, so the old index stays valid. |
| ConfigCAS garbage collection and its bookkeeping | Housekeeping of extension content, independent of the main configuration. |
| `_ExtensionsRestructNGS`, `_IDRRef` renewal in `_ConfigChngR` | Scratch of the restructuring framework (three constant rows in every native apply that takes the long path, none on the short one); the registrations keep their ids, so `_ConfigChngR_ExtProps` stays consistent. |

### Known differences from the native apply

Everything that differs between our end state and the native one on the same stage, and why
the platform does not mind (S2, S3, S4, and the probe below):

- **`Params` `.ui`**: the native apply re-encrypts two rows on every apply (the large one
  is the configuration-licensing request, the small one its answer; the ui track decrypts
  them, `docs/apply/params-ui.md` when it lands). We leave them as they were. Evidence
  that the platform opens the database without warnings: `ibcmd infobase config check`
  succeeds on the result (S2, S4, probe); the native `config apply` afterwards answers
  "Обновление конфигурации базы данных не требуется" (S2, S4); a **standalone server
  (`ibsrv`) started cold on the probe database** answers an HTTP web-service call with status 200
  and the marker of the module change the apply moved, with an empty error stream; and **new
  sessions in the 1C cluster** read the changed module (see [Verification](#verification) and
  `docs/apply/evidence/own-apply/cluster-sessions.md`). Track ui measures the same for 8.5 and
  licensing; its result replaces this paragraph when it lands.
- **All `.si` rows and their versions** are rewritten by the native apply even when their
  content is unchanged; we write only what changes (S4: the rows are identical in text).
- **`_IDRRef`** of `_ConfigChngR` is renumbered by the native apply on the long path (S1-S3);
  we keep the ids (the short path, S4, does not renumber either). Ids of new registrations
  differ by construction.
- **Help index**: the native apply rebuilds `userDocs_ru`, `userVocabulary_ru` and `userPostings_ru`
  when help pages are staged (S5A printed «Построение индекса справки»); this one leaves them, so a
  new or changed help page is not found by the help search until a native apply rebuilds the index.
  Nothing in the platform's checks, sessions or exports reads the index.
- **ConfigCAS garbage collection, `_ExtensionsRestructNGS`**: caches and scratch of the native apply,
  see the table above.
- **`Creation`/`Modified` of `Files.MobileVersions.dat`** and the random head guid differ.
- **`_ConfigChngR._MessageNo` of objects the stage does not touch (the long path)**:
  on a base whose register holds NULL for such objects (a clone restored from a corpus backup:
  17 280 of 20 685 rows on the 8.3.27 twin, 17 374 of 21 219 on the 8.5 twin) the native apply, when
  it rebuilds the register (more than 20 staged rows), writes 0 to them; this apply writes NULL for the
  staged objects only (42 of 42 rows equal on 8.5) and leaves the others. Measured equal from a register
  that held 0 (S2-S4, E1-E4). **The message for a node does not depend on it**: with an earlier
  apply's NULL rows at an imaged node turned to 0 by the native long path (a 4-module apply, then a
  30-module one, no message between), the native message and this apply's both carried the same 34 objects,
  with the node before and after it had acknowledged a message
  ([exchange plans](#exchange-plans-of-distributed-infobases-412)).
- **`.si` rows on 8.5**: the native apply rewrites all 16 with their records in another order
  (2 identical, 13 the same lines permuted, and the 29 MB XDTO-model row `ea13a2c9` at the same
  length in another base64 layout); on 8.3.27 the same rows come back with identical text. This
  apply leaves them; check, export and a cluster session are the same.
- **`DBSchema` after a native structural apply (T1, BSP 8.3.27)**: the native apply also rewrites
  the entries of the two service tables `DbCopies` and `DbCopiesUpdates` (+53 and +166 bytes,
  `DbCopiesSettings` appears); the table lists (1 761 tables), all columns and indexes of the 11
  rebuilt tables are equal but for the names SQL Server generates for primary keys. A native
  `config check` and `config apply` on this apply's result accept it.
- **Help index files on the first apply of a restored clone**: native removes the old chunk rows
  (`userdocs_ru_<hash>.bin`, 309 to 327 rows on the БСП twins) while rebuilding the index; this
  apply leaves them (the help-index difference above, seen in `Files` as 44 rows against 353).

### Which per-apply writes are required

The trace track's list of what every native apply writes (`docs/apply/native-apply-trace.md`
sections 3.2 and 7) against what this apply does, with the verdict the evidence supports.
The verdicts come from applying with the minimal set at once (S2, S3, S4, the probe) and
opening the result with the platform's own tools; a single omitted group was not dropped
alone from a full native set, because nothing in the results asks for it.

Two of the rows below -- the change registrations (with the imaged nodes' rows) and `Files.MobileVersions.dat` -- are also what
the platform's **dynamic** apply (`--dynamic=force`) writes besides its alias rows: the measurement of #347 found them, and
the text in the script is the same `sqlgen::render_parity_writes` the dynamic apply runs in its own transaction
([`dropin-dynamic.md`](dropin-dynamic.md), [evidence](evidence/dropin-dynamic/acceptance.md)).

| Native write | This apply | Verdict and evidence |
|---|---|---|
| `Config` rows: `.new` copy, `commit` marker, promotion row by row outside a transaction | one `INSERT ... SELECT` in one transaction; `commit`, `dbStruFinal`, `dynamicCommit` are never written | **required**: the rows (identical to native in S2, S3, S4). The markers exist to resume an interrupted promotion; a transaction has nothing to resume. |
| Fold of dynamic overlays (`X_dynupdate_G` over `X`, `versions_dynupdate_G`, `DynamicallyUpdated`) | written, oldest generation first; done **whether or not the stage lists those rows** (it never does) | **required** when the base has an overlay (S1-S4 all had one); rows identical. See "Dynamic-update rows" below. |
| `_ConfigChngR._MessageNo := NULL` | written for every owner of a staged row; a row is **inserted** where a node of the plans that register changes has none | **required** (it is what the exchange plans read); identical in S2-S4 and, for imaged nodes, in the messages of [#412](#exchange-plans-of-distributed-infobases-412). |
| New objects: rows in `_ConfigChngR`, `_ConfigChngR_ExtProps` | written | **required**; identical to native but for the ids. |
| `_ConfigChngR` and `_ExtProps` rebuilt through `..NG` tables with new ids (long path) | not written | **not required**: the platform runs on the old ids (check, apply afterwards, cold server); the short path does not renumber. |
| 16 `.si` rows and `siVersions` rewritten | only the main `.si` row and its version, when a new form or template adds records | **required for new objects** (the row lists them; text identical to native in S4); **not required otherwise** (content unchanged, S2-S4). |
| `Files.MobileVersions.dat` (ring of 1 000 guids) | written, one new guid in front | kept: it is what mobile clients compare, and it is cheap. Not shown to be needed by anything measured here. |
| `Files.extd_props_cached/gc.mrk`, `CAS_GC_Info`, ConfigCAS garbage collection | not written | **not required**: bookkeeping of the extension content store, independent of the change (first apply of a lineage only for the collection). |
| Help index `userDocs_ru`, `userVocabulary_ru`, `userPostings_ru` | not written | **not required to open the database** (not a session or export input); a stale index only affects searching help. Rebuilding it needs the platform's indexer. |
| `DBNames*.New` passes, `DBNames`, `DBNamesVersion-DBNames` | not written | **not required**: unchanged for every stage measured (no table is added or renamed by a module, form or template). |
| `DBSchema` and `SchemaStorage` walk 100 -> 200 -> 400 -> 500 -> 100 (long path) | not written; a state other than 100 is refused | **not required** while no table changes: the bytes are identical before and after. |
| `_ExtensionsRestructNGS` clean-up | not written | scratch; identical rows in every native apply. |
| `Params` `.ui` (3 rows) | not written | licensing records (track ui): see known differences. |

### Dynamic-update rows go even when the stage omits them

A database that received an online (dynamic) update carries `DynamicallyUpdated` (in `Config`
and in `Params`), `versions_dynupdate_<g>` and, for the objects the update changed,
`<id>_dynupdate_<g>` and `<id>_dynupdate_<g>.<n>` rows; the alias row is the text the
configuration runs. The importers do not stage these rows (patch mode "leaves those six rows alone",
`docs/import/patch-mode.md`), and the native apply removes them all the same: first it merges each
alias over its base row, then the staged rows replace what they name. This apply does exactly that,
inside its transaction, oldest generation first, whether or not the stage lists a row (the stage never
does): the base row is deleted and the alias renamed over it, `versions_dynupdate_<g>` and the two
`DynamicallyUpdated` markers are deleted (the `Params` one only when the stage carries a descriptor;
the native apply leaves it after a stage of body rows alone, S5C). A `deleted_dynupdate_<g>` row (the
removal list of a dynamic update, never seen here) and an overlay in `Params` (`.si` rows under an alias
name) are refused, not folded.

Evidence on the БСП clone, which carries one generation (`06cb0442-...`) with two objects
(`a627e390-...`, a common module, and `ab132638-...`, a form), each with an alias descriptor and an alias
`.0` body whose text differs from the plain `.0` row:

- S2, a full patch stage that stages both plain `.0` rows: after the native and the own apply alike the
  staged text wins, the six dynamic rows are gone, and `Config` is identical row for row (9 838 of 9 838);
- S3, a 30-row delta stage that stages **neither** object: after both applies the two `.0` rows hold the
  **alias text** (`6c4f62ac...` and `825c84b3...`, not the plain `a0779dc7...` and `91bc23be...`), the six
  rows are gone, `Config` is identical (9 841 of 9 841). The alias survives as the ordinary row; nothing is
  lost by omitting it from the stage.

### Removals (the stage's `deleted` row)

**What the native apply does, measured on 8.3.27.2214 (#393, this section's twins).** It never deletes a `Config` row that the
stage merely omits: a form removed from the tree leaves its three rows in `Config` (import track,
`docs/import/patch-mode.md`, section 6.3). A removal travels in the row `deleted` of the stage:
`<BOM><count>,"<row name>",<flag>,...`, flag `0` for a `Config` row and `1` for an element that has no row of its own (an
attribute id, the platform's own import). The native apply copies the row as `deleted.new` and deletes it **without promoting
it** (trace, phase D2), so `Config` after the apply equals the stage minus the named rows.

The edit is the БСП tree without the form `ВсеЗаметки` of `Catalog.Заметки` (rows `8a7546f4-...`, `.0`, `.1`) and the template
`ДатыПасха` of `DataProcessor.ЗаполнениеКалендарныхГрафиков` (rows `0384ec55-...`, `.0`); the БСП clone carries a pending
dynamic update (`06cb0442`), so the platform's list also names its six rows. Three native runs: an import of the whole tree and
an import of the unedited tree, each followed by `config apply --force --dynamic=disable` (`fdk`, `fdc`: the control, whose end
state differs from `fdk` in nothing but the removal), and the apply of the stage that this repository's import writes (`fdp`).
What the removal does, from `snapdiff`, `si_diff` and `reg_cmp` of the kit against the control:

| State | Native apply of a stage that drops a form and a template |
|---|---|
| `Config` | the five rows of the two objects are gone (9 836 rows: 9 847 - 5 - 6 dynamic-update rows); the staged owners' descriptors replace the active ones |
| `_ConfigChngR`, `_ConfigChngR_ExtProps` | with `_MessageNo` NULL everywhere (the corpus): **untouched for the removed objects**: 20 685 rows and 21 365 file rows, the removed objects' rows (5 nodes for the form, 3 for the template) and their file lists stay. Against the control: 0 rows differ, 0 message numbers, 0 file lists. With message numbers and a missing row (twin `fdq`): the removed object is registered **like a changed one** -- its message numbers become NULL (7 and 0 in the twin) and a node that has no row of it gets one, `_MessageNo` NULL, with the object's body files in name order (`.0`, `.1` for the form, `.0` for the template) |
| `Params` main search information (`1a621f0f-....si`) | the two records are gone (`{10807,` becomes `{10805,`); nothing else changes |
| `Params` properties row (`c4629235-....si`) | the form's entry is gone (`8a7546f4-...,1,0,{"S","v8config://v8cfgHelp/mdobject/id8a7546f4-.../038b5c85-..."}`: a form with a help page has one, a template none); the count `{2376,` becomes `{2375,` |
| `siVersions` | both rows have new versions |
| `DBSchema`, `SchemaStorage`, `DBNames` | unchanged (a form and a template have no table; `DBNames` does not know their ids) |
| the other cache rows, `IBVersion` | as in any full apply (the `.ui` rows, `MobileVersions.dat`, the help index, the CAS collection: [known differences](#known-differences-from-the-native-apply)) |

**The `deleted` row of the platform against the one of this repository's import** (branch `feat/0.4-import-override`, built
from source in the apply lab): the same text format (BOM, count, `"name",0` pairs joined by commas, no line break, 655 bytes for
11 names), the same set of 11 names: the five rows of the two objects and the six dynamic-update rows
(`tools\deleted_cmp.py`). The order differs, and the platform's own order is not stable either: two runs on the same tree wrote
two different orders, the same bytes otherwise. So byte equality of the row is not a property the platform itself has; the set
of names is the comparison, and it is equal.

**What this apply does with a `deleted` list.** It answers the list name by name (`mod.rs`: `answer_removals`):

1. an empty list, and a list that names **all** the rows of the dynamic update `Config` carries (or none of them): consumed,
   the rows deleted without folding ([step 1](#what-this-apply-does), E1-E4 below);
2. the rows of **a removed form or template** that the analysis (`removals.rs`) accounts for: deleted, with the records of the
   two search-information rows. The analysis takes an object only when all of this holds, and refuses it (the list is refused whole)
   otherwise:
   - the list names exactly the object's rows: its descriptor and every `<uuid>.<n>` that `Config` holds, nothing that `Config` does
     not hold; a name that is a service row, a body of an object that stays, or of no known shape is refused;
   - the stage has no row of the object (a stage that removes an object and stages its bodies is contradictory);
   - exactly one descriptor of the active `Config` mentions the object's uuid (its owner, which lists it in a group of forms or of
     templates: another kind of owned object is refused), and no staged descriptor mentions it;
   - the owner's descriptor is staged and equals the active one **minus the removed references** as a tree (the inverse of the
     new-object analysis in `objects::analyze`: a descriptor that changes anything else is a metadata change and stays for the
     native apply);
   - the object has no table or column: `Params.DBNames` and the `DBNames-Ext-*` rows do not mention its uuid;
   - the staged `versions` row no longer lists its rows;
   - no extension adopts it (`restructure::extensions`, the check of S1-I: what the platform does to an extension whose adopted
     object goes is not measured);
   - 8.3.27 only (on 8.5 the platform's own apply removes it);
3. anything else, that is the names with flag `1` (attributes) or a flag-`0` name the analysis did not take: **the gate judges
   them when it says it can** (`StructuralGate::judges_deleted_row`, the S1 gate for the attributes the stage removes), and the
   list is consumed only when a structure phase answers for them; otherwise the whole list is refused with
   `NeedsNativeApply::apply`, the form included, naming the first name it cannot account for and the reasons of the analysis.

The names that steps 1 and 2 execute are handed to the gate (`GateInput::removed_rows`); the S1 gate leaves them out of the
`deleted` list it gives its plan (`restructure::s1::without_names`), so the plan sees the attribute ids alone, and the owners'
descriptors are accepted like those of new objects (`accepted_owner_descriptors`).

*The change register* is the plan's (`registrations::plan`): the removed rows' names go in with the names of a dynamic update that
the list names, so the objects' rows get their `_MessageNo` reset, a node with no row of the object gets one with the files, and
the bodies missing from an existing list are appended -- what the twin `fdq` showed the native apply does. *In the script* (one
transaction, as before): the fingerprint of the removed rows (count, bytes, three digest sums) is asserted under the locks with
the others (`57320`); after the dynamic rows are dropped or folded the rows are deleted by name, every part, and both the count
of the deleted rows and the absence of any of them are checked (`57321`); then the move, then the change registrations. *In the
plan*: the two cache rows are edited **on top of** whatever the new objects and a restructuring of the same stage rewrite in the
same stage (`removals::plan_search_info`): the edit is by uuid on the text that the earlier edit produced and keeps the digest of
the stored row, so the object registry `1a621f0f` can lose the records of a form and of an attribute in one stage, where a new form
and an attribute still clash (`merge_params_rewrites`). A record has to be the object's own (parent, name and class as its owner
files it, no children), or the plan refuses. *The recovery artifact* gets `removed_rows.tsv` (name, part, attributes, dates, and
where the bytes are in `rows.pack`). *The report* names the removed objects (`removals`: objects, rows, records, property entries).

**Evidence for the rows of a dynamic update (checkpoint 2): twins of БСП 8.3.27 (`tools\verify_new.py`, native against own on byte-equal stages).** A delta stage
of four modules and their `versions`; E1 and E2 on a base without an overlay, E3, E3b and E4 on a base that
carries the dynamic generation `06cb0442` (two objects, `a627e390` and `ab132638`).

| Case | `deleted` | Native | This apply | End state, own against native |
|---|---|---|---|---|
| E1 | `0`, base without overlay | consumed | consumed | `Config` 9 841 of 9 841, `_ConfigChngR`, `ExtProps` (16 343 objects), all 16 `.si` texts, `Params`, `Files` equal |
| E2 | empty text, base without overlay | consumed | consumed | the same |
| E3 (first try) | the 6 overlay names, base with overlay | rows deleted, **no fold** | folded | `Config` differed for the two objects: the native apply keeps the plain text |
| E3b | the 6 overlay names | rows deleted, no fold | deleted, not folded | `Config` 9 841 of 9 841; `.si`, `siVersions`, `Params` equal; the native apply also sets `_MessageNo` NULL for the two objects and appends their alias file names to `_ConfigChngR_ExtProps` (2 objects x 4 nodes): **this apply does the same since #412** (the owners of the rows a `deleted` list names are reset, the alias bodies appended to the lists, and the row inserted where a node has none) |
| E4 | `0`, base with overlay | consumed, **folded** | consumed, folded | `Config`, register, `ExtProps`, `.si`, `Params` equal (the alias text is the ordinary row afterwards) |

The `Files` rows differ in E3b and E4 by the help-index chunks only (known difference). The E3b
register difference is reproduced since #412 (see the row above); the case needs a list that names overlay rows, which
neither importer of this repository writes yet.

**Evidence: twins on the same stage** (`tools\native_twin.ps1`, `native_cmd.ps1`, `deleted_cmp.py`, `removals` tests). The stage is
what this repository's import writes for the tree (`import-override` build, 9 516 rows, the row of 11 names); native and this apply
run on byte-equal copies of it (a COPY_ONLY backup restored twice). The same acceptance on the build that has the import merged
(`feat/0.4` 029b4a2b), through the drop-in commands `infobase config import` and `infobase config apply`
(`scripts/apply-lab/final_acceptance.ps1`, `evidence/own-apply/removals-393-final-acceptance.txt`): `Config` 9 836 of 9 836 equal to the
platform's apply of the same stage, 15 of 16 `.si` texts equal, the platform's export equal to the tree (12 190 of 12 190 files,
`ConfigDumpInfo.xml` aside), the platform's second apply «не требуется».

| Check | Form and template (`fdp` native, `fdo` ours) | Form, template and an attribute, gate S1 (`mxp`, `mxo`) |
|---|---|---|
| `Config`, dates and attributes included | 9 836 of 9 836 rows equal (0 differ) | 9 836 of 9 836 |
| the 16 `.si` rows compared after inflate | 15 of 16 equal; `1a621f0f` and `c4629235` equal: the records and the entry are exactly the platform's. The sixteenth, `c77bc206`, is the same list in another order (71 116 characters both) | the same 15 of 16; `1a621f0f` equal with **both** edits, the attribute's record (the gate's) and the two records of the form and the template (the apply's) |
| tables, `DBSchema`, `DBNames` | not touched (no difference) | the rebuilt `_Reference3347`: 4 rows both ways with `EXCEPT`; `DBNames` text equal; entries equal but `DbCopies`/`DbCopiesUpdates` (known) |
| a native `config export` of our result against the tree | 12 190 of 12 190 files, `ConfigDumpInfo.xml` aside | 12 190 of 12 190 |
| a native `config apply` on our result | «Обновление конфигурации базы данных не требуется» | the same; `config check` succeeds |
| the change register | 20 685 rows both; the same as the known long-path difference (782 objects' `_MessageNo` NULL against 0) and the order of the file list of two objects that have a body unchanged and a body changed (6 rows: the platform lists the unchanged file first) | the same 782 and 6 |
| a register with message numbers and a missing row for the removed objects (`fdq` native, `fdq2` ours; the form has 7 at one node, 0 at another and no row at a third, the template 7 and no row at the third) | the removed objects' rows: `_MessageNo` NULL everywhere, the missing rows inserted with the body files; equal on both (`reg_state.py show`); the other differences are the 782 and 6 above (the native apply writes the node's last message number, 7 here, where the corpus had NULL) | |
| a rehearsal | `snapdiff` before and after: nothing changed | |
| a tampered removed row after the plan | error `57320`, nothing changed (Config 9 847, ConfigSave 9 516) | |

Time of the real run: 375 s for the first (debug build, other tracks running; 187 s of them SQL, 124 s the recovery artifact), 232 s
for the second; the native apply of the same stage 137 s and 227 s.

**Lists refused whole** (`tools\refusal_cases.ps1`, `--dry-run` on the clone that holds the stage of the first twin; the control, the
platform's list of 11 names, is planned; each of the others is `needs_native_apply` and names its reason):

| The list, changed | Answer |
|---|---|
| an attribute id with flag 1 next to the form and the template | «1 name(s) not accounted for, the first c1a2b3d4-...» (the default gate does not judge attributes; under `--allow-restructure s1` the same list is the gate's, as the second twin shows) |
| a body row of an object that stays (`00149051-....0`) | «a body row of an object that stays (a module, picture or help page of it): the removal of one file of an object is not measured» |
| the form's `.1` left out | «a row of a removed object that the list does not name: the list must name the object's rows exactly» |
| a name `Config` does not hold (`<form>.7`) | «the list names a row that Config does not hold» |
| four of the six rows of the dynamic update | «it names 4 of the 6 rows of the dynamic update that Config carries, and the native apply's answer to a partial list is not measured» |

The other refusals of the analysis (a second descriptor that mentions the object, an owner that is not staged or changes more
than the references, a table or column in `DBNames`, a staged `versions` that still lists a row, a message number or a missing row in
the register, an extension that adopts the object, 8.5) are unit tests of `removals_tests.rs` on a database of canned answers, one
test each, and of `answer_removals` (`mod.rs`) for the whole-list rule, the mixed list of a form and an attribute included; the seam
with the S1 gate (`without_names`, the plan's refusal of a form's rows it is still handed) is `restructure::tests_s1`.

**What is not done, and why** (the twin is missing or the answer is a refusal):

- **one file of an object that stays** (a module, a picture, a help page): the list names `<owner>.<n>` of an object that is not
  removed, the analysis refuses it, and this repository's import does not write it either (its guard refuses the stage). A native twin
  of the `moddel` edit (the manager module of a data processor) is prepared; the platform's first `config import` of a fresh clone
  fails or stages a subset in most attempts (finding below);
- a **common module** or any other object with rows of its own that other descriptors and the roles' rights mention (all 801 common
  modules of the БСП tree are mentioned by some XML file: a role's rights, a subsystem's content, an event subscription): the edit that removes one changes the
  roles as well, a stage of many objects; not measured;
- an owner that both loses and gains a form or a template in one stage: the two analyses each see the other's change as "more
  than the references", the stage is refused;
- **objects with tables or columns** (a catalog, a document, a register; an attribute or a tabular section): the platform removes
  the columns; refused as before unless the S1 gate judges the attribute ids;
- 8.5.

**A finding for the twins of the platform's own import.** On a fresh clone the platform's first `config import` of a tree stages a
subset of the rows (9 620-9 640 of 9 840) or ends with «Ссылка на неизвестный предопределенный элемент - ...» or another
critical error of the same family, whatever the tree (the unedited tree fails the same way); a second and a third import, into the
same clone, stage everything (9 837 rows for the tree without the form and the template, 9 842 for the unedited one). A twin made
from the platform's own stage repeats the import until the row count is the tree's, and checks it (`native_twin.ps1`). Two
native stages of trees that differ by the removal alone are not equal outside it either: 1 949 `Config` bodies differ between
the two apply results (the importer re-serialises), which is why the twins above start from **one** stage.

### Which native path these writes correspond to, and why

**The short path.** The write set is the one the native apply runs when it takes the short path (trace track,
section 6.4; here S4, S5A, S5C): the staged rows into `Config` with the fold of a dynamic overlay,
`_MessageNo` reset in place for the owners of the staged rows, registrations for new objects, `.si` for new
objects, `MobileVersions.dat`; no register rebuild, no ConfigCAS garbage collection, no `..NG` DDL. Proved against a
native twin on three stages: `Config`, `_ConfigChngR`, `_ConfigChngR_ExtProps`, all 16 `.si` texts and the
other service tables equal but for the documented differences (`docs/apply/evidence/own-apply/s4-write-families.md`
holds the trace comparison of S4 with the kit's `compare_traces.py`: native 195 write statements in 24
transactions, this apply 17 in one; the rows this apply leaves out are the `.ui`, the `.new` copies, the 16 `.sinew`
rows, the markers and the `_ExtensionsRestructNGS` clean-up).

**What happens where the native apply takes the long path** (more than 20 staged rows, or the descriptor of a
catalog or a document that differs: S1, S2, S3, S5B). This apply writes the same short-path set and leaves the
long-path extras out. Accepted by the platform in every such case, each time with the platform's own tools on the
result: `config check`, a native `config apply` afterwards ("Обновление конфигурации базы данных не требуется"),
`generation-id`, `config export` (byte-identical to the native twin's, S2), a cold `ibsrv` and a new session in the
1C cluster. The extras, one by one:

| Long-path extra | Why it is not written |
|---|---|
| `_ConfigChngR` / `_ExtProps` rebuilt through `..NG` tables, new `_IDRRef` | Only renumbers; the registrations keep their ids and stay consistent. The rebuild is DDL (create, load, drop, rename) in a transaction that has none. |
| `SchemaStorage` walk 100 -> 200 -> 400 -> 500 -> 100, `DBSchema` rewritten | Same bytes before and after (S2, S3); a state other than 100 is refused, so nothing is left half-way. |
| ConfigCAS garbage collection, `CAS_GC_Info`, `gc.mrk`, `_ExtensionsRestructNGS` | Bookkeeping of the extension store; "unreferenced" is not decoded (trace 4.1), and no reader of the main configuration depends on it. |
| Help index (`userDocs_ru*`, `userPostings_ru*`, `userVocabulary_ru*`) | Needs the platform's indexer; only help search reads it (known difference). |
| 16 `.si` rows and `siVersions` rewritten | Content unchanged for module, form and template edits (S2-S4); the main row is edited when new objects add records. |
| `DBNames*` passes | Thrown away by the native apply itself unless a table was added or renamed. |

**Why not choose by the rule, or write every extra.** The rule (a changed table-owning descriptor, more than 20
rows) describes what the native apply does, not what the platform needs to read the result: the short-path result
is accepted after a stage the native apply takes on the long path. An extra that needs the platform's indexer or
a collector this program cannot reproduce cannot be written whatever the rule says; the register rebuild could be, but
would add DDL and renumbering that nothing reads. Each extra that is ever added needs the trace comparison
(`compare_traces.py`) against a native twin first; this comparison was done on the short path (S4) and, for the long
path, as end-state comparisons (S2, S3, S5B), not as a trace of a long native apply. It is the check to run before
writing any long extra, and to run once on a long stage of 21 to 30 rows if a reader of the extras turns up.

## Safety

- **Fail closed**: an unknown storage layout (table fingerprint of the profile), an
  unsupported platform profile, a `deleted_dynupdate_*` row, an unfinished operation,
  a `deleted` list with a name that the analysis of removals cannot account for, a reused generation,
  an unlisted staged row (warning), any structural blocker, a restructuring without a stated
  way back: no write.
- **Plan without locks, verify under locks**: the plan reads metadata and server-side
  fingerprints only; the transaction re-checks them after taking the locks.
- **Exclusive access** is proven by SQL Server's session list, not assumed. A working
  process that still holds a pooled connection makes the apply refuse (stop the
  process or the infobase's sessions first). `--exclusivity assumed` is for the operator
  who has proved it with `rac session list`.
- **Recovery artifact** (before the transaction): the bytes of every row the apply
  changes (`--recovery-blobs changed`, default) or a hash manifest (`none`), the special
  rows, `MobileVersions.dat`, the `_MessageNo` values, the `Params` rows rewritten for new
  objects with their old bytes (`params_replaced.tsv`) and the registrations added
  (`new_registrations.tsv`); the bytes in one file, the newest five artifacts per database
  kept ([recovery](#recovery-artifact-and-its-retention)).
- **Typed refusals**: a caller sorts them by type, not by text
  ([refusals](#refusals-a-caller-can-sort)).
- **Dry run** writes nothing at all; **rehearsal** writes nothing that survives.

## The structural gate

Two gates stand behind the `StructuralGate` trait; `structural_gate()` is the one function that picks
(`ConfigApplyOptions::gate`, `--gate`). The default is `ApplyCheckGate`
([below](#the-default-gate)). `--gate conservative` selects the rule described here.

`ConservativeGate` (module `mssql_config_apply::gate`, one call site in `plan`) admits:

- `root` and `version` unchanged (or absent: a delta stage), `versions` replaced with a new
  generation. `root` may differ in its bytes when its inflated text is the same: the importer stages it deflated, and a
  database the platform has applied to keeps it as one stored block (`compare_root`; found by the twin of S1-F N1,
  before which every stage that carried `Configuration.xml` on such a database was refused as "the service row root
  changes");
- a descriptor row that exists in `Config` and inflates to the same text;
- a body row whose owner kind and suffix the source-asset registry names as a module,
  form, template, picture or help page; other body roles pass only when the inflated
  text is unchanged;
- a **body row of several parts** (the platform cuts a value at 10 MB into `PartNo` 0, 1, ...): one
  change, whichever part differs. Its first part stands for it in the role check and the parts
  beyond the first are counted (`extra_parts`). Refused: a part whose first part is not staged; a
  descriptor with a part other than 0; a row of several parts (staged or active) whose role would
  need a text comparison -- predefined data, an exchange plan's content -- because that
  comparison reads one part only;
- **new rows**, judged by `mssql_config_apply::objects` and passed to the gate as accepted:
  - a *new form or template of an existing object*: the owner's staged descriptor must be
    the active one plus the references (equal as brace trees once the new uuids are taken
    out of every `{class,count,uuid...}` group and the counts are put right); nothing removed,
    nothing else changed; the new descriptor's own header carries its row's id; a form has
    the 0/1 flag behind its header; its bodies are `.0` (and `.1` for a form), nothing else;
  - a *new body row of an existing object* that has a descriptor row and change registrations;
    one per object (the order of several is not known);
  - the search information must list the owner, the class of the group must be in its class
    list, and where the first child of a group goes must follow from the order other owners
    of the kind list their groups in; the exchange-plan nodes must be readable
    (`_Node<n>` tables) and none marked for deletion.

It refuses new objects of every other kind (a catalog or a document unless a structure phase answers for it, an attribute, a command, a subsystem),
new bodies of nested objects or of the configuration, owners whose descriptor changes more
than the lists, descriptors whose text differs, predefined data, rights, interface, package
and unknown bodies, and unknown row names. Its rows read from a consumed `deleted` list are
skipped. The new-row analysis is the plan's, not the gate's, and applies whichever gate is used.

### The default gate

`ApplyCheckGate` (`mssql_config_apply::check_gate`) holds the plan's `SqlExec` and calls track
rcheck's `apply_check::check_staged`, the code behind `ibcmd-rs mssql-apply-check`. The check reads
what it compares (the old and the staged descriptors and bodies of the changed names) from SQL
itself, set-wise. The gate refuses when the verdict says `needs_restructuring`, when **any** reason is
of class `unknown`, and when the verdict is not conclusive (files left out); the refusal text is
`Verdict::refusal()` (the Russian «требуется штатный config apply: ...»), the blockers are listed
as `[class] summary` (at most 200), and the check's figures (`staged_rows`, `descriptors_compared`,
`body_rows_compared`, `objects_changed`, `notes`) are in the report at `gate.stats.restructure_check`.
Reasons that name a consumed `deleted` row are dropped before the verdict is read. New forms and
templates are judged by the plan's `objects::analyze` whichever gate runs. Tests: the verdict
conversion (six), the plan with each gate, and the consumed-row skip.

**Regression with a new form and a new template.** The base-free stage of S1 (9 841 rows, every row staged, a new
form and a new template of one owner; the route-point flowchart body of the business process left out, which the
conservative gate refused in S1) passes the default gate (`objects_changed` 3, ten notes, no blocker) and the
new-object analysis (the two objects, three registration nodes, two search-information records). Own against native
on twins: `Config` **9 842 of 9 842**, `_ConfigChngR` 20 691 rows equal (messages too), `.si` 15 identical and one
permuted, `siVersions` and `Params` equal; `_ConfigChngR_ExtProps` equal as sets, but the native long path lists the
files of one object (the business process, on its five nodes) in another order (`.7` first). Native 1 463 s and own
430 s, both under load from other tracks' runs (own: 13 s in a dry run, 331 s of SQL under load).

**Finding for the import track: `versions` must be based on the effective row.** A base that carries
a dynamic overlay has `versions_dynupdate_<g>` next to `versions`; for the names the overlay updated
the overlay row holds the current version ids. A stage whose `versions` was built from `Config`'s
plain `versions` lists stale ids for those names, and `check_staged` flags every one as `unknown`.
Measured on the S3 delta stage on the overlay base: refused until `versions` was rebased onto the
overlay's row (4 entries changed); then the gate passed and the result equals the native apply's
(E3b/E4 above). The conservative gate does not look at ids.

**The gate's share of the time** (ERP УХ clone, 118 377 `Config` rows, 4.3 GB; the iter build of this
branch): 20.0 s of 34.8 s for 300 modules (57 %), 24.9 s of 33.8 s for one module of 4 MB, 27.8 s of
226 s for a dry run of the whole configuration (12 %), 21.2 s of 699 s for its real run (3 %); on the
БСП clone 1 to 5 s. It is a fixed cost of the size of the base, not of the stage: one scan of
`Config` by name, `SELECT FileName, COUNT_BIG(*), SUM(DataSize) ... GROUP BY FileName`, takes 9.8 s
on this clone alone, and the check runs it for `Config` and `ConfigSave`, after the plan's own
inventory has run the same scan (11 s). One shared scan would cut 20 to 30 s from a small apply on a
base this size; proposal for track rcheck (their `Db::names`), not done here.

## Command line

```
ibcmd-rs mssql-config-apply --platform-profile platform-8.3.27.2214|platform-8.5.1.1150 --database <db>
    [--server localhost] [--sql-user U --sql-pwd P | --sql-pwd-env IBCMD_DB_PSW]
    [--dry-run | --rehearse] --allow-non-lab
    [--exclusivity sql|assumed] [--recovery-dir DIR] [--recovery-blobs changed|none]
    [--recovery-keep N]                       (default 5; 0 keeps every artifact)
    [--gate apply-check|conservative]         (default apply-check)
    [--allow-restructure s1] [--recovery-backup FILE | --i-have-a-backup]
    [--script-output FILE] [--report FILE]
```

`--allow-restructure` needs one of the two backup options when the stage restructures and the run
writes; a stage that does not restructure needs neither
([S1-A](#restructuring-inside-the-apply-s1-a-397)).

## Verification

Lab: `F:\ibcmd\lab\04\apply` (scripts in `tools\`, snapshots in `snap\`, traces in `xe\`, logs in
`logs\`). Twins of one БСП 8.3.27.2214 clone (`ibcmd_rs_04_apply_bsp8327_*`) carry byte-identical
`ConfigSave` rows; one is applied natively, the other by `mssql-config-apply`; a row-level snapshot
(`tools\snapshot.sql`) of every service table is taken before and after (`tools\explain_diff.py`
sorts every difference into a class).

| Scenario | Stage | Native apply | Own apply |
|---|---|---|---|
| S2 patch stage of 227 module, form and template edits, one earlier dynamic generation in the base | 9 517 rows, 81.3 MB | 298 s (under load; 110 s in S1) | 22 s in all: 4 s inventory, 1 s fingerprints, 9 s recovery artifact, **5.6 s transaction** |
| S3 delta stage of three objects (catalog, document, common module) | 30 rows, 0.4 MB | 87.5 s | **1.2 s** in all, 0.6 s transaction |

End state, own vs native, on the same stage:

- `Config`: **9 838 of 9 838 rows identical** (S2), **9 841 of 9 841** (S3) -- names, parts, sizes,
  attributes, `Creation`/`Modified` and bytes, including the aliases folded into the ordinary rows
  and the removed `DynamicallyUpdated` rows.
- `_ConfigChngR`: the message number of **all 20 685 rows** equal (S2 and S3, from a state where
  every row was set to 0 first, so that a reset shows).
- `Params`: the dynamic marker gone; 18 (S2) rows identical, 16 rewritten by the native apply with
  unchanged content; only the two `.ui` rows, `siVersions` and (S2) one `.si` row differ.
- `Files`: `MobileVersions.dat` differs in its random head GUID only; the help-index files and
  chunk rows and the CAS bookkeeping are the native apply's own housekeeping.
- `DBSchema`, `SchemaStorage`, `IBVersion`: identical.
- `ConfigCAS`: 636 rows identical, 12 161 garbage-collected by the native apply only.

The platform on the own result (`ibcmd_rs_04_apply_bsp8327_ours2_20260929`):

- `ibcmd infobase config apply --force --dynamic=disable` afterwards: "Обновление конфигурации базы
  данных не требуется" in 4 s, zero row changes;
- `ibcmd infobase config check --force`: "Проверка корректности метаданных успешно завершена";
- `ibcmd infobase config generation-id`: `b904aa5eecc9ad469d3ff335dab4f27b00000000`, the value the
  native apply printed for the same stage;
- `ibcmd infobase config export`: **all 12 198 files byte-identical** to the native export of the
  natively applied twin (`ConfigDumpInfo.xml` included), 12 180 identical to the edited tree; the
  other 18 are `ConfigDumpInfo.xml` (`configVersion`) and 17 modules that were empty in the reference
  tree, where the test edit appended a comment with LF and the export writes CRLF.

S4, a delta stage with a **new form and a new template** (16 rows: the two new objects with their
`.0` bodies, their owner, ten module edits, `versions`; no `root`/`version`), native 39.6 s against
own 9.7 s in all (transaction 1.8 s); traced with the kit of track trace: native 195 write statements
in 24 transactions, own 17 in one (`docs/apply/evidence/own-apply/s4-write-families.md`). Own vs native end state (`tools\verify_new.py`):
`Config` **9 845 of 9 845 rows identical** (the four new rows and the owner's descriptor included);
`_ConfigChngR` 20 691 rows, the same (node, object) pairs and message numbers, ids unique;
`_ConfigChngR_ExtProps` identical for 16 349 objects; **all 16 `.si` rows have identical text**
(the main row with the two new records at the same places, `siVersions` the same entries);
`Params`, `Files`, `DBSchema`, `SchemaStorage`, `ConfigCAS` identical but for the documented
differences. Native `config check` succeeds on the own result, `generation-id` equals the native
twin's (`206601e511d02f4f844aa29ae70f045300000000`), the native `config apply` afterwards
says "Обновление конфигурации базы данных не требуется".

S5, the other shapes of new objects, each applied natively and by the own apply to twins of one stage
(`tools\verify_new.py`; a first attempt that cloned a data processor's form into a catalog was refused by
the native metadata check, «Ошибка формата потока», before it wrote anything -- the clone source matters,
not the apply):

- **S5A** (15 rows): a form with a help page (`.0` and `.1`) appended to a data processor that has forms
  and commands; the **first template** of a data processor that has a form and commands (its record goes
  between them); the **first form** of a data processor that has only attributes; the first template of
  a data processor **with no children at all**; a **help page added to an existing form** (`.1`).
  `Config` 9 851 rows, `_ConfigChngR` 20 697 rows, `_ConfigChngR_ExtProps` of 16 355 objects and all 16 `.si`
  rows (text) identical to the native result.
- **S5B** (4 rows): a **form of a catalog** (cloned from a form of that catalog) on an owner that is
  registered for five nodes: identical; the form is registered for the same three ordinary nodes as
  any other new object, and the native apply took the long path (a catalog's descriptor), so its `_IDRRef` differ.
- **S5C** (2 rows): an **object module added to a data processor** (a container row `.0` next to its
  existing `.1`): `Config`, `_ConfigChngR` and `_ConfigChngR_ExtProps` identical (the new file appended
  behind the existing one, as predicted), `.si` untouched by both; the native apply took the third path
  above and left `Params.DynamicallyUpdated`, which this apply now leaves too for such a stage.
- **S6** (3 rows, made by hand on the S5C twin, `s5our`): `versions` with a new generation and a template
  body of two parts -- both parts of an existing `.0` row, 10 000 000 and 974 171 bytes, one byte of the
  second flipped. The gate passes it as one changed `Template` body (`extra_parts: 1`); the apply moves both
  parts and its postconditions hold (every staged part in `Config` byte for byte, `ConfigSave` empty,
  `DataSize` 10 974 171 on both). The start state -- the `Params` marker left by S5C, no `Config` marker --
  is a legal input now (`parse_dynamic_history` reads a lone `Params` marker as "no overlays"). Not compared
  with a native apply: the import of this repository staged such a row as one `PartNo = 0` row (in S2 it
  became one part in the native twin too, see "What the native apply does"), so a stage with parts needs
  the platform's own import.

The native log as an oracle. The native apply names in its log the objects it treated as changed
(«Объект изменен: X», «Новый объект: X») and the phases it ran. Against every stage of this track (the
logs are `logs\native_*.out` in the lab):

| Stage | «Объект изменен» / «Новый объект» | «Обработка структуры базы данных» | «Построение индекса справки» | This apply's gate |
|---|---|---|---|---|
| S1 (base-free stage, new form and template) | БизнесПроцесс.Задание | yes | yes | refused: `root` changes; `dad11c2e-....7`, the body of БизнесПроцесс.Задание, which the source-asset registry does not name |
| S2 (227 edits) | none | yes | yes | passes |
| S3 (delta of 3 objects) | none | yes | yes | passes |
| S4 (new form and template) | none | no | no | passes |
| S5A (help, first template, first form) | none | no | yes | passes |
| S5B (form of a catalog) | none | yes | no | passes |
| S5C (object module added) | none | no | no | passes |

The one object the platform called changed is the one the gate refuses, by its body (the route-point
flowchart `Задание.7`, which the base-free importer recompiles); no stage the gate passed made the native
log report an object, and no new form or template is reported as «Новый объект» (forms and templates are
not objects for that log). The direction that matters -- a change the platform reports and the gate passes --
is empty on all seven stages; the wider matrix of edits is measured with the same oracle by track rcheck
(`docs/apply/restructuring-check.md`). The structure line marks the long path exactly: S1, S2, S3 and S5B have it,
S4, S5A and S5C do not (rows and descriptors against the trace track's rule: see "The short path").

Exclusivity with a real 1C process: an `ibsrv` (standalone server) started on a staged twin holds 23
connections; the apply refuses with the sessions listed, and a second apply started meanwhile is
refused by the in-transaction check (`THROW 57302`) -- both leave the database unchanged.

Cluster sessions (`docs/apply/evidence/own-apply/cluster-sessions.md`): the lab clone registered in the
8.3.27 cluster (`tools/register-ib.ps1`), sessions through the COM connector reading a probe function of a
common module. Before the apply a new session reads `ibcmd-rs-apply-probe-v1`; after the own apply a **new session reads
`ibcmd-rs-cluster-probe-v1`**. Second change, applied under an open (warm) session with `--exclusivity assumed`
(the default check refuses: three `1CV83 Server` connections): a session opened after it reads the new value
(`ibcmd-rs-cluster-probe-v2`) while the warm session still reads the old one 30 s later -- information for 0.5.

Session probe (`ibcmd_rs_04_apply_bsp8327_probe_20260929`, a clean БСП clone with a full-tree
stage of 9 517 rows -- the 227 S2 edits and a marker added to `ПоддерживаемыеВерсииПрограммногоИнтерфейса`
in `СтандартныеПодсистемыСервер`): a standalone server (`ibsrv`, integrated SQL login, `tools\srv.ps1`)
started cold on the staged, unapplied database answers the web service
`InterfaceVersion.GetVersions("UiProbe")` with status 200 and no version; after the own apply
(the gate passed, 9 517 rows) a server started cold again answers 200 with
`ibcmd-rs-apply-probe-v1` and an empty error stream (`tools\probe_http.py`). The own apply ran
under load from other tracks' native runs (transaction 120 s against 5.6 s on an idle machine).

Also exercised: a rehearsal (`--rehearse`) leaves the database identical to its before-snapshot;
another session on the database makes the apply refuse with the session named; the recovery
artifacts verify against the before-snapshots (`tools\verify_recovery.py`: every manifest row equals
the snapshot row, every saved file hashes to its manifest entry).

## Refusals a caller can sort

The refusals a caller has to tell apart are types in `mssql_config_apply` (`anyhow::Error::downcast_ref`),
with the messages they always had. The drop-in `ibcmd infobase config apply` (`src/dropin/apply.rs`) and
any other caller sort by type, not by text; `run_command` prints the same distinction as a JSON report with a
`refused` key. Nothing was written in any of them.

| Type | Fields | When | `refused` in the report |
|---|---|---|---|
| `StructuralRefusal` | the gate's verdict | the gate refuses the stage (message: the gate's own text, for the default gate `Verdict::refusal()`) | `needs_native_apply` |
| `NeedsNativeApply` | `command` (`NativeCommand::Apply` or `Repair`), `reason` | the stage or the base needs the native tool: a `deleted` list with a name the apply cannot account for, an overlay in `Params`, a `deleted_dynupdate_*` row, a new object on an empty change register, a new object on 8.5, an unfinished operation (`Repair`) | `needs_native_apply` (with `native_command`, `reason`) |
| `ExclusiveAccessRefused` | `database`, `sessions` (id, login, host, program, ...), `in_transaction` | other user sessions on the database; `in_transaction` is true when the in-transaction check (`THROW 57302`) found them after the plan had not | `exclusive_access` |
| `ExclusiveAccessUnprovable` | `reason` | exclusivity cannot be proved: no `VIEW SERVER STATE` (57301) | `exclusive_access_unprovable` |
| `BackupRequired` | none | a restructuring that writes, without `--recovery-backup` or `--i-have-a-backup` (Russian message naming both) | `backup_required` |

Refusals raised inside the transaction are mapped from the SQL Server error **code** of the `THROW` (57302
other sessions, 57301 no `VIEW SERVER STATE`, 57307 and 57316 an unfinished operation or schema state), not from
text. One test per type checks the message, the downcast and, for the transaction codes, the mapping; the drop-in's
tests (its side) switch from message matching to these types.

## Recovery artifact and its retention

The artifact went from thousands of files to **one**: the saved row bytes are appended to `rows.pack` and the
`.tsv` manifests refer to them as `rows.pack@<offset>+<length>`. A stage of the whole tree saved 9 176 files
(78 MB) in `%TEMP%` and took 14 s of a 62 s apply (Defender scans every file). Next to the pack: `manifest.json`
(schema 2: `pack`, counts, and `backup` when the apply took or was told of one), `config_replaced.tsv`,
`special_rows.tsv`, `change_registrations_before.tsv`, `MobileVersions.dat.before`, the new-object files, and a
`README.txt` that says how to read them back. `tools\verify_recovery.py` (lab) checks every manifest row against the
before-snapshot and every pack range against its hash.

**Retention.** Without `--recovery-dir` the artifact goes to
`%TEMP%\ibcmd-rs\config-apply-recovery\<database>-<16 hex digits>`; after a **successful** run the older
artifacts of **that database** beyond the newest five are removed (`--recovery-keep N`; 0 keeps all). Only
directories with that name pattern are candidates; a directory the caller names with `--recovery-dir` is never
touched, and a failed run deletes nothing. The volume that remains is what one apply saves (the replaced rows
and the manifests): 78 MB for a whole-tree stage of a 9 500-row base.

## Restructuring inside the apply (S1-A, #397)

The restructuring that track ddl developed (rebuilding the tables of a catalog or document for new attributes,
tabular sections, wider strings, the index flag, plain new catalogs and documents) runs **inside this apply's
transaction**, so a failed assertion rolls the rebuilt tables back with the rest and `ConfigSave` is emptied only at
the commit. Ported from `feat/0.4-restructure-s1` (not merged), where their spike does it in a transaction of its own.

**The seam.** A gate that lets a restructuring through hands the apply a `StructurePhase` through
`StructuralGate::take_structure` (default: none; the conservative gate and `ApplyCheckGate` refuse
restructurings and have none):

- `sql`: the structure work as T-SQL. The script runs it after the fingerprint assertions and `@now`, before the fold
  and the move; the text assumes the transaction, `XACT_ABORT ON`, the exclusive table locks, the variable `@now`,
  declares variables with the prefix `@ddl_` only and does not read `Config`.
- `params_rewrites`: the `Params` cache rows the phase makes stale (the XDTO model `.si`, the object registry `.si`,
  `siVersions`). They are merged with the apply's own guarded rewrites (`merge_params_rewrites`); a row **both**
  want -- the object registry `1a621f0f` and `siVersions` when one stage adds a form and an attribute -- is a
  **refusal** (`NeedsNativeApply`): apply the two changes in two steps.
- `tables`, `objects`, `caches`: what the report names (`structure`), and the apply's `tables_touched` and
  `not_written` follow from it.
- `created` (uuid, kind and staged files of each catalog or document the phase creates, S1-F, `new-object.md` 3.4) and
  `answered_rows` (the configuration's descriptor, which lists them): the plan's `objects::analyze` knows a new form or
  template only and blocks a new catalog or document, so the apply takes the phase first and drops the blockers on these
  rows; the staged rows move like any staged row, and each created object is registered like a new form, at every node of
  the exchange plans but the plans' own, with its files as the list in `_ConfigChngR_ExtProps` (#412; measured on
  8.3.27 only: on 8.5, or on a change register that has no rows, `NeedsNativeApply`). The size guard counts a table the
  database does not have yet as empty.

`--allow-restructure s1` (`ConfigApplyOptions::allow_restructure`) picks the restructure track's S1 gate in `structural_gate()`
(`restructure::s1::S1Gate`, wired on `feat/0.4` by track ddl; the gate also judges the stage's `deleted` row of removed attributes:
`StructuralGate::judges_deleted_row`, `StructurePhase::consumed_staged_rows`). **The script tests of the seam** (all in
`cargo test --lib mssql_config_apply`):

| What #397 asks | Test |
|---|---|
| the structure phase runs between the checks and the move, inside the transaction, also ahead of a dynamic fold; without a phase the script is what it was | `sqlgen::a_structure_phase_runs_inside_the_transaction_between_the_assertions_and_the_move` |
| a rehearsal rolls the phase back with the rest; the phase text stands once | `sqlgen::a_rehearsal_rolls_the_structure_phase_back_with_the_rest` |
| the cache rows of a phase are written guarded by the digest the plan saw, after the phase, in the same transaction | `sqlgen::the_cache_rows_of_a_structure_phase_are_written_guarded_after_it_in_the_same_transaction` |
| a conflict of two `Params` edits is refused (the object registry `1a621f0f` / `siVersions` wanted by the phase and by a new form), distinct rows join | `tests::a_cache_row_both_the_restructuring_and_a_new_form_want_is_a_refusal` |
| no backup word: a structural apply that writes refuses, by type, in Russian, naming both options; a dry run and a rehearsal need none | `tests::a_structural_apply_that_writes_needs_a_word_about_a_backup`, `errors::backup_required_is_typed_russian_and_names_both_options`, `tests::the_backup_is_named_in_the_report` |
| `--allow-restructure s1` builds the S1 gate; without it the gate is the check | `tests::the_s1_class_builds_the_s1_gate_of_the_restructure_track` |

**Acceptance: the types case through `mssql-config-apply` equals native, checks 1-10** (`docs/apply/evidence/own-apply/s1a-types-case-checks.txt`).
Twins of ddl's staged backup of T1 (`F:\ibcmd\lab\04\restructure\bak\...t1_staged.bak`, read only) on the БСП 8.3.27 base, ddl's kit
(`scripts/restructure-lab`, not modified) for the checks and the apply lab's own runners for the native commands. **The case is T1 without
the three objects the extension `_ДемоРасширение` adopts** (`_ДемоМестаХранения`, `_ДемоГруппыДоступаПартнеров`, `_ДемоПартнеры`): since
S1-I (#405) the own restructure refuses to change an object an extension adopts (the full T1 is refused with that reason: `S1: catalog
_ДемоМестаХранения is adopted by the extension _ДемоРасширение ...`), so the three were unstaged on both twins. What runs: the catalog
КлючевыеОперации (attributes of Булево, СтрокаПеременная, СтрокаФикс, СтрокаНеогр, ЧислоЦелое, ЧислоДробное, ЧислоНеотр, Дата, ДатаВремя),
the catalog Удалить_ДемоОбщиеСведения (a string) and the document _ДемоЗаказПокупателя (a number, Булево, Строка, Число, Дата, ДатаВремя,
СтрокаНеогр): 6 tables rebuilt. **Not covered by this run**: the types Время and ЧислоПапки, which only `_ДемоПартнеры` carried (the full
T1 was compared before S1-I: `s1-port-acceptance.txt`, 11 tables, checks 3-8).

| # | Check | Result |
|---|---|---|
| 1 | the plan made offline from the staged snapshot equals the native result | `corpus_plan_of_the_types_case_equals_the_native_result` passes (ddl's snapshots of the full T1, read only) |
| 2 | tables, columns, indexes | identical but the drift list: `_DbCopies*`, the auto-named primary keys of `_ConfigChngR`, `_Reference2598`, `_Reference9367` |
| 3 | data of the 6 rebuilt tables | `EXCEPT` both ways: 0 rows in every table (716, 3, 8, 7, 4, 1 rows) |
| 4 | `Config` rows, `Creation`/`Modified` included | 0 rows on either side (9 841 of 9 841) |
| 5 | `DBSchema` entries and `DBNames` | 1 761 tables; entries equal but `DbCopies`, `DbCopiesUpdates`; `DBNames` text equal (348 071 characters) |
| 6 | the 16 `.si` rows | 16 of 16 have the same text |
| 7 | a native `config apply` on our twin | «Обновление конфигурации базы данных не требуется» (3.4 s); native `config check` succeeds |
| 8 | native `config export` of both twins and `source-diff` | 12 198 files, 0 differing |
| 9 | a session in the 8.3.27 cluster on both (external connection: defaults, write, read back through the object and a query, condition on a new attribute, delete) | 58 lines, identical on the two twins, twice; the tables have their row counts again |
| 10 | a rehearsal changes nothing | `snapdiff` before and after the rehearsal: no table added, removed or changed, no data change, `DBSchema` the same |

The real run: gate 7.4 s (debug build), transaction 4.2 s, 12.6 s in all, against 155 s of the native apply. Without a backup word
the same run refuses before writing (exit 1, the Russian text of `BackupRequired`); `--i-have-a-backup` is recorded in the report. The
change register (`tools\reg_cmp.py`): the same 20 685 rows and file lists on both; the message numbers differ as in the known difference of
the long path (native 3 NULL per node, ours the NULL the corpus came with; not visible in an exchange message,
[exchange plans](#exchange-plans-of-distributed-infobases-412)). Checks 11 (refusals) and 12 (a failure inside the transaction) belong to
the case-by-case protocol of the restructure track; the refusals of this seam are the tests above and the adoption refusal seen here.

**Backup policy for structural applies.** A restructuring drops the old tables inside the transaction; the recovery
artifact keeps the `Config` rows and the caches, not the tables. So an apply that **restructures and writes** refuses
unless the operator names a way back:

- `--recovery-backup <file>` (recommended): before the transaction the apply takes
  `BACKUP DATABASE ... TO DISK = <file> WITH COPY_ONLY, COMPRESSION` (a path the SQL Server service can write),
  refuses if the file exists, fails without touching the database if the backup fails, and names the file in the
  report (`backup`: kind `file`, `path`, `seconds`; `timings.backup_ms`) and in the recovery artifact
  (`manifest.json` `backup`, `README.txt`). Measured on the 8.5 twin: 5.2 s for a 1 GB database, the backup taken
  before the recovery artifact.
- `--i-have-a-backup`: the operator says they have one; recorded in the report as `backup` kind `acknowledged`.

A stage that does not restructure needs neither. `--dry-run` and `--rehearse` need neither and write nothing that
stays (the plan prints a warning that a real run will need one). The refusal is `BackupRequired`, in Russian, exit 1,
and names both options; the drop-in accepts the same two options (its side, `ConfigApplyOptions::backup`,
`BackupPolicy::{None, Acknowledged, File}`). Tests: the rule, the type and its text, the report, the ordering of the
backup before the recovery artifact and of both before the transaction.

## ERP UH 8.3.27 at scale (#392)

**Base.** The corpus `uha8327` (ERP УХ on 8.3.27), restored with `restore-clone.ps1 -Corpus uha8327`: 118 377 `Config`
rows (51 second parts, 1 638 572 555 bytes), database 4.3 GB, 21 187 tables, no infobase users, `SchemaStorage` state
100, 156 dynamic alias rows of two generations with no `DynamicallyUpdated` marker (the plan warns that they belong to no
generation and leaves them), `_ConfigChngR` **empty** (no node registers changes, so `_MessageNo` has nothing to reset, no node exists to
insert a row for, and a new form or template is refused). The twin of the staged clone is
a `BACKUP ... COPY_ONLY` of it, restored (`uha_stage_a.bak`, 1.7 GB); native runs took the heavy lock and the native lock,
one command per hold; ours the heavy lock.

**Stage A: 300 edited common modules** (a comment line appended; 603 rows, 5.5 MB).

| | Native `config apply --force --dynamic=disable` | This apply |
|---|---|---|
| Time | 639.8 s | **34.8 s** in all: transaction 1.4 s, gate 20.0 s, inventory 11.0 s, recovery 1.4 s, fingerprints 0.2 s |
| Transaction log | | +17.5 MB |
| `Config` | | **118 377 of 118 377 rows identical** (per-row SHA-256 of name, part, size, attributes, timestamps and bytes) |
| `Params` | | differ only in what the native apply writes at every apply: 16 `.si` rows re-encoded with unchanged text and `siVersions`, the two `.ui` rows, and the identity GUIDs of `ibparams.inf` and `locale.inf` |
| `Files` | | `MobileVersions.dat` only |

The native `config export` of the own-applied twin (887 s, 140 709 files) against the reference export of the same
corpus: exactly the 300 edited files differ, and each equals the edited source (300 of 300). The native twin's export was
not made: `Config` is equal row for row, and the export is a function of it.

**Are native's extra writes needed?** Tested on the own-applied twin, in this order:

1. native `ibcmd infobase config check`: succeeds (121.2 s);
2. a probe stage (one module of 4 MB, marker `v1`) applied by this apply (33.8 s in all), then a **new session in the 8.3.27
   cluster** (the twin registered with `register-ib.ps1`, COM connector, no user): it reads `ibcmd-rs-uh-probe-v1` (opening
   a session on this base takes 55 to 75 s);
3. a **later native apply** of a second probe stage (`v2`) on the same twin: exit 0 in 70.3 s ("Проверка корректности
   метаданных", "Принятие изменений", a new generation; no structure phase, no help index), and a new cluster session reads
   `ibcmd-rs-uh-probe-v2` (63 s).

So the minimal write set is enough on this base: the `.ui` rows, the `.si` re-encodings and `siVersions`, the identity
GUIDs of `ibparams.inf`/`locale.inf` and the `_DbCopies*` entries are not needed for a check, for a session or for a later native
apply, which writes what it wants. This apply never copies the identity GUIDs.

**Stage B: the whole configuration** (every published row, no aliases, a new `versions` generation: the worst case of the
importer that stages nearly all rows; 118 221 rows, 56 758 descriptors, 61 460 bodies, 1 634 192 780 bytes):

| Phase | Dry run (cold cache) | Real run |
|---|---|---|
| storage check | 1.3 s | 0.1 s |
| inventory | 141.5 s | 40.0 s |
| gate | 27.8 s | 21.2 s |
| fingerprints | 54.3 s | 91.0 s |
| recovery artifact | - | 19.5 s |
| SQL (the transaction, with its own re-verification and the move) | - | 526.9 s |
| **total** | **226.1 s** | **699.4 s** |

Transaction log (recovery model SIMPLE, sampled every 5 s from `master` so that the sampler holds no connection to the
target): the log file grew from 72 MB to **9 224 MB**; peak used **9 162 MB**, of it the transaction's own 4 894 MB used
and 3 496 MB reserved -- about **5.6 times the bytes moved**. The data file grew from 4 296 to 5 704 MB. A whole-tree apply
therefore needs log space of about six times the stage and data space of about its size, on the volume of the log file
(F: had 540 GB free). The gate's share of a whole-tree run is small (3 %); the plan's hashing is not: inventory and
fingerprints together take 131 s of the 699 s (and the transaction hashes the same rows again for its assertions).
Proposals: skip rows byte-identical to `Config` (all but 300 of the 118 221 in stage A are), and reuse the plan's hashes in
the transaction; neither is done.

## 8.5 (#392)

The blanket refusal of the 8.5 profile is lifted. Capability `mssql.config.apply` (profiles/platform/*.json; documented in
`profiles/README.md`) admits the own apply per build; it is independent of `mssql.main.write`, which stays unsupported on
8.5. `8.3.27.1989` declares neither. New forms, templates and bodies are measured on 8.3.27 only and are **refused on
8.5** (`NeedsNativeApply`). `verify_mssql_storage_profile` checks the new capability.

**Evidence: twins of the БСП 8.5.1.1150 corpus** (`bsp85_*`, 9 948 `Config` rows, 1 GB), the same stages given to the
native apply on one twin and to this apply on the other:

| Stage | Native | This apply |
|---|---|---|
| A: 12 edited modules, 27 rows, 412 KB | 233.8 s | **13.0 s** in all (transaction 8.0 s) |
| C: one module, 5 rows, 366 KB | short (no structure phase) | 6.3 s |

After both: `Config` **9 948 of 9 948 rows identical**; `_ConfigChngR` (21 219 rows): the 42 rows of the staged objects equal,
the others differ by the `_MessageNo` normalisation of the first native apply (known difference); `ExtProps` (16 769
objects) and `Params` (39 rows) equal, `siVersions` 16 entries equal; `.si` texts permuted by the native apply (known
difference); `Files` 42 rows against 369 (help-index chunks). The native `config export` of both twins: **12 337 files, all
identical**. A **new session in the 8.5 cluster** (`register-ib.ps1 -Platform 8.5`, COM connector `V85.COMConnector`,
`localhost:3541`, user «Администратор (обычное приложение)») reads the applied change: `ibcmd-rs-85-probe-v1` after the first
own apply and `ibcmd-rs-85-probe-v2` after a second one (`--exclusivity assumed`: the cluster's working process holds
SQL connections while a session is open). The backup policy
(`--recovery-backup`) was also run end to end on this twin (5.2 s).

Two remarks for the lab tools, not for this program: `register-ib.ps1 unregister -Platform 8.5` fails for an infobase
whose administrator is «Администратор (обычное приложение)» (it falls back to the name `Администратор`); the twin was
removed with `rac infobase drop` under its own name and uuid. And the drop-in keeps a refusal of 8.5 of its own
(`src/dropin/apply.rs`); with this branch it can go (it went in rcheck-6, together with the text matching of the errors).

## Exchange plans of distributed infobases (#412)

**The question.** The change register `_ConfigChngR` (rows `(_NodeTRef, _NodeRRef, _MDObjID, _MessageNo, _IDRRef)`) and
`_ConfigChngR_ExtProps` (the object's changed files) decide which configuration objects an exchange message for a node of a
distributed infobase (DIB) carries. The own apply reset `_MessageNo` in the rows that existed and inserted rows for new objects;
was the message for a node after it the same as after the native apply?

**Setup (lab only; scripts in `F:\ibcmd\lab\04\apply\tools`, not in the repository).** The БСП 8.3.27 clone (`bsp8327`) has the plan
`_ДемоОбменВРаспределеннойИнформационнойБазе` (`DistributedInfoBase`, the plan's own node ДМ and the nodes ПА, ПБ, МД, all with register
rows for 4 929 to 5 749 objects, `_MessageNo` NULL for 4 929 of them as the corpus came). A **cluster session writes to the clone
only**: the clone is registered in the 8.3.27 cluster (`register-ib.ps1`), an external connection (`V83.COMConnector`) creates nodes
(`СоздатьУзел`, `Записать`), writes a message (`ПланыОбмена.СоздатьЗаписьСообщения`, `НачатьЗапись`, `ПланыОбмена.ЗаписатьИзменения`,
`ЗакончитьЗапись`) and reads an acknowledgement (`СоздатьЧтениеСообщения`, `НачатьЧтение`). Two things the platform demands before it
writes a DIB message: no data-changing extension that is not used in the DIB, and no disabled DIB extension in the session (the corpus
has four extensions: two were deleted, `ServiceDesk` was marked "used in DIB" and left active). **An initial image**
(`ПланыОбмена.СоздатьНачальныйОбраз(Узел, "File=...;")`, a 282 MB file infobase) was made for ПА and for a new node ЯТ1; a second new node
ЯТ2 has no image. An imaged node has **no rows** in `_ConfigChngR`: the platform deletes them when it creates the image (ПА's 5 749
rows were gone), and a node made afterwards has none either.

**What a message carries.** The body is `<v8de:Changes>` with `<v8de:Config>` (one `<v8md:Metadata>` per configuration object: `ObjectID`,
`ClassID`, `Version`, `Name`, `Content` = the descriptor, and `<v8md:Externals>` = the object's listed files that exist in `Config`), the
extensions, the nodes and the data. Measured: **(1)** a node **without an image** gets the whole configuration (4 929 objects, 4 906
files) in every message, whatever its register rows say (the register as the corpus came, set to all zeros, or after a bare acknowledgement message made by hand
-- `ReceivedNo` 1, no body), so the register differences between the applies are invisible there; **(2)** a node **with an image** gets exactly
the objects it has a row for, with their listed files, and after the message the rows carry the message number.

**What the native apply does.** For every object a stage changes (the uuid a staged name starts with, the objects whose file lists name a
staged row, and the owners of the rows a `deleted` list names) and every node of the plans that register changes except the plan's own
node: **updates the row's `_MessageNo` to NULL, or inserts the row** (`_MessageNo` NULL, a new `_IDRRef` from a clock) **where the node has
none** -- for the imaged nodes ПА and ЯТ1 and for the plain new node ЯТ2 alike. The inserted row's file list is the **changed files** of the
object: the staged and dropped body files that are its own (`<uuid>.<n>`, `<uuid>_dynupdate_<g>.<n>`), the new form's `.0`, the alias body
of an overlay owner; **empty** for an owner whose descriptor alone is staged. (Measured on twins in the five cases below and on scratch
copies with the register inspected: `tools\reg_inspect.py`, `reg_objects.py`.)

**What was wrong.** This apply updated the existing rows and inserted rows for new objects at the nodes the register already held. An
imaged node has none, so nothing was registered for it: **after the own apply the message for the node carried none of the change**
(objects in the native message against ours: 4 against 0, 3 against 0, 8 against 0, 30 against 0, 6 against 0), and the subordinate would
have kept the old configuration. (The earlier rule "never insert register rows for existing objects" came from the УХ base, whose register is
empty: no node registers changes there.)

**The fix** (`registrations.rs`, `sqlgen.rs`, `recovery.rs`): the plan reads the nodes of the plans that register changes **from the plans'
node tables** (`_Node<plan>`, `_PredefinedID` = 0 for a node that is not the plan's own), not only from the register; lists the changed objects
the register knows at some node and the (node, object) pairs with no row; the file list of each. The transaction asserts the nodes' count per
plan and that exactly the planned rows are missing, inserts them (`_MessageNo` NULL, ids continuing the table's greatest) and their lists;
the owners of the rows a `deleted` list names are reset like the owners of staged rows, and the bodies it names are appended to the lists that
exist. A node marked for deletion makes the apply refuse (`NeedsNativeApply`, unknown). The recovery artifact records the inserted pairs
(`added_registrations.tsv`); the report has `registrations` (nodes, changed objects, rows and file rows added). Cost: a whole-tree stage of
the БСП clone with five nodes (4 929 objects) plans 14 787 rows and 14 718 file rows in the 9 s of a dry run and inserts them in a 22 s
transaction (debug build).

**Proof: the message for the node after the own apply against after the native apply**, twins of the imaged base (`ПА`, `ЯТ1` imaged, `ПБ`, `МД`
plain), the same stage on both, then one message for each imaged node from each twin (`tools\dib_case.py`, compared by `msg_survey.py`:
`ObjectID`, `ClassID`, `Version`, `Name`, SHA-256 of `Content`, and of every external with its name and version):

| Case | Stage | Native message | Own apply before | Own apply after |
|---|---|---|---|---|
| module change | 4 common modules, 11 rows (native: short path) | 4 objects | 0 | **equal** |
| new form and template | delta: the two new objects, the owner's descriptor, `versions` | 3 objects (lists `[.0]`, `[.0]`, `[]`) | 0 | **equal** |
| overlay removal (E3b) | 6 modules and a `deleted` list naming the 6 overlay rows | 8 objects (6 and the 2 owners; the owners' lists name the alias body, which is not in `Config`, so no externals) | 0 | **equal** |
| long path | 30 modules (native: register rebuilt) | 30 objects | 0 | **equal** |
| empty `deleted` list on the overlay base | 6 modules, overlay folded | 6 objects | 0 | **equal** |
| two applies in a row | 4 modules, then 30 others, no message between | 34 objects | - | **equal**, with the node acknowledged or not |

The register itself (`tools\reg_cmp.py`, the final build): the same rows, message numbers and ordered file lists in the module change, the
new form and the empty-list cases; the long path differs in message numbers only (native 0, ours NULL at the nodes that hold NULL: 12 326 rows in
M30, 12 328 in the two-step case, all of them at nodes whose messages do not depend on it); the overlay removal differs in two list rows of
the plans' own nodes (below). Nodes without an image: the messages carry the whole configuration and were **equal**
as well (module change with the register as it came and with all zeros; overlay removal with all zeros; the new-form case was not run
for them). The M30 and D6 messages were written before the last two fixes of the script order and the node check, which only concern new
objects and dropped bodies; the registers of all five cases were compared again with the final build. One run where the own apply refused: a delta stage on the overlay base whose `versions` was not based on the overlay's row (the default
gate's finding of checkpoint 2, `versions must be based on the effective row`).

**Not reproduced** (each measured or bounded):

- **`_MessageNo` NULL -> 0 at the nodes the long path rebuilds**: invisible in the messages (above); the own apply keeps NULL, which is what
  a message would send anyway. If a later exchange reads the number (an acknowledged node with a number in the register), it reads what the
  platform wrote, not what the native apply would have.
- **The alias body's name in the file lists of the plans' own nodes** (E3b): the native apply appended it to the lists of every node
  when the register was all zeros, and to the lists of the nodes that are not the plan's own when the rows were NULL already; this apply
  appends it everywhere. Two list rows differ in the second case (an own node is nobody's recipient: no message reads them).
- **Row ids**: the native apply numbers inserted rows from a clock (`8F5B00E0...`), this one continues the table's greatest id. Unique, and
  `_IDRRef` is not part of a message.
- **The order of a multi-file object's list** (only the names' order is used) and objects that no node registers (the native rule is not
  measured: the apply skips them). A database whose every subordinate node is imaged and whose own-node rows do not know an object cannot
  tell that the object is registered: it is skipped, not guessed.

## Limits and open points

- **New rows**: only a new form or template of an existing object (bodies `.0`, and `.1` for
  a form) and a body row an existing object gains are done; a new catalog, attribute, command,
  subsystem or any other object is structural or needs records this apply does not know, and
  is refused. Several new body rows of one object are refused (the order of their
  registration is not known). The importer's patch mode cannot stage a new form or template
  yet (`Config row not found`); a delta stage made by hand or by the base-free import can.
- **Sessions**: new sessions see the change in the 1C cluster (8.3.27, clients `localhost:2541`): see
  "Cluster sessions" above and `docs/apply/evidence/own-apply/cluster-sessions.md`. A session that was
  open before the apply keeps its old configuration (read the old value 30 s after the apply); the
  apply therefore demands exclusive access and refuses while the working process holds its connections.
- **Dynamic-update overlays in `Params`** (a `.si` row under a `_dynupdate_` name, left by a native
  dynamic apply) are refused: this apply folds only `Config` overlays.
- **Big stages**: a row above 10 MB (several parts) is moved as the stage has it (S6, a hand-made stage); one of the roles that
  need a text comparison is refused when it has parts. The whole apply is one transaction: a stage of 1.6 GB (the whole
  ERP УХ configuration) needs a log of about six times its size (9.2 GB measured) and took 699 s; see
  [scale](#erp-uh-8327-at-scale-392). (The S2 stage, 9 517 rows and 81 MB, took 5.6 s of SQL on an idle machine and 120 s under load from other tracks.) The importer stages every row of the tree
  although only the edited ones differ from `Config` (9 517 staged, 9 515 identical in the cluster proof); the native apply
  moves them all too. A mode that would leave the byte-identical rows out is possible, and would differ from the native
  apply only in `Creation`/`Modified` of those rows -- a proposal, not done.
- **Removals**: a `deleted` list is executed for the forms and templates the analysis accounts for name by name (their rows are
  deleted with the two search-information records; [removals](#removals-the-stages-deleted-row)), for the rows of a dynamic update, and
  for the attributes the S1 gate judges; any list with a name it cannot account for is refused whole. Not done: one file of an object
  that stays, common modules and other objects the rest of the configuration mentions, objects with tables, 8.5.
- **8.5** is admitted for the same stages as 8.3.27 minus new objects (see [8.5](#85-392)); on any other 8.x profile the apply
  is refused.
- **Restructuring**: `--allow-restructure s1` runs the restructure track's S1 gate in the apply's transaction (attributes added or
  deleted, strings widened, the index flag; the operations built by the track); an object an extension adopts is refused (S1-I); a
  structural apply needs `--recovery-backup` or `--i-have-a-backup`. Proven on the БСП 8.3.27 and the БСП 8.5.1.1150
  (`evidence/dropin-apply/s1-acceptance.md`, `s1-acceptance-85.md`) and the ERP УХ 8.3.27 on two cases with empty tables (`s1-acceptance-uha.md`); ERP УХ 8.5 and the
  copy of a large table are not measured.
- **Exchange plans**: proved on one exchange plan (`_ДемоОбменВРаспределеннойИнформационнойБазе`, DIB) of the БСП 8.3.27
  clone; an object with no register row at any node is not registered (the native rule for it is unknown); a node marked for
  deletion makes the apply refuse (`NeedsNativeApply`); the file list of an inserted row is measured for one-file objects
  and for a descriptor-only owner; a multi-file object's order is the names' order. See
  [exchange plans](#exchange-plans-of-distributed-infobases-412).
- The help index and the extension CAS garbage are left as they are; they are caches.
- A working process that keeps a pooled connection makes the SQL exclusivity check refuse; the
  native standalone `ibcmd` does not check at all.
