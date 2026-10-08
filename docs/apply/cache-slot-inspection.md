# Read-only owner-slot inspection (#403)

`restructure::caches::slots::RecordMap::inspect(kind, record)` borrows the flat
owner record returned by `owner_record`. It reads the existing private
`metadata_model::objects::layout` and `objects_parts::Slot` declarations. There
is no additional family layout or stored-payload grammar.

The returned facts are ordered by the outer Slot declaration, including
branch-suppressed declarations. Generated pairs occupy two physical owner
ordinals. Each fact exposes its role, conditional gates, expected width and
range, clipped actual range and borrowed values. Ranges are parsed owner
ordinals, **not byte offsets into the original descriptor**.

All three declared recipes whose numeric tag matches the record are retained.
A recipe is never selected by first match, widest width, exact input width or a
preferred compatibility. An exact-width observation does not hide another
compatible observation's missing pair or extra tail. Only fully present facts
which agree across every retained observation are exposed by
`shared_present_slots()`.

| Result | Meaning |
| --- | --- |
| `OwnerPositionsExact` | Every retained recipe agrees on all declared positions and fits the owner width. |
| `AmbiguousOwnerPositions` | Retained recipes disagree, even if one fits exactly. |
| `IncompleteOwnerShape` | Recipes agree, but the actual owner is shorter or has a tail. |
| `UnknownLayout` | The kind or integer tag has no declared recipe; original values remain available. |

An empty record or noninteger first value is an error. Unknown integer tags do
not receive guessed modern roles. `RecordMap::new` retains its historical
unknown-tag behavior for existing callers; only the new inspector refuses to
infer such a recipe.

`SemanticPayloadUnchecked` and `CompatibilityStatus::Unbound` always remain
separate from positional confidence. Header contents, UUID validity or domains,
reference targets, typed payloads, collection counts, topology and whole-graph
completeness are not validated here. A full-width record containing a malformed
Header can therefore have exact **positions**, while still having unchecked
payloads. This API does not establish absence of references, graph completeness,
source compatibility, cache emission order or native exactness.

The legacy map and inspector share one declaration walk. Existing named indices,
generated-category order, first Header position, tag flags, `slot`,
`generated_types`, `owner_record`, `md_base` and `object_identity` behavior remain
unchanged. `Slot` remains private. No current planner, cache renderer or diagnostic
consumer is migrated in this slice.

Prepared controls in `tests/cache_slot_inspection.rs` invoke the actual base-free
compiler on independently authored XML, then inspect its actual serialized owner
records: Catalog old/new, ExchangePlan old/new, Report19/20, Document40 and a
characteristic chart's type pattern. They check map results, gated absence,
borrowed roles, source/descriptor immutability, truncation, extra tails and unknown
layouts. Two private synthetic Slot tests specifically exercise ambiguous
same-tag widths; they do not add production families or measured native claims.

This is the S2/S3/S5 source component of the admitted diagnostic design. Tests
are prepared for coordinator execution. Issue #403's native insertion-order and
final cache-byte acceptance remain open. S4 consumer migration and any source
traversal hypothesis require their own design and review gate.
