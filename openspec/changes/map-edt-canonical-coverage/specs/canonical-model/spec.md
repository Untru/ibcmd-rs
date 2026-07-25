## ADDED Requirements

### Requirement: Every EDT feature has a preservation status

Every committed EDT feature SHALL map to exactly one canonical preservation
status: typed, opaque-lossless, unsupported or platform-only.

#### Scenario: New EDT feature is imported

- **WHEN** Xcore corpus gains a feature without coverage entry
- **THEN** strict coverage validation fails
- **AND** writer migration cannot declare the family complete

### Requirement: Opaque coverage is lossless and attributable

An opaque-lossless mapping SHALL define canonical placement and source
provenance sufficient for same-profile emission.

#### Scenario: Unknown same-profile feature is preserved

- **WHEN** decoder cannot type a mapped opaque-lossless feature
- **THEN** raw semantic payload is retained with placement and provenance
