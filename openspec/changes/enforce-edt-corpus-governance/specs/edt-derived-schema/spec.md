## ADDED Requirements

### Requirement: Corpus governance is enforced by CI

CI SHALL reject proprietary binary/source payloads, machine-specific paths and
verified facts without provenance in EDT-derived corpus changes.

#### Scenario: JAR is added under schema data

- **WHEN** repository tree contains a JAR in EDT-derived paths
- **THEN** offline governance gate fails before release build

#### Scenario: Verified fact has no source

- **WHEN** committed rule has verified status and empty provenance
- **THEN** schema validation fails
