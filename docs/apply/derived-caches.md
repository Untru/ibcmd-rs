# The derived caches of a new object (S1-G, issue #403)

Part of S1 (#391, `docs/apply/restructuring.md` 12.5 and 12.7). When a configuration change adds an object, a
tabular section or an attribute the platform rewrites the `Params` `*.si` rows. This is what each row holds, what a
new catalog, a new document, a new tabular section and new attributes change in it, and how
`src/restructure/caches/` reproduces it. Everything here is measured on БСП 8.3.27 (compatibility 8.3.27):

| name | database / label | what it is |
|---|---|---|
| `pristine` | `ddl_bsp8327_a` / `a2_staged` | the caches before cases c and h |
| `c2` | `ddl_bsp8327_c2` / `c2_now` | native after case c: a new catalog `ДемоНовыйСправочник` (and the two attributes of case a2 / b) |
| `m` | `ddl_bsp8327_m` / `m_now` | native after case h: a new tabular section `ДемоНоваяТЧ` of `_ДемоКонтрагенты`, attributes of every type on three catalogs and a tabular section, and edits of registers |
| `t1_before`, `t1_nat` | `ddl_s1_base` / `t1_staged`, `ddl_s1_t1_nat` / `nat_after` | the types case (23 attributes of every primitive type, no new object) |
| `d_staged`, `d_after` | `trace_d_base` / `d_staged`, `d_after` | case d: a new document `ДемоНовыйДокумент` with an attribute and a tabular section, cloned from `_ДемоОприходованиеТоваров`, staged with the native partial import on a fresh БСП clone and applied natively (kit `scripts/apply-trace/lab/s1g-caches/`) |
| `n_base`, `n_ours` | `trace_n_base`, `trace_n_ours` | two restores of the native state after case c (`ddl_bsp8327_c2_native_after.bak`); in `n_ours` the eight rows of case c are ours (section 8) |

`cargo test --lib restructure::caches` runs the checks below against them (they skip without the lab; the
`d_*` snapshots come from `IBCMD_RS_TRACE_LAB`, default `F:\ibcmd\lab\05\s1g\store`).

## 1. The order is a hash map's

Six of the eight rows a new object rewrites are dumps of in-memory hash maps, and to be byte-exact a row has to
repeat the **iteration order** of the map. Measured on all of them, the map behaves like a Microsoft
`std::unordered_map` keyed by the uuid with `Data1` (the first 8 hex digits) as the hash:

- the elements are one list; the elements of a bucket are contiguous;
- the table starts with 8 buckets and grows after an insertion that leaves more elements than buckets:
  x8 while below 512 buckets, x2 from there (8, 64, 512, 1024, 2048, 4096);
- an element of an **empty** bucket goes to the **end** of the list, an element of a bucket that has elements
  goes to the **front** of that bucket's run;
- growing walks the list and regroups it by the new mask: a bucket's run stands where its first element
  stood and is reversed inside (each later element goes to the front).

The order is a function of the insertion order of the keys, so adding **one** key early shifts every growth
point after it; that is why a new object moves a few unrelated entries (case c: `28e59c50` next to the new
catalog and `d6704410`, in four rows). `caches::order` implements the model; the tests reproduce **every**
root section of `2203278d` (21 sections, 3 snapshots), the tabular-section sections of catalogs (111 and 112
entries) and of documents (48), `42ed49cc` (114 and 115 catalogs), `facbfffe` (sections 1-6, both snapshots) and,
in a scratch experiment, `c77bc206`, byte for byte.

**Insertion orders found.**

| container | insertion order |
|---|---|
| a `2203278d` section of a kind the root lists | the root's collection of the kind, in the root's order |
| `2203278d`, tabular sections (`932159f9-...` of catalogs, `21c53e09-...` of documents) | the objects of the kind in the root's order, each one's sections in the order of its descriptor |
| `42ed49cc`, catalogs | the root's catalog collection |
| `facbfffe`, a section | the kinds in the platform's order (`ExchangePlan`, `Constant`, `Catalog`, `Document`, ... `synonyms::CLASS_ORDER`), each in the root's order |
| `a07b62f0` | not a hash map: metadata order, kind by kind, each in the root's order |
| `1a621f0f` | not a hash map: pre-order of the metadata tree, the objects of a kind in the root's order (section 5) |
| `fe8acd6a` | not a hash map: sets sorted as text |
| `c77bc206` (unchanged by all cases) | the uuids sorted as text in the `pristine` clone; in `s1_base` and in a fresh restore of the corpus (`d_staged`) it is another order, and a native apply leaves either text as it is |
| `c4629235` | **not found** (see 4) |

The kind order of `facbfffe` is a partial order (`ExchangePlan < Catalog < Document < DocumentJournal <
InformationRegister < ChartOfAccounts`, `Constant < Report < DataProcessor`, `ChartOfCharacteristicTypes <
ChartOfAccounts`); it is exact for sections 1-6 of the corpus; sections 0 and 7 (7 and 65 entries; they have
keys that the root does not list) are refused.

## 2. The rows and what a new object does

| row | holds | new catalog (c) / new document (d) | equal to native |
|---|---|---|---|
| `1a621f0f` | the object registry: every metadata object in pre-order, `uuid, owner, kind, "name", {synonym}, flag, flag` | the records of the object, of its attributes, of its tabular sections and their attributes, after the subtree of the object the root lists before it (section 5) | yes (c, d, h, t1) |
| `2203278d` | per kind: object -> generated types `(TypeId, ValueId, index)`; the index numbers the *category* (`Object` 0, `Ref` 1, `Selection` 2, `List` 3, `Manager` 4), not the position | one entry (5 types) in the section of the kind, the section refilled in the root's order; the object's tabular sections join the section of their class | yes (c, d, h) |
| `a07b62f0` | `"Catalog.X","Справочник.X",<uuid>,1,0,"Reference11036",11036,<Ref TypeId>` (documents: `"Document.X","Документ.X",...,"Document11034",...`; then a second list of change tables) | one entry after the entry of the previous object of the kind; the count | yes (c, d) |
| `42ed49cc` | three maps; the one whose keys are the catalogs: `<catalog>,<n>,<owner>...` | catalog: `<uuid>,0` (or its owners), section refilled. **A document changes nothing in this row** (d: text equal before and after) | yes (c); n/a (d) |
| `facbfffe` | eight maps object -> one localized string of one property (1 `ListPresentation`, 2 `ExtendedListPresentation`, 3 `ObjectPresentation`, 4 `ExtendedObjectPresentation`, 7 `Explanation`) | an entry per non-empty property; **the object presentation, not the synonym** (case c: the catalog was copied from `Удалить_ДемоОбщиеСведения` and kept its presentation) | yes (c: section 3; d: sections 1 and 3) |
| `fe8acd6a` | named sets of type ids, sorted; `e61ef7b8` = every catalog `Ref`, `e2cb8e3e` = every catalog `Object`, `38bfd075`/`f72bc2d7` the same for documents, `280f5f0e` = every reference type | the `Ref` id into two sets, the `Object` id into one (`type_sets::FAMILIES`, checked against the corpus) | yes (c, d) |
| `c4629235` | one map over the objects and their nested elements (2376 keys, 4096 buckets). Catalog: `2 EditType`, `5 Hierarchical`, `6 HierarchyType`, `21 SubordinationUse`, `10 QuickChoice=1`, `0` a help reference when a `Config` row `<uuid>.1` exists, `3` the `Ref` type, `24` type sets that mention the object. Document: `19 Posting`, `20 RealTimePosting`, `0`, `3`, `24` | one entry, exact for all 115 catalogs of `c2`, all 25 documents and the new one of `d` (property 24 aside: it is not made of the descriptor) | **entry yes, position no** |
| `ea13a2c9` | the XDTO model | `CatalogRef.X` / `DocumentRef.X` after the previous object's; the block (row types of its sections, then `CatalogObject.X` / `DocumentObject.X` with the standard properties and the attribute lines of section 6) after the previous object's block | yes (c, d, t1); h: yes in the 438 types of catalogs and documents |

The standard properties of `CatalogObject.X` are `IsFolder` (hierarchical, folders and items), `Ref`,
`DeletionMark`, `Owner` (`AnyIBRef`, nillable), `Parent` (hierarchical), `Code` (`xs:decimal` for a numeric
code), `Description`, `PredefinedDataName`; those of `DocumentObject.X` are `Ref`, `DeletionMark`, `Date`,
`Number` (`xs:string`), `Posted`; both are checked against all objects of the corpus (114 catalogs, 25 documents).

## 3. A new tabular section (case h)

Three of my rows change: the registry (the section's record and the records of its attributes, section 5),
`2203278d` (the section of the tabular-section class: an entry `<section>,2,<type>,<value>,0,<row type>,<row
value>,1`, refilled in the traversal order of 1) and the XDTO model (`CatalogTabularSectionRow.X.T` /
`DocumentTabularSectionRow.X.T` right before the object type or after the previous section's row type, and the
property `T` in the object type, `lowerBound="0" upperBound="99999"`). All equal native's for catalogs (h); for
documents the new section of case d is part of the new document and equals native's too.

**Several sections in one stage (S1-E, cases e1 and e4).** The sections of a kind are added in **one** refill of the
section of `2203278d` (`plan::new_tabular_sections`): the traversal lists the sections of every object of the kind in the
root's order, each object's in the order of its descriptor, and the section is rebuilt from that order with the new
entries in it, so it does not matter how many are new. Measured against native: e1 (five new sections: three
catalogs, one of them hierarchical and one subordinate, and two documents) and e4 (a catalog with one new section
and a document with two, both with new own attributes and a new attribute in a section they had) -- the registry,
`2203278d` and the XDTO model equal native's text in both, and in e3 (new attributes of old sections only) the
registry and the XDTO model (`tests_corpus.rs` of `restructure`: `corpus_plan_of_*_equals_the_native_result`). The
registry and the XDTO model are text edits and compose in any order.

## 4. `c4629235`: proven acceptable, not byte-exact

The map is filled in an order that is neither the root's nor the registry's (`1a621f0f`) nor a finer hash
order (tested), and its tail interleaves the nested elements of documents, data processors and registers (the
insertion looks like "an object and the objects it refers to", subsystems in root order between the register and
the common-form passes). A new object early in its collection moves **the same** few other entries whatever
it is: in both cases c and d the entries `0aba89f9`, `778cd9f9`, `02a84df9` (near position 731) and `d74ccadb`
(from 1946 to 2188) stand elsewhere, plus one entry next to the new one (`28e59c50` in c, `36810e6e` in d) --
the growth points of the table shift by one. With the approximate placement (`HelpProps::add_entry_approximately`:
where a last-inserted key goes) 10 or 11 of the 2376 neighbour links of native's row are broken and
`CacheRow::exact` is `false`. The decision of the coordinator is to stop here: `exact = false` and the proof below.

**The platform accepts the row.** Two twins of the native state after case c (`ddl_bsp8327_c2_native_after.bak`);
in one the row `c4629235` is replaced by ours with a new `siVersions` guid, the other is untouched
(`scripts/apply-trace/lab/s1g-caches/prove.ps1`, results in `docs/apply/evidence/derived-caches/`):

| step | untouched twin | twin with our `c4629235` |
|---|---|---|
| stand-alone server (`ibsrv`) starts, thin client runs the new-catalog probe (metadata, query, write, type of the reference, XDTO of the reference and the object, query by the reference) | ready in 9 s, 10 of 10 lines | ready in 6 s, the same 10 lines |
| the same probe in a session of the 8.3.27 cluster | the same 10 lines | the same 10 lines |
| native `infobase config check` | exit 0, «Проверка корректности метаданных успешно завершена» | the same |
| native `infobase config apply` afterwards | exit 0, «Обновление конфигурации базы данных не требуется» | the same |

Neither of these operations reads the row in a way that shows (12.5 measured the same for a stale and a
deleted `c4629235`), so this proves that our row does no harm, not that it is used; the next structural native
apply rewrites all sixteen rows anyway. Twin check 6 ("16 of 16 rows have the same text") tolerates the order
of the entries of this row, like the other drift (see 10). Section 8 repeats the proof with all eight rows of
case c ours.

## 5. The registry row `1a621f0f`

`caches::registry` on top of `mssql_config_apply::si` (the apply track's parser and editor of the row). Measured
on cases c, d, h and the types case, and checked against **every catalog and document of four snapshots** (139
objects, 2299 records each: the records the descriptor explains are the registry's, byte for byte):

- the objects of a kind stand in the root's order; a new object goes after the subtree of the object the root
  lists before it (before the next one when it is the first); its owner is the configuration record;
- the two flags of an object are `IncludeHelpInContents` and `UseStandardCommands` of its descriptor (catalogs
  and documents; an attribute, a tabular section and its attribute have `0,0`);
- the synonym block is the object's synonym (`{1,0}` when empty);
- below an object: its attributes (class `cf4abea7-...` of a catalog, `45e46cbc-...` of a document), then its
  tabular sections each followed by its attributes (class `888744e1-...`); a document lists its forms between
  the attributes and the sections (`si::child_order`);
- a new attribute of an existing object goes after the previous attribute, the first one between the groups the
  other owners of the kind list (`si::place`); a new tabular section after the previous section.

An object that lists forms, templates, commands or any collection beyond its attributes and tabular sections has
more records than the descriptor's attributes explain: such a new object is refused.

A changed synonym of an existing metadata header is also written into this registry, for both the
nonstructural gate and S1. `mssql_config_apply::synonyms` reads every staged descriptor's headers by UUID
and reconciles their localized text with the registry. It deliberately does not filter by physical Config
digests or join physical Config descriptors: a stage can revert a newer dynamic alias to the exact bytes
of its physical base. Descriptor parts are joined before inflation. `registry::set_synonyms` changes just
the matching record's localized block, on top of any edits from
new objects, members, forms, templates or removals. The guarded rewrite preserves the stored-row digest
and bumps the registry's `siVersions` entry. Empty synonyms, multiple languages and doubled quotes are
covered by unit tests. Headers absent from the registry require no record edit.

The plain `syn2` and structural `mix2` twin measurements, including the older build's stale registry and
fresh-session metadata presentations, are recorded in
[evidence/dropin-apply/synonyms.md](evidence/dropin-apply/synonyms.md).

## 6. The attribute lines in the XDTO model

The property line of an attribute is made of its type pattern (`caches::xdto_types::attribute_property`):

| pattern | line |
|---|---|
| `{"B"}` | `type="xs:boolean"` |
| `{"S",...}` | `type="xs:string"` |
| `{"N",...}` | `type="xs:decimal"` |
| `{"D",...}` | `type="xs:dateTime"` (date, date and time and time alike) |
| `{"#",e199ca70-...}` | `xmlns:d4p1="http://v8.1c.ru/8.1/data/core"` `type="d4p1:ValueStorage"` |
| `{"#",fc01b5df-...}` | the same with `d4p1:UUID` |
| `{"#",<Ref TypeId>}` | `xmlns:d4p1=".../current-config"` `type="d4p1:CatalogRef.X"` (`EnumRef`, `DocumentRef`, ...: the kind of the object and `Ref`; the type id is the `Ref` type of the entry of `a07b62f0`) |
| several items (a composite type) | no type: `<property name="X" [lowerBound="0" ]nillable="true"/>` |
| a defined type | **refused**: the line is that of the defined type's content (`xs:decimal`, a reference, `nillable`), which needs the defined type |

`lowerBound="0"` stands exactly when the field is nullable: an attribute of a hierarchical catalog of folders and
items whose `Use` is `ForItem` or `ForFolder`; never for a document, a flat catalog or a tabular section. The
lines of the attributes follow the standard properties and precede the properties of the tabular sections; a new
attribute is the property line number `standard + position` (`XdtoModel::insert_property_lines`).

Checked against **all attributes of all catalogs and documents and their tabular sections in five snapshots**
(pristine, c2, m, t1_nat, d_after: 139-140 objects, about 2150 lines each, references, composite types,
value storage and uuid included): every line equal, with three sorts of lines that are not attributes left out:
the platform adds a property named after a **common attribute** to the XDTO type of every object that
the common attribute lists with `Use` (`ОтредактированныеПредопределенныеРеквизиты`, `ОбластьДанныхВспомогательныеДанные`,
`НаименованиеЯзык1`/`2`, `КомментарийЯзык1`/`2`; 33 catalogs of the corpus), after the attributes and before the section
properties. They are not made of the object's descriptor but of the common attributes' `Content` (checked on all 114
catalogs: the service properties of a catalog are exactly the common attributes that list it with `Use`,
114 of 114; `docs/apply/new-object.md` 1.2). A **new** object is in no `Content`, and the only common attribute
with `AutoUse = Use` in the corpus is the data separator, which adds a column but no XDTO property: a new
object gets none, as cases c and d show. A stage that lists the new object in a common attribute changes that
common attribute's row (refused by the classification); a configuration where another common attribute has
`AutoUse = Use` is refused by the planner of S1-F (`common.rs`). A new object with predefined items (a `Config` row
`<uuid>.1c`) is refused as well.

## 7. Every row against native, per case

`caches::change::rewrite` composes the pieces: for a staged change (the stored and the staged descriptors of the
changed catalogs and documents, the root, the cache rows) it gives the final text of every rewritten row. The
test `tests_change.rs` replays each native case and compares **all sixteen rows**:

| case | rows native changed | ours |
|---|---|---|
| c: a new catalog and two attributes of other objects | `1a621f0f`, `2203278d`, `42ed49cc`, `a07b62f0`, `c4629235`, `ea13a2c9`, `facbfffe`, `fe8acd6a` | seven equal to native's after inflate, `c4629235` the same 2377 entries with 11 of 2376 neighbour links differing (order only); the other eight rows native left alone |
| d: a new document with an attribute and a tabular section | `1a621f0f`, `2203278d`, `a07b62f0`, `c4629235`, `ea13a2c9`, `facbfffe`, `fe8acd6a` | six equal, `c4629235` order only (10 links); `42ed49cc` untouched, as native's |
| h: a new tabular section, 12 attributes of every type (references, composite, value storage, uuid), an attribute of a hierarchical catalog and one of a tabular section, edits of registers | `1a621f0f`, `2203278d`, `ea13a2c9` | `2203278d` equal; the registry equal but two records of the registers' edit (native's side); the XDTO equal in the 438 types of catalogs and documents, native also changed 3 types of registers, and ours leaves every other type as it was |
| t1: 23 attributes of every primitive type on six objects | `1a621f0f`, `ea13a2c9` | both equal (full text); the other fourteen native left alone |

Refused by the replay (tests): a new catalog that lists forms, templates or commands; one with predefined items;
two new catalogs in one stage; an attribute of a defined type.

## 8. The necessity table, repeated on our result

12.5 measured what the platform needs of each row on the native state of case c. The same probe on `n_ours`,
where **all eight rows of case c are ours** (`change::rewrite`, deflated, each with a new `siVersions` guid),
and on `n_base`, the untouched native twin, one row absent or stale (the row as before the case) at a time
(`scripts/apply-trace/lab/s1g-caches/necessity.ps1`, reports in `docs/apply/evidence/derived-caches/`):

| row | state | `n_base` (native rows) | `n_ours` (our rows) |
|---|---|---|---|
| all eight | as they are | the probe passes: 10 lines | the same 10 lines (stand-alone server and cluster session) |
| `1a621f0f` | absent | the server exits at start | the server exits at start |
| `1a621f0f` | stale | the client fails: «Тип не определен» | the client fails: «Тип не определен» |
| `a07b62f0` | absent | the server exits at start | the server exits at start |
| `a07b62f0` | stale | no result in 150 s (the job hangs) | no result in 150 s (the job hangs) |
| `2203278d` | absent | the client fails: «Тип не определен» | the client fails: «Тип не определен» |
| `2203278d` | stale | the client fails: «Тип не определен» | the client fails: «Тип не определен» |
| `ea13a2c9` | absent | everything works | everything works |
| `ea13a2c9` | stale | the probe passes but the XDTO serialization of the object fails: «Несоответствие типов» | the same |

The symptoms of our rows are those of native's, and those of 12.5: eight variants, eight identical outcomes
(the "hang" is a 150 s time-box here; 12.5 waited 348 s). After the ablation both twins were given the platform's
own checks: native `config check` succeeds on both («Проверка корректности метаданных успешно завершена») and
native `config apply` says «Обновление конфигурации базы данных не требуется» on both (`prove_ours_all_native.txt`,
`prove_base_all_native.txt`).

## 9. Twin protocol 12.6: what the caches can answer, and what moves to S1-F

The twin of 12.6 needs the new object's tables (`DBNames`, `DBSchema`, the `Config` rows, the exchange-plan
registration): S1-F. What the caches can answer without them is a **cache-only twin**: the native state after a
case with the rows we make swapped in (sections 4 and 8).

| check of 12.6 | this issue |
|---|---|
| 1 the plan offline equals the native result | the cache rows: yes (section 7, all four cases); `DBNames`, `DBSchema`: S1-F |
| 2 tables, columns, indexes | S1-F |
| 3 data of the rebuilt tables | S1-F (a new object has none; the attribute operations are ddl's) |
| 4 the `Config` rows | S1-F |
| 5 `DBSchema` and `DBNames` | S1-F |
| 6 the 16 `.si` rows | yes, offline (section 7) and on the platform (section 8), `c4629235` modulo the order of its entries |
| 7 native `config apply` on the twin | yes on the cache-only twin (section 8); on the real twin S1-F |
| 8 native export of both | S1-F |
| 9 a session in the cluster | yes on the cache-only twin (section 4, 8) |
| 10 a rehearsal changes nothing | S1-F (`change::rewrite` is a pure function of its inputs) |
| 11 the refusals | the caches' own refusals (section 7); the classification is S1-H |
| 12 a failure in the transaction takes everything back | S1-F (the rows are written by the apply's guarded rewrite) |

## 10. Proposed text for the drift list of 12.6

Add to the platform's own derived state that a twin comparison tolerates:

- `Params` `facbfffe`: section 3 holds the **object presentation** (the second localized string of the row),
  not the synonym; a twin whose new object was cloned from another keeps the source's presentation. Compare the
  rows, not the synonyms.
- `Params` `c77bc206`: the text is equal before and after a native apply in the same database but differs
  between databases of the same configuration (a fresh restore of the corpus, `s1_base`, the `pristine` clone):
  the order of its hash sets depends on the database. Compare as sets, or skip in check 6.
- `Params` `c4629235`: our rows carry every entry exactly but not the position of about five entries of 2376
  (see 4); the platform accepts the row (server start, cluster session, `config check`, apply «не требуется»).

## 11. What is left

- **`c4629235` exact order** (see 4): stopped by decision; `exact = false` stays.
- **The service properties** of section 6 are the common attributes that list an object: none for a new object of
  the corpus (the risk is closed there); the refusal of another `AutoUse = Use` common attribute is S1-F's.
- **Defined types** of attributes: refused (the `Ref` and composite mapping is built).
- **`facbfffe` section 7** (`Explanation`) and the sections 0, 5, 6 (constants, registers) are refused.
- **Kinds other than catalogs and documents** are refused (`Enum` and `ExchangePlan` on purpose).
- **Objects with forms, templates or commands** are refused: their records reach rows this module does not build.
- **More than one new catalog or document of a kind in one stage**: the rows are refilled in the hash order of all
  the keys; the sequential composition has it only for the last addition. Refused. The new tabular sections of
  *existing* objects are not in this list (S1-E, section 3): any number of them come in one refill; together with the
  sections of a *new* object of the same kind they are refused.
- `DocumentObject` `Number` as `xs:decimal` for a numeric `NumberType`: only the string is measured.

## 12. API (`src/restructure/caches/`)

```rust
change::rewrite(&Staged { root, before, after, changed: &[ChangedObject { kind, uuid, table_number, has_help,
                          has_predefined }], cache }) -> Result<Vec<CacheRow>>
    // the final text of every rewritten row: a new object (plan::new_object), a new tabular section
    // (plan::new_tabular_section), new attributes of existing objects and sections
plan::new_object(&NewObject { kind, descriptor, root, table_number, has_help, has_predefined, cache, name_of,
                              descriptors }) -> Result<Vec<CacheRow>>
    // 1a621f0f 2203278d a07b62f0 [42ed49cc: catalogs] facbfffe fe8acd6a c4629235 ea13a2c9
plan::new_tabular_section(&NewSection { kind, root, descriptor, owner, section, cache }) -> Result<Vec<CacheRow>>
    // 1a621f0f 2203278d ea13a2c9
plan::new_tabular_sections(&NewSections { kind, root, descriptor, sections: &[(owner, section)], cache })
    // the same for several sections of a kind in one refill of 2203278d (the one above is a call with one)
members::Members::parse(kind, descriptor)      // attributes, tabular sections, the other collections
registry::{add_object, add_section, add_attributes}
xdto_types::{attribute_property, RefNames, XdtoModel::insert_property_lines}
```

`cache(name)` gives the inflated text of a `Params` row, `root` is the root row **after** the change,
`before` / `after` the stored and the staged descriptor rows by uuid. The result rows are inflated text with
`exact` (false only for `c4629235`); deflating, the `siVersions` guids and the guarded rewrite (`ParamsRewrite`)
are the apply's.
