## ADDED Requirements

### Requirement: ChoiceParameterLinks UUID-terminal is schema-owned

The system SHALL decode the exact mirrored native UUID-terminal profile into a
canonical `FormChoiceParameterLink` before XML emission. XML writers SHALL NOT
read raw slots, object names, or physical UUIDs.

#### Scenario: UUID-terminal resolves within the bound metadata owner

- **GIVEN** exact mirrored `5006/5007` collections with `mode=2`,
  a numeric form attribute owner and terminal `{0,non-nil-uuid}`
- **AND** the UUID resolves to a field owned by that form attribute's exact
  metadata owner
- **WHEN** the input field is converted
- **THEN** the canonical link contains the owner-scoped data path
- **AND** writer order is `Name`, `DataPath`, `ValueChange`.

#### Scenario: UUID-terminal mirror differs

- **GIVEN** independently valid `5006` and `5007` collections
- **BUT** their UUID-terminal values differ
- **WHEN** the collections are decoded
- **THEN** the whole value is rejected as `MirrorMismatch`
- **AND** no partial links are emitted.

#### Scenario: UUID-terminal is unresolved or belongs to another owner

- **GIVEN** an exact UUID-terminal envelope
- **BUT** the UUID is absent, ambiguous, or outside the bound metadata owner
- **WHEN** semantic resolution runs
- **THEN** the value remains typed opaque and source export fails closed
- **AND** diagnostics contain no raw payload.

#### Scenario: Existing standard-marker terminal remains exact

- **GIVEN** the existing `mode=2` terminal `{-5}` or `{-8}`
- **WHEN** the value is decoded
- **THEN** its prior canonical data path and fail-closed behavior are
  unchanged.
