## ADDED Requirements

### Requirement: Writes require an evidenced platform storage profile

The MSSQL source pipeline SHALL identify an evidenced platform/storage profile
before staging or activation and SHALL reject unknown or partially evidenced
profiles before mutation.

#### Scenario: Platform 8.5.1.1150 profile is evidenced

- **GIVEN** an MSSQL infobase running on platform 8.5.1.1150 with matching native evidence
- **WHEN** a supported source cohort is compiled, staged, or activated
- **THEN** the report identifies the 8.5 platform and storage profile
- **AND** only invariants verified for that profile are used

#### Scenario: Unknown 8.5 build is rejected

- **GIVEN** an infobase whose native storage profile has no matching evidence
- **WHEN** a write operation is requested
- **THEN** it fails before `ConfigSave` or ordinary configuration rows change

### Requirement: Platform 8.5 worker activation is isolated

The system SHALL support same-session source activation on 8.5 only through a
verified dedicated laboratory worker process and SHALL refuse shared workers.

#### Scenario: Open 8.5 session observes saved code

- **GIVEN** an open session on a disposable 8.5 clone served by a dedicated worker
- **WHEN** a watched module or managed form is saved
- **THEN** the database remains online
- **AND** the same client PID and 1C session UUID observe the new client and server code

#### Scenario: Source BSP worker is shared

- **GIVEN** the registered source BSP infobase shares workers with other infobases
- **WHEN** an activation is attempted against it
- **THEN** the operation is rejected before mutation
- **AND** no source session or process is terminated
