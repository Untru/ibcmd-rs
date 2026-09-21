# The `{12,…}` decoration record, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\try-tooltip.py`,
`try-decoration.py`, over the 464 551 decoration records of `join-12.tsv`.
Reports: `tooltip-try.txt`, `decoration-try.txt`.

## The extended tooltip is the commonest record in a body

| tag | records |
|---|---|
| `ExtendedTooltip` | 421 852 |
| `LabelDecoration` | 35 477 |
| `PictureDecoration` | 7 222 |

A tooltip is a decoration like any other, told apart only by where it sits.
Member 5 is the kind: 0 for a label or a tooltip, 1 for a picture.

## Result

```
ExtendedTooltip | records 421852 | fixed-length 421849 | exact 421844 (100.00%)
```

With the optional functional-options block at member 4 lifted -- the same block
a field carries, 4 331 tooltips have one -- the record is **34 members in all
421 849**, and **421 844 rebuild byte for byte**. The five that differ do so in
members 2 and 32, which the element does not carry.

The first attempt, before the block was lifted and the titles came from the
caller, gave 98.29%. The block accounted for 4 331 of the difference and the
titles for 2 216.

## What the source decides

| member | source | absent |
|---|---|---|
| 4 | 1 when a functional-options block follows | 0 |
| 5 | the kind: label or tooltip 0, picture 1 | |
| 10 | `<Width>` | 0 |
| 11 | `<Height>` | 0 |
| 12 | `<HorizontalStretch>` | 2 |
| 13 | `<VerticalStretch>` | 2 |
| 25 | `<AutoMaxWidth>` | 1 |
| 26 | `<MaxWidth>` | 0 |
| 28 | `<AutoMaxHeight>` | 1 |
| 29 | `<MaxHeight>` | 0 |
| 30 | `<GroupHorizontalAlign>` | 3 |
| 31 | `<GroupVerticalAlign>` | 3 |

Members 25 and 28 were swapped on the first reading -- 25 is the width's
auto flag and 28 the height's -- which cost 2 379 records.

## The check that matters

`format_decoration_item` with its defaults reproduces, member for member, what
the narrow `format_extended_tooltip` writes -- and that shape was read off the
bodies independently, earlier in this work. As with the button, two readings
taken from different directions agree.

## What is left

A `<LabelDecoration>` and a `<PictureDecoration>` are the same record **two
members longer**, 36 in all 35 468 and 7 222 of them, and the two extra members
sit from member 19 on. Their layout is not yet read; the writer serves the
tooltip shape, which is 91% of all decoration records.
