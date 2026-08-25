# Form root `AutoURL`: the trailer's declared optional-block count, 20260825

Status: root cause and fix for `<AutoURL>false</AutoURL>` being dropped from
every ERP УХ form. Measured over all 19 271 form roots `cf export` walks on
the stand ($D = `/Users/untru/Documents/ChatGPT/ibcmd-stand`), across all
seven configurations.

## Symptom

On `uh`, all 271 exported forms whose native document carries
`<AutoURL>false</AutoURL>` were missing the element. On 207 of them it was
the *only* difference from native -- a clean single-tag bucket. `sslbase`
(31/31), `ssl` (44/44) and `ut` (231/231) emitted it correctly, so the
property was not broken in general, only on ERP УХ.

## Root cause

`extract_form_auto_url` reached `FormRootAutoUrlSchema::from_raw_layout`
through `form_root_child_items_tail_start`, which searches for the root
trailer at a hardcoded arity of exactly 24 members. ERP УХ's root `50`
trailers are 25 members long, so the search returned `None` and the property
was omitted before any value was ever read. Doctrine point 7 exactly: a
whitelist of arity standing in for a declared count.

The schema then compounded it with `trailer.len() != 24` and a fixed
`AUTO_URL_SLOT = 3`.

## What the trailer actually says

Trailer member 2 is a *declared count* of optional blocks sitting between
itself and the `AutoURL` flag:

```
count 0 (БСП/УТ/WMS):  "" | "" | 0 |              1 | "" | 0 0 0 0 0 0 | 3 3 0 0 0 100 1 1 0 0 0 {50,0} 1
count 1 (ERP УХ/MDM):  "" | "" | 1 | {22,{0},0,0,0,} | 0 | "" | 0 0 0 0 0 0 | 3 3 0 0 0 100 1 1 0 2 0 {50,0} 1
                                ^          block      ^
                              count                 AutoURL
```

So the trailer is `24 + count` members long and the flag sits at
`3 + count` -- never at a per-arity constant.

Over all 18 634 root `50` forms on the stand, member 2 equals
`trailer.len() - 24` with **zero** exceptions, and no form validates a
trailer at both 24 and 25 members. The member at `3 + count` reads `0` on
exactly the 587 forms whose native document carries the element and `1` on
the other 18 047, with no overlap and no third value:

| corpus  | count | trailer | `0` (element present) | `1` (absent) |
|---------|------:|--------:|----------------------:|-------------:|
| wms     |     0 |      24 |                     0 |            5 |
| sslbase |     0 |      24 |                    31 |          878 |
| ssl     |     0 |      24 |                    44 |        1 119 |
| ut      |     0 |      24 |                   231 |        4 970 |
| mdm     |     1 |      25 |                     0 |            7 |
| uh      |     1 |      25 |                   281 |       11 068 |
| **all** |       |         |               **587** |   **18 047** |

`ws` contributes no rows: it has no form roots at all.

## Fix

- `form_root_child_items_tail_start_50_24_or_25` -- a *separate* entry point
  admitting both observed shapes, deliberately not a broadened gate on the
  shared `form_root_child_items_tail_start`, whose dozen-plus callers read
  fixed start-anchored slots that would all shift by one on ERP УХ.
- `FormRootAutoUrlSchema::from_raw_layout` reads the declared count, refuses
  any trailer whose count disagrees with its own length, and derives the slot
  as `3 + count`.

Root `49` is left fail-closed: its 1 543 ERP УХ roots put a braced group
where root `50` keeps the count, and only 3 of them carry the element -- too
thin a positive population to attribute a slot from. Those 3 files stay in
the remainder.

## Measured effect

`uh` exact 119 049 -> 119 256 (**+207**, BROKEN = 0) against the same
worktree's own `41808c3` baseline. All other keys unchanged.

## Reproduction

`zsh $D/kit/run.sh <key> <worktree> <out>` on all seven keys, exact-set diff
against `$D/base789/<key>.parity.json`.

---

# The same declared count governs every root trailer property

Status: follow-up on the same day. `AutoURL` was not the only reader the
25-member ERP УХ trailer blinded -- it was one of twelve.

## Scope

`form_root_child_items_tail_start` (arity 24, hardcoded) fed twelve root
property readers, every one of which returned `None` on ERP УХ. `AutoURL` was
simply the one visible in a clean single-tag bucket.

## Measurement

For each property, `trailer[base + count]` was cross-tabulated against the
native document's own value, over all 18 634 root `50` forms on the stand
(count 0: wms, sslbase, ssl, ut; count 1: mdm, uh). Every property produced an
**identical value -> native mapping at count 0 and count 1**, with no value
mapping to two different outcomes and no disagreement between the two
populations -- zero contradictions across all twelve:

| property | base slot | uh forms with a non-default value |
|---|---:|---:|
| SaveWindowSettings | 23 | 98 |
| MobileDeviceCommandBarContent | 22 | 84 |
| Group | 14 / 21 | 69 |
| VerticalSpacing | 10 | 58 |
| ScalingMode | 6 | 33 |
| ShowCloseButton | 18 | 31 |
| ConversationsRepresentation | 19 | 29 |
| CollapseItemsByImportanceVariant | 20 | 28 |
| HorizontalSpacing | 9 | 24 |
| HorizontalAlign | 11 | 15 |
| VerticalAlign | 12 | 14 |
| ChildrenAlign | 13 | 5 |

The shift had already been found twice, one property at a time, without the
count behind it being identified: `FormRootVerticalScrollSchema` (slots 5/15
at 24 members, 6/16 at 25) and `extract_form_show_title` (17 at 24, 18 at 25).
Both are special cases of `base + count`.

## Fix

`form_root_trailer_optional_blocks` reads member 2 and verifies it against the
trailer's own length; every reader adds it to its base slot. The tail search
`form_root_trailer_start_50` admits 24 and 25.

## Measured effect

`uh` exact 119 256 -> 119 525 (**+269**, BROKEN = 0), i.e. **+476** against the
original `41808c3` base. ws/mdm/wms/sslbase/ssl/ut all BROKEN = 0, FIXED = 0 --
as predicted, since at count 0 the read is identical by construction.

Of the 285 uh forms carrying a native `<AutoURL>`, 244 are now byte-exact. The
41 remaining are: 14 never exported at all (separate root cause), 16 differing
only in `<Shortcut>` (the documented macOS/Windows host dependency --
`Cmd+T` vs `Ctrl+T`, not a defect), 3 root `49`, and 8 with unrelated
item-level tags.

## Root `49`: the same rule, applied

Root `49`'s trailer is root `50`'s minus the trailing member: `23 + count`
members, the same `base + count` slots. Over its 1 548 forms (uh 1 543, mdm 5,
all declaring count 1) the identical value tables hold with zero
contradictions, and all 1 548 validate at 24 members only -- never 23 or 25.

Applying it also closed a latent defect in `extract_form_show_title`, which
keyed its slot off the trailer's length alone (24 -> 17, 25 -> 18). That is
right for root `50` but wrong for root `49`, whose 24-member trailer declares
a count of 1 and so keeps the flag at 18. Slot 17 holds the constant `100` on
all 1 548 root `49` forms, so the property was silently dropped for every one
of them; slot 18 separates `0` -> false from `1` -> absent with no
counter-example.

The three trailer searches (`_49_or_50`, the 24-only one, and the root `50`
24-or-25 one) collapse into a single `form_root_trailer_start`: it tries the
three arities those shapes take and keeps a candidate only when its declared
count agrees with its own length, so the count -- not the arity list -- decides
which reading is real. `FormRootVerticalScrollSchema`'s three special cases
(`(50,24)` -> 5/15, `(49,24)` and `(49|50,25)` -> 6/16) turn out to be exactly
`base + count` and collapse with them.

## Final state

`uh` exact 119 049 -> **119 536** (+487, BROKEN = 0) against the `41808c3`
base. All other keys BROKEN = 0 against their own base-commit state.

Of the 285 uh forms carrying a native `<AutoURL>`, 244 are byte-exact. Of the
2 294 uh `Form.xml` still differing, only **10** mention any root trailer
property at all; the largest remaining class is 643 files differing solely in
`<Shortcut>` -- the documented macOS/Windows host dependency (`Cmd+T` vs
`Ctrl+T`, see `host-dependent-export-2214-20260823.md`), not defects.

Still open, outside the trailer: 73 files where the
`MobileDeviceCommandBarContent` block is now emitted but its item contents do
not yet match, and item-level properties (`Field`, `DataPath`,
`AdditionSource`, button/command-bar shapes).
