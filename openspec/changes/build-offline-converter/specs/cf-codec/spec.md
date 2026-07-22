# Native CF Codec

## ADDED Requirements

### Requirement: CF containers are parsed natively

The system MUST parse supported Format15 and Format16 CF layouts, chained block pages, nested containers and stored/raw-DEFLATE payloads without external platform code.

#### Scenario: Read a multi-page element

- **GIVEN** a valid element spanning three pages
- **WHEN** the CF reader streams the element
- **THEN** bytes are returned in chain order without loading the full CF into memory

#### Scenario: Read a corrupt chain

- **GIVEN** a cycle, overlap, invalid address or truncated block
- **WHEN** parsing reaches the defect
- **THEN** it returns a typed corruption error without panic or unbounded allocation

### Requirement: CF output is deterministic and atomic

The system MUST preflight, write deterministic page/address plans to a temporary file, re-open the result for structural validation and atomically publish it.

#### Scenario: Write the same storage image twice

- **WHEN** identical profile/options are used
- **THEN** both outputs have identical bytes

#### Scenario: Writer fails after preflight

- **THEN** the existing destination remains unchanged and no partial destination is published

### Requirement: CF capability levels are explicit

The system MUST distinguish inspect, repack, export, overlay, bootstrap and cross-version conversion capabilities.

#### Scenario: Bootstrap compiler is incomplete

- **GIVEN** one required entry is `NeedsBase`
- **WHEN** XML-to-new-CF is requested
- **THEN** the build is rejected, while an explicitly supplied base-CF overlay may remain available

