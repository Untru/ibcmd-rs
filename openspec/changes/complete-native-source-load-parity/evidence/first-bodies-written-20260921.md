# The first form bodies written end to end

Date: 2026-09-21. Command:

```
ibcmd-rs audit-native-form-writer <native source tree> <inflated bodies>
```

over ERP УХ.

## Result

```
forms 13044 | compared 83 | exact 41 | different 42
```

**41 whole form bodies are now written byte for byte** from `Form.xml` through
the real code path -- the frame, the root record, the attributes section with
every attribute's type pattern, and the settings composer. Before this run the
writer produced nothing at all.

## What it took

Three things, in this order.

**The parser had to read more.** A form attribute's `<Title>` is on 71% of the
140 288 attributes in the corpus and the parser did not read it; neither did it
read `<SavedData>` or `<FillCheck>`, which the `{9,…}` record needs. An item's
`<Visible>`, `<Enabled>`, `<EnableContentChange>` and `<ContextMenu>` were
missing too. The parts of an attribute that name configuration objects --
`<Columns>`, `<UseAlways>`, `<FunctionalOptions>`, `<View>`, `<Edit>`,
`<Save>` -- are now recorded as refusals rather than ignored.

The new arms first fired on empty text, because the same element names appear
inside settings and appearance blocks where they carry none; guarding on
non-empty text fixed 5 395 spurious refusals.

**The writer needed the configuration.** An attribute's type pattern names
configuration types, whose uuids only the configuration can give, so the audit
now hands the writer a `MetadataSourceContext` over the tree it is reading --
the way the loader does.

**The auto command bar's payload carries `<Autofill>`**, and it is on unless
the form turns it off. Writing 0 there cost 33 of the 41: the run went from
8 exact to 41 with that one line.

## What the 42 that differ still say

Two things, both named:

* **The navigator.** 5 458 of the 12 488 root records carry a
  `{22,{0},0,0,0,7,"Navigator",…}` group in the tail and 7 030 do not, and
  **nothing in the form's own XML partitions the two** -- not a scalar
  property, and not the class of the main attribute, which splits both ways for
  every class. Where it comes from is the open question.
* **The form's own `<Enabled>`**, which the parser does not read. It is member
  15 of the root head and exactly five forms of the corpus write 0 there.
