# Witnessed current EDT conditional-appearance framing

The authentic EDT `2026.2.1.3rc0` public DcsFilterEvidence capture contains an
LF-only form-level `ConditionalAppearance.dcssca`: 1,317 bytes, SHA-256
`c6e6e9b8b6055d5103a49a944f2d2cf2a6913b5678bbb648170beac6a71c7373`.
The post-DCSS reverse candidate reached this next boundary and refused LF before
publication. The original failure report and source remain retained externally.

An explicit framed API captures LF from the XML declaration separator, preserves
verbatim typed text and emits that framing from current typed values. The old
API and new bodies keep CRLF. Pipeline attachment carries a serde-excluded
source facet independently of Form.form framing; clearing all items still emits
no sidecar. Complete ordered typed items and namespace flavor remain in host
sidecar accounting. Unknown cells, BOM and unsupported declarations remain errors.

Focused regressions cover both namespace flavors, legacy exact bytes, explicit
CRLF text edits, well-formed unknown attributes, partial namespace refusal,
semantic serialization, independent pipeline framing and empty-sidecar omission.
All eight focused DCSSCA/DCSS/form tests pass with Rust 1.95 and jobs=2.
The authentic 1,317-byte LF sidecar and generated 1,340-byte CRLF counterpart
round-trip byte for byte with unchanged inputs and the same complete typed
semantic SHA-256
`a060ff36cc0cbbbbacce830d02e08a418f59364670b9bc670eb5aab7f5510579`. The full reverse/native rerun belongs to #356. This boundary
fix does not establish BSP/UH, full current schema or installed-EDT acceptance,
and does not rename legacy profiles.
