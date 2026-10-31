## ADDED Requirements

### Requirement: Bounded synchronous diagnostic observations

The engine SHALL borrow immutable packed candidates synchronously and return owned, bounded, occurrence-preserving diagnostics with computed full ordered-view and per-payload hashes.

#### Scenario: Separate old and new views
- **GIVEN** immutable old and new candidate slices
- **WHEN** the observer processes each view separately
- **THEN** each owned result binds its exact labels, ordinals, keys and bytes by a computed digest without retaining borrowed pointers or accessing storage

#### Scenario: Limit or cancellation
- **GIVEN** oversized packed/inflated input or a cancellation callback error
- **WHEN** observation runs
- **THEN** it returns an error without a partial successful result or external writes

### Requirement: Decoder observations do not grant authority

The observer SHALL preserve warnings, misses, duplicates and ambiguity and SHALL keep full owner/completeness, versions and projection authority unsupported.

#### Scenario: Four-family graph candidates
- **GIVEN** a descriptor accepted by an existing four-family graph decoder
- **WHEN** collections are observed
- **THEN** ordered collection/item occurrences and schema provenance are retained while full parent and publication authority remain unsupported

#### Scenario: Unknown profile or missing owner
- **GIVEN** an unknown profile, opaque payload, header/key mismatch or an unsupported family
- **WHEN** candidate observations are requested
- **THEN** the result explicitly records unsupported or ambiguous evidence and cannot establish completeness

#### Scenario: Duplicate and alias collisions
- **GIVEN** duplicate ordinals, names, keys or slash/case aliases
- **WHEN** observation runs
- **THEN** all bounded occurrences are retained and ambiguity is recorded without overwrite or UUID wildcard inference

#### Scenario: Recognized family without key-anchored header
- **GIVEN** a recognized four-family descriptor without a header bound to the exact supplied key
- **WHEN** the metadata audit cannot bind that header
- **THEN** its warning is retained, the occurrence is explicitly unbound/ambiguous, the graph is rejected and no other UUID or multiple owners are inferred
