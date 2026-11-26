# The import that carries what the target's rows cannot (#388 step 2, #393 import side)

Checkpoint 2 made `ibcmd infobase config import` refuse what a patch stage drops (`docs/import/guard.md`). Step 2
makes the stage carry it: an attribute, a tabular section, an enum value, a property, a subsystem's content, a new
catalog, form or template, a removed form or template, predefined items, the version of the configuration. The
guard stays on and checks the result of every case.

## 1. What the stage does

A stage into a database that holds the configuration (`stage_mode: patch`) still starts from the target's own rows and
takes from the tree what it can carry: names, synonyms, comments, modules, forms, templates, rights, command
interfaces. What differs from the target in a way those rows cannot carry is now built from the tree
(`src/mssql/override_stage.rs`):

1. **What differs** is asked of `apply_check::check_tree_against_db` (track rcheck): every metadata file of the tree
   against the descriptor the target stores for the same uuid, as the target's export would write it. It answers per
   object: `Added` (the target has no descriptor of this uuid), `Changed`, `Removed` (the target has it, the tree does
   not). With `--path-prefix` only the objects under the prefix count and nothing is removed (a partial tree is not a
   configuration).
2. **A new object** (a catalog, a form, a template, a common module, ...) is built whole the way a base-free stage
   builds it (`prepare_empty_object`): its descriptor and every body row, compiled from the tree. Its descriptor
   row takes no `Attributes` from a Config row (`Kind 0`), and its names are appended to `versions`.
3. **A changed descriptor**: the descriptor is compiled from the tree and used when it is not the row the stage would
   leave anyway. The comparison is between the compiler's text and the patched row (the target's row with the name,
   synonym and comment of the tree), and it matters: the check compares exported XML, so an object whose export
   differs from its file only in how the model writes it would count as changed, and a change of the header alone
   is carried by the patch. In both cases the target's row stays - with the state the XML does not carry, such as
   the always-used flag of a constant, which is copied into a compiled constant in any case. An object a patch
   stage does not stage at all (a nested subsystem) is built whole when its compiled descriptor differs from the
   target's.
4. **Predefined data**: the stored row is patched item by item as before. When the tree's set of items is not the
   row's (an item added, an item removed; the nil-uuid root row of a hierarchical catalog is not an item) the row
   cannot be patched, and the object is built whole with its predefined data compiled from the tree.
5. **What leaves the configuration.** The rows of an object the tree no longer has (its descriptor and every
   `<uuid>.<n>`) are listed in the platform's `deleted` row and their names go out of `versions`. The rows of an
   online (dynamic) update of the target that are still pending (`DynamicallyUpdated`, `<uuid>_dynupdate_<generation>`,
   `versions_dynupdate_<generation>`) are listed too, as the platform's own import lists them.
6. **The rows a stage patches start from what the storage publishes.** When an online (dynamic) update of the target
   is pending, its alias rows (`<uuid>_dynupdate_<generation>`, `versions_dynupdate_<generation>` first among them)
   hold the current content, and the staged `versions` is based on that one: the apply's gate flags ids that come from
   the plain row as unknown otherwise. The rule that names the alias is the export's overlay, the same one the online apply's
   row-by-row stage asks (`mssql_dump::stored_row_name`, `docs/apply/online-activation.md` section 6.4).
7. The rows are staged over the target's as before (one transaction that replaces ConfigSave), with two changes of
   shape that make ConfigSave the platform's own: the dates are 2000 years ahead (the platform's `_YearOffset`, 4026,
   where the stage wrote 2026), and a row larger than 10 000 000 bytes is written in parts of that size, each part
   carrying the whole `DataSize`.
8. **The guard** exports the state the apply would leave - the target's rows as the storage publishes them, the
   `deleted` names removed, the staged rows in place - and compares it with the tree. Whatever the build got wrong
   still refuses the import.

An object the tree names but cannot be built from (a writer refuses its file, a reference names an object the tree
has no file of) refuses the import before ConfigSave is touched, in Russian, with the file and the reason.

## 2. The `deleted` row

The platform writes it as raw deflate of a UTF-8 text with a byte order mark: the number of names, then each name in
quotes followed by `,0` (the flag of a Config row):

```
11,"0384ec55-...",0,"0384ec55-....0",0,"8a7546f4-....0",0,"8a7546f4-...",0,"8a7546f4-....1",0,
"a627e390-..._dynupdate_06cb0442-...",0, ... ,"DynamicallyUpdated",0,"versions_dynupdate_06cb0442-...",0
```

For the tree with a form and a template removed, the native import (`bsp_nat2`) and ours list the same 11 names: the
five rows of the two objects and the six rows of the online update the БСП clone carries. The order differs (native
lists the rows of an object in an order of its own, the dynamic ones by name); the apply does not depend on it.
Native also drops the names `root`, `version` and `versions` from its `versions` and gives every name a new
generation; a patch stage keeps the target's entries and generations for the rows it does not stage.

Not written: the entries the platform makes for removed *children* of an object (an attribute, `<uuid>,1`), and the
rows of a module or another file of an object that stays. See section 6.

## 3. Acceptance on БСП 8.3.27

The lab's kit: `scripts/import-lab/run_override_acceptance.ps1` (our drop-in import with its default flags into a
clone, a summary of what it left in ConfigSave, the platform's own `config apply --force --dynamic=disable` and
`config export`, `source-diff` of the tree against the export) and `run_guard_acceptance.ps1 -Expect override`
(every edit of `edits.py` through the import, the guard on).

| tree | ours import | native apply | native export against the tree |
|---|---|---|---|
| `add5`: an attribute, a tabular section, a new catalog, a synonym, a new form | exit 0; 2 objects built, 4 descriptors compiled, guard 12 201 files | exit 0 | 12 201 of 12 201 files identical, `ConfigDumpInfo.xml` aside |
| `rem2`: a form and a template removed | exit 0; 2 descriptors compiled, 5 rows removed, 11 names in `deleted`, guard 12 190 files | exit 0, Config 9 836 rows (9 847 - 5 - 6) | 12 190 of 12 190 files identical, `ConfigDumpInfo.xml` aside |
| `all19`: the two above and 12 more edits (a property of a catalog, a property of an attribute, a removed attribute, an enum value, a subsystem and a nested subsystem, a command interface, a module, a predefined item added and one edited, the version of the configuration, a new template) | exit 0; 5 objects built (3 new), 10 descriptors compiled, 5 rows removed, 11 names in `deleted`, guard 12 196 files | exit 0 | 12 196 of 12 196 files identical, `ConfigDumpInfo.xml` aside |

`add5` and `rem2` are the acceptance of #388 criteria 1 and 2 and of #393's import side. The twin of the second:
native `config import` of `rem2` into a second clone and native apply give a Config with the same 9 833 row names as
ours (no name only on one side). In `all19` the native apply also dropped the column of the removed attribute
(`_Reference3347` has 8 columns against 9 in an untouched clone) although the `deleted` row does not name the
attribute: the platform finds the removal itself.

Run again on the merged `feat/0.4` (1d55fe46, with master's #387 and track apply's checkpoint 2) with the final
binary: `add5` 12 201 of 12 201, `rem2` 12 190 of 12 190, `all19` 12 196 of 12 196 files identical (native apply 257 s,
451 s and 80 s on a busy and a quiet machine).

The regression of the whole matrix (`run_guard_acceptance.ps1 -Expect override`, 24 cases, the import only, guard on,
the clone never applied): every edit of `edits.py` loads and passes the guard (22 edits and the control; the removed
template `tpldel` is the new one), and the two edits that leave a reference to a removed catalog (`catdel`,
`catfile`) are refused in Russian with ConfigSave untouched: 24 of 24 as expected. An import of a tree the clone
already holds (the applied `all19` onto its clone) builds nothing, compiles nothing, lists no name in `deleted` and
passes the guard: a second import is a no-op.

The tree with all 21 edits also loads and passes the guard; after the native apply its export differs from the tree in
three files, each an inconsistency of the edits and not of the stage: the role whose right the edit set to `false`
(the platform's export drops a `false`; `docs/import/patch-mode.md` section 8, item 6), and two forms of the catalog whose
predefined item `predefdel` removed while the forms' queries still name it (the platform marks their data paths
`~`).

### 3.1 Platform 8.5 (dialect 2.21) and the parent configuration row

Release continuation: [the 8.5 acceptance evidence](8.5-acceptance.md) covers the
native-empty/default-import cycle, an actual palette-color edit, and current
add/remove reruns with strict service-inventory checks against native twins.

The same kit on the 8.5 БСП (3.2.1.356; `run_override_acceptance.ps1 -Platform 8.5`, which runs the native ibcmd of
8.5.1.1150 as "Администратор (обычное приложение)"; `-ApplyWith ours` applies with the drop-in `infobase config apply`
instead of the platform's). The trees are the 8.5 native export with the same edits of `edits.py`.

| tree | ours import | apply | native export against the tree |
|---|---|---|---|
| one module comment | exit 0, 9 635 rows, 26 s, guard 12 336 files 4.7 s | ours: exit 0, 61 s | 12 336 of 12 336 files identical, `ConfigDumpInfo.xml` aside |
| the same, a twin clone | exit 0, 84 s (busy machine), guard 12.9 s | native: exit 0, 178 s | 12 336 of 12 336 |
| `add5` | exit 0; 2 objects built, 4 descriptors compiled, guard 12 339 files | native: exit 0, 113 s, Config 9 951 rows | 12 339 of 12 339 |
| `rem2` | exit 0; 2 descriptors compiled, 5 rows removed, 5 names in `deleted`, guard 12 329 files | native: exit 0, 127 s, Config 9 943 rows (9 948 - 5) | 12 329 of 12 329 |

The 8.5 БСП is a configuration on vendor support: it stores its parent configuration whole in one row,
`<configuration uuid>.<parent uuid>` (99 678 751 bytes in 10 parts, the file `Ext/ParentConfigurations/<name>.cf` of
the tree, 103 199 340 bytes, deflated twice with zlib level 9 and memLevel 9; Python's zlib reproduces both streams
byte for byte). The library the program deflates with cannot set memLevel 9 (flate2's C backend fixes it at 8), so a
stage used to write another stream for it (99 663 207 bytes inside, against the stored 99 664 295) that inflates to the
same file. The check of a stage against the target (`apply_check`, finding 10 of `docs/apply/restructuring-check.md`)
compares the row after one inflation and read this as a change ("Configuration: row .81401d17-...: content
changed"): the drop-in apply refused a stage of one module comment on 8.5, and the finding counted the row among the
differences of a base-free stage of an unchanged БСП.

A stage against a database now keeps the stored row (`stored_parent_configuration_row` in `src/mssql.rs`): it reads
the row with all its parts, inflates it twice and, when that is the tree's file, stages the row as it is (all 10
parts byte-identical to Config). Only a file that differs from the stored one is deflated again. What is not
changed: a base-free stage has no row to keep, so its parent row is still another stream of the same file. To make
that row equal to the platform's, the check must compare it after two inflations (it knows the row by its name:
the configuration's uuid and the parent's) or the deflate must set memLevel 9 (zlib-rs or libz-sys directly; flate2
does not offer it).

### 3.2 Our import and the drop-in S1 apply, against the platform's twin

The cases of the ddl track (`restructure-check\dn`: `b1` attributes deleted, `c1` strings widened, `d1` indexes
switched) and `attrdel` of `edits.py` (the first attribute of a catalog removed), on the БСП 8.3.27 base tree.
`scripts/import-lab/run_s1_case.ps1` (feat/0.4 at df27d983 and later: the drop-in `config apply --recovery-backup`
serves S1): a native twin (the platform's `import files --partial` of the edited descriptors, the platform's apply) and
an own twin (our `config import` of the whole tree, default flags, then `config apply --recovery-backup=<file>`), then
rcheck's checks against the native twin.

| case | our import | our apply | check 7 (the platform applies) | check 8 (exports) | check 3 (rebuilt tables) |
|---|---|---|---|---|---|
| `c1`: 6 strings widened in 5 objects | exit 0, 5 descriptors compiled, guard 12 197 files | exit 0, gate `s1`, backup file, 117 s | `не требуется` | native twin against ours and the tree against ours: 12 197 of 12 197 identical | 10 tables equal |
| `d1`: 12 indexes switched in 6 objects | exit 0, 6 descriptors compiled | exit 0, 106 s | `не требуется` | 12 197 of 12 197 | 11 tables equal |
| `attrdel`: an attribute removed | exit 0, 1 descriptor compiled | exit 0, 151 s | `не требуется` | 12 197 of 12 197 | `_Reference3347` equal |
| `b1`: 11 attributes removed, 2 added | refused | - | - | - | - |

Check 4 (every `Config` row by content, `compare_config_content.py`): the names are the same 9 838 in both twins and
the service rows are equal, but about 3 060 rows differ in content: the descriptors and bodies our patch stage
compiles from the tree are the same in meaning (the exports are identical) and not in layout (the gate counts 4 404
descriptors that differ only in layout), because a patch stage writes every row of the tree. The platform's twin keeps
the rows it did not stage. Rows equal to the platform's need the stage to write only what changed (#395).

**The S1 plan and the `deleted` row.** `restructure::plan::check_deleted` accepted only the removed attributes in the
stage's `deleted` row, so every import into a target that carries a pending online update (the lab's БСП clone does)
was refused: "the staged image deletes ..._dynupdate_..., which is not an attribute it removes". The platform's own
import lists those rows there (flag 0), and so does this program's (section 2). The plan now lets a flag-0 name with the
mark of an update row through (`DynamicallyUpdated`, `versions_dynupdate_<g>`, `<uuid>_dynupdate_<g>[.0]`), and refuses
anything else as before. That is a change to the ddl track's module, made here because the acceptance needs it.

**`b1` is not a tree the platform imports.** The case edits the descriptors only; nine forms of the tree still bind
the removed attributes (`Объект.Ставка`, `Список.КоррСчет`, ...). The platform's `import files --partial` of those forms
and descriptors is refused: `Неверный путь к данным: "Объект.Ставка"` for every one of them (the native twin of the
case does not stage the forms, so it never meets this), and after its apply the platform exports those forms with
`<DataPath>1/0:<uuid of the attribute></DataPath>`, not the tree's text. This program refuses the tree as well: the
native form writer says "`<InputField>` binds to Объект.Ставка, which the writer cannot place" for each form, and
the patch of the target's layout that follows fails ("cannot patch Form UseForFoldersAndItems without an existing
property bag"). Both refusals stop the import before ConfigSave is written. A consistent `b1` must drop the bindings
from the forms too.

## 4. Cost

The comparison of the tree with the target is the new work of every patch stage, the build of the objects is the new
work of a stage that has something to build. Measured with the stage's own timing lines
(`IBCMD_RS_STAGE_TIMING=1`, `run_uha_import.ps1`):

| stage of an unchanged tree | compare the tree with the target | base rows read | guard | rows into tempdb | move into ConfigSave | whole import | peak memory |
|---|---|---|---|---|---|---|---|
| БСП 8.3.27 | 1.1 s | 3.9 s | 3-12 s | 7 s | 3-126 s (SQL Server, machine load) | 27-44 s on a quiet machine | not measured apart |
| ERP УХ 8.3.27, a real database, a quiet machine | 20.9 s | 19.8 s | 63.5 s (rows 8.4 s, export 54.9 s) | 14.2 s | 120.3 s | 725 s wall, 1 316 s CPU | 8.5 GB |
| the same on a machine busy with other work | 76-114 s | 64-110 s | 157-281 s | 45-84 s | 250-256 s | 1 476-1 926 s wall | 6.6-8.5 GB |

On УХ the tree is the unchanged native tree: the comparison left nothing to build or compile among the 56 758
descriptors, the guard compared 140 708 of 140 708 files identical, and the 156 pending online-update rows of the
clone went into `deleted`. A stage with changes adds the setup of the base-free context (the walk, the reads of all
descriptor XMLs, the name index): 64 s on the base-free stage of УХ, once, when there is an object to build.

The guard on a base-free stage of УХ (offline, `run_uha_guard.ps1 -BaseFree`): 77.8 s on a stage of 109.5 s on a quiet
machine (94 s of 121 s when the machine was busy), 8.0 GB.

Two things made the guard cheaper on УХ and are worth knowing when the cost is measured again: it takes the rows the stage
has read (`StateBase::Prefetched`, 195 s of reading a second time saved), and a base-free stage hashes the tree's files
beside the export (a scan after it cost 68 s, reading each file when the export produced it 177 s more).

## 5. Where it lives

`src/mssql/override_stage.rs` (the plan, the build, the `deleted` row, the `versions` entries, the constant's flag),
`src/mssql.rs` (`stage_source_objects`, `StageAdditions`, the rows in parts, the dates), `src/mssql/empty_stage.rs`
(`EmptyStageContext::for_objects`: a context that does not throw the process-wide switches of a base-free stage),
`src/module_blob.rs` (`patch_predefined_data_blob_from_xml`), `src/mssql_dump/offline_rows.rs` and
`src/mssql/stage_guard.rs` (the `deleted` names out of the guarded state), `src/sql/mod.rs` (a detached handle says
why it is detached, which is how a writer knows it works for a base-free object inside a patch stage).

## 6. Limits

- **A single file of an object that stays**, removed from the tree (a module, a picture, a help page): not carried.
  The guard sees the file the target still has and refuses ("удаление отдельных файлов объекта пока не
  переносится"). Listing the row in `deleted` needs to tell such a row from the content-free stub rows the storage
  keeps for an emptied part (`ConfigDumpInfo.xml` lists them, the XML has no file), which the row roles of
  `apply_check::roles` describe; not done.
- **Removed children** of an object (an attribute, a tabular section): the platform's import writes them in
  `deleted` as `<uuid>,1`. Ours does not, and neither the platform's apply (section 3) nor the drop-in S1 apply
  (`attrdel`, section 3.2) needs them: the S1 plan takes the removed attributes from the descriptors and checks only
  that the row names nothing else.
- **A form that binds a removed attribute** is refused (section 3.2); the message names the native writer's refusal
  (the binding) and the failed patch of the target's layout that follows it.
- **State the XML does not carry** stays only for rows the stage does not rebuild: a compiled descriptor takes the
  compiler's values for it, except the always-used flag of a constant, which is copied. On ERP УХ 9 of the 56 758
  descriptors differ from what the XML gives (six of them are those flags); an object of that kind that the tree
  changes loses the difference (`docs/import/patch-mode.md` 4.1). The flowchart of a business process is patched
  as before.
- **The comparison's blind spots** fail closed: a descriptor `check_tree_against_db` calls unchanged stays the
  target's, and the guard refuses the import if the tree says otherwise. Bodies are not compared by it; the
  predefined items are compared by the set of items in the stage itself, the exchange plan's content, the modules,
  forms, templates, rights and command interfaces are always compiled from the tree.
- **A partial import** (`--path-prefix`) builds and compiles inside the prefix, removes nothing and does not clear
  the pending online update. `--per-row` and an offline stage (`--script-only` with `IBCMD_RS_BASE_ROWS_DIR`) do
  not compare the tree with a database at all, so they carry nothing new.
- **Every row was staged** (9 521 rows for an unchanged tree on БСП, 116 717 on УХ) when this stage was written; #395
  stages only the rows that change (`docs/import/delta.md`).
- **Not run:** the whole matrix on platform 8.5 (dialect 2.21: a module comment, `add5` and `rem2` ran, section
  3.1); ERP УХ 8.5; ERP УХ with real changes (a second copy of the tree is not allowed in the lab; the run is of the
  unchanged tree, so the build path costs there are the setup of the base-free stage: about 64 s for the walk, the
  descriptor reads and the context).
- **Edits that contradict themselves** are not the stage's to repair. The native export of a tree where a role
  right was set to `false` drops it, and of a tree where a form's query names a predefined item the tree removed
  marks the data paths with `~`; both showed in the tree with all 21 edits (section 3).
