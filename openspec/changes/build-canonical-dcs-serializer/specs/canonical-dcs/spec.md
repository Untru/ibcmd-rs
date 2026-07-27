## ADDED Requirements

### Requirement: DCS semantics use one canonical model

Standalone DCS documents and Form ListSettings SHALL use the same bounded
canonical representation and serializer.

#### Scenario: Equivalent settings have different physical sources

- **WHEN** standalone settings and Form ListSettings decode to equal canonical IR
- **THEN** they serialize to equal semantic XML
- **AND** neither path applies a source-specific string normalizer

### Requirement: Unknown DCS extensions are lossless

The canonical DCS layer SHALL retain bounded unknown extensions with explicit
placement and source provenance.

#### Scenario: A supported profile contains an unknown extension

- **WHEN** the extension fits configured resource limits
- **THEN** it is retained with exact placement and provenance
- **AND** same-profile serialization preserves it

### Requirement: DCS writer decisions require evidence

The DCS writer SHALL require an exact verified rule for every QName, TypeId,
qualification and ordering decision that affects emitted XML.

#### Scenario: QName or TypeId rule is pending

- **WHEN** serialization requires that rule
- **THEN** the writer returns a stable typed diagnostic
- **AND** it does not infer the value from object names or input spelling
