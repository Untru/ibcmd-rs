# "Почти пустое тело формы": scope resolution and one confirmed cause, 20260825

Status: the ~530-file "near-empty form body" bucket this pass was assigned
does **not** have one root cause -- it is a *symptom* shared by at least two
structurally unrelated defect classes, one of which (short item-record
revisions) is explicitly owned by a parallel session. This pass isolated and
fixed the piece that was genuinely free: `FormConditionalGroupSchema`
refusing three wrapper-`22` discriminators it should have admitted.

## The bucket does not have a single cause

The task handoff described the bucket as "~530 forms losing `ChildItems`/
`Table` content almost entirely, 15+ tags diverging at once" and noted a
prior session on the same topic "ended without shipping a result." Both
facts turned out to have the same explanation.

Classifying the `uh` `differing` set by content specifically inside each
form's outer `<ChildItems>...</ChildItems>` block (not whole-file line
counts, which are diluted by `Attributes`/`Commands`/`Events` scaffolding
that never shrinks) found 1,338 of the 2,268 differing forms losing
*something* from that block. Tracing concrete examples back to raw CF bytes
(`ibcmd-rs cf extract <cf> <uuid>.0 <outdir>`, `--compression raw-deflate`)
showed every one of the first several samples reducing to the *same*
mechanism already flagged as a parallel session's territory
(`form-item-record-revisions.md`): a form-item record whose leading member
declares a *shorter* revision than the reader's arity whitelist admits
(`Button` under wrapper `30` instead of `31`/`34`, a decoration under `11`
at 35/36 members instead of `12`). `form_child_item_tag` returns `None` for
the unrecognized wrapper, the whole item is dropped, and because
`parse_form_child_item_pairs` requires `items.len() == count` *exactly*
before accepting a candidate list as complete, one unrecognized item
anywhere in an array silently empties that entire array via its
best-effort partial-match fallback -- not just the one item. A `Button
30` sitting alone in a form root's built-in `AutoCommandBar` therefore wipes
that `AutoCommandBar`'s whole `<ChildItems>`; a `Button 30` or decoration
`11` buried one level inside a large nested menu wipes everything else at
that same nesting level too, which is why some near-empty-body files lose
hundreds of lines from a single bad item deep in an otherwise-healthy tree
(`Documents/ВерсияДокументацииЗакупочныхПроцедур/Forms/ФормаРедактированияТекстаЗакупочнойПроцедуры`:
one dropped `CommandBar`, 1,054 native lines gone, ~768 tags).

A regex scan of the whole `uh` corpus for forms whose native document
carries `<AutoCommandBar name="..." id="-1">` with a non-empty
`<ChildItems>` that comes out empty or self-closed in ours found 129 such
forms; every sampled case traced to the same `Button 30` mechanism. That
confirms the short-revision bucket, not a container-parsing defect, is the
dominant cause of "root command bar loses its buttons" specifically -- and
by extension a large share of the near-empty-body bucket generally, since
the root `AutoCommandBar` is the single most common place a form's *entire*
visible command surface lives in one small array.

**Consequence for planning**: once the short-revision session lands
recognition for `Button 30` and the decoration `11` shape, a large fraction
of the remaining near-empty-body files should resolve without further work
here -- the two buckets were never independent, and assigning them to
separate sessions risked (and in the prior session's case, apparently did)
duplicate investigation into the same root bytes.

## What was independently mine: `FormConditionalGroupSchema`'s missing three

Not every near-empty-body case was a short-revision item. Diffing all
"vanished-subtree" deletions (contiguous native-only line ranges the
`ours` tree cut cleanly, no reordering) across the whole corpus and
grouping by the first line's own tag turned up large populations rooted at
`<ChildItems>` (195 occurrences), `<CommandBar>` (169), `<Popup>` (95) and
`<ButtonGroup>` (92) that were *not* explained by wrapper-`30`/`11`
recognition -- their raw item records used the ordinary, already-recognized
wrappers (`22`/`0`, `22`/`1`, `22`/`6`) and canonical field counts.

Ground-truthing one concrete case
(`Documents/ВерсияДокументацииЗакупочныхПроцедур/Forms/ФормаРедактированияТекстаЗакупочнойПроцедуры`,
`CommandBar` id `73`) with a temporary instrumented `cargo test` against the
real raw blob (not a hand simulation -- see method note below) showed
`form_child_item_tag("22", fields)` returning `None` even though the
record's own shape was unremarkable: 35 top-level fields, the conditional
`UserVisible`-common prefix tuple `{0,{0,{"B",1},0}}` at slot 5, shifted
discriminator `"0"` (CommandBar) at slot 6.

`FormConditionalGroupSchema::from_raw_layout` (`src/form_schema.rs`) is the
function responsible for detecting that prefix tuple and telling every
downstream reader (`form_child_item_tag`, `parse_form_child_item_name`, the
field-normalization that strips the tuple before recursing into the item's
own children) to shift by one slot. Its match arms admitted shifted
discriminators `2`/`3`/`4`/`5` (`ColumnGroup`/`Pages`/`Page`/`UsualGroup`)
on a `field_count >= 31, (field_count - 31) % 2 == 0` floor, and separately
`8`/`9` (`Table`'s own `ContextMenu`/`AutoCommandBar`) on a shorter
`field_count >= 30` floor -- but never `0` (`CommandBar`), `1` (`Popup`) or
`6` (`ButtonGroup`). The neighbouring `FormChildItemVisibleSchema`, a few
hundred lines below in the same file, already lists all seven wrapper-`22`
kinds together for the identical tuple (`field_count >= 30`), so the
codebase had already independently confirmed the shape exists for these
three kinds -- `FormConditionalGroupSchema` had simply never been extended
to match.

Without the shift, the discriminator/name read lands on the prefix tuple's
own opening brace (never a bare `"0"`.."9"` digit, never a quoted string),
both `form_child_item_tag` and `parse_form_child_item_name` refuse, and
`parse_form_child_item_with_metadata_owners` returns `None` for the whole
item -- doctrine point 2/6 exactly: a silent default (the item vanishes from
its parent's best-effort partial list), not a typed refusal.

### Fix

Folded `"0"`, `"1"`, `"6"` into the existing `31 + 2k`-floor arm alongside
`"2"`..`"5"`, with a doc comment recording the real-byte evidence. Left the
`8`/`9` arm (shorter `30`-floor) untouched -- confirmed independently on
`ContextMenu`/`AutoCommandBar` already, not part of this gap.

Updated `form_schema::unemitted_property_tests::conditional_group_prefix_admits_pages`,
a pre-existing test that had *encoded* the old, incomplete belief as a
negative assertion ("the tags that never carry the prefix stay out",
covering `0`/`1`/`6`/`8`/`9`). Per doctrine point 8, a failing test can
codify an old rule; the corpus-derived evidence above is the rewrite. `8`/`9`
stay in the negative list -- they are still correctly excluded *from this
arm* at these particular (odd, `31 + 2k`) counts, since they use the
different even `30 + 2k` floor one arm below.

### Confirmed on real bytes

Three independent ERP УХ 3.2.12.6 forms, all field_count 35 (`k = 2`):

| discriminator | tag | form | item |
|---|---|---|---|
| `0` | `CommandBar` | `Documents/ВерсияДокументацииЗакупочныхПроцедур/Forms/ФормаРедактированияТекстаЗакупочнойПроцедуры` | `ГруппаКоманднаяПанель` id 73 (a deeply nested "Вставить в текст" insert-phrase menu; losing it dropped 1,054 native lines) |
| `1` | `Popup` | `DocumentJournals/ДвижениеИнвестиций/Forms/ФормаРеестраИнвестиций` | `ИзменитьДолюУчастия` id 107 |
| `6` | `ButtonGroup` | `Catalogs/БланкиОтчетов/Forms/ФормаВыбора` | `ГруппаСтандартные` id 28 -- the *sole* child of the form root's own `AutoCommandBar`, so losing it emptied that `AutoCommandBar`'s `<ChildItems>` entirely, not just this one item |

Fixtures: `tests/fixtures/native-evidence/8.3.27.2214/form-conditional-group-command-bar-popup-buttongroup/`
(raw `.deflate` + native XML fragment + `manifest.json` per object).
Regression tests: `extracts_command_bar_with_conditional_group_prefix`,
`extracts_popup_with_conditional_group_prefix`,
`extracts_button_group_with_conditional_group_prefix`
(`src/mssql_dump/tests.rs`).

### Gate

`uh`: `BROKEN=0`, `FIXED=48` against `$D/baselines/2ccd98f/uh.parity.json`.
`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`: `BROKEN=0`, `FIXED=0` -- this exact
shape was not observed carrying the conditional prefix in the sampled
corpora, consistent with `form-item-record-revisions.md`'s note that short
revisions (and, it now appears, this adjacent gap) are a `uh`-only
phenomenon. `cargo test --lib`: 2,307 passed / 33 failed, the same 33 names
as `$D/baselines/2ccd98f/fail-base.txt`. `cargo test -p ibcmd-schema`: 108.
`cargo test -p ibcmd-xml`: 262. `bundled9`: 9/9. `cargo fmt --check` and
`git diff --check` clean.

48 is smaller than the ~530 estimate because most near-empty-body files
carry the *other* (short-revision) defect, not this one, and some carry
both -- fixing only this gap does not make a file byte-exact if a
`Button 30`/decoration-`11` item elsewhere in the same tree still drops
its own subtree.

## Method note: hand simulation is not proof, instrumented `cargo test` is

Two false leads worth recording so the next pass does not repeat them:

1. A from-scratch Python replica of `parse_form_child_item_pairs`'s
   brute-force count-list scan (necessary to reason about candidate
   indices without rebuilding) initially used a *stricter* UUID pattern
   (the 8-4-4-4-12 grouping) than the real `is_uuid_text` (length 36, any
   mix of hex digits and `-`, no grouping requirement). That specific gap
   did not change the outcome here, but it is exactly the kind of
   silent-divergence risk hand simulation carries generally. Once the
   candidate mechanism looked plausible, the actual root cause was found
   by temporarily instrumenting the real function (`eprintln!` gated on
   `std::env::var`, removed before the final commit -- never left in) and
   running it through `cargo test --lib <name> -- --nocapture` against a
   real blob loaded via `include_bytes!`, which is both faster (~1s vs. a
   6-9 minute full corpus export) and authoritative in a way a hand
   replica cannot be.
2. `cf extract <cf> <uuid>.0 <outdir>` (raw-deflate) plus a tiny
   `include_bytes!` + `extract_form_body_xml` test is the fast path for
   verifying a *specific* form's fix without a full corpus export -- use it
   before reaching for `run.sh uh`.

## Operational hazard: shared scratchpad paths collide across sessions

Mid-pass, a from-scratch `uh` export to the scratchpad's `uh-out` directory
was silently corrupted when a *different* worktree-agent, sharing the exact
same scratchpad path (the session-scoped scratchpad directory appears to be
shared across the whole swarm of worktree-agents spawned under one parent
orchestration, not private per worktree as the harness documentation
states), concurrently ran its own `run.sh uh ... uh-out` -- same conventional
output name, same `rm -rf "$OUT"` at the top of `run.sh`, same target
directory. The two processes' outputs interleaved: aggregate corpus
statistics (differing/missing counts across all ~140k files) swung by
several thousand between two exports of the *same, unmodified* binary
minutes apart, though the `uh` *forms* subset specifically (12,997 files)
was unaffected in both runs -- apparently because forms are written early/
small enough to finish before the collision's damage landed on larger,
later-processed entries. Anyone doing corpus-wide (not forms-only)
statistics from a `run.sh`-produced tree in a shared scratchpad should
either confirm no other `run.sh uh`/`run.sh ut` process is concurrently
targeting the identical output path, or use a per-session-unique output
directory name (not the examples' conventional `uh-out`) to avoid this
entirely. This pass switched to a unique suffix (`uh-out-<worktree-id>`)
after discovering the collision and re-ran every corpus-wide check against
the clean tree before trusting any number derived from it.

## Files

- Code: `src/form_schema.rs` (`FormConditionalGroupSchema::from_raw_layout`),
  `src/mssql_dump/tests.rs` (updated
  `conditional_group_prefix_admits_pages`, three new fixture tests).
- Fixtures: `tests/fixtures/native-evidence/8.3.27.2214/form-conditional-group-command-bar-popup-buttongroup/`.
- Commit: `bd45064`.

## What is still open

- The short item-record revision bucket (`Button 30`, decoration `11` at
  35/36 members, and any siblings) remains the dominant cause of the
  near-empty-body symptom and is owned by a parallel session -- see
  `form-item-record-revisions.md`. Once it lands, re-measure the
  near-empty-body bucket's remaining size before assuming more work is
  needed here.
- The 195/169/95/92 "vanished-subtree" tag histogram (`ChildItems`/
  `CommandBar`/`Popup`/`ButtonGroup`) computed for this pass was **not**
  re-derived against the clean tree after the scratchpad-collision
  discovery (only the specific fixed cases and the aggregate `uh` gate
  were re-verified cleanly) -- the true post-fix distribution of remaining
  causes within that histogram is unknown and would be a reasonable
  starting point for the next pass, once it has been re-run against a
  private, uncontended export.
- `Documents/ВерсияДокументацииЗакупочныхПроцедур/Forms/ФормаРедактированияТекстаЗакупочнойПроцедуры`
  is very unlikely to be fully byte-exact even after this fix (a
  919-tag native body, one of many nested items) -- it was used as a
  ground-truth trace, not confirmed end-to-end against native bytes.
