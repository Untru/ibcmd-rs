# The writer is wired, and load parity is now measurable

Date: 2026-09-21. Command:

```
ibcmd-rs audit-native-form-writer <native source tree> <inflated bodies>
```

Run over the native export of ERP УХ and the inflated form bodies of the same
database. Report: `F:\ibcmd\lab\native-writer-audit.json`.

## What it does

For every `Form.xml` of a source tree it writes the body the platform would
store and compares it to the body the platform **did** store. A form the writer
refuses is counted as refused, never as wrong -- the writer is fail-closed, so
a body is either what the platform would have written or is not written at all.

This is the end-to-end load-parity measurement. Until now every number in this
change measured one record against the corpus; this one measures whole bodies
through the real code path, from `Form.xml` to body text.

## The first run

```
forms 13044 | compared 0 | exact 0 | different 0
```

Nothing is written yet, and the refusal histogram is the whole roadmap:

| forms | refused because |
|---|---|
| **10 171** | **child items -- the item writers are not wired** |
| 1 455 | the parser rejects an `<ExcludedCommand>` spelling |
| 569 | the parser rejects a conditional appearance |
| 401 | the parser rejects a list setting -- a filter, an order, an appearance |
| 195 | the parser cannot read the DCS children |
| 90 | attributes |
| 85 | a `<MobileDeviceCommandBarContent>` |
| 29 | the parser rejects a `<CurrentRowUse>` spelling |
| 8 | no stored body in the dump |
| 3 | parameters or commands |

**78% of the corpus is held up by one thing**: the item records are written and
measured -- containers 100%, fields 99.89%, buttons 99.72%, decorations 100%,
tables 99.96% and 99.81% -- but nothing calls them yet. That is the next step,
and it is worth more than everything else on the list put together.

The next-largest group, 2 620 forms, is not a writer gap at all: the XML parser
that feeds the writer refuses spellings of its own, in excluded commands, in
conditional appearance and in list settings. Those are separate, and each one
is small.
