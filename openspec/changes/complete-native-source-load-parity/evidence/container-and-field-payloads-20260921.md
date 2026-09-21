# Every container and field payload, read in one sweep

2026-09-21, ERP УХ (`E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native`),
bodies from `F:\ibcmd\lab\form_bodies\Config_inflated`.

The partition test — *which property maps each of its spellings, **and its
absence**, to exactly one stored value over every record* — closed every
remaining container payload and four of the field payloads. The tool is
`F:\ibcmd\lab\tools\partition-container.py`; it finds a record's payload by
position, as the member that follows the `{0,0,0},1` pair every item record
carries before it.

## The usual group, `{29,…}`, 29 members over 61 256 records

24 members carry an XML property:

| slot | property | coding |
|---|---|---|
| 1, 22, 27 | `<Group>` | 1: `Vertical` 0, else 1. 22: absent 2, `Vertical` 0, both horizontals 1. 27: absent 2, `Vertical` 0, `Horizontal` 1, `AlwaysHorizontal` 3 |
| 2 | `<ChildItemsWidth>` | absent 0, `Equal` 1, `LeftWide` 2, `LeftWidest` 3, `LeftNarrow` 4, `LeftNarrowest` 5 |
| 3 | `<Representation>` | `None` 0, `StrongSeparation` 1, absent 2, `NormalSeparation` 3 |
| 4 | `<ShowTitle>` | `false` 0, absent 1 |
| 5 | `<TitleDataPath>` | the data-path block |
| 6 | `<Format>` | localized |
| 9 | `<BackColor>` | the colour block |
| 10, 24, 28 | `<Behavior>` | 10: `Usual` 0, else 1. 24: `Usual`/absent 0, `Collapsible` 1, `PopUp` 2. 28: `Usual` 0, `Collapsible` 1, `PopUp` 2, absent 3 |
| 11 | `<ControlRepresentation>` | absent 0, `Picture` 1 |
| 12 | `<Collapsed>` | |
| 13 | `<ShowLeftMargin>` | `false` 0, absent 1 |
| 14 | `<CollapsedRepresentationTitle>` | localized |
| 15, 16 | `<HorizontalSpacing>`, `<VerticalSpacing>` | absent 0, `None` 1, `Half` 2, `Single` 3, `OneAndHalf` 4, `Double` 5 |
| 17, 18 | `<HorizontalAlign>`, `<VerticalAlign>` | absent **3** |
| 19 | `<ThroughAlign>` | `Use` 0, `DontUse` 1, absent 2 |
| 20 | `<ChildrenAlign>` | absent 0, six spellings 1…6 |
| 21 | `<United>` | `false` 0, absent 1 |
| 23 | `<HiddenStateTitleBackColor>` | the colour block |
| 25 | `<CurrentRowUse>` | `Use` 0, `DontUse` 1, absent 2 |
| 26 | `<AssociatedTableElementId>` | the named item's id |

`<Group>` and `<Behavior>` are the sixth and seventh properties found to be
**written more than once under different codings**, and the first written
three times. As everywhere else, each later reading is finer than the one
before it.

Slots 5, 6 and 14 were named by pulling the 250 records whose value was not
the default and listing their XML children: every one of them carried
`<TitleDataPath>`, `<Format>` and `<CollapsedRepresentationTitle>`
respectively. Slot 14 is **not** the tooltip — it agrees with the record's
tooltip member in only 39 592 of 61 256 records, and disagrees in all 180
where it is not empty.

## The other containers

- **auto command bar**, `{0,<align>,<autofill>}`: `<HorizontalAlign>` absent
  0, `Center` 1, `Right` 2, `Auto` 3; `<Autofill>` `false` 0, absent 1. Pure
  over all 12 506 root bars.
- **button group**, `{2,<command source>,2,<representation>}`: absent 0,
  `Usual` 1, `Compact` 2.
- **command bar**, `{1,<location>,<command source>}`: `<HorizontalLocation>`
  absent 0, `Center` 1, `Right` 2, `Auto` 3.
- **pages**, `{4,<a>,<events>,2,<associated>,<b>}`: `<PagesRepresentation>`
  in both, absent 1 in the first and 6 in the second.
- **popup**, nine members: `<Representation>` absent 3, `<Shape>` absent 0,
  `<ShapeRepresentation>` absent 0, plus the two colours.
- **column group**, twelve members: `<Group>` absent 1, `Horizontal` 0,
  `InCell` 2; `<ShowTitle>`, `<ShowInHeader>`, `<HeaderHorizontalAlign>`
  absent 3, the title background, `<FixingInTable>`.
- **page**, `{18,…}`, 20 members, 15 of them read, with `<Group>` written
  three times again.

A container's `<Representation>` was being dropped by the parser for button
groups and popups — the arm was guarded to `UsualGroup` and `Button` alone.

## The field payloads

- **check box**, `{11,…}`, 13 members, 8 read over 10 385 records, with
  `<CheckBoxType>` written twice (slot 4 folds `Switcher` into `Auto`, slot 12
  separates them).
- **radio button**, `{8,…}`, 12 members, 6 read over 2 282 records.
- **spreadsheet document**, `{13,…}`, 32 members, 24 read over 942 records,
  with both scroll bars and `<SelectionShowMode>` written twice.
- **picture**, `{10,…}`: `<Zoomable>` at slot 7 and `<EnableDrag>` at slot 15,
  which the earlier reading had taken as constants.

## The parser's scalar bag

Rather than a typed field per property, `FormXmlChildItem` now carries
`scalars: BTreeMap<String, String>` — every scalar child element no earlier
arm read, by element name. The payload writers take what they need from it and
refuse a spelling the corpus never stored.
