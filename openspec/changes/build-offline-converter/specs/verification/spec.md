# Verification and Compatibility Evidence

## ADDED Requirements

### Requirement: Fixtures are versioned and attributable

Every committed fixture MUST have a manifest entry with artifact kind, independent version coordinates, SHA-256, provenance, features and expected outcome/losses.

#### Scenario: Fixture metadata is incomplete

- **WHEN** CI validates the corpus
- **THEN** the build fails before codec tests run

### Requirement: Compatibility is derived from evidence

A profile/family/operation MUST NOT be marked verified without a passing fixture or matrix test referenced by the compatibility record.

#### Scenario: New profile has no fixtures

- **THEN** it remains experimental and encoding is not advertised as verified

### Requirement: Standard CI proves standalone operation

Portable tests MUST run on Windows and Linux with no platform components available.

#### Scenario: Product code tries to execute a platform process

- **WHEN** the clean-environment smoke suite runs
- **THEN** the test fails and reports the attempted executable
