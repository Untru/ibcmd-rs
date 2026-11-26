# Many generations, and the main configuration: 29.09.2026 (#390)

Measured on localhost, platform 8.3.27.2214, with the drop-in
`ibcmd-rs infobase config export --dbms=MSSQLServer --db-server=localhost
--db-name=<db> --platform=8.3.27 <folder>` and the native
`ibcmd infobase config export` (`--user=Администратор`, its own `--data`
folder) of the same database. Trees compared by SHA-256 of every file
(`compare_trees.py` of the lab folder `F:\ibcmd\lab\04\export-dyngen\tools`).
Databases are only read, except the two clones named below.

## The database that failed

`ibcmd_rs_04_rcheck_bsp_a` (БСП 8.3.27 after 16 `--dynamic=force` applies of a
full stage, on top of an earlier generation):

| | |
|---|---|
| rows in `Config` | 137 727 |
| rows with the `_dynupdate_` infix | 127 885 |
| plain rows | 9 842 |
| generations in `DynamicallyUpdated` | 17 |
| rows in `ConfigSave` | 9 516 (a full stage, `versions` of 9 839 pairs) |

The export before this change: exit -1 after 12.1 s,

```
failed to read the rows of ibcmd_rs_04_rcheck_bsp_a.Config: ... executing  on line 1 (code: 8621, ...)
```

The expression that failed listed each alias as a `CASE` branch and twice in
an `IN` list (nearly 10 000 names each). The expression now built is 2 343
characters and does not depend on the number of aliases (a unit test builds it
for 120 000 aliases and gets the same text).

After this change the export ends with exit 0 and writes **12 198 files, all
byte-identical to the native export** (12 198 of 12 198, `ConfigDumpInfo.xml`
included).

## Why the native export differed from `Config` alone

The native export publishes the *main* configuration. `ConfigSave` holds the
stage a completed import left, and the native export writes it:

- `ConfigDumpInfo.xml` of the native export carries the stamps of
  `ConfigSave.versions` on all 9 835 of 9 835 entries, and of none of the 17
  `versions_dynupdate_*` records;
- an export of `Config` alone was 12 040 files identical, 156 different, 2
  native-only, 1 ours-only;
- on `ibcmd_rs_04_rcheck_nat_a2` (a native stage of an added catalog attribute,
  no dynamic generation at all) an export of `Config` alone differed in 2 files
  (`Catalogs/_ДемоПартнеры.xml`, which lacks the attribute, and
  `ConfigDumpInfo.xml`); with the main configuration it differs in 2 other
  files, listed under "What is left" below.

The stage's `versions` row is not shaped like a `Config` one: the header
declares 9 834 pairs for 9 836 and no service pairs (`root`, `version`,
`versions`) are inside. The lab databases a native import and apply left
behind carry the same shape in `Config` itself (`ddl_a2_nat`, `_fin`, `_own`:
9 834 for 9 836; `ddl_bsp8327_a`: 9 835 for 9 836), and the strict reader
refused every one of them ("declares 9834 pairs but contains 19674 fields").

## What the platform does with generations (native, own clones)

- `ibcmd_rs_04_exdg_b`, the БСП corpus with its generation `06cb0442-…` and a
  crafted second generation that carries `ab132638-….0` with the module text
  of another module: the native export writes that text. A crafted third
  generation with an edited descriptor of the same object: the native export
  writes the edited property. The newest generation of the marker wins, for a
  module row and for a descriptor row.
- `ibcmd_rs_04_exdg_a`, a copy of the database above with `ConfigSave`
  emptied (the native export then reads `Config` alone): it fails in 88 s with
  «Ошибка формата потока» while `Config` holds rows the newest `versions` does
  not list (6 names, 11 rows: the objects and the modules a later generation
  removed). With those 11 rows deleted it exports, and its `ConfigDumpInfo.xml`
  is the newest `versions` on 9 834 of 9 834 entries. The export then differs
  from ours in 17 files: 17 objects whose newest carrier is a generation the
  tool-made history flipped a flag in are written by the platform from an
  older variant of the row, and the same happens with the history cut back to
  16, 13 and 10 generations. That is not explained and the export does not
  imitate it; it does not matter for the target database, where the staged
  rows decide those objects.

## Regression checks

| check | result |
|---|---|
| БСП with an active generation from SQL, `ibcmd_rs_bsp_8327_native_20260919` against `..._bsp_recheck\native` | 12 198 of 12 198 identical |
| БСП 8.3.27 offline (`--rows-dir F:\ibcmd\lab\rawrows\bsp\Config`), old binary against new | 12 199 of 12 199 identical; new against the native reference tree 12 198 identical (the 12 199th file is `manifest.json`) |
| БСП 8.5 offline (`F:\ibcmd\lab\v85\rawrows\bsp\Config`), old against new | 12 338 of 12 338 identical; new against the native reference 12 337 identical (+ `manifest.json`) |
| ERP УХ 8.3.27 offline, the 234 rows around its 156 superseded aliases, old against new | 255 of 255 identical |
| УХ, the constants: the native tree names 126 distinct `Constant.<n>.StandardCommand.<x>` and all belong to constants with `UseStandardCommands=true` | the constant rule cannot move a УХ file |
| the drop-in export of `ibcmd_rs_04_rcheck_nat_a2` (a native stage) | 12 196 of 12 198 identical; the two others are listed under "What is left" |
| a clean native stage: `ibcmd_rs_04_exdg_c` (the БСП corpus, one module edited in the tree, imported by the native `config import` until `ConfigSave` held the 9 842 rows of a full stage), native export against ours | 12 196 of 12 198 identical; the edited module is among them, the two others are the same two as on `nat_a2` |

No full УХ tree was written (lab rule); the УХ tables have no `DynamicallyUpdated`
row and no staged rows, so `install_storage_overlay` returns their headers
unchanged and every statement is built on the bare table name.

## Time

The small БСП database with the active generation, three alternating pairs of
runs on a machine other tracks were also loading (wall time follows that load):

| | wall | process CPU | SQL statements | SQL CPU | logical reads |
|---|---|---|---|---|---|
| old, runs 1-3 | 48.6 / 27.6 / 19.2 s | 21.2 / 23.9 / 20.5 s | 18 | 1 406 / 952 / 859 ms | about 108 000 |
| new, runs 1-3 | 50.4 / 14.2 / 14.9 s | 22.5 / 18.8 / 19.6 s | 20 | 1 251 / 1 267 / 1 188 ms | about 186 000 |

The two extra statements read the `ConfigSave` headers and its `versions` row.
The client CPU is the same, the server CPU is 0.3 s more on a 15-25 s export,
and the wall time is not longer.

`ibcmd_rs_04_rcheck_bsp_a`: 20 statements, 16 of them the range slices of the
row fetch, 21.0 s of server CPU in total on a loaded machine (each slice reads
the narrow columns of the table once), 1.15 million logical reads; process CPU
19-22 s; 24 s wall on a quiet machine, 89 s on a loaded one. The old
expression does not run at all (12 s to the error).

## The two files a native stage still differed in (#411, 30.09.2026)

Both were found on native stages and both have a cause of their own; the fix
touches only rows of that shape.

- `Configuration.xml`: the platform prints field 26 of a staged `{68}`
  Configuration row as `CompatibilityMode` and field 43 as
  `ConfigurationExtensionCompatibilityMode`. The stage of the БСП demo holds 80324
  and 80327 there, the platform prints `Version8_3_24` and `Version8_3_27`; the model
  refused rows whose two fields differ and the legacy converter took the
  extension mode from field 26. The model now reads field 43 for it (only for
  the `{68}` shape on 8.3.27, the one shape the platform writes there). All four
  corpora have the same value in both fields (БСП 8.3.27: 80324/80324 in the
  short `{67}` shape, УХ 8.3.27 and 8.5: 80327/80327, БСП 8.5: 80501/80501 in
  `{76}`), so no corpus row can change.
- `ConfigDumpInfo.xml`: the platform lists in it the first `count` pairs of
  the `versions` row, the generation entry counting as the first, and the
  staged row states a count below the pairs it holds. What lies beyond it is
  left out, an object's children with it, while the files are still written:

| row | declared / pairs | omitted by the platform |
|---|---|---|
| the БСП stage of the ddl track (`nat_a2`) | 9 834 / 9 836 | `ffd14055-….0`, `….3` |
| a clean native stage, first recipe (`exdg_c`; 9 836 pairs, the count was not read before the database was dropped) | 9 835 / 9 836 | `ffd14055-….3` |
| a clean native stage, second recipe (`exdg_e`) | 9 833 / 9 836 | `ffd14055-…` with `….0`, `….3` and its attributes |
| the `Config` an apply of a native stage left (`import_bsp_nat2`) | 9 830 / 9 831 | one entry |

  The four corpora state their count exactly (БСП 8.3.27 9 839 / 9 839, УХ
  118 171 / 118 171, БСП 8.5 9 936 / 9 936, УХ 8.5 118 171 / 118 171), so
  every entry of them is listed as before. The inventory check, the decision
  what a stage publishes and the children of an object read all the pairs.

Results with the fixed build against the native export of the same database:

| database | before | after |
|---|---|---|
| `exdg_d`: the `nat_a2` fixture (`F:/ibcmd/lab/04/restructure-check/fixtures/nat_a2`) loaded into a restore of the corpus | 12 196 of 12 198 (`Configuration.xml`, `ConfigDumpInfo.xml`) | 12 198 of 12 198 |
| `exdg_e`: a clean stage, a module edited in the tree, native `config import` until `ConfigSave` held 9 842 rows | 12 196 of 12 198 (the same two files) | 12 198 of 12 198 |
| `ibcmd_rs_04_import_bsp_nat2`, the `Config` of another track after a native apply, no `ConfigSave` | 12 189 of 12 191 (the same two) | 12 191 of 12 191 |
| `ibcmd_rs_04_rcheck_bsp_a`, the database of #390 | 12 198 of 12 198 | 12 198 of 12 198 |
| `ibcmd_rs_bsp_8327_native_20260919`, from SQL, against the stored native tree | 12 198 of 12 198 | 12 198 of 12 198 |
| БСП 8.3.27 and 8.5 offline, old binary against the fixed one | | 12 199 and 12 338 files identical; both equal the native reference trees |

The first attempt at the fix listed only the counted entries and failed on
`exdg_e`: there the omitted object is a whole catalog and its attributes, known
from the descriptors, were left without an owner. The writer now reads every
entry and leaves the uncounted ones, and their children, out of the file.

ERP УХ 8.3.27 from its stored rows, whole (140 709 files, default path, heavy
lock, 30.09.2026 before this change): 140 709 of 140 709 identical to the
native reference; the fixed build cannot move a УХ file (count stated exactly,
fields 26 and 43 equal), and no second full УХ export was made.

## The port onto feat/0.4 (30.09.2026)

The eight commits were re-applied on `feat/0.4` at `0f6b6e1e` (the verified
master merge of PR #387, the import guard `export_staged_state`, the apply and
rcheck checkpoints). The reconciliation:

- `config_dump_info.rs`: the origin-aware parser of #387 (`ExtensionConfigInfo`,
  `service_names`, the string `version`, `without_unstored_nil_versions`) stays;
  the reader by pairs, the `counted` flag and the unlisted-name decision are
  added to it, and the inventory check keeps its origin argument.
- `offline_rows.rs`: import's in-memory rows (`from_memory`, `with_staged`)
  and its tests stay; the tests of the folder take the overlay are added.
- `mod.rs`: `dump_table_rows_streamed` has both parameters (import's sink and
  `main_configuration`); the staged-state export passes `false`, it reads
  memory and never a database's ConfigSave. The stage guard keeps its own
  semantics (a file the tree lacks is something the stage leaves in), so it does
  not drop the names a staged `versions` does not list, and the export of a
  database does.
- `root_export.rs`: the staged split of fields 26 and 43 is decoded only when 43
  is the greater, the rule `fold_split_compatibility` of the row comparison
  already uses.
- `main_configuration: false` at the one other literal (`mssql_apply.rs`).

Acceptance on the ported branch, against the native export of the same
database: `ibcmd_rs_04_rcheck_bsp_a` 12 198 of 12 198; the `nat_a2` fixture
loaded into a corpus restore 12 198 of 12 198; the БСП database with an active
generation from SQL 12 198 of 12 198; БСП 8.3.27 and 8.5 from rows equal the
native reference trees (12 198 and 12 337 files, plus `manifest.json`); ERP УХ
8.3.27 from rows, default path, 140 709 of 140 709 identical with
`ConfigDumpInfo.xml` byte for byte. `cargo test -p ibcmd-rs
--no-default-features`: 3 436 passed, 0 failed, 12 ignored in 46 binaries.

