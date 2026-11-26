## ADDED Requirements

### Requirement: Live main activation switches an existing 1C session

The product SHALL provide an explicit MSSQL `live` mode that publishes a
bounded non-structural main-configuration change and makes an already-connected
8.3.27 session observe the new client and server code without changing its
process identity or 1C session identity.

#### Scenario: Existing session observes the new module generation

- **GIVEN** an evidenced 8.3.27 MSSQL infobase and a connected session on v1
- **WHEN** a supported existing module or managed-form body is applied in `live` mode
- **THEN** the same client PID and 1C session UUID observe v2 client and server markers
- **AND** the report identifies the old and new generations and live-transition timings

#### Scenario: Saving a watched source file activates it through a dedicated worker

- **GIVEN** a worker editor watch for one supported existing source closure on a dedicated development rphost
- **WHEN** the editor completes an atomic or in-place save
- **THEN** one serialized worker activation starts after the debounce interval
- **AND** the database never enters `RESTORING`
- **AND** the same open session observes the saved client and server code

#### Scenario: Shared working process is rejected before mutation

- **GIVEN** the target infobase shares its working process with another infobase
- **WHEN** worker activation or its watcher attempts to apply a save
- **THEN** it fails before staging or SQL promotion
- **AND** it identifies the foreign infobase in the diagnostic

### Requirement: Live activation preserves committed database work

The live transition SHALL prevent new database writes before capturing all
committed log records and SHALL retain the resulting tail-log backup as an
operator-visible recovery artifact.

#### Scenario: Tail-log destination already exists

- **GIVEN** the requested tail-log destination already exists
- **WHEN** live activation is requested
- **THEN** activation fails before staging promotion or database state changes

#### Scenario: Recovery fails after NORECOVERY

- **GIVEN** the tail-log backup completed and the database is restoring
- **WHEN** automatic recovery fails
- **THEN** the command fails with the exact `RESTORE DATABASE ... WITH RECOVERY` action
- **AND** it does not claim that the database is online

### Requirement: Existing activation modes remain compatible

`online` SHALL retain already-loaded generations, while `exclusive` SHALL
publish ordinary rows only without active sessions and normalize both dynamic
markers with bounded recovery data.

#### Scenario: Live is requested for an extension

- **GIVEN** an extension source target
- **WHEN** `live` mode is selected
- **THEN** the command rejects the request before mutation until extension live switching is independently evidenced
