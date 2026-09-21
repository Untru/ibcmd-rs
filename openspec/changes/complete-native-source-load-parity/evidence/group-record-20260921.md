# The `{22,…}` container record, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\map-group-head.py`,
`try-group-head.py`, over the 9 357 container records of `join-22.tsv`.
Reports: `group-head-map.txt`, `group-head-try.txt`.

Supersedes the shape-only reading in
[group-record-shape-20260921.md](group-record-shape-20260921.md).

## Result

```
records 9357 | normalised 9357 | head exact 9357 (100.00%) | tail exact 9354 of 9357 (99.97%)
```

Across **all nine kinds** of container -- command bar, popup, column group,
pages, page, usual group, button group, context menu, auto command bar.

## The head

```text
{22,{<id>,<item namespace>},0,0,<flag>[,<functional options>],<kind>,
    "<name>",<title>,<tooltip title>,
    <content change>,<enabled>,<read only>,<width>,<height>,
    <horizontal stretch>,<vertical stretch>,
    <back colour>,<font>,{0,0,0},1,<payload>,<child count>, …
```

| member | source | absent |
|---|---|---|
| 4 | 1 when a functional-options block follows | 0 |
| 5 | the kind: `CommandBar` 0, `Popup` 1, `ColumnGroup` 2, `Pages` 3, `Page` 4, `UsualGroup` 5, `ButtonGroup` 6, navigator 7, `ContextMenu` 8, `AutoCommandBar` 9 | |
| 9 | `<EnableContentChange>` | 0 |
| 10 | `<Enabled>` | 1 |
| 11 | `<ReadOnly>` | 0 |
| 12 | `<Width>` | 0 |
| 13 | `<Height>` | 0 |
| 14 | `<HorizontalStretch>` | **2** |
| 15 | `<VerticalStretch>` | **2** |
| 18 | always `{0,0,0}` | |
| 19 | always 1 | |

Lifting the optional block at member 4 out of the alignment is what makes the
rest line up: read as fixed, 6.5% of the records do not parse, and a slot map
of the usual groups shows two populations at every member past 3.

## The tail

```text
…, <visible>, <tooltip representation>, <flag>[,<extended tooltip>],
   0, <horizontal align>, <vertical align>, 0}
```

| member | source | absent |
|---|---|---|
| 0 | `<Visible>` | 1 |
| 1 | `<ToolTipRepresentation>`: `Auto` 0, `None` 1, `Button` 3, `ShowTop` 5, `ShowBottom` 7 | 0 |
| 2 | 1 when an extended tooltip follows | 0 |
| then | 0 | |
| | `<GroupHorizontalAlign>`: `Left` 0, `Center` 1, `Right` 2 | 3 |
| | `<GroupVerticalAlign>`: `Top` 0, `Center` 1, `Bottom` 2 | 3 |
| last | 0 | |

7 031 records have no extended tooltip and 2 326 have one.

## What is left

Three records of the 9 357 write 1 in the last member where every other
container writes 0. All three are column groups that both sit in a cell and
carry an extended tooltip -- but so do others that write 0, so the reason is
not in the element itself and the writer does not guess it.
