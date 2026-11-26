- [x] Shared file database source with backward compatible settings and RPC schema
- [x] Fail-before-write regression tests
- [x] Locked effective-stage partial import and verification
- [x] Reference preflight, dependency confirmation, stale/cancel checks
- [x] Editor actions and focused client tests
- [x] Native module + document form apply/export acceptance
- [x] Independent review, identity/secret checks, commits and approved tracking

Evidence: native module/form staging → platform apply/export and all eight
inventory tuples are recorded in `evidence/native-8.5.1.md`. Focused editor/RPC,
source-custody and file-db fault regressions pass; independent reviews cover
publication/recovery, descriptor confinement, cancellation and UI confidentiality.
PR #481 is Ready under the explicit user instruction; issue #382 remains open.
Ready is not merge/release approval. Exact-head CI and external maintainer
workflow approvals are tracked separately from these delivered implementation
tasks; a new head does not inherit an older head's CI result.
