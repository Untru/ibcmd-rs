## ADDED Requirements

### Requirement: Explicit standalone apply oracle
A platform-oracle build SHALL route a local PID apply to the explicitly
configured native executable and preserve its standard streams and exit code.

#### Scenario: Local PID apply with the oracle configured
- **GIVEN** a platform-oracle build with an absolute native executable path
- **WHEN** the operator runs infobase config apply with a positive PID
- **THEN** the tool starts that executable with the original apply arguments
- **AND** the server's interaction and exit code pass through

#### Scenario: Release-shaped build
- **GIVEN** a build without platform-oracle
- **WHEN** the operator requests PID apply
- **THEN** the CLI refuses the standalone route
