## ADDED Requirements

### Requirement: Standalone EDT project adapter
The system SHALL read an EDT project into existing canonical metadata and
source-tree contracts without invoking EDT, Java or the 1C platform.

#### Scenario: Read complete project
- **WHEN** a supported EDT project with DT-INF and src is supplied
- **THEN** objects, UUIDs, properties, modules, forms and assets are inventoried
  and represented without silent losses, under bounded resource limits.

### Requirement: Explicit lossless conversion and publication
The system SHALL support explicit edt/xml conversion endpoints and profiles,
preflight all input files, and atomically publish only a validated new output.

#### Scenario: Unsupported or conflicting input
- **WHEN** an unknown cross-format fragment, conflicting path, ambiguous version
  or stale preservation record is encountered
- **THEN** conversion fails with diagnostic evidence before publishing output.

#### Scenario: Dry run
- **WHEN** dry-run is requested
- **THEN** full decoding and encode preflight run without output publication.

#### Scenario: Importable project and exact return
- **WHEN** a supported native XML tree is converted to EDT and back unchanged
- **THEN** EDT can import the project and the returned XML tree equals its input.

### Requirement: Independent full-corpus acceptance
The system SHALL verify both directions with installed EDT and native ibcmd on
BSP and ERP UH, independently of the borrowed converter's claims.

#### Scenario: Three-way oracle
- **WHEN** milestone acceptance is recorded
- **THEN** exact tool versions, commands, corpus identities and complete tree
  comparison verdicts are retained, and missing or failed runs do not count as pass.

#### Scenario: Exporter-specific comparison
- **WHEN** the two real exporters produce different serialization
- **THEN** each conversion route is compared exactly against its corresponding
  exporter baseline, all raw divergences remain visible, and native import and
  export success is distinguished from optional database activation.

#### Scenario: Existing source diagnostics
- **WHEN** the authentic project already contains structured validation findings
- **THEN** the generated project adds no diagnostic to the exact baseline multiset,
  existing findings remain reported, and new or unclassified import errors block acceptance.
