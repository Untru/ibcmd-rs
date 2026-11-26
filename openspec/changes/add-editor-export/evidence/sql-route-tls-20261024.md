# SQL candidate repair evidence

The prior Windows candidate at `d16570d1` did not establish SQL acceptance:
export info reached an unknown typed row-role route, and a negative certificate
trust probe still connected because the export constructor forced trust.

A sanitized native receipt binds UT 11.5.27.88, native 8.3.27.2342,
compatibility 8.3.27 / XML 2.20: CommonCommand's `.1` is Help and `.2` is
CommandModule. Receipt SHA-256:
`12f3f01d1355ea35f25025fe87697a8e92015cf5f33a4962098fbed4815f6719`.
The typed Help registry now agrees with the existing corpus-measured Help reader.
No role inference or fallback was added. The empty-help regression reproduces the
old missing-route error; `.5`, `.99`, and unknown owners remain refused.
The synthetic compiler fixture now uses the native-proven `.1`; its target and
payload assertions remain intact. No private XML or metadata names are included.

Export and compatibility detection now share a certificate-validating SQL
constructor. Implicit trust was removed from related save, editor/object and
native extension entrypoints. Required TLS encryption and explicit research trust
switches remain intact. The legacy `SqlOptions::integrated` helper used by other
research commands is unchanged; this is not a global-default claim.

Local Rust 1.95, locked/offline/no default features, external jobs=2:
13 targeted unit checks passed, including real constructor-policy checks for
both SQL backends/authentication modes and all six editor read-scope controls.
42 focused export integration tests passed; one optional private corpus ignored.
61 JS tests passed, zero skips, including actual CLI/RPC against the freshly built
debug candidate. Rust formatting and unchanged physical-adapter policy passed.
Red and final green logs are retained externally. A shell bookkeeping error in
red/green handoff required explicit candidate restoration; the intermediate
baseline run is retained and excluded from passing-candidate claims.

Independent routing review verified the native receipt hash and exact suffix
mapping; primary review verified scoped TLS changes and preserved explicit flags.
CI now runs certificate-policy, empty-help and Help-compilation unit regressions
as separate steps, preserving Windows failure propagation.

Windows executable build/hash, negative-PEM rejection, valid-PEM success,
UT inventory parity and source integrity require the sole Lenovo operator's
serialized retest on the new exact candidate. No new VM action, private transfer,
ERP timing, merge, release or Ready transition occurred in these local gates.
Prior benchmarks remain attributed to their recorded heads. The PR stays Draft.
