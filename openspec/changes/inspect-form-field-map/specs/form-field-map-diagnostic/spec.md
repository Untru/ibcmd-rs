## ADDED Requirements

### Requirement: Bounded existing-parser inspection

The diagnostic SHALL inspect one local Form body using existing parser/resolver
logic with compressed, inflated and report bounds, without network or writes.

#### Scenario: Valid structural observation

- **GIVEN** one supported bounded Form body and dynamic-list settings
- **WHEN** the diagnostic completes
- **THEN** it reports requested duplicate, required, ordering and alias facts
- **AND** it reports unavailable context as unknown rather than inferring a cause

#### Scenario: Unsupported or oversized input

- **GIVEN** malformed, unsupported, oversized or ambiguous input
- **WHEN** inspection cannot establish the requested facts safely
- **THEN** it refuses or marks those facts unknown
- **AND** it does not publish a partial successful report

### Requirement: Non-disclosing output

The diagnostic SHALL emit only a fixed typed schema of booleans, counts and
anonymous ordinals and fixed public classification enums; failure diagnostics SHALL be static non-sensitive codes.

#### Scenario: Source contains private sentinel values

- **GIVEN** private names, IDs, query strings, module text or paths in the input
- **WHEN** inspection succeeds or fails
- **THEN** neither stdout nor stderr discloses those values or raw error payloads

### Requirement: Export behavior remains unchanged

Ordinary exports SHALL retain their existing parser and resolver behavior while
sharing the same raw attribute views with the diagnostic.

#### Scenario: Normal export

- **GIVEN** an ordinary export outside diagnostic execution
- **WHEN** it parses the same attributes as before
- **THEN** the shared views introduce no policy change, state leak or output change

### Requirement: Exact frozen-input context binding

Optional context inspection SHALL bind the body and both XML byte streams by
digest and join selected raw attributes by exact unique name, without serializing
private names or inferring positional correspondence.

#### Scenario: Raw groups appear in a different order

- **GIVEN** frozen context records and matching attributes in another raw order
- **WHEN** the context diagnostic succeeds
- **THEN** it reports the exact XML-to-raw ordinal correspondence
- **AND** it derives finite final-query identity facts using the existing parser

#### Scenario: Custody or correspondence is invalid

- **GIVEN** a changed digest, missing or duplicate name, or unequal MainTable pair
- **WHEN** context inspection is attempted
- **THEN** it refuses with a fixed code before publishing any successful report

#### Scenario: Query source identity is unavailable

- **GIVEN** an absent, ambiguous or unresolvable main-table query source
- **WHEN** the finite query projection is produced
- **THEN** unavailable identity observations are null
- **AND** represented path absence is never asserted to prove semantic nonselection
