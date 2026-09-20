## ADDED Requirements

### Requirement: Native write policy is declared by the platform profile

The MSSQL write entry points SHALL resolve `mssql.main.write` and
`mssql.extension.write` from the bundled profile of the selected platform build
and SHALL treat an undeclared capability as unsupported.

#### Scenario: Evidenced build is admitted

- **GIVEN** a platform profile that declares the required write capability as supported
- **WHEN** a write command selects that profile
- **THEN** the policy check passes and live verification continues

#### Scenario: Build without declared capability is rejected

- **GIVEN** a bundled platform profile that declares no write capability
- **WHEN** a write command selects it
- **THEN** the command fails before starting rac, sqlcmd or reading the source tree
- **AND** the diagnostic names the missing capability and the profile

### Requirement: Live storage identity matches the declared fingerprints

The live verification SHALL compare the observed `IBVersion` identity and the
canonical five-table schema digest with the fingerprints declared by the
selected profile, and SHALL reject a profile that declares neither.

#### Scenario: Observed identity differs from the declaration

- **GIVEN** a database whose `PlatformVersionReq` differs from the declared fingerprint
- **WHEN** a write command verifies the profile
- **THEN** it fails and reports both the observed and the declared value

### Requirement: Staging and publication refuse together

A command that stages and publishes an extension in one operation SHALL verify
the registry transition the publisher requires before writing the first staged
row.

#### Scenario: Registry envelope is not publishable

- **GIVEN** an extension whose registry envelope is outside the evidenced transition
- **WHEN** `mssql-apply-source-change` selects a source body in that extension
- **THEN** the command fails before staging
- **AND** `ConfigCASSave` keeps the row count it had before the command
