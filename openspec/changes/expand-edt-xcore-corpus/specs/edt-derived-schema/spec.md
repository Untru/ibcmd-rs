## ADDED Requirements

### Requirement: Selected Xcore resources are fully accounted

Importer SHALL classify every selected Xcore resource as processed or rejected
and SHALL NOT silently skip an unsupported declaration.

#### Scenario: New multiplicity is encountered

- **WHEN** selected resource contains unsupported multiplicity
- **THEN** resource appears in deterministic reject report
- **AND** no guessed feature is committed

### Requirement: Full corpus remains reproducible

The all-model corpus SHALL be byte-identical for the same release, inventory and
importer version.

#### Scenario: Full import is repeated

- **WHEN** all model resources are imported twice
- **THEN** corpus and report SHA-256 values match
