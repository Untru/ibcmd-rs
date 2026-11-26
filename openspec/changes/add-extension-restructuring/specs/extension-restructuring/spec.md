## ADDED Requirements

### Requirement: Extension update creates data-bearing objects atomically
The updater SHALL support the measured additive own-catalog and independent-information-register extension updates on 8.5.1.1150 and 8.3.27.2214, with corresponding schema, names, registry and cache publication.

#### Scenario: Multiple new objects
- **GIVEN** a usable versioned extension with existing control records
- **WHEN** import and apply add multiple own catalogs and information registers
- **THEN** a fresh platform session can write and read the new objects
- **AND** existing records, references, main configuration and unrelated extensions remain intact
- **AND** native re-export and subsequent native apply confirm a consistent active configuration

### Requirement: Failed updates preserve the prior database
The updater SHALL reject unsupported or destructive plans before writes and SHALL roll back all DDL, metadata and data effects on failure or trial execution.

#### Scenario: Failure during publication
- **GIVEN** a planned extension schema addition
- **WHEN** execution fails after table creation or a concurrent registry/schema change is detected
- **THEN** no partial schema, metadata or data changes remain committed
- **AND** the previous extension remains usable

#### Scenario: Repeated application
- **GIVEN** an already applied extension image
- **WHEN** the same update is applied again
- **THEN** no duplicate objects, records or allocations are created

#### Scenario: Structural refusal has no replacement plan
- **GIVEN** a proposed stage that the nonstructural admission gate refuses
- **WHEN** schema planning returns no modeled migration
- **THEN** staging and publication retain the original refusal
- **AND** no ConfigCASSave, data or registry mutation occurs

#### Scenario: SQL engine cannot execute restructuring assertions
- **GIVEN** a structural update using ordered aggregates and modern catalog facts
- **WHEN** SQL Server is older than 2017, database compatibility is below 110,
  or the server capability facts cannot be established
- **THEN** a separate compatible preflight reports the unsupported capability
  before submitting SQL that depends on it or writing any data
- **AND** code-only updates do not acquire the structural SQL-version requirement

### Requirement: Accompanying data-owning objects are classified
The updater SHALL classify accompanying metadata by its measured physical effects, including non-predefined scheduled-job tables and their service mapping, and SHALL apply measured base data-separation contributions to extension-owned objects.

#### Scenario: Full application update
- **GIVEN** an extension update with new catalogs, registers, a common module, HTTP service, subsystem and non-predefined scheduled job
- **WHEN** its complete source tree is imported and applied
- **THEN** all required tables and metadata are published atomically
- **AND** existing control records and references remain available in a fresh session

#### Scenario: Unmodeled predefined job instance
- **GIVEN** a new predefined scheduled job requiring a persistent activation-condition instance
- **WHEN** its update is proposed without an evidenced instance writer
- **THEN** the command names the unsupported job before staging or publication
- **AND** no database mutation is performed

#### Scenario: Legacy structural container
- **GIVEN** an active or staged extension outside the measured structural format/profile pairing
- **WHEN** its update requires new SQL schema records
- **THEN** the updater reports the active and staged formats before staging or publication
- **AND** no manifest version is silently changed
- **AND** previously supported body-only overlays retain their prior format support

### Requirement: Direct SQL profile verification without an infobase account
Direct SQL import and apply SHALL NOT require an infobase application user or
password and SHALL NOT open an authenticated infobase session as a preflight.
The tool SHALL retain exact platform-build validation and live SQL-schema
validation, and SHALL state the evidence actually used for RAS registration.

#### Scenario: Protected infobase with SQL access
- **GIVEN** a registered infobase with application users and valid SQL access
- **WHEN** import or apply is invoked without infobase credentials
- **THEN** profile verification uses administrative summary data and SQL facts
- **AND** it does not call `rac infobase info` or request an application login

#### Scenario: Wrong platform build or absent registration
- **GIVEN** an unverified build or missing selected RAS registration
- **WHEN** the operation is prepared without application credentials
- **THEN** it refuses before staging and preserves existing data
### Requirement: Nested dialogs of existing own data processors
Whole-tree extension import SHALL compile additions of nested managed forms
on existing own DataProcessor objects together with a Version increment and
new tableless root children. The compiler SHALL verify the complete proposed
source image and SHALL NOT treat a form-only edit as a physical table change.

#### Scenario: Version, common module and processor dialog in one update
- **GIVEN** an extension containing an own processor with an existing form
- **WHEN** a higher-version source tree adds a managed dialog and common module
- **THEN** import compiles and stages the complete faithful image on both builds
- **AND** apply preserves the existing data schema and prior source objects

#### Scenario: Unrelated root property changes
- **GIVEN** an unmodeled semantic change to a root property
- **WHEN** it accompanies a nested-form addition
- **THEN** the compiler refuses before staging and identifies the changed property
#### Scenario: Root collection edits lack matching objects
- **GIVEN** existing root ChildObjects entries and their compiled descriptors
- **WHEN** an edit changes root membership without matching admitted descriptor changes,
  duplicates an entry or adds a root entry without a matching added object
- **THEN** the tree compiler refuses before staging
- **AND** full-tree and selected imports retain root/descriptor correspondence

#### Scenario: Coordinated tableless metadata removal
- **GIVEN** an already supported tableless object and its root entry
- **WHEN** both its descriptor and root entry are removed in the selected edit
- **THEN** the prior metadata-only admission remains supported
- **AND** full-image verification preserves the requested values and ordering

### Requirement: Proven Designer XML defaults remain importable
Designer XML source comparison SHALL treat only schema-owned, native-proven
omissions as their platform defaults. It SHALL preserve meaningful scalar text,
unknown properties and object identities instead of accepting arbitrary drift.

#### Scenario: Managed dialog with omitted default properties
- **GIVEN** a Designer-source button without Type and a form with CommandBarLocation None
- **WHEN** a higher-version extension adds the dialog on either measured build
- **THEN** the button is compiled with the native Designer default UsualButton
- **AND** XML 2.21 ShowCommandBar is derived as false and form metadata defaults
  UseInInterfaceCompatibilityMode to Any when those properties are absent
- **AND** XML 2.20 does not gain XML 2.21-only properties

#### Scenario: Only lexical XML differences
- **GIVEN** recognized metadata XML differing only in declaration, BOM or indentation
- **WHEN** the complete proposal is compared to its native-shaped source image
- **THEN** lexical differences do not prevent import
- **AND** changed values, identities, child order, default namespaces or used prefix bindings still refuse
#### Scenario: Native export declares an unused metadata prefix
- **GIVEN** a metadata descriptor without an unused palette prefix declaration
- **WHEN** native-shaped export includes that declaration without using the prefix
- **THEN** the source comparison treats the addition as lexical XML only
- **AND** prefixes used by element names, attribute names or QName-like values
  retain their declared namespace bindings, including inherited bindings
#### Scenario: SQL session visibility is not established
- **GIVEN** an SQL login whose effective permissions cannot establish visibility
  of other logins' sessions, or whose visibility facts are NULL or unsupported
- **WHEN** an actual-target session gate prepares activation or LIVE disruption
- **THEN** it refuses before publication or disruption instead of treating a
  filtered DMV as an empty database
- **AND** preflight and rendered guards require the documented server permission
  for the SQL major version without requesting an application account