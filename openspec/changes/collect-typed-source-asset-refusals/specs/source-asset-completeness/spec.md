## ADDED Requirements

### Requirement: Classified non-form refusals are collected, not fatal

In the collect-all diagnostic mode, a source-asset codec refusal that carries a
stable diagnostic code and a source-side failure class SHALL be recorded as a
not-emitted asset, and the export SHALL continue with the next row.

#### Scenario: Data-composition template is outside the evidenced cohort

- **GIVEN** a full diagnostic export with `--collect-all-source-asset-diagnostics`
- **WHEN** a data-composition template body is refused as `unsupported`
- **THEN** no `Template.xml` is written for it
- **AND** the completeness report contains one entry with family `dcs`, the stable code, the classification, the asset path, the row id, the raw length and the raw SHA-256
- **AND** the next row is still exported

#### Scenario: Internal invariant stays fatal

- **GIVEN** the same diagnostic export
- **WHEN** a codec fails with an `invariant` classification or without one
- **THEN** the export fails immediately

#### Scenario: Default mode is unchanged

- **GIVEN** an export without the diagnostic flag
- **WHEN** a codec refuses a body
- **THEN** the export fails on that body
