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

### Requirement: The gate is portable and privacy-preserving

The validator and its self-tests SHALL run on Windows and Linux PowerShell
without an infobase, platform installation, or network access.  Diagnostic
output SHALL contain only categories and hashes, never local paths or source
literals.

#### Scenario: CI evaluates the guard on both supported runners

- **WHEN** the offline CI workflow runs on Windows and Linux
- **THEN** it invokes the validator and its synthetic self-tests before builds
