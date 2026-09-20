# Tasks: evidence-bound platform profiles

- [x] 1. Add the `platform-8.3.27.2214` profile with inherited storage constants, declared fingerprints and evidenced write capabilities.
- [x] 2. Resolve MSSQL write policy from the bundled profile registry and fail closed on undeclared capabilities.
- [x] 3. Compare the live `IBVersion` and schema digest with the declared fingerprints instead of hard-coded constants.
- [x] 4. Run the declared-policy check before rac, sqlcmd and source access in every main write entry point.
- [x] 5. Require the publishable extension registry transition before the first staged row.
- [x] 6. Update focused unit tests for capability resolution, fingerprints, ordering and the registry precheck.
- [ ] 7. Re-run the 8.3.27.2214 BSP and ERP UH load parity with the new profile and record the evidence.
- [ ] 8. Update README and `profiles/README.md` for the new profile and the data-driven policy.
