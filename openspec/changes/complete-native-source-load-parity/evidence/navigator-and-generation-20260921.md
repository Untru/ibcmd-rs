# The navigator is the same unknowable as the settings blob

Date: 2026-09-21. Corpus: the 12 507 inflated form bodies of ERP УХ.

## The question

5 458 of the 12 488 root records that split carry a navigator in the tail --
`{22,{0},0,0,0,7,"Navigator",…}` -- and 7 030 do not. **Nothing in a form's XML
partitions the two.** Three candidates were tried and all three split both
ways for every value:

* every scalar property of the form, by the partition test;
* the class of the form's main attribute -- `<none>`, `DynamicList`,
  `DocumentObject`, `CatalogObject` and the rest each appear on both sides;
* whether the form has a `<CommandInterface>`, and whether that names a
  navigation panel.

## The answer

The navigator travels with the **settings composer spelling**, which
[body-frame-20260921.md](body-frame-20260921.md) already showed is not in the
source either:

| | settings `<Settings/>` | settings with `<outputParameters/>` | other |
|---|---|---|---|
| **no navigator** | 6 761 | 0 | 273 |
| **navigator** | 21 | 5 060 | 392 |

Of the 11 842 forms carrying one of the two canonical blobs, **11 821 agree and
21 do not** -- 99.8%. Two facts that no element of the source decides, and they
decide each other.

So both come from the same hidden cause: what the platform wrote when the form
was last saved. A form saved by one generation has the empty settings and no
navigator; a form saved by the other has `<outputParameters/>` and a navigator.

## What it means for the measurement

The writer picks the empty settings, so it must also write no navigator, and it
does. That pairing is self-consistent, and it is why the 41 bodies that match
match at all.

It also means **comparing against the original database undercounts**: a form
whose stored body came from the other generation can never match, however
complete the writer becomes. The closing criterion stays the one stated when
the settings blob was first read -- export → load → export, byte-identical to
the *second* export -- and against that criterion the generation difference
disappears, because both exports are written by this writer.

The 21 forms that break the pairing are the only ones where the two facts
disagree, and they are worth a look when the round trip is closed.
