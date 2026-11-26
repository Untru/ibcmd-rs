# ERP УХ form property-bag revisions, 20260825

Status: measurement, no fix shipped. Traces the 33 `uh` forms left `differing`
after the four short *item-record* revisions were closed
(`uh-form-item-tree-revision-map-20260825.md`), and finds the same defect
class one level deeper -- in the property-bag blocks *inside* an item record.
Measured at `2ccd98f` + this branch's four revisions, against
`$D/baselines/2ccd98f/` (`uh` `exact=130419`, `BROKEN=0`, `gained=45`).

**Update -- the field block shipped (`6ed3d4e`); the group block did not, and
this document's claim about it was wrong.** The section below says both blocks
are "pure tail truncations of their canonical form". For the `InputField` bag
that is true in meaning as well as in shape, and it now ships. For the
`UsualGroup` bag it is true only in *shape*: the `28` bag's `Behavior` sits at
slots 10 and 24, not at the `29` bag's slot 28, so normalizing it to `29`
reads the wrong slots. The codebase already knew this and already reads the
compact bag through dedicated readers
(`parse_form_usual_group_property_bag_behavior`'s compact sibling), with real
ERP УХ bytes behind them; my normalization broke
`extracts_compact_usual_group_collapsible_behavior` and
`extracts_compact_usual_group_horizontal_and_show_title_false`, which is how
the error surfaced. Those tests were right and the change was wrong; the
group half was dropped.

**The correction that matters beyond this document:** a matching slot *shape*
(`b`/`q`/`s` per member) is necessary evidence for a truncation and is not
sufficient. Every short revision in the sibling map
(`uh-form-item-tree-revision-map-20260825.md`) was additionally confirmed by
its own byte-level reading, and that is what earns the claim -- not the shape
comparison on its own. For this fix the confirmation is
`Reports/РегламентированныйОтчетСтатистикаФорма1Т/Forms/ОсновнаяФорма`'s
`InputField` `ПолеРедакцияФормы` id 44, whose eight boolean options come out
of the short bag exactly as native ibcmd writes them, two of them `true`.

Gate against `$D/baselines/999565e/`: `uh` `130419 -> 130514` (`gained=95`,
`BROKEN=0`), the other six corpora byte-for-byte unchanged. `BROKEN=0` is
substantive here rather than merely reassuring: 145 already-byte-exact forms
carry a `32` bag, and a wrong slot mapping would have broken them by emitting
properties native omits.

Still open in this family, each needing its own reading rather than a shape
argument: the `UsualGroup` `28` bag's remaining properties (`Representation`
among them, which `parse_form_usual_group_extended_options` supplies only for
`29`), `Page`'s `17`/18 bag against its canonical `18`/20 (73 records, and its
`len - lead` is not constant, so it is a different shape rather than a
truncation), and `Pages`' `3`/5 against `4`/6 (25 records).

**Answer up front.** All 33 are explained, and by one construction. An item
record carries its properties in a nested block whose own leading member is,
again, that block's declared length -- and the readers again whitelist the
lengths they have seen. Two such blocks were found short in ERP УХ:

```
block                        canonical      short        records   forms
InputField extended options  36 / 66 mem    32 / 62 mem    4 717     785
UsualGroup extended options  29 / 29 mem    28 / 28 mem      762     363
```

Both are pure tail truncations of their canonical form, with exactly one slot
shape each corpus-wide: the `32` block reproduces a `36` block's shape for all
62 of its members and `36` adds four (`bsbs`); the `28` block is a `29` block
minus its final scalar. `len - lead` is 30 for both field blocks and 0 for
both group blocks -- the leading member is the declared length, literally.

The readers:

* `form_input_field_extended_options` (`src/mssql_dump/form_body.rs`) scans
  from slot 39 for the first nested block and accepts it only when its leading
  member is `matches!(options.first().copied(), Some("36" | "38"))`.
* `parse_form_usual_group_extended_options` matches `options.first()?.trim()`
  against `"29"` and returns `None` for anything else.

Neither is arity-driven; both are literal whitelists, which is doctrine point
7 at a third level. Note also that the field reader compares
`options.first().copied()` *without* `trim()`, unlike its group counterpart --
latent, not the cause of anything measured here, but worth fixing alongside.

## What the 33 actually look like

Every difference is an element we **omit**; no wrong values, no extra
elements, and only 28 items dropped across all 32 captured files (0 invented).
The item trees are complete -- this is purely property-level.

19 of the 32 share one identical changed-tag signature:

```
AutoMaxWidth, ChoiceButton, ClearButton, DropListButton, ExtendedEdit,
HorizontalAlign, ListChoiceMode, PasswordMode, Representation, TextEdit
```

Those 19 are the `Reports/РегламентированныйОтчетСтатистика*/Forms/ОсновнаяФорма`
family. A representative diff
(`Reports/РегламентированныйОтчетСтатистикаФорма1Т/.../ОсновнаяФорма`) is
nothing but missing lines:

```
< <Representation>None</Representation>            (UsualGroup, 4 occurrences)
< <AutoMaxWidth>false</AutoMaxWidth>
< <PasswordMode>false</PasswordMode>
< <ExtendedEdit>false</ExtendedEdit>
< <DropListButton>true</DropListButton>
< <ChoiceButton>false</ChoiceButton>
< <ClearButton>false</ClearButton>
< <ListChoiceMode>true</ListChoiceMode>
< <TextEdit>false</TextEdit>
```

`<Representation>` comes from the group block, the rest from the field block.
Every item record in that form is itself a short revision this branch now
reads (`Button` `30` x2, decoration `11` x3, field `34` x5, groups `22` at 34,
36 and 38 members) -- which is exactly why these forms surface now: the tree
had to come back before the property gap could be the last thing wrong.

**All 33 carry at least one short options block.** None is left over for
another cause.

## Why the correlation is not total, unlike the item records

For the item-record revisions, no byte-exact form carried one -- 0 of 9 431.
Here 254 of the 969 carrier forms are already `exact`. That is expected and
not a contradiction: failing to read a property block only shows up when a
property in it is non-default, since the platform omits defaults from the XML
either way. So the carrier set is an upper bound on the affected forms, not a
prediction of them.

```
field-options 32   785 forms   634 differing   145 exact    6 missing
group-options 28   363 forms   243 differing   113 exact    7 missing
either             969 forms   708 differing   254 exact
```

## What a fix would have to establish

The same normalization this branch already uses for item records applies
directly -- rewrite the leading member to the canonical length and pad the
dropped trailing members as absent -- but two things need checking first, and
neither is checked here:

1. **Whether the dropped members carry anything the reader wants.** No option
   slot constant in `src/form_schema.rs` lands in the field block's dropped
   tail (members 62..65) or the group block's (member 28) -- the highest is
   `OPTIONS_SLOT`/`OPTIONS_BASE_SLOT` at 39, and the group's own option slots
   top out around 20. So on present evidence admitting the short block
   surfaces every property the reader knows how to read, and the padding is
   never reached. This is an argument from the code, not from bytes, and
   should be confirmed by measuring a short block against a canonical twin on
   a form whose native XML carries the properties.
2. **Whether `38` is real.** The field reader already admits `36 | 38`, but
   `38` does not occur once in the census over all six corpora that have
   forms. Either it is reached through a path this census does not walk, or it
   is a literal nobody has evidence for -- worth resolving before adding a
   third value next to it.

Blast radius if it lands: an upper bound of 708 currently-`differing` forms,
of which the 33 traced here are the ones where it is demonstrably the *only*
remaining defect.

## Method

Class-uuid-anchored walk of the 12 215 dumped `uh` form layouts (plus the
5 202 `ut`, 1 163 `ssl`, 910 `sslbase`, 12 `mdm` and 6 `wms` ones), taking for
each item record the nested block its reader would read -- slot >= 39 for the
field class, slot 20/21 for a `UsualGroup` -- and tallying leading member,
length, slot shape and trailing scalar run. Shapes were compared as strings
(`b` block / `q` quoted / `s` scalar) to test the truncation relation without
assuming which members mean what. Per-file attribution came from intersecting
the carrier sets with the run's own `exact`/`differing`/`missing` partition.
