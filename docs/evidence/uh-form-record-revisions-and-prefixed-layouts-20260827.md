# ERP УХ form bodies: record revisions, prefixed layouts and eight closed classes

Base: `19a2f2f`. Snapshots: `$D/baselines/19a2f2f/`. Own export reproduced the
`uh` snapshot byte for byte before any change (exact 139 169 / differing 1 173 /
missing 69 / extra 0).

## 1. Method

Two histograms per corpus, both over the whole differing set, never a sample.

* **First difference.** Byte-compare native against our export, take the first
  differing byte, reconstruct the XML element name on each side at that offset.
  Class key = (native element, our element).
* **Whole diff.** Line-level `difflib` opcodes over the CRLF-split documents;
  class key = the *set* of XML tags any changed line on either side touches.
  A file whose whole diff touches one tag is closed by fixing that tag.

Two further projections were what actually drove this wave:

* **Owner attribution.** For every deleted block, the innermost still-open
  `<Tag name=… id=…>` element in the native document. This turns
  "`<TitleLocation>` missing 168 times" into "`InputField` loses
  `<TitleLocation>`", which is a mechanism.
* **Layout join.** A temporary probe dumped, for every child item of every form
  of the corpus, `(form path, tag, name, wrapper, record length, every nested
  bag's declared revision and length)`; joining it with the owner attribution
  says which *record shape* loses which property. 687 293 item rows. The probe
  is not in the tree.

## 2. `uh` form bodies at the base: owner × property, 915 differing files

```
  222 files=29   owner=Button                 prop=ShapeRepresentation
  198 files=96   owner=Attribute              prop=Field
  150 files=67   owner=Button                 prop=DataPath
  109 files=59   owner=Button                 prop=ToolTipRepresentation
  106 files=44   owner=UsualGroup             prop=HorizontalStretch
   87 files=33   owner=InputField             prop=TitleLocation
   71 files=24   owner=Button                 prop=CommandName
   65 files=19   owner=InputField             prop=DataPath
   47 files=30   owner=CheckBoxField          prop=TitleLocation
   41 files=17   owner=LabelField             prop=DataPath
   37 files=11   owner=InputField             prop=Visible
   35 files=17   owner=UsualGroup             prop=VerticalSpacing
   35 files=12   owner=UsualGroup             prop=VerticalAlign
   30 files=14   owner=UsualGroup             prop=ThroughAlign
   29 files=11   owner=UsualGroup             prop=United
   26 files=13   owner=InputField             prop=Mask
   21 files=18   owner=Button                 prop=BackColor
```

Files whose *whole* diff sits on one item kind at the base: `Attribute` 144,
`InputField` 83, `Button` 77, form-level 68, `UsualGroup` 44, `Table` 27,
`ExtendedTooltip` 27, `LabelField` 22, `Command` 21.

The layout join collapses most of that into two record facts:

```
  105 prop=TitleLocation  tag=InputField    wrapper=35 len=57  bags 39:32/62 42:22/29 52:12/34
   67 prop=HorizontalStretch tag=UsualGroup wrapper=22 len=34  bags 20:28/28 …
   59 prop=TitleLocation  tag=CheckBoxField wrapper=35 len=57  bags 39:11/13 …
   38 prop=Visible        tag=InputField    wrapper=35 len=57
   30 prop=VerticalAlign  tag=UsualGroup    wrapper=22 len=34  bags 20:28/28 …
   27 prop=Mask           tag=InputField    wrapper=35 len=57
```

Every `UsualGroup` in the differing set that loses one of those properties
carries a `28`-member option bag at slot 20; every field item that loses
`TitleLocation`, `Visible`, `Mask` or `ReadOnly` is a wrapper-`35` record.

## 3. Classes closed

### 3.1 The field class's `35` revision (commit 5)

`form_item_record_canonical_revision` already normalizes the class's shortest
revision `34` up to the canonical `37` and states the arity invariant:
`field_count - wrapper` is 22 with the name at slot 6 and 23 with the
conditional `UserVisible`-common prefix pushing it to slot 7, and the shorter
revision is a *tail* truncation. `35` was excluded and recognized one property
at a time instead, so whichever reader had been shown a `35` record admitted it
and every reader that had not refused it outright.

`35` at 57 members is therefore `37` at 59 minus its final two, and `35` at 58 is
`37` at 60 — the prefixed shape, whose own offset the field schema reads off the
name slot. 1 035 such bags and 201 `<TitleLocation>`, 53 `<Visible>`,
27 `<Mask>` and 13 `<ReadOnly>` elements ride on it in `uh` alone.

Two absolute-slot readers had to learn the offset with it, because the
normalization moves prefixed records into their reach for the first time:
`form_document_field_geometry_options` (below) and the `TypeLink` audit.

### 3.2 The `UsualGroup` compact `28` bag (commit 6)

`form_property_bag_canonical_revision` already states the pair: `29` at 29
members (97 347 records) or `28` at 28 (765), the short one the long one minus
its final scalar, `len - lead` 0 for both. The compact bag was nevertheless read
by an arm that answered three properties and `None` for the other twenty.

Padding the one missing member and reading it through the wide arm restores
`HorizontalStretch`, `VerticalAlign`, `VerticalSpacing`, `ThroughAlign`,
`United`, `ReadOnly`, `EnableContentChange`, `ChildItemsWidth`, `CurrentRowUse`
and the rest at their own slots. `Behavior` is the exception and keeps the
compact reading: the wide bag holds it in exactly the padded member, and the
compact bag states it at members 10 and 24.

The normalization is applied at that one call site, so
`FormChildItemShowTitleSchema` — which deliberately claims no colour coordinate
for the compact bag — keeps the shape it was measured under.

### 3.3 The prefixed `Button` record (commit 3)

A `Button` ships at 52 members and at 53 with the conditional-appearance prefix
ahead of the name. `FormButtonCommonSchema` already carried that shift for
`enabled`, `check`, `font`, `parameter`, the geometry pair and both stretch
flags; four readers spelled the unprefixed layout alone and answered nothing for
the prefixed one — the data-path slot, the shape-representation slot, the
tooltip-representation slot and all three colour slots.

Evidence that fixes the whole tail: ERP УХ 3.2.12.6
`Catalogs/ГруппыВНАМСФО/Forms/ФормаЭлемента` button
`ФормаОтчетДвиженияВНАДвиженияВНАМСФО` spells its name at member 6 and the chain
`{2,{1},{-8}}` at member 10 — one behind each unprefixed position — and the
platform writes `<DataPath>Объект.Ref</DataPath>` for it.

### 3.4 The `5007` mirror is an optional member (commit 2)

The `InputField` extended-option bag is `36` at 66 members or `32` at 62. The
links live at member 26, which both carry; the `5007` mirror at member 64 is
what the short revision ends before. The reader required both and answered
`Absent` when either was missing.

The mirror is a duplicate of the same collection, not a second half of it: both
sides are parsed independently and compared for equality, and every field the
caller receives comes from the primary. Dropping the cross-check a revision
cannot supply is not dropping evidence.

**Its cost, measured, and the fix:** shipped without a distinction between the
two arms, the unmirrored one answered `Opaque` whenever the primary did not
parse or resolve, and an opaque choice-parameter collection is a whole-form
refusal — `uh` gained 65 files and lost 199 previously byte-exact ones, with 279
forms no longer written at all. The unmirrored arm now answers `Absent`, exactly
what it answered before it could read anything; the mirrored arm keeps the hard
refusal.

### 3.5 An accounting register's record set (commit 1)

A document's `RegisterRecords` chain into an accounting register resolved the
table path and the uuid-named members and nothing else. The record set is not
the register's `<StandardAttributes>` list: that list carries one `Account`
descriptor, the record set of a correspondence register carries `AccountDr` and
`AccountCr`.

Three forms in the whole stand bind such a record set —
`Documents/ОперацияБух`, `Documents/ОперацияМСФО`,
`Documents/ОперацияМеждународный`, five record sets over two charts of accounts
and both settings of `Correspondence`. Pairing every terminal against the
`<DataPath>` the platform writes for the item that carries it:

* marker terminals `-2` Period (3 sets), `-4` LineNumber (3), `-5` Active (2),
  `-6` AccountDr (3), `-7` AccountCr (3);
* uuid terminals: the marker is the side — 13 `Dr`/`Cr` pairs under `2`/`3` and
  14 plain members under `0`;
* ext dimensions: `{index, 1ab44b24-…}` and `{index, f77758c9-…}` are
  platform-wide families serving all three registers across both charts, spelled
  `ExtDimensionDr1..3` / `ExtDimensionCr1..3` with index `0` spelling `1`.

`Recorder`, `RecordType`, the non-correspondence `Account` and
`PeriodAdjustment` have no evidenced marker and stay unresolved.

### 3.6 `TypeLink` (commits 4 and 8)

Two independent gaps. The binding was audited at the unprefixed record length
alone, so a prefixed `InputField` was written with no `<TypeLink>`; and the
frame's trailing member — the ext-dimension index the linked type is taken from
— was admitted only as `0` or `1`. Over the eight stand corpora the platform
writes 207 `0`, 5 `1`, one `2` and one `3`; the two above the bound are
`Субконто2` and `Субконто3` of
`Catalogs/ПараметрыУчетаФИРСБУ/Forms/СчетаУчетаДокумента`, and the bound dropped
their whole element, not just the index. It is read as the canonical decimal it
is.

### 3.7 A prefixed document field's geometry (commit 7)

`form_document_field_geometry_options` read the bag at the absolute slot 39.
A prefixed record holds it at 40, so a prefixed document field found no bag and
lost `<Height>`, `<Width>`, `<MaxHeight>`, `<MaxWidth>` and its font —
`Catalogs/РасширенияПанелиНалоговогоМониторинга/Forms/ФормаЭлемента` writes
`<Height>` on both of its `TextDocumentField` items.

## 4. Measured gate, exact-set difference against `$D/baselines/19a2f2f`

| key | exact base | exact now | gained | broken | extra | missing |
|---|---:|---:|---:|---:|---:|---:|
| `uh` | 139 169 | 139 388 | **+219** | 0 | 0 | 69 → 69 |
| `do` | 25 332 | 25 338 | **+6** | 0 | 0 | 14 → 14 |
| `mdm` | 162 | 163 | **+1** | 0 | 0 | 0 → 0 |
| `ut` | 50 898 | 50 898 | 0 | 0 | 0 | 0 → 0 |
| `ssl` | 12 701 | 12 701 | 0 | 0 | 0 | 0 → 0 |
| `sslbase` | 9 617 | 9 617 | 0 | 0 | 0 | 0 → 0 |
| `ws` | 29 | 29 | 0 | 0 | 0 | 0 → 0 |
| `wms` | 226 | 226 | 0 | 0 | 0 | 0 → 0 |

`uh` missing holds at 69 by an exchange, not by standing still:
`DataProcessors/НастройкаПравилИмпортаОбъектовADO/Forms/Форма` is written for the
first time (and its `Items/…/ValuesPicture.png` with it), while
`Documents/ВходящийДокументСЭДОФСС/Forms/ФормаВыбораТребования` — a file that
differed before, never an exact one — is now refused outright. Its cause is
named and open, see §6.

`cargo test --lib` 2 357 passed / 33 failed, the 33 names byte-identical to
`$D/baselines/19a2f2f/fail-base.txt`. `cargo test -p ibcmd-schema` 111/0.
`bundled9.sh` 9/9. `cargo fmt --check` and `git diff --check` clean.

## 5. `uh` form bodies after the wave: 696 differing files

```
  105  Field                                  35  DataPath
   27  CommandName                            24  Field UseAlways
   21  AdditionalColumns Column Columns …     20  Command
   15  ExcludedCommand                        15  TypeLink xr:DataPath xr:LinkItem
   13  Event Events                           11  v8:content v8:item v8:lang
   10  ChoiceParameterLinks …                 10  CommandSet ExcludedCommand
    9  SettingsStorage                         9  PagesRepresentation
    8  GroupList                               7  BackColor
    7  UseForFoldersAndItems                   7  ChoiceParameters …
    7  ScalingMode
```

Files whose whole diff sits on one item kind and one property set, the shape a
wave is nursed by:

```
  105  owner=Attribute              Field
   27  owner=Button                 CommandName
   24  owner=Attribute              Field UseAlways
   16  owner=InputField             TypeLink xr:DataPath xr:LinkItem
   15  owner=Command                Command
   15  owner=LabelField             DataPath
   11  owner=InputField             DataPath
   11  owner=SpreadSheetDocumentField Event Events
   10  owner=InputField             ChoiceParameterLinks …
```

## 6. Named and open

* **`Attribute`/`Field`, 129 files.** The `~` marker of a dynamic list's
  available-fields collection, the research task
  `EVIDENCE-form-bodies.md` §9 already states. Nine occurrences in the whole
  stand additionally merge two spellings into one element
  (`~Список.Code~Список.Код`, five distinct values, all in `uh`), which nothing
  in the reader models.
* **`Documents/ВходящийДокументСЭДОФСС/Forms/ФормаВыбораТребования`.** The
  wrapper-`35` normalization brings the record's `ChoiceList` into the reader's
  reach for the first time, and its value is a design-time `v8:ValueList`
  (`{"#",4772b3b4-…,{6,<element type uuid>,0,0,{0},{"Pattern"},0,-1}}` under a
  nil identifier pair) that
  `parse_form_choice_list_item_inner` has no arm for. An unreadable choice list
  is a whole-form refusal by design, so the file is refused honestly rather than
  written with an invented element; closing the `v8:ValueList` value kind closes
  it.
* **`SpreadSheetDocumentField`/`Event Events`, 11 files**, and
  `InputField`/`TypeLink`, 16: both are property readers whose owning field
  schema still declines the record; the layout join says which shape, the byte
  pass has not been done.
