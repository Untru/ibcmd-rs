# Backend-neutral planner contract

The mechanical dependency is crate::import_plan::prepare(effective_rows,
source, selection, platform), under the existing export-engine lock.

- effective_rows is an owned BTreeMap<String, Vec<u8>> of immutable, already
  validated effective CONFIG + CONFIGSAVE packed entries.
- source is the original HeldSourceRoot, not a fresh authorization by pathname.
- Selection and MissingReference remain reachable at file_import's public paths
  through reexports; their fields/serialization are unchanged.
- Prepared owns source, changed files, reference diagnostics and either fully
  compiled/mandatory-verified packed entries or no entries for unresolved refs.
- The caller retains database identity/schema/readset/CAS/locks, source final
  revalidation after adapter packing, cancellation and publication outcomes.
- The shared planner has no session, database pathname or publication method.
- FileDb still assembles its effective rows and materializes its original row
  format/deletion list outside this planner; all final guards and Plan methods
  remain in file_import unchanged. SQL needs its own immutable snapshot and
  exact unselected-stage preservation, not the FileDb whole-stage conversion.

Preservation/refusal contracts precede the extraction: one test seeds a valid
previous Language change in effective rows, prepares a different module change,
and requires the previous language payload to remain byte-identical. Another
removes an actual existing baseline module (retaining its artifact outside the source) and adds an unsupported new body; both must refuse. This pure planner test is not adapter/no-publication evidence.
Existing FileDb partial/reference/stale/cancellation tests remain unchanged.
The original e6150297 checkpoint passed the two neutral planner unit tests,
14 unchanged FileDb integration tests and six source admission tests. The
corrected 59a4a8f4/c9a229fa pair differs only by an unused import deletion and
passed a library check. A cleanup unit has removal assertions while its runtime
scratch honors KEEP1;
any failure/pre-existing label requires a base/head reproduction, not a waiver.


## Opt-in raw compiler evidence (diagnostic-only implementation)

The additive prepare_with_export_evidence path is separate from prepare, so the
FileDb call does not run a second export or acquire a new refusal policy. Capture
old StorageImageSourceExportReport.storage and descriptor candidate bytes right
after baseline export, before selected fs::write. Capture new raw report and
candidates only by exporting the independently verified compiled CF. Retain the
raw vectors/notes/dispositions and full packed key sets; never reduce first via
load::keys_by_output or ConfigIndex.

Evidence binds actual caller snapshot/schema/profile/epoch plus internally
derived source census, exact selection/platform and old/new packed-map digests.
It is not a write token. Bounds and cancellation cover report entries/edges/text,
packed bytes and descriptor counts/bytes. An unknown profile/version refuses the
opt-in candidate without changing prepare's FileDb behavior.

A pure claims builder preserves raw duplicate occurrences, validates paths with
the existing SourceFileDigest canonical path policy and detects exact/case-fold
collisions before returning an unambiguous mapping. One key may emit multiple
distinct outputs. Unsupported/opaque entries and notes remain in raw evidence;
a selected unsupported key or omitted output refuses. Supported is not a
completeness certificate, and ConfigIndex/StoredObject are not ambiguity or
complete-parent-chain certificates. No production constructor may manufacture
those missing proofs from raw strings; the SQL candidate remains insufficient
until the separately reviewed engine witness seam supplies them.

## Real diagnostic consumer (focused synthetic verification)

The SQL-owned compiler_diagnostic::inspect_provided_view library function consumes
bounded SourceAdmission and calls the opt-in planner. Its input is explicitly a
PROVIDED packed view and opaque context labels, not a certified SQL effective
snapshot. No RPC dispatch, DB client call, accepted capability or publication
method is added. The raw witness API remains crate-private.

The consumer emits only counts and framed digests of descriptor candidates,
exact output-pair records and bindings. Raw paths/descriptor bytes/notes remain
internal. Typed stage errors have no private source/compiler chains; cancellation
is preserved by type. A known mapping ambiguity returns OutputMappingUnavailable
with no filtered pair subset. A stable mapping still returns the owner/
completeness refusal. Missing references remain a separate refusal. No successful
diagnostic result has database_accessed or publication_allowed set to true.
Final original-source validation and cancellation run after ledger construction.

The initial check of the unconnected private prototype compiled but added 13
owned dead-code warnings; that receipt is retained. No warning suppression,
fictitious call or public raw API was introduced. The real consumer passed a new library check with zero warnings in the owned
files (98 warnings elsewhere in the root library). Scoped fresh reviews approved
the raw evidence, diagnostic consumer and test corrections. The separately
allocated runtime passed 15 planner/witness tests, 14 unchanged FileDb regressions
and 29 SQL contract tests, with retained external receipts identifying exact code
hashes. A later test-only oracle tightening passed one separately allocated exact
smoke; broader suites remain attributed to their earlier recorded hashes.
These results do not establish owner completeness, SQL provenance or readiness.


### Compiler lifetime and test isolation

The existing offline metadata compiler installs PREFETCHED_BASE_ROWS once per
process before scanning. It refuses a later metadata compile, including after a
later-phase failure. The diagnostic consumer inherits this limitation. Module
patches can follow other paths; this is not a promise that all preparations are
reusable. Production state is never reset by this change.

Three metadata-CF fixture cases run in serialized fresh Rust test processes.
Each child inherits external TMPDIR/KEEP1, uses an exact case marker, and must
exit successfully with exactly one passed and zero failed test. Original test
assertions remain inside each child. This isolates test state, not production
compiler lifetime; a reusable worker/scoped-engine design requires separate
review before repeated SQL preparations are advertised. No native platform or
shell fallback is introduced.

The retained public 8.5.1 CF fixture diagnostic observed the owner/completeness
refusal with one output pair and three old/new descriptor candidates. Its final
oracle requires that exact refusal and one pair; it no longer accepts mapping
refusal as a substitute. This is diagnostic-only synthetic execution, not real
SQL/native acceptance or mutation authority.

## Passive engine observer adapter (integration contract)

The opt-in raw-evidence path may synchronously observe immutable assembled packed
entries through the engine-owned `import_evidence::observe_view` seam. This does
not change legacy FileDb `prepare`. The supplied origin/table/profile strings
remain assertions; an occurrence ordinal identifies an entry in an already
reduced provided map, never a SQL PartNo or proof of physical uniqueness/source
completeness. No SQL headers are synthesized and no parent source handle is passed.

Old observation binds the complete baseline map before source edits. New
observation binds the complete compiled artifact map before service filtering;
it is admitted only after mandatory staged verification and artifact hash checks.
The ordered observation digest and canonical planner map digest remain distinct
and are retained together. Failed verification returns neither as accepted
evidence. Only redacted counts/digests and unavailable dimensions leave the
private adapter; raw labels, header candidates and collection contents do not.

The initial adapter uses `descriptor_bytes` as the combined inflated+normalized
observer cap, and `text_bytes` as the separate label cap. These caps do not bound
the total Capture allocator. It uses conservative non-transferable old/new
budget partitions:
each view receives floor(total/2) for candidates, label bytes, packed bytes and
collection items. Both inflated and normalized representations each receive
floor(total/4) per view against the one combined text budget. Thus the sum of both admitted
views cannot exceed the one request budget, even though the engine API does not
report consumed allocation counters. Odd remainders and unused capacity are not
reused; tiny/invalid partitions refuse. These are observer input/output bounds,
not a total allocator or decoder CPU bound. Cancellation restores the original
typed parent error rather than stringifying it into a generic compilation error.

Recognized graph candidates and key-anchored header UUIDs establish no independent
owner identity. Full parent, body role, raw profile, versions, projection and owner
completeness remain Unsupported; the diagnostic consumer still refuses authority.
No observer result can reach a SQL writer or change final source/cancel guards.

### Directory membership dependency

The current `source_census` binding hashes existing file observations. Bounded
traversal and ancestor custody do not extend it to all directory membership or
empty-directory changes. Passive observations do not fill that gap. Strict
all-entry claims require the separately owned source-census donor, pinned review
and actual consumer negative controls before integration; this adapter remains
diagnostic-only while that dependency is pending.
