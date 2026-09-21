# The table record's head, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\map-table-head2.py`,
`try-table-head.py`, `partition-table-head.py`, over all 7 183 `{55,…}`
records of `join-55-full.tsv`. Reports: `table-head-map2.txt`,
`table-head-try.txt`, `table-head-partition.txt`.

This is the last item record of a form body.

## The shape, in full

```text
{55,{<id>,<item namespace>},0,<representation>,<flag>[,<functional options>],
    "<name>", … 54 fixed members in all …,
    <bag size>,(<key>,<value>) x size,
    <events>,{0},
    1,<context menu>,1,<command bar>,
    <column count>,(<kind uuid>,<column>) x count,
    … 37 tail members …}
```

The bag is **the same keyed property bag the root record carries**. With the
two children lifted out, a head is `59 + 2 x <bag size>` members long, and that
predicts **every length the corpus stores** -- 59, 61, 63, 77, 79, 81, 83 and
85 -- with nothing else. All 6 903 records parse, none left over.

## Result

```
table heads 6903, fixed part exact 6900 (99.96%)
```

The three that differ carry a `<Shortcut>` at member 51, written as
`{0,<key code>,8}`, which the caller supplies.

## The tool that made it quick

The witness search -- "which property explains the records that differ" -- is
misled by any property that is merely common, and it sent the first reading of
the table tail from 57% down to 31%. `partition-table-head.py` asks the
stronger question instead: **which property, over every record, maps each of
its spellings and its absence to exactly one stored value**. Run over the
members that were still unread, it named fifteen of them in one pass:

| member | source | | member | source |
|---|---|---|---|---|
| 21 | `<HeightInTableRows>` | | 32 | `<HorizontalLines>` |
| 22 | `<ChoiceMode>` | | 33 | `<VerticalLines>` |
| 24 | `<SelectionMode>` | | 36 | `<UseAlternationRowColor>` |
| 25 | `<RowSelectionMode>` | | 37 | `<AutoInsertNewRow>` |
| 26 | `<Header>` | | 38 | `<InitialListView>` |
| 28 | `<Footer>` | | 41 | `<HorizontalStretch>` |
| 30 | `<HorizontalScrollBar>` | | 52 | `<EnableStartDrag>` |
| 31 | `<VerticalScrollBar>` | | 53 | `<EnableDrag>` |

Four of those -- 24, 26, 30 and 31 -- write their **absent** value as something
other than 0, which a witness search would never have shown: a table that names
no `<SelectionMode>` writes 1, one that names no `<Header>` writes 1, and one
that names neither scroll bar writes 2 for both.

## The rest of the head

| member | source | absent |
|---|---|---|
| 3 | `<Representation>`: `List` 0, `Tree` 2 | 1 |
| 4 | 1 when a functional-options block follows | 0 |
| 6 | `<TitleLocation>`: `Auto` 1, `Left` 2, `Top` 3, `Right` 4, `Bottom` 5 | 0 |
| 7 | `<TitleHeight>` | 0 |
| 8 | `<CommandBarLocation>`: `None` 0, `Auto` 1, `Top` 2, `Bottom` 3 | 1 |
| 12 | `<Autofill>` | 0 |
| 13 | `<Enabled>` | 1 |
| 14 | `<ReadOnly>` | 0 |
| 16 | `<DefaultItem>` | 0 |
| 17 | `<ChangeRowSet>` | 1 |
| 18 | `<ChangeRowOrder>` | 1 |
| 19, 20 | `<Width>`, `<Height>` | 0 |
| 23 | `<RowInputMode>`: `AtTheEnd` 1, `AfterCurrentRow` 2 | 0 |
| 27 | `<HeaderHeight>` | 1 |
| 29 | `<FooterHeight>` | 1 |
| 39 | `<InitialTreeView>`: `ExpandTopLevel` 1, `ExpandAllLevels` 2 | 0 |
| 40 | `<Output>`: `Enable` 1, `Disable` 2 | 0 |
| 42 | `<VerticalStretch>` | 1 |

## The bag's keys

Twenty keys, all typed 1C values. Key 19 (`{"S",""}`, 5 565 uses) and key 13
(`{"U"}`, 3 317) are the commonest; keys 5 to 14 travel together in 2 435
records, which is the block a dynamic list carries. What each key holds is the
same open question as the root's bag.
