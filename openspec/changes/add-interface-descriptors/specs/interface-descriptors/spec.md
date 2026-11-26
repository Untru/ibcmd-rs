## ADDED Requirements

### Requirement: Bidirectional Interface metadata

The converter SHALL resolve Interface UUIDs to canonical names and preserve Name, Synonym, Comment and Switchable in supported native descriptor records across XML, CF and SQL conversion.

#### Scenario: An interface is part of a configuration

- **GIVEN** a configuration containing interfaces and other metadata kinds
- **WHEN** its sources are exported and compiled again
- **THEN** Interface descriptors and all Configuration ChildObjects remain present in native order
- **AND** Switchable true and false retain their distinct native meanings

### Requirement: Explicit refusal of invalid inventory

The converter SHALL report unsupported or malformed Interface records and unresolved root inventories as errors rather than silently omitting ChildObjects.

#### Scenario: An interface record has unknown storage members

- **GIVEN** an Interface record with an unrecognized member or version
- **WHEN** conversion is requested
- **THEN** conversion refuses the unsupported profile explicitly
- **AND** it does not claim a complete successful export
