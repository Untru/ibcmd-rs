## ADDED Requirements

### Requirement: Selected extraction is exact and bounded

The CF CLI SHALL extract only one exact top-level element and SHALL decode it
under the configured resource limits using an explicit payload encoding.

#### Scenario: Exact native Task row is selected

- **WHEN** `cf extract` receives the exact Task UUID element name
- **THEN** only that element is read and its packed and unpacked SHA-256 values are reported

### Requirement: Evidence publication never overwrites

The CF CLI SHALL publish extracted bytes only into a destination directory that
does not exist before the command starts.

#### Scenario: Destination already exists

- **WHEN** the requested output directory already exists
- **THEN** extraction fails without modifying any file in that directory
