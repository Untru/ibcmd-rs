- [x] 1. Add failing clean-room structural/privacy/bounds tests (Refs #381).
- [x] 2. Implement bounded decode, existing-parser raw views and typed report.
- [x] 3. Add read-only example with static failure handling and no partial output.
- [x] 4. Run targeted tests and unchanged governance/policy gates in an allocated slot.
- [x] 5. Obtain fresh independent privacy/correctness review before VM execution.
- [x] 6. Hand source/binary provenance to the sole operator for serialized observation.
- [ ] 7. Use observed facts for a separately verified production fix; no parity claim from diagnostics.

Completed steps 4–6 are bound to source `9e05b83d037386407f7af0bfc4e6c78291f2c886`.
See `evidence/local-validation.md` for exact limits and the one completed
operator observation. A later XML-only helper has separate unrun gates; do not
repeat the original observer to satisfy that separate diagnostic.

## Optional context extension (new source, historical gates do not carry forward)

- [x] 8. Review bounded frozen-input custody and exact attribute/MainTable joins.
- [x] 9. Test occurrence-preserving final-query projection and all refusal/privacy cases.
- [ ] 10. Build and bind the new Windows artifact; review the exact sole-operator packet.
- [ ] 11. Run public controls then necessary frozen-input observation under a separate lease.
- [ ] 12. Select and verify a production correction from actual facts; default parity remains unresolved.

Steps 8–9 apply only to the optional context extension and the local public gates
documented in `evidence/context-query-local-validation.md`. They do not complete
Windows artifact/worker controls or a real frozen-input query observation.
