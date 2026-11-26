# Tasks

- [x] 1. Verify native named-extension export and missing-name refusal on a disposable macOS 8.3.27.2214 file infobase (#520; preflight run 2026-10-23).
- [x] 2. Measure the file extension registry and prepared-state representation; record clean-room evidence (#520).
- [x] 3. Add failing-first tests for registry validation, selection and the public export route (#520; native-tree and missing-name regressions first failed with the unsupported-route error).
- [x] 4. Implement bounded registry reads and named CAS resolution; reuse the existing extension writer (#520).
- [x] 5. Prove own/adopted objects and two-extension selection against native full trees; preserve database hashes (#520; active, initial prepared and active-plus-prepared snapshots, exact native per-file hashes).
- [x] 6. Verify negative cases, targeted regressions, xtask check and ci; independent Rust review (#520).
