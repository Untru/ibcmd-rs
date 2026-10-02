# Extension export parity (`mssql-dump-extension`)

Status of issue #348 (milestone 0.5): the tree that `mssql-dump-extension`
writes for a configuration extension equals the tree that the native
`ibcmd infobase config export --extension=<name>` writes for the same database.

## Result

Measured 2026-09-29 on a clone of the БСП 8.3.27 database (four extensions),
native reference exports against ours with `ibcmd-rs source-diff`
(`ConfigDumpInfo.xml` included, byte for byte):

| extension | files | differences before | differences now |
|---|---|---|---|
| `_ДемоПустоеРасширение` | 3 | 2 different, `ConfigDumpInfo.xml` missing | 0 |
| `_ДемоРасширение` | 185 | 47 different, 28 missing | 0 |
| `ServiceDesk` | 465 | 46 different, 6 missing | 0 |
| `VAExtension` | 84 | 16 different, 2 missing | 0 |

"Before" is the export of commit 08de112f (the state the issue was opened
against). The ordinary export is unchanged: the offline export of the main
configuration from saved rows (`mssql-dump-config --rows-dir`) still equals the
native one in all 12198 files of the БСП 8.3.27 corpus and in all 140709 files
of the ERP УХ 8.3.27 corpus. The default export writes descriptors through the
metadata model (issue #389 later corrected three slot pairs there, see "Compile
side" below; the offline empty-database cycles show the same differences before
and after); the extension export shares the *legacy converters* with
`--legacy-export`, so that path was measured too: the БСП 8.3.27 corpus is identical in all 12198 files and the ERP УХ 8.3.27 corpus in all 140709 files (the only extra entry is the dump's own manifest.json).

Measured 2026-09-29 on a clone of the БСП 8.5 database (three extensions,
platform 8.5.1.1150, XML 2.21, `--platform 8.5.1`), same method:

| extension | files | differences now |
|---|---|---|
| `_ДемоПустоеРасширение` | 3 | 0 |
| `_ДемоРасширение` | 187 | 0 |
| `ServiceDesk` | 633 | 0 |

The offline export of the main configuration is unchanged by the 8.5 work on
both paths: the БСП 8.3.27 corpus is identical in all 12198 files and the БСП 8.5
corpus in all 12337 files, through the default (model) export and through
`--legacy-export`.

The export reports `native_xml_parity: true` when no storage row is opaque or
failed. That is a claim about the readers and not a proof: they read fail-closed
(a row whose shape they cannot state exactly is reported, never written
approximately), and the proof is the table above.

## How the export works

An extension stores its objects in `ConfigCAS` in the ordinary row layout of the
main configuration. What differs is written into the rows and not stated by the
XML the platform prints, so the export works in two steps that keep every
ordinary converter blind to extensions (`src/mssql_dump/extension/`):

1. **Normalize.** The md header of an adopted (borrowed) object carries an
   adoption tail after the comment. `normalize_descriptor` rewrites it to the
   ordinary tail and remembers it; the ordinary family converters then print the
   object as if it were an ordinary one.
2. **Project.** `project_object_xml` reduces that print to what the platform
   writes for an adopted object: the properties the header lists (plus name,
   comment and `ExtendedConfigurationObject`), `xr:PropertyState` blocks,
   `MultiState` type lists.

**Which image.** The native export writes the *staged* state of an extension
when it has one: the БСП 8.5 clone keeps two changed modules of ServiceDesk in
`ConfigCASSave`, and the native tree holds them. A staged namespace
(`<extension _IDRRef>__<logical name>`) is the changed rows plus a new
`configinfo` (the manifest); the rows it leaves out are the ones `ConfigCAS`
already holds under the same digest. `mssql-dump-extension --image auto` (the
default) roots the graph at the staged `configinfo` when the namespace exists,
supplies the staged rows and fetches the rest from `ConfigCAS`; `--image active`
reads the generation the registry names, `--image staged` fails when nothing is
staged. The report says which image was exported (`image`, `image_cas_root`).

The root object is rendered by `extension/root.rs`, `ConfigDumpInfo.xml` from the
digests of the CAS manifest, and forms get a post-pass over their XML
(`extension/form.rs`, `adjust_form_files`). Every relaxation of an ordinary
reader is gated by `extension::active()`, so an export of an ordinary
configuration takes exactly the old paths.

Adopted header tail, member by member:

```text
belonging(1) N (property guid, state) x N  extended-object uuid  M (guid, state, value) x M
```

An own object of an extension carries `0,0,<nil uuid>,0` there. State 2 is a
property that only records its value, state 3 one the extension overrides; the
`N` pairs are exactly the properties the platform prints. The `M` triples carry
the values an extension adds to a type list.

## Differences found and fixed, by kind of file

### `ConfigDumpInfo.xml`

Not written at all before (the extension CAS has no `root`/`version`/`versions`
rows). `configVersion` of an entry is the SHA-1 of the packed CAS row of that
object; in the main configuration it is the 16-byte identity and `00000000`.

### `Configuration.xml` (root object)

`ObjectBelonging`, `ConfigurationExtensionPurpose`,
`KeepMappingToExtendedConfigurationObjectsByIDs`, `NamePrefix`,
`ConfigurationExtensionCompatibilityMode` (tuple members 44, 41, 42, 43), the
run-mode group (`DefaultRunMode`, `UsePurposes`, `InterfaceCompatibilityMode`),
`DefaultRoles`, `Vendor`, `Version`, the information addresses, the
`PropertyStates` of the root header, `ChildObjects`. Rows of the root's own
modules and command interface are routed by the root's header uuid.

### Adopted objects

Catalog, document, register, common form, common module, role, subsystem, style
item and the other families print only the listed properties. Values of a
`Type`-like list the extension adds to (`DefinedType`, `FilterCriterion`,
`CommonCommand.CommandParameterType`, attributes) are written as `MultiState`
with `xr:ExtendedProperty`.

### Own objects of an extension: shapes the ordinary readers had not met

Each entry is a value that never occurs in the ordinary corpora; the reader was
relaxed for an extension export only unless noted.

* Catalog attributes, `DefinedType`, `FilterCriterion` with an empty
  `{"Pattern"}` (the type stays that of the extended configuration).
* Types of the extended configuration in a pattern (`{"#",<type id>}` with no
  object of the extension behind it): printed as `<v8:TypeId>`, and a fill value
  of such an attribute is `xsi:nil`.
* Charts of accounts, calculation types and characteristic types with no
  standard attributes and no standard tabular sections (`{0}`), and no
  `StandardTabularSections`/`StandardAttributes` element.
* Chart of accounts: no `ExtDimensionTypes` (nil uuid), `AutoOrderByCode` rides
  member 24 (member 27 is the constant), empty `CodeMask` is `<CodeMask/>`, an
  empty `<ChildObjects/>` is written (charts only).
* Chart of calculation types: `DependenceOnCalculationTypes` rides member 27
  (member 35 is the constant), empty `<BaseCalculationTypes/>`.
* Accounting register: `DataLockControlMode` rides header+6 and `FullTextSearch`
  header+7 (the reader had them the other way round; the ordinary corpora write
  `0` in both, so the mistake could not show; the ordinary export is unchanged),
  no standard attributes; calculation register `BasePeriod` rides member 18.
* Role rights: the "defaulted flag is false" value `2`, and for an adopted
  object every nested `View`/`Edit` right is printed.
* Pictures: `TransparentPixel` is written only for a pixel `>= 0` (`-1,-1` is
  "none"); `MinValue`/`MaxValue` of a number, date or boolean keep their XML type
  (`xs:decimal`, `xs:dateTime`, `xs:boolean`; the ordinary readers knew only
  strings and failed the whole attribute).
* Style: standard style items `-47` (`ImportantColor`) and the order
  `ActivityColor`, `NavigationColor`, `AuxiliaryNavigationColor`,
  `ImportantColor`.
* Module text: a character outside the basic plane is stored as two code unit
  escapes, each behind a quote (`"\d83d"\dcce`); the container reader took the
  first quote for the end of the string and lost the module. Both the scan and
  the decoding of `module_blob` now follow the rule `mssql_dump` already used.

### Compile side: the same pairs (issue #389)

The extension's own register and charts were the first objects on record whose
values tell two neighbouring slots apart, so they exposed the same mistakes in
the base-free compile. It had two independent copies of them, both fixed:

* the metadata model (`src/metadata_model`: the default main export and the
  empty-database compile) had three pairs swapped: chart of accounts
  `AutoOrderByCode`/`EditType` (slots 24/27), chart of calculation types
  `DependenceOnCalculationTypes`/`EditType` (27/35) and accounting register
  `DataLockControlMode`/`FullTextSearch` (21/22). Each pair holds one value on
  every ordinary object, so compile and export agreed with the stored rows of
  all four corpora and the main export stayed exact;
* the typed compiler (`src/compiler/families/business_object/register_native.rs`,
  used by `cf bootstrap` and the extension overlay) had been written from an
  earlier reading of the rows: seven properties of the chart of accounts, eight
  of the chart of calculation types, six of the chart of characteristic types
  and three of the accounting register sat in another property's slot, a
  period adjustment did not write the 31-slot record, and the recalculation's
  lock mode stood where the collection count belongs. The calculation register
  (`ActionPeriod` 17, `BasePeriod` 18) was right.

The slots, as the samples of the four corpora (БСП 8.3.27, БСП 8.5, ERP УХ
8.3.27, the extension) separate them:

| object (slots) | property: slot, the values that fix it |
|---|---|
| accounting register (30; 31 as `{22,22,...}` with a period adjustment), 10 samples | `DataLockControlMode` 21: `1` on the extension's register, `0` on nine; `FullTextSearch` 22: `0` on all ten (by elimination); `Correspondence` 20; `EnableTotalsSplitting` 23; `PeriodAdjustmentLength` last: `1` on the two ERP УХ registers |
| chart of accounts (57), 6 samples | `AutoOrderByCode` 24: `0` on the extension's chart, `1` on five; `CheckUnique` 34: `0` on two ERP УХ charts; `CodeSeries` 35: `1` on the two БСП charts; `DataLockControlMode` 36: `0` on two ERP УХ charts; `EditType` 27, `ChoiceMode` 31, `CreateOnInput` 49 hold one value on all six |
| chart of calculation types (63), 5 samples | `DependenceOnCalculationTypes` 27: `0` on the extension's chart; `ActionPeriodUse` 29; `IncludeHelpInContents` 37: `1` on the two ERP УХ charts; `DataLockControlMode` 41: `0` on the two ERP УХ charts; `CodeAllowedLength` 53: `0` on ERP УХ `Начисления`; `EditType` 35, `ChoiceMode` 38, `QuickChoice` 39, `CreateOnInput` 55 hold one value on all five |
| chart of characteristic types (59), 36 samples | `EditType` 25: `0`/`1`/`2`; `ChoiceMode` 31; `CheckUnique` 34; `DataLockControlMode` 36; `CodeAllowedLength` 49; `CreateOnInput` 51: `1`/`2`; `PredefinedDataUpdate` 53: `0`/`1`/`2`; `DefaultPresentation` 24 and `CodeSeries` 35 hold one value on all 36 |
| calculation register (33), 5 samples | `ActionPeriod` 17: `0` on ERP УХ `Удержания` and the extension's; `BasePeriod` 18: `0` on the extension's; `DataLockControlMode` 26 |

A property that holds one value everywhere is placed by the model's layout, which
reproduces the stored row of every object on record; only a value the samples
do not hold could show a mistake there. The recalculation is the same: both on
record (БСП) are Managed, and its lock mode is the last slot of the owner record
with the collection count after it.

The offline empty-database cycles (`scripts/empty-load/ve.sh`: every row a load
into an empty infobase would write, exported from those rows alone and diffed
against the native export) do not move: БСП 8.3.27 12197 files unchanged, БСП 8.5
12336 and ERP УХ 8.3.27 140708, each with `ConfigDumpInfo.xml` as the one
different file, as before. In all three the registers and charts compile to
their stored rows exactly: 2, 2 and 5 accounting registers (the two ERP УХ
registers with a period adjustment among them), 1, 1 and 3 charts of accounts,
1, 1 and 2 charts of calculation types, 5, 5 and 25 charts of characteristic
types, 1, 1 and 2 calculation registers.

`src/metadata_model/slot_evidence_tests.rs` compiles the extension's register
and four charts (`tests/fixtures/native-evidence/extension-register-slots/`,
the native XML next to the stored rows) to their stored rows byte for byte,
reads every scalar property back, and edits one property of each at a time,
requiring that exactly the slot above moves; the typed compiler has one such
test per family (`business_object.rs`). The tests fail on the previous layouts.

### Forms

* **Root namespaces.** An extension in a compatibility mode older than the one
  that introduced the data-composition schema namespace declares no `dcssch` on
  the root of `Ext/Form.xml`.
* **`BaseForm`.** A form the extension adopted carries the extended
  configuration's form after its own tree (container section 6 is `1`, section 7
  is a complete form record). `form_extension::form_adoption` (upstream PR 387,
  moved into the form writer by #414) writes it as a document of its own, which
  the form writer closes its document with as `<BaseForm version="...">`, one
  level deeper, for every source of forms: the `.cfe` container and the SQL
  export share it. A form whose base form does not read is not emitted at all.
* **`callType`.** The event block of a form body stores every handler of every
  event (`{N,(event,"first handler")xN,1,0,(event,code,n,("handler",code)x(n-1))xN}`,
  code `0` Before, `1` After, `2` Override), and the platform writes one
  `<Event>` per handler with its call type, the base form's events included
  (upstream fixtures `adopted/form_events`, `form_events_shared`). The command
  handlers (`<Action>`) of an adopted form are written `Before`: all six on
  record say so, and where a command record keeps an interceptor code is not on
  record. An adopted form that carries no base form record (the common form
  `СвязанныеДокументы` of the БСП 8.3.27 ServiceDesk) gets `Before` on all its
  events and commands from the export's own pass (`extension::form`), for lack
  of a second sample.
* **`Usual` group behavior.** An explicit `<Behavior>Usual</Behavior>` follows
  the compatibility mode of the configuration the extension EXTENDS, not the
  extension's own: `VAExtension` (mode 8.3.14) extends a configuration in 8.3.27
  and the native export writes it. The extended configuration's mode is read
  with its references, before the rows are converted (`ExtensionContext`).
* **Forms saved by an older platform.** Items the platform completes on load are
  completed in the XML, in document order, with the ids taken from the largest
  item id plus one: a table without an extended tooltip gets one, and each of its
  search-string, view-status and search-control additions gets a context menu and
  an extended tooltip; a command bar without a tooltip gets one; a table over a
  dynamic list gets the list settings, any other table that can filter rows
  (not a value list) gets `<RowFilter xsi:nil="true"/>`.
* **Planner field.** `DisplayImportance` at the common slot, `EnableDrag` in the
  option slot behind `EnableStartDrag`.
* **Record columns.** A calculation register's record set names its columns by
  the register's own standard-attribute markers (`-3` `LineNumber`, `-4`
  `CalculationType`, `-11` `ReversingEntry`, ...).

### Platform 8.5.1

The 8.5 rows and XML differ from 8.3.27 in these places (all measured on the
three БСП 8.5 extensions, `src/mssql_dump/extension/`):

* **Root.** An extension the 8.5 platform converted stores the `{76,` tuple: 77
  members, 0-60 as in `{68,`, 61-68 the enumerations added by 8.5 (with the
  `Caption` and `ShortCaption` in 64 and 65), 69-76 the auxiliary forms. The
  packed platform version `80501` prints as `Version8_5_1`, and the interface
  compatibility mode `3` with `6` in member 62 as `Version8_5EnableTaxi`. XML 2.21
  writes `Caption`/`ShortCaption` after `Version` for every extension (an
  extension that was not converted has none: both empty).
* **Adopted root blocks.** An extension that changes the home page work area, the
  logo or the splash screen lists three ids (`d98a8e01`, `740eb5f6`, `3035a9db`)
  in state 3 and the platform prints `HomePageWorkArea`, `Logo` and `Splash` as
  `Extended` after the module states. Which id is which is not on record (one
  extension); they are a group and appear whole or not at all. The logo is an
  extension picture and goes out as such.
* **Home page work area.** Template `0` is `OneColumn` and prints one `<Column>`;
  the stored row still closes with an empty right column (the reader and writer
  are upstream PR 387's, measured on its `home_page/one_column*` fixtures and on
  the ServiceDesk extension).
* **Styles.** The body is revision `2`, colours `{4,...}`, fonts `{8,...}`, and it
  ends with one record `{1,{0,<colour>}}` that the platform prints as the last
  item `FirstBrand`.
* **Types.** `{"R"}` (and `{"R",<length>,<flag>}`) is the binary data type,
  `xs:base64Binary` with `BinaryDataQualifiers`. HTTP method code `10` is `PATCH`.
* **Commands.** An adopted command of a register lists its parameter types
  (`7d14f63a`) in state 2, printed empty.
* **Characteristics.** An item may name no source: both sources are the nil uuid
  and the fields sentinels; the platform prints `from=""`.
* **Forms.** The base form of an adopted form is a record of its own in the 8.5
  layout: its items number the same ids as the form's, so the form's
  down-conversion leaves it as it is (`adopted_base_record_slot`) and
  `form_extension::form_adoption` converts and completes it with its own facts
  for the form writer to write after the form's tree. A planner field
  keeps its 8.3.27 property bag. The appended importance member of a button is
  `0` (`Main`) exactly for the default button (88 of 88 on ServiceDesk), so the
  writer adds `DefaultButton` there; the 8.3.27 slot that also says so is set on
  six of the eight default buttons only.

`tests/fixtures/native-evidence/extension-empty/` holds the two stored rows of
`_ДемоПустоеРасширение` (identical on 8.3.27 and 8.5) and the tree each platform
wrote for them; `the_empty_extension_exports_to_the_native_tree_of_both_platforms`
exports both dialects from those rows and compares the three files byte for
byte.

### References to the extended configuration

The extension's own type lists know nothing of the objects it does not adopt, so
the platform writes their types as ids. A *value* is named through the whole
configuration: the empty reference in a fill value prints as
`Catalog.Пользователи.EmptyRef`. The export reads the metadata rows of the same
database once, when a value first needs them (`extension::base_index_provider`),
and resolves type ids and object ids through them. If the rows cannot be read,
the reference stays an id.

## Load and activation round trip (bounded module change)

`mssql-load-extension` compiles the source tree of an extension into an
overlay on the extension's active rows: every metadata row is kept, the bodies
the tree carries replace theirs. Its metadata compile is the strict
`cf bootstrap` one, which refuses what a native export writes for an extension
(the document of an adopted object lists only the properties the extension
records; the root carries `DefaultRoles`), so none of the four БСП 8.3.27
extensions loaded from its own native tree (`Missing("Synonym")` on the
adopted language, `business object property inventory is not exact` on an
adopted common module, `DefaultRoles ... has no proven base-free projection`).
A bounded load (`--path-prefix`, which `mssql-apply-source-change
--extension` always passes) whose selection holds nothing but module bodies now
skips that compile: `compile_extension_module_overlay` reads the family and
uuid off each owner's document, consumes it unread, and compiles the `.bsl`
files (`compiler::bootstrap`). A body must replace an existing row (adding one
to an object that has none is a structural change and is refused); a form,
picture or template in the selection goes to the whole-tree route below.

Measured 2026-09-29 on БСП 8.3.27 clones, extension `_ДемоРасширение`, module
of the adopted common module `ОбщегоНазначенияПереопределяемый` (one comment
line added):

| step | result |
|---|---|
| `mssql-load-extension --path-prefix CommonModules/...` | 1 compiled row, 170 staged rows, root `28c928ac...` |
| our export of the staged image | equal to the changed tree in 184 of 185 files; `ConfigDumpInfo.xml` differs in the module's `configVersion` only |
| native `config export --extension` of the staged state | 185 of 185 files equal to ours, `ConfigDumpInfo.xml` included |
| native `config apply --extension` (39 s) | "Создано поколение расширения конфигурации: 28c928ac..." = our root |
| native and our export after the apply | 185 of 185 equal; our active image root `28c928ac...` |
| second change, staged on two clones | root `19eb4a58...` on both |
| clone X: native apply (51 s) / clone Y: `mssql-activate-staged-extension --mode online` (6 s) | both end at `19eb4a58...`; native and our export of both 185 of 185 equal |

The two clones differ only in what their history makes different: Y keeps two
more immutable `ConfigCAS` rows (the first change's `configinfo` and module),
and bytes 32-34 of the registry blob carry a row version (our publisher writes
the version the row had before its update; the platform wrote `18 5f` on both
clones, which is the version of the untouched row of the same database; the
versions are the databases' own counters, so no two histories agree on them).

Two findings on the publisher: it refuses an extension whose registry blob has
byte 30 set (`_ДемоПустоеРасширение` and `_ДемоРасширение` are stored that way in
the corpus, until the platform has applied them once; `ServiceDesk` and
`VAExtension` have it clear), and `--mode exclusive` refuses while the cluster
holds sessions of the database (a clone registered with `register-ib.ps1`
does), so `--mode online` is the one that runs on a registered clone.

## Whole-tree load

The strict compile above refuses what a native export writes for an extension,
so no whole native tree of the four БСП 8.3.27 extensions loaded, and a form, a
picture or a template failed even in a bounded selection. `mssql-load-extension`
now goes the other way round (`mssql_extension_tree_load`), for the whole tree
and for a `--path-prefix` selection that holds more than module bodies:

1. the extension's active image is exported (the export equals the platform's
   `config export --extension`, see the sections above) and the tree is compared
   with it file by file. `ConfigDumpInfo.xml` is derived and left out; a module
   is compared by its text, so a byte order mark or LF line ends saved by an
   editor are no change;
2. the objects that own a changed file are compiled by the staging compiler of
   `cf load` (`load::compiled::compile_edit`, the one that loads a `.cfe`),
   offline against the extension's own rows; only the rows of the changed files
   replace the active ones, every other row stays the active image's byte for
   byte (a recompile could lose what the compiler cannot read);
3. the proposed image is exported again and must equal the tree, file for file
   (`ConfigDumpInfo.xml` aside). A difference refuses the load by file name and
   nothing is staged, so an edit is either carried exactly or refused, never
   approximated. A dry run does all three.

Activation publishes rows and restructures nothing, so a change that would
change what a database table holds is refused before any compile, by file and
with the reason: the descriptor of an object of a family that may own a table
(catalogs, documents, registers, ...), the root descriptor unless only the
`<ChildObjects>` lines of table-less families changed, an asset of the root, a
body file added or removed (except the module of a form), an object added or
removed outside the table-less families (common forms, modules, pictures and
templates, roles). Bodies (module, form and its module, template, picture,
rights, help) and the descriptors of forms, templates and table-less families
change in place; an object of a table-less family may come or go (the root's
child list follows the tree's). A tree equal to the export of the active image
is refused as "no changes" (`--all-extensions` lists such extensions as
`unchanged_extensions`). `--path-prefix` takes only the files under the given
paths and leaves the rest of the proposal the active image's; a new object
needs `--path-prefix Configuration.xml` beside its own path, since the root's
child list is a file too. A selection of module bodies alone is still overlaid
without any comparison (`selection: "bounded"`, what `mssql-apply-source-change
--extension` uses), and gives the same rows as the tree route (checked: root
`bf0f9c26...` on both). One process compiles one extension's tree offline, so
several changed extensions are loaded one by one with `--extension` (module
edits and unchanged extensions do not count; the refusal says so).

Side files: the export report names the file an entry is written as, not the
files unpacked beside it (`Ext/Help/ru.html`, `Ext/Picture/Picture.png`); such a
file belongs to the entry of its `Ext/Help.xml` / `Ext/Picture.xml`.

One writer rule came out of it: an ExtPicture with `LoadTransparent` true and no
`TransparentPixel` is stored `{1,0,-1,-1}` (6 of 6 pictures of the extensions;
the main-configuration corpora never have it, and the compiler refused it).

Only platform 8.3.27.2214 writes extensions (8.5 is declared unsupported for
extension writes).

### Measured against the platform, 2026-09-30

Each case is a copy of the native tree of one extension of the БСП 8.3.27 clone
with one edit (`F:\ibcmd\lab\05\ext\tools\tl_edit.py`), loaded twice from the
corpus backup: by us (`mssql-load-extension`, whole tree) on one clone and by
the platform (`ibcmd infobase config import --extension=<name> <tree>`) on a
twin (`tl_twin.ps1 -Apply`). The staged state and then the state after the
platform's `config apply --extension` are exported by the platform and by us;
all four exports of both clones equal the edited tree, file for file
(`ConfigDumpInfo.xml` aside), and the generation the platform creates from the
rows we staged has exactly the root we proposed (the CAS root is the SHA-1 of
`configinfo`, which lists the SHA-1 of every row, so the platform kept every row
byte for byte). The twin's rows differ from ours in bytes, as its import
recompiles every object (element timestamps, the `Navigator` records of forms,
the layout of a few rows), and export identically.

| case | extension | edit | changed files | staged rows |
|---|---|---|---|---|
| m1 | `_ДемоРасширение` | a line in the module of an adopted common module | 1 | 170 |
| m2, m3 | `_ДемоРасширение` | manager module, and manager + record set module, of an adopted register | 1, 2 | 170 |
| fm1 | `VAExtension` | the module of an own form (inside the form's body row) | 1 | 70 |
| fx1 | `VAExtension` | `Form.xml` of an own form: a command title | 1 | 70 |
| fx2 | `ServiceDesk` | `Form.xml` of an own form of an own catalog | 1 | 431 |
| fx3 | `VAExtension` | `Form.xml` and its module together | 2 | 70 |
| fa1 | `_ДемоРасширение` | a module written for an adopted form that had none | 1 | 170 |
| fa2 | `_ДемоРасширение` | `Form.xml` of an adopted form (165 KB, `BaseForm`): the title | 1 | 170 |
| tc1 | `ServiceDesk` | data composition schema of an own report: a parameter title | 1 | 431 |
| tm1 | `_ДемоРасширение` | spreadsheet template of an own document: a text | 1 | 170 |
| p1 | `ServiceDesk` | an SVG picture recoloured | 1 | 431 |
| p2 | `_ДемоРасширение` | a PNG picture replaced | 1 | 170 |
| r1 | `_ДемоРасширение` | rights of an own role: one object taken out | 1 | 170 |
| h1 | `_ДемоРасширение` | help page of an own catalog: a paragraph | 1 | 170 |
| d1 | `ServiceDesk` | the descriptor (synonym) of an own common picture | 1 | 431 |
| all1 | `_ДемоРасширение` | module, spreadsheet template, rights, help page and picture at once | 5 | 170 |
| n1 | `ServiceDesk` | a new common picture (descriptor, `Picture.xml`, SVG) + the root's list | 4 | 433 |
| n2 | `VAExtension` | a new common module + the root's list | 3 | 72 |
| n3, n4 | `ServiceDesk` | a new role, a new common form (`Form.xml`, module) | 3, 4 | 433 |
| n5, n6 | `ServiceDesk` | a role removed, a common module removed | 3 | 429 |

Refused, as designed (`ServiceDesk`): the synonym of an own catalog
(`Catalogs/сд_Контрагенты.xml`: "the descriptor of an object that may own a
table") and the synonym in the root `Configuration.xml`. Not part of the table:
one platform export of the twin's staged state (case h1) came out with 171 of
184 files, the known intermittent short export of the native `ibcmd` (our export
of the same state has 184, and so has the platform's after its apply); it was
repeated.

What the comparison found on the way, all fixed: side files of help and picture
entries had no entry key (an edit of `Ext/Help/ru.html` was lost, caught by the
export-back check); a module saved without a byte order mark exported with one;
`LoadTransparent` true without a pixel (SVG pictures) was refused by the
ExtPicture writer.

## Cross-check against the platform fixtures of upstream PR 387

Upstream PR 387 carries platform-made extension fixtures
(`tests/fixtures/external/{extension_roots,adopted}`): an extension as a `.cfe`
container beside the tree the 8.3.27.2214 platform dumped, each set apart by a
probe that changes one value. The test
`the_export_equals_the_platform_dumps_of_the_upstream_fixtures` runs the
extension export over them (skipped where the fixtures are absent; point
`IBCMD_UPSTREAM_FIXTURES` at the directory). The first pass over them found six
readings the БСП corpus could not tell apart and the probes show wrong; all
are corrected and equal the platform now:

| reading | first pass | probes |
|---|---|---|
| catalog `37f2fa9d` / `37f2fa9e` | code type / code length | code length / code type |
| information register `09c412e0` / `13134205` | periodicity / write mode | write mode / periodicity |
| root `6a447e3f` / `d22e852a` | managed application module / hidden | `DefaultRoles` / managed application module |
| root tuple member 3 | default run mode | `ScriptVariant` (0 English, 1 Russian) |
| root tuple member 21 | not read | `DefaultRunMode` (0, 1, 2 auto) |
| root tuple member 49 (41 before) | keep mapping to the extended objects | keep mapping (0, 1); member 41 is 2 everywhere |

The extension corpus agreed with every wrong reading (both members of a pair
always listed together; the run mode and the script variant both `1`). What the
export equals now: the five root cases (`values`, `spellings`, `modules`,
`roles`, `values_v85`) and the adopted-object cases `props_all`, `props_b0`,
`props_b1`, `props_b2`, `module_all`, `module_b0..b2`, `catalog_modules`,
`catalog_object_module`, `document_children`, and, with the adopted-form writer
of upstream PR 387 (`form_extension`, one implementation for the `.cfe` and the
SQL export), `form_events`, `form_events_shared`, `role`, `subscription`,
`kinds` and `foreign_links` (22 cases in all). Ids the export did not know were
added from the same probes (owners, hierarchy, number properties, object and
manager modules, `ReturnValuesReuse`, `Value` of a style item, `Group` of a
common command, event subscription `Source`, filter criterion `Content`,
form type of an adopted form; the root's modules, `ModalityUseMode`,
`CompatibilityMode`), an adopted register prints `<ChildObjects/>` and a
subsystem `<Content/>` when empty, the states of an object's modules are
written in the platform's fixed order, and the `80327` storage format of the
extensions the 8.3.27.2214 platform creates itself is readable.

Still different (fail closed unless noted): predefined items and exchange plan
content of an adopted object (`<ExtensionState>`, `<ExtensionProperty>`), a
widened type with a check value, and the container `extension_roots/unknown_property`
(its CAS content is missing). The test prints them (`open upstream case ...`).

## Inferences from one sample

These are read off a single native sample; the evidence is in the lab folder
(`F:\ibcmd\lab\05\ext`), and a second sample may refine them:

* the `dcssch` boundary: 8.3.14 writes none, 8.3.21 and 8.3.24 write it, 8.3.15 is
  assumed;
* `EnableDrag` of a planner field is option slot 6 (the only planner on record);
* `callType="Before"` on the events and commands of an adopted form without a
  base form record (one form on record);
* the completion of old items is proved on 8 forms of one extension (ids, order,
  the dynamic list defaults); other kinds of items (pages, groups) that an older
  platform completed may exist;
* 8.5: the asset ids (one extension), `Version8_5EnableTaxi` (the one
  combination `3` + `6` on record) and the brand colour of a style (one style
  body) are read off single samples;
* `OnBasePeriod` for `DependenceOnCalculationTypes` `2` and the values other than
  `0`/`1` of the accounting register's lock mode and full-text search are the
  enumeration order, not observed.

## Not covered yet

* the ERP УХ 8.5 corpus for the 8.5 readers (no such clone was made);
* the load of an 8.5 extension (the 8.5 form loader has the planner bag entry
  and the empty-source characteristic compiles, but no 8.5 extension was loaded);
* the whole-tree load on the 8.5 clone (extension writes are declared
  unsupported for 8.5), of interceptors of an adopted form and of a binary
  template (`Template.bin`): the compiler has them, no case ran;
* a structural change of an extension (a new attribute, an object with a table,
  a body added to an object that has none) is the platform's own load: the
  load refuses it by file and the activation restructures nothing;
* the drop-in route on the other extensions (the route is the same export; only
  `_ДемоРасширение` of the БСП 8.3.27 clone and `ServiceDesk` of the БСП 8.5
  clone were run through it).

## The drop-in route

`ibcmd-rs infobase config export --extension=<name> <dir>` (also `-e <name>`)
runs the export above under the drop-in command line: the connection, the XML
version (`--platform`) and the output directory (empty or absent, as for the
configuration) are the ones of `infobase config export`; the image is the
staged one when the extension has staged rows, else the active one
(`--image auto`). An unknown name fails; so does an extension with an opaque or
failed storage row, after writing the tree. `infobase config import` still
refuses `--extension`. Measured 2026-09-30: `_ДемоРасширение` of the БСП 8.3.27
clone (`--platform=8.3.27`) equals the native export in all 185 files, and
`ServiceDesk` of the БСП 8.5 clone (`--platform=8.5.1`) in all 633 files
(`ConfigDumpInfo.xml` included, compared byte for byte).

## Reproducing

```bash
# reference: native export of every extension of the clone
pwsh -NoProfile -File F:/ibcmd/lab/05/ext/tools/native_export.ps1
# ours + comparison, listing of what is left
bash F:/ibcmd/lab/05/ext/tools/run_ours.sh <run>
# the БСП 8.5 clone: reference tree, database, platform
bash F:/ibcmd/lab/05/ext/tools/run_ours.sh <run> native/85 <database> 8.5.1
python -X utf8 F:/ibcmd/lab/05/ext/tools/left.py <run>
# the ordinary export must not move
bash F:/ibcmd/lab/05/ext/tools/main_regress.sh bsp8327 <run>
```
