- [x] 1. Add failing clean-room structural/privacy/bounds tests (Refs #381).
- [x] 2. Implement bounded decode, existing-parser raw views and typed report.
- [x] 3. Add read-only example with static failure handling and no partial output.
- [x] 4. Run targeted tests and unchanged governance/policy gates in an allocated slot.
- [x] 5. Obtain fresh independent privacy/correctness review before VM execution.
- [x] 6. Hand source/binary provenance to the sole operator for serialized observation.
- [x] 7. Use observed facts for a separately verified Form production fix; no parity claim from diagnostics.

Completed steps 4–6 are bound to source `9e05b83d037386407f7af0bfc4e6c78291f2c886`.
See `evidence/local-validation.md` for exact limits and the one completed
operator observation. Later XML/context helpers completed separate reviewed gates and frozen-input
observations. The original observer was not repeated for those diagnostics.

## Optional context extension (new source, historical gates do not carry forward)

- [x] 8. Review bounded frozen-input custody and exact attribute/MainTable joins.
- [x] 9. Test occurrence-preserving final-query projection and all refusal/privacy cases.
- [x] 10. Build and bind the new Windows artifact; review the exact sole-operator packet.
- [x] 11. Run public controls then necessary frozen-input observation under a separate lease.
- [x] 12. Select and verify the Form correction from actual facts; five Template differences remain open.

Steps 8–11 have separately retained context/Windows observation evidence.
Step 12 is bound to product `c45d1bb2a9ea260dd5d7bb32f3143eea7ab29076`: the
original retained UT Form is byte-exact, with zero ordered UseAlways/DataPath
differences. See `evidence/context-query-local-validation.md`. This closes only
the Form loop; it does not close #381, the five Templates, or profile-wide gates.
