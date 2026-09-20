## ADDED Requirements

### Requirement: The export publishes the active dynamic generation

The export SHALL resolve the active dynamic generation from the storage
table's `DynamicallyUpdated` record and SHALL publish the configuration that
generation defines: a row named `<base>_dynupdate_<active generation>` is the
content of `<base>`, and the plain row of that name is not published.

#### Scenario: A database holds an active dynamic generation

- **GIVEN** a `Config` table whose `DynamicallyUpdated` record names generation `G`
- **AND** a row `<uuid>_dynupdate_G.0` beside the row `<uuid>.0`
- **WHEN** the export publishes the object that uuid names
- **THEN** it writes the body of `<uuid>_dynupdate_G.0`
- **AND** it writes no artefact under the alias name

#### Scenario: A database holds only superseded generations

- **GIVEN** a `Config` table with no `DynamicallyUpdated` record
- **AND** rows carrying a `_dynupdate_<generation>` infix
- **WHEN** the export publishes the configuration
- **THEN** it writes the plain rows
- **AND** the aliased rows claim no output path

#### Scenario: The versions record follows the active generation

- **GIVEN** an active generation `G` and a row `versions_dynupdate_G`
- **WHEN** the export builds `ConfigDumpInfo.xml`
- **THEN** it reads that row as the `versions` record
