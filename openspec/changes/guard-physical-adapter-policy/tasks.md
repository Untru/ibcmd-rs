# Physical-adapter policy guard — implementation plan

**Goal:** make new physical-adapter literals reviewable through an offline,
committed inventory gate.

## Tasks

- [x] Define the bounded non-MXL scope and baseline semantics.
- [x] Implement a deterministic validator and committed normalized baseline.
- [x] Add synthetic mutation self-tests for every acceptance and rejection
  branch.
- [x] Invoke the validator and self-tests on Windows and Linux offline CI.
- [x] Document scope, exclusions, update procedure, and limitations.
- [x] Run the validator, self-tests, and strict OpenSpec validation.
