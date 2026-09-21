# The `{37,…}` field record, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\map-field.py`, `try-field.py`,
over the 69 526 input field records of `join-37-full.tsv`. Reports:
`field-map.txt`, `field-try-full.txt`.

## The record is fixed-length

Member 4 is the same flag a container carries: when it is 1 a
functional-options block follows before the kind. With that block lifted the
record is **59 members, in 69 521 of the 69 526** input fields -- five outliers
carry more.

## Result

```
InputField | records 69526 | fixed-length 69521 | exact 69456 (99.91%)
```

**69 456 of 69 521 rebuild byte for byte** once the members that name a
configuration object come from the caller: the id, the name, the titles, the
data paths, the pictures, the colours, the font, the payload, the events, the
context menu and the extended tooltip.

## What the source decides

| member | source | absent |
|---|---|---|
| 5 | the kind: 1 label, 2 input, 3 check box, 4 picture, 5 radio group, 6 spreadsheet, 7 HTML, 9 indicator, 15 formatted document | |
| 7 | `<TitleLocation>`: `None` 0, `Left` 2, `Top` 3, `Right` 4, `Bottom` 5 | 1 |
| 8 | `<TitleHeight>` | 0 |
| 13 | `<Enabled>` | 1 |
| 14 | `<ReadOnly>` | 0 |
| 15 | `<SkipOnInput>` | **2** |
| 16 | `<DefaultItem>` | 0 |
| 17 | `<WarningOnEditRepresentation>`: `Show` 0, `DontShow` 1 | 2 |
| 20 | `<ShowInHeader>` | 1 |
| 21 | `<ShowInFooter>` | 1 |
| 22 | `<CellHyperlink>` | 0 |
| 23 | `<HorizontalAlign>`: `Left` 0, `Center` 1, `Right` 2 | 3 |
| 24 | `<HeaderHorizontalAlign>`: same, plus `Auto` 3 | **0** |
| 25 | `<FooterHorizontalAlign>`: same | 3 |
| 26 | `<EditMode>`: `Directly` 0, `Enter` 1, `EnterOnInput` 2 | 1 |
| 27 | `<VerticalAlign>`: `Top` 0, `Center` 1, `Bottom` 2 | 3 |
| 28 | `<AutoCellHeight>` | 0 |
| 43 | `<Visible>` | 1 |
| 49 | `<FixingInTable>`: `Left` 1, `Right` 2 | 0 |
| 50 | `<ToolTipRepresentation>`: `Auto` 0, `None` 1, `Balloon` 2, `Button` 3, `ShowAuto` 4, `ShowTop` 5, `ShowLeft` 6, `ShowBottom` 7, `ShowRight` 8 | 0 |
| 53 | `<GroupHorizontalAlign>` | 3 |
| 54 | `<GroupVerticalAlign>` | 3 |

Member 24's absent value is 0 where every other alignment's is 3, and member 15
is a tri-state like the stretches: 2 when the field names neither value.

Three readings had to be corrected against the corpus rather than guessed:
member 23 is `<HorizontalAlign>` and member 25 is `<FooterHorizontalAlign>`,
not the other way round (138 records); `<EditMode>` has a `Directly` spelling
that writes 0 (66 records); and `<TitleLocation>` writes 4 for `Right`, not 5
(56 records).

## What is left

65 records of the 69 521 differ, all at member 55, where the witnesses
disagree: `<VerticalStretch>`, `<SkipOnInput>` and `<MaxWidth>` each fit some
of them and none fits all. The writer leaves it at the 0 that the other 69 456
write.

Over **all** tags that use the `{37,…}` wrapper -- labels, check boxes,
buttons and the rest, 136 025 records -- the same writer reaches 93.16%. The
members that differ are 26, 7 and 14, which each kind reads its own way; those
are the next thing to measure, one kind at a time.
