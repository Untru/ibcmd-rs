# The table record's tail, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\shape-table.py`,
`map-table-tail.py`, `try-table-tail.py`, over the 7 183 `{55,…}` records of
`join-55-full.tsv`. Reports: `table-shape.txt`, `table-tail-map.txt`,
`table-tail-try.txt`.

## The shape

A table carries the same optional functional-options block at member 4, and its
columns are `(kind uuid, record)` pairs like a group's children. The column
count sits anywhere from member 61 to member 87 -- the head grows with the
children the table shows -- but what follows the columns is **exactly 37
members in all 6 903 records** that split, with no exceptions.

## Result

```
table tails 6903, exact 6890 (99.81%)
```

The 13 that differ do so in members 34 and 28, whose driver is not in the
element.

## The reading that could not be guessed

Member 35 is `<FileDragMode>`, and it is **1 when the table does not name one**
and 0 when it names `AsFile`. All 4 593 tables that name it write 0; all 2 310
that do not write 1; nothing is in between.

Three witnesses pointed elsewhere first -- `EnableStartDrag`, `Representation`
and the data path each fit part of the population -- and writing
`EnableStartDrag` there took the measurement from 57% down to 31%. What settled
it was asking the opposite question: not "which property explains the 1s" but
"which property partitions the whole column", which only `<FileDragMode>` does.

## What the source decides

| member | source | absent |
|---|---|---|
| 0 | `<AutoMarkIncomplete>` | 2 |
| 1 | `<AutoAddIncomplete>` | 2 |
| 2 | `<Visible>` | 1 |
| 3 | `<MultipleChoice>` | 0 |
| 7 | `<SkipOnInput>` | 2 |
| 8 | `<SearchOnInput>`: `Use` 0, `DontUse` 1 | 2 |
| 9 | `<ToolTipRepresentation>` | 0 |
| 12 | `<SearchStringLocation>`: `None` 1, `CommandBar` 2, `Top` 3, `Bottom` 4, `FormCaption` 5, `PullFromTop` 6 | 0 |
| 13 | `<ViewStatusLocation>`: `None` 1, `Top` 2, `Bottom` 3 | 0 |
| 14 | `<SearchControlLocation>`: `None` 1, `CommandBar` 2 | 0 |
| 21 | `<RefreshRequest>`: `PullFromTop` 1 | 0 |
| 22 | `<AutoMaxWidth>` | 1 |
| 23 | `<MaxWidth>` | 0 |
| 25 | `<AutoMaxHeight>` | 1 |
| 26 | `<MaxHeight>` | 0 |
| 29 | `<HeightControlVariant>` | 0 |
| 30 | `<AutoMaxRowsCount>` | 1 |
| 31 | `<MaxRowsCount>` | 0 |
| 32 | `<CurrentRowUse>`: `Choice` 1, `SelectionPresentation` 2, `SelectionPresentationAndChoice` 3 | 0 |
| 35 | `<FileDragMode>`: `AsFile` 0 | **1** |

Members 15, 17 and 19 are always 1: a table always carries its three `{5,…}`
additions -- the search string, the view status and the search control -- even
when it shows none of them.

## What is left

The table's **head**, from `{55` to the column count: 61 to 87 members with the
context menu, the command bar and the additions inline behind flags. That is
the last item record still unread.
