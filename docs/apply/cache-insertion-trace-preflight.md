# Cache insertion trace: admitted inputs and key coverage

These are diagnostic components for #403, in integration-test support.
It does not change the production planner or `CacheRow.exact`. Source review
and ROOT execution are separate gates. No native insertion history has been
identified by this component.

`tests/support/cache_insertion_trace.rs` reuses the public `Brace`, `SiMain`,
`TypeSets`, and `HelpProps` parsers. Before the typed parsers allocate from
declared counts, the diagnostic checks actual list cardinality with checked
arithmetic, complete UUID grammar, duplicate declarations/property IDs,
class bounds, and an explicitly declared root with a closed depth-first owner
roster. Missing, forward, cyclic, or reopened owners refuse admission.

`ManifestV1` has an explicit source head, case, stage, purpose, root identity,
hash-bound predecessor files, and exactly the three required row roles. Each
row independently binds its Params filename, part, version, locator, lengths,
and SHA256 hashes. All paths are absolute regular files with no symlink/reparse
ancestry. Locators admit a whole plain file, a whole raw-DEFLATE file, or one
exact gzip member range containing raw DEFLATE. Packed/plain hashes and full
stream consumption are checked; the rest of an append-only pack is not part
of the selected member's custody. Sources and predecessor files are checked
again after projection. There are no implicit lab paths, missing-input skips,
fixed row-count limits, decompressed-size limits, or expansion-ratio limits.

The coverage result maps every HelpProps entry to exactly one metadata record,
type-set declaration, or nil aggregate. It retains registry nodes which emit
no help key, every type set, primitive/unknown member payloads, all property
values, and their original order. The measured nil property-23 wrapper has
closed tag/class/version/count/member validation and must reference exactly
the set keys emitted in HelpProps. The historical corpus counts are evidence,
not acceptance constants. A metadata/type-set UUID collision or an unresolved
help key refuses projection.

Whole input bytes are retained independently of parsed values. The standalone
literal comparator considers BOM, CRLF, property bytes, and EOF. It does not
generate a candidate sequence from the expected row. Cache ordinals and source
record ordinals remain observations; neither establishes insertion order.

Progress for the seven-atom design:

- [x] A1.1 input-ledger/count/duplicate/owner preflight source delivered.
- [x] A1.2 complete key-origin coverage source delivered.
- [x] A1.3 bounded source owner/typed-input observations; graph gaps remain explicit.
- [x] A1.4 separate observation event contract; insertion authority remains absent.
- [ ] A1.5 candidate rendering and complete per-event difference report.
- [ ] A1.6 remaining graph/event/model controls.
- [ ] A1.7 retained corpus manifest/run/report.

Every result reports `GraphCompleteness::Partial`, `TraceStatus::NotIdentified`,
three pending atoms A1.5–A1.7 in the facts component, `candidate: None`,
and `first_visit_authority: None`. The original coverage-only result retains
its original five pending atoms for API compatibility. Unknown semantic values
are preserved, not promoted to an empty successful reference graph. These
statuses remain true even when key coverage is complete.

The ordinary generated controls cover actual rosters through 2377 entries,
positive payload preservation, empty/mismatched/overflow counts, duplicates,
UUID/domain/owner failures, nil-link shape/class/count/bijection failures,
explicit manifests for all three encodings, wrong bindings, late source or
predecessor drift, truncated/trailing streams, range overflow, and literal
byte differences. They launch no processes and contact no databases.

After independent SOURCE review, ROOT may run the named ordinary target using
its owned build/FIFO context:

```powershell
cargo test --locked --offline --test cache_insertion_trace -- --test-threads=2
```

The design's proposed ignored retained-corpus command does not exist yet.
No corpus/runtime/native acceptance is implied by source delivery or by
passing generated preflight controls. Issue #403 remains open.


The facts component accepts an explicit `FactsManifestV1` containing additional
already inflated descriptor and TypeIndex rows. It binds the exact admitted base
ledger, source head, case, stage, table, filename, part, version, regular physical
file, complete length and SHA256. Aliases/duplicate declarations/mixed contexts
refuse; no files are discovered. Retained bytes are revalidated before and after
projection. Packed supplementary rows are not admitted by this API; ROOT must
provide independently bound plain rows. The original three-role encodings stay
unchanged.

`TypeIndex` counts, UUID roles, owner/class alignment, duplicate owners,
TypeId/ValueId declarations and per-owner category indexes are checked before
its existing parser. Descriptor collection counts, own header/name/synonym
cardinality and generated UUID pairs are checked before `ObjectFacts`. Its kind
comes from the registry's actual class through the existing class lookup.
`RecordMap` supplies physical named indexes and generated positions; named
facts are ordered by those indexes, never by HashMap iteration. Missing or
unsupported descriptor maps remain explicit Partial facts.

An iterative walk records each recognized tagged metadata/TypeId/design-time
occurrence with its original row binding, hash and Brace path, including edges
inside unknown envelopes. Only SiMain's original byte spans are reported as byte
spans. Legitimate repeated references retain separate occurrences; their domain
first-observation flags do not establish insertion order. Generated ValueId,
TypeId, metadata UUID and design-time instance value UUID are distinct roles.
An instance value is never resolved merely by matching a generated ValueId.
Canonical empty metadata references and `{0,nil,nil}` design-time values are
retained as distinct `EmptyValue` observations with the complete value, row/hash
and occurrence path. They remain Partial because semantic slot roles are not
proved; nil is never resolved to an owner, type declaration or reference edge.
Malformed tag/version/arity and nil declaration identities still refuse.
Exact primitive pattern forms from the existing type encoder are observed;
unknown qualifiers/members retain their whole value and a Partial reason.

Owner visits, reference visits, TypeSet member visits, the observed nil aggregate
and its property23 link occurrences are separate events. Nil links preserve the
original vector order; this does not establish construction/link/insertion time.
The current component emits no `EmitKey` event, insertion candidate or authority.
It reuses the existing typed parsers and does not call the hash/order model or
mutate the production planner. Native HelpProps order is never a traversal seed.

The existing public RecordMap API does not expose all semantic slot roles, and
its tag selection does not close family version/constant-tail semantics. Thus
bare UUID/field references, unmapped descendant semantics, unsupported classes,
missing inputs, unresolved type/reference targets and dispatch timing remain
specific gaps. The complete named values and original rows are retained. A
successful observation projection does not claim a complete reference graph,
full family admission, native parity, or closure of #403. Subsequent A1.5–A1.7
work and any proposed H1 still need their own source gate. Ten appended
ordinary controls exercise this component; ROOT owns actual execution.


## Appended comparison and closed diagnostic runner (A1.5/A1.7)

The child `tests/support/cache_insertion_comparison.rs` composes the existing
admitted readers, counted key coverage, observation graph, HelpProps renderer and
`order::iteration_order`/`MsvcTable`. It does not migrate Slot inspection or change
production planning, cache rows, Exact flags, parsers or hash-model policy.

`compare_current(inputs, None)` reports the original payload's copied roundtrip,
without a candidate. The separate explicitly supplied `DiagnosticProposal` is
untrusted diagnostic input, never a source traversal generator. Before the model
is called, every actual emitted key must appear exactly once, event IDs must be
unique, and every key occurrence must match its CURRENT case/stage/version,
source SHA, actual Brace path and original registry span. Batch key witnesses use
one bound projection, not a source-file hash/Git lookup per key. The model receives
only admitted key strings; a copied payload is rendered without editing the caller.
An exact copied/model comparison is not independent native generation or insertion
history authority. Distinct insertion histories may produce the same final order.

Comparators separately retain complete Byte/EOF first differences, literal hashes,
BOM/CRLF and lengths; complete key and typed property vectors; and recognized
reference/type/empty-value/nil event endpoints including original occurrences and
unknown raw values. Missing observations are Unavailable, never NoRefs. Event
comparison includes provenance changes as well as typed value changes; the full
current event vector is in the report. Covered key domains, origins, complete row
bindings, supplementary facts, original proofs and Partial observations are retained.

The named ignored `retained_closed_corpus_projection` requires all three variables:
`IBCMD_RS_CACHE_TRACE_MANIFEST`, `IBCMD_RS_CACHE_TRACE_MANIFEST_SHA256`, and
`IBCMD_RS_CACHE_TRACE_OUTPUT`. Manifest/output must be absolute F paths and output
must be NEW. Missing variables fail; there is no successful skip or fallback. Its
portable ordinary handler is also exercised by generated controls on other hosts.
The closed outer schema `cache-trace-retained-corpus-v1` includes independently
bound child manifests and capture witnesses, admitted source/audit owner head, and the
compiled `include_bytes!` digests of the diagnostic helper/test/model/renderer.
ROOT must supply the independent outer SHA and verify historical provenance before
running. The admitted input source-owner SHA is separate from the running compiled-source identity.
The field `input_exporter_source_head` retains the input API's name but means the
ROOT-admitted source/audit owner: it must match capture/input/facts declarations.
It does not recover historical producer Git from svc.json or byte hashes; if that
producer is unknown it remains unknown. ROOT binds the selected audit owner and
actual historical svc/blob proofs independently. The generated b74 source fixture
is only a declared cleanroom test owner, not a capture receipt or traversal proof.
Capture provenance is an explicitly pinned declaration, not fresh native proof.
There is no JSON proposal field: all retained cases run with proposal None.

Per ROOT's admitted locator clarification, distinct complete nonoverlapping members
of one indexed pack are allowed. An identical read-only payload/range may be reused
across independently bound case/stage captures; its stage versions need not be equal.
Each stage still needs its own capture witness, input manifest and original proof
binding. Conflicting row identities, overlapping members, duplicate roles, physical
proof aliases (including hard links), mixed stages and proof/output/input aliasing
refuse. This uses actual file identity on Windows/Unix, not global uniqueness of
all pack/blob pathnames. Supplementary rows retain their own complete plain bindings.

The handler revalidates every used input, writes/syncs a staged `report.json` through
CreateNew, revalidates the source closure again, then writes/syncs
`completion.json` bound to the report and outer hashes. `report.json` alone has
`diagnosis_finished:false`; only a known successful ROOT handler receipt together
with its completion companion establishes completion of the diagnostic. A failed
or crashed run's partial output is retained and is not adopted/retried/replaced.
Existing outputs and caller files are preserved. This is not native acceptance:
all paths retain Partial, NotIdentified, authority null, native false and issue open.

Nine appended ordinary controls and one ignored target are prepared for independent
SOURCE review; author did not run them. ROOT may run the original ordinary target
above after that gate, then (with its owned context and separately reviewed inputs):

```powershell
$env:IBCMD_RS_CACHE_TRACE_MANIFEST = 'F:/ROOT-reviewed/current-corpus.json'
$env:IBCMD_RS_CACHE_TRACE_MANIFEST_SHA256 = '<independently pinned SHA256>'
$env:IBCMD_RS_CACHE_TRACE_OUTPUT = 'F:/ROOT-reviewed/new-diagnostic-output'
cargo test --locked --offline --test cache_insertion_trace retained_closed_corpus_projection -- --ignored --exact --nocapture
```

These paths are placeholders, not a ready retained run. The selected historical
HelpProps ranges alone do not provide matched Registry/TypeSets source provenance.
ROOT must bind all three same-stage rows first; weak plain-only comparisons remain
labelled diagnostic. S4, remaining semantic graph work, any proposed H1 construction
hypothesis and actual native exactness criteria remain separate pending work. Issue
#403 remains open; no public product capability or release acceptance is claimed.