# ERP УХ form item-tree loss: the record-revision map, 20260825

Status: measurement, no fix shipped. Characterizes the `uh` bucket of
`*/Forms/*/Ext/Form.xml` documents whose diff against native touches 15 or
more distinct XML tags at once -- 531 files of the 2 661 differing form
documents under that path shape, i.e. not a targeted property gap but
wholesale loss of the form's item tree. Measured against a
byte-for-byte compare of the platform-native `uh` tree
(`$D/cap/uh-r1/src`, 140 411 files) at base `d0457a6`; the run reproduces
`$D/base789/uh.parity.json` exactly (`exact` sets identical, 120 592 /
18 487 / 1 332 / 64).

**Answer up front.** The bucket is *not* homogeneous, but it has one
dominant, exactly-attributable cause, and that cause is a fourth
occurrence of this codebase's most common defect class (doctrine point 7 --
hardcoded arity whitelists instead of reading declared counts), in a code
path none of the three previous occurrences touched. The platform declares
each form item's *class* with a uuid written immediately before the item's
record; `form_child_item_tag` ignores that uuid and dispatches on the
record's own leading member -- which is not a type code at all but the
record's **declared member count**, and therefore changes value between
schema revisions of the same class. ERP УХ ships two revisions of four
classes; the reader learned one of each and silently drops every record
carrying the other, taking that record's entire subtree with it.

## Corpus accounting

```
native  uh  */Ext/Form.xml                       12 997
  emitted (reached the item reader)              12 215   9 431 exact / 2 784 differing
  never emitted (missing)                           782
```

The 782 never-emitted forms are a *different* population: they fail
upstream in `module_blob::parse_form_body_plain`, before any item is read.
The 12 215 forms below are exactly the ones that passed that container
check.

## Method

1. Temporary probe (`IBCMD_FORM_LAYOUT_DUMP`/`IBCMD_FORM_LAYOUT_PATHS`,
   added and removed in this pass -- see the commit pair at the end) dumping
   every emitted form's raw `body.layout` plus a TSV census of its root
   members, for one full `cf export` run against the real `uh` `1cv8.cf`.
   Verified inert: the run's `exact` set is identical to `$D/base789`'s.
2. A form layout encodes a child-item list as
   `<count>, (<class-uuid>, <record>) * count`. Walking every layout for
   that construction, at every nesting depth, yields 459 639 item records
   across the six gate corpora that have forms at all (`ws` has none), each with a class uuid and a leading member
   this document calls the *wrapper*.
3. The class-uuid-to-XML-tag correspondence was not assumed: for every
   byte-exact form, each record's own item id (first member of its
   `{<id>,<form-uuid>}` identity slot) was looked up in the native XML by
   `id="..."` and the element name tallied. Six class uuids exist in the
   whole corpus, and each resolves to exactly one XML element family.
4. Bucketing of the 531 is by *construction*, not by file: which
   revisions a form's layout carries, and what became of the items those
   records name.

## The six item classes and their revisions

Census over all 12 215 `uh` layouts, with the byte-exact/differing split
of the forms each revision appears in:

```
class uuid                             XML element family        wrapper   records   appears in
143c00f7-a42d-4cd7-9189-88e4467dc768   Table                          55     9 390   exact + differing
                                                                      54       296   differing only
3d3cb80c-508b-41fa-8a18-680cdf5f1712   LabelDecoration/Picture…       12    42 445   exact + differing
                                                                      11       102   differing only
77ffcc29-7f2d-4223-b22f-19666e7250ba   InputField/LabelField/…        37   118 775   exact + differing
                                                                      35     1 179   exact + differing
                                                                      34     2 104   differing only
a9f3b1ac-f51b-431e-b102-55a69acdecad   Button                         31    21 189   exact + differing
                                                                      30       111   differing only
cd5394d0-7dda-4b56-8927-93ccbe967a01   UsualGroup/Page/Pages/…        22    92 480   exact + differing
c5259a1d-518a-4afd-b98d-0176027e4feb   Search…/ViewStatusAddition      5       169   exact + differing
```

`form_child_item_tag` (`src/mssql_dump/form_body.rs:16334`) knows the
wrappers `22`, `12`, `31`, `34`, `35`, `37`, `48`, `5`, `6`, `73`, `55` and
returns `None` for anything else. Three of the four short revisions --
`54`, `30`, `11` -- are simply absent from it. The fourth, `34`, is present
but wired to the **wrong** arm: it is listed as `"31" | "34" => Button`,
while the class uuid on all 2 104 of its records is the *field* class.

## The wrapper is a declared member count, not a type code

`field_count - wrapper`, over all item records of all seven corpora:

```
Button       {21: 36 313,  22:  1 143}
Decoration   {24: 61 874,  25:  1 007}
Field        {22: 173 845, 23: 20 954}
Addition     {19:    411,  20:      2,  21: 1}
Group        spread (variable child-item tail)
Table        spread (variable column tail)
```

For every fixed-arity class the difference takes exactly two values, `k`
and `k+1` -- the same optional-trailing-member split `ab58c3f` documented
for the `{3,…}`/`{2,…}` metadata header wrappers. The wrapper therefore
*is* the record's own declared length, and a revision that drops one member
writes a wrapper one lower:

```
Button       wrapper 31 -> 52/53 members     wrapper 30 -> 51/52
Decoration   wrapper 12 -> 36/37             wrapper 11 -> 35/36
Field        wrapper 37 -> 59/60   35 -> 57/58   34 -> 56/57
```

The slot layout does **not** shift with the revision. Reading slots 4/5/6
of every record shows the two shapes each class carries (`slot4=0,
slot5=<name string>` and `slot4=1, slot5=<UserVisible-common tuple>,
slot6=<name string>`) occurring under the long and the short wrapper
alike, in the same proportions. The short revisions are the same records
with one member fewer, not a different encoding -- which is why the
current readers get the *right* answer for every slot they are allowed to
reach, and why the whole defect is a gate, not a decode.

## Where the item is lost

`parse_form_child_item_with_metadata_owners` returns `None` the moment
`form_child_item_tag` does. Its caller
`parse_form_child_item_pairs` (`form_body.rs:8877`) then finds
`items.len() != count` for that candidate position and keeps looking; when
no position yields a complete list it returns `None`, and both call sites
--- `extract_form_child_items` (`form_body.rs:7300`) and
`parse_form_auto_command_bar_fields` (`form_body.rs:2862`) --- turn that
into `.unwrap_or_default()`, an empty item vector. This is a *silent*
default, not a typed refusal (doctrine points 2 and 6): the form is still
written, still passes every writer preflight, and the loss is invisible to
the report. The 449 affected forms are all `differing`; none is `missing`,
none is `opaque`, none carries a diagnostic.

Fate of the 2 613 records carrying an unlearned revision, resolved by
matching each record's own `name`+`id` against the native and our XML:

```
Table:54       296   193 dropped    103 dropped (id also names an <Attribute>)
Button:30      111   106 dropped      5 dropped (id also names a <Command>)
Decoration:11  102   102 dropped
Field:34     2 104  1 848 dropped    255 emitted as <Button>
                                      (195 InputField, 41 LabelField,
                                       16 CheckBoxField, 3 SpreadSheetDocumentField)
                                      1 native identity unresolved
```

2 358 items vanish outright with their subtrees; 255 are emitted under the
wrong element name because wrapper `34` is routed to the Button arm. Over the
531-file bucket our `(element, id)` set is a strict subset of native's in
458 files and equal in 45; the remaining 28 diverge both ways, which is the
signature of the wrapper-`34` misclassification -- the id survives, the
element name does not.

## Minimal specimen, with a working twin

`Enums/СрочностьЗадолженности/Forms/ФормаВыбора` (differing) against
`Enums/ВариантыОбеспечения/Forms/ФормаВыбора` (byte-exact). Both forms are
a single `<Table>` under the form root; both carry the same class uuid
`143c00f7-a42d-4cd7-9189-88e4467dc768` at the same pair position:

```
failing  … [25] 1   [26] 143c00f7-…   [27] {54,…}  n=119   root discriminator 49
working  … [23] 1   [24] 143c00f7-…   [25] {55,…}  n=103   root discriminator 50
```

Member for member the two records hold the same things in the same
order, modulo the short revision's one-member shift and the
`{0,{0,{"B",1},0}}` `UserVisible`-common tuple the failing one carries at
slot 5 (the working one leaves it out and puts its name there). Native
writes a full `<Table>` with a `<ContextMenu>`, an `<AutoCommandBar>`, an
`<ExtendedTooltip>`, all three `…Addition` items and a `<LabelField>`
child; we write the form with its `<VerticalScroll>`, its correct
`<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>` and its complete
`<Attributes>` block including the whole `DynamicList` `ListSettings`
document -- and no `<ChildItems>` at all. 103 native lines become 34.

## Decomposition of the 531-file bucket

Grouped by what happened to the item tree, then by whether the form
carries an unlearned revision:

```
                                              files   carries an unlearned revision
A  item tree entirely absent from our output    115   115  (100 %)
B  item tree present but truncated              345   204  ( 59 %)
C  item tree intact, properties differ           71     4  (  6 %)
```

Sub-bucket A is homogeneous and fully explained. Its families are
`Catalogs` 76, `DataProcessors` 14, `Enums` 10, `InformationRegisters` 8,
`Documents` 3, `FilterCriteria` 2, `AccumulationRegisters` 1, `Reports` 1;
its form kinds are dominated by `ФормаСписка` (33) and `ФормаВыбора` (32).
All 115 lose `ChildItems`, `ContextMenu`, `DataPath`, `ExtendedTooltip` and
`Type`; 113 of 115 lose `Table`, `SearchStringAddition`,
`ViewStatusAddition`, `SearchControlAddition` and `AdditionSource` -- the
signature of one dropped root `Table` taking its whole subtree with it.
Form `<Attributes>` and `<Commands>` counts match native in **all 531**
files of the bucket, in every sub-bucket: nothing outside the item tree is
affected.

The defect is not confined to this bucket. 449 `uh` forms carry at least
one unlearned revision; 323 of them are in the 531, the other 126 land at
5-14 changed tags because the lost record sat deeper in the tree. Not one
of the 9 431 byte-exact forms carries any -- an exact-set result, not a
sample. By family: `Catalogs` 226, `InformationRegisters` 73,
`DataProcessors` 51, `Reports` 33, `Documents` 32, `CommonForms` 17,
`Enums` 11, `AccumulationRegisters`/`ExchangePlans`/`FilterCriteria` 2 each.
Files touched per revision: `Field:34` 415, `Table:54` 286, `Button:30` 62,
`Decoration:11` 38.

## This is not the `type marker 4` class

The 179-file `Form body does not start with type marker 4` bucket that
`uh-missing-root-cause-map-20260825.md` records as untouched is a
different code path and a different parity class. That check lives in
`module_blob::FormBodyContainer::parse` (`src/module_blob.rs:19671`) and
tests the *container* discriminator of the whole compiled body,
`{4, <layout>, "<module text>", …}`. Failing it returns `Err`, the source
asset is never written, and the file lands in `missing`. Everything in this
document happens strictly downstream: `parse_form_body_plain` already
succeeded, `validate_form_body_layout` already accepted the layout root
(it admits *any* numeric marker), and the form is written -- wrong. The two
sets are disjoint by construction, and the 12 215 layouts measured here are
by definition the ones that passed the marker-4 check.

Nor is the layout's own root discriminator the gate. `uh` roots at both
`49` (1 424 forms) and `50` (10 791), and both appear on either side of the
`<ChildItems>` split; `extract_form_child_items` does not consult the root
discriminator at all -- it scans every root field for a
`<count>,(uuid,record)*` run. Root `49` is nonetheless correlated
(193 of the 279 no-`ChildItems` forms root at `49`) because the same
configuration areas that ship short item revisions also ship the `49`
root.

## No fast corpus exercises this

Same census over the other six gate corpora:

```
        forms   Table      Decoration   Field           Button    Group   Addition
ws          0   --
wms         5   55            12        37              31        22      --
mdm        11   55            12        37, 35          31        22      --
sslbase   909   55            12        37              31        22       5
ssl     1 162   55            12        37              31        22       5
ut      5 201   55            12        37              31        22       5
uh     12 215   55, 54        12, 11    37, 35, 34      31, 30    22       5
```

Every short revision is `uh`-only; `35` occurs only in `mdm` (17 records)
and `uh`. There is no 0,2 s / 6 s / 16 s iteration loop available for this
defect -- `uh` (~12 min per cycle on this host, plus ~7 GB of output tree)
is the only oracle, and a fix has to be designed against dumped bytes
rather than discovered by trying variants.

## The gate, and what shape it is missing

Two gates drop the item, and a third layer would still mangle it if the
first two were opened.

1. `form_child_item_tag` (`form_body.rs:16334`) -- a `match wrapper`
   over eleven string literals. It needs the *class uuid* the platform
   writes immediately before the record; that uuid is already in hand at
   the pair-scanning site (`parse_form_child_item_pairs`,
   `form_body.rs:8877`, currently validates it with `parse_non_zero_uuid`
   and then throws it away). Six uuids, one element family each, and the
   per-family sub-discriminator the function already reads from slot 5
   stays exactly as it is. This also repairs `34`, which is not an unknown
   wrapper but a wrong arm: routing by uuid puts it in the field family
   where its 2 104 records belong.
2. `parse_form_child_item_name` (`form_body.rs:16439`) -- a per-wrapper
   table of name slots (`"73"|"55" => [5]`, `"31"|"34" => [5,6]`,
   `"35"|"37"|"48" => [6,7]`, else `[6]`). Same fix shape: the slot follows
   the class, and the `slot4 == "1"` conditional-prefix marker the record
   itself carries already tells the reader whether the name sits one slot
   further out. `FormConditionalTableSchema::from_raw_layout`
   (`src/form_schema.rs:4638`) reads that marker correctly today and then
   throws the answer away behind `wrapper == "55" && field_count >= 100 &&
   (field_count - 100) % 2 == 0`.
3. `src/form_schema.rs` carries 35 hardcoded arity comparisons over 16
   distinct literals, and every one of them is a *canonical* revision's
   member count:
   `field_count == 52` (Button 31), `== 59` (Field 37), `== 36`
   (Decoration 12), `== 24` (Addition 5), `>= 99`/`(field_count - 99)`
   and `>= 100`/`(field_count - 100)` (Table 55), `>= 30`/
   `(field_count - 30)` (Group 22). A short-revision record admitted past
   gates 1 and 2 would reach these with every count one lower and be
   refused property by property -- so the items would come back but their
   `Visible`, `TitleLocation`, `DisplayImportance` and the rest would not.
   The measured invariant above (`field_count - wrapper` takes exactly two
   values per class) is what these literals should be expressed in terms
   of; that is the same `enclosing_counted_block_start` move `ab58c3f` and
   `0575505` made for the metadata header wrappers, one layer up.

Because of layer 3 this is not a two-line change, and admitting the
revisions without it would move forms from one wrong output to another
wrong output while the exact-set gate reports `BROKEN=0` either way. It
needs its own pass, per class, with the dumped bytes of a short-revision
record and a long-revision twin side by side -- the specimen pair above is
the smallest one in the corpus and is a good starting point. Nothing here
should be attempted against a whitelist broadened by guessing: three of the
four short revisions were invisible until the class uuid was read, and
there is no reason to assume `uh` is the last configuration to ship one.

## What this bucket does *not* explain

- **141 files** in sub-bucket B carry no unlearned revision and still lose
  items: `ExtendedTooltip` (1 693 items / 141 files), `Button` (1 022 /
  124), `ButtonGroup` (239 / 68), `Popup` (190 / 85), `CommandBar` (98 /
  63) -- command-bar button trees, at shallow drop fractions (65 of the 141
  lose under 10 % of their items). Families `Reports` 37, `Documents` 37,
  `DataProcessors` 30, `Catalogs` 26. Their records all carry canonical
  wrappers, so this is a separate cause inside the button/group readers,
  not a revision gap. Not investigated here.
- **67 files** in sub-bucket C keep the whole item tree and differ only in
  properties: `Events`/`Event` (38 files), `TitleLocation` (31),
  `AutoMaxWidth` (28), `ChoiceButton` (28), `ListChoiceMode` (23),
  `TextEdit` (19) -- the extended `InputField` option block -- plus 16
  files where we write a `<ToolTip>` where native writes a `<Title>`. A
  third, unrelated class. Not investigated here.
- The `Shortcut` bucket (615 of the 1 276 single-tag differing forms) and
  the `AutoURL` bucket (195) are the known host-dependent and
  sibling-task-owned classes respectively, out of scope by prior handoff
  (`docs/evidence/host-dependent-export-2214-20260823.md`).

## Reproducing

`$D/xeno/` holds this pass's derived data: `uh.parity.json` (the run this
document measures), `uh-forms.json` (per-file changed-tag buckets),
`uh-big-shape.json`, `uh-big-items.json` (item-set relations),
`uh-attrib.json` (per-file revision attribution) and the scripts
(`bucket.py`, `shape.py`, `items.py`, `pairs.py`, `idmap.py`, `attrib.py`,
`slots.py`). The probe that produced the layout dumps is in commit
`53240d0` and removed again in the commit that follows this document.
