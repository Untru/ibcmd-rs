# Version Profiles

## ADDED Requirements

### Requirement: Version axes are independent

The system MUST represent platform build, XML dialect, compatibility mode, storage profile and CF container revision as independent open values.

#### Scenario: XML dialect is shared by multiple platform builds

- **GIVEN** two platform profiles that use the same XML dialect
- **WHEN** profiles are loaded
- **THEN** the profiles remain distinct and no platform build is inferred solely from the XML version

### Requirement: Profiles are data-driven and extensible

The system MUST load schema-validated bundled profiles and MAY load external experimental profiles without recompiling the application.

#### Scenario: Add a future profile

- **GIVEN** a valid profile with a previously unknown dotted version
- **WHEN** the external profile directory is loaded
- **THEN** the version is accepted as data and its explicitly declared capabilities become discoverable

#### Scenario: Profile inheritance contains a cycle

- **WHEN** effective profiles are resolved
- **THEN** loading fails with a deterministic cycle diagnostic

### Requirement: Encoding requires an unambiguous target

Artifact detection MUST return evidence and distinguish exact, ambiguous and unknown results. Encoding MUST NOT select the nearest profile implicitly.

#### Scenario: Artifact fingerprints match multiple profiles

- **WHEN** the user does not specify a target profile
- **THEN** preflight fails and lists the competing profiles and evidence
