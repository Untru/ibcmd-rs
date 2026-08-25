# Role Rights: the top-level `setForNewObjects`-default rule, 20260825

Status: closes the dominant remaining defect class in `Roles/<Name>/Ext/
Rights.xml` on ERP Управление холдингом 3.2.12.6 (base commit `41808c3`,
`$D/base789/uh.parity.json`). After `role-rights-configuration-root-
20260824.md` closed the Configuration-root parsing gap, 1,401 role Rights.xml
files remained `differing` — files whose Configuration-root `<object>` block
already byte-matched the native export, but whose *other* objects (Catalog,
Document, InformationRegister, Constant, …) did not. This pass closes two
defects, landed as two commits: the top-level `setForNewObjects`-default
rule (1,171/1,401 files) below, and a follow-on nested-object
`setForAttributesByDefault` rule (see "A third fix" at the end) that closes
most of the remainder. Combined: **1,381 of 1,401 resolved (98.6%), 20
still differing, 0 broken** by exact-set difference against
`$D/base789/uh.parity.json`.

## Root cause

`role_rights_for_xml` in `src/mssql_dump/role_rights.rs` rendered a
non-Configuration object's plain-`false`, unrestricted rights using a
patchwork of per-kind, per-name suppression rules: a hardcoded ~20-name list
for top-level `Document` objects, a 3-name list (`Edit`/`Update`/`View`) for
top-level `AccumulationRegister` objects, and a restriction-gated heuristic
(`should_suppress_plain_false_role_rights`) for `Catalog`/`Document`/
`InformationRegister`/`AccumulationRegister` objects that had *some* right
under row-level restriction. Every other top-level kind (`Constant`,
`ChartOfAccounts`, `ChartOfCharacteristicTypes`, `DataProcessor`, `Report`,
`Task`, `DocumentJournal`, …), and every top-level object of *any* kind with
no restricted right at all, fell through to the default arm: `true` (show).

The platform does not do this. Diffing our output against native for all
1,401 differing files showed 1,381/1,401 (98.6%) had their *entire* diff
explained by rights we render that native omits — overwhelmingly the
DataHistory-right family (`UpdateDataHistory`, `ReadDataHistory`,
`ViewDataHistory`, …, up to 1,312 distinct files each) and the
`Interactive*`/predefined-data family (`InteractiveDelete`,
`InteractiveDeleteMarkedPredefinedData`, …), but also plain `Edit`, `Update`,
`Insert`, `Delete`, `View`, `TotalsControl`, `InputByString` wherever they
were plain-`false` and unrestricted — on every top-level kind, not just
Document and AccumulationRegister.

## What the corpus proved

Two independent, corpus-wide checks (ERP УХ 3.2.12.6, native tree `$D/cap/
uh-r1/src`, all 2,118 roles), both scanning native XML directly (no
extraction needed — `setForNewObjects` and every right's value/restriction
are already plaintext in the native `Rights.xml`):

1. **Printed ⇒ rule.** For every top-level object (`Kind.Name`, exactly one
   `.`; excludes `Configuration.<Name>`, already proven separately) in every
   native role file: is every *printed* right either restricted, or does its
   value differ from that role's own `<setForNewObjects>`? **95,124/95,124
   printed rights confirm, 0 counterexamples.**
2. **Rule ⇒ printed** (the direction that catches under-suppression). Using
   our own parser's full per-object right list (value + restriction status
   are parsed correctly today; only *rendering* was wrong) against all 1,401
   then-differing files: for every right we hold, does `restricted ||
   value != setForNewObjects` correctly predict whether native prints it?
   **274,936/274,936 confirm, 0 violations** — both directions (rights we
   wrongly showed, and the much smaller set we already correctly showed).

The critical falsification check: is "plain `false`, unrestricted, shown
anyway" ever real, and if so under what condition? A direct census across
the whole native corpus found **10,532** such occurrences (top-level, no
restriction, value `false`, but printed) — **all 10,532 from a single role**:
`ПолныеПрава` ("full access"), the corpus's only role with
`setForNewObjects: true`. Every one of its false-shown rights is exactly the
predicted inversion: value differs from the role's own `true` default. This
is the same rule already proven for the Configuration root
(`role-rights-configuration-root-20260824.md`), just never generalized past
that one object.

**Nested objects are unaffected.** Attribute-level, standard-attribute,
tabular-section-attribute, command/resource and addressing-attribute objects
(anything with more than one `.` in the name) do not follow this rule:
`Edit`/`View` rights on 366,965/354,209 nested objects show both `true` and
`false` freely under both `setForNewObjects` values, with no suppression
pattern — the pre-existing "print every right" behavior for nested objects
is left untouched. A small residual gap here (~137 files with an extra
nested `View`) is separate and not addressed by this change.

**Empty `<object>` blocks never occur.** A direct scan of every `<object>`
element in the whole native corpus found 0 with no `<right>` children. The
new top-level rule can now empty an object's right list purely by every
right matching the role default with nothing restricted (this did not
previously happen: the old logic's unconditional-`true` fallback meant a
top-level object was never fully suppressed unless it was Document/
AccumulationRegister-shaped). `format_role_rights_xml`'s existing
empty-object omission — previously gated on `has_conditionless_restrictions`
only (`role-rights-configuration-root-20260824.md`'s value-only-mode fix) —
is generalized to skip any object whose rendered right list is empty,
regardless of why.

## The fix

`src/mssql_dump/role_rights.rs`:

- `role_rights_for_xml`: the whole per-kind/per-name suppression stack
  (`should_suppress_plain_false_role_rights`, `is_top_level_document_object`,
  `is_top_level_accumulation_register_object`,
  `is_top_level_role_rights_restriction_object`,
  `is_top_level_role_object_kind`, and the two hardcoded name lists) is
  removed and replaced with one rule for `is_top_level_rights_object_name`
  objects: `restriction_by_condition.is_some() || value != rights.
  set_for_new_objects` — textually the same predicate already used for the
  Configuration root, now applied uniformly. Nested objects keep the
  pre-existing "show everything" behavior unchanged.
- `is_top_level_rights_object_name(name)`: `name.matches('.').count() == 1`
  — a kind-agnostic replacement for the four hardcoded-kind top-level check.
- `format_role_rights_xml`: the empty-`<object>` skip is now unconditional
  (`if object_rights.is_empty() { continue; }`), not gated on
  `has_conditionless_restrictions`.

Four tests updated per this project's rule that a falling test may codify a
disproven rule and the corpus rewrite decides:

- `format_role_rights_preserves_false_rights_without_restrictions` →
  renamed `format_role_rights_top_level_object_hides_plain_false_rights_
  matching_set_for_new_objects_default`; its "CustomRight" fixture (a
  plain-false, unrestricted, unlisted right, deliberately chosen to prove
  the old per-name-list default was "show") now asserts the opposite,
  corpus-proven default ("hide").
- New `format_role_rights_top_level_object_inverts_when_set_for_new_objects_
  true`, mirroring the existing Configuration-root inversion test, for an
  ordinary top-level object under `setForNewObjects: true`.
- `writes_role_rights_to_source_layout`'s multi-object integration fixture
  (which happens to set `setForNewObjects: true` for unrelated Configuration-
  root coverage) had three right values adjusted (`Insert`/`View` on
  `Catalog.Products`, `Use` on the `WebService` operation, all flipped to
  `false`) so each fixture object keeps at least one rendered right under
  the new rule instead of becoming empty and vanishing from the ordering
  assertions; `Delete` on the register object, previously asserted absent
  under the old Document/AccumulationRegister-only suppression, is now
  correctly asserted present (it differs from the role's `true` default).
- `format_role_rights_omits_plain_false_rights_for_restriction_only_top_
  level_objects`, `format_role_rights_omits_plain_false_rights_when_only_
  view_input_by_string_are_true` and `format_role_rights_omits_non_native_
  top_level_accumulation_register_false_rights` needed no assertion changes
  — their fixtures happen to already match the new rule's predictions
  (their restricted/true-valued rights show because they differ from the
  role's `false` default or carry a restriction, not because of the old
  per-name lists), so they now serve as regression coverage for the new
  rule instead of the old one.

## Measured against the full ERP УХ gate

`$D/kit/run.sh uh <worktree> <out>` vs `$D/base789/uh.parity.json`, by
exact-set difference (never counters):

| | exact | differing | missing | extra |
| --- | ---: | ---: | ---: | ---: |
| before | 119,049 | 19,849 | 1,513 | 64 |
| after | 120,220 | 18,689 | 1,502 | 64 |
| Δ | **+1,171** | **−1,160** | **−11** | 0 |

**Broken (previously exact, now not exact): 0**, by exact-set difference
(`base_exact − after_exact`, verified directly, not inferred from counters).

Of the original 1,401 differing `Roles/*/Ext/Rights.xml`: **1,171 resolved**
(moved to `exact`), **230 still differing**, **0 moved to `missing`**, **0
vanished** from all four parity buckets (sanity check on the file-set
accounting). The `−11 missing` and part of the exact/differing shuffle come
from a second, independent fix landed in the same pass (see below); the
1,171 top-level-suppression figure above already excludes that overlap
(measured as "Roles/* files present in the original 1,401 differing set that
are no longer differing or missing").

The 230 still-differing Roles files were dominated (180/230 as the *sole*
cause) by a residual nested-object `View`-right defect, closed by a third,
follow-on fix — see "A third fix" below for that investigation, including a
regression it initially introduced and how the gates caught it.

## A second, independent fix landed in the same pass

This pass also applied and independently re-verified a rescued, previously
unlanded patch (from a session that died mid-work; its own evidence doc,
`role-rights-conditionless-restriction-20260824.md`, has unfilled `TBD`
gate numbers) that closes a different parser gap: a restriction-condition
wrapper of kind `0` (`{RIGHT_UUID,{0}}` — "conditionless": no condition
payload at all, distinct from kind `1` plain-text and kind `2`
field-referencing conditions) was unhandled and fell through to `_ => None`,
failing the whole Rights blob closed. That patch's own evidence (161
occurrences across 11 roles, ERP УХ only, 84/84 affected objects matched a
value-only rendering rule with 0 counterexamples across eight corpora) was
re-verified here independently: base blob hash of `src/mssql_dump/
role_rights.rs` at this pass's starting commit (`41808c3`) matched the
patch's own recorded base blob exactly (`be6831d`), `cargo build --release`
was clean, and `cargo test --lib role_rights` passed 47/47 (the lone
role_rights-adjacent failure, `infers_role_rights_body_path`, is
pre-existing — present in `$D/fail-base.txt`). This closes 11 of the 13
previously-flagged general-parser-gap roles from `role-rights-configuration-
root-20260824.md`'s "16 still-missing" list; the `−11 missing` line in the
table above is this fix, not the top-level-suppression fix.

## A third fix: nested attribute `View`/`Edit`, and a regression the gates caught

After the two fixes above, 230 `Roles/*/Ext/Rights.xml` remained differing.
Reclassifying their diffs by object/right showed 180/230 (78%) had their
*entire* diff explained by a single cause: an extra, plain-`false`-matching
`View` right on nested (attribute-level) objects — `InformationRegister.
nested` (95 files), `Document.nested` (78), `Catalog.nested` (58, files
overlap across kinds) and smaller counts on `AccumulationRegister`,
`DataProcessor`, `ExchangePlan`, `Report`.

**What the corpus proved.** `RoleRights` already carries a
`set_for_attributes_by_default` field (parsed, but until now unused in
rendering) — the attribute-level counterpart of `set_for_new_objects`. A
direct scan of the same native corpus (2,118 roles) found: restricted to
`View`/`Edit` rights on nested objects, **146,661/146,661 checks confirm**
`restricted || value != setForAttributesByDefault`, 0 violations, both
directions (native-printed rights, and our own parsed right list against
the 1,401 originally-differing files). Every violation found while testing
a broader "any right name" version of this rule (see below) came from
exactly one closed set of nested *categories* (the `Kind.Name.<Category>.
<Leaf>` segment): `Command`, `Subsystem`, `Operation`, `URLTemplate`,
`IntegrationServiceChannel` — these name a specific command/subsystem/
service-operation, not a data attribute, and their one right (named `View`
for `Command`/`Subsystem`, `Use` for the other three) always prints
regardless of value (1,887/1,887 confirms) — the pre-existing "print
everything" nested behavior, left unchanged for these.

**The regression, and why the gates exist.** A first version of this fix
applied the `setForAttributesByDefault` rule to *every* right on any
non-action-category nested object, gated only by category. `cargo test
--lib` passed clean, and the full `uh` gate showed 0 broken — but the `ssl`
fast gate (base789 `ssl.parity.json`, exact-set diff) caught **1 broken**
file: `Roles/_ДемоДобавлениеИзменениеЗарплаты/Ext/Rights.xml`. The cause:
`CalculationRegister._ДемоОсновныеНачисления.Recalculation.
ПерерасчетОсновныхНачислений`, a nested object with `Read`/`Update` rights
(not `View`/`Edit`) that don't occur anywhere in the ERP УХ census this
rule was proven against — SSL is a different configuration with its own
nested-category vocabulary. The category-only version of the rule wrongly
suppressed `Read`/`Update` there, emptying and omitting the whole object.
This is exactly the doctrine's point of running every fast gate, not just
the corpus the fix was derived from: a rule proven with 0 counterexamples
on one corpus can still be too broad for another. The fix was narrowed to
require *both* the right name (`View`/`Edit` only) and the category gate —
re-verified: `cargo test --lib` still matches `$D/fail-base.txt` exactly,
and every fast gate (including `ssl`) plus the full `uh` gate show 0 broken.

## Measured against the full ERP УХ gate (both fixes combined)

`$D/kit/run.sh uh <worktree> <out>` vs `$D/base789/uh.parity.json`, exact-set
difference, after all three fixes (top-level default rule, the rescued
conditionless-restriction patch, and the nested attribute default rule):

| | exact | differing | missing | extra |
| --- | ---: | ---: | ---: | ---: |
| before | 119,049 | 19,849 | 1,513 | 64 |
| after | 120,434 | 18,475 | 1,502 | 64 |
| Δ | **+1,385** | **−1,374** | **−11** | 0 |

**Broken: 0.** Of the original 1,401 differing `Roles/*/Ext/Rights.xml`:
**1,381 resolved (98.6%)**, **20 still differing**, **0 moved to `missing`**,
**0 vanished**. The 20 remaining are not addressed by this pass — likely a
mix of the previously-flagged non-`{0}` general-parser gaps and independent,
unclassified defects.

## Fast-gate parity (no-regression check, final state)

`$D/kit/run.sh <key> <worktree> <out>` vs `$D/base789/<key>.parity.json`,
exact-set difference:

| key | exact (before → after) | broken |
| --- | --- | ---: |
| `ws` | 28 → 28 | 0 |
| `mdm` | 159 → 159 | 0 |
| `sslbase` | 9,573 → 9,573 | 0 |
| `ssl` | 12,643 → 12,643 | 0 |
| `wms` | 226 → 226 | 0 |
| `ut` | 50,454 → 50,454 | 0 |

`bundled9`: 9/9. `cargo fmt --check` and `git diff --check`: clean.

## Verification

```text
cargo build --release
cargo test --lib role_rights   # 48 passed, 0 new failures
cargo test --lib                # 2,237 passed / 33 failed; the 33 are
                                 # byte-identical (by name) to $D/fail-base.txt
```

The full ERP УХ gate (140,411 files) and the six fast gates above are the
decisive corpora for this fix; their exact-set results are tabulated above,
not inferred from percentages.
