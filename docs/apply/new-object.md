# S1-F: a new catalog or document on the own apply (issue #402): design and the native write plan

Status (checkpoint 4): **built and twin-verified** on seven native cases (N1-N7). The planner creates a new catalog or
document (tables, names, schema entry, the eight cache rows, the registration), the S1 gate lets it through, and
`mssql-config-apply --allow-restructure s1` (and `ibcmd-rs infobase config apply --recovery-backup`) run it in the apply's
transaction. Evidence: `evidence/new-object/s1f-twin-compare.txt` (the twin protocol of `restructuring.md` 12.6 per case).
Sections 1 to 7 are the design of checkpoint 1; where a trace corrected it, the text says so (1.3, 3.3, 3.4) and section 8
has the results and what is left. Part of S1 (#391, `docs/apply/restructuring.md` 12). The caches of a new
object are done (S1-G, `docs/apply/derived-caches.md`); this is the rest: the tables, the names, the schema, the
registration and the wiring into the planner, the gate and the apply.

Evidence used (all measured on БСП 8.3.27): case c, a new catalog `ДемоНовыйСправочник` (native apply of a
`--base-free` stage, with an XE trace: `docs/apply/evidence/new-object/c-structure-statements.sql`), case d, a new
document with an attribute and a tabular section with an attribute (native apply of a native partial import;
snapshots, no trace), and the traces of the `ddl` track for a new tabular section of an existing object (e1, forwarded
by the coordinator): the platform creates a sub-table that has no old table with the same statements as a rebuilt one
minus the copy and the drop.

## 1. What the platform does for a new catalog

### 1.1 The writes, in order (case c, one apply)

| phase (`SchemaStorage.Status`) | statements for the new table `_Reference11036` | for a rebuilt table (compare) |
|---|---|---|
| structure, `200` | `create table dbo._Reference11036NG (...)` then `ALTER TABLE ... SET (LOCK_ESCALATION = DISABLE)`; the indexes in order: non-clustered ones by number (`_1NG`, `_2NG`, `_3NG`), the clustered `_S_HPKNG` last; then `UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200` | the same, then `INSERT ... SELECT` from the old table |
| `400` | nothing: **no copy, no drop** (the table has no old table) | `drop table dbo._<old>` |
| `500` | `sp_rename '_Reference11036NG' -> '_Reference11036'`, then the indexes `_Reference11036._Reference11036_1NG -> _Reference11036_1`, `_2`, `_3`, `_S_HPK` | the same |
| `100` | `UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = <empty>, NewGenDropped = <empty>`; `UPDATE DBSchema SET SerializedData = @P1` | the same |

The same shape holds for a sub-table that is new in an existing object (`ddl`'s e1: `create <t>NG`, its indexes, no
`INSERT`, no drop, renamed with the rest). The trace of case c also holds the platform's own work of that run, which is
**not** ours (drift list of 12.6): `_DbCopiesInfoBaseUse`, `_DbCopiesUpdateTableStat`, `_DbCopiesUpdateStat`,
`_WebSocketClients` (new system tables of the build), `_BPrPoints10` and `_ConfigChngR` (rebuilt), the three
`_ExtensionsRestructNGS` scratch rows.

### 1.2 The table entry

`Params.DBSchema` and `SchemaStorage.CurrentSchema` hold one entry per table; the new one stands **before
`ConfigChngR`**, after the tables rebuilt earlier (12.4). For case c, a flat catalog without attributes
(`evidence/new-object/c-dbschema-entry-Reference11036.txt`):

```
{"Reference11036","N",11036,"",
 {7, ID(R Reference11036,2) Version(V) Marked(L) PredefinedID(B,16) Code(S var 9) Description(S var 25) Fld2683(N 7,0)},
 {0},                                                        -- sub-tables
 {3, ByPredefinedIDNotUniq{1,"PredefinedID"}, Code{2,"Code","ID"}, Descr{2,"Description","ID"}},   -- declared indexes
 1,"R", {1,{{1,"Fld2683"}}}, {1,{{1,"Fld2683"}}}, "",0,0}    -- data-separation lists
```

For a document with one attribute and one tabular section (case d, `d-dbschema-entry-Document11034.txt`): the fields
`ID Version Marked Date_Time Number Posted Fld<attr> Fld2683`, one sub-table `{"VT11036","I",0,"Document11034",
{LineNo11037 N(5,0), Fld11038 ...},{0},{0},1,"S",{0},{0},"",0,0}`, the indexes `ByDocNum{2,"Number","ID"}`,
`ByDocDate{3,"Date_Time","ID","Marked"}`. The SQL of the table (columns, the physical indexes `_S_HPK`, `_1`...) is
derived from the entry by the code that exists (`schema::physical_tables`, checked against the stored entries of the
whole БСП: `docs/apply/evidence/restructuring/dbschema-conformance-8327.txt`).

The rules that decide the entry (`docs/apply/restructuring.md` 5, checked on the БСП for the fields of all 139 catalogs
and documents; the index sets below counted on the same corpus, 282 main tables):

| part | rule |
|---|---|
| fields of a catalog | `ID`, `Version`, `Marked`, `PredefinedID`, [`OwnerID`], [`ParentID`, and `Folder` only for folders and items], [`Code` when `CodeLength > 0`, `S` variable or fixed / `N` by `CodeType`], [`Description` when `DescriptionLength > 0`], the attributes in metadata order, then the fields of the **common attributes that apply** in the configuration's order |
| fields of a document | `ID`, `Version`, `Marked`, `Date_Time`, [`NumberPrefix` when the number is periodic], `Number` (`S` or `N`), `Posted`, the attributes, the common attributes |
| declared indexes of a catalog | `ByPredefinedIDNotUniq`, then by the properties: `OwnerCode`, `OwnerDescr`, `ParentCode`, `ParentDescr`, `Code`, `Descr` (11 index sets in 114 catalogs), then per indexed attribute `ByOwnerFieldFld<n>`, `ByParentFieldFld<n>`, `ByFieldFld<n>` |
| declared indexes of a document | `ByDocNumPrefix` (periodic number), `ByDocNum`, `ByDocDate`, then `ByFieldFld<n>` per indexed attribute |
| sub-table | `VT<n>`, `LineNo<m>` `N(5,0)`, the attributes as `Fld<k>`, no common attributes |
| data-separation lists | both `Fld2683` for 112 of 139 (the data separator applies), both empty for 24 catalogs, `Fld3454` / empty for 3: decided by the **content of the common attributes** |

**Which common attributes apply** (settled by the hint of the coordinator and a check on the БСП). The БСП has seven
common attributes and each is a service field of the tables *and* of the XDTO types: `ОбластьДанныхОсновныеДанные`
(`Fld2683`, the data separator), `ОбластьДанныхВспомогательныеДанные` (`Fld3454`), `НаименованиеЯзык1`, `НаименованиеЯзык2`,
`КомментарийЯзык1`, `КомментарийЯзык2`, `ОтредактированныеПредопределенныеРеквизиты`. The descriptor of a common attribute
is `{5,<body>,<Content>,<Indexing>,<FullTextSearch>,<DataSeparation>,<AutoUse>,...}` with `Content =
{3,<n>,<object uuid>,{2,<use>,<condition>},...}`, `use` 0 Auto, 1 Use, 2 DontUse, and `AutoUse` 0 Use, 1 DontUse
(`metadata_model::simple`). What the БСП has (`Content` size, `AutoUse`):

| common attribute | listed objects | `AutoUse` | so an object that is not listed |
|---|---|---|---|
| `ОбластьДанныхОсновныеДанные` | 347, all `DontUse` | **Use** | **has the field**, unless it is one of the 347 |
| `ОбластьДанныхВспомогательныеДанные` | 57, `Use` | DontUse | does not |
| `НаименованиеЯзык1` / `2` | 11 each, `Use` | DontUse | does not |
| `КомментарийЯзык1` / `2` | 3 each, `Use` | DontUse | does not |
| `ОтредактированныеПредопределенныеРеквизиты` | 17, `Use` | DontUse | does not |

Checked on the 114 catalogs: the service properties of a catalog's XDTO type are **exactly** the common attributes that list
it with `Use` (114 of 114); and the 112 data-separated objects are the 139 minus the 24 (`Fld2683` absent) and 3 (`Fld3454`
in the first list). A **new** object is in no `Content`, so it gets only the fields of the common attributes with
`AutoUse = Use`: in the БСП `Fld2683` alone (as in cases c and d) and **no** service property in the XDTO type. The 33
catalogs with service properties are the members of the other lists, and `derived-caches.md`'s "open risk" is closed for
this configuration. A stage that lists the new object in a common attribute changes that common attribute's descriptor
(the classification refuses it: another kind); a configuration where another common attribute has `AutoUse = Use` would
add columns and properties this design does not model: `common.rs` reports it and the new object is **refused**. So: build
`Fld2683`-like fields (the type is the common attribute's own pattern, the number its `Fld` entry in `DBNames`), refuse
everything else. The two lists (`{1,{{1,"Fld2683"}}}` twice) come from the same descriptors (`DataSeparation`,
`UsersSeparation`, `SeparatedDataUse`, ...) and are part of the corpus test.

What is not known yet, and is the first work of S1-F: the nullable / index rules for attributes of a **hierarchical**
catalog, and the rule of the two separation lists. The old-attribute code already maps an attribute's type to its field
(`catalog::type_entries`, primitive types, `ValueStorage`, `UUID`); references and composite types need the map from a
type id to a table name (`R`, `E` fields), which the `a07b62f0` row and `DBNames` hold.

### 1.3 `DBNames`

`Params.DBNames`: `{<header>,{<count>,{<uuid>,"<kind>",<number>},...}}`. The header is the last number handed out; the
numbers come from one counter shared with the extensions. What the platform numbers, in which order, is **traced on the
twins of N1-N7** (the native result of each stage, `scripts/apply-trace/lab/s1g-caches/twin_stage.ps1`):

| case | numbers of the stage (header last) |
|---|---|
| N1 a flat catalog, seven attributes | `Reference` 11041, `Fld` 11042-11048, header **11049** |
| N2 a hierarchical catalog, four attributes, a tabular section of three | `Reference` 11041, `Fld` 11042-11045, `VT` 11046, `LineNo` 11047, `Fld` 11048-11050, header **11051** |
| N4 a document, three attributes, a tabular section of one | `Document` 11041, `Fld` 11042-11044, `VT` 11045, `LineNo` 11046, `Fld` 11047, header **11047** |
| N5 a catalog and a document | `Reference` 11041, `Document` 11042, `Fld` 11043 (catalog), `Fld` 11044 (document), `VT` 11045, `LineNo` 11046, `Fld` 11047, header **11048** |
| N3 a new catalog listed **before** a changed catalog | `Reference` 11041, `Fld` 11042, *(11043: no entry)*, `Fld` 11044 (the changed catalog's new attribute), header **11044** |
| N6 a new catalog listed **after** a changed catalog | `Reference` 11041, `Fld` 11042, `Fld` 11043 (the changed catalog's new attribute), *(11044: no entry)*, header **11044** |
| N7 a new document beside a changed catalog | `Document` 11041, `Fld` 11042, `VT` 11043, `LineNo` 11044, `Fld` 11045, `Fld` 11046 (the changed catalog's new attribute), header **11046** |

Read:

1. **The created objects take their numbers first, all of them, in three passes**: the main tables of every created
   object (kinds in the configuration's order, catalogs then documents, and inside a kind in the order the staged
   configuration lists them), then the attributes of each, then the tabular sections of each with `VT`, `LineNo` and their
   attributes (`create::allocate`; N5 tells the passes from "object by object", N3/N6/N7 tell "created first" from "in
   the configuration's order").
2. **Then the platform walks the kinds and, inside a kind, the objects in the configuration's order** (`ddl` traced this for
   changed objects; `create::walk_positions` is the same order over the staged configuration). A **changed** object
   takes the numbers of its new attributes there. A **created catalog takes one number in this walk that no entry records**
   (a document does not: N4, N7): at the place of the catalog in the list, which is why N3 has the gap before the changed
   catalog's attribute and N6 after it, and why a stage of created objects alone ends with the header one above the last
   entry when it has a catalog (N1, N2, N5). `Running::reserve`.
3. The tables of the created objects are also **first in the `DBSchema` list**, before the rebuilt ones (N3, N6, N7).
4. A catalog without exchange-plan membership allocates only `Reference` (no `ChngR`, no `RefSInf`); a document only
   `Document`. On a base that has not had a native apply the platform's build also takes three numbers for its own new
   system tables (`DbCopies*`, after the object's): that is the upgrade of a restored base, not the object's; the twins are made
   on a base that already had one, and the three entries are tolerated otherwise (12.6 drift). `DBNamesVersion-DBNames`
   gets a new guid as always.

Two corrections to checkpoint 1, both found by twins, neither visible to a comparison of `DBNames` entries alone: case c
gave the order "changed attributes first, then the new catalog", but `c2` is the state after **three** native applies (a
catalog's attribute, a document's attribute, the new catalog) and says nothing of one stage; and the header of a stage with a
catalog is one above the last entry, which the first version of the tests hid by recomputing the platform's header from its
entries. Several new objects of one kind in one stage are refused (`create::check_count`): whether the extra number is
then taken once or once each is not traced.

### 1.4 The rows and their registration

The staged rows of the object (its descriptor `<uuid>`, and the body rows it has) are promoted like any staged row.
The registration for exchange plans (measured on the native result of case c): **three rows in `_ConfigChngR`**, one per
ordinary node of the exchange plans (`_MDObjID` = the uuid in the platform's byte order, `_MessageNo` NULL), and **no
`_ConfigChngR_ExtProps` row**, because this catalog has no body row (a body row would be listed as `(0, '<uuid>.0')`).
That is what the own apply already writes for a new form or template (`mssql_config_apply::objects`, `NewRegistration`),
so the apply's writer is reused for the created object; only the finding of the object differs (section 3.4).

### 1.5 The caches

Done: `caches::change::rewrite` gives the eight rows (`derived-caches.md` 7), equal to native's in cases c and d.

## 2. The plan of writes for a new catalog (what our transaction does)

Inside the own apply's transaction (`s1.rs` phase, one `StructurePhase`), after the assertions and before the move of
the staged rows:

1. **Assert** the table `_Reference<N>` and its sub-tables are absent; the schema and the names are those the plan read
   (their SHA-256, as for a rebuild).
2. **Numbers**: the counter from the header of the main `DBNames` and the extensions'; the created objects first (the
   three passes of 1.3), then the walk of the kinds and objects in the configuration's order, where a changed object takes
   its new attributes' numbers and a created catalog one number that no entry records. `DBNames` text gets the entries in
   number order, the header the last number handed out, `DBNamesVersion-DBNames` a new guid.
3. **Entry** of every table, built from the staged descriptor (section 3.2), inserted into the schema before
   `ConfigChngR`, the created objects' entries before the rebuilt ones.
4. **Structure phase**: `TablePlan::created` for the main table and each sub-table: `create ...NG`, its indexes, the
   marker (`Status 200`), no copy, no drop (`400`), the renames (`500`), the final update of `SchemaStorage`
   (`100`) and `DBSchema`.
5. **Caches**: `caches::change::rewrite` over the cache rows the changed objects' updates left; `siVersions` gets the new guids of all
   rewritten rows (`ParamsRewrite`).
6. **Registration**: `_ConfigChngR` rows for the ordinary nodes and the `_ConfigChngR_ExtProps` rows of the body files,
   by the apply's `NewRegistration`; `_MessageNo` reset as for any staged object.
7. **Postconditions** (asserted before commit, as for a rebuild): the new tables exist with the planned columns and
   indexes, and are empty; the names and the schema are the planned text.

The rehearsal, the recovery backup (12.4.4) and the rollback of a failure are the apply's, unchanged: the operation adds
statements to the script, it does not add a phase.

## 3. Design

### 3.1 Where the code goes

New files (so that the shared planner of `ddl` is touched in a few small places):

| file | content |
|---|---|
| `src/restructure/entry.rs` | `main_entry(kind, &NewObjectFacts, &Numbers, &CommonAttributes, &TypeMap) -> Result<Brace>`: the DBSchema entry of a new catalog or document with its sub-tables; `Numbers` = the numbers of section 1.3; `CommonAttributes` = which common attributes apply. Validated against the stored entries of the whole БСП |
| `src/restructure/common.rs` | the `CommonAttribute` descriptors: `AutoUse`, `Content` (`{2,use,cond}` per object), `DataSeparation`; `applies_to(object) -> bool` and the fields they add; a report of the common attributes other than the data separator that have `AutoUse = Use` (then the new object is refused) |
| `src/restructure/create.rs` | `plan_created(stored, running, &NewPrepared) -> Result<(ObjectPlan, Brace)>` (the new object's tables as `TablePlan::created`, the entry), `created_caches(...)` (the call of `caches::change::rewrite`) |

Shared with `ddl` (each a small separate commit, after their creation primitive is merged: `TablePlan::created`,
`Running::allocate`, `schema::subtable_entry`, `push_subtable`):

| hook | change |
|---|---|
| H1 `plan.rs::check_files` | a file that is new is accepted when the object it belongs to is a created object of `create.rs` |
| H2 `plan.rs::plan` | the created objects are numbered before the loop over the changed objects (`create::allocate`, the same `Running` counter; a created catalog's extra number is reserved in the loop, `Running::reserve`); their entries are planned after it and come first in the schema |
| H3 `plan.rs::ObjectPlan` | `created: bool`, and `changes()` says "new catalog X" |
| H4 `plan.rs` caches | the created objects' caches are computed **after** the attribute updates of `xdto_update` / `registry_update` and chained on their rows (inflate, `change::rewrite` with `changed` = the created objects, deflate); one `siVersions` update covers the union |
| H5 `s1.rs::decide` | `AddObject` (and `AddTabularSection`, ddl's) leaves the "designed, not built" list; the two decoders agree on the set of created objects (names, kinds) as they do on attributes |
| H6 `s1.rs::phase_of` / `gate::StructurePhase` | a list `created: Vec<(uuid, kind, body files)>` for the apply's registration and its report |

And one change in the apply (`mssql_config_apply`, owner: apply track): the rows of a created object are **answered for
by the structure phase** (like the planned objects' descriptors), and the object is registered like a new form:
`objects::analyze` takes `phase.created` as new objects of kind `Catalog` / `Document` instead of finding them.

### 3.2 The entry builder and its proof

`entry.rs` reproduces, from the staged descriptor and the numbers, an entry that is byte for byte what the platform
stored. Its test is offline and needs no native run: for **every catalog and document of the БСП** (139 stored
entries, sub-tables included) build the entry from the stored descriptor and the stored `DBNames` numbers and require
the text to equal the stored one. Objects the builder does not cover (owner-subordinated catalogs, defined types and
characteristics, predefined items, exchange-plan membership, register-like extras) must be refused **before** they are
attempted, by a list checked in the same test: covered + refused = 139. A new object only ever gets what the test proved.

### 3.3 Refusals (fail closed)

**As built** (`create::find_created`, `create::plan_created`, `create::check_count`, `entry::main_entry`, the S1 gate), refused with
the reason: forms, templates and commands of the object (`Members::others`); predefined items (`<uuid>.1c`); any file of the
object other than its object module, manager module and help page (by the source-asset routes `ObjectModule`,
`ManagerModule`, `Help`; the apply moves those rows unread); data history; an owner or a characteristic type the entry builder
cannot prove (31 of the 139 corpus objects; `tests_entry.rs` counts them by reason); a common attribute that applies to
every new object and is no data separator; a stage that changes a common attribute; a table the schema has already; more
than one new catalog or one new document in a stage (`check_count`: the header rule of 1.3 is traced for one catalog);
and, of the changed objects the same stage may hold, every object the S1 gate refuses anyway (subordinate catalogs,
objects an extension adopts: N3 first tried `_ДемоПартнеры` and `_ДемоКассы` and both were refused, which is the
refusal test). In the apply: a created object on the 8.5 profile, or on a change register that has no rows, goes to the
native apply (`NeedsNativeApply`); the registration of a created object is measured on 8.3.27 only.

### 3.4 How the pieces meet

The gate: `check_staged` names the new catalog by the pair of reasons that `apply_check::s1::classify` already turns into
`S1Operation::AddObject`. `decide` runs `plan`, which reads the created objects from the staged image: every
descriptor that is new in `ConfigSave` and has a listing in the configuration row. The conservative gate's blockers on the
created objects' descriptors and on their module and help rows, and on the configuration's descriptor that lists them, are
withdrawn (`decide`); the phase carries `created` (uuid, kind, files) and `answered_rows` (the configuration's descriptor).

The apply (`mssql_config_apply/mod.rs`, one small block; `gate.rs`, one field and one rule): `objects::analyze` knows a new
form or template only and blocks a new catalog or document, so the apply takes the phase first
(`take_structure`, before the analysis blockers are merged) and drops the blockers on `created` uuids and files and on
`answered_rows`. The created objects then join `new_registrations`: the same `NewRegistration` a new form gets, at every node
of the exchange plans but the plans' own (`objects::registration_nodes`, #412), with the object's files as the list in
`_ConfigChngR_ExtProps`. The rows themselves are moved like any staged row. The size guard (S1-J) counts a table the
database does not have yet as empty, so a created table adds nothing to the limit (`size_check` in the report: rows 0).

One rule of the conservative gate was wrong for every stage that carries `Configuration.xml`, and a new object's stage always
does: the `root` service row. The importer stages it deflated; a database the platform has applied to keeps it as one
stored block. The gate compared the bytes and refused ("the service row root changes"). It now compares the inflated text
like a descriptor (`compare_root`). The twin of N1 found it; none of the earlier cases stages `Configuration.xml`.

## 4. Verification

The twin protocol of 12.6, per case, with the split of `derived-caches.md` 9:

| check | how, for a new catalog |
|---|---|
| 1 | a test per case: the plan made offline from the staged snapshot equals the native result: the `DBNames` text (on a base that already had a native apply), the entry of every new table, the cache rows (done) |
| 2 - 5 | `snapshot.py` / `snapdiff.py`, `compare_config.ps1`, `dbschema_cmp.py` on the twins; the new tables are empty |
| 6, 7, 9 | `si_diff.py`; native `config apply` afterwards «не требуется»; a session that creates and reads an element of the new catalog (the probe of S1-G) |
| 8 | native export of both; `source-diff` |
| 10 | `--rehearse` changes nothing |
| 11 | the refusals of 3.3 as tests of `decide` and a dry run on a stage that has one change more |
| 12 | a `THROW` before `COMMIT` in the script; the database as it was, no `NG` table, no new name |

## 5. Native cases to trace (checkpoint 2)

Each on a base that already had a native baseline apply (so the platform's upgrade tables are not in the diff), staged by
the native partial import, a COPY_ONLY backup, the native twin traced by XE (recipe 10.8 of `restructuring.md`), ours
compared. The kit of `scripts/apply-trace/lab/s1g-caches/` (case d) makes the stage.

| case | what | why |
|---|---|---|
| N1 | a flat catalog with attributes of string, number, date, boolean, a reference to a catalog, one indexed | fields, the index names, the numbers, the `R` type entry |
| N2 | a hierarchical catalog (folders and items) with a tabular section | `ParentID`, `Folder`, `ByParent...` indexes, the nullable rule |
| N3 | a new catalog that the root lists **before** a catalog with a new attribute, in the same stage | the order of the numbers across old and new objects (1.3). The catalogs of the first tries were refused by the gate (an extension adopts `_ДемоПартнеры`; `_ДемоКассы` is subordinate), which is the refusal test; the case is `_ДемоСтавкиНДС` |
| N4 | a new document with a periodic numeric number and an indexed attribute | `NumberPrefix`, `Number N`, `ByDocNumPrefix`, `ByFieldFld` |
| N5 | a new catalog and a new document in one stage | the caches and the numbers across kinds |
| N6 | like N3 but the new catalog is listed **after** the changed one (added after N3 showed the numbers) | whether the created objects come first or in the configuration's order (1.3) |
| N7 | a new document beside a changed catalog | the kinds across created and changed objects |

## 6. Work packages, in this order

1. `common.rs`, `entry.rs` and their corpus test (3.2): done (checkpoint 2; 108 of 139 stored entries rebuilt byte for byte, 0
   wrong, 31 refused by reason).
2. `create.rs` and the hooks H1-H4 in small separate commits: done and merged into `feat/0.4`.
3. H5, H6, the apply's registration, the twin runs of section 4 per case, the refusals, the failure injection: done
   (checkpoint 4, section 8).

## 7. Open points

- **The creation primitive of `ddl`** (`TablePlan::created`, `Running::allocate`, `subtable_entry`, `push_subtable`): used
  as designed.
- **The apply's registration of created objects**: done, in the apply's own files, as a small block and one rule (3.4), with the
  apply track's agreement (the seam is `phase.created`).
- **The service properties** of the XDTO type are the common attributes that list an object (1.2): none for a new object
  of the БСП, so the caches' open risk is closed for it; the refusal of a configuration with another `AutoUse = Use`
  common attribute is `common.rs`'s.
- **What is not built** (refused, with the reason): forms, templates, commands and predefined items of a new object; an
  owner or a characteristic type the entry builder cannot prove (31 of the 139 corpus objects); an exchange-plan membership;
  data history; several new objects of one kind in a stage; removed attributes or new tabular sections of existing objects
  beside a created object (the caches are chained on additions only); a configuration with a common attribute other than the
  data separator that applies to every new object; 8.5 and an empty change register (in the apply).
- **Combinations not traced**: a created object beside a changed *document*, a changed hierarchical catalog, or a
  changed object that gains a tabular section (N3/N6/N7 have a flat catalog that gains an attribute); the rules of 1.3 say
  what the walk does for them, and the planner applies them, but no native trace covers them. The next twin to run is a
  created catalog beside a changed document.
- **`c4629235`** (the row that lists the object types for the platform's type cache) is still made in an approximate order;
  the platform accepts it and reads the same (S1-G, `derived-caches.md`): the one `.si` row of 16 that differs in every
  twin.
- **A created object with a module or a help page** is planned and its rows move (unit tests of the gate and the plan), but no
  twin has one: the rows of a module are a plain moved row.

## 8. Results (checkpoint 4)

Seven native cases, each a native partial import of the edited tree on a clone of a base that already had a native apply
(`twin_stage.ps1`: one staged state, a `COPY_ONLY` backup, twins from it, the native apply on one), ours on the other
through `mssql-config-apply --allow-restructure s1 --i-have-a-backup` (`twin_run.ps1`, the ddl kit's twin protocol of
`restructuring.md` 12.6) and, for N3, through the drop-in `ibcmd-rs infobase config apply --recovery-backup=<file>`. The
evidence is `evidence/new-object/s1f-twin-compare.txt`; the kit is `scripts/apply-trace/lab/s1g-caches/`
(`make_cases_n.py`, `twin_stage.ps1`, `twin_extra.ps1`, `twin_inject.ps1`, `twin_final.ps1`, `reg_cmp.py`, `f_created.bsl`, `f_cleanup.bsl`, `job_one.ps1`,
`assemble_twin_evidence.py`; its README has the run of one case).

| check (12.6) | N1 | N2 | N3 | N4 | N5 | N6 | N7 |
|---|---|---|---|---|---|---|---|
| 10 a rehearsal changes nothing | yes | yes | yes | yes | yes | yes | yes |
| 2 tables, columns, indexes | equal but the random `PK___ConfigChngR` name (and the platform's own `_DbCopiesInfoBaseUse`) | same | same | same | same | same | same |
| 3 data of the created tables (and of the changed one) | empty, `EXCEPT` 0 | same | same, `_Reference23` 2 / 2 | same | same | same, 2 / 2 | same, 2 / 2 |
| 4 `Config` rows, dates included | equal | equal | equal | equal | equal | equal | equal |
| 5 `DBSchema` entries (byte equal), their order, `DBNames` text | equal | equal | equal | equal | equal | equal | equal |
| 6 the `.si` rows | 15 of 16 | 15 of 16 | 15 of 16 | 15 of 16 | 15 of 16 | 15 of 16 | 15 of 16 |
| register: `(node, object)` rows, the new object's 3 rows (6 for N5), file lists | equal | equal | equal | equal | equal | equal | equal |
| 7 a native `config apply` on our twin | «не требуется» | same | same | same | same | same | same |
| 9 a cluster session on both twins (lines of output) | identical (21) | identical (23) | identical (30) | identical (20) | identical (32) | identical (30) | identical (33) |
| 8 native export of both, `source-diff` | 0 different of 12 200 | | | | 0 different of 12 201 | | |
| 12 a `THROW` before `COMMIT` | | digest unchanged | digest unchanged | | digest unchanged | | |

Also: N3 through the drop-in `ibcmd-rs infobase config apply --recovery-backup=<file>` (1 minute with the `BACKUP`): checks 2, 4, 5, 6 and the tables as
above; and N3, N5, N6 again on the merge with `feat/0.4` 999e8fd5 (S1-E sections, S1 on 8.5, #409, #348) with the binary of the
merge commit, on fresh own twins: the same. The `.si` row that differs is `c4629235` (approximate order, `derived-caches.md` 4). Blank cells were
not run: check 8 is a pair of native exports of 10 GB per case, check 12 a restore and a 30-second script.

**What the twins found**, each fixed in its own commit, each a thing no test against the native result could see:

1. The conservative gate compared the `root` row by its bytes; a base the platform has applied to keeps it as one stored
   block, the importer stages it deflated. A new object's stage always carries `Configuration.xml`, so every such stage was
   refused. Now by text (3.4).
2. The apply's analysis blocked the created objects and the configuration descriptor that lists them; the phase now names
   what it answers for (3.4).
3. The `DBNames` header (1.3): one above the last entry for a catalog. The tests recomputed the platform's header.
4. The order of the numbers across created and changed objects and of the tables in `DBSchema` (1.3, N3/N6/N7); case c had
   been read as one stage.
5. N3's first two choices of a changed catalog were refused by the gate (an extension adopts `_ДемоПартнеры`, `_ДемоКассы` is
   subordinate to owners): the refusals of the changed objects hold in a stage that also creates one. The case uses
   `_ДемоСтавкиНДС`.
6. The `root` rule composes with rcheck's (the merge with S1 on 8.5): `decide` withdraws the conservative gate's byte-wise `root`
   blocker (a real change of the root is refused earlier, by the check), so on the S1 path `compare_root` and that withdrawal
   say the same in two places. `compare_root` is what a stage needs that reaches the conservative verdict with no restructuring
   (no `decide` is asked then). No lab stage has the root row as its only blocker: a stage of `Configuration.xml` alone is
   refused for another reason on this base (`row-format-unproven`: the importer writes the Configuration row in another record
   format), so the rule is covered by the unit tests of the gate (another layout is no change, another configuration is refused).

**Drift** (the same on every case, documented in `restructuring.md` 12.6 and `own-apply.md`): the random primary-key
name of `_ConfigChngR`; `_DbCopiesInfoBaseUse` (the 2214 build of the native apply creates it on a base restored from
an older build's backup, this apply does not); `Files` `MobileVersions.dat` and `gc.mrk`; `Params` `.ui` rows and the guids of
`siVersions`; `_ConfigChngR._MessageNo` of the objects the stage does not touch (the native rebuild writes 0 to the NULLs of the
base, this apply leaves them: 17 313 to 17 325 rows, every one NULL in the staged clone already, none of the objects of the stage).

**The size guard** (S1-J) counts a table the database does not have yet as empty: `size_check` of every real run reads
rows 0, `within_limit` true (the created tables add nothing to the limit).
