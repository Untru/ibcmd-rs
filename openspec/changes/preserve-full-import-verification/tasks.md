# Tasks

- [x] 1. Reproduce guarded-import differences and capture focused failing tests for #572 follow-up codecs and comparisons.
- [x] 2. Implement bounded comparison and typed codec repairs; pass 302 selected root tests and 382 XML tests and independent source review, including modern Group and byte/hash-identity refusal regressions.
- [x] 3. Complete repository check (five steps), physical-adapter policy and strict OpenSpec validation.
- [x] 4. Rebuild and reproduce the previously failed application inputs offline, retaining unsupported-property refusals; the two separately approved application-source corrections are applied, all 12 focused Forms pass comparison, and all four previously refused actual stage-guard pairs now pass.
- [x] 5. Complete a fresh full-tree verified test-server import and record actual elapsed time separately from database apply.

Acceptance: full-folder auto/patch import 557.253 s, all 122,550 staged files identical; native 8.3.27.1859 UpdateDBCfg 316.201 s, process/result 0, ConfigSave empty and active generation changed. Sum 873.454 s; wall cycle 903.502 s. Existing database rows were reused. Full CI remains unverified; PR stays draft.
