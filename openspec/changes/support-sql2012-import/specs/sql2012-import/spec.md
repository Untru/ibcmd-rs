## ADDED Requirements

### Requirement: Explicit SQL Server 2012 compatibility
The built-in SQL backend SHALL enable this compatibility route only with IBCMD_RS_MSSQL_COMPATIBILITY=2012 and SHALL preserve the original startup and query routes otherwise.

#### Scenario: Actual SQL Server 2012
- **GIVEN** the explicit 2012 selector
- **WHEN** a connection opens
- **THEN** the first query reads actual ProductVersion and admits only four numeric components with major 11 before session setup or command execution

#### Scenario: Other server or invalid selector
- **GIVEN** an invalid, empty or non-Unicode selector, or a selected server whose ProductVersion is NULL, malformed or not major 11
- **WHEN** the policy is resolved or the server version is read
- **THEN** the operation fails before any database write without echoing invalid selector input

#### Scenario: New server with legacy database level
- **GIVEN** a newer SQL Server with database compatibility level 110
- **WHEN** the 2012 route is selected
- **THEN** the actual server version causes refusal

### Requirement: Explicit native SQL encryption policy
The built-in SQL backend SHALL require encrypted TDS by default and SHALL select a different transport policy only from an explicit valid IBCMD_RS_MSSQL_ENCRYPTION value together with the 2012 compatibility selector.

#### Scenario: Default connection
- **GIVEN** no encryption environment setting
- **WHEN** a built-in client is constructed
- **THEN** all pooled and dedicated connections require encryption

#### Scenario: Explicit legacy transport
- **GIVEN** the 2012 compatibility selector and an explicit setting of required, login-only or disabled
- **WHEN** the client is constructed
- **THEN** the selected policy is retained for the lifetime of that client without automatic fallback

#### Scenario: Invalid transport setting
- **GIVEN** an empty, unknown or non-Unicode setting
- **WHEN** a client is constructed
- **THEN** construction fails before any connection or database write and does not echo the supplied value

### Requirement: SQL Server 2012 metadata reads
Only the verified SQL Server 2012 compatibility client SHALL read target counts, storage statistics and dynamic row names without server-side JSON or whole-blob server hashing.

#### Scenario: Existing configuration
- **GIVEN** typed Config row counts from the database
- **WHEN** import selects its target mode
- **THEN** it retains the current matching-UUID, empty-target and different-UUID behavior

#### Scenario: Empty and large storage statistics
- **GIVEN** zero rows with a NULL checksum, or counts and byte sizes exceeding 32-bit values
- **WHEN** statistics are read
- **THEN** NULL checksum and full 64-bit numeric values are preserved

#### Scenario: Dynamic generation rows
- **GIVEN** stored dynamic aliases and the generation marker, or an empty result
- **WHEN** full import obtains removal names
- **THEN** it returns every selected name using the existing case-insensitive ordering, or an empty list

#### Scenario: Default or modern client
- **GIVEN** a client without the verified 2012 compatibility predicate
- **WHEN** target counts, storage statistics or dynamic names are read
- **THEN** the existing JSON query paths are retained

### Requirement: Preserve import safeguards and scope
This change SHALL preserve candidate-versus-source verification, binary content handling and transactional ConfigSave publication, and SHALL NOT silently admit unsupported apply profiles or concurrent configuration work.

#### Scenario: Candidate mismatch
- **GIVEN** prepared records that differ from the requested complete XML tree
- **WHEN** import verifies the candidate
- **THEN** import refuses before publishing ConfigSave

#### Scenario: Unsupported apply build
- **GIVEN** a platform build without an own-apply profile
- **WHEN** transport and import metadata compatibility succeed
- **THEN** no apply profile is inferred from another exact build
