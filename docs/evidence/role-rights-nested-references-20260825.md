# Role Rights: nested references, dangling references, condition bytes, 20260825

Status: closes the three residual defect classes left by
`role-rights-top-level-default-suppression-20260825.md` on ERP Управление
холдингом 3.2.12.6, plus one more found while measuring them. Branch base
`e60c978` (itself on `41808c3`).

After the two `setForNewObjects` / `setForAttributesByDefault` suppression
rules landed, 27 `Roles/<Name>/Ext/Rights.xml` files still differed on the
`uh` gate and 5 more were `missing`. This pass leaves **11 differing**, all of
them on one newly-isolated cause that is *not* closed here (see "What is
left").

## Measured result

`zsh $D/kit/run.sh uh <worktree> <out>`, exact-set difference against the same
tree exported from the branch base:

| | exact | differing | missing | extra | differing `Roles/*/Ext/Rights.xml` |
| --- | ---: | ---: | ---: | ---: | ---: |
| base `e60c978` | 120,434 | 18,475 | 1,502 | 64 | 27 |
| after | 120,453 | 18,456 | 1,502 | 64 | **10** |
| Δ | **+19** | **−19** | 0 | 0 | **−17** |

The 19 files that became exact: 17 `Roles/*/Ext/Rights.xml`, plus
`AccumulationRegisters/ОперацииБюджетов/Forms/ФормаСписка/Ext/Form.xml` and
`AccumulationRegisters/ПланированиеПотребностей/Forms/ФормаСписка/Ext/Form.xml`,
which fell out of §2 (the two forms name register attributes the index was
missing).

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

## What is left

**11 differing role Rights files, one cause, not closed here.** Every one of
them differs only by objects native prints and we do not — 0 extras, 0 value
differences, 0 ordering differences — and all of those objects are commands:

```
DataProcessor.ГенерацияМоделиОтчетностиКИК.Command.МодельОтчетностиКИК
DataProcessor.ГенерацияМоделиОтчетностиКИК.Command.ФормыСбораДанныхИНалоговыеРегистрыКИК
DataProcessor.ЗадачиИОповещенияТекущегоПользователя.Command.МоиЗадачи
DataProcessor.ЗакрытиеПериодаМСФО.Command.ЗакрытиеПериодаМСФО
DataProcessor.РасчетРасхожденийВГО.Command.РасчетРасхожденийВГО
DataProcessor.ТрансформационнаяТаблица.Command.ТрансформационнаяТаблица
FilterCriterion.ДокументыВНАПоОснованию.Command.ДокументыСобытияПоОснованию
InformationRegister.ВерсииОбъектовДляЕИС.Command.РедактироватьВерсииДляЕИС
Report.АнализПоставок.Command.АнализПоставок
Report.ПланФактныйАнализЗакупок.Command.ПланФактныйАнализЗакупок
```

Root cause is an index gap of the same shape as §2, and the trail is already
laid:

* The right is in the blob. `Roles/ЧтениеДанныхМСФОУХ`'s blob carries
  `{1, 303b322e-b0c5-4f51-b9bd-b6cd380652a8, 0, 0}` with `{0, aa6448f2-…(View), 1}`;
  `303b322e-…` is `DataProcessor.ЗакрытиеПериодаМСФО.Command.ЗакрытиеПериодаМСФО`
  per `DataProcessors/ЗакрытиеПериодаМСФО.xml`.
* The uuid is missing from `object_refs`, so before §3 it printed as a bare
  uuid and now it is dropped. Either way the object is absent; §3 did not
  cause this and does not hide anything that was previously correct.
* Once named, it renders: `role_rights_for_xml`'s nested branch treats
  `…Command.<Name>` as an action-like category and always prints, which is
  what native does (`View`/`true`).
* **The gap is the command entry's own shape, not its owner kind.**
  `nested_command_headers_for_owner_from_text` accepts a header only when
  `is_offset_inside_metadata_object_code(text, marker_start, 9)` holds, i.e.
  when the entry's payload block is `{9, …}`. Two shapes exist:

  | shape | payload block | header wrapper | example |
  | --- | --- | --- | --- |
  | recognized | `{9, {4,0,…}, 3, {1,"ru",…}, 1, …, {3, {1,0,uuid}, "Name", {1,"ru",…}, "", 0, 0, 000…, 0}, 0,0,0}` | `{3,` | `DataProcessors/ПанельАдминистрированияУХ` (11 commands) |
  | missed | `{8, {4,0,…}, 3, {0}, 1, …, {2, {1,0,uuid}, "Name", {1,"ru",…}, "", 0, 0, 000…}, 0, 0}` | `{2,` | `DataProcessors/ЗакрытиеПериодаМСФО` (1 command) |

  Verified by replaying the containment check at the command header's offset:
  `ПанельАдминистрированияУХ`'s first three commands are each inside a `{9,`
  span (markers at 744/1094/1439, spans opening at 633/983/1328), while
  `ЗакрытиеПериодаМСФО`'s single command header (marker at 6455) has **no
  `{9,` anywhere in the element**.

  Both shapes occur under the same collection families
  (`45556acb-826a-4f73-898a-6025fc9536e1` for a data processor,
  `e7ff38c0-ec3c-47a0-ae90-20c73ca72246` for a report — `Reports/АнализПоставок`
  is the `{8,` shape and `Reports/ОСВМСФО` the `{9,` one), so the collection
  uuid is not the discriminator either.

  Do **not** simply add `8` alongside `9`: code 8 already means
  `Resource` / `TabularSection.Attribute` in `standalone_child_reference`, and
  one of the ten missing objects is on an `InformationRegister` owner, where
  both readings are live. The measurement that would settle it is the one §2
  used: gate the code-8 acceptance on the header being inside a *command
  collection* span, identified by family uuid — `45556acb-…` (data
  processor), `e7ff38c0-…` (report), `a49a35ce-120a-4c80-8eea-b0618479cd70`
  (document journal), `4c7fec95-d1bd-4508-8a01-f1db090d9af8` (chart of
  accounts), and whatever `FilterCriterion` and `InformationRegister` use,
  which still has to be extracted. Then re-derive over the corpus that the
  headers inside those spans are exactly the `<Command>` uuids the owners'
  native XML declares, with no `Resource` or attribute among them.

Also unresolved and untouched: **5 role Rights files never reach the
exporter at all** — `БазовыеПраваБПУХ`,
`ДобавлениеИзменениеБюджетированиеИОтчетность`,
`ИспользованиеПлатежногоКалендаряУХ`, `ЧтениеБюджетированиеИОтчетность`,
`ЧтениеВекселей`. Their storage entries are reported `opaque` with "no legacy
family decoder recognized this storage entry", which is a decoder gap
upstream of everything in this document.

## Gates

| gate | result |
| --- | --- |
| `uh`, exact-set vs branch base | broken 0 (+19 exact) |
| `ws` / `wms` / `mdm` / `sslbase` / `ssl`, measured before → after every step | broken 0 each, new 0 |
| `ut` | broken 1 vs the reference, see below |
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
`ssl` (those three were measured before and after every step in this pass and
never moved). No `ut` baseline was exported from the branch base, so this one
is reported by class rather than by before/after. It cannot be this pass's
doing on the code either: the whole diff can change only (a) the bytes of
`Roles/*/Ext/Rights.xml` and (b) names of the shape
`AccumulationRegister.<X>.Attribute.<Y>` wherever `object_refs` is read — and
that template contains zero `AccumulationRegister` references of any kind.

The temporary `IBCMD_ROLE_SLOT_PROBE` used to recover the slot codes (it
renamed unresolved nested references to `<owner>.PROBE.k<kind>s<slot>` so the
codes could be read out of a full `uh` export) is not in any commit.
