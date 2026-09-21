# The form attribute record, measured over ERP УХ

Date: 2026-09-21. Corpus: the native export of ERP УХ against its inflated form
bodies. Tools: `F:\ibcmd\lab\tools\join-attributes.py`, `try-attribute.py`.
Reports: `attr-slots.txt`, `attr-try.txt`.

## The join is exact

```
forms: 12507 rows: 135039 forms whose counts differ: 0
```

The attributes section's count equals the number of `<Attribute>` elements in
**every** form, and the records come in the order the XML lists them. Nothing
has to be matched by name or id to pair a record with its source.

## The shape

```text
{9,{<id>},0,"<name>",<title>,<type pattern>,
   <use always>,<use always>,<view>,<edit>,
   <main>,<saved>,<fill check>,<column count>,
   <column> x count,
   <block>,<block>}
```

A record is `16 + <member 13>` members long. Over the 102 895 records:

* 100 740 have that length, so the count and the two closing blocks account for
  the whole record;
* 94 031 of those close with two empty pairs, and the other 6 709 carry a type
  or an object there, such as `{0,1,"ElementType",{"#",f5c65050-…,{"Pattern"}}}`;
* 2 155 carry more members still, and are not yet read.

## What the source decides

```
records 102895 | shape holds in 94031 | fixed part exact 94031 (100.00%)
```

The fourteen fixed members rebuild byte for byte once the six that name
configuration objects come from the caller -- the title, the type pattern, the
two `<UseAlways>` blocks and the two that carry `<View>` and `<Edit>`
restrictions, all of which hold uuids the source alone cannot resolve.

The three that *are* in the source were all read off the corpus, and the first
attempt had two of them swapped:

| member | source | wrong reading cost |
|---|---|---|
| 10 | `<MainAttribute>` | 4 295 records |
| 11 | `<SavedData>` | 5 012 records |
| 12 | `<FillCheck>`, 1 for `ShowError` | 701 records |

Member 13 is the number of columns the attribute declares, and each column is a
`{5,<index>,0,"<name>",<title>,<pattern>,…}` record of its own -- what a value
table or value tree attribute carries.
