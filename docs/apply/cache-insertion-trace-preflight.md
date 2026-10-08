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
