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

## One layout for all three

```
@all | records 464551 | parsed 464539 | exact 464526 (100.00%)
```

A `<LabelDecoration>` and a `<PictureDecoration>` are not a different record.
What makes them longer is two **children** a tooltip does not carry, each
behind its own flag:

```text
…,<payload>,<menu flag>[,<context menu>],<visible>,<skip on input>,
   <content>,<tooltip representation>,<tooltip flag>[,<extended tooltip>],
   <auto max width>,…
```

A tooltip writes both flags as 0 and is 34 members; a label decoration writes
both as 1 and is 36. Read that way, one layout rebuilds **464 526 of the
464 539** decoration records of the corpus -- every tooltip, label and picture
decoration alike -- with 13 left over.

Member 9, which the tooltip reading took for a constant 1, is `<Enabled>`.

The four lengths the corpus stores are exactly the four the model predicts:
34 (417 518 records), 35 (4 331, with the options block), 36 (41 341, with
both children) and 37 (1 349, with all three).
