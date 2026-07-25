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

### Requirement: Coverage aggregates and migration backlog are recomputable

The committed coverage corpus SHALL contain deterministic aggregates for
metadata, forms, DCS, MXL, common and other, plus an ordered migration backlog
for `unsupported/schema.unmapped` features. Validation SHALL reject any
aggregate, backlog count, grouping or ordering that differs from independent
recomputation over the exact feature-to-coverage join.

#### Scenario: Derived coverage data drifts

- **WHEN** a committed family total or backlog group no longer matches the join
- **THEN** strict coverage validation fails

#### Scenario: A package or classifier route is unknown

- **WHEN** the feature corpus introduces a package/classifier-kind pair without
  an explicit canonical family route
- **THEN** generation and validation fail closed
- **AND** the feature is not silently assigned to `other`

#### Scenario: Route identity differs only by case

- **WHEN** a package/classifier route differs from a declared route only by
  character case
- **THEN** generation fails closed under ordinal case-sensitive comparison

### Requirement: Public coverage parsing is bounded and closed

The public coverage parser SHALL reject oversized documents, strings, entry
arrays, family aggregates, migration backlog arrays and evidence-source arrays
before materializing the complete coverage graph. It SHALL reject unknown and
duplicate fields at every coverage-specific object level.

#### Scenario: Input is at a declared limit

- **WHEN** a valid coverage document is exactly at a declared byte, string or
  vector limit
- **THEN** parsing succeeds
- **AND** increasing that dimension by one is rejected

#### Scenario: Input forges implementation-specific fields

- **WHEN** a coverage entry adds an undeclared UUID, object name or other field
- **THEN** parsing fails before coverage validation

#### Scenario: Input duplicates a field or coverage key

- **WHEN** JSON repeats an object field or the coverage map repeats an exact
  namespace/classifier/feature key
- **THEN** parsing fails closed
