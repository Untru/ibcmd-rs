## ADDED Requirements

### Requirement: Form XML policy is schema-owned

The Form writer SHALL obtain ChoiceList XML order, default emission and version
behaviour from an exact verified schema rule rather than a physical slot.

#### Scenario: ChoiceList is serialized

- **WHEN** a canonical ChoiceList has an exact rule for the target profile
- **THEN** the writer follows that rule
- **AND** the raw slot index does not affect XML order

### Requirement: ListSettings uses the DCS boundary

The Form writer SHALL delegate typed ListSettings content to the canonical DCS
serializer.

#### Scenario: ListSettings is present

- **WHEN** a form contains supported ListSettings
- **THEN** the DCS layer serializes its content
- **AND** the Form layer does not normalize DCS XML as text

### Requirement: Unsupported form semantics fail closed

The Form writer SHALL reject a required rule that is not verified for the exact
target profile.

#### Scenario: Required rule remains pending

- **WHEN** the exact ChoiceList or ListSettings rule is pending or unsupported
- **THEN** serialization returns a stable typed diagnostic
- **AND** no guessed XML is emitted
