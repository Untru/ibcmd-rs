## ADDED Requirements

### Requirement: Physical adapter policy additions are inventory-gated

The repository SHALL validate the configured non-MXL physical adapter slice
offline against a committed normalized baseline.  The validator SHALL reject a
new scoped source file or a new UUID/name/XML-policy occurrence fingerprint,
while allowing removal of existing inventory entries.

#### Scenario: A new hardcoded adapter UUID is introduced

- **WHEN** a production physical-adapter source gains a UUID literal not present
  in the baseline
- **THEN** the offline validator fails without printing the literal

#### Scenario: Existing technical debt is removed

- **WHEN** a baseline fingerprint is no longer found in the source
- **THEN** the offline validator succeeds

### Requirement: Test-only modules are outside the guarded slice

The validator SHALL leave out of the guarded slice every source file the
compiler reads only for tests: the module a scoped file declares as
`#[cfg(test)] mod name;` and every file below that module's directory. It SHALL
NOT need a list of test file names, and SHALL keep guarded a module whose cfg
predicate can be true in a production build and a module named by a `#[path]`
attribute.

#### Scenario: A new out-of-line test module is added

- **WHEN** a scoped file gains `#[cfg(test)] mod new_tests;` and the file
  `new_tests.rs` holds UUID literals
- **THEN** the offline validator passes without a baseline change

#### Scenario: A module that is not test-only is added

- **WHEN** a scoped file declares `mod plain_tests;` or
  `#[cfg(any(test, feature = "x"))] mod partly_tests;` and the file holds a
  UUID literal not present in the baseline
- **THEN** the offline validator fails without printing the literal

### Requirement: The gate is portable and privacy-preserving

The validator and its self-tests SHALL run on Windows and Linux PowerShell
without an infobase, platform installation, or network access.  Diagnostic
output SHALL contain only categories and hashes, never local paths or source
literals.

#### Scenario: CI evaluates the guard on both supported runners

- **WHEN** the offline CI workflow runs on Windows and Linux
- **THEN** it invokes the validator and its synthetic self-tests before builds
