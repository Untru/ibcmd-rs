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

### Requirement: The storage expression does not grow with the number of aliases

Every statement the export sends to the storage table SHALL have a text that
depends on the generation history and on the names the export leaves out, and
not on the number of rows or aliases the table holds.

#### Scenario: A database updated online many times

- **GIVEN** a `Config` table with 17 generations and 127 885 aliases
- **WHEN** the export reads any set of rows
- **THEN** the table expression it builds is a few thousand characters long
- **AND** the server does not refuse the statement (SQL error 8621)

### Requirement: A published name the active versions record does not list is not published

The published configuration SHALL be the one the active `versions` record
lists. A row an earlier generation, or the plain table, still holds for a name
that record does not list SHALL NOT be published.

#### Scenario: An online update removed a module

- **GIVEN** an active generation whose `versions` record does not list `<uuid>.0`
- **AND** an older generation that carries `<uuid>_dynupdate_<older>.0`
- **WHEN** the export publishes the configuration
- **THEN** it writes no artefact for `<uuid>.0`
- **AND** the versions/manifest inventory check passes

### Requirement: The drop-in export publishes the main configuration

`infobase config export` SHALL publish the main configuration: when `ConfigSave`
holds a complete stage (a `versions` row and no `commit` or `*.new` row), an
object the stage holds a row for SHALL be written from that row, every other
object from `Config`, and `ConfigDumpInfo.xml` SHALL follow the staged
`versions` record. Without a complete stage the export SHALL publish `Config`.

#### Scenario: An import staged a configuration

- **GIVEN** a `ConfigSave` table with a `versions` row and rows for some objects
- **WHEN** `infobase config export` runs
- **THEN** each of those objects is written from its `ConfigSave` row
- **AND** every other object is written from `Config`
- **AND** the entries of `ConfigDumpInfo.xml` carry the staged version stamps

#### Scenario: The stage is not complete

- **GIVEN** a `ConfigSave` table without a `versions` row, or with a `commit` or `*.new` row
- **WHEN** `infobase config export` runs
- **THEN** the export publishes `Config` and says why on standard error

### Requirement: A Config versions row is read by its pairs

A `versions` row of a `Config` or `ConfigSave` table SHALL be read by its
name/version pairs. The pair count in its header SHALL NOT be required to equal
the number of pairs (it SHALL NOT exceed it), and the service pairs (`root`,
`version`, `versions`) SHALL NOT be required, because a row the platform's own
import stages, and an apply of it leaves behind, has neither.

#### Scenario: A row a native apply left behind

- **GIVEN** a `Config` `versions` row whose header declares 9 834 pairs and which holds 9 836, with no service pairs
- **WHEN** the export builds `ConfigDumpInfo.xml`
- **THEN** it reads the 9 835 object entries the row holds

### Requirement: A command of a constant that uses no standard commands keeps its sentinel

A command interface that names a standard command of a constant whose
`UseStandardCommands` is `false` SHALL keep the platform's `<code>:<uuid>`
sentinel instead of a synthesized `Constant.<name>.StandardCommand.Open`.

#### Scenario: A subsystem hides the Open command of such a constant

- **GIVEN** a constant with `<UseStandardCommands>false</UseStandardCommands>`
- **AND** a subsystem whose command interface hides its `Open` command
- **WHEN** the export writes the subsystem's `Ext/CommandInterface.xml`
- **THEN** the command is written as `100:<constant uuid>`

### Requirement: ConfigDumpInfo lists the first count pairs of a versions row

`ConfigDumpInfo.xml` SHALL list the entries among the first `count` pairs of the
`versions` row, the generation entry being the first, where `count` is the
number the row's header declares. The pairs beyond it belong to the
configuration all the same: they take part in the inventory check and in the
decision what a stage publishes, and their files are written.

#### Scenario: A row a native import staged

- **GIVEN** a `ConfigSave` `versions` row whose header declares 9 834 pairs and which holds 9 836
- **WHEN** the export builds `ConfigDumpInfo.xml`
- **THEN** the last two entries of the row are not listed
- **AND** the files of their modules are written

### Requirement: The extension compatibility of a staged Configuration row is field 43

A Configuration row in the shape the platform writes (`{68,…}`) whose fields 26
and 43 differ SHALL be exported with field 26 as `CompatibilityMode` and field
43 as `ConfigurationExtensionCompatibilityMode`; a row whose fields agree is
exported as before.

#### Scenario: A native stage of an older configuration

- **GIVEN** a staged Configuration row with 80324 in field 26 and 80327 in field 43
- **WHEN** the export writes `Configuration.xml` for platform 8.3.27
- **THEN** `CompatibilityMode` is `Version8_3_24`
- **AND** `ConfigurationExtensionCompatibilityMode` is `Version8_3_27`
