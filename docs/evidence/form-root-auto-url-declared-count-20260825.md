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
