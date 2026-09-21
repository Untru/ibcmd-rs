# The `{22,…}` group record's shape, measured over ERP УХ

Date: 2026-09-21. Tool: `F:\ibcmd\lab\tools\try-group.py`, over the 10 004
group records of `join-22.tsv`. Report: `group-shape.txt`.

## Member 5 is the group's kind

Every container a form can hold is a `{22,…}` record, told apart by one member:

| kind | member 5 |
|---|---|
| `CommandBar` | 0 |
| `Popup` | 1 |
| `ColumnGroup` | 2 |
| `Pages` | 3 |
| `Page` | 4 |
| `UsualGroup` | 5 |
| `ButtonGroup` | 6 |
| navigator | 7 |
| `ContextMenu` | 8 |
| `AutoCommandBar` | 9 |

The correspondence is exact: every `AutoCommandBar` of the sample writes 9,
every `UsualGroup` 5, every `ContextMenu` 8, and so on, with no kind sharing a
number.

## The head carries an optional block

The head is not fixed-length. Member 4 is a flag, and when it is 1 a
functional-options block follows before the kind:

```text
{22,{<id>},0,0,0,<kind>,"<name>",…                        -- 9 217 records
{22,{<id>},0,0,1,{0,{0,{"B",1},0}},<kind>,"<name>",…      --   787 records
```

Reading the head as fixed is what left 6.5% of the records unaligned, and it is
also why a slot map of the UsualGroup records shows two populations at every
member past 3.

## The tail is four members and an optional tooltip

After the children the record closes with

```text
1, 0, 0,               0, 3, 3, 0     -- 7 members, no extended tooltip
1, 0, 1, {12,…},       0, 3, 3, 0     -- 8 members, with one
```

Member 2 of the tail is the flag and member 3 the `{12,…}` extended tooltip
record. 7 142 of the aligned records have no tooltip and 2 075 have one, and
nothing else varies: the closing `0,3,3,0` is the same in all of them.

## What is still open

The head's members 9 to 15 and 18 to 19. A slot map of them is not meaningful
until the optional block above is taken out of the alignment, which is the next
step for this record.
