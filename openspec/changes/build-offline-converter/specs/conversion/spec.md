# Conversion and Migration

## ADDED Requirements

### Requirement: Conversion follows one adapter pipeline

Supported operations MUST execute `decode -> validate -> migration plan -> migrate -> encode preflight -> atomic encode`.

#### Scenario: Dry-run conversion

- **WHEN** `--dry-run` is supplied
- **THEN** the complete plan and diagnostics are produced but no output is written

### Requirement: Migration paths are explicit and deterministic

The graph MUST select a deterministic supported path and MUST NOT traverse an unverified downgrade edge.

#### Scenario: No safe path exists

- **THEN** conversion stops with the missing/blocked edges and leaves the source unchanged

### Requirement: Losses are machine-readable

Every warning or loss MUST contain a stable code, object/property path, source/target profile, reason and applied policy.

#### Scenario: Unsupported property during downgrade

- **WHEN** the default policy is used
- **THEN** conversion fails
- **WHEN** explicit `drop` is used and the codec declares the drop safe
- **THEN** the property is removed and recorded in the report

