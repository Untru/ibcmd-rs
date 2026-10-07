# Params marker: independently recovered F4 counterexample

Historical measurement: 2026-09-30, 8.3.27.2214. Independent readback:
2026-10-02. This is evidence for open issue #418, not a completed native-twin
matrix or a new activation run.

The captured five-row stage includes one CommonModule descriptor. Its stored
DEFLATE bytes change from 173 to 174 bytes. Both complete streams decode to
exactly the same 289 bytes, including line endings and BOM. No normalization
was needed. The staged module body and versions change separately.

Native `config apply` succeeds and preserves the exact prior 119-byte
`Params.DynamicallyUpdated` payload. OWN apply succeeds but removes that row.
The native final snapshot and OWN final snapshot independently confirm the
different row sets. The current OWN decision uses the presence of any staged
descriptor, rather than a proven change requiring native cache collection.

The adjacent fixtures are exact bytes extracted from the preserved recovery
artifact; `f4-counterexample.json` pins all seven original sources and their
lengths/digests. Original artifacts on F were read without modification.
The native final snapshot contains physical headers, but the recovery's old
marker headers are empty: only exact payload preservation is proved here.

Before changing the production rule, measure native/OWN twins of body-only,
byte-identical descriptor, recompressed identical descriptor, and genuinely
changed descriptor stages, plus repeated apply. Include pending descriptor
overlays when determining the effective preimage; ordinary-row equality alone
does not establish equality to an active alias. Keep the original stage and
full Params row/header inventories for each twin.

No production rule, database, registration, client, lease, or extension was
changed by this investigation. Issue #418 remains open.
