# Role Rights: nested references, dangling references, condition bytes, 20260825

Status: closes the three residual defect classes left by
`role-rights-top-level-default-suppression-20260825.md` on ERP Управление
холдингом 3.2.12.6, plus two more found while measuring them. Branch base
`e60c978` (itself on `41808c3`).

**`Roles/*/Ext/Rights.xml` differing on the `uh` gate: 27 → 0.** Five role
Rights files remain `missing` for an unrelated reason (see the end).

After the two `setForNewObjects` / `setForAttributesByDefault` suppression
rules landed, 27 `Roles/<Name>/Ext/Rights.xml` files still differed on the
`uh` gate and 5 more were `missing`. All 27 are closed here, by five rules —
four found from the role diffs themselves, the fifth (§6) turning out to be
a command-indexing defect well outside roles.

## Measured result

`zsh $D/kit/run.sh uh <worktree> <out>`, exact-set difference against the same
tree exported from the branch base:

| | exact | differing | missing | extra | differing `Roles/*/Ext/Rights.xml` |
| --- | ---: | ---: | ---: | ---: | ---: |
| base `e60c978` | 120,434 | 18,475 | 1,502 | 64 | 27 |
| after §1–§5 | 120,453 | 18,456 | 1,502 | 64 | 10 |
| after §6 | 120,492 | 18,434 | 1,485 | 64 | **0** |
| Δ | **+58** | **−41** | **−17** | 0 | **−27** |

The 58 files that became exact are not all roles — §2 and §6 both reach
outside `Roles/`:

| what | count |
| --- | ---: |
| `Roles/*/Ext/Rights.xml` | 27 |
| `*/Commands/<Name>/Ext/CommandModule.bsl` (were `missing`, §6) | 17 |
| `Subsystems/**/Ext/CommandInterface.xml` (§6) | 10 |
| `Documents/ПрограммаЗакупок/Forms/{ФормаВыбора,ФормаСписка}/Ext/Form.xml` (§6) | 2 |
| `AccumulationRegisters/{ОперацииБюджетов,ПланированиеПотребностей}/Forms/ФормаСписка/Ext/Form.xml` (§2) | 2 |

**Broken: 0** on every step, measured as `base.exact − new.exact`, never as a
counter.

Note on `$D/base789/uh.parity.json`: this branch is a different lineage from
the reference (the reference tree carries MXL and module fixes that
`41808c3` predates), so `reference.exact − ours.exact` is 7,319 files here —
`.bsl` modules and `Ext/Template.xml` spreadsheets, none of them a role.
That gap is identical before and after this pass (7,319 → 7,319, and the two
sets are equal), i.e. nothing in this pass moved it. The honest
no-regression measurement on this lineage is against the branch base, and it
is 0.

## 1. Condition text: one `\r\n` collapse, not two

`format_role_rights_xml` ran a restriction condition through
`normalize_role_condition_text` (`\r\n`→`\n`, then `\r`→`\n`) and *then*
through `escape_xml_element_text`, which already does `\r\n`→`\n` for every
element text it writes. Two collapses plus a bare-`\r` pass.

The platform does exactly one. Measured by extracting role Rights storage
elements (`<role uuid>.0`) with `cf extract` and comparing the inflated bytes
against the native `Ext/Rights.xml`:

| role | bytes in blob | native prints |
| --- | --- | --- |
| `ЧтениеПеремещенийОС` | `\r\n` ×3,196, no `\r\r\n` | bare `\n` |
| `ПросмотрТрансляции` | `\r\r\n` ×3,219 | `\r\n` ×3,219 |

One replacement maps both (`"\r\r\n"` → `"\r\n"` — Rust's `str::replace`
consumes only the trailing pair). A second collapse, or the lone-`\r` pass,
flattens the second case to `\n\n`; that is exactly what our output showed —
`ПросмотрТрансляции` was byte-for-byte the same *length* as native (276,610
both) with every one of its 3,219 condition line breaks written `\n\n`
instead of `\r\n`.

Corpus scope: of 2,118 roles, 1,149 have a condition at all and exactly **4**
carry `\r` in a native condition — all four as `\r\n`. No native condition
anywhere prints a lone `\r`, so removing the bare-`\r` pass has nothing to
contradict it.

Closed: `Roles/ПросмотрТрансляции`, `Roles/БазовыеПраваБюджетированиеИОтчетность`.
Commit `738a16b`.

## 2. Accumulation register attributes were missing from the reference index

`standalone_child_reference` had no arm for an accumulation register's
attribute list, so every `AccumulationRegister.<Register>.Attribute.<Name>`
was absent from `object_refs` and the role printed the object under its bare
uuid.

The list's family uuid is `b64d9a42-1642-11d6-a3c7-0050bae0a776`, adjacent to
the two the code already knew: `…a41` resources, `…a43` dimensions. Measured
over nine extracted `AccumulationRegisters/*` storage elements: the headers
inside `…a42` spans are exactly the **74/74** uuids their native XML declares
as `Attribute` — none missing, no `Dimension` or `Resource` inside any such
span, and all 74 satisfy the same code-2 containment the existing
AccountingRegister arm requires.

Commit `2e6859e`.

## 3. A dangling reference is dropped, not printed as a uuid

`role_object_ref_name` fell back to the uuid string when `object_refs` missed.
The platform prints no `<object>` at all for such a reference.

Measured: 578 uuids across 13 role Rights blobs are declared **nowhere** in
the native source tree — checked against the tree's full `uuid="..."`
inventory, 187,101 distinct uuids — and native prints no `<object>` for a
single one of them. The platform admits the same thing about the same uuids
elsewhere: `Subsystems/.../Ext/CommandInterface.xml` carries them as
unresolved `0:<uuid>` command names.

The measurement had to be done in this order. Before fix 2, 623 uuids were
unresolvable, of which 45 were real (the accumulation-register attributes
above, which native *does* print, by name) and 578 dangling. Dropping on
"unresolvable" alone would have been a rule resting on a defect. With the
index gap closed first, unresolvable means dangling on this corpus.

## 4. Slot tables for nested standard attributes

`role_standard_attribute_descriptor` covered `Catalog`,
`ChartOfCharacteristicTypes`, `Document`, `ExchangePlan`, and the four
register kinds' shared rows. `DocumentJournal`, `Task` and `ChartOfAccounts`
fell through to `_ => None`, as did an accounting register's `Account`,
`RecordType` and `ExtDimension<n>` / `ExtDimensionType<n>`. A reference whose
descriptor is `None` was named after its owner, so a whole group of standard
attributes collapsed onto one name.

### How the tables were measured

A metadata object's storage element declares its standard attributes in one
list, each entry `{slot[,family]},510405d3-2a0c-4fea-960a-7fee59b32f9b`, and
the platform writes that same list — same order, same length — as the
object's `<StandardAttributes>` block in its native XML. Pairing the two by
position gives slot → name directly, and the lengths match exactly in every
object checked (11/11, 12/12, 14/14, 6/6, 8/8).

The method was validated against the two kinds whose tables were already
corpus-hardened:

| element | element slot order | existing table |
| --- | --- | --- |
| `Catalogs/Организации` | `-13 -10 -8 -7 -6 -5 -4 -3 -2` | identical, 9/9 |
| `AccountingRegisters/МСФО` | `-5 -4 -3 -2` → Active, LineNumber, Recorder, Period | identical, 4/4 |

13/13 agreements, no disagreement. (`Document`'s five existing rows pair the
same five slots in the opposite order. Nothing in any corpus distinguishes
the two — see the note below — so they were left exactly as they were.)

### What is observable and what is not

The `order` half is measured separately and directly, from the native
`Roles/*/Ext/Rights.xml` print sequence inside one owner's `<object>` group.
That is what the export actually depends on, and it is the only half the
corpus can see: across **all 10,619** standard-attribute groups in the whole
2,118-role ERP УХ corpus, exactly **one** —
`Catalog.СоответствиеВнешнимИБ` in `Roles/БазовыеПраваБПУХ` — prints two
different right lists inside a group. Every other group prints every member
with an identical right list, so which *name* a slot carries is
unobservable there, while the order they print in always is. The same holds
in the other six configurations, none of which contains a single
`DocumentJournal`, `Task` or `ChartOfAccounts` standard-attribute group at
all.

### The tables

`DocumentJournal` (`DocumentJournals/ДокументыБюджетирования`,
`.../ОперацииСЦеннымиБумагами` agree):

| slot | name | order |
| ---: | --- | ---: |
| −60003 | Type | 1 |
| −101 | Ref | 2 |
| −100 | Date | 3 |
| −7 | Posted | 4 |
| −4 | DeletionMark | 5 |
| −2 | Number | 6 |

`Task` (`Tasks/БюджетнаяЗадача`, `Tasks/ЗадачаИсполнителя` agree): −10
Executed, −9 Description, −8 RoutePoint, −7 BusinessProcess, −5 Ref, −4
DeletionMark, −3 Date, −2 Number (orders 1..8).

`ChartOfAccounts` (`ChartsOfAccounts/МСФО`, `.../Хозрасчетный` agree): −28
PredefinedDataName (1), −17 Order (2), −11 OffBalance (8), −10 Type (9), −8
Description (10), −7 Code (11), −6 Parent (12), −5 Predefined (13), −4
DeletionMark (14), −2 Ref (15). Orders 3..7 belong to the standard tabular
section `ExtDimensionTypes` (slot −12 in family
`28db313d-dbc2-4b83-8c4a-d2aeee708062`, order 3) and its own four standard
attributes (−15 TurnoversOnly 4, −14 Predefined 5, −13 ExtDimensionType 6,
−12 LineNumber 7), which native prints between `Order` and `OffBalance` —
see `Roles/БазовыеПраваУХ`.

`AccountingRegister`: −10 Account (1), −9 RecordType (2), −5 Active (3), −4
LineNumber (4), −3 Recorder (5), −2 Period (6), then the ext-dimension
pairs. Those are generated per ext-dimension slot of the register's chart of
accounts rather than carrying a fixed negative slot: outer
`{n, 03f171e8-326f-41c6-9fa5-932a0b12cddf}`, inner
`{n, 91162600-3161-4326-89a0-4a7cecd5092a}` for `ExtDimension<n+1>` and
`{n, b3b48b29-d652-47ab-9d21-7e06768c31b5}` for `ExtDimensionType<n+1>`,
printing interleaved from order 7. Family split and numbering come from
pairing `AccountingRegisters/МСФО`'s eleven element entries with its eleven
`<StandardAttributes>` names, and again from
`.../МеждународныйБезКорреспонденции`'s twelve; both agree.

`RecordType` is additionally pinned by set difference alone: three registers
print an 11-member group without it and one prints a 12-member group with
it. `ChartOfAccounts.PredefinedDataName` likewise —
`Roles/БазовыеПраваБПУХ` prints a one-member `ChartOfAccounts.МСФО` group
containing only it, and `Roles/НастройкаПравДоступаКЭкземплярамОтчетов`
prints the complementary fourteen.

One pre-existing rule was kept deliberately: a bare (unbraced) integer slot
carries no print order, only a braced `{slot, family}` reference does. Every
slot code in every ERP УХ role Rights blob is braced, so nothing here
measures the bare form; the unit test that codifies it
(`role_rights_blob_resolves_standard_attribute_refs`) stands.

Commit `2efd98a` (with 3).

## 5. An empty restriction-template condition is an empty element

Native writes `<condition/>`; we wrote `<condition></condition>`. One
occurrence in the whole stand — `Roles/ЧтениеКовенантов`, template
`ПоЗначениямРасширенный` — and zero counterexamples: across the role trees
of all seven configurations `<condition/>` appears once and
`<condition></condition>` never. The `restrictionByCondition` condition is
never empty in any of them, so nothing measures its empty form and it was
left as it was.

## 6. The short command entry, `{8, … {2, …}}`

Every one of the ten role Rights files still differing after §1–§5 differed
only by objects native prints and we did not — 0 extras, 0 value differences,
0 ordering differences — and all of those objects were commands.

`nested_command_headers_for_owner_from_text` accepted a nested header as a
command only when `is_offset_inside_metadata_object_code(text, marker_start,
9)` held. A command entry comes in two shapes:

| shape | payload block | header wrapper | example |
| --- | --- | --- | --- |
| recognized | `{9, {4,0,…}, 3, {1,"ru",…}, 1, …, {3, {1,0,uuid}, "Name", …, 0}, 0,0,0}` | `{3,` | `DataProcessors/ПанельАдминистрированияУХ` |
| missed | `{8, {4,0,…}, 3, {0}, 1, …, {2, {1,0,uuid}, "Name", …}, 0, 0}` | `{2,` | `DataProcessors/ЗакрытиеПериодаМСФО` |

**The shape is not what identifies a command; the collection it sits in is.**
Measured by dumping, for every nested header of every metadata row, the
innermost enclosing `{uuid,…}` span, and cross-referencing each header against
the native source tree's own inventory of declared children (139,868 of them,
1,097 `<Command>`):

* all **1,097/1,097** declared commands lie inside one of fourteen spans,
  **none outside**;
* those spans contain **nothing else** — 0 `Attribute`, `Resource`,
  `Dimension`, `TabularSection` or any other declared child, across all
  fourteen;
* each family belongs to exactly one owner kind, and its member count equals
  that kind's command count exactly.

| family uuid | owner kind | commands |
| --- | --- | ---: |
| `45556acb-826a-4f73-898a-6025fc9536e1` | DataProcessor | 426 |
| `4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6` | Catalog | 208 |
| `b544fc6a-2ba3-4885-8fb2-cb289fb6d65e` | Document | 207 |
| `b44ba719-945c-445c-8aab-1088fa4df16e` | InformationRegister | 115 |
| `e7ff38c0-ec3c-47a0-ae90-20c73ca72246` | Report | 82 |
| `a49a35ce-120a-4c80-8eea-b0618479cd70` | DocumentJournal | 19 |
| `d5207c64-11d5-4d46-bba2-55b7b07ff4eb` | ExchangePlan | 12 |
| `7a3e533c-f232-40d5-a932-6a311d2480bf` | BusinessProcess | 10 |
| `f27c2152-a2c9-4c30-adb1-130f5eb2590f` | Task | 8 |
| `0df30176-6865-4787-9fc8-609eb144174f` | ChartOfAccounts | 3 |
| `23fa3b84-220a-40e9-8331-e588bed87f7d` | FilterCriterion | 2 |
| `95b5e1d4-abfa-4a16-818d-a5b07b7d3f73` | ChartOfCharacteristicTypes | 2 |
| `7162da60-f7fe-4d78-ad5d-e31700f9af18` | AccountingRegister | 2 |
| `99f328af-a77f-4572-a2d8-80ed20c81890` | AccumulationRegister | 1 |

Five of the fourteen (InformationRegister, ChartOfAccounts, FilterCriterion,
AccountingRegister, AccumulationRegister) were not known to `mssql_dump` at
all. Re-measured the same way on the rest of the stand: `mdm` 3/3, `sslbase`
102/102, `ssl` 153/153 declared commands inside these families, none outside,
nothing else inside; `ws` and `wms` declare no command.

19 of the 1,097 carry the short shape — 8 DataProcessor, 5 Report, 5
InformationRegister, 1 FilterCriterion. The code-9 arm is kept exactly as it
was and the family gate added as a union, so the rule can only add a name,
never move one.

This turned out to reach well past roles. Besides the ten role files, it made
exact 10 `Subsystems/**/Ext/CommandInterface.xml`, two
`Documents/ПрограммаЗакупок/Forms/*/Ext/Form.xml`, and 17
`*/Commands/<Name>/Ext/CommandModule.bsl` that were not being written at all
(`missing` 1,502 → 1,485) — an unnamed command has no output directory to
write its module into.

Commit `3075b4e`.

## What is left

**No `Roles/*/Ext/Rights.xml` differs on the `uh` gate.** What remains in this
area is a decoder gap upstream of everything in this document: **5 role Rights
files never reach the exporter** — `БазовыеПраваБПУХ`,
`ДобавлениеИзменениеБюджетированиеИОтчетность`,
`ИспользованиеПлатежногоКалендаряУХ`, `ЧтениеБюджетированиеИОтчетность`,
`ЧтениеВекселей`. Their storage entries are reported `opaque` with "no legacy
family decoder recognized this storage entry", so nothing about role rights
is even attempted for them. Their packed sizes (21 KB … 2.2 MB) span two
orders of magnitude, so it is unlikely to be one size-related cause.

## Gates

| gate | result |
| --- | --- |
| `uh`, exact-set vs branch base | broken 0 (+58 exact) |
| `ws` / `wms` / `mdm` / `sslbase` / `ssl`, measured before → after every step | broken 0 each, new 0 |
| `ut`, measured before → after §6 | broken 0, new 0 |
| `bundled9` | 9/9 |
| `cargo test --lib` | 2,237 passed / 33 failed, names identical to `$D/fail-base.txt` |
| `cargo fmt --check`, `git diff --check` | clean |
| instrumentation | removed; only the pre-existing `TEMPORARY_ATTEMPTS`, `IBCMD_DCS_CANDIDATE_OUT`, `dcs_schema` constants remain |

### The two reference-tree caveats

`$D/base789/*.parity.json` was refreshed by the concurrent wave while this
pass ran (`uh`'s reference `exact` moved 120,592 → 127,753 mid-session), so
it reflects work this branch does not carry. On `uh` that shows as
`reference.exact − ours.exact` = 7,369 files — `.bsl` modules and MXL
`Ext/Template.xml`, no role among them — and the *same 7,369 files, as a set*,
are missing at the branch base too. Nothing in this pass moved that number,
which is why the no-regression measurement above is stated against the branch
base.

`ut` shows one file the reference has exact and we do not:
`Reports/СравнительныйАнализПоказателейРаботыМенеджеров/Templates/
СравнительныйАнализМенеджеров/Ext/Template.xml`, an MXL spreadsheet of the
same class as the single pre-existing MXL-template gaps on `ws`, `mdm` and
`ssl`. It is the *same file* before and after §6 — `ut` was exported twice on
this branch, and `reference.exact − ours.exact` is that one file in both runs
— so it is pre-existing here, not this pass's doing. (`ws`, `mdm` and `ssl`
were likewise measured before and after every step and never moved.)

The temporary `IBCMD_ROLE_SLOT_PROBE` used to recover the slot codes (it
renamed unresolved nested references to `<owner>.PROBE.k<kind>s<slot>` so the
codes could be read out of a full `uh` export) is not in any commit.
