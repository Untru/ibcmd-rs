# Reading a payload slot's meaning off the corpus, 21.09.2026

The records are written; what is left is which member of a payload carries
which XML property. That does not have to be guessed either.

## The method

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
