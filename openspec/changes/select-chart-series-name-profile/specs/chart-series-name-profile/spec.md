## ADDED Requirements

### Requirement: Evidence precedes eligibility selection
The implementation SHALL establish profile eligibility using reviewed controlled
source-owned experiments before introducing a public entrypoint or new default.
Zero real-series count and XML scale-element presence SHALL NOT independently be
assumed to select stored names from the currently available observations.

#### Scenario: A native observation contradicts the old shape predicate
- **GIVEN** an unchanged automatic name and zero real series with native English republication
- **WHEN** profile eligibility is designed
- **THEN** the old positive-count predicate is not treated as a universal gate
- **AND** unknown target-setting behavior remains unresolved until controlled evidence exists

### Requirement: Explicit chart automatic-name representation
The export SHALL select the stored or English automatic-series-name representation
from an explicit per-export profile, preserving stored behavior by default.

#### Scenario: Stored profile preserves the cache
- **GIVEN** a chart eligible for automatic-name republication
- **WHEN** the stored profile is selected or no override is supplied
- **THEN** the writer preserves the stored localized text

#### Scenario: English profile publishes an unchanged automatic name
- **GIVEN** the evidenced chart shape and an unchanged automatic-series name
- **WHEN** the English profile is selected
- **THEN** the writer emits the publicly evidenced English automatic name

#### Scenario: User text and excluded shapes remain unchanged
- **GIVEN** a user-changed name or a chart outside the evidenced shape
- **WHEN** either supported profile is selected
- **THEN** the writer preserves the stored text

### Requirement: Profile isolation and refusal
The export SHALL isolate policy per operation and refuse unknown profiles before
publishing output.

#### Scenario: Concurrent exports retain their profiles
- **GIVEN** exports with different supported profile selections
- **WHEN** their work overlaps or one request fails
- **THEN** neither selection changes the other operation's output

#### Scenario: An unsupported profile is refused
- **GIVEN** an unsupported profile value
- **WHEN** export is requested
- **THEN** it fails before publishing any output file or inventory
