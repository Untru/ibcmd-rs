# Import into an existing database: what patch mode loses (#388)

> Checkpoint 1 research. The guard is `docs/import/guard.md` (checkpoint 2); the stage that carries what patch mode
> lost, and writes `deleted`, the dates of 4026 and rows in parts, is `docs/import/override.md` (step 2).

Research for checkpoint 1 of track `import` (issue #388), 2026-09-29. Branch `feat/0.4-import`, kit
`scripts/import-lab/`, lab `F:\ibcmd\lab\04\import`. Corpus: БСП 8.3.27 (`bsp_native_20260923.bak`), platform
8.3.27.2214, the binary of release 0.3.0 (commit `f882224a`, `iter` profile).

## 1. Summary

- **Cause.** `ibcmd infobase config import` into a database that holds the tree's configuration stages in
  *patch mode*. For nearly every kind of object patch mode takes the descriptor row from the database and
  rewrites only its header (name, synonym, comment). Attributes, tabular sections, enum values, properties of
  the object and of its attributes, the content of a subsystem, the properties of `Configuration.xml` and the
  ChildObjects lists stay the database's, whatever the tree says. Nested subsystems are not staged at all.
- **Matrix** (section 6): 22 edits of the БСП tree, each staged in patch mode, in `--base-free` mode and (three
  of them) by the platform's own `config import`. Patch mode **silently loses 11** (exit 0; for ten of them no
  row differs from the stage of the unchanged tree, for the removed form only the help pages that linked to it
  change), **refuses 6** with `Config row not found: <uuid>` or a packer error, and carries 5 (synonym, role
  rights, command interface, module, an edited predefined item). `--base-free` reaches the right rows for 20
  and refuses 2 (references to a removed catalog). The native `config apply` of the base-free stage of all
  edits together, and the native export after it, give the tree back in 12 197 of 12 199 files (the two others:
  `ConfigDumpInfo.xml` and a role file, section 8.6); the same for the patch stage of the four edits patch mode
  carries (12 197 of 12 198, `ConfigDumpInfo.xml` aside), which restructures nothing but the one object it edits.
- **But base-free is not what the platform does.** Native import is a *merge* onto the database's
  configuration: for the unchanged tree 7 305 of its 9 839 staged rows are byte-identical to Config (forms and a
  flowchart counter the XML does not carry included) and it writes a `deleted` row for what left the
  configuration. A base-free stage rebuilds everything from the XML and forgets that state. Measured: the native
  apply of the base-free stage also restructured the route points of `БизнесПроцесс.Задание`, a table none of
  the edits touched - the only row of that process that differs from Config is the flowchart, whose closing
  counter `23` replaces the stored `37` (that this triggered the restructuring is a hypothesis); the three rows of
  a removed form stay in Config; ERP УХ has 6 constants flagged always-used that a base-free stage writes as 0.
  So base-free must not become the default for a database that holds the configuration.
- **The three "missing multi-part rows"** are not missing content: the native import cuts any row larger than
  10 000 000 bytes into parts (`5189beb9…0` is 22.5 MB, 3 parts; `7e3283df…0` is 11 MB, 2 parts) and we write one
  row each. The fourth row native has and we do not is `deleted` (section 7).
- **Proposal** (section 9): keep the database's rows for everything the tree did not change and build the rest
  from the tree - *(b)* patch mode that compiles a descriptor from the tree whenever the tree's XML differs from
  what the database exports, stages nested objects, writes `deleted` and drops removed names from `versions` -
  behind a **guard** that exports the staged state with our model and refuses on any difference from the tree.
  The guard alone, on the patch stage, already turns every silent loss of the matrix into a refusal (all 11
  refused, the 5 carried edits pass; section 6.5) and can ship first.
- **Two more facts from the experiments.** The БСП clone carries the platform's dynamic-update rows
  (`<id>_dynupdate_<id>`, `DynamicallyUpdated`, ...); the import stages around them, the native import lists them
  in `deleted`, the native apply removes them (section 6.3). And the platform's own `config import` refuses the
  18-edit tree that our `--base-free` stage turned into a correct configuration (`unknown predefined item`,
  leaving 6-9 rows in ConfigSave): the cause is an interaction of eight of the edits, no single edit (section 6.3).
- **Other defects of the staged rows** that track trace reported (dates of 2026 against the platform's 4026,
  399 content differences for a no-op tree, form rows with extra default properties): none drops a change; the
  dates are worth a one-line fix, the rest is cosmetic until the delta stage stops restaging unchanged rows
  (section 8.1).

## 2. How the import chooses its mode

`src/infobase.rs::import_config`, stage mode `Auto` (the drop-in default): `import_target_state` counts the
target's `Config` rows and whether one is named by the tree's configuration uuid. If it is, the target "holds
the configuration" and the stage runs in **patch** mode (`mssql::stage_source_objects`); otherwise (an empty
infobase) in **base-free** mode (`mssql::empty_stage::stage_source_objects_base_free`). The drop-in flag
`--base-free` forces the second.

## 3. What each mode stages

### 3.1 Patch mode (`src/mssql.rs`)

Input: `Configuration.xml`, the root-level XML of 45 collections (`is_stage_root_metadata_collection`),
`Forms/*.xml` and `Templates/*.xml` under an object, and the root common modules. **Not staged at all:**
subsystems nested in subsystems (317 Config rows of БСП with their command interfaces and helps), recalculations
and three content-free body rows that only `ConfigDumpInfo.xml` lists (321 rows in all).

| row | patch mode |
|---|---|
| descriptor `<uuid>` | the **database's row**; `pack_simple_metadata_blob_from_xml_with_source` -> `patch_simple_metadata_header_text` rewrites name, synonym, comment. More is patched only for Constant (type, standard commands), DefinedType (types), CommonCommand, CommandGroup, CommonModule (flags) and the header of the Configuration row |
| body rows `<uuid>.N` (forms, rights, command interface, help, modules, templates, exchange plan content, additional indexes) | compiled **from the tree** (`prepare_metadata_body_rows`), by the writers base-free uses; a form or a rights file a writer refuses falls back to patching the database's row |
| predefined data `.1c`/`.7`, business-process flowchart, graphical-schema template | the database's row patched from the XML (`pack_*_from_xml(&base_body, &xml)`); an item the base lacks makes the packer fail |
| `versions` | the database's row; every staged name gets a new generation uuid, new names are appended (`patch_versions_blob_bytes_allowing_additions`), **no name is ever removed** |
| `root`, `version` | copied from Config by the apply SQL (`build_bulk_stage_apply_sql`) |
| `Attributes` of a staged row | the Config row's (0 in both corpora) |

A new object has no Config row: `fetch_config_blob` fails with `Config row not found: <uuid>` and the import ends
with exit -1 before ConfigSave is touched.

### 3.2 Base-free (`src/mssql/empty_stage.rs`)

Every descriptor XML of the tree (`descriptor_xmls_of`, nested objects included) -> `compile_descriptor` (the
metadata model, no database) and its bodies with `BASE_FREE_STAGE` set (a writer that would read a base row fails
naming it), the content-free rows from `ConfigDumpInfo.xml` (`stub_rows`), and `root`, `version` and a fresh
`versions` (a new uuid for every name). One row that cannot be produced fails the import (exit -1, ConfigSave
untouched). The whole set replaces ConfigSave in one transaction (`DELETE FROM ConfigSave` + insert, every row
`PartNo 0`, `Attributes 0`).

### 3.3 Native `config import` (the reference)

Stages every row of the tree (9 839 names, 9 842 part rows for the БСП tree) with a fresh generation uuid for
every name in `versions` (none equal to the database's), the parts cut at 10 000 000 bytes, and a row `deleted`:
`<count>,"<name>",0,...`, the Config names the new configuration does not contain.

## 4. What patch mode reads that a rebuild from the XML cannot know

1. The **descriptor row** of every object, all of it but the header.
2. `versions` (321 names of БСП keep their generation), `root`, `version`.
3. The **always-used flag of constants** (slot 11 of the constant's `{16,...}` record; no exported property
   carries it). Patch mode keeps the rows; it also reads the flags (`target_always_used_constants`,
   `IBCMD_RS_ALWAYS_USED_CONSTANTS`) to write a constants set's `<UseAlways>` as the delta the platform keeps.
   Base-free writes slot 11 as `0` (`metadata_model/simple.rs::constant`) and compiles constants sets against
   clear flags. БСП: 0 flagged of 245. **ERP УХ: 6 of 1 435** (`ВсегдаКонтролироватьБалансРучныхОпераций`,
   `ДополнительныеЯзыкиВыводаОтчета`, `НастройкиКолонтитуловПоУмолчанию`, `ПутьККаталогуИмпорта`,
   `СрокОплатыПокупателей`, `СрокОплатыПоставщикам`) in the natively loaded database and in the patched one;
   the base-free load into an empty УХ database has 0.
4. The **counters and history that only the stored rows hold** (section 4.1).
5. Predefined items, flowchart and graphical-schema bodies (patched into the database's row), and the base
   fallbacks of forms, rights and command interfaces.
6. `Attributes` of the Config rows (0 everywhere).
7. Stored object ids: the tree carries every uuid (objects, children, generated types, predefined items);
   nothing else was found.

### 4.1 State the XML does not carry

- The closing counter of a flowchart or a graphical template counts deleted items too (`bodies_flowchart.rs`:
  the item count is right for 18 of 20 flowcharts and 29 of 64 УХ templates only). БСП `BusinessProcess.Задание`:
  stored `},37}`, compiled `},23}`; the XML holds items up to id 36.
- `empty-database-load-20260928.md`: 12 of 145 content-free ERP УХ rows differ from the stored ones "in history
  the source does not keep" (the next row index of an emptied tree, the column lengths of a catalog before its
  properties changed, the old dimensions and column titles of a register).
- The always-used flags above.
- The offline audit of the ERP УХ 8.3.27 rows (`F:\ibcmd\lab\model\integration\ve_uha8327_i03a\empty.json`, base-free
  rows against the stored ones): of 56 758 descriptor rows 56 749 are identical and 9 differ (the 6 constants' flag,
  a reference uuid in 2 documents, one flag of a role), so descriptor state is a small residue; but among the bodies
  that carry data the predefined items of 72 catalogs (`.1c`) and 8 charts of characteristic types (`.7`), 17
  exchange plan contents and 2 business-process flowcharts differ from the stored rows, besides 8 233 form, 14 124
  template and 285 role bodies (invisible in the XML export). What the platform makes of the predefined items,
  contents and route points of a stage that rewrites them is not measured.

The native import reproduces them: its `Задание.7` row is byte-identical to Config's (`},37}`), and so are 386 of
the 389 form bodies (`Form.0`, `CommonForm.0`) our writers change (1 101 of its 1 109 form bodies are). It starts
from the database's configuration and applies the XML.

## 5. Method

Everything on БСП 8.3.27.2214, one staging clone `ibcmd_rs_04_import_bsp_s1` that is **never applied**, so every
stage starts from the same Config (9 847 rows, 0 in ConfigSave).

- Base tree: the corpus's native export (12 198 files, byte-identical to what the clone exports).
- `scripts/import-lab/edits.py`: 22 named edits of the tree, each on its own object, applied to a working copy
  and reset afterwards. They keep BOM, CRLF and the bare LF inside text values.
- `scripts/import-lab/run_matrix.ps1`: for a change and a mode (patch = the drop-in default on a database that
  holds the configuration; bf = `--base-free`; native = the platform's `config import`) stage the edited tree
  into the clone, then compare ConfigSave **row by row with the stage of the unchanged tree in the same mode**
  (`rowdiff.py`: parts joined and inflated; classes identical bytes / same text / layout only / v8 container with
  another header time / different / in one stage only). A row that differs from that baseline is a row the change
  reached; a change that reaches no row is lost. The baselines differ from Config in a stable way (5.1), which is
  why they, not Config, are the reference.
- `run_survival.ps1`: stage, native `config apply --force --dynamic=disable`, native `config export`,
  `ibcmd-rs source-diff` against the tree. `ve_tree.sh`: our own row -> model export without a database.
  `guard_probe.ps1`: the guard of section 9.3 as an offline experiment (`overlay_rows.py` builds ConfigSave over
  Config).

### 5.1 The stage of the unchanged tree against Config

| unchanged БСП tree | patch | base-free | native |
|---|---|---|---|
| names / part rows staged | 9 517 / 9 517 | 9 838 / 9 838 | 9 839 / 9 842 |
| identical bytes | 357 | 377 | 7 305 |
| same text, other compressor output | 6 100 | 6 400 | 0 |
| layout only (line breaks) | 722 | 722 | 0 |
| v8 container, other header time | 1 939 | 1 939 | 1 939 |
| really different | 399 | 400 | 594 |
| in Config, not staged | 327 | 6 | 6 |
| staged, not in Config | 0 | 0 | 1 (`deleted`) |

- The 399/400 different rows are body rows our writers compile another way than the platform stored them (372
  `Form.0`, 17 `CommonForm.0`, 6 `ExchangePlan.1`, one module, one template, `...f`, and in base-free the
  flowchart `Задание.7`): internal numbering the XML export does not show. Both modes stage all of them for
  the unchanged tree. Native rewrites 594 descriptors in another record shape (`{14,25,...}` for the database's
  `{13,24,...}`) and MXL templates (a flag), which we do not.
- Patch mode does not stage 327 Config rows: 225 nested-subsystem descriptors, their 67 command interfaces and 25
  helps, four content-free rows, and the platform's 6 dynamic-update rows (`DynamicallyUpdated`,
  `<uuid>_dynupdate_<uuid>[.0]`, `versions_dynupdate_<uuid>`). Base-free does not write those six either; the
  native import lists them in `deleted`.
- `versions`: native stages new generation uuids for **all** 9 835 names (0 equal to Config's), base-free too
  (0 of 9 838), patch keeps 321.

## 6. Results

### 6.1 What each change stages

`LOST` = exit 0 and no row differs from the stage of the unchanged tree. `REFUSED` = exit -1, ConfigSave
untouched. The cells list the rows that differ from the baseline of the same mode, `versions` aside; `+` is a row
the baseline lacks, `-` a row it has and the stage lacks. `formdel`: the rows that differ in both modes are help
pages linking to the removed form, and the form's three rows are missing from the stage; patch mode's owner
descriptor still lists the form, so its removal is lost too.

| change | what | patch | base-free | native |
|---|---|---|---|---|
| `attr` | String(50) attribute ДемоНовыйРеквизит added to Catalog._ДемоПартнеры | **LOST** | 1 row(s): Catalog._ДемоПартнеры | 2 row(s): Catalog._ДемоПартнеры; Configuration.БиблиотекаСтандартныхПодсис... |
| `ts` | tabular section ДемоНоваяТЧ (clone of the smallest one) on Catalog._ДемоКонтрагенты | **LOST** | 1 row(s): Catalog._ДемоКонтрагенты | 2 row(s): Catalog._ДемоКонтрагенты; Configuration.БиблиотекаСтандартныхПодсис... |
| `newcat` | Catalog ДемоНовыйСправочник (clone of Удалить_ДемоОбщиеСведения) + Configuration.xml entry | REFUSED: Config row not found: `<uuid>` | 2 row(s): Configuration.БиблиотекаСтандартныхПодсис...; +Catalog.ДемоНовыйСправочник | 2 row(s): Configuration.БиблиотекаСтандартныхПодсис...; +Catalog.ДемоНовыйСправочник |
| `syn` | synonym of Document._ДемоОприходованиеТоваров changed | 1 row(s): Document._ДемоОприходованиеТоваров | 1 row(s): Document._ДемоОприходованиеТоваров | - |
| `prop` | QuickChoice flipped on Catalog._ДемоГруппыДоступаПартнеров | **LOST** | 1 row(s): Catalog._ДемоГруппыДоступаПартнеров | - |
| `attrprop` | FillChecking of an existing attribute of Catalog._ДемоФизическиеЛица set to ShowError | **LOST** | 1 row(s): Catalog._ДемоФизическиеЛица | - |
| `attrdel` | the first attribute removed from Catalog._ДемоГруппыДоступаНоменклатуры | **LOST** | 1 row(s): Catalog._ДемоГруппыДоступаНоменклатуры | - |
| `newform` | form ДемоНоваяФорма (clone of ФормаЭлемента) on Catalog._ДемоГруппыДоступаНоменклатуры | REFUSED: Config row not found: `<uuid>` | 3 row(s): Catalog._ДемоГруппыДоступаНоменклатуры; +Form.ДемоНоваяФорма (+1) | - |
| `newtpl` | text template ДемоНовыйМакет (clone of ДатыПасха) on DataProcessor.ЗаполнениеКалендарныхГрафиков | REFUSED: Config row not found: `<uuid>` | 3 row(s): DataProcessor.ЗаполнениеКалендарныхГрафиков; +Template.ДемоНовыйМакет (+1) | - |
| `predef` | predefined item ДемоНовыйЭлемент added to Catalog.СостоянияОригиналовПервичныхДокументов | REFUSED: failed to pack PredefinedData `<path>` PredefinedData XML contains items | 1 row(s): Catalog.СостоянияОригиналовПервичныхДокум... | - |
| `enumval` | enum value ДемоНовоеЗначение added to Enum.ТипыХраненияФайлов | **LOST** | 1 row(s): Enum.ТипыХраненияФайлов | - |
| `rights` | View of Catalog.ВнешниеПользователи.Command.ВнешнийДоступ switched off in Role.ДобавлениеИзменениеВнешнихПользователей | 1 row(s): Role.ДобавлениеИзменениеВнешнихПользовате... | 1 row(s): Role.ДобавлениеИзменениеВнешнихПользовате... | - |
| `subsys` | Catalog.Заметки added to the content of Subsystem._ДемоНачальнаяСтраница | **LOST** | 1 row(s): Subsystem._ДемоНачальнаяСтраница | - |
| `nestsub` | Catalog.Заметки added to the content of the nested Subsystem.КонтрольВеденияУчета | **LOST** | 1 row(s): Subsystem.КонтрольВеденияУчета | - |
| `ci` | visibility of a command flipped in Subsystem._ДемоАнкетирование/Ext/CommandInterface.xml | 1 row(s): Subsystem._ДемоАнкетирование.1 | 1 row(s): Subsystem._ДемоАнкетирование.1 | - |
| `module` | a comment line appended to CommonModule._ДемоЗаметки/Ext/Module.bsl | 1 row(s): CommonModule._ДемоЗаметки.0 | 1 row(s): CommonModule._ДемоЗаметки.0 | - |
| `confver` | Configuration `<Version>` bumped 3.1.11.466 -> 3.1.11.467 | **LOST** | 1 row(s): Configuration.БиблиотекаСтандартныхПодсис... | - |
| `catdel` | Catalog.Удалить_ДемоОбщиеСведения removed (file and Configuration.xml entry) | REFUSED: failed to pack ExchangePlan Content `<path>` failed to resolve ExchangeP | REFUSED: 9 rows cannot be built (dangling references) | - |
| `formdel` | form ВсеЗаметки removed from Catalog.Заметки (files and the `<Form>` entry) | 8 row(s): Form.ЗаметкиПоПредмету.1; Form.МоиЗаметки.1 (+6) | 9 row(s): Form.ЗаметкиПоПредмету.1; Form.МоиЗаметки.1 (+7) | - |
| `catfile` | Catalogs/Удалить_ДемоОбщиеСведения.xml removed but Configuration.xml still lists it | REFUSED: failed to pack ExchangePlan Content `<path>` failed to resolve ExchangeP | REFUSED: 10 rows cannot be built (dangling references) | - |
| `predefdel` | predefined item ФормаНапечатана removed from Catalog.СостоянияОригиналовПервичныхДокументов | **LOST** | 1 row(s): Catalog.СостоянияОригиналовПервичныхДокум... | - |
| `predefedit` | Description of the predefined item Россия changed in Catalog.СтраныМира | 1 row(s): Catalog.СтраныМира.1c | 1 row(s): Catalog.СтраныМира.1c | - |

Median stage time on this machine (shared with six other tracks, CPU at 100 %): patch 20 s (n = 17, 7-170 s),
base-free 9 s (n = 21, 4-95 s), native import ~97 s (n = 2; 29 s on a quiet machine).

### 6.2 By kind of change

- Carried by patch: synonym and comment (header), role rights, the command interface of a top-level object,
  modules, help pages, forms and templates that already exist, an edited predefined item.
- **Lost without a word:** an added attribute, tabular section or enum value; a removed attribute; any property of
  an object or of an attribute; the content of a subsystem (top-level or nested); the version and every other
  property of `Configuration.xml`; a removed predefined item; a removed form (the descriptor still lists it).
- Refused by patch with text a user cannot act on (`Config row not found: <uuid>`, `failed to pack PredefinedData
  ... items missing in base blob`): a new catalog, form, template or predefined item; a removed catalog that
  other objects still reference (base-free refuses it as well: `unknown type cfg:CatalogRef...`, `unresolved
  reference`, listing every row that cannot be built).

### 6.3 Apply and export

Two stages went through the platform's own `config apply --force --dynamic=disable` and `config export`, and
`ibcmd-rs source-diff` compared the export with the tree (`run_survival.ps1`, one clone each, never shared):

- `bf1`: the tree `combo2` with 18 edits at once (attr, ts, newcat, syn, prop, attrprop, attrdel, newform, newtpl,
  predef, enumval, rights, subsys, nestsub, ci, module, confver, formdel) staged with `--base-free`;
- `ap2`: the tree `combo_patch` with the four edits patch mode carries (syn, ci, module, predefedit), staged in
  patch mode. The other edits are lost or refused by patch mode; a stage that lacks them proves nothing about
  the apply.

| | base-free, 18 edits (`bf1`) | patch, 4 carried edits (`ap2`) |
|---|---|---|
| stage | 9 840 rows, 22 s | 9 517 rows, 40 s |
| native apply | exit 0, 533 s (a second apply ran beside it) | exit 0, 679 s (machine at 100 %) |
| Config rows after the apply | 9 843 = 9 840 staged + the 3 rows of the removed form | 9 838 = 9 844 - the 6 dynamic-update rows (below) |
| objects the platform reports as changed | the five edited ones, and **`БизнесПроцесс.Задание` (no edit touches it)**: its route points table is restructured | `Справочник.СтраныМира` only (the edited predefined item) |
| native export against the tree | 12 197 of 12 199 files identical; different: `ConfigDumpInfo.xml` and `Roles/ДобавлениеИзменениеВнешнихПользователей/Ext/Rights.xml` (8.6) | 12 197 of 12 198 identical; different: `ConfigDumpInfo.xml` only |

Every edit of both columns survived the native apply and export: the synonym, the command interface, the
module text, the predefined item for patch mode; all 18 for base-free. What the apply does not do is remove what
the stage does not mention: the three rows of the removed form are still in Config (`8a7546f4-...`, `.0`, `.1`).

The structure of the base-free clone against the pristine one (`equiv_cmp.py`, the table text of the ddl kit): the
new tables `_Reference11034` (the catalog) and `_Reference15_VT11038` (the tabular section), the column
`_Fld11042` (the attribute), `_Fld4382RRef` gone (the removed attribute), the platform's own `_DbCopies*` tables
and `_StorageVariant`, and rebuilt primary keys on `_Enum588` (the enum edit), `_ConfigChngR` (always) and
**`_BPrPoints10`, the route points of `БизнесПроцесс.Задание`**, which no edit touches. The patch stage restructures
nothing that its edits did not touch.

**Dynamic-update rows.** The БСП backup was taken from a database that had received an online (dynamic) update: Config
holds `DynamicallyUpdated`, `versions_dynupdate_<id>` and, for two objects (the common module `_ДемоЗаметки` and a
form), `<id>_dynupdate_<id>` and `<id>_dynupdate_<id>.0`. For the module the `.0` row holds the text of the
database before the online update and the `_dynupdate_` row the text after it; the platform's export, and ours,
write the latter (the tree's `Module.bsl` ends with a comment the `.0` row lacks). Patch mode stages `<id>.0` and
leaves those six rows alone. The native apply removes all six and the staged text wins (`ap2`: the edited module
is in the export). Native import lists the six names in `deleted`. So a model of "what the apply produces" - the
guard's - has to drop the dynamic-update rows (`overlay_rows.py` does), and a `deleted` row written by our import
should list them as native does.

**The platform's own import refuses `combo2`.** `config import` of the same 18-edit tree ends with exit -1 and
`Ссылка на неизвестный предопределенный элемент - ChartOfCharacteristicTypes.ОбъектыАдресацииЗадач.ВсеОбъектыАдресации`
(twice: the two `FillValue`s of `Catalogs/РолиИсполнителей.xml`, which no edit touches; the same in three runs).
Bisect (`native_subset.ps1`: the native import of a subset of the edits into the `nat` clone, one hold of the native
lock each, `logs/bisect_*.log`): each of the eight edits of the group syn, prop, attrprop, attrdel, newform,
newtpl, predef, enumval imports alone (as do attr, ts and newcat); so do its two halves of four, four of its
six-edit subsets, its two seven-edit subsets tried (without `syn`, without `enumval`) and another group of eight
edits (attr, rights, subsys, nestsub, ci, module, confver, formdel). The eight together are refused, and so is the
whole tree. It is an interaction of those eight edits - not one edit, and not a limit on their number. The
platform's rule was not pursued further; the only consequence here is that a stage of ours can be a valid
configuration (the apply and the export accept it) that the platform's importer would not build. After a failed
native run ConfigSave holds 6-9 leftover rows (`<id>.1c.new` predefined-data rows): the platform's import is not
atomic either, which is one more reason for the check "nothing written" to sit in our stage.

### 6.4 Offline round trip of a tree with 18 changes

`ve_tree.sh`: the rows `--base-free` would stage (`audit-empty-stage --rows-out`), exported again from those rows
alone (`mssql-dump-config --rows-dir`), against the tree itself. The tree carries all the edits of the matrix but
the two removals of a referenced catalog: 9 840 rows, 0 failures, **12 198 of 12 199 files byte-identical to the
tree**; the other file is `ConfigDumpInfo.xml`, which holds the fresh generation ids. This is the property a
guard needs.

### 6.5 The guard, tried on the patch stage

`guard_probe.ps1`: stage the edited tree in patch mode into the staging clone, build the state an apply would
produce - ConfigSave over Config, without the names of the `deleted` row and without the dynamic-update rows
(`overlay_rows.py`) - export it offline with the model (`mssql-dump-config --rows-dir`, no database) and diff the
export with the edited tree (`source-diff`; `ConfigDumpInfo.xml`, which holds the fresh generation ids, is left
out). Sixteen of the 22 edits were probed; the other six (`newcat`, `newform`, `newtpl`, `predef`, `catdel`,
`catfile`) are refused by patch mode before any row is written, so they never reach a guard.

| change | patch mode (matrix) | files where the export of the staged state differs from the tree | guard |
|---|---|---|---|
| `attr` | LOST | different `Catalogs/_ДемоПартнеры.xml` | refuses |
| `ts` | LOST | different `Catalogs/_ДемоКонтрагенты.xml` | refuses |
| `syn` | carried | none | passes |
| `prop` | LOST | different `Catalogs/_ДемоГруппыДоступаПартнеров.xml` | refuses |
| `attrprop` | LOST | different `Catalogs/_ДемоФизическиеЛица.xml` | refuses |
| `attrdel` | LOST | different `Catalogs/_ДемоГруппыДоступаНоменклатуры.xml` | refuses |
| `enumval` | LOST | different `Enums/ТипыХраненияФайлов.xml` | refuses |
| `rights` | carried | none | passes |
| `subsys` | LOST | different `Subsystems/_ДемоНачальнаяСтраница.xml` | refuses |
| `nestsub` | LOST | different `Subsystems/СтандартныеПодсистемы/Subsystems/КонтрольВеденияУчета.xml` | refuses |
| `ci` | carried | none | passes |
| `module` | carried | none | passes |
| `confver` | LOST | different `Configuration.xml` | refuses |
| `formdel` | LOST | different `Catalogs/Заметки.xml`; right_only `Catalogs/Заметки/Forms/ВсеЗаметки.xml`; right_only `Catalogs/Заметки/Forms/ВсеЗаметки/Ext/Form.xml` (+3) | refuses |
| `predefdel` | LOST | different `Catalogs/СостоянияОригиналовПервичныхДокументов/Ext/Predefined.xml` | refuses |
| `predefedit` | carried | none | passes |

- **All eleven silent losses are refused** by the guard, and each names the file that carries the change - one
  file of 12 198 for an attribute, a tabular section, an enum value, a property, a subsystem, the version, a
  removed predefined item; six for the removed form (the owner's descriptor and the form's files).
- **All five carried edits pass with 0 differences**, so for a patch stage the model reproduces the other 12 197
  files of the БСП tree byte for byte: no false positive on БСП. The same holds for a base-free stage
  (section 6.4).
- `module` first failed (`Module.bsl` of `_ДемоЗаметки`): the export took the text of the database's
  `_dynupdate_` row (section 6.3) instead of the staged one, while the platform's apply drops that row and uses
  the staged one (`ap2`). With the dynamic-update rows left out of the modelled state the probe passes. The state
  a guard exports must be what the *apply* produces, not a plain overlay.
- Time of one export of the state (БСП, machine at 100 %): name and reference indexes 12.7 s, reading the 9 839
  rows 20 s, processing 13.9 s wall; 79 s more went into writing 12 199 files to disk, which an in-memory
  guard does not do. ERP УХ was not measured (phase 2).

## 7. The three multi-part rows, and a fourth row

- The database keeps `5189beb9-b939-4f97-aca0-c2f3492ac8b9.0` in 3 parts (10 000 000 + 10 000 000 + 2 571 377
  bytes) and `7e3283df-05a6-4dc5-a786-056c71bdbf6f.0` in 2 parts (10 000 000 + 974 171); every part row carries
  the whole `DataSize`. The native import writes any row larger than **10 000 000 bytes** in parts of that size;
  the bulk stage of both our modes writes one row, `PartNo 0`. So native stages 9 842 part rows for 9 839 names,
  we 9 838 for 9 838, and "three rows are missing" counts parts, not content.
- The platform reads the single-part rows: native apply and export after a stage of ours returned the whole tree
  (12 197 of 12 198 files, `empty-database-load-20260928.md`), and afterwards the database holds one part of
  22 647 015 and 11 010 923 bytes (`ibcmd_rs_04_ddl_bsp8327_m`, `ibcmd_rs_04_trace_noop`). It is a difference in
  shape, not a defect; splitting at 10 000 000 bytes like the native import is cheap and makes ConfigSave the
  same shape as native's.
- The fourth row is **`deleted`** (`<count>,"<name>",0,...`; six names in the БСП clone, all of them the
  dynamic-update rows of section 6.3): the Config names the new configuration does not contain. Neither of our
  modes writes it, so a removed form leaves its three rows in Config (section 6.3). The native apply removes
  the dynamic-update rows even when the stage does not list them (`ap2`); what `deleted` does for a removed
  object was not observed here (the native import of an 18-edit tree, which includes a removed form, is refused,
  section 6.3) and must be checked in phase 2 with the single edit `formdel`.

## 8. Other findings

1. **Patch mode never stages nested subsystems** (321 rows of БСП): a change to a nested subsystem's content or
   command interface is lost, a new nested subsystem cannot be staged.
2. **`versions` in patch mode never loses a name**, so a removed object stays listed.
3. The refusals of both modes are English technical text; the acceptance asks for a clear Russian message.
4. README and `docs/COMMANDS.md` say a load into an existing database is verified by "native export equals the
   source XML"; that holds only for trees whose descriptors did not change.
5. The base-free rights writer has a read-back check ("the written row reads back into a different
   Rights.xml"): it refused an inconsistent role edit (View kept, Read removed) that patch mode wrote through.
6. A role edit that sets a right to `false` is written back by our export as `false` and dropped by the platform's
   (an object with no true right disappears from `Rights.xml`): the tree's non-canonical form does not survive the
   native export. Canonical trees never hold a `false`.
7. **Neither mode looks at the database's dynamic-update rows** (section 6.3). A comparison of the tree with the
   database's descriptors and modules (step 2's detection; `apply_check::dbtree` of track rcheck) has to take the
   effective text - the `_dynupdate_` row over `<id>.0` - or an object updated online counts as changed for the
   wrong reason; and a stage cannot be judged by the export unless the state model drops those rows, as the
   apply does.

### 8.1 Findings of track trace for the import, and whether they matter

Track trace (`docs/apply/native-apply-trace.md`) reported five things about the rows our import stages. None of them
drops a change, so none is part of the bug of this document; two are worth the fix in step 2.

1. **Dates.** Our rows carry `Creation`/`Modified` of 2026; the platform writes 4026 (`_YearOffset`, +2000 years).
   Confirmed on the clones: pristine Config, 0 rows of 2026 and all of 4026; the base-free clone after the native
   apply, 9 840 of 9 843 rows dated 2026-09-29, and the platform's own writes (`Params`) 4026. A platform that
   subtracts the offset reads our dates as the year 26. Nothing observed depends on them (the apply, the export and
   the restructuring are right), so it is **cosmetic for correctness** as far as measured, but a mixed table is
   unclean and any tool that orders rows by date would trip on it; the fix is one expression
   (`DATEADD(year, 2000, SYSUTCDATETIME())`): step 2.
2. **9 517 staged rows for a no-op tree, 399 of them different in content from Config** - the numbers of section
   5.1 (357 identical, 6 100 compressed differently, 722 line breaks, 1 939 container time, 399 content: 372
   `Form.0`, 17 `CommonForm.0`, 6 `ExchangePlan.1`, 4 others), measured independently. The 9 rows native stages for a
   one-attribute change are its `files --partial` import of one file; a full native import stages 9 842 rows for the
   same change (section 6.1). Cosmetic for correctness in БСП (forms, exchange plan content); it costs stage and
   apply time, and in ERP УХ the same class holds predefined items, flowcharts and exchange plan contents (section
   4.1). The cure is not to stage what the tree did not change: step 3.
3. **Form rows with other properties than the platform stored** (`15 {"U"}`, `19 {"S" ""}`, counters `11 -> 13`).
   Seen here as well, in the same rows as (2): explicit default values the platform omits, and item numbers.
   The export shows no difference and the apply accepts them; whether a client session treats them differently was
   not tested here. A defect of the form writer, separate from #388, **cosmetic until a session test says
   otherwise**, and staged by patch mode and base-free alike.
4. `import files --partial Configuration.xml` re-imports the whole tree (9 615 rows, 362 s): not a mode of the
   drop-in import (`import files` is refused), so it does not matter here.
5. Adding an object adds an entry to the rights rows of all 517 roles: **not seen with a full import.** The native
   full import of the new catalog staged its row, `Configuration` and `versions` only, and after our base-free stage
   and the native apply exactly one role row differs from the pristine database (the one edited). It belongs to the
   partial import (the roles are not reloaded, so the platform registers the object in them). In a full import a role
   without an entry for the new object has no rights on it, as in the tree.

## 9. Proposal

### 9.1 Options

- **(a) the drop-in import always stages base-free.** Correct for every change of the matrix, one code path, and
  faster on БСП. Rejected as the default for a database that holds the configuration: it forgets state the XML
  does not carry (section 4.1) - a spurious restructuring of the route points of `Задание` in the measured
  apply, six cleared flags on УХ, no `deleted` - and a tree with one row no writer can build is refused where
  patch mode used to pass it. It stays the mode for an empty infobase.
- **(b) patch mode with descriptors built from the tree where the tree differs from the database.** The row of
  an object the tree did not change stays the database's; the rest is built from the tree by the base-free
  writers. **Recommended.** Needs the database's descriptors as the XML an export would write, to compare with
  the tree (a descriptor-only export; `apply_check::dbtree` of track rcheck does exactly this comparison).
- **(c) a delta stage** (only the changed rows). The apply of an ERP УХ stage (118 025 rows) takes hours because
  it moves every staged row, so this is the real speed-up for large configurations. It is (b) plus a body-level
  comparison; worth doing after (b).

### 9.2 Recommendation: (b), in three steps

1. **The guard on the current patch stage** (small, first): before anything is written, export ConfigSave over
   Config with the model and compare with the tree; any difference refuses the import with a Russian message
   and exit -1, and nothing is written. From that day the import never silently drops a change.
2. **Override from the tree** in patch mode: a descriptor, or a body that carries data (predefined items,
   flowchart, exchange plan content), whose XML differs from the database's export is built from the tree
   (`compile_descriptor` and the base-free body writers); an object that is not in the database is built the
   same way (fixes the six refusals); nested subsystems and recalculations are staged; the always-used
   flags and the other state of the target are read from the target for the objects that are built; the
   `deleted` row lists the rows of objects that left the tree and the target's dynamic-update rows, and
   `versions` drops the names of objects that left; rows over 10 000 000 bytes are cut into parts. What cannot be built refuses the import, in Russian, with the kind, the
   path and the reason.
3. **Delta stage** (option c) as a follow-up.

### 9.3 The guard

After the rows are built and **before** the transaction that replaces ConfigSave:

1. Give the model export the state an apply would produce - the staged rows over the database's Config, minus
   the names of `deleted` and minus the dynamic-update rows (the apply removes them; without this a module
   the target updated online is compared with its old text, section 6.5) - in memory (`mssql_dump::offline_rows` reads `<name>__part<N>.bin` folders today; an in-memory row
   set is a small addition, `overlay_rows.py` is its prototype).
2. Write no files: hand each exported file to a comparer in `output_writer` instead of the disk, against the tree
   file at the same relative path - bytes for text and binary files, a leaf-level comparison (the one
   `source-diff-explain` uses) for XML, so a hand-formatted tree is not refused for its whitespace.
3. Any difference refuses the import: exit -1, a Russian message with the first N files and the first differing
   leaf. Nothing was written, so ConfigSave keeps its prior state (a check after the transaction could only be
   worse). `ConfigDumpInfo.xml` is left out; it holds the fresh generation ids. The message, for the attribute
   of the matrix:
   ```
   [ERROR] Загрузка отменена: состояние базы после загрузки не совпадает с деревом (файлов: 1). В ConfigSave ничего не записано.
   [ERROR]   Catalogs/_ДемоПартнеры.xml: в дереве есть элемент Catalog/ChildObjects/Attribute «ДемоНовыйРеквизит», в собранной конфигурации его нет
   [ERROR] Импорт конфигурации из XML завершен с ошибкой
   ```

Cost: one export of the state; the tempdb load of the rows can run beside it and the move into ConfigSave waits
for the verdict. Risk: it refuses a tree our export does not write back byte- or leaf-for-leaf - which is the
point (the acceptance), and both corpora round-trip.

### 9.4 Risks

- **Detection rests on our export.** An object whose descriptor our export does not write as the tree has it (a
  hand-formatted file, a construct the model does not know) counts as changed and is built from the tree; the
  leaf-level comparison keeps formatting from doing that, and the guard checks the result either way.
- **A built object forgets what the XML does not carry** (its counters, its flags unless read from the target). The
  objects the tree did not change are untouched; and of all 56 758 descriptor rows of ERP УХ only 9 differ from what
  the XML gives, so the residue is small.
- **Deletions come from the lists, not from missing files**: an object is deleted when the database's
  Configuration/owner descriptor lists it and the tree's does not. A tree that lists an object and lacks its file
  is refused (dangling reference), so an incomplete tree deletes nothing.
- **8.5**: the `deleted` list and the `<configuration>.<uuid>` cache row (`missing_platform` in the audit) must be
  checked on the 8.5 corpora; nothing here was run on 8.5.
- Effort (an estimate), by step: the guard 2-3 days, the descriptor override with nested objects, `deleted`, parts
  and messages 4-6 days, the delta stage after that.

### 9.5 Acceptance and phase 2

Code: `src/infobase.rs`, `src/mssql.rs` (patch descriptor override, nested objects, `deleted`, parts),
`src/mssql/empty_stage.rs` (the constants' flags for a built object), `src/mssql_dump/offline_rows.rs` and
`output_writer.rs` (in-memory rows, comparing sink), tests. БСП 8.3.27: attribute, tabular section, new catalog,
synonym, new form loaded by our `ibcmd infobase config import` into the existing database, native apply, native
export equals the tree; a tree the writers cannot build gives exit -1 and an unchanged ConfigSave. ERP УХ: the
stage timing of patch against the change (offline with the row cache, in the heavy lock) and the flag check. Open
questions: whether the coordinator wants step 1 alone as the first commit; how the descriptor comparison is shared
with track rcheck.

## 10. Evidence

Repo: `scripts/import-lab/` (`edits.py`, `rowdiff.py`, `run_matrix.ps1`, `run_survival.ps1`, `native.ps1`,
`native_complete.py`, `native_subset.ps1`, `summarize.py`, `equiv_cmp.py`, `ve_tree.sh`, `guard_probe.ps1`,
`guard_table.py`, `overlay_rows.py`).
Lab `F:\ibcmd\lab\04\import`: `out/matrix/v0`, `v1` (`<change>.<mode>.json` and `.log`), `out/rows_<mode>_pristine`
(the three baselines), `out/ve_combo2_v2` (offline round trip), `out/guard` (guard probe), `out/survival-*.json`
(`bf1`, `ap2`) and `out/export/*.diff.json`, `logs/` (`survival_*.log`, `guard_patch*.log`, `bisect_*.log`,
`native-import-*.err.txt`), `STATUS.md`. Databases: `ibcmd_rs_04_import_bsp_s1` (staging only), `_ap` and `_bf`
(applied), `_nat` (native imports only).
