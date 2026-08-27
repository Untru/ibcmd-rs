# ERP УХ form bodies under `Catalogs/`, `CommonForms/`, `InformationRegisters/`

Base `d4544e9`. Snapshots: `$D/baselines/d4544e9/`. The export reproduced the
`uh` snapshot byte for byte before any change (exact 139 999 / differing 377 /
missing 35 / extra 0), so every number below is measured against a tree this
package actually produced.

The section of the remainder this document covers is the 118 `Ext/Form.xml`
documents of ERP УХ 3.2.12.6 whose path starts with `Catalogs/`,
`CommonForms/` or `InformationRegisters/` — 68, 26 and 24 of them.

## 1. Method

Two histograms over the whole differing set, never a sample: the **first
difference** (byte-compare native against ours, reconstruct the XML element on
each side at the first differing offset) and the **whole diff** (line-level
`difflib` opcodes over the CRLF-split documents, keyed by the *set* of XML tags
any changed line touches). A file whose whole diff touches one tag is closed by
fixing that tag.

Two projections drove the wave: an **element census** (`<Tag>` values the
platform writes and we do not, and the reverse, over the whole differing set,
each with the files it occurs in) and a **native-only census** — every rule
below was measured by reading the platform's own trees, and for the two record
questions by extracting the compiled body of a named form
(`ibcmd-rs cf extract <cf> <uuid>.0 <dir>`) and reading its members directly.

### 1.1 The base histogram, 118 files

```
  21  DataPath                                    2  ChildItemsWidth TitleLocation
  14  Field                                       2  Group Type v8:Type
   5  Field UseAlways                             2  Group
   4  Type v8:Type                                2  CommandSet ExcludedCommand
   3  Command DefaultVisible Item Type …          2  Event Events …ScrollBar
   2  TitleLocation                               2  ShowTitle
   2  Command CommandGroup DefaultVisible …       2  xr:DataPath
```

with 55 further classes of one file each.

## 2. Closed

### 2.1 The root command set is silent about a list whose row set cannot change

`<ChangeRowSet>false</ChangeRowSet>` belongs to the dynamic list, not to the
table's grammar, and the table's *own* `<CommandSet>` already reads it that way:
`FORM_TABLE_ROW_SET_EXCLUDED_COMMANDS` — `Copy`, `Create`, `CreateFolder`,
`Delete`, `MoveItem`, `SetDeletionMark` — are not named there. The root set is
the second reader of the same fact and was not asking it at all.

Census over the native trees of all eight stand corpora: of the 1 867 root
`<CommandSet>` blocks on a form whose main attribute is a `cfg:DynamicList`,
79 are shown by a table declaring `ChangeRowSet=false`, and **not one** of the
79 names any of the six; of the remaining 1 788, 1 620 name at least one.

| key | crs=false | …names one of the six | no such declaration | …names one of the six |
|---|---:|---:|---:|---:|
| `uh` | 41 | **0** | 884 | 828 |
| `ut` | 23 | **0** | 539 | 508 |
| `do` | 13 | **0** | 235 | 161 |
| `ssl` | 1 | **0** | 68 | 64 |
| `sslbase` | 1 | **0** | 62 | 59 |

`mdm`, `ws` and `wms` have no such population at all. No counter-example
anywhere.

This closes the three ERP УХ differences
`docs/evidence/uh-command-set-and-type-link-20260827.md` §5 left open:
`Catalogs/ВидыДоговоровКонтрагентовУХ/Forms/ФормаВыбора` and `.../ФормаСписка`
lost their whole set (`Copy`, `Create`, `CreateFolder`, `Delete`,
`SetDeletionMark`, and `MoveItem` on the list), and
`InformationRegisters/УниверсальныеКомментарии/Forms/ФормаСпискаКомментариев`
kept its `Change` and lost exactly `Copy` and `Create`. The catalogue's own
hierarchy — which §3 of that document is about — never entered into it.

### 2.2 A chart-of-accounts *reference* names its `Description`

`form_standard_attribute_table_for_type_reference` knew the `ChartOfAccounts`
family only in the `Object` role, so a chain whose last step leaves a value
declared `cfg:ChartOfAccountsRef.*` resolved to nothing and the item lost its
`<DataPath>` outright.

The marker is the family's own: `-8` is `Description` in
`CHART_OF_ACCOUNTS_STANDARD_ATTRIBUTE_DEFINITIONS`, the table the metadata
compiler writes `<xr:StandardAttribute name="Description">` from. Fourteen data
paths on nine ERP УХ forms leave such a value and every one of them is spelled
`Description`:

* through a metadata field — `{3,{1},{0,cb76e05c-…},{-8}}` in
  `Catalogs/СоответствияСчетовМеждународногоУчета/Forms/ФормаЭлемента`, written
  `Объект.СчетРеглУчета.Description`; four more in
  `.../СоответствияОборотовМеждународногоУчета`;
* through a form attribute's own column — `{3,{1},{5},{-8}}` in
  `InformationRegisters/ПравилаУточненияСчетовВМеждународномУчете/Forms/НастройкаУточненияСчетов`,
  written `ШаблоныПроводок.СчетУчета.Description`;
* through the attribute itself — `CommonForms/НастройкаСчетовУчетаОперации`,
  written `СчетУчета.Description`.

A census of every native `<xr:Value>`, `<xr:DataPath>`, `<xr:LinkItem>` and
`<DataPath>` naming a member of an attribute or column declared exactly one
chart-of-accounts reference finds that one spelling and no other, so the
`-8 => Ref` reading of last resort in
`form_choice_parameter_link_standard_terminal_member` — which the new table now
takes precedence over for this family — has no case in the corpus. One row is
claimed, because one row is what is observed.

### 2.3 Three platform types in a type pattern, and the same pattern on an attribute

A type the configuration-wide index cannot name and the builtin table does not
carry refuses the **whole** tuple: the `Значение` column of
`Catalogs/ЭтапыСогласования/Forms/ФормаНастройкиУсловногоПерехода` declares
three types, the platform writes three `<v8:Type>` elements and the export
wrote none.

Census of every `<v8:Type>` the platform writes and the export does not, over
the whole differing set of `uh`: exactly three platform types on 13 files —
`ent:ComparisonType` (7), `ent:AccountingRecordType` (6),
`v8ui:HorizontalAlign` (1). The rest of the missing lines are the `cfg:`
siblings of the same refused tuples (7 `EnumRef.ВидСравненияЛимитовЗаявок`, 5
`CatalogRef.ЭтапыУниверсальныхПроцессов`, 2 `CatalogRef.ЭтапыСогласования`).
Each pairing is read off the bytes:

* `b1b064f3-…` is the middle element of `{"#",67e063e3-…},{"#",b1b064f3-…},
  {"#",f2c84078-…}`, whose outer two are the configuration's own declared
  `EnumRef.ВидСравненияЛимитовЗаявок` and `CatalogRef.ЭтапыСогласования` type
  ids, and the platform writes the three names in that same order;
* `43f9c095-…` is the column next to `52616226-…`, which the configuration's
  index already names `v8ui:VerticalAlign` and the export already writes;
* `741ae838-…` is the pattern of six *attributes* of
  `Catalogs/ТиповыеОперацииМеждународныйУчет/Forms/ФормаЭлемента`.

That last case is an attribute, not a column, so the builtin overlay is now
read for an attribute's own pattern too: the role of the record never changes
what a type identifier is called.

### 2.4 The short revision of the page option bag carries the same grouping triple

A `Page`'s option bag has two revisions — the canonical one (lead `18`, 20
members) and a short one (lead `17`, 18). `FormPageSchema` admitted only the
first, so `<Group>` of the second went unwritten: 11 elements on 9 forms, with
nothing extra on our side anywhere.

Joining the compiled bodies of six ERP УХ forms against the pages the platform
writes for them — 14 pages, both revisions, no counter-example — slots 2, 16
and 17 carry the same code in both:

| form | page | bag | triple | platform |
|---|---|---|---|---|
| `InformationRegisters/ЗапросыВычислявшиеРасхождения/Forms/ФормаЗаписи` | `Группа1` | 17/18 | `(1,1,1)` | `Horizontal` |
| `Catalogs/ЭтапыСогласования/Forms/ФормаСписка` | `ПустойМаршрут` | 17/18 | `(1,1,1)` | `Horizontal` |
| `Catalogs/ЭтапыСогласования/Forms/ФормаНастройкиУсловногоПерехода` | `УсловныйПереход` | 17/18 | `(1,1,1)` | `Horizontal` |
| `Catalogs/ЭлементыФинансовыхОтчетов/Forms/РедактированиеЭлементаУсловногоОформления` | `СтраницаУсловие` | 18/20 | `(1,1,1)` | `Horizontal` |
| `Catalogs/ТиповыеОперацииМеждународныйУчет/Forms/ФормаЭлемента` | `СтраницаВыберитеПланСчетов` | 18/20 | `(1,1,3)` | `AlwaysHorizontal` |
| nine sibling pages of the same six forms | | both | `(0,0,0)` | no element |

Only the triple is read. The remaining members of the short bag — the spacing
pair, the alignment pair, the children width, the scroll flag, the colour and
the picture — have no measurement against the platform, so every other reader
still refuses it rather than taking a canonical slot number in a bag two
members shorter. Verified on the extracted bodies of five of those forms: the
number of `<Group>` elements written matched the native document on each
(1/1, 2/2, 1/1, 2/2, 1+1/1+1).

### 2.5 What cannot be named is written physically — twice more

Both halves of this are the rule
`docs/evidence/uh-declared-owner-of-a-name-20260827.md` already states; both
were spelled as a special case of one slot and are now read once.

**An attribute the form does not declare.** A binding `{1,{<id>}}` onto an id
the form's `<Attributes>` collection does not carry was resolved by the *item's
own name* — an invention, because there is no such attribute. Over the eight
native trees exactly twelve `<DataPath>` elements hold a bare integer, all
twelve in ERP УХ and all on such a binding:
`Catalogs/АналитическаяПодписка/Forms/ФормаЭлемента` binds `22` and `23`
against a collection that ends at 19, and the platform writes `22` and `23`.
The old fallback cost nothing to drop: over all 12 658 byte-exact `uh` forms
there is no single-segment `<DataPath>` equal to the enclosing item's own name
that the form does not also declare as an attribute, and among the heads of
every path-bearing element (`DataPath`, `RowPictureDataPath`, `TitleDataPath`,
`xr:DataPath`) on those forms not one fails to name an attribute or an item.

**A command uuid nothing names.** The `0` arm and the `5`/`6`/`7` arm each
carried their own copy of "a well-formed uuid that names nothing in this
configuration keeps the raw `kind:uuid` sentinel"; the other arms refused, and
refusing an item of a command interface drops the whole `<Item>` — and, when
the refused item is the container's only one, the container. The platform
writes 65 `<Command>` values inside `<CommandInterface>` that the export did
not, on 21 forms; 44 distinct uuids carry them, and a scan of all 56 697
metadata documents of the configuration finds **38 of the 44 nowhere at all**.
They are spelled `1:`, `2:`, `3:`, `4:`, `5:` and `8:`, so the sentinel is not
a property of the slot number.
`Catalogs/ОбъектыЭксплуатации/Forms/ФормаЭлемента` alone carries fifteen of
them and `Catalogs/Сценарии/Forms/ФормаЭлемента` seven.

## 3. Named and open

### 3.1 `<UseAlways>` of a `ConstantsSet`, 84 files — the biggest single class left

ERP УХ 3.2.12.6 declares **84** form attributes of type `cfg:ConstantsSet`, and
**all 84** differ. Every form carrying one of `ut`'s 61, `do`'s 38, `ssl`'s 22
or `sslbase`'s own is byte-exact — none of them appears in those corpora's
differing sets — so whatever this is, it is not the plain reading of the list.
Nor is it a rule this stand can generalize: the eight constants below exist in
ERP УХ alone (`ut` declares none of them), which is exactly why the mechanism
is stated here rather than implemented.

The disagreement is confined to eight constants out of the configuration's
1 423, and it is not noise — for four of them the export and the platform
disagree on *every one* of the 84 forms:

| constant | platform writes, we do not | we write, platform does not | sum |
|---|---:|---:|---:|
| `СрокОплатыПокупателей` | 77 | 7 | **84** |
| `СрокОплатыПоставщикам` | 77 | 7 | **84** |
| `ВсегдаКонтролироватьБалансРучныхОпераций` | 69 | 15 | **84** |
| `ПутьККаталогуИмпорта` | 69 | 15 | **84** |
| `ДополнительныеЯзыкиВыводаОтчета` | 0 | 15 | 15 |
| `НастройкиКолонтитуловПоУмолчанию` | 0 | 15 | 15 |
| `СтатусОбновленияКонфигурации` | 0 | 8 | 8 |
| `ПараметрыАдминистрированияИБ` | 0 | 8 | 8 |

The record is read correctly. The attribute record is
`{9,{<id>},0,"<name>",{1,0},{"Pattern",{"#",dcfc3784-…}},<view>,<edit>,
{0,<count>,{1,{0,<constant uuid>}}*count},{0,0},1,<saved>,0,0,{0,0},{0,0}}`,
slot 8 is the list, each entry names a real `Constant` uuid and every uuid
resolves to the constant the configuration declares (checked against
`Constants/*.xml`: 1 423 constants, zero uuid collisions, and each of the eight
appears nowhere else in the tree).

Three forms, read out of their compiled bodies:

| form | slot 8 declares | platform writes |
|---|---|---|
| `CommonForms/ВключениеПроверкиРНПТ` | `ВалютаРегламентированногоУчета`, `ИспользоватьПроверкуРНПТ` | those two **plus** all four of the top group |
| `CommonForms/ПериодичностьОтчетностиМСФО` | 7, including all four of the top group | `ВалютаУправленческогоУчета` alone |
| `DataProcessors/ПанельАдминистрированияУХ/Forms/ОбщиеНастройки` | 32, including four of the eight | those 32 minus the six declared specials, **plus** `СрокОплатыПокупателей` and `СрокОплатыПоставщикам` |

The four uuids the first form's platform document names are **not in its body
at all** (searched the whole decompressed record), so the platform is reading
them from somewhere outside the form.

The model all three forms and all 84 counts fit:

* for the top four, the platform writes the constant **iff the form's list does
  *not* declare it** — an inversion, as if the list were a delta against a
  default of "always used" for those four;
* for the bottom four, the platform never writes it at all.

What has been ruled out by census: no uuid collision; the eight are not named
by any functional option (`<Location>`) or common attribute; and the metadata
properties of the four inverted ones do not separate them — the signature they
share (`ChoiceHistoryOnInput` `Auto`, `FillChecking` `DontCheck`,
`DataLockControlMode` `Automatic`, `ExtendedEdit false`, `QuickChoice Auto` and
eighteen more) is shared by 38 constants of which only four are in the group.
The default therefore lives outside `Constants/*.xml` — most plausibly in a
configuration-level descriptor of the `ConstantsSet` itself, which this package
does not read today. That is where the next pass should look.

### 3.2 The six command uuids that *do* name something

Of the 44 sentinel uuids of §2.5, six name a real object and the platform still
writes the sentinel. They are a rule about the target, not about the name being
unconstructible:

* `InformationRegister.ПротоколыОбъектов` declares `UseStandardCommands=false`,
  and the gate that already answers exactly that is not reached from slot `8`;
* `InformationRegister.ЦеныНоменклатуры` and
  `InformationRegister.ПараметрыУчетаВНАМСФО` both declare
  `WriteMode=RecorderSubordinate` — the export names their `OpenByValue`;
* `InformationRegister.НастройкаРаспределенияПоНаправлениямДеятельности`
  declares `WriteMode=Independent` — the export names its `OpenByRecorder`,
  which an independent register has no recorder for;
* `Catalog.СхемаДоступностиРеквизитов` (`OpenByValue`) and a *subsystem*
  (`3:9b13d197-…`, `Subsystems/УправлениеДоговорамиИПроектами/Subsystems/
  ОтражениеВУчете`) have no explanation yet.

The register pair reads like one rule — `OpenByValue` belongs to an independent
register and `OpenByRecorder` to a recorder-subordinate one — but the
population is four forms, so it is stated here and not implemented.

### 3.3 `Owner` on a catalogue that declares no owner — measured, not plumbed

`form_choice_parameter_link_standard_terminal_member` answers the marker `-5`
with `Owner` for every owner it is given, and the physical fallback beside it
(`{attribute_id}/{marker}`) is therefore never reached. The platform reaches it:
`Catalogs/ВидыДвиженийМСФО/Forms/ФормаЭлемента` writes
`<xr:DataPath xsi:type="xs:string">1/-5</xr:DataPath>` and
`Catalogs/УдалитьКонтрольныеСоотношения/Forms/ФормаЭлемента` writes it twice.

The census separates the two outcomes with no overlap. Over the five corpora
that carry the element at all, every `<xr:DataPath>` naming `.Owner` — 46 of
them — sits on an attribute whose catalogue declares at least one `<Owners>`
member (36 declare one, two declare two, two declare three, one declares four,
and five sit on a multi-typed attribute this scan does not resolve but which is
named all the same), and all three physical `<id>/-5` values sit on a catalogue
whose `<Owners/>` is empty. It is the same law
`docs/evidence/uh-declared-owner-of-a-name-20260827.md` states, and
`MetadataTableStandardAttributes::declares("Owner")` — the very index the root
command set already reads for `Parent` and `IsFolder` — already answers it.

What is missing is only the plumbing: the declaration index reaches
`extract_form_body_attributes_with_dcs_type_index` and the root command set, but
not `parse_form_child_item_with_metadata_owners`, and threading it there touches
every call site of the child-item parser. Three elements on two forms did not
justify that here; the measurement is done and the change is mechanical.

### 3.4 Still open in this section

* `Attribute`/`Field` on a dynamic list, and the `~Список.Code~Список.Код`
  double spelling: the research task `EVIDENCE-form-bodies.md` §9 already
  states the marker; nine occurrences in the whole stand merge two spellings
  into one element and nothing in the reader models that.
* `<AdditionalColumns>` of a dynamic list —
  `Catalogs/СпособыОтраженияРасходовПоАмортизацииМСФО/Forms/ФормаСписка` loses
  the whole block and seven `Items.Список.CurrentData.Способы.*` paths with it.
* `КомпоновщикНастроек.Settings.OutputParameters` on a `Table`, two forms.
