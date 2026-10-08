# Cache insertion trace: admitted inputs and key coverage

This is the first diagnostic component for #403, in integration-test support.
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
- [ ] A1.3 complete source owner/reference/type facts and descriptors.
- [ ] A1.4 source-authorized event/first-visit contract.
- [ ] A1.5 candidate rendering and complete per-event difference report.
- [ ] A1.6 remaining graph/event/model controls.
- [ ] A1.7 retained corpus manifest/run/report.

Every result reports `GraphCompleteness::Partial`, `TraceStatus::NotIdentified`,
five pending atoms, and `first_visit_authority: None`. Unknown semantic values
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
