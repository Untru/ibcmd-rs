## ADDED Requirements

### Requirement: TableCurrentData ChoiceParameterLinks is schema-owned

The system SHALL decode the exact mirrored native TableCurrentData profile
into a canonical `FormChoiceParameterLink` before XML emission. XML writers
SHALL NOT read raw form slots or infer paths from application object names.

#### Scenario: Table and column ids resolve in the same form

- **GIVEN** exact mirrored `5006/5007` collections with `mode=2`
- **AND** owner is `{positive-table-id, exact-form-item-type-uuid}`
- **AND** terminal is `{positive-column-id}`
- **WHEN** both ids resolve through the form indexes
- **THEN** the canonical data path is
  `Items.<Table>.CurrentData.<Column>`
- **AND** writer order remains `Name`, `DataPath`, `ValueChange`.

#### Scenario: TableCurrentData mirror differs

- **GIVEN** independently valid primary and duplicate collections
- **BUT** their table id, type UUID, or column id differs
- **WHEN** the collections are decoded
- **THEN** the whole value is rejected as `MirrorMismatch`
- **AND** no partial links are emitted.

#### Scenario: Platform type UUID is not exact

- **GIVEN** a two-field owner
- **BUT** its UUID is nil, non-canonical, or not the exact form-item type UUID
- **WHEN** physical decoding runs
- **THEN** the corresponding collection is malformed
- **AND** semantic resolution is not called.

#### Scenario: Table or column id is unresolved

- **GIVEN** an exact TableCurrentData envelope
- **BUT** either id is absent from the form indexes
- **WHEN** semantic resolution runs
- **THEN** source export fails closed with an opaque diagnostic
- **AND** no guessed data path is emitted.

#### Scenario: TableCurrentData metadata binding UUID resolves unambiguously

- **GIVEN** an exact TableCurrentData owner
- **AND** terminal is `{0,canonical-lowercase-non-nil-uuid}`
- **AND** the same form proves one table/child binding route for that UUID
- **WHEN** semantic resolution runs
- **THEN** the canonical path is
  `Items.<Table>.CurrentData.<ChildField>`
- **AND** `object_refs` is not used for this terminal.

#### Scenario: TableCurrentData metadata binding UUID is ambiguous

- **GIVEN** two different child field names for the same table and binding UUID
- **WHEN** the form indexes are built
- **THEN** the route is absent from production lookup
- **AND** ChoiceParameterLinks remains opaque and fails closed.

#### Scenario: Numeric binding id and UUID identify the same column

- **GIVEN** terminal is `{positive-binding-id,canonical UUID}`
- **AND** the authoritative UUID route exists
- **WHEN** a numeric route also exists
- **THEN** both routes SHALL be equal before the link is emitted
- **AND** disagreement leaves the complete link collection opaque.
