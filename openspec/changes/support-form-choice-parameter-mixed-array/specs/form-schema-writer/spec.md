## ADDED Requirements

### Requirement: FixedArray preserves heterogeneous typed values

The system SHALL preserve the type and source order of every supported
ChoiceParameters FixedArray element.

#### Scenario: Reference followed by string

- **GIVEN** an exact array containing a `U` reference and an `S` string
- **WHEN** it is decoded and emitted
- **THEN** the first value uses `xr:DesignTimeRef`
- **AND** the second uses `xs:string`
- **AND** their order is unchanged.

#### Scenario: String side identifiers are not nil

- **GIVEN** an `S` element with a non-nil or non-canonical side identifier
- **WHEN** decoding runs
- **THEN** the whole ChoiceParameters value remains opaque
- **AND** no partial array is emitted.
