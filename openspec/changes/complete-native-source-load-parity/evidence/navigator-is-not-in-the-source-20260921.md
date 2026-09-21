# The navigator is not in the form XML

2026-09-21, ERP УХ, 12 507 forms with both an XML and a stored body.

5 468 of those bodies carry a `{22,{0},0,0,0,7,"Navigator",…}` record — the
navigation panel — and 7 039 do not. **No property of the form XML decides
which.** Every scalar property of the form root, and the presence of every
element it can hold, was partitioned against the navigator: not one is pure,
and the strongest is only 60.9% against a 56.3% baseline (the metadata kind
the form belongs to, which is not a property of the form at all).

The same holds for a second fact. The root auto command bar carries an
optional block at member 4, always spelled `{0,{0,{"B",1},0}}`, in 449 of
12 506 bodies. Two bars whose XML is character-for-character identical —
`<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>` — differ on it. No
property of the bar or of the form partitions it either. It travels with the
platform generation: every one of the 449 also carries a navigator, and only
5 of them carry the short `<Settings/>` blob that the older generation writes.

## What follows

Byte-exact reproduction of a stored body from its XML is **impossible in
principle**, and the audit's `different` count can never reach zero. These
facts are of the same class as the settings-composer blob spelling: the
platform writes them from what it knows at save time, and the export does not
put them back into the source.

The closing criterion for the load side is therefore not "the body matches the
one the platform stored" but the round trip:

> export → load into an empty database → export, byte-identical to the
> **second** export.

The writer omits both facts. A body without a navigator loads, and its export
spells the same XML, which is what parity means here.
