# Hardcoded-arity / literal-header-search audit, 20260825

Status: a full sweep of `src/mssql_dump/` for the recurring defect class
`ab58c3f`, `0575505`, `06249bd` and `ab3466d`/`uh-missing-root-cause-map-
20260825.md`'s "third UH pass" note already fixed independently: code that
locates a variable-arity 1C header/wrapper block by a hardcoded literal
(`rfind("{3,")`, `.len() != 9`) instead of reading the block's own declared
member count back from the text. The platform omits certain optional
trailing members from a `{N, member1, ...}` record (and decrements the
wrapper's own leading count digit `N` to match) whenever those members are
left at their default value; code that assumes one fixed `N` either finds
nothing, finds the *wrong* occurrence of the literal elsewhere in the
record, or (worst) accepts a structurally-unrelated block that happens to
share the same leading digit -- silently producing wrong output instead of
a typed refusal (doctrine point 2/6).

## Method

```
grep -rn 'rfind("{\|\.len() != [0-9]\|len() == [0-9]' src/mssql_dump/ | grep -v tests
```

515 hits total: 5 `rfind("{<literal>"` (read individually, full function
context each) and ~510 `.len() != N`/`.len() == N` (read in batches by
surrounding function, filtered for anything touching a `Name`/`Synonym`/
`Comment`-shaped header, an owner/wrapper/detail block, or a
`metadata_header_field_index` call). Every hit was classified SAFE
(unrelated to this defect class -- a truly fixed-arity 1C tuple, an
emptiness check, a parsed-collection size, a different object-generation
kind, or a family whose header dispatch already runs entirely through one
of the four already-fixed shared helpers: `enclosing_counted_block_start`,
`parse_information_register_owner_header`, `parse_metadata_code27_payload_
fields`, `innermost_metadata_object_fields_around_header`) or SUSPECT
(itself hardcodes a literal/single-length check on what is, by inspection,
the same variable-arity header production). Every SUSPECT was then checked
against the real `uh` corpus (140,411 files, `$D/cap/uh-r1/src` as
reference) via that family's disposition in the `cf export` parity report,
and where real byte evidence for the short form existed or could be cheaply
obtained via targeted `cf extract` (not a full corpus re-export), fixed
with a fixture-backed, negative-control-verified regression test. Where no
short-form evidence exists in any of the seven gate corpora, recorded here
as checked-and-safe rather than spelled speculatively.

## Fixed: six new instances of the arity-omission class

All six use the identical repair: read the block's own `.len()` first,
branch on the two lengths the platform actually writes (9/8 for the
generic `Name,Synonym,Comment,0,0,NilUuid,0` header shape; 13/12 for one
outer command-block variant), pick the matching discriminator digit, and
only require the trailing optional field when the longer length says it
should be present. Five were `.len() != N` hardcodes; the sixth
(`parse_bot_properties_from_text`) had no length check at all, just a bare
`rfind("{3,")`.

| function | file | family / blast radius | real short-form evidence |
|---|---|---|---|
| `parse_web_service_header` | mod.rs | WebService root header, ~19 real objects in `uh` | **yes** -- `WebServices/ManagedApplication_1_0_0_1`, uuid `91c6887c-aa41-4a36-ae08-24a86e53c77f`, the only short form among the 19 surveyed |
| `register_common_child_header_matches` | mod.rs | InformationRegister-owned children (code27 reimplementation) | shared grammar, real bytes reused from the code27 fix (`Catalogs/ВариантыЗаполненияШаблонов`) |
| `parse_information_register_child_value_types_from_fields` | mod.rs | InformationRegister Resource/Attribute/Dimension value types (code27 reimplementation) | ditto |
| `parse_information_register_child_command_properties_from_fields` | mod.rs | InformationRegister-owned commands (two independent instances in one function: outer command block *and* nested header) | pattern already evidenced on real bytes for the sibling `parse_common_command_properties_from_text` (`{9,`/`{8,` pair); this function reimplements the same shape by hand |
| `is_configuration_root_property_header` | refs.rs | Configuration root, one object per config, but total-parse failure of `default_roles`/`use_purposes`/localized properties if hit | shared grammar; no config in the seven gate corpora happens to leave this specific header short (see caveat below) |
| `parse_bot_properties_from_text` | mod.rs | Bot, the purest instance -- no length check *at all* before this fix | shared grammar only; the one real Bot object across all seven corpora (`Bots/ОповещенияПользователейОСобытиях`) uses the full form |

Commits: `578288c` (first five), `69a78c3` (Bot). Each has its own
negative-control test (fails without the fix, applied and re-verified by
hand for every one of the six: toggle the `8 => false`/`match` branch to
`8 => return None`, confirm the new test fails, restore, confirm it passes
again). `BROKEN=0`, `gained=0` on `ws`/`mdm`/`wms`/`sslbase`/`ssl` for both
commits (exact-set diff against `$D/base789` -- none of the five small/
medium corpora happen to exercise any of these six shapes' short form).
`cargo test --lib`: 2256 passed / 33 failed after both commits (33 --
`fail-base.txt`, unchanged).

**Caveat on `is_configuration_root_property_header`:** the first attempt
at a regression test used the existing `flat_configuration_fixture()`
helper (built for an unrelated `InternalInfo`/reference-children test) and
passed *even without the fix* -- that fixture's own header-carrying
block doesn't actually reach `configuration_root_property_fields`'s
60/61/77-member match at all (a different, smaller `{67,header}` shape
used only for building contained-object references), so the "fix" and "no
fix" cases were indistinguishable through it. Caught before shipping by
checking that the *baseline* (full-header) case also failed under a
temporary revert, which it should not have. The real regression test uses
`flat_configuration_properties_text(67, 60, ...)` with genuinely non-empty
`BriefInformation`/`DetailedInformation`/`DefaultRoles` content instead
(mirroring the existing `extracts_configuration_default_roles_for_proven_
layouts` test), so a rejection is observable. Recorded here as a
methodology note: an empty/minimal fixture can't tell a silent rejection
apart from a legitimate empty result for any of these gated-properties
functions.

## Second wave: targeted length-bucket search

After the coordinator's baseline-refresh false alarm was resolved (see
`uh-missing-root-cause-map-20260825.md`), continued the sweep with a more
targeted method: instead of reading all ~505 `.len()` hits in file order,
grouped them by their literal length (`grep -oE '\.len\(\) != [0-9]+|\.len\(\)
== [0-9]+' | sort | uniq -c`) and reviewed every hit at the two lengths
that exactly match this defect class's known signatures first -- 8/9 (the
generic header) and 12/13 (the outer command-block pair
`parse_common_command_properties_from_text` already evidenced). This found
three more real instances, all sharing the identical 13/12-member,
`"9"`/`"8"`-tagged outer command-properties block:

- **`parse_owner_graph_command_identity_slot`** -- shared by
  `parse_owner_graph_command_identity_slots_indexed`'s three call sites
  (`parse_strict_catalog_properties_from_text`, `parse_document_properties_
  from_text`, `parse_business_process_properties_from_text`): Catalog,
  Document, and BusinessProcess owned commands.
- **`parse_task_commands`** (Task) and **`parse_document_journal_commands`**
  (DocumentJournal) -- hand-duplicate the identical logic instead of
  reusing the shared function above.

All three already correctly called the already-fixed
`parse_information_register_owner_header` for the *nested* header: only
the *outer* wrapper's own length/discriminator check was still hardcoded.
Fixed the same way as every other instance this pass: branch on the two
lengths, matching discriminator. Commit `2a3a382`. The shared function has
a negative-control test (`accepts_short_owner_graph_command_properties_
block`, fails without the fix, passes with it, verified by hand); the two
hand-duplicated sites received a mechanically identical code change but no
separate integration test (would need a `nested_child_commands_from_text`-
compatible full record; out of scope for the time available this pass).
`uh`: `BROKEN=0`, `gained=50` against `$D/baselines/d0457a6` -- identical
to the count before this specific fix (which was already 50 from the
first wave's six fixes), meaning this fix is confirmed non-regressive by
the full corpus but not currently load-bearing there: no real Catalog/
Document/BusinessProcess/Task/DocumentJournal command in `uh` happens to
carry the short outer-block form today. Consistent with `is_configuration_
root_property_header` and part of `parse_web_service_header`'s own outer
wrapper earlier in this pass -- applied on the strength of the shared,
now nine-times-independently-confirmed grammar production, not because
this specific corpus exercises it.

**Exhaustive real-corpus cross-check of every remaining caller.** Having
found three genuine misses this way, went further and checked *every*
caller of `parse_information_register_owner_header` (18 total) against
real `uh` bytes for whichever family had a currently-`missing` specimen
available, rather than trusting the length-bucket sample alone. Every
single one checked out: the arity check itself matched the real bytes
exactly (including several real short-8-member-header specimens), so the
object's `missing` status has a *different*, unidentified cause in each
case -- not an unfixed arity-omission bug. Confirmed this way, by family
(uuid checked via `cf extract`, member count read back with a small
Python brace-aware splitter mirroring `split_1c_braced_fields`):

| family | function checked | real specimen | result |
|---|---|---|---|
| DocumentJournal | root/owner-wrapper length (7/17) | `ДокументыАккредитации` (missing) | 7 and 17 both matched exactly; header inside was the *short* 8-member form, already handled |
| Report | strict root fields (8/18) | `ОтчетПоПоказателям` (missing) | 8 and 18 both matched; short header handled |
| CommonAttribute | additional-order tail (15) + code27 (23) | `КлассВНА` (missing) | both matched exactly (full-form header this time) |
| Task | properties block (9/52) | `БюджетнаяЗадача` (missing) | 9 and 52 both matched |
| FilterCriterion | legacy/wrapped header (3-member wrappers) | `ДокументыВНАПоОснованию` (missing) | matched; short header handled |
| ChartOfCalculationTypes | strict root (8) + typed block (63) | `Начисления`/`Удержания` (both `differing`, not `missing`) | matched on both real specimens available |
| ExchangePlan, InformationRegisterOwnerFields, SettingsStorage, WebService(root wrapper) | various fixed-root/wrapper lengths | multiple, see "Checked and confirmed safe" and body of this doc | all already validated safe (0 missing in-family, or check matched real bytes) |

This is a useful negative result in its own right: it means the
arity-omission class this document tracks is very likely now *exhausted*
as an explanation for `uh`'s remaining missing files in these particular
families -- a future pass chasing `DocumentJournals`/`Reports`/
`CommonAttributes`/`Tasks`/`FilterCriteria`'s remaining `missing` entries
should look elsewhere (DCS, form/layout content, or other
content-validation causes already flagged as out of scope for this pass),
not re-suspect arity here without new evidence.

## Checked and confirmed safe: no fix needed

- **`parse_constant_properties_from_text`'s `rfind("{16,")`** (mod.rs) --
  "16" is Constant's own fixed record-kind tag, not a variable member
  count, so this is the narrower wrong-occurrence-latch risk, not the
  arity-omission class. Checked against the real `uh` corpus: **0
  `Constants` in `missing`** (5 remain in `differing`, for an unrelated,
  separate cause -- not investigated here, out of this pass's scope). Zero
  opaque/unparseable Constants across the whole real population is direct
  evidence this literal search is not currently latching onto the wrong
  occurrence in practice.
- **`parse_defined_type_properties_from_text`'s `rfind("{0,")`** (mod.rs)
  -- flagged as the highest-risk candidate on inspection alone ("{0," is
  an extremely common literal elsewhere in this text format), but checked
  against the real `uh` corpus: **743/743 `DefinedTypes` exact, 0 missing,
  0 differing**. Full-population zero-failure evidence outweighs the
  a priori risk assessment.
- **`parse_common_module_flags_from_text`'s `rfind("{12,")`** (mod.rs) --
  "12" is CommonModule's fixed record-kind tag (analogous to Constant's
  "16"), used to locate the *owner* object one level out from the header
  wrapper `enclosing_counted_block_start` already resolves correctly.
  Checked against the real `uh` corpus: **9,391/9,391 `CommonModules`
  exact, 0 missing, 0 differing**.

These three share a different sub-class from the six fixed above (a kind
TAG that's genuinely constant by the format's own grammar, vs. a variable
member COUNT that legitimately shrinks) -- doctrine point 8 applies: a
theoretical risk isn't itself grounds to rewrite working, corpus-proven
code without an actual failing specimen. If a future corpus run surfaces
an opaque object in any of these three families, re-open this file rather
than assume the risk was fully ruled out for all possible inputs, not just
this project's seven gate corpora.

## Everything else in the 515-hit grep: safe, by pattern

The remaining ~505 `.len() != N`/`.len() == N` hits are unrelated to this
defect class. By pattern, not exhaustively re-verified line by line beyond
sampling: fixed-arity reference/type tuples (`{"#", TYPE_UUID, {1,
value}}`, always 3 members by the format's own grammar), boolean/flag
tuples of genuinely fixed width (e.g. CommonModule's 8-wide flag list,
bounded via `.take(8)` rather than assumed), UUID/hex string-length
checks unrelated to record parsing, single-byte enum-field checks,
XML/text-length checks, `Vec`/`BTreeSet` size checks on already-parsed
results (not on-disk record shape), `.len() != 0`/`== 0` emptiness checks,
and object-generation-kind discriminators that are genuinely different
record shapes rather than the same shape's optional-field variants (e.g.
`root.len() != 8/9` paired with a `get(2)` tag distinguishing two
unrelated kinds, not one kind's two lengths). The recurring `{11, ...,
header@5}` 9-field TabularSection-envelope shape (~10 hits across
`mod.rs`) is explicitly corpus-validated in its own comment ("across the
1,438 tabular sections that reach this scan in UT 11.5.27.75 exactly one
brace group encloses the marker") and already delegates its *inner*
header to the already-fixed `parse_wrapped_register_owner_header`/
`parse_information_register_owner_header`. `parse_catalog_attribute_
wrapper_fields`/`parse_document_attribute_wrapper_fields`/`parse_cct_
attribute[_properties]` all funnel through the already-fixed
`parse_metadata_code27_payload_fields` and `parse_metadata_header_from_
text` (the latter has no arity check at all -- it scans forward from the
`{1,0,uuid},` marker rather than assuming a member count, so it was never
subject to this class of bug).

## Follow-up on `innermost_metadata_object_fields_around_header`'s other two call sites

`uh-missing-root-cause-map-20260825.md`'s "third UH pass" update left the
short-header question open for this function's other two call sites
(`parse_metadata_tabular_section_properties`'s non-`DataProcessor` branch,
`http_service_child_candidates_from_text`) because the corpus survey that
found 3,000+ real short-header hits at the Catalog/DataProcessor
attribute-Pattern call site was cut short by the disk-space emergency
before reaching those object kinds. Followed up here with cheap, targeted
`cf extract` calls instead of a full corpus re-export, per the coordinator's
guidance (see Operational note):

- **`http_service_child_candidates_from_text`:** all 7 real HTTPServices in
  `uh` extracted directly by their own top-level uuid (`edo`,
  `exchange_dsl_1_0_0_1`, `Биллинг`, `ОткрытаяАвторизацияПочтовогоСервиса`,
  `ПередачаДанных`, `СерверЛояльности`, `ЭДО`). 144 URL-template/method
  headers total across the seven, **zero** short-form (`"2,"`-tagged)
  occurrences -- all 144 are the full 9-member `"3,"` form. **Checked,
  short form not observed for this call site in the real `uh` corpus.**
  This pass's fix to the shared function is still correct and non-
  regressive either way (the real-layout HTTPService test still passes
  unchanged), just not currently load-bearing here.
- **`parse_metadata_tabular_section_properties`'s non-`DataProcessor`
  branch:** checking this surfaced a *different*, independently confirmed
  bug, unrelated to the short/full header ambiguity this pass's main fix
  targeted. Real bytes (`cf extract` on `uh`'s `Catalogs/
  АналитическаяПодписка`, tabular section `РассылаемыеСостояния`, uuid
  `a1797ec2-1651-4dba-a3b6-8b2c07c2fcd1`, real non-default `ToolTip` =
  "Состояния показателей, в которых будет производиться их рассылка")
  confirmed via a direct call that `parse_metadata_tabular_section_
  properties("Catalog", ...)` returns an **empty tooltip** for this
  object -- even though its header is the *full* 9-member form, not the
  short one. The cause: `innermost_metadata_object_fields_around_header`
  correctly resolves the 2-member `{0, header}` wrapper (the same shape
  the DataProcessor branch's own dedicated parser handles differently),
  but this function's `fields.get(header_index + 1/+2/+4/+5)` reads
  assume a *much longer* record than those 2 fields -- the real tooltip
  instead lives in a sibling field of the *outer* `{11, ids..., {0,
  header}, tag, attributes, tooltip}` envelope (9 top-level members, with
  the tooltip as the *last* one, after the tabular section's own
  attribute collection), which nothing currently walks out to. This is a
  distinct defect from the arity-omission class this document otherwise
  tracks (it doesn't matter whether the header is short or full -- the
  offset math is wrong either way) and was not fixed here, for two
  reasons: it's structurally a different, larger fix (correctly locating
  and reading the *outer* envelope's own field layout, not just picking
  the right wrapper level), and its real-world impact could not be
  confirmed in the time available -- the one real specimen checked did
  **not** actually go through this legacy function at all.
  `parse_metadata_tabular_section_properties` is only reached as a
  fallback when `parse_exact_metadata_tabular_section` (a separate,
  "strict"/owner-graph-based decoder tried first, mod.rs ~line 16055)
  returns `None`; for `АналитическаяПодписка`'s tabular section the strict
  decoder succeeds, so the legacy bug never manifests in that file's
  actual output (confirmed directly: `parse_exact_metadata_tabular_
  section(...).flatten().is_some() == true` for this exact input) --
  consistent with the file being `exact` in the real `uh` parity gate.
  Whether any real `uh`/`ut`/`ssl` object actually falls through to the
  legacy path *and* has a non-default tooltip (the only way this bug can
  surface in real output) is unknown; flagging this precisely, rather than
  leaving it as a vague "still open," is this follow-up's main
  contribution -- a future pass should grep the strict decoder's own
  rejection conditions for which real objects fall through, before
  attempting a fix.
- The five `Constants/*.xml` still in `differing` on real `uh` (see
  "Checked and confirmed safe" above) are a separate, unidentified cause
  -- not the `rfind("{16,")` risk this pass ruled out, not investigated
  further here.
- `is_configuration_root_property_header`'s short form remains
  corpus-unconfirmed (see caveat above) -- fixed defensively on the
  strength of the shared grammar production being independently confirmed
  short-form-capable at six other sites now, not on a real Configuration-
  root specimen.

## Operational note

A full `cf export` of `uh` (~5.5 min, ~11 GB written) was used once early
in this pass to run the `IBCMD_DEBUG_SHORT_HEADER_SCAN` survey that
produced the 3,000+-hit confirmation above; it was killed mid-run (before
writing any of the XML tree) when a host disk-space emergency dropped free
space to 10 GB, per the coordinator's redirect. The WebService short-form
specimen (`ManagedApplication_1_0_0_1`) and the follow-up HTTPService (7
objects) and TabularSection-tooltip (1 object) checks above were instead
found via cheap, targeted `cf extract <cf> <object-uuid> <dir>` calls (each
a few hundred bytes to a few KB) against the real `uh` `1cv8.cf` directly
-- CF top-level storage element names are the metadata object's own uuid
for every family checked this way (WebService, HTTPService, Catalog), so
no full export or XML tree was needed for any of these lookups. This is
the cheaper method for "does this specific family's header ever show up
short" questions and should be preferred over a full corpus run when only
one or a few object kinds need checking; a full-corpus
`IBCMD_DEBUG_SHORT_HEADER_SCAN`-style survey is still the right tool when
the question is "does this happen anywhere across the whole corpus" for a
call site with no small, enumerable object list (as it was for the
Catalog/DataProcessor attribute-Pattern call site's 3,000+-hit
confirmation).
