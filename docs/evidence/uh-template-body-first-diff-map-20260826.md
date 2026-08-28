# ERP УХ template bodies: first-difference map, 20260826

Scope: the `**/Templates/*/Ext/Template.xml` half of the `uh` gate's
`differing` set at base **911e86e** — 754 files out of 2 292, of which 609 sit
under `Reports/**`. The meta files beside them (`Templates/<name>.xml`) are
not in the differing set at all; only the bodies break.

By root element the 754 split as `<document>` (MOXCEL spreadsheet) 623,
`<DataCompositionSchema>` 130, `<GraphicalSchema>` 1.

## Method

Each file is classified by the **first line at which our output and the
platform's differ**, keyed by the pair of element names on that line. The
companion map of 20260825 rightly warns that a first-difference key names
whichever cosmetic difference happens to come first; here it is used as a work
queue, not as a size estimate — each class is opened, the storage record behind
it is read out of `cf extract`'s `unpacked.bin`, and the rule is then measured
against the whole corpus before it is written.

## Histogram at 911e86e (754 files)

| files | native | ours | class |
| ---: | --- | --- | --- |
| 274 | `note` | `</c>` | cell note dropped |
| 122 | `id` | `id` | a `Picture` drawing dropped, the rest renumbered |
| 52 | `</dcsat:tableCell>` | `<dcsat:appearance></dcsat:appearance>` | DCS empty element the platform omits |
| 30 | `v8ui:style` | `v8ui:style` | cell line palette |
| 28 | `</document>` | `<picture>` | a picture published that the platform does not |
| 25 | `drawing` | `templateMode` | a `Chart`/`GanttChart`/`Picture`/`Text` drawing dropped |
| 23 | `v8:Type` | `v8:TypeId` | config type not resolved to a name |
| 22 | `leftMargin` | `</format>` | format margin dropped |
| 21 | `dcsat:tableCell` | `dcsat:tableCell` | DCS area template |
| 16 | `value` | `value` | DCS QName body on a stale prefix |
| 14 | `backColor` | `backColor` | style/palette slot |
| 13 | `tl` | `tl` | empty-content text list |
| 12 | `dcscor:value` | `dcscor:value` | DCS value |
| 11 | `borderColor` | `borderColor` | style/palette slot |
| 9 | `line` | `line` | cell line palette |
| 7 | `defaultFormatIndex` | `defaultFormatIndex` | format table |
| 7 | `textPosition` | `</format>` | format member dropped |
| — | | | 88 further files across 30 classes of ≤ 5 |

## Rules this pass established

### 1. A note's text is a declared-count localized container — 274 files

The note member is the triple `<text list>, 1, {…}` at the tail of the cell
record, and its text list is the same `{1, <count>, {<lang>,<content>}, …}`
container the cell text uses. The reader took exactly one pair and refused any
record that declared more, so every bilingual note was dropped — and with it
the note's own `<formatIndex>`, which shifted the format numbering of
everything published behind it.

Measured over `Templates/*/Ext/Template.xml` of ERP УХ 3.2.12.6, 1С:УТ
11.5.27.75, БСП demo/base and Документооборот КОРП 3.0.21.3: **1 899 notes,
1 576 of them with two declared languages and 323 with one**. The member set
and its order are constant in all 1 899 — `drawingType` (always `Comment`),
`id` (always `0`), `formatIndex`, `text`, the eight geometry members,
`autoSize` (1 757 `true`, 142 `false`) and `pictureSize` (always `Stretch`).

**uh: exact 137 614 → 137 852 (+238), broken 0.**

### 2. The note's own leading record may stop at three members — 21 files

The note's format record is the same `{mask, index, …members}` grammar the
drawing head uses, with mask 16 naming the localized text. The reader required
a fourth member spelling `0`. All 37 note records of the 21 ERP УХ templates
that still differed after rule 1 stop at three members; the ones rule 1 already
accepted carry the fourth. The trailing member is optional, exactly as the cell
record's trailing "formatted" flag is.

### 3. Picture size code 3 is `Tile` — 122 files

The `Picture` drawing record's thirteenth member is the picture size, and code
`3` was not in the reader's table, which refused the whole record. A refused
drawing is skipped silently and `<zOrder>` is assigned from the surviving
sequence, so one refusal renumbered every drawing behind it.

Pairing each native `<drawing>` with the stored record carrying its `<id>`,
over the 25 ERP УХ templates that publish a tiled picture: **42 records with
code 3, every one published `Tile`**, beside 64 code-1 `Stretch` and 9 code-0
`RealSize` in the same files, no counterexample. `Tile` is published 228 times
across the five corpora and never appears as a format `pictureSizeMode`, so the
format member's own table is left alone.

### 4. An item with empty content is not published — 13 files

A stored text list may declare one item whose content is the empty string
(`{1,1,{"ru",""}}`); the platform publishes the self-closed `<tl/>` for it, not
an item with an empty `<v8:content>`.

Over every `Template.xml` of ERP УХ 3.2.12.6, 1С:УТ 11.5.27.75 and
Документооборот КОРП 3.0.21.3 — **10 163 199 `<tl>` and 1 610 230 `<tl/>` —
there is not one `<v8:content></v8:content>` and not one `<v8:content/>`**. No
stored list mixes empty with non-empty content either: of 36 267 lists read out
of 88 decoded bodies, every one is wholly empty (50, all single-item) or wholly
not.

### 5. A format margin is published as stored — 22 files

Members 42–45 of the format record are the four cell margins, and the mask
already decides whether each is stored at all. The reader admitted a stored
margin only when it spelled `0`, which dropped every non-zero one. Inside
`<format>` the five corpora publish `leftMargin` 8 (62), 40 (13), 24 (6) and 16
(4), `rightMargin` 40 (13) and 8 (4), and `bottomMargin` 8 (3) and 28 (1),
across 32 documents. (`printSettings` margins have their own reader and were
never affected.)

### 6. An `xsi:type="v8:Type"` body is a QName in every document — 16 files

The DCS transliterator re-mints the storage document's generated `dNpM`
prefixes against the depth of the position it writes them at, in the
declaration and in the QName that uses it. Character data was treated as a
QName when the element itself is `{data/core}Type`/`TypeSet`, but when the
element merely *carries* `xsi:type="v8:Type"` only the `ListSettings`-child
mode did so. A schema parameter's `<value>` is the same construct in a template
document, so its declaration was reminted to `d3p1` while its body kept the
storage document's `d4p2`.

## Not ours to fix, and not counted

`Shortcut` (the neutral modifier bitmask the platform renders per host) is
recorded in `host-dependent-export-2214-20260823.md`; none of the 754 files
here is first-different on it.
