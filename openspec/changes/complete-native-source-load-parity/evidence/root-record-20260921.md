# The body frame and the root record, measured over ERP УХ

Date: 2026-09-21. Corpus: the inflated form bodies of ERP УХ,
`F:\ibcmd\lab\form_bodies\Config_inflated` (12 515 rows), against the native
8.3.27.2214 export of the same database.

Tools: `F:\ibcmd\lab\tools\census-frame.py`, `join-root.py`, `try-root.py`,
`try-root-tail.py`. Reports: `frame-census.txt`, `root-shape.txt`,
`root-map.txt`, `root-tail-try.txt`.

## The frame is ten members, always

Every one of the 12 507 bodies read is

```text
{4,<root {50,…} record>,<module text>,<attributes>,<parameters>,<commands>,
 <conditional appearance>,<one more section>,0,0}
```

with no exceptions: `frame widths: [(10, 12507)]`. Member 0 is always `4`,
member 1 is always the root record, members 8 and 9 are always `0`.

The attributes section carries the form's settings composer with it: a form
with no attributes writes `{4,0,0,0,{#base64:…}}`, where the blob is an empty
`<Settings/>` document.

The existing base-free writer emits a seven-member frame
(`{4,<layout>,"",{0},{0,0},{0,0},{0}}`), which is why it produces a body the
platform did not write. That is the gap this change closes.

## The root record is a head, the children, and a tail

The head is **variable**: the title, an optional property block and the optional
auto command bar all sit inline, so the child block cannot be found by counting
from the front. Two anchors find it instead: the tail always opens with two
empty strings, and each child is a `(kind uuid, record)` pair. Walking back from
the anchor over whole pairs finds the count, and the stored count confirms it.
That splits **12 488 of 12 507** records; the other 19 carry a literal `"",""`
inside a child, which fools the anchor.

Tail lengths are exactly two: **24 (7 030 records) and 25 (5 458)**. The
difference is one member -- the navigator group, which is preceded by a flag:

```text
"", "", 0                       -- no navigator
"", "", 1, {22,…,"Navigator",…} -- with one
```

What follows is **exactly 21 members** in every record: the form's own property
bag.

## The 21 members

| # | source | absent |
|---|---|---|
| 0 | `<AutoURL>` | 1 |
| 1 | always `""` | |
| 2 | `<VerticalScroll>`: `useIfNecessary` → 2, `useWithoutStretch` → **0** | 0 |
| 3 | `<ScalingMode>`: `Normal` → 1, `Compact` → 2 | 0 |
| 4, 5 | always 0 | |
| 6 | `<HorizontalSpacing>`: `None` 1, `Half` 2, `OneAndHalf` 4, `Double` 5 | 0 |
| 7 | `<VerticalSpacing>`, same codes | 0 |
| 8 | `<HorizontalAlign>`: `Left` 0, `Center` 1, `Right` 2 | 3 |
| 9 | `<VerticalAlign>`: `Top` 0, `Center` 1, `Bottom` 2 | 3 |
| 10 | `<ChildrenAlign>`: `None` → 1 | 0 |
| 11 | `<Group>`: `Horizontal` 1, `AlwaysHorizontal` **1**, `HorizontalIfPossible` 2 | 0 |
| 12 | `<VerticalScroll>` again: `useIfNecessary` → 2, `useWithoutStretch` → **3** | 0 |
| 13 | always 100 | |
| 14 | `<ShowTitle>` | 1 |
| 15 | `<ShowCloseButton>` | 1 |
| 16 | `<ConversationsRepresentation>`: `Show` 1, `DontShow` 2 | 0 |
| 17 | `<CollapseItemsByImportanceVariant>`: `Use` 1, `DontUse` 2 | 0 |
| 18 | `<Group>` again: `Horizontal` 1, `AlwaysHorizontal` **3**, `HorizontalIfPossible` 2 | 0 |
| 19 | `{50,0}` | |
| 20 | `<SaveWindowSettings>` | 1 |

Two properties are read **twice and not the same way both times**:
`useWithoutStretch` writes 0 at member 2 and 3 at member 12, and
`AlwaysHorizontal` writes 1 at member 11 and 3 at member 18. Reading either one
consistently costs 3 386 records.

## Result

```
tails 12410, exact 12410 (100.00%)
```

**12 410 of 12 410 tails rebuild byte for byte from the source alone.**

The refused cohort is a single, sharp one: the 78 forms that carry a
`<MobileDeviceCommandBarContent>`. Their member 19 is not `{50,0}` but
`{50,1,"",{"N",<n>}}` -- a reference into the body's own value table, which the
source alone cannot resolve. Every form that differed carried that element, and
no form without it differed. `format_root_tail` writes the tail for the rest and
refuses a spelling it has not measured.

## What is still open in the root

The **head** -- everything from `{50` to the child count. Its slots 2, 3, 4, 9,
10, 17 read `<WindowOpeningMode>`, `<Width>`, `<Height>`, the title and
`<CommandBarLocation>`; slots 18 to 20 are a variable block that carries the
events, the command set and, in 3 219 forms, one member more. That block is the
next thing to measure.
