# The `{31,…}` button record, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\map-button.py`, `try-button.py`,
over all 77 127 button records of `join-31.tsv`. Reports: `button-map.txt`,
`button-try.txt`.

## The block sits one member earlier than a field's

A button carries the same optional functional-options block a container and a
field do, but at **member 3, not member 4**. With it lifted the record is
fixed-length -- **52 members in all 77 127 records**, with none left over.

## Result

```
Button | records 77127 | fixed-length 77127 | exact 76913 (99.72%)
```

The 214 that differ do so **only** at member 48, which the button's own element
does not carry; the writer takes it from the caller and defaults to the 0 the
other 76 913 write. Every other member of every record rebuilds byte for byte.

## Two properties written twice, under two codings

This is the fourth and fifth instance of the pattern, after `<VerticalScroll>`
and `<Group>` in the root tail and `<CurrentRowUse>` in a command:

| `<Type>` | member 4 | member 46 |
|---|---|---|
| `CommandBarButton` | 0 | 0 |
| `UsualButton` | 1 | 1 |
| `Hyperlink` | 2 | 2 |
| `CommandBarHyperlink` | **0** | **3** |

| `<LocationInCommandBar>` | member 15 | member 49 |
|---|---|---|
| absent | 2 | 0 |
| `InAdditionalSubmenu` | 0 | 1 |
| `InCommandBar` | 1 | 2 |
| `InCommandBarAndInAdditionalSubmenu` | **1** | **3** |

Member 4 is the coarse reading -- a command-bar hyperlink counts as a
command-bar button, and a button in both places counts as one in the command
bar -- and member 46 and member 49 are the fine ones. Reading either property
once leaves 14 616 records wrong at member 15 alone.

## What the source decides

Beyond those, the record carries `<Enabled>` (7), `<Representation>` (10),
`<DefaultButton>` (11), `<DefaultItem>` (13), `<Width>` (16), `<Height>` (17),
`<TitleHeight>` (18), `<Check>` (24), `<Visible>` (26), `<SkipOnInput>` (29,
a tri-state), `<ToolTipRepresentation>` (30), `<AutoMaxWidth>` (34),
`<MaxWidth>` (35), `<AutoMaxHeight>` (37), `<MaxHeight>` (38),
`<HorizontalStretch>` (39), `<VerticalStretch>` (40), the two group aligns
(41, 42), `<RepresentationInContextMenu>` (43), `<Shape>` (44),
`<ShapeRepresentation>` (45), `<PictureLocation>` (47) and
`<CommandUniqueness>` (50).

## The check that matters

`format_button_item` with its defaults reproduces, member for member, what the
narrow `format_standard_command_button` writes for the same facts -- and that
narrow writer was read off the bodies independently. Two readings of the same
record, taken months apart in the work and from different directions, agree.
