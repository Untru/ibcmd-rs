## ADDED Requirements

### Requirement: DCS semantics use one canonical model

Standalone DCS documents and Form ListSettings SHALL use the same bounded
canonical representation and serializer.

#### Scenario: Equivalent settings have different physical sources

- **WHEN** standalone settings and Form ListSettings decode to equal canonical IR
- **THEN** they serialize to equal semantic XML
- **AND** neither path applies a source-specific string normalizer

### Requirement: Unknown DCS extensions are lossless

The canonical DCS layer SHALL retain bounded extensions with explicit placement
and source provenance when the selected profile has a positive retention rule.
An XML child that is truly unknown to the profile SHALL fail closed rather than
being relabeled as opaque automatically.

#### Scenario: A supported profile contains an unknown extension

- **WHEN** the profile classifies the extension as source-owned or
  opaque-lossless and the extension fits configured resource limits
- **THEN** it is retained with exact placement and provenance
- **AND** same-profile serialization preserves it

#### Scenario: The selected profile rejects an unknown QName

- **WHEN** the profile has no positive retention rule for the QName
- **THEN** decoding returns a stable unsupported-source diagnostic
- **AND** no partial canonical mutation or inferred placement is emitted

### Requirement: DCS writer decisions require evidence

The DCS writer SHALL require an exact verified rule for every QName, TypeId,
qualification and ordering decision that affects emitted XML.

#### Scenario: QName or TypeId rule is pending

- **WHEN** serialization requires that rule
- **THEN** the writer returns a stable typed diagnostic
- **AND** it does not infer the value from object names or input spelling
