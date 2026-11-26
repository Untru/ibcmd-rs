# The import stages only the rows that change (#395)

Until #395 a patch stage - `ibcmd infobase config import` into a database that already holds the configuration -
compiled every object of the tree and staged all its rows (9 521 for the БСП, 116 717 for ERP УХ). Almost all of
them were the rows the target already held, written again by another deflate stream and, for forms and modules, by
another writer: about 3 000 of the БСП's rows had another text, though the exports were identical. The platform's own
partial import stages what changed and nothing else, and the rows that stay are the target's, to the byte. This stage
does the same.

## 1. How it decides

The rows the stage has read - the target's Config as the storage publishes it - are exported with the model, in
memory, exactly as the guard exports a staged state (`docs/import/guard.md`), with nothing staged, and every file of
the export is compared with the tree (`stage_guard::compare_tree_with_target`). A file the export reproduces is a file
the tree has not changed, and the rows it comes from stay the target's. The comparison is the guard's, by content: a
file that differs only in formatting counts as equal.

From the files that differ (`src/mssql/delta_stage.rs`):

1. **An object none of whose own files differ is not prepared at all.** An object's own files are its metadata file
   and the files under its folder, but not the folders of the objects it owns (a form and a template have their own
   metadata files). The nearest metadata file above a differing file owns it; the configuration owns the files of the
   root `Ext`. A БСП stage prepares 5 of its 4 700 objects for a change of five descriptors.
2. **Of an object that is prepared, a row is staged when its own source differs:** the descriptor row when the
   metadata file differs (or the override compiles it), a body row when a file of the `Ext` folder it comes from
   differs. A body row of a file that is in no `Ext` folder, or outside the tree, is staged.
3. **Rows an online update of the target replaces.** When the target carries a pending online update, the storage
   publishes `<uuid>_dynupdate_<generation>` rows in place of the plain rows, and the apply of a stage that names them in
   `deleted` (section 2 of `docs/import/override.md`) leaves the plain row. A row of such an object that the tree does
   not change is therefore staged with the bytes of the published row, as they are, which is the row the platform's own
   apply leaves after it promotes the update; when the published bytes are the plain row's, nothing is staged. The
   БСП clone of the lab has two such rows.
4. **The override's rows are staged** whatever the comparison says: the descriptors it compiles, the objects it builds,
   the `deleted` row, `versions`, `root` and `version`.

The guard then exports the state the stage leaves - the target's rows with these few in place - and compares it with
the tree, so a row left out that should have gone in refuses the import instead of being lost.

**When it does not decide.** A differing file that belongs to no object the stage prepares (a nested subsystem's
command interface, say) leaves every row staged, as before; the report says why (`all_rows_because`).
`IBCMD_RS_STAGE_ALL_ROWS=1` does the same on request.

A file the tree lacks and the target's export has (a removed module of an object that stays, the files of a removed
object) does not mark any object as changed: removals belong to the override's plan, and a file left in the state the
tree does not have is what the guard refuses, as before.

## 2. The report

`overrides` in the import's JSON report: `differing_files` (files of the tree the target's export does not reproduce),
`objects_left_out` (objects not prepared), `rows_left_out` (rows of the prepared objects that stay), `compare_seconds`
(the export and the comparison), `all_rows_because` (when every row was staged).

## 3. When the guard disagrees

A row the target keeps can make the export of the staged state differ from the tree where the platform's own import
does the same. The case in the lab: the tree of `rem2` removes a form, and the help pages of the catalog and of its
other forms still link to it by name (`Catalog.Заметки.Form.ВсеЗаметки/Help`). The target's help rows link the form by
id; once the form is gone the export writes the id, and the tree says the name. The platform's own partial import of the
same tree ends with the same five help files different from the tree.

The tree wins: the guard hands its differing files back with the refusal, the objects that own them are prepared and
staged whole from the tree (`Delta::widen`), and the guard runs a second time. What it still refuses, it refuses as
before, and the message is the second run's.

## 4. Acceptance

БСП 8.3.27, the lab's clone (a pending online update on it: six names). Each case is the platform's twin against ours:
the twin is a fresh clone, the platform's `import files --partial` of the edited files and the platform's apply; ours
is a fresh clone, `ibcmd-rs infobase config import` of the whole tree with the default flags, and the apply (the
drop-in `config apply --recovery-backup` where S1 serves the change, the platform's otherwise).
`scripts/import-lab/run_s1_case.ps1` (`-NativeTwin` reuses a twin) and `compare_config_content.py` (check 4, by class:
identical, same text, other text), `row_diff.py` for where two rows differ.

| case | rows staged (before) | our apply | check 4: rows of Config identical to the twin's, of 9 838 | export |
|---|---|---|---|---|
| `c1`, 6 strings widened in 5 objects | 11 (9 521) | 12 s (117-230 s) | 9 832; the other 6 are the 5 changed descriptors and `versions` | ours == the twin's == the tree, 12 197 of 12 197; the platform's apply on ours: nothing to apply |
| `d1`, 12 indexes switched in 6 objects | 12 (9 521) | 29 s (106 s) | 9 831; 6 descriptors and `versions` | the same |
| `attrdel`, an attribute removed | 7 (9 521) | 85 s (151 s) | 9 834; the descriptor, two module rows the platform rewrote, `versions` | the same |
| `rights`, one role's `Rights.xml` | 7 | 10 s | 9 836; the role's rights row and `versions` | ours == the twin's (both drop the object whose rights are all false), != the tree in that one file |
| `add5`, an attribute, a tabular section, a new catalog, a synonym, a new form | 14 (9 521) | the platform's | the platform's partial import of these files stages 9 588 rows (`Configuration.xml` is among them); not comparable | ours == the twin's == the tree, 12 201 of 12 201 |
| `rem2`, a form and a template removed | 24 (9 521) | the platform's | 9 813 identical, 5 same text, 15 other: the twin's own rewrites | tree == ours, 12 190 of 12 190 (the twin's help pages differ from the tree, section 3) |

What still differs from the twin is what the platform writes and this program does not: a changed descriptor in the
newest record format (`56` -> `57` in the row's head, one more field per attribute; ours keep the format of the rows around
them), the `versions` generations, module containers with a fresh time stamp, help pages normalized.

After the apply of `c1` the `configVersion` of `ConfigDumpInfo.xml` differs from the previous export in 7 objects of
9 835 (a full stage: 9 514, the count that opened #395); `d1` 8, `attrdel` 3, `rights` 3.

## 5. Cost

БСП, `c1` (the same machine, one after another): import 14-24 s against 28-35 s; the stage's steps are the tree's scan
1 s, the comparison of the tree with the target 1 s, the rows 0.4 s, the comparison with the target's export 3 s, the
guard 2-3 s, ConfigSave 1 s.

ERP УХ 8.3.27, the unchanged tree, a database-backed stage of a clone that is never applied, the machine busy:

| | before (the rows all staged) | after |
|---|---|---|
| wall | 1 028 s | 496 s |
| CPU | 1 351 s | 965 s |
| peak memory | 8.7 GB | 6.4 GB |
| rows staged | 116 717 | 6 |
| scan of the tree | - | 174 s |
| comparison of the tree with the target (rcheck) | 20 s | 47 s |
| base rows read | 16 s | 30 s |
| comparison with the target's export | - | 90 s |
| guard | 111 s | 142 s |
| rows into tempdb, into ConfigSave | 37 s, 294 s | 0.5 s, 0.4 s |

(the steps of the second column are on the same busy machine; the quiet-machine stage of checkpoint 3 took 725 s.)
The guard's export is now paid twice - the target's, then the staged state's - and the tree's scan, which was always
paid, is now the largest step.

## 6. Limits

- The two exports (the target's, then the staged state's) cost twice the export; the second is the guard's. On a small
  configuration this is seconds; see the cost table for ERP УХ.
- The rows the stage carries are compiled by this program and are not the platform's bytes: a changed descriptor is in
  the record format of the target's own rows (the platform's stage writes its newest format), and every row of the
  stage is deflated by another library. Rows the stage leaves alone are the target's own.
- A row an online update replaces and that is kept in parts (over 10 000 000 bytes) is staged as the stage prepared it.
