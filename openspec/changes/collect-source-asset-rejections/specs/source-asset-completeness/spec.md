## ADDED Requirements

### Requirement: Full-scope source-asset rejection collection

The MSSQL source exporter SHALL provide an explicit diagnostic mode that
continues after a typed form source-asset rejection only when structured
property diagnostics are available.

#### Scenario: Multiple diagnosed rejections are collected

- **GIVEN** a full Config export containing multiple form assets rejected by
  known typed property profiles
- **WHEN** collect-all diagnostics are enabled
- **THEN** every diagnosed rejection is recorded
- **AND** later source assets are still processed
- **AND** no rejected `Form.xml` is emitted.

#### Scenario: Undiagnosed failure remains fatal

- **GIVEN** a malformed container, operational error, invariant failure, or
  writer rejection without structured diagnostics
- **WHEN** collect-all diagnostics are enabled
- **THEN** the export fails immediately
- **AND** the failure is not reclassified as an opaque source property.

### Requirement: Deterministic safe diagnostic clusters

The source-asset completeness manifest SHALL aggregate affected assets into
bounded deterministic clusters without storing raw source payload.

#### Scenario: Equivalent profiles form one cluster

- **GIVEN** affected assets with the same family, code, classification,
  parse-error class, property and property profile
- **WHEN** the report is finalized
- **THEN** they produce one cluster with the exact total
- **AND** samples are sorted and bounded
- **AND** only raw length and SHA-256 are retained.

### Requirement: Strict completeness remains fail-closed

Collect-all diagnostics SHALL NOT make a partial source export successful or
release-eligible.

#### Scenario: Collect then reject

- **GIVEN** collect-all diagnostics and
  `--require-complete-source-assets`
- **WHEN** at least one source asset is not emitted
- **THEN** all diagnosed assets are present in the written candidate manifest
- **AND** the final completeness gate returns an error
- **AND** the parity run remains failed and release-ineligible.
