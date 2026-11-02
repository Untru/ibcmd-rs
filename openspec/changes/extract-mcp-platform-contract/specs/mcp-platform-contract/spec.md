## ADDED Requirements

### Requirement: Typed platform admission

MCP SHALL separate host adapter capabilities from fixed operator authorization.
Current unsupported hosts and SQL SHALL refuse both reads and staging, regardless
of model parameters or operator write opt-in. Rows fixtures SHALL remain read-only.

#### Scenario: Unsupported adapter cannot be opted into writes

- **GIVEN** an unsupported adapter and an explicit fixed file-db write opt-in
- **WHEN** a request asks to stage data
- **THEN** admission refuses before snapshot or publication work

#### Scenario: Unix opt-in remains necessary

- **GIVEN** the existing Unix file-db adapter and a fixed database
- **WHEN** operator write opt-in is absent
- **THEN** reads can be admitted and staging is refused

### Requirement: Portable control contract

MCP SHALL reject model-controlled policy/target/confirmation overrides and unsafe
relative path syntax on every host. Binding changes, cumulative budget overflow
and cancellation SHALL refuse before result or staging publication. Lexical
validation SHALL NOT replace native filesystem identity and confinement checks.

#### Scenario: Model confirmation cannot authorize writes

- **WHEN** an import supplies confirm, settings, database, server or agent-write
- **THEN** argument validation refuses without adopting any override

#### Scenario: Cancelled or over-budget snapshot

- **GIVEN** a synthetic snapshot adapter with a cumulative operation control
- **WHEN** cancellation or a budget violation occurs
- **THEN** it refuses without consuming further input or publishing anything

### Requirement: Required snapshot boundary

A snapshot adapter SHALL capture through admitted native handles, retain an
opaque binding witness, and revalidate the same binding/content with cumulative
limits and cancellation. A witness SHALL NOT be interpreted as authorization.

#### Scenario: Snapshot binding changes

- **GIVEN** a fixed session snapshot
- **WHEN** the authoritative current binding differs
- **THEN** validation refuses; a model-selected replacement cannot rebind the session
