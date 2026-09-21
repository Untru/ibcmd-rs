# Reading a payload slot's meaning off the corpus, 21.09.2026

The records are written; what is left is which member of a payload carries
which XML property. That does not have to be guessed either.

## The method

`map-payload-slots.py` runs the second pass over any join, so reading a new
kind costs one command rather than a new script.

`F:\ibcmd\lab\tools\join-items.py` joins every stored item record to the XML
item of the same id: it reads a form's `Form.xml`, finds the body row of that
form's uuid in the inflated dump, and writes one line per item with the item's
tag, its XML scalar properties and the members of its payload tuple. Over 400
forms that is 5 037 field records.

A second pass then asks, for each slot and each property, whether the
property's value *determines* the slot's value across every record that names
it. A slot that varies and that exactly one property predicts is a slot whose
meaning has been read, not inferred.

## What it reads on the first two payloads

`LabelField`, payload `{11,…}`, 20 members, 2 424 records:

| slot | property | mapping |
|---|---|---|
| 1 | `Width` | the number itself |
| 2 | `Height` | the number itself |
| 3 | `HorizontalStretch` | `false` → 0, `true` → 1, absent → 2 |
| 6 | `Format` | the localised format string |
| 12 | the item's own event bindings | `{0,1,0}` when it has none |
| 16 | `MaxWidth` | the number itself |

`InputField`, payload `{36,…}`, 66 members, 1 784 records:

| slot | property | mapping |
|---|---|---|
| 2 | `Width` | the number itself |
| 3 | `Height` | the number itself |
| 4 | `HorizontalStretch` | `false` → 0, `true` → 1, absent → 2 |
| 5 | `VerticalStretch` | `false` → 0, `true` → 1, absent → 2 |

Slots 8, 9 and 10 of the label payload carry the text colour, the background
colour and the font; slot 1 of the input payload carries its choice list.

`UsualGroup`, payload `{29,…}`, 29 members, 1 357 records -- the most common
payload in the corpus. Twenty of its twenty-nine slots were read in one pass:

| slot | property | mapping |
|---|---|---|
| 3 | `Representation` | `None` → 0, `StrongSeparation` → 1, `NormalSeparation` → 3 |
| 9 | `BackColor` | the colour tuple itself |
| 10 | `Behavior` | `Usual` → 0, `Collapsible` and `PopUp` → 1 |
| 19 | `ThroughAlign` | `Use` → 0, `DontUse` → 1, absent → 2 |
| 22, 27 | `Group` | `Vertical` → 0, `Horizontal` → 1, `AlwaysHorizontal` → 1 and 3 |
| 24, 28 | `Behavior` | `Usual` → 0, `Collapsible` → 1, `PopUp` → 2 |

`Page`, payload `{18,…}`, 20 members: slot 2 and slot 4 answer to `Group`,
slot 1 carries the page picture.

## Every payload kind, one run each

Pointed at both joins with no kind filter, over 400 forms:

| kind | payload members | records | slots that vary and were read |
|---|---|---|---|
| `InputField` | 66 | 1 784 | **49** |
| `UsualGroup` | 29 | 1 357 | **20** |
| `PictureField` | 24 | 41 | 16 |
| `LabelField` | 20 | 2 424 | 15 |
| `Page` | 20 | 224 | 7 |
| `CheckBoxField` | 13 | 396 | 5 |
| `RadioButtonField` | 12 | 71 | 3 |
| `ColumnGroup` | 12 | 304 | 2 |
| `Pages` | 6 | 75 | 2 |
| `ButtonGroup` | 4 | 217 | 1 |
| `AutoCommandBar` | 3 | 682 | 1 |
| `ContextMenu` | 2 | 6 349 | 0 -- its payload does not vary |
| `SpreadSheetDocumentField` | 32 | 9 | 0 -- too few records |
| `TextDocumentField` | 16 | 6 | 0 -- too few records |
| `FormattedDocumentField` | 16 | 3 | 0 -- too few records |

The kinds that read nothing are of two sorts. A context menu's payload is the
same two members everywhere, so there is nothing to read. A spreadsheet
document field appears nine times in 400 forms, which is below the threshold
the second pass needs to call a mapping proved -- those want a wider join, not
a different method.

## A wider join reads more of the same payloads

The same run over 3 000 forms -- 27 222 field records instead of 5 037:

| kind | payload members | records | read at 400 forms | read at 3 000 |
|---|---|---|---|---|
| `InputField` | 66 | 12 582 | 49 | **57** |
| `Command` | 66 | 639 | 16 | **37** |
| `LabelField` | 20 | 8 588 | 15 | 15 |
| `PictureField` | 24 | 253 | 16 | 13 |
| `CheckBoxField` | 13 | 2 569 | 5 | 7 |
| `RadioButtonField` | 12 | 669 | 3 | 6 |
| `SpreadSheetDocumentField` | 32 | 35 | 0 | 6 |
| `TextDocumentField` | 16 | 32 | 0 | 5 |
| `HTMLDocumentField` | 13 | 21 | 0 | 5 |
| `FormattedDocumentField` | 16 | 15 | 0 | 4 |

So the threshold was the join's width, exactly as the note said. What stays at
zero are the kinds that are genuinely rare -- a chart field, a calendar field,
a track bar, a Gantt chart, a PDF document field and a graphical schema field
appear once or twice in 3 000 forms. Those want the whole corpus and the BSP
tree beside it.

## Closing a payload: claim, measure, refine

Reading a slot is a claim. A candidate writer turns it into a number.

`try-label-payload.py` builds the label payload from the XML properties with
the mapping read so far, copies the slots it does not claim from the stored
record, and compares. Over every label payload of every ERP УХ form body --
37 078 records from all 12 515 forms -- three rounds:

| round | the rule under test | exact |
|---|---|---|
| 1 | `Width`, `Height`, `HorizontalStretch`, `MaxWidth`, and slot 15 as "a maximum width was named" | 34 195 of 37 078 (92.22%) |
| 2 | slot 15 also 0 when `AutoMaxWidth` is false | 36 947 (99.65%) |
| 3 | slot 15 0 **exactly when** `AutoMaxWidth` is false | **37 078 (100.00%)** |

The mistake in round 1 was reading `MaxWidth` as the cause of slot 15; it is
`AutoMaxWidth` alone, and a form may name a maximum width while leaving the
flag alone. The corpus said so in 131 records, and the loop found it in one
pass.

`format_label_payload` now carries those five rules. This is the shape of the
remaining work: not "look at the record and guess", but claim, measure over
tens of thousands of records, refine, and stop when it is 100%.

## Why this matters for the estimate

Each payload kind has a fixed member count -- 20 for a label, 66 for an input,
29 for a usual group, 20 for a page -- and each member is either constant
across the corpus or predicted by one property. So the remaining work is
bounded and mechanical: run the join per kind, read the slots, write them, and
prove the writer against the records it was read from.

What it is not is small. Six field payloads and eight group payloads at
twenty to sixty-six members each, plus the root property bag and the dynamic
list's DCS settings, are days of measurement -- and only after all of them can
the writer be wired into compilation and the round-trip closed.
