# ERP УХ missing-file root-cause map, 20260825

Status: measurement of the full 1,977-file `uh` missing set from
`ab58c3f`'s report.json, of the 1,594 remaining after it, and of the 1,513
remaining after the second fix (`0575505`, see below). Produced by
cross-referencing `cf export`'s per-record `disposition`/`message` against
the `missing` set from a byte-for-byte compare against a freshly-regenerated
platform-native `uh` tree (140,411 files; the shared scratchpad tooling this
project normally reads from `$S` was wiped twice by host `/tmp` cleanup
during this pass and has since moved to `/Users/untru/Documents/ChatGPT/
ibcmd-stand`, referenced below as `$D`).

## Method

For each of the 1,977 (then 1,594) missing native paths, resolve its root
object's uuid from the native tree's own `uuid="..."` attributes, look up
that uuid's `cf export` report entry, and record its `disposition` +
normalized `message` (uuids, paths and large numbers replaced with
placeholders so identical failure *shapes* group together regardless of
which object hit them). Grouped by message, sorted by how many missing
files each shape accounts for -- the corpus's own multiplication factor,
not a guess at severity.

## Before `ab58c3f` (1,977 missing)

```
495 files / 267 roots  (failed) OpaqueDcsFormAttributesConditionalAppearance: "conditional-appearance nested filter is outside the complete one-comparison cohort"
833 files / 799 roots  (opaque) no legacy family decoder recognized this storage entry
179 files /  49 roots  (failed) Form body does not start with type marker 4
102 files /  71 roots  (failed) OpaqueDcsFormAttributesConditionalAppearance: "Form Attributes conditional-appearance storage document is unreadable"
 59 files /   9 roots  (failed) OpaqueDcsFormAttributesConditionalAppearance: "appearance parameter or color is outside the authenticated cohort"
 34 files /  26 roots  (failed) OpaqueDcsFormAttributesConditionalAppearance: "conditional-appearance item has unsupported extra children"
 20 files /   4 roots  (failed) OpaqueDcsFormAttributesConditionalAppearance: "empty conditional-appearance selection is unsupported"
 16 files /   8 roots  (failed) DCS primary-schema-parse: dataSet xsi:type is outside the cohort (+ 6 more DCS shape variants, 1-16 files each)
  6 files /   4 roots  (failed) MXL codec rejected data: native value exceeds its node bound
  ~20 files, singletons  (failed) OpaqueChoiceParameterLinks, exchange-plan content references, predefined-data references, schedule/style parse
```

The `no legacy family decoder recognized` bucket (833 files / 799 roots)
was the single largest class -- 42% of the whole missing set -- and, unlike
the DCS/form-attribute buckets, spanned *every* metadata family with a
`{1,0,<uuid>},Name,Synonym,Comment,...}`-shaped header wrapper:
`CommonModules` 232, `Constants` 122, `Catalogs` 119 (mixed cause, see
below), `Reports` 87 (ditto), `CommonCommands` 85, `Documents` 63,
`AccumulationRegisters` 17, `Roles` 16, `FilterCriteria` 14,
`DataProcessors` 13, `ChartsOfCharacteristicTypes` 12, `StyleItems` 10,
`CommandGroups` 9, `SessionParameters` 9, `BusinessProcesses` 6,
`DocumentJournals` 5, `InformationRegisters` 4, `ChartsOfAccounts` 3,
`SettingsStorages` 3, `CommonAttributes` 2, `Tasks` 1, `WebServices` 1.

## Root cause of the largest bucket, and the fix (`ab58c3f`)

Four parsers (`parse_common_module_flags_from_text`,
`parse_typed_metadata_value_types_before` for `Constant`,
`parse_command_group_properties_from_text`, and
`parse_common_command_properties_from_text`) located their object's
header-wrapper block by `rfind`-ing one hardcoded literal (`"{3,"`, `"{9,"`)
backward from a `{1,0,<uuid>}` marker, assuming a fixed member count. The
platform writes a *shorter* wrapper (one fewer trailing member -- no
optional-language `Synonym` entry, no trailing default-value `0`) whenever
an object leaves those fields at default; the fixed-literal search then
either found nothing (`CommonModule`) or, worse, latched onto the wrong
occurrence of the same digit elsewhere in the record (`Constant`, when the
inner header-wrapper happened to declare the same count as the outer
typed-value wrapper it sits inside). See `ab58c3f`'s commit message and
`src/mssql_dump/mod.rs`'s `enclosing_counted_block_start` doc comment for
the full trace, including the regression this pass caught and fixed during
verification (`CommonCommand`'s nested-owner-command context needed
marker-anchored parsing, not the position-assumes-first-child approach that
works for the standalone case).

## After `ab58c3f` (1,594 missing, −383)

Rigorous exact-set diff against the original 789b1ae binary run on the same
freshly-regenerated native tree: `BROKEN=0`, `gained=426` (`118,484 →
118,910` exact). The opaque bucket dropped from 833/799 to 450/416; every
*other* bucket (DCS conditional-appearance, form-body type-marker,
MXL/exchange-plan/schedule singletons) is byte-for-byte unchanged in count
-- confirming the fix's effect is isolated to the class it targeted.

```
450 files / 416 roots  (opaque) no legacy family decoder recognized this storage entry
```

by family:

```
119 Catalogs   87 Reports    73 CommonModules   63 Documents
 17 AccumulationRegisters    16 Roles*          14 FilterCriteria
 13 DataProcessors           12 ChartsOfCharacteristicTypes
 10 StyleItems                6 BusinessProcesses
  5 DocumentJournals          4 InformationRegisters
  3 ChartsOfAccounts          3 SettingsStorages
  2 Constants                 1 CommonAttributes/Tasks/WebServices each
```

`CommonCommands` and `CommandGroups` are fully closed (0 remaining).
`Constants` dropped from 122 to 2. `CommonModules`' remaining 73 are a
*different*, second defect this same investigation surfaced and evidenced
but did not ship a fix for -- see
`plain-text-module-body-lead-20260825.md`. `Catalogs`' and `Reports`'
remaining 119/87 are a *mix*: some are the object's own descriptor (cause
not yet identified -- these families dispatch through the owner-graph
system, not the header-wrapper parsers this pass fixed, so it is a
different code path), others are exactly the same plain-text module-body
gap as `CommonModules`' remainder (`Ext/ManagerModule.bsl`,
`Ext/ObjectModule.bsl`).

*Roles (16): excluded from this pass's scope by explicit handoff -- a
separate session owns the `{0}`-kind restriction-condition-wrapper subclass
this bucket overlaps with.

## After `0575505` (1,513 missing, −464 total, −81 from this fix)

The `Catalogs`/`Documents`/`BusinessProcess`/`ChartOfCharacteristicTypes`
owner-graph descriptor gap above *was* the same defect class after all --
just in a third location. `parse_information_register_owner_header`
(`src/mssql_dump/mod.rs`), the function `OwnerHeaderEncoding::Wrapped`
resolves to for these four families' `owner_header_slot`, hardcoded
`fields.len() != 9` and discriminator `"3"` exactly like the three
functions `ab58c3f` fixed -- and, being shared plumbing, is called at 19
sites total, reaching well past the four owner-graph families into
`InformationRegister`, `ExchangePlan`, `DocumentJournal`, `Sequence`,
`WSReference` and others. Confirmed on real bytes
(`Catalogs/АлгоритмыОпределенияБазовойДаты`, uuid
`0b69b382-479d-4709-bd5d-bc499e5b3bf5`: an 8-member, `"2"`-discriminator
wrapper at slot 9, no counterpart to the hardcoded 9-member check). Fixed
in `0575505`, gated behind the same declared-length read
`enclosing_counted_block_start` already established for the marker/rfind
parsers, plus a real-bytes regression test with a negative control (fails
without the fix, passes with it).

Verified on all seven gate corpora before shipping, given the 19-site blast
radius: `ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut` all `BROKEN=0` against
`$D/base789` (zero gains on any of the six -- none of them happen to
exercise the short-wrapper shape). `uh`: `BROKEN=0` against both `ab58c3f`
(`gained=74`) and the original 789b1ae baseline (`gained=500` total,
`118,484 → 118,984` exact). `cargo test --lib`: 2213/33 (was 2212/33),
33 failures still name-for-name identical to base789's `fail-base.txt`.

Family effect, by comparing the opaque bucket before/after this fix:

```
        before  after
Catalogs   119 -> 111   (partial -- remainder is a separate cause, see below)
Reports     87 ->  15   (mostly this fix -- Report-owned child objects reuse
                          the same shared header parser even though Report's
                          own top-level dispatch does not go through
                          OwnerHeaderEncoding::Wrapped)
Documents   63 ->  63   (unaffected -- these roots hit a different cause)
```

`CommonModules` stays at 73 (the separate, unshipped plain-text
module-body gap). The opaque bucket overall: 450/416 -> 362/329.

## What is still open

- `Catalogs` (111 remaining)/`Documents` (63)/`AccumulationRegisters`/
  `Reports` (15)/`FilterCriteria`/`DataProcessors`/
  `ChartsOfCharacteristicTypes`/`StyleItems`/`BusinessProcesses`/
  `DocumentJournals`/`InformationRegisters`/`ChartsOfAccounts`/
  `SettingsStorages`/`CommonAttributes`/`Tasks`/`WebServices`: descriptor-
  or child-level opaque roots not yet individually traced past confirming
  they are *not* explained by either fix in this pass (their counts did not
  move, or only partially moved, when each fix landed). Each family needs
  its own byte-level investigation before a fix is attempted -- two
  different functions turning out to share one root cause in this pass is
  a reason to check for a third, not a license to assume every remaining
  family does too.
- The plain-text module-body gap (`plain-text-module-body-lead-20260825.md`):
  evidenced, one fix attempt reverted after it regressed `sslbase`/`ssl`
  (spurious `Bots/.../Ext/Module.bsl` files from a `module_text_paths`
  collision the permissive content check exposed). Needs either the
  collision fixed at its source or a materially tighter content
  discriminator, verified against all seven gate corpora, not just the
  ones the fix targets.
- The `OpaqueDcsFormAttributesConditionalAppearance` reason variants (497 +
  102 + 59 + 34 + 20 = 712 files after both fixes, essentially unchanged by
  this pass -- the +2/+1 root drift on the largest variant is objects whose
  *parent* descriptor now succeeds thanks to the second fix, exposing a
  pre-existing, separate defect in one of the object's own forms that was
  previously invisible behind the parent's failure, not a new regression;
  confirmed via the rigorous exact-set `BROKEN=0` diff, which is unaffected
  by which bucket a still-missing file is classified under) and the `Form
  body does not start with type marker 4` class (179 files, fully
  unchanged) remain the two largest untouched buckets. Neither was in this
  pass's assigned scope (`CommonModules`/`Reports`/`Constants`/`Catalogs`)
  and neither was investigated here.
