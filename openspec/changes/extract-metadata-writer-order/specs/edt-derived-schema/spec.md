## ADDED Requirements

### Requirement: Metadata order is provider-derived

Metadata writer SHALL obtain section and feature order from verified autonomous
provider-derived rules rather than local XML property arrays.

#### Scenario: Catalog is serialized

- **WHEN** Catalog has a verified properties order for target version
- **THEN** writer follows that ordered token list
- **AND** raw decoder slots do not influence XML order

### Requirement: Order evidence does not imply default evidence

Order-provider rules SHALL NOT mark QName, default, nil, empty or compatibility
behaviour verified without separate writer evidence.

#### Scenario: Provider lists an optional feature

- **WHEN** feature appears in provider order
- **THEN** only its relative order is verified
- **AND** emission/default behaviour remains pending
