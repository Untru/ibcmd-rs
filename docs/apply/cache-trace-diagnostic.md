# Typed current-cache observations (#403 A1)

`restructure::caches::trace_diagnostic::inspect` takes borrowed canonical `SiMain`, `TypeSets`,
`HelpProps`, optional `TypeIndex`, and explicitly supplied descriptor rows. It is read-only:
no loader, planner, apply consumer, cache renderer, native actor, or mutation of approximate flags.
Public typed collections are checked again rather than trusting `SiMain`'s private historical index.

The report classifies each existing help key by its actual metadata-record ordinal, set ordinal,
or nil aggregate. It rejects duplicate/ambiguous keys, invalid UUID declarations, inconsistent owner
preorder, duplicate properties and mismatched nil-to-emitted-set coverage. Metadata records and sets
which emit no help key remain distinct from emitted keys. Actual properties and entry order stay in
the original models. No corpus count is an admission limit.

Owner edges use registry ordinals and original SiMain byte spans. Generated TypeId membership uses
the actual type-index section/entry/type ordinal and descriptor pair coordinates; the semantic
category index remains in the original `TypeSlot.index` (it is not inferred from physical order).
The observer does not
derive categories from a UUID or a property name. Descriptor reference observations use the existing
`RecordMap::inspect` semantic roles and only positions shared by every matching owner recipe.
Bare metadata UUID slots and tagged list members keep their actual descriptor row ordinal, schema
slot and Brace path. A nil reference slot is an empty value; it does not become the nil aggregate's
insertion action. Repeated references keep separate occurrences, including shared referents.

Unknown generated declarations, descriptor facts, field-reference/type-pattern semantics, owner
payload and compatibility produce addressed residues. A known physical position does not prove a
complete graph. In particular the present owner inspector always reports unchecked semantic payload
and unbound compatibility; set property23 has no complete semantic admission here. Consequently
`insertion_candidate` remains `None`, with precise blocking coordinates, rather than inventing
outgoing references or a native first-visit order. Proving source dispatch/compatibility and complete
reference semantics requires a separate canonical admission atom; no speculative traversal is added.

The laboratory-only `tests/support/cache_trace_typed_current.rs` composition reuses the retained
`ProjectionInputs`/`FactInputs` full-input and source-custody gates, keeps unsupported fact rows and
their original bindings, matches the full key-source witness roster, and calls the existing literal
comparison with `proposal=None` by default. `inspect_with_proposal` also accepts an explicitly selected,
independent untrusted experiment through the existing complete key/event/source-witness protocol.
That comparator reuses `order::iteration_order`, preserves full properties, and reports concrete
literal/property/key first differences; it grants no first-visit/native authority. The cleanroom
experiment selects owner declaration order, then set declaration order and nil, not the expected
cache ordinal. The composition does not create a second raw-row grammar, discovery path or
automatic successful raw cache. Raw BOM/CRLF/EOF comparisons remain independent of typed observations.

The cleanroom ordinary target is `cache_trace_typed_diagnostic`. SOURCE review does not execute it.
ROOT owns compilation/runtime after independent admission. #403 criteria 12.5/12.6 and all twelve
physical checks remain open: native insertion measurement, a proved generator and full acceptance
are separate future atoms. Existing resource limits in canonical parsers are not removed by A1.
