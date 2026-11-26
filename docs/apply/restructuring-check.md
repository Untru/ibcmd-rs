# Restructuring check: does applying the staged configuration change the data structure?

Issue [#338](https://github.com/Untru/ibcmd-rs/issues/338), track "rcheck", 0.4 "Своё применение конфигурации".
Checkpoint 1 of 2026-09-29.

Scope of the measurements: platform 8.3.27.2214 (native `ibcmd`), Microsoft SQL Server 2025, the БСП demo
configuration (4 929 descriptors and 4 906 body rows, 9 838 rows in `Config`) restored from the lab corpus; the 8.5 БСП for the
dialect smoke test. Everything below is **measured** unless it says **hypothesis**. The evidence files are listed
in section 9; the harness that produced them is in the lab folder `F:\ibcmd\lab\04\restructure-check` (its scripts
are summarised in section 9, they are not part of the product).

## 1. Summary

1. **What it is.** `ibcmd-rs mssql-apply-check --database X` reads the `ConfigSave` of a database and says
   whether applying it changes the **data structure** (tables, columns, indexes) or **data the platform derives
   from the configuration** (predefined items, route points of a business process, the content of an exchange
   plan, the scheduled jobs table). The answer is a `Verdict`: `needs_restructuring`, the reasons (object,
   Config row, property path, change) and the harmless changes. The future own apply (#337) refuses with
   `требуется штатный config apply: <причины>` when `needs_restructuring` is true.
2. **Fail closed.** A change is a restructuring unless a rule names its property as harmless, and a rule is
   written only for a property the native platform was **seen** to apply without touching a table, a column, an
   index or a stored row, and that is presentation or behaviour by its meaning. A row the check cannot read, a
   kind it does not know, a body row of an unknown role: a reason (class `unknown`). A false "needed" only costs
   the time of the native apply; a false "not needed" would corrupt a database.
3. **Three modes, one engine.** The ConfigSave of a database against its active Config (`mssql-apply-check`);
   a source tree against the active Config (`mssql-apply-check --tree`, which sees what a stage dropped);
   two source trees (`apply-check-trees`). All three go through the same descriptor rules and give the same
   `Verdict`.
4. **The oracle is not `--dynamic=force`.** The task assumed that `infobase config apply --dynamic=force`
   refuses a change that needs restructuring. It does not (section 5.1): with no sessions it restructured
   dynamically every time, so "force succeeded" says nothing about the structure. The oracle is the native
   apply's own log (`Объект изменен`, `Новый объект`, `Реструктуризация …`) plus digests of the SQL schema.
5. **Evidence.** 159 single-property probes of 32 kinds and eight focused runs
   gave the rules (6.1, 6.2). 34 further modifications of 12 kinds of object, staged by our import on fresh clones
   and applied by the native platform, were judged by the check before the apply: it agrees with the platform on
   all 34 (13 restructured or rewrote data, 21 did not) and never said "no restructuring" where the platform
   restructured; one false alarm (an exchange plan's flag) led to a rule (6.3). The four reference exports
   (БСП and ERP УХ, 8.3.27 and 8.5) give no reason against databases restored from the same corpora, and 17 596
   real metadata files rewritten by the model's writer compare equal (6.4). The 8.5 dialect and the 8.3.27 one
   both work. A native-imported ConfigSave of the corpus (the ddl track's case a2, 9 842 rows) gives its one real
   reason and nothing else: 516 descriptors that differ in bytes are proven to differ by the record format only
   (3.6), and the `deleted` row is read (#404). **Not achieved:** a patch-mode stage is judged correctly but is
   not what the tree said (finding 1, why `--tree` exists). 115 unit tests.
6. **For the own restructuring (S1).** Every reason carries a stable rule id, the kind of the object, its path as
   steps and the operation (section 2), and `apply_check::s1::classify` maps a verdict onto the operations of S1
   or a refusal with a named code, without reading any text (section 10).

## 2. Commands and API

```
ibcmd-rs mssql-apply-check --database X [--server S] [--sql-user U --sql-pwd-env V] [--xml-version 2.20|2.21]
                           [--json] [--fail-on-restructuring] [--s1]
ibcmd-rs mssql-apply-check --database X --tree DIR [--partial] [--json] [--fail-on-restructuring] [--s1]
ibcmd-rs apply-check-trees --old DIR --new DIR [--json] [--fail-on-restructuring] [--s1]
```

All three only read. Exit code: 0 (the check ran; `--fail-on-restructuring` turns "needed" into 10), 1 on an error.
The text report is Russian, `--json` prints the `Verdict`:

| field | meaning |
|---|---|
| `needs_restructuring` | some reason exists |
| `reasons[]` | `class` (`structure`, `data`, `unknown`), `object` (`Catalog._ДемоКассы`), `file_name` (Config row or tree file), `property` (`Properties/CodeLength`, `ChildObjects/Attribute[Код]/Properties/Type`, or a body role such as `Predefined`), `change` (`9 -> 12 (a property no rule covers)`, `added`, `removed`); and the same as data, for a program: `rule` (a stable id, `column-added-or-dropped`), `kind` (`Catalog`), `path` (`[{"name":"ChildObjects"},{"name":"Attribute","label":"Код"}]`), `op` (`{"added"}`, `{"modified":{"old","new"}}`, `removed`, `reordered`; absent for a reason that is not a change of a descriptor) |
| `notes[]` | the changes that were seen and are harmless (capped at 500, the rest is counted) |
| `objects[]` | **every** object whose descriptor differs, harmless or not, no cap: `object`, `id` (uuid), `file_name`, `kind`, `op` (`added`, `removed`, `changed`), `class` (the most serious among its changes, `null` when all are harmless), `changes` |
| `stats` | files, rows, descriptors compared, body rows by role, unreadable rows, files not compared, `format_upgrades` (rows proven to differ by the record format only, 3.6) |
| `source` | `rows`, `tree-db` or `trees` |
| `incomplete` | files that carry data were left out of the comparison (`stats.body_files_not_compared`, a tree against a database): `needs_restructuring: false` says nothing about them; `Verdict::is_conclusive()` |

Rust: `apply_check::check_staged(sql, database, xml_version)`, `check_tree_against_db(sql, database, tree,
xml_version, partial)`, `check_trees(old, new)`, all returning `Verdict`; `Verdict::refusal()` is the message an
apply prints.

**Rule ids.** `RuleId` (`apply_check::rule_id`) names the rule of `rules::decide` or the step of the check that
made a reason: `column-added-or-dropped`, `tabular-section-added-dropped-moved`, `property-not-covered`,
`row-decodes-to-same-xml`, `row-format-upgrade-unproven`, `deleted-row-not-empty`, ... (`RuleId::ALL`). The
text that `change` still prints in brackets is `RuleId::text()`; nothing should match that text. A reason built
without a rule is `unspecified`, class `unknown` (`Reason::default()`), and every consumer treats it as a refusal.
`--s1` appends what the own restructuring makes of the reasons (section 10).

`--tree` is what an **import** must run on its input. The ConfigSave check judges what the ConfigSave holds; it
cannot tell that a stage dropped a change the tree had. The patch-mode import of this tool does exactly that with
an added attribute (finding 1 of the ddl track, reproduced in section 6.3): the ConfigSave has no attribute, so
its check is right to say "nothing to restructure" and wrong to be the only check. `check_tree_against_db` answers
"which objects' descriptors differ between this tree and the database, and is each difference structural"
(`Verdict::objects`); it needs only the tree and a connection. It compares descriptors; the files that carry
stored data (predefined items, flowcharts, the content of exchange plans, aggregates, additional indexes) are
counted in `stats.body_files_not_compared`, not judged: compare an export of the database with the tree
(`apply-check-trees`) or check the ConfigSave the import produced.

## 3. How the check decides

### 3.1 The configuration in the tables

`Config` holds the active configuration, `ConfigSave` the staged one; each is a set of named rows (`FileName`,
`PartNo`, raw-deflated `BinaryData`, parts joined in order):

| row | what |
|---|---|
| `<uuid>` | the **descriptor** of a metadata object (a brace-text tree: properties, child objects) |
| `<uuid>.<suffix>` | a **body** of the object; the meaning of the suffix depends on the owner's kind (`.0` is the module of a catalog and the form text of a form) |
| `root`, `version`, `versions` | the configuration's root, its compatibility record and the **inventory** of every file of the configuration (`{1,<count>,"",<generation>,"<name>",<version>,…}`) |
| `<base>_dynupdate_<generation>[.suffix]`, `DynamicallyUpdated` | the state of an online (dynamic) update, see below |

The inventory is what makes a removal visible: a file the staged `versions` no longer lists is gone, a file the
active one did not list is new, and a file a ConfigSave lists but does not hold is unchanged. So the **effective
staged configuration** is the active Config overlaid by the ConfigSave, restricted to the files the staged
`versions` lists.

**The active Config is not the plain rows.** An online update leaves the plain rows as they were and writes the
rows it changed under `<base>_dynupdate_<generation>`; `DynamicallyUpdated` lists the generations, oldest first,
and for each name the newest generation that carries it wins. Reading the plain rows would compare a stage with
a configuration that no longer exists. An object added by an online update has **only** an alias row, and an
object it dropped keeps its plain row; the check therefore takes the descriptors from the inventory, not from
the list of plain rows (`sql::published_names`).

### 3.2 Three boxes for every staged row

* **Descriptor** (a row without a suffix). Decoded on both sides with the metadata model (`metadata_model`, the
  same decoders the export uses) into the element tree the XML export would write, then diffed **property by
  property** (`tree_diff`): child objects are matched by uuid, then by name, then by position; localized strings
  item by item; a reordered collection is one change. Every change goes through `rules::decide` (section 4). Two
  rows that differ in bytes but decode to the same tree are compared as brace trees (3.6): a reason
  (`unknown`, `row-format-upgrade-unproven`, with the first place that differs) unless every difference is the
  record format of the platform that staged the row.
* **Body** (`<uuid>.<suffix>`). The owner's kind and the suffix give the role (`roles::body_role`, read off the
  `ConfigDumpInfo.xml` of the four reference exports). Roles that copy rows when applied (modules, forms,
  templates, help, pictures, rights, command interfaces, schedules, packages) are **counted, not compared**.
  Roles that carry data (`Predefined`, `Flowchart`, `Content`, `Aggregates`, `AdditionalIndexes`) are compared by
  content; a difference is a reason of class `data` (or `structure` for `Content`, `Aggregates`,
  `AdditionalIndexes`). The `Content` row of an exchange plan is compared as a **set of objects with their flags**:
  the same items in another order (the platform keeps an order of its own, a stage writes them in another; six
  rows of the БСП differ in nothing else) and the same objects with another flag of automatic registration are
  harmless; an object that joins or leaves is `structure` (a registration table comes or goes).
* **Service** (`root`, `version`, `versions`). `root` and `version` must not change (a `root` that differs from the
  stored one in the final 16-byte block of its payload only is the 8.5 platform's re-stamp, which it applies on every
  write, and passes with a note: `crates/ibcmd-apply-check/src/apply_check/root_row.rs`, `evidence/dropin-apply/s1-acceptance-85.md`);
  `versions` is the inventory,
  and it also gives every file a version id. A file that the staged `versions` gives another version than the
  active one (or lists as new) and that the ConfigSave does not hold means a partial stage (the native import
  under load made some, see finding 6): `unknown`. (A base-free stage gives *every* file a new version id, so the
  ids cannot say which files changed; they only have to be backed by rows.)

The `deleted` row a native import writes is read: its content `0` means no file was deleted and it changes
nothing; anything else (a list of files) is `unknown`, `deleted-row-not-empty`, because the check judges removals
by the staged `versions` row and has not read a list.

Anything else in the ConfigSave (a row named like an online-update alias or `DynamicallyUpdated`, a row the
inventory does not list, a row of an unknown shape) is a reason of class `unknown`.

### 3.3 Fail-closed defaults

* `rules::decide` returns `structure` unless a rule says `safe`.
* A kind the model has no decoder for, a row that does not parse, a plan that cannot be built completely (a name
  nobody could find, an undecodable Configuration row: the compatibility mode then defaults and thousands of rows
  would misdecode, so the root cause is reported once as `unknown`): reasons.
* A ConfigSave without a `versions` row cannot say what is gone: `unknown`.
* The classes: `structure` (tables, columns, indexes change), `data` (stored data the platform derives from
  the configuration changes: predefined items, enumeration values, route points, scheduled jobs), `unknown`.

### 3.4 Tree against tree, tree against database

The tree modes use the same descriptor rules. Trees are matched **by object uuid**, not by path, so a moved or
renamed file is one object; the other files are told apart by where they lie under the object's `Ext` folder
(`roles::file_role`). `ConfigDumpInfo.xml` is derived from the tree and never a change of its own.

Tree against database exports each descriptor of the active Config with the metadata model (the text an export
would write, `export_descriptor`) and compares it with the tree's file: byte-equal is unchanged, otherwise both
are parsed and diffed. A tree that is an export of the database therefore compares as unchanged whatever the
formatting, and a hand-edited one reports exactly its edits. An empty container written the long way
(`<ChildObjects>` newline `</ChildObjects>`) is the same as `<ChildObjects/>`.

### 3.5 The verdict is built to take a stronger test

The whitelist of harmless properties is the first cut. The platform itself decides by recomputing `DBSchema`
from the metadata and rebuilding exactly the tables whose entry differs (ddl track, `docs/apply/restructuring.md`,
finding 7). A check on `DBSchema` entries is stronger and plugs into the same place without changing the verdict:
`Verdict::objects` lists every changed object with its kind, uuid and Config row, so a generator of table entries
walks the table-owning ones, and a `structure` or `unknown` reason on such an object is what it may confirm or
withdraw (its own reasons would carry `property = DBSchema/<table>`). The generator is not part of this module
(it belongs to the ddl track).

### 3.6 Rows that differ only by the record format

A native `config import` writes every descriptor it stages in the record format of its own edition, changed or
not (finding 9 of section 8): in the ddl track's case a2 (9 842 rows, an attribute added to
`Catalog._ДемоПартнеры`) 517 of 4 929 descriptors differ from the stored ones in bytes, one of them by the
attribute. The model reads a row by the fields it knows, so 396 of them decode to the same XML in the same format,
119 decode to the same XML only in the newer format, and the Configuration row is rewritten from the `{67}` to the
`{68}` shape. A check that trusted the decoding would call them unchanged, one that trusted the bytes would call
all of them changes. `apply_check::upgrade` settles it on the rows: both are read as brace trees and aligned
item by item, and the staged row is the stored one in the newer format when every difference is one of

| difference | meaning |
|---|---|
| the first item of a list is one greater | the record version |
| an integer after the head is one greater, and an entry was inserted in that same list (never more counters than inserted entries in the row) | a counter of a counted list |
| an entry was inserted, and it is one of the platform's defaults | the entries a new record version introduces: `0`, `1`, `5`, the class id `3b10624f-...`, `{1,<nil uuid>}`, `{"#",502b7765-...,{502b7765-...,0}}` (and `...,2}`), and three entries of the Configuration row |
| an empty string became `"5"` | the default of a new version |
| in the Configuration row, an integer in `80300..80399` grew | the extension compatibility mode names the edition that staged the row |

The table of defaults is what the native import showed (`upgrade::FILLERS`), each entry an exact value, and
nothing else passes: not a number that grew by two, not a list that lost an item, not an entry inserted with a
value that is not in the table (a new attribute, a predefined item). A row that is not proven is a reason
(`row-format-upgrade-unproven`, class `unknown`) that names the first place that no rule explains
(`at 5/2/3: 24 -> 26`, or `items 2..: nothing -> {9,"X"}`); a caller that has no raw rows (the tree modes, the
tests' fake rows) keeps the old behaviour, the reason `row-decodes-to-same-xml`. The proof is a property of the
two rows and does not depend on the decoder being total. What it cannot vouch for is a change that a native
import of an XML could not have made: every property an XML can express is decoded and diffed as before.

**The Configuration row of the `{68}` shape.** The import stores the compatibility mode (field 26, 80324) as it was
and its own edition as the extension compatibility mode (field 43, 80327). The model refuses such a row (no corpus
had shown which of the fields the platform prints), and the check used to stop at it with two reasons. For the
comparison only, `fold_split_compatibility` reads field 26 as the compatibility mode and puts the value of field 43
back into `ConfigurationExtensionCompatibilityMode`, which is exactly what the `{67}` row prints (the edition of
the platform); the export keeps refusing the row.

Result on the a2 image (`s1h_corpus_tests`, ignored, the rows are in the lab): one reason, `structure:
Catalog._ДемоПартнеры ChildObjects/Attribute[ДемоНовыйРеквизит] added`; `format_upgrades` 516 (396 + 119 + the
Configuration row); the `deleted` row `0` accepted. The same image with one default entry changed by one digit is
refused by name for that row and no other, and a `deleted` row that lists a file is refused.

### 3.7 The active row can be in the newer format too

A native `config apply` promotes the staged rows into `Config` as the import wrote them. So an object that was
restructured natively once is stored from then on in the record format of that platform: on the twin of the
`rcheck-6` finding (a БСП 8.3.27 clone, compatibility mode 8.3.24, an attribute added to
`Catalog._ДемоПартнеры` by a native `import files --partial` and a native `apply`) the row `5eab8a1b-...` of `Config`
went from `{56,...}` to the very bytes of the staged row, `{57,...}`. The second change staged for the same catalog
was then compared against a row that the decoder, reading in the compatibility mode, refuses ("record version 57,
the compatibility mode stores 56"): a reason `row-undecodable`, class `unknown`, and S1 refused it as `unknown-step`.
The stored side now goes through the same fallback as the staged one (`Decoder::decode_stored`, `Describe::
describe_stored`; the export of `--tree` mode too): the compatibility mode first, else the newest mode the model
knows. Result on the twin, three second changes to the same catalog, each staged by the native `import files
--partial`:

| second change | before | after |
|---|---|---|
| another attribute | `unknown`, `row-undecodable` | `structure`, `column-added-or-dropped`, S1 `add-attribute` |
| `Length` 30 -> 60 of the attribute the first apply added | `unknown` | `structure`, S1 `widen-string` |
| `CodeLength` 9 -> 12 | `unknown` | `structure`, S1 refusal `property-outside-s1` |

The proof of 3.6 reads both ways (`Upgrade::reversed`): the staged row may be the older format over a promoted newer
one (a stage that an importer writes for the compatibility mode), with the same table of defaults. Unit tests:
`record_format_tests` (rows compiled by the model's writer from a small synthetic catalog in the two compatibility
modes; four of the six fail without the fix), evidence: `evidence/restructuring-check/second-change-after-native-apply.md`.

## 4. The rules

`rules::decide(kind, change)` gives every change of a descriptor one of `safe`, `structure`, `data`, and the rule
that gave it (it is printed in the reason: `10 -> 601 (a property no rule covers)`). The default is `structure`.
A property is listed as `safe` only when **both** hold:

* the native platform was **seen** to apply the change without a `Объект изменен` / `Реструктуризация` line and
  without a change of columns, indexes or tables in the digests (section 5), and
* the property is presentation or behaviour by its meaning, not a way the data is kept. A property the platform
  left alone in one probe but that decides how data is stored stays off the list: `HierarchyType` and
  `LimitLevelCount` of a catalog, the attribute-level `DataHistory` (harmless in a probe on an object whose own
  history is off), `Owners`, the type of anything.

The tables below are `rules.rs`; the evidence for each row is the probe (section 6.1), the focused experiment
(6.2) or the validation run (6.3) named there.

### 4.1 Kinds that own no stored data: every change is harmless

`CommonModule`, `CommonForm`, `CommonTemplate`, `CommonPicture`, `CommonCommand`, `CommandGroup`, `Role`,
`Subsystem`, `EventSubscription`, `FunctionalOption`, `FunctionalOptionsParameter`, `StyleItem`, `Style`,
`PaletteColor`, `Interface`, `FilterCriterion`, `Report`, `DataProcessor`, `Form`, `Template`, `WebService`,
`HTTPService`, `WSReference`, `XDTOPackage`, `IntegrationService`, `Bot`. Adding or removing such an object is
harmless too. Why: none of them allocates a table or a field. The ddl track's census of the `DBNames` of the
БСП (`docs/apply/evidence/restructuring/dbnames-kinds.txt`) lists the metadata classes that do (catalogs,
documents, registers, constants, enumerations, exchange plans, charts, business processes, tasks, document
journals, scheduled jobs, common attributes, and the attributes, dimensions, resources, columns and tabular
sections inside them); none of the kinds above is there. The probes touched every top-level property of at least
one object of every kind in the list (6.1) and no property of any of them made the platform act; an attribute
added to a report and to a data processor (`p5_children`), a common module and a role added and dropped
(`p7a`, `p7b`, `v08`, `v13`) likewise.

### 4.2 Properties harmless on every kind that has them

`Synonym`, `Comment`, `Explanation`, `ObjectPresentation`, `ExtendedObjectPresentation`, `ListPresentation`,
`ExtendedListPresentation`, `IncludeHelpInContents`, `DefaultObjectForm`, `DefaultFolderForm`, `DefaultListForm`,
`DefaultChoiceForm`, `DefaultFolderChoiceForm`, `DefaultRecordForm`, `DefaultForm`, `AuxiliaryObjectForm`,
`AuxiliaryFolderForm`, `AuxiliaryListForm`, `AuxiliaryChoiceForm`, `AuxiliaryFolderChoiceForm`, `QuickChoice`,
`ChoiceMode`, `EditType`, `CreateOnInput`, `ChoiceHistoryOnInput`, `DefaultPresentation`,
`SearchStringModeOnInputByString`, `FullTextSearchOnInputByString`, `DataLockControlMode`, `FullTextSearch`.
The Extended and Auxiliary ones were probed on catalogs (`p8`); `AuxiliaryRecordForm` and `AuxiliaryForm` were not
and are `structure` until someone does.

### 4.3 Properties harmless on one kind

| Kind | Properties |
|---|---|
| `Catalog` | `UseStandardCommands`, `Autonumbering`, `CheckUnique`, `InputByString`, `PredefinedDataUpdate`, `UpdateDataHistoryImmediatelyAfterWrite`, `ExecuteAfterWriteDataHistoryVersionProcessing` |
| `Document` | `UseStandardCommands`, `Autonumbering`, `PostInPrivilegedMode`, `UnpostInPrivilegedMode`, `RealTimePosting`, `RegisterRecordsDeletion`, `RegisterRecordsWritingOnPost`, `SequenceFilling`, `UpdateDataHistoryImmediatelyAfterWrite`, `ExecuteAfterWriteDataHistoryVersionProcessing` |
| `Constant` | `UseStandardCommands`, `ExtendedEdit`, `FillChecking`, `MarkNegatives`, `MultiLine`, `PasswordMode`, `UpdateDataHistoryImmediatelyAfterWrite`, `ExecuteAfterWriteDataHistoryVersionProcessing` |
| `InformationRegister` | `UseStandardCommands`, `UpdateDataHistoryImmediatelyAfterWrite`, `ExecuteAfterWriteDataHistoryVersionProcessing` |
| `Enum`, `AccumulationRegister`, `AccountingRegister`, `CalculationRegister`, `ChartOfAccounts`, `ChartOfCharacteristicTypes`, `ChartOfCalculationTypes`, `ExchangePlan`, `BusinessProcess`, `Task` | `UseStandardCommands` |
| `Configuration` | `Version`, `Vendor`, `UpdateCatalogAddress`, `Copyright`, `BriefInformation`, `DetailedInformation`, `VendorInformationAddress`, `ConfigurationInformationAddress` |
| `DocumentJournal` | none (the probe of its `UseStandardCommands` made the platform rebuild a registration table) |
| `ScheduledJob` | `Comment` only; every other property is stored data (`data`) |

### 4.4 Child objects

* **Attributes, dimensions, resources, columns, accounting flags, addressing attributes, parameters**: adding,
  dropping or moving one is `structure`; a property is harmless when it is one of `Name` (a rename: the column is
  named by a number), `Synonym`, `Comment`, `ToolTip`, `FillChecking`, `QuickChoice`, `ChoiceHistoryOnInput`,
  `CreateOnInput`, `MultiLine`, `PasswordMode`, `ExtendedEdit`, `MarkNegatives`, `FillFromFillingValue`,
  `FullTextSearch`, `ChoiceFoldersAndItems`, `DenyIncompleteValues`. Anything else (`Type`, `Indexing`, `Use`,
  `DataHistory`, `Master`, `MainFilter`, `Format`, `Length`, ...) is `structure`.
* **Standard attributes** (`Code`, `Description`, `Parent`, ...): the same list of properties.
* **Tabular sections**: adding, dropping or moving one is `structure`; `Name`, `Synonym`, `Comment`, `ToolTip`,
  `FillChecking` and the same list of properties of its own attributes and standard attributes are harmless; adding
  or dropping an attribute of one is `structure`.
* **Enumeration values**: adding, dropping or moving one is `data` (a value is a row of the platform's table);
  `Name`, `Synonym`, `Comment` of a value are harmless.
* **Forms, templates, commands** listed in `ChildObjects`: harmless (they are rows and lists of names).
* `InternalInfo` (the generated types of an object) is `structure`.

### 4.5 Adding and dropping objects

By kind: an object of a kind of 4.1 is harmless, any other kind is `structure`. In `Configuration.xml` the list
`ChildObjects/<Kind>` follows the same rule, so listing a new common module is harmless and listing a new catalog
is not. What the platform does when an object is dropped is bigger than the object: dropping the catalog of
`p7b` made it rebuild the change-registration tables of 29 other objects.

### 4.6 Body rows

Read off the `ConfigDumpInfo.xml` of the four reference exports; `roles::ROLES` has 96 (kind, suffix) pairs.

| effect | roles |
|---|---|
| copies a row when applied | every module (`ObjectModule`, `ManagerModule`, `RecordSetModule`, `CommandModule`, `Module`, `ValueManagerModule`, the configuration's application and session modules), `Form`, `Template`, `Picture`, `Help`, `Rights`, `CommandInterface`, `Schedule`, `Package`, `Style`, `WSDefinition`, `Splash`, `HomePageWorkArea`, `MainSectionCommandInterface`, `MainSectionPicture`, `ClientApplicationInterface`, `ParentConfigurations`, `MobileClientSignature`, `StandaloneConfigurationContent` |
| `data` when the content changes | `Predefined` (catalogs `.1c`, charts of characteristic types `.7`, of accounts `.9`, of calculation types `.2`), `Flowchart` (business process `.7`) |
| `structure` when the content changes | `Content` (exchange plan `.1`, compared as a set), `Aggregates` (accumulation register `.3`), `AdditionalIndexes` (accumulation register `.4`, document `.3`) |

An unknown (kind, suffix) pair is `unknown`.

## 5. The oracle: what the native platform does

### 5.1 `--dynamic=force` does not refuse a restructuring

The task's premise was that `infobase config apply --dynamic=force` fails when the staged configuration needs
restructuring, so that "force succeeded" could serve as the oracle for "no restructuring needed". It does not
fail. Setting: no sessions on the infobase (the lab cannot register a clone in the 1C server cluster: the server's
service account has no SQL login), `--force --dynamic=force --session-terminate=disable`, native 8.3.27.2214. Of 15
applies of `--dynamic=force` recorded in the lab, 13 ended with exit 0 and the two others were rejected by the
metadata check ("Проверка корректности метаданных": probes with invalid values), never by the dynamic mode.
Every structural change in them was **carried out dynamically**: the platform created the new generation of the
table, copied the data and switched it, and printed the same lines as the exclusive apply of the ddl track:

| change staged (one native `force` apply each) | what the platform did (log and SQL digests) |
|---|---|
| a `String(20)` attribute added to `Catalog._ДемоКассы` (`e08_attr_add`) | `Объект изменен: Справочник._ДемоКассы`, `Реструктуризация Справочник._ДемоКассы`; `DBSchema`, `SchemaStorage`, columns changed; exit 0 |
| `CodeLength`, `DescriptionLength`, `DataHistory` of three catalogs (`p1_catalog_props`) | three objects changed and restructured, columns changed; exit 0 |
| `Indexing` and `StringLength` of attributes (`p2_attribute_props`) | two objects restructured, columns and **indexes** changed; exit 0 |
| an enumeration value added, `Use` of an attribute, a dropped index (`p4_children`) | three objects restructured, row counts changed (the value is a row); exit 0 |
| a tabular section added, a table attribute added, a dimension added to an accumulation register, an attribute dropped (`p5_children`) | four objects restructured, a **new table** (`sql-tables`), totals of the register recalculated; exit 0 |
| a new catalog (`p7a_objects_add`) | `Новый объект: Справочник.СправочникRcheck`, `Изменена структура таблиц базы данных`, a new table; exit 0 |
| the same catalog dropped (`p7b_objects_remove`) | `Объект удален: Справочник.СправочникRcheck`, and 29 other objects restructured; exit 0 |
| 20 properties of 20 objects in one stage (`probe_auto_a`, third run) | 20 objects changed and restructured at once; exit 0 |

So `force` succeeding says nothing about the structure: a check must not be validated against it and an apply must
not rely on it. **What was not measured:** what `--dynamic=force` does when sessions are connected (the platform
may then refuse a change that needs exclusive access; the lab has no way to open one). This does not change the
design: the check answers "does applying it change tables or stored data", whatever the way it is applied.

### 5.2 What the oracle is

For every modification the harness stages an edited tree with **our import in `--base-free` mode** (the only mode
that stages descriptor edits; section 6.3 shows why patch mode cannot serve), records the verdicts of the three
modes, runs the native `infobase config apply --force --dynamic=<mode> --user=Администратор` (under the lab's
`native` lock) and reads three things:

1. the platform's own report: `Объект изменен: X` (the object's descriptor changed in a way the platform acts on),
   `Новый объект: X`, `Объект удален: X`, `Создана таблица: …`, `Изменена структура таблиц базы данных`,
   `Реструктуризация X` (rows of X are copied to the new generation of its table);
2. digests of the SQL schema before and after (`tools/snapshot.ps1`): every table's columns, an index signature by
   columns, the set of table object ids, row counts;
3. which of `Config`, `Params`, `DBSchema`, `SchemaStorage` changed.

A modification counts as **restructuring** when the log names the object as changed, new or deleted, or the
columns, indexes, tables, `DBSchema` or `SchemaStorage` differ, or the row count of a table of the configuration's
data changed (a predefined item, an enumeration value). Noise that every apply makes is ignored: the tables
`_ConfigChngR` and `_ConfigChngR_ExtProps` are rebuilt on every apply (the primary key gets a new name; the log line
`Реструктуризация Таблица регистрации изменений конфигурации` is theirs), `Params` rows (`*.si`, `siVersions`) are
rewritten, and the first apply of a clone also collects garbage in `ConfigCAS` and `Files` and fills
`_ExtensionsRestructNGS` (row counts of service tables do not count).

### 5.3 Two ways to apply, one way to read

`--dynamic=disable` (exclusive) promotes every staged row into `Config` under its own name; `--dynamic=force` leaves
the plain rows alone and writes **every staged row** as `<name>_dynupdate_<generation>` (a full ConfigSave doubles
`Config` at each apply: about 10 000 rows; a lab database that had ten of them held 60 000 rows) and appends the
generation to `DynamicallyUpdated`. The check reads the active rows in both cases (section 3.1). The lab
chain for section 6.1-6.2 used `force` (database `a`), the validation lanes of section 6.3 used `disable`
(databases `b1`-`b3`), and the two agree wherever both were run.

## 6. Evidence

### 6.1 Probes: one property of one object, 159 times

`probe.py` of the lab (not part of the product) takes the top-level `<Properties>` leaves of the objects of each
kind, changes **one leaf of one object** (a boolean flipped, an enumeration moved to another value, a number
changed by one, a string lengthened), stages all of them at once and applies once: the native log names the
objects it acted on, and because every object carries exactly one change, each change is attributed. 168 probes
were generated; 9 of them (an exchange plan's `IncludeConfigurationExtensions`, a common attribute's `AutoUse`
and `DataSeparation`, an HTTP service's `RootURL`, a web service's `Namespace` and `DescriptorFileName`, an
information register's `WriteMode`, an XDTO package's `Namespace`) were rejected by the metadata check of the
platform for the value chosen and left out, 159 were applied (native exit 0). 139 were applied without the platform
acting, 20 made it restructure or rewrite stored data. `Applied` below means "no `Объект изменен`, no
`Реструктуризация`, no change of columns, indexes or tables"; the rule tables of section 4 take from this only the
properties that are also presentation or behaviour by meaning.

| Kind | Applied without touching tables or stored data | Made the platform restructure or rewrite stored data |
|---|---|---|
| AccountingRegister | `IncludeHelpInContents`, `UseStandardCommands` | - |
| AccumulationRegister | `UseStandardCommands` | `RegisterType` |
| BusinessProcess | `DefaultObjectForm`, `UseStandardCommands` | - |
| CalculationRegister | `UseStandardCommands` | - |
| Catalog | `Autonumbering`, `CheckUnique`, `ChoiceHistoryOnInput`, `ChoiceMode`, `Comment`, `CreateOnInput`, `DataLockControlMode`, `DefaultChoiceForm`, `DefaultFolderChoiceForm`, `DefaultFolderForm`, `DefaultListForm`, `DefaultObjectForm`, `DefaultPresentation`, `EditType`, `ExecuteAfterWriteDataHistoryVersionProcessing`, `FullTextSearch`, `HierarchyType`, `IncludeHelpInContents`, `LimitLevelCount`, `PredefinedDataUpdate`, `QuickChoice`, `SearchStringModeOnInputByString`, `UpdateDataHistoryImmediatelyAfterWrite`, `UseStandardCommands` | `CodeAllowedLength`, `CodeSeries`, `CodeType`, `DataHistory`, `FoldersOnTop`, `Hierarchical` |
| ChartOfAccounts | `UseStandardCommands` | - |
| ChartOfCalculationTypes | `UseStandardCommands` | - |
| ChartOfCharacteristicTypes | `IncludeHelpInContents`, `UseStandardCommands` | `CodeAllowedLength`, `FoldersOnTop`, `Hierarchical` |
| CommandGroup | `Category`, `Representation` | - |
| CommonAttribute | `ExtendedEdit`, `FillFromFillingValue`, `MarkNegatives`, `MultiLine`, `PasswordMode` | - |
| CommonCommand | `Comment`, `Group`, `IncludeHelpInContents`, `ModifiesData`, `ParameterUseMode`, `Representation` | - |
| CommonForm | `Comment`, `IncludeHelpInContents`, `UseStandardCommands` | - |
| CommonModule | `ClientManagedApplication`, `ClientOrdinaryApplication`, `Comment`, `ExternalConnection`, `Global`, `Privileged`, `ReturnValuesReuse`, `Server`, `ServerCall` | - |
| CommonPicture | `AvailabilityForAppearance`, `AvailabilityForChoice`, `Comment` | - |
| CommonTemplate | `Comment`, `TemplateType` | - |
| Constant | `Comment`, `DataLockControlMode`, `DefaultForm`, `ExecuteAfterWriteDataHistoryVersionProcessing`, `ExtendedEdit`, `FillChecking`, `MarkNegatives`, `MultiLine`, `PasswordMode`, `UpdateDataHistoryImmediatelyAfterWrite`, `UseStandardCommands` | - |
| DataProcessor | `Comment`, `DefaultForm`, `IncludeHelpInContents`, `UseStandardCommands` | - |
| Document | `Autonumbering`, `DataLockControlMode`, `DefaultChoiceForm`, `DefaultListForm`, `DefaultObjectForm`, `ExecuteAfterWriteDataHistoryVersionProcessing`, `IncludeHelpInContents`, `PostInPrivilegedMode`, `RealTimePosting`, `RegisterRecordsDeletion`, `RegisterRecordsWritingOnPost`, `SequenceFilling`, `UnpostInPrivilegedMode`, `UpdateDataHistoryImmediatelyAfterWrite`, `UseStandardCommands` | `CheckUnique`, `NumberAllowedLength`, `NumberPeriodicity`, `Posting` |
| DocumentJournal | `DefaultForm`, `IncludeHelpInContents` | `UseStandardCommands` |
| Enum | `ChoiceHistoryOnInput`, `ChoiceMode`, `Comment`, `QuickChoice`, `UseStandardCommands` | - |
| EventSubscription | `Comment` | - |
| ExchangePlan | `DefaultObjectForm`, `DistributedInfoBase`, `IncludeHelpInContents`, `QuickChoice`, `UseStandardCommands` | - |
| FilterCriterion | `UseStandardCommands` | - |
| FunctionalOption | `Comment`, `Location`, `PrivilegedGetMode` | - |
| InformationRegister | `Comment`, `DataLockControlMode`, `DefaultListForm`, `DefaultRecordForm`, `EditType`, `EnableTotalsSliceFirst`, `EnableTotalsSliceLast`, `ExecuteAfterWriteDataHistoryVersionProcessing`, `FullTextSearch`, `IncludeHelpInContents`, `MainFilterOnPeriod`, `UpdateDataHistoryImmediatelyAfterWrite`, `UseStandardCommands` | `InformationRegisterPeriodicity` |
| Report | `Comment`, `DefaultForm`, `DefaultSettingsForm`, `IncludeHelpInContents`, `MainDataCompositionSchema`, `UseStandardCommands` | - |
| Role | `Comment` | - |
| ScheduledJob | `Comment` | `Description`, `Key`, `Predefined`, `Use` |
| SessionParameter | `Comment` | - |
| StyleItem | `Comment` | - |
| Subsystem | `Comment`, `IncludeHelpInContents`, `IncludeInCommandInterface`, `UseOneCommand` | - |
| Task | `UseStandardCommands` | - |

The 20 that acted are exactly the storage properties of section 4: the hierarchy, the code type and length and the
series of a catalog and of a chart of characteristic types, the history of data, the number length, periodicity,
uniqueness and posting of a document, the periodicity of an information register, the kind of an accumulation
register, the standard commands of a document journal, and four of the five properties of a scheduled job.

### 6.2 Focused experiments

Each ran on the БСП database `a` (a cumulative clone) with `--dynamic=force`, several objects per stage and one
change per object; the "native" column is the platform's log.

| run | changes | native acted on | ours |
|---|---|---|---|
| `p1_catalog_props` | 18 catalogs, one property each: `Autonumbering`, `Explanation`, `ObjectPresentation`, `ListPresentation`, `DescriptionLength`, `DefaultPresentation`, `QuickChoice`, `ChoiceMode`, `CodeLength`, `IncludeHelpInContents`, `CheckUnique`, `DataHistory`, `PredefinedDataUpdate`, `FullTextSearch`, `DataLockControlMode`, `UseStandardCommands`, `ChoiceHistoryOnInput`, `EditType` | exactly three: `DescriptionLength`, `CodeLength`, `DataHistory` | the same three |
| `p2_attribute_props` | 16 catalogs, one attribute property each: `Synonym`, `ToolTip`, `FillChecking`, `QuickChoice`, `Indexing`, `FullTextSearch`, `ChoiceHistoryOnInput`, `CreateOnInput`, `MultiLine`, `PasswordMode`, `ExtendedEdit`, `MarkNegatives`, `DataHistory`, `FillFromFillingValue`, `StringLength`, `Name` | exactly two: `Indexing` (index changed) and `StringLength` (column changed) | the same two; the attribute-level `DataHistory` is `structure` for us although the platform left it alone (the object's own history was off) |
| `p4_children` | an attribute's `ChoiceFoldersAndItems` and, on another catalog, its `Use` (`ForItem` to `ForFolderAndItem`, which makes the column nullable); an `Indexing` switched off; five standard-attribute properties (`FillChecking`, `MultiLine`, `FullTextSearch`, `DataHistory`, `ExtendedEdit`) on five catalogs; an enumeration value added; another renamed | three objects: the `Use` change, the `Indexing`, and the enumeration with the new value (a row added) | the same three; the enumeration value is `data` |
| `p5_children` | an attribute added to a report and to a data processor; a tabular section's `FillChecking`; a tabular section attribute's `MultiLine`; an attribute added to a tabular section; a whole tabular section added; a register dimension's `DenyIncompleteValues`; a dimension added to an accumulation register; the last attribute of a catalog dropped | four objects: the tabular section attribute added, the tabular section added, the dimension added (with the totals recalculated), the attribute dropped | the same four |
| `p6_config`, `p6b_kinds` | `Version`, `UpdateCatalogAddress`, `Copyright`, `BriefInformation`, `Vendor`, `DetailedInformation`, `VendorInformationAddress`, `ConfigurationInformationAddress` of the configuration; three scheduled jobs (`MethodName`, `RestartCountOnFailure`, `RestartIntervalOnFailure`); an event of an event subscription; `Comment` of a job, a subscription, a web service, an HTTP service, an XDTO package, a settings storage, a sequence | only the three scheduled jobs | the configuration's information and the comments are harmless; the jobs are `data` |
| `p7a_objects_add` | a common module, a catalog, a role added | the catalog (`Новый объект`, a new table) | the catalog is `structure`; the module and the role are harmless |
| `p7b_objects_remove` | the same three dropped | the catalog (`Объект удален`, and 29 other objects restructured) | the catalog is `structure`; the module and the role are harmless |
| `p8_catalog_presentation` | `ExtendedObjectPresentation`, `ExtendedListPresentation`, `FullTextSearchOnInputByString`, a field added to `InputByString`, and the auxiliary forms (`AuxiliaryObjectForm`, `AuxiliaryListForm`, `AuxiliaryChoiceForm`, `AuxiliaryFolderForm`, `AuxiliaryFolderChoiceForm`) of nine catalogs, one property each | none of them (Config and Params only) | all harmless after the rules were extended with `InputByString` (it was a `structure` reason before) |

The two properties that a reader may distrust because they were changed in a run that also changed something
structural, `ChoiceFoldersAndItems` and `DenyIncompleteValues`, sit in objects of their own in these runs (the
native log names the neighbours, not them).

### 6.3 Validation: modifications staged by our import, judged by us, applied by the native platform

The rules of section 4 were derived from 6.1 and 6.2 on the database `a`. The validation is a second set of
modifications, none of them probed before, staged on **fresh clones of the corpus backup** (`b1`, `b2`, `b3`,
pristine БСП 8.3.27; `bsp85` for the 8.5 dialect; `n1` for the native import), one after the other on each clone
(the base tree follows the database), each staged by our import, checked in three ways and applied by the native
`infobase config apply --force --dynamic=disable`:

* **ConfigSave check**: `mssql-apply-check --database X` on the staged rows, before the apply;
* **Tree vs database**: `mssql-apply-check --database X --tree <edited tree>` before anything is staged;
* **Two trees**: `apply-check-trees` on the base tree and the edited one (the changed files only).

Several modifications are staged together when they touch objects of their own (a batch): the native log names
the objects it acts on, so each modification is judged on its own object. **Match** compares the ConfigSave check
with the platform: `agree` (both restructure or both do not), `over` (we say restructuring, the platform did
not: costs a native apply), `UNSAFE` (we say none, the platform restructured: must not happen).

**Harmless on the platform's side (it named no object): 21**

| # | Modification | ConfigSave check | Tree vs database | Native platform | Match |
|---|---|---|---|---|---|
| 1 | [8.5] synonym of Catalog._ДемоКассы | harmless | harmless | nothing named | agree |
| 2 | EventSubscription _ДемоАвтономнаяРаботаРегистрация: Event: BeforeWrite -> OnWrite | harmless | harmless | nothing named | agree |
| 3 | Configuration Version -> 3.1.11.467 | harmless | harmless | nothing named | agree |
| 4 | StandardAttribute Description FillChecking: ShowError -> DontCheck | harmless | harmless | nothing named | agree |
| 5 | a comment line appended to CommonModule._ДемоЗаметки | no change seen | no change seen | nothing named | agree |
| 6 | WindowOpeningMode of Catalog._ДемоГруппыДоступаНоменклатуры.Form.ФормаЭлемента: LockOwnerWindow… | no change seen | no change seen | nothing named | agree |
| 7 | a title in the composition schema of CommonTemplate.ДанныеПечатиРегистрСимволов | no change seen | no change seen | nothing named | agree |
| 8 | synonym of Catalog._ДемоКассы | harmless | harmless | nothing named | agree |
| 9 | a paragraph added to the help of Catalog._ДемоКассы | no change seen | no change seen | nothing named | agree |
| 10 | the first right of Role._ДемоБазовыеПраваБСП: true -> false | no change seen | no change seen (bodies not compared) | nothing named | agree |
| 11 | Attribute of Catalogs _ДемоВидыНоменклатуры: Comment set | harmless | harmless | nothing named | agree |
| 12 | TabularSection СчетаНаОплату of _ДемоЗаказПокупателя: Comment set | harmless | harmless | nothing named | agree |
| 13 | Command of Catalogs _ДемоБанковскиеСчета: Synonym Демо: Банковские счета -> Демо: Банковские сч… | harmless | harmless | nothing named | agree |
| 14 | Catalogs _ДемоДоговорыКонтрагентов: Command КомандаRcheck added (copy of _ДемоДоговорыКонтраген… | harmless | harmless | nothing named | agree |
| 15 | a new form ФормаRcheck of Catalog._ДемоГруппыДоступаПартнеров (a copy of the item form of _Демо… | harmless | harmless | nothing named | agree |
| 16 | a new CommonModule МодульRcheck (a copy of _ДемоЗаметки) | harmless | harmless | nothing named | agree |
| 17 | Catalog._ДемоКлючиАналитикиНоменклатуры: Attribute Номенклатура Name: Номенклатура -> Переимено… | harmless | harmless | nothing named | agree |
| 18 | an attribute added to Report._ДемоФайлы | harmless | harmless | nothing named | agree |
| 19 | CommonModule _ДемоСвойства removed (and its item in the content of _ДемоСвойства.xml) | harmless | harmless (3 changes seen) | nothing named | agree |
| 20 | the first item of the content of ExchangePlan._ДемоАвтономнаяРабота: AutoRecord Allow -> Deny | harmless | no change seen (bodies not compared) | nothing named | agree |
| 21 | [second run] a comment line appended to CommonModule._ДемоЗаметки | no change seen | no change seen (bodies not compared) | nothing named | agree |

**The platform restructured or rewrote data: 13**

| # | Modification | ConfigSave check | Tree vs database | Native platform | Match |
|---|---|---|---|---|---|
| 1 | [8.5] Catalog._ДемоКонтрагенты: CodeLength 9 -> 12 | structure: Catalog._ДемоКонтрагенты Properties/CodeLength | structure: Catalog._ДемоКонтрагенты Properties/CodeLength | changed: Справочник._ДемоКонтрагенты; restructured: Справочник._ДемоКонтрагенты | agree |
| 2 | Catalog._ДемоДоговорыКонтрагентов: Attribute НомерДоговора Indexing: DontIndex -> Index | structure: Catalog._ДемоДоговорыКонтрагентов ChildObjects/Attribute[НомерДоговора]/… | structure: Catalog._ДемоДоговорыКонтрагентов ChildObjects/Attribute[НомерДоговора]/… | changed: Справочник._ДемоДоговорыКонтрагентов; restructured: Справочник._ДемоДогово… | agree |
| 3 | Catalog._ДемоКонтрагенты: CodeLength 9 -> 12 | structure: Catalog._ДемоКонтрагенты Properties/CodeLength | structure: Catalog._ДемоКонтрагенты Properties/CodeLength | changed: Справочник._ДемоКонтрагенты; restructured: Справочник._ДемоКонтрагенты | agree |
| 4 | Catalogs _ДемоКлючиАналитикиНоменклатуры: Attribute Комментарий removed | structure: Catalog._ДемоКлючиАналитикиНоменклатуры ChildObjects/Attribute[Комментар… | structure: Catalog._ДемоКлючиАналитикиНоменклатуры ChildObjects/Attribute[Комментар… | changed: Справочник._ДемоКлючиАналитикиНоменклатуры; restructured: Справочник._Демо… | agree |
| 5 | Document.СообщениеSMS: CheckUnique true -> false | structure: Document.СообщениеSMS Properties/CheckUnique | structure: Document.СообщениеSMS Properties/CheckUnique | changed: Документ.СообщениеSMS; restructured: Документ.СообщениеSMS | agree |
| 6 | a new Catalog СправочникRcheck (a copy of _ДемоКассы) | structure: Catalog.СправочникRcheck | structure: Catalog.СправочникRcheck | new: Справочник.СправочникRcheck | agree |
| 7 | an attribute (String 20) added to Catalog._ДемоКассы | structure: Catalog._ДемоКассы ChildObjects/Attribute[РеквизитRcheck] | structure: Catalog._ДемоКассы ChildObjects/Attribute[РеквизитRcheck] | changed: Справочник._ДемоКассы; restructured: Справочник._ДемоКассы | agree |
| 8 | the first string attribute of Catalog._ДемоФизическиеЛица: length 100 -> 120 | structure: Catalog._ДемоФизическиеЛица ChildObjects/Attribute[МестоРождения]/Proper… | structure: Catalog._ДемоФизическиеЛица ChildObjects/Attribute[МестоРождения]/Proper… | changed: Справочник._ДемоФизическиеЛица; restructured: Справочник._ДемоФизическиеЛи… | agree |
| 9 | Documents _ДемоОприходованиеТоваров: tabular section ТСRcheck added (copied from _ДемоЗаказПоку… | structure: Document._ДемоОприходованиеТоваров ChildObjects/TabularSection[ТСRcheck] | structure: Document._ДемоОприходованиеТоваров ChildObjects/TabularSection[ТСRcheck] | changed: Документ._ДемоОприходованиеТоваров; restructured: Документ._ДемоОприходова… | agree |
| 10 | AccumulationRegisters _ДемоОстаткиТоваровВМестахХранения: Dimension ИзмерениеRcheck added (copy… | structure: AccumulationRegister._ДемоОстаткиТоваровВМестахХранения ChildObjects/Dim… | structure: AccumulationRegister._ДемоОстаткиТоваровВМестахХранения ChildObjects/Dim… | changed: РегистрНакопления._ДемоОстаткиТоваровВМестахХранения; restructured: Регист… | agree |
| 11 | ScheduledJob ЗагрузкаКурсовВалют: Use: false -> true | data: ScheduledJob.ЗагрузкаКурсовВалют Properties/Use | data: ScheduledJob.ЗагрузкаКурсовВалют Properties/Use | changed: РегламентноеЗадание.ЗагрузкаКурсовВалют | agree |
| 12 | a predefined item added to Catalog.ГруппыПользователей | data: Catalog.ГруппыПользователей Predefined | no change seen | changed: Справочник.ГруппыПользователей; restructured: Справочник.ГруппыПользовател… | agree |
| 13 | a value added to Enum._ДемоПолФизическогоЛица | data: Enum._ДемоПолФизическогоЛица ChildObjects/EnumValue[НеУказан] | data: Enum._ДемоПолФизическогоЛица ChildObjects/EnumValue[НеУказан] | changed: Перечисление._ДемоПолФизическогоЛица; restructured: Перечисление._ДемоПолФ… | agree |

**Controls: the unchanged tree of a pristine clone, staged base-free: 2**

| Database | ConfigSave check | Native platform | Match |
|---|---|---|---|
| b2 | data: BusinessProcess.Задание Flowchart | changed: БизнесПроцесс.Задание | agree |
| b3 | data: BusinessProcess.Задание Flowchart | changed: БизнесПроцесс.Задание | agree |


Result: **34 modifications of 12 kinds of object, and the ConfigSave check agrees with the platform on all 34**:
13 were restructured or rewrote data and the check said so, 21 were not and the check said so. The check never said
"no restructuring" where the platform restructured (0 `UNSAFE`). The check of the tree against the database, run
before anything was staged, gave the same answer for every descriptor change; for the changes that are not in a
descriptor (a module, a form, help, rights, a template, the data bodies) it has nothing to compare and says so
(`bodies not compared`).

Things to read in the table:

* **The controls.** The very first apply of a base-free stage on a pristine clone always restructures the route
  points of the business process `Задание`, even when the tree is unchanged: the flowchart row the stage compiles
  differs from the stored one in a counter (finding 5), and the check says exactly that (`data:
  BusinessProcess.Задание Flowchart`, no other reason). Each modification is judged on its own object; the
  controls show what the first stage of a clone adds to any run. The runs of `b1` and `b2` that came first also
  carry six exchange plan content rows written in another order, which the first build of the check took for
  changes and the next ones for what they are (notes).
* **One false alarm, then a rule.** Flipping `AutoRecord` of one item of the content of the exchange plan
  `_ДемоАвтономнаяРабота` was first reported as `structure` (`over`) and the platform named nothing. The rule was
  refined (a flag on the same objects is harmless, an object that joins or leaves is not) and the same
  modification (Deny -> Allow first, Allow -> Deny the second time) agrees. The flag rule rests on that one
  observation of one plan.
* **Body-only changes** (the module, the form layout, the template, the help, the rights of a role) are counted
  by role in the ConfigSave check and are not descriptors; the platform named none of them, and the whole
  base-free stage carries 4 900 rows of these roles that it promotes every time.
* **Rights of a role** cannot be staged by the base-free import (its rights writer refuses the row, "the written
  row reads back into a different Rights.xml"): that change was staged as a partial stage of the role (`--path-prefix`).
* **8.5** (the two `[8.5]` rows): XML 2.21, the native 8.5 platform, the administrator `Администратор (обычное
  приложение)`. The check says `structure` for the code length with exactly one reason and the platform
  restructures the catalog; the synonym is harmless on both sides. The first apply of that clone also rebuilt the
  route points of `Задание` and the stage differed from the stored root and Configuration rows (finding 10).
* **Runs lost and redone**, for the record: the controls of `b1`, the first `v19` and `bs3_safe` of `b3`
  (a trial stage I made overwrote the ConfigSave of a run that was waiting for the native lock; the apply then
  applied the trial, a module removal with its subsystem item, and changed `Config` and `Params` only; the base tree
  was put right and both were run again), and three runs killed by a 60-minute limit of my own harness while
  they waited for the lock (one of them left the lock held for 17 minutes; it was released by hand and the
  limit is 3 hours now).
* **The patch-mode import** is not in the table because it did not stage what it was given: the attribute of
  `v14` on a pristine clone, staged in patch mode, never reached the ConfigSave (finding 1), and the native apply
  of that stage changed `Config` and `Params` only. That run is the reason for `--tree`.
* **The native import** did not get through: three attempts on the unchanged native export of the БСП (with and
  without edits) ended with the predefined-item error of finding 11. The native-staged ConfigSave is therefore
  the ddl track's own (case a2, an attribute added to `Catalog._ДемоПартнеры`, restored as `nat_a2`): our check
  finds the attribute (`structure`) and the ddl track's native apply of the same stage rebuilt `_Reference20`, but
  it cannot clear the stage (finding 9).

### 6.4 The reference trees against themselves and against their databases

| tree | files | rows in `Config` | `apply-check-trees` old = new | `mssql-apply-check --tree` against the database restored from the same corpus |
|---|---|---|---|---|
| БСП 8.3.27 (XML 2.20) | 12 197 | 9 838 | no restructuring, 0 reasons, 5 s | no restructuring, 0 reasons, 0 objects, 11 s |
| БСП 8.5 (XML 2.21) | 12 336 | 9 935 | no restructuring, 0 reasons, 6 s | no restructuring, 0 reasons, 0 objects, 14 s |
| ERP УХ 8.3.27 (XML 2.20) | 140 708 | 118 170 | no restructuring, 0 reasons, 54 s | no restructuring, 0 reasons, 0 objects, 144 s |
| ERP УХ 8.5 (XML 2.21) | 140 708 | 118 170 | no restructuring, 0 reasons, 68 s | no restructuring, 0 reasons, 0 objects, 202 s |

Every descriptor of every corpus exports, with the metadata model, to exactly the bytes the native export wrote
(`descriptors_compared: 0`: nothing had to be parsed and diffed). The objects, the kinds and the dialect (2.20 and
2.21) all match: the model decodes the four configurations completely, and a tree that is an export of its database
gives no reason. `cargo test -p ibcmd-apply-check apply_check::corpus_tests -- --ignored`
repeats it (the trees against themselves; a sample of every seventh metadata file rewritten by the model's writer
and compared with the original, to make the check parse and diff real files; the БСП export against its database
when `IBCMD_RS_APPLY_CHECK_DB` names one).

## 7. Known gaps

1. **The whitelist is as wide as the evidence.** Kinds and properties nobody probed are `structure`: for
   `SettingsStorage`, `Sequence`, `DefinedType`, `SessionParameter`, `Language`, `CommonAttribute` and
   `DocumentNumerator` only the presentation properties of 4.2 are on the list, and the objects of the БСП are a
   sample: a kind that the БСП does not use was never seen. This costs an unnecessary native apply, never a
   corruption.
2. **Extensions.** The check reads `Config` and `ConfigSave`, the main configuration. The rows of the extensions
   (`ConfigCAS`, `ConfigCASSave`, `_ExtensionsRestruct*`) are not judged; an apply with an extension staged must
   refuse on its own account.
3. **Type conversion.** A change of type is always `structure`. Whether the platform can convert the data
   (the ddl track's case k: strings are truncated, numbers saturate) is for the apply that restructures.
4. **Row bodies that copy rows are counted, not compared.** The claim that applying a module, a form, a template,
   a picture, a help page, a set of rights or a command interface changes no table is supported by every run
   (`v01`-`v06`, and the 4 900 rows of these roles that every base-free stage of the БСП carries and the platform
   promotes each time), but it is a claim about the platform.
5. **Byte comparison of the data bodies.** `Predefined`, `Flowchart`, `Aggregates` and `AdditionalIndexes` are
   compared byte for byte (`Content` as a set). A stage that writes the same items in another encoding gives a
   false `data`/`structure` reason. Measured on the БСП: a base-free stage of the unchanged export reproduces all
   28 predefined bodies exactly, but not the flowchart of the business process `Задание` (a counter, 37 stored,
   23 compiled), and the platform does rebuild that process's route-point table for it, so the reason is right.
6. **Both sides go through one model.** A property the metadata model does not decode cannot be seen changing; a
   row that differs while both sides decode to the same tree is reported as `unknown` (fail closed) unless every
   difference is a known record-format upgrade (3.6), which also catches the non-decoded case. A row that has a
   decoded change is not searched for further undecoded ones.
7. **Sessions.** Nothing here says how a change can be applied to an infobase that has sessions. The check is
   about the content of the change.
8. **Tree against database compares descriptors only**, see section 2; the data-carrying bodies of a tree are
   counted in `stats.body_files_not_compared` (28 in the БСП, 271 in ERP УХ).
9. **`DBSchema` is not consulted** (section 3.5): a property the whitelist does not know is `structure` even when
   the platform would find every table entry unchanged.
10. **Native-staged (lifted in #404) and 8.5 base-free ConfigSave.** The ConfigSave of a native import of the
    corpus БСП is judged (3.6): its own change, and the record-format noise proven harmless by the table of
    defaults seen in one import (8.3.27.2214 on the БСП). Another platform edition or another configuration may
    write a default the table does not have; such a row is refused by name, never guessed. The base-free stage of
    the 8.5 БСП is still refused as it is (finding 10 of section 8).
11. **Speed.** ConfigSave check on the БСП: about 5 s; on ERP УХ (118 000 rows) the whole gate of the S1 apply (the check
    and the S1 plan together) took 56 s in a release build and 327 s in a debug build (`s1-acceptance-uha.md`). Tree against database: 11 s (БСП 8.3.27), 14 s (БСП 8.5), 144 s (УХ 8.3.27), 202 s (УХ 8.5), of
    which the model's export of every descriptor is most.

## 8. Findings other tracks need

1. **A ConfigSave alone cannot tell that a stage dropped a change** (import track, #388; ddl track finding 10.1,
   reproduced): on a pristine БСП clone the patch-mode import of a tree with an attribute added to
   `Catalog._ДемоКассы` staged 9 517 rows and not one changed descriptor (0 compared: all byte-equal to the active
   ones); the ConfigSave check said `no restructuring`, the native apply then changed `Config` and `Params` only,
   and `mssql-apply-check --tree` on the same tree and database said `structure: Catalog._ДемоКассы
   ChildObjects/Attribute[РеквизитRcheck] added` (`runs_p1/v14_attribute_added`). A stage must be checked against its input: `check_tree_against_db` before it,
   or `--base-free`, which stages every row from the tree. The `objects` list of the verdict is the API for it: which
   descriptors differ, their uuid, kind and Config row, whether each difference is structural.
2. **`--dynamic=force` writes an alias of every staged row and does not refuse structure** (apply track, #337):
   section 5. A full ConfigSave doubles `Config` at each apply (127 892 rows, 118 050 of them aliases, after 13
   applies of one lab clone).
3. **Our exporter fails on such a database** (export track): `ibcmd-rs infobase config export` on the clone above
   ends with SQL Server error 8621 ("the query processor ran out of stack space during query optimization") while
   reading `Config` through the derived table that publishes the aliased rows under their published names (the
   export's `dynamic_generation` overlay, 118 050 aliases here). The check reads rows in batches of 200 names and is
   not affected.
4. **A patch-mode stage of a dynamically updated database reads the plain rows** (import track): on `a`, a clone after 16 dynamic applies, the patch-mode import of its
   own unchanged tree staged 9 516 rows, and the ConfigSave check found 149 of their descriptors different from the
   active ones (40 reasons: `Catalog._ДемоКонтрагенты Properties/CodeLength 12 -> 9`, `ScheduledJob…
   RestartIntervalOnFailure 601 -> 10`, ...). The stage carries the *plain* rows, the configuration as it was before
   the online updates; applied, it would roll them back (`runs_demo/v00_control`).
5. **The base-free stage of an unchanged БСП is not neutral** (import track): the flowchart row of the business
   process `Задание` differs from the stored one in a counter (37 stored, 23 compiled) and the platform rebuilds the
   route-point table for it at the first apply; the six exchange plans whose content row it writes in another
   order (a set, no consequence, and the check compares it as one); 3 multi-part rows are missing against the native
   stage (ddl track, 10.1).
6. **Everything native that writes takes the lab's `native` lock** (coordinator, 11:30): the harness holds it around
   each native `apply` and `import`; the row count of the ConfigSave after a native import must be checked (a full
   stage of this configuration is 9 842 rows; a partial one made the next apply fail at once in the ddl track).
7. **The removal of an object rebuilds more than the object.** The native apply that dropped a catalog restructured
   29 other objects (change-registration tables of the exchange plans that register them): an own apply that
   handles a removal will meet this.
8. **The vocabulary of the native apply log** (trace and apply tracks): `Объект изменен: X`, `Новый объект: X`,
   `Объект удален: X`, `Регистрация изменена: X`, `Создана таблица: T`, `Изменена структура таблиц базы данных`,
   `Обработка данных Реструктуризация X [пересчет итогов | таблица регистрации изменений] [n]`,
   `Создано поколение конфигурации: <uuid>`. The first four are printed only for objects that own a table or
   stored data: a new common module or role prints nothing.
9. **A native import rewrites the stage in the record format of its own edition** (import and model tracks). The
   БСП of the corpus is stored in record version 56 (compatibility 8.3.24); the 8.3.27 import writes 57: in the
   ddl track's case a2 (`ibcmd_rs_04_ddl_bsp8327_a_a2_staged.bak`, restored as `ibcmd_rs_04_rcheck_nat_a2`) 517 of
   4 929 descriptors differ in bytes from the stored ones (one is the change of the case; 119 decode to the same
   descriptor in the other format, 396 to the same XML in the same one), and the Configuration row is rewritten from
   the `{67}` to the `{68}` shape with the compatibility mode at 80324 and the extension compatibility at 80327.
   The model refuses that Configuration row ("fields 26 and 43 hold 80324 and 80327; no corpus shows which one the
   platform prints"). The check reads a staged row in either format and does find the change of that case
   (`structure: Catalog._ДемоПартнеры ChildObjects/Attribute[ДемоНовыйРеквизит] added`), but the Configuration row,
   the 396 byte-only differences and a row `deleted` (content `0`: the native way to list deleted files, which the
   check does not know yet) stay `unknown`, so the ConfigSave of a native import of this corpus is refused.
   Two things would lift that: the model decoding a `{68}` Configuration row whose fields 26 and 43 differ, and a
   way to tell a format upgrade from a field the model does not carry. **Lifted in #404** by 3.6 (the check
   reads the `{68}` row for the comparison, proves the 516 rows, and reads the `deleted` row `0`); the source tree
   remains the reliable input for a stage the check cannot prove.
10. **The 8.5 base-free stage of an unchanged БСП is not neutral either** (import track): the `root` row, the
    Configuration row (139 204 -> 139 197 bytes), `Task.ЗадачаИсполнителя` (same size) and a 99 MB body row of the
    configuration differ from the stored ones, and the flowchart of `Задание` as on 8.3.27; the check reports
    them (`unknown`, fail closed) and the platform's apply rebuilt the route-point table of `Задание` only. The
    8.5 БСП clones keep the administrator under the name `Администратор (обычное приложение)` with an empty
    password (found by the ui track): `--user=Администратор` is refused there. **The `root` row is the platform's
    stamp** (s185): a native `import files --partial` of one unchanged file rewrites the final 16-byte block of the row's
    128-byte payload, and does so again on every later import; the check now reads it as unchanged (`root_row.rs`), so
    the platform's own stage of a real change on 8.5 reaches the S1 classification.
11. **Native `config import` is flaky on a busy machine** (import and ddl tracks): twice in a row the import of the
    unchanged native export of the БСП ended with "Ссылка на неизвестный предопределенный элемент -
    ChartOfCharacteristicTypes.ОбъектыАдресацииЗадач.ВсеОбъектыАдресации" while other native runs were queued
    (lane `n1`, 14:20-14:50); the tree is the very export the corpus came from.

## 9. Evidence files and how to reproduce

In the repository, `docs/apply/evidence/restructuring-check/`:

| file | what |
|---|---|
| `validation.md`, `validation.json` | section 6.3, one line per modification with the three verdicts and the native outcome |
| `probes-auto-a.tsv` | the 159 probes of 6.1: kind, object, property, from, to, whether the platform acted |
| `force-applies.md` | the native `--dynamic=force` applies of 5.1 and 6.2: exit, seconds, what changed, the objects named |
| `selftest.md` | section 6.4: the four reference exports against databases restored from the same corpora |
| `s1-matrix.md` | section 10.4: every case of the probe runs through the check and the S1 classification, one line each |
| `nat-a2-image.md` | section 3.6 and 10.3: the whole native image of the ddl track's case a2 before and after (`--s1` output, counts) |

The lab is `F:\ibcmd\lab\04\restructure-check` (its scripts are not part of the product):

* databases (`ibcmd_rs_04_rcheck_...`): `bsp_a` (cumulative, 16 dynamic generations; 6.1, 6.2), `bsp_b1`,
  `bsp_b2`, `bsp_b3` (the validation lanes), `bsp_p1` (the patch-mode demonstration, a second control), `bsp85`
  (the 8.5 dialect), `bsp_n1` (the lane of the native import; the import never got through), `nat_a2` (the ddl
  track's native-staged case a2), `uha8327`, `uha85` (the self-tests of 6.4);
* `tools/snapshot.ps1` digests the schema before and after an apply (the columns of every table, an index
  signature by columns, the object ids of the tables, row counts); `tools/native.ps1` runs the native `ibcmd`
  with a timeout and closed stdin, holds the lab's `native` lock for that one command and removes its 800 MB data
  folder afterwards; `py/exp.py run <name>` is one experiment (edit the base tree, tree against database, stage,
  ConfigSave check, native apply, digests, the result in `runs_*/<name>/result.json`); `py/lanes.py` runs the
  lanes; `py/edits*.py` are the edits; `py/make_validation.py` and `py/make_evidence.py` write the tables;
* one check by hand:

```
ibcmd-rs mssql-apply-check --database ibcmd_rs_04_rcheck_bsp_b2 --json
ibcmd-rs mssql-apply-check --database ibcmd_rs_04_rcheck_bsp_b2 --tree F:\path\to\export --json
ibcmd-rs apply-check-trees --old F:\path\to\export --new F:\path\to\edited --json
```

Tests: `cargo test -p ibcmd-apply-check` (129 tests: every rule with synthetic XML, the
row check with fake rows, the modes on synthetic trees, the upgrade proof, the S1 classification and its matrix; 6 more
are the ignored lab-corpus tests below); `... apply_check::corpus_tests -- --ignored` for the reference exports (6.4); `... apply_check::s1h -- --ignored`
for the a2 image (3.6; the rows are the lab's `fixtures/nat_a2` and `fixtures/nat_a2_base`).

## 10. What the own restructuring takes of a verdict (S1, #404)

The own restructuring (#391, S1) does a few restructurings itself and hands everything else to the platform: on
catalogs and documents, add or delete an attribute, widen a variable string, switch the index of an attribute, add a
tabular section, add a plain object. The gate that decides needs the check's answer as **data**: which operation, on
which object, and if none, why not. It used to be text ("the property path and the first word of the change").

### 10.1 The types

* `Reason` (section 2) has four typed fields beside the text: `rule: RuleId`, `kind: String`, `path: Vec<Seg>` (the
  element names from the object down, and a child's name in `label`), `op: Option<ChangeOp>` (`Added`, `Removed`,
  `Reordered`, `Modified { old, new }`). `Reason::default()` is class `unknown`, rule `unspecified`: a literal that
  predates the fields is refused, never taken. `Reason::step(class, rule, object, file, property, change)` builds the
  reason of a step of the check.
* `apply_check::s1::classify(&Verdict) -> Classification`:

```rust
pub struct Classification { pub operations: Vec<S1Operation>, pub refusals: Vec<Refusal> }
pub enum S1Operation {                    // every variant carries `object: ObjectId { kind, name, row }`
    AddAttribute { attribute },  DeleteAttribute { attribute },
    WidenString { attribute, from: u32, to: u32 },
    SwitchIndex { attribute, from: IndexMode, to: IndexMode },   // DontIndex <-> Index or IndexWithAdditionalOrder
    AddTabularSection { section },  AddObject,                   // a catalog or a document
    AddSectionAttribute { section, attribute },                  // a new attribute in a section the object had
}
pub struct Refusal { pub code: RefusalCode, pub rule: RuleId, pub reason: String, pub detail: String }
```

`row` of an `ObjectId` is the Config row of the object's descriptor (its uuid) as the reason gave it, for a new object
the row of the new descriptor, not the configuration's; `by_object()` groups the operations by it.
`Classification::accepted()` is "no refusal" (a stage with no operations is accepted too: the caller has its own
plain path for it); `refusal_message()` is the text of the first four; `--s1` prints it. Each reason is exactly one of
an operation or a refusal, except the two reasons of a new object, which are one operation together (10.2).

### 10.2 Reason to operation or refusal

The rows are the whole table: what is not here is refused (`structure-outside-s1`, `rule-unspecified`, ...). The
match is on `rule`, `kind`, the steps of `path` and `op`, never on words.

| reason (rule; kind; path; op) | S1 |
|---|---|
| `column-added-or-dropped`; Catalog, Document; `ChildObjects/Attribute[A]`; added | `AddAttribute` |
| the same, removed | `DeleteAttribute` |
| `attribute-property-not-covered`; ...; `ChildObjects/Attribute[A]/Properties/Type/StringQualifiers/Length`; modified `n -> m`, both numbers, `m > n` | `WidenString` |
| the same, `m <= n` or not a number | refused `length-not-widened` |
| `attribute-property-not-covered`; ...; `.../Properties/Indexing`; `DontIndex <-> Index` or `DontIndex <-> IndexWithAdditionalOrder` | `SwitchIndex` |
| the same between `Index` and `IndexWithAdditionalOrder` (not traced), or to or from any other value | refused `index-mode-outside-s1` |
| `tabular-section-added-dropped-moved`; ...; `ChildObjects/TabularSection[T]`; added | `AddTabularSection` |
| `tabular-section-column-added-dropped-moved`; ...; `ChildObjects/TabularSection[T]/ChildObjects/Attribute[A]`; added | `AddSectionAttribute` |
| the same, dropped or moved; the section dropped or moved; any other change inside a section | refused `tabular-section-outside-s1` |
| `object-with-storage-added-or-dropped`; Catalog, Document; no path; added, **and** the same rule on `Configuration`, `ChildObjects/<Kind>[Name]`, added, of the same kind and name | `AddObject` (one operation for the two reasons) |
| the one without the other | refused `object-list-mismatch` |
| the same rule, dropped | refused `object-removed` |
| `property-not-covered`, `properties-changed`, `standard-attribute-property-not-covered` (`CodeLength`, `HierarchyType`, ...) | refused `property-outside-s1` |
| any other property of an attribute (`Type/Type`, `Use`, `DataHistory`), a part of an attribute | refused `attribute-property-outside-s1` |
| attributes reordered | refused `attribute-moved` |
| `InternalInfo`, a child kind no rule covers, the object itself | refused `child-outside-s1` |
| any other kind (registers, charts, enumerations, ...); the configuration row other than a listing | refused `kind-outside-s1`, `configuration-outside-s1` |
| a name with a dot after the kind (`Catalog.X.Form.F`) | refused `nested-object-outside-s1` |
| class `data` (predefined items, enumeration values, scheduled jobs, route points) | refused `data-change` |
| class `unknown`: `row-decodes-to-same-xml`; `row-format-upgrade-unproven`; the rest | refused `row-unexplained`; `row-format-unproven`; `unknown-step` |
| `root` or `version` row changed (a `root` re-stamped in its final payload block on 8.5 is not a change); a body row that carries data | refused `service-row-changed`; `body-data-outside-s1` |
| a verdict with `incomplete` set (a tree against a database left data out) | refused `incomplete-verdict` |

`exactly_the_operations_of_s1_pass_and_nothing_else_does` holds `classify` to this table from outside: 62 rule ids x
10 kinds x 12 paths x 10 operations x 3 classes, 223 200 reasons one at a time, and exactly the 14 that the table
names (seven operations on two kinds) come out as operations.

An operation says what the descriptors changed by, not what a reason cannot see: that a string being widened is
variable-length (`AllowedLength` is another leaf), that a new object has no predefined items or hierarchy, that a
new attribute has a primitive type. The plan of the restructuring reads the same descriptors and refuses what it
does not build.

### 10.3 The whole native image

| | reasons | of which |
|---|---|---|
| before (section 8, finding 9) | 400 | 1 real, 396 `unknown` "the row differs but both sides decode to the same XML", 2 for the Configuration `{68}` row, 1 for `deleted` |
| after | 1 | `column-added-or-dropped`, `Catalog._ДемоПартнеры`, `ChildObjects/Attribute[ДемоНовыйРеквизит]`, added |

`--s1` says: `add-attribute: Catalog._ДемоПартнеры`. The 516 rows are counted in `stats.format_upgrades` and are
neither reasons nor notes; the Configuration row goes through the fold of 3.6; `deleted` is `0`. A row outside the
table is refused by name and the first deviation (`row-format-upgrade-unproven`, `at 5/2/3: ...`), so the stage is
too. `apply` still refuses **any** `deleted` row before it asks the gate (ddl track, 12.2.3): to take a whole native
image it has to let the row `0` through, and refuse a non-empty one, as the check does.

### 10.4 The refusal matrix

`s1_matrix_tests` sends every case of the probe runs of 6.1 and 6.2 through the check's own comparison
(`descriptor::compare` on synthetic descriptors, the same code the rows and trees use) and `s1::classify`. The runs
are `p1`, `p2`, `p4`, `p5`, `p6`, `p7a`, `p7b`, `p8` (there is no `p3` and no `p9`-`p12` in the record; the probe
table of 6.1 is the 159 further cases). Asserted for every case: what the platform acted on is never harmless,
each case is what the table below says, and an operation is never given to a change the platform did not act on.

| run | cases | platform acted on | harmless | S1 operations | refused |
|---|---|---|---|---|---|
| `p1` catalog properties | 18 | 3 | 15 | 0 | 3 (`property-outside-s1`) |
| `p2` attribute properties | 16 | 2 | 13 | 2 (`switch-index`, `widen-string`) | 1 (the attribute's `DataHistory`, over-refused) |
| `p4` children | 10 | 3 | 6 | 1 (`switch-index`) | 3 (`Use`; the enumeration value, `data-change`; a standard attribute's `DataHistory`, over-refused) |
| `p5` children | 9 | 4 | 5 | 3 (`add-tabular-section`, `add-section-attribute`, `delete-attribute`) | 1 (`kind-outside-s1`) |
| `p6` configuration, jobs, comments | 19 | 3 | 16 | 0 | 3 (`data-change`, the scheduled jobs) |
| `p7a` add a module, a role, a catalog | 3 | 1 | 2 | 1 (`add-object`) | 0 |
| `p7b` drop them | 3 | 1 | 2 | 0 | 1 (`object-removed`) |
| `p8` presentation | 9 | 0 | 9 | 0 | 0 |
| S1's own two (an attribute added to a catalog, to a document) | 2 | 2 | 0 | 2 (`add-attribute`) | 0 |
| **focused** | **89** | **19** | **68** | **9** | **12** |
| the 159 probes of 6.1 | 159 | 20 | 128 | 0 | 31: the 20 the platform acted on, and 11 it left alone |

The 11 over-refusals are the price of failing closed, and they are the properties the platform applied without
acting that the rules do not list: `HierarchyType` and `LimitLevelCount` of a catalog (`property-outside-s1`),
`ExtendedEdit`, `FillFromFillingValue`, `MarkNegatives`, `MultiLine`, `PasswordMode` of a common attribute,
`DistributedInfoBase` of an exchange plan, `EnableTotalsSliceFirst`, `EnableTotalsSliceLast`, `MainFilterOnPeriod`
of an information register (all `kind-outside-s1`). None of them is an operation of S1 and none is let through; the
number is pinned in the test and only goes down when a property is probed and listed. `s1-matrix.md` in the evidence
folder has every case.

### 10.5 What the gate should consume (proposal for `src/restructure/s1.rs`)

Replace `classify_reason(&Reason)`, which matches `reason.object.split_once('.')`, `reason.property.split('/')` and
the first word of `reason.change`, by the verdict-level call:

```rust
use crate::apply_check::s1::{self, S1Operation};

let class = s1::classify(&verdict);                 // verdict: apply_check::check_staged(...)
if let Some(why) = class.refusal_message() {        // "code: reason [detail]; ..."
    return blockers(format!("S1: {why}"));
}
for operation in &class.operations {
    match operation {
        S1Operation::AddAttribute { object, attribute } => /* built */,
        other => /* NotBuilt { operation: other.name(), subject: other.object().full_name() } */,
    }
}
```

`Operation::AddAttribute(String)` of the gate is `S1Operation::AddAttribute { object, attribute }`; `NotBuilt` is any
other variant; the gate's `ObjectOperations { object, row, operations }` is `class.by_object()` (`object.full_name()`,
`object.row`, the operations). Two things change in the tests: a `Reason` literal now has nine fields (`..Reason::default()` keeps the
old five and makes the reason `unknown`/`unspecified`, which the gate refuses; build a real one with the fields of 10.2
or run the check on a tree), and the agreement of the two decoders (12.2.2) can compare `operations[i].object()` and
the attribute name with the plan's, without parsing anything.

### 10.6 Limits

* The first object of a kind in a configuration: the listing `ChildObjects/Catalog` of a configuration that has no
  other catalog is not a list of values, its item has no name in the path, and the new object is refused
  (`configuration-outside-s1`). A configuration that has catalogs is not affected.
* `WidenString` does not say `AllowedLength` (the plan must find it `Variable` on both sides) and `AddObject` does
  not say the object is plain.
* The proof of 3.6 is a table measured in one native import (8.3.27.2214, the БСП demo): another edition or another
  configuration can write a default the table lacks, and is then refused by name.
* `mssql-apply-check --s1` classifies the verdict of whichever mode it ran; a tree mode has no raw rows, so it
  cannot prove a format upgrade and does not need to (it compares XML).
