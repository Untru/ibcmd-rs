## ADDED Requirements

### Requirement: Native export queries share the storage/export pipeline
The system SHALL serve export info/status/objects on supported storage backends without invoking native ibcmd. Native stdout and --out artifacts SHALL match measured native bytes. Custom editor APIs SHALL remain compatible.

#### Scenario: Unchanged status
- WHEN the current version inventory equals --base
- THEN stdout is empty, exit code is zero, and an explicitly requested output file contains only UTF-8 BOM.

#### Scenario: Native incremental conflict
- WHEN drop-in export receives both --base and --sync
- THEN it SHALL fail before database access with native rc2 and the measured conflict text.
