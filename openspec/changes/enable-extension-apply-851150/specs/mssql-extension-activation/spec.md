## ADDED Requirements

### Requirement: Measured exact-build extension publication

The extension loader and activator SHALL admit platform 8.5.1.1150 only after matching the exact server build and native storage fingerprints, and SHALL publish supported selected-extension changes using their own implementation.

#### Scenario: Native-equivalent extension change

- **GIVEN** a verified 8.5.1.1150 database and a supported versioned extension edit
- **WHEN** XML is staged and the extension is applied
- **THEN** the active image and required service state match the independent native control, and native re-export contains the new version and content
- **AND** the main configuration and unrelated extensions remain unchanged

#### Scenario: Unknown or structural input

- **GIVEN** an unverified build or extension change requiring an unmodeled schema transition
- **WHEN** loading or applying it is requested
- **THEN** the operation fails explicitly before publication and preserves the active extension

#### Scenario: Concurrent stage or registry mutation

- **GIVEN** a prepared extension stage whose registry or namespace changes before publication
- **WHEN** activation executes
- **THEN** optimistic predicates reject the stale plan and the transaction rolls back

### Requirement: Standard extension import and apply

The drop-in CLI SHALL route config import --extension and config apply --extension to the selected-extension staging and activation paths with explicit profile verification.

#### Scenario: Standard command sequence

- **GIVEN** a supported existing extension on verified 8.5.1.1150 or 8.3.27.2214
- **WHEN** the operator imports its XML tree and applies that extension
- **THEN** import leaves a complete selected stage and apply publishes it without invoking native ibcmd

#### Scenario: Unsupported dynamic request

- **GIVEN** extension publication whose requested session behavior is not evidenced
- **WHEN** that behavior is requested
- **THEN** it is refused explicitly rather than silently falling back


#### Scenario: Measured exact-27 compatibility

- **GIVEN** the independent native 8.3.27.2214 control with storage format 80327
- **WHEN** the same standard import/apply sequence changes Version and supported bodies
- **THEN** native re-export and a fresh session observe the new content and version
- **AND** its measured marker rule remains separate from the exact 8.5 rule
