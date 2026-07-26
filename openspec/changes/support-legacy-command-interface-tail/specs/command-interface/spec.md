# Command interface

## Requirement: Selected command ownership is structural

The selected exporter SHALL resolve command owners only from parsed command
headers. Every unresolved command SHALL have exactly one owner row.

#### Scenario: UUID occurs outside a command header

- **WHEN** a metadata row contains the UUID only in an unrelated payload
- **THEN** that row SHALL NOT be selected as the command owner

## Requirement: Legacy visibility-only tail is exact

The decoder SHALL accept the legacy tail only when the complete remaining
sequence is exactly three empty sections.

#### Scenario: Tail is missing, extended or non-empty

- **WHEN** arity differs or any legacy section is non-zero
- **THEN** the legacy branch SHALL NOT accept the record
