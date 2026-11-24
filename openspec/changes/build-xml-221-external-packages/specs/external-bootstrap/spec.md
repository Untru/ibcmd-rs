## ADDED Requirements

### Requirement: XML 2.21 external bootstrap

The base-free writer SHALL select the native 8.5.1.1150 seed of the input object's kind for XML 2.21 and verify the compiled package before publication.

#### Scenario: A self-contained processor has only a root XML file

- **GIVEN** the native XML 2.21 tree of a self-contained 8.5.1.1150 data processor
- **WHEN** the caller runs base-free bootstrap and exports the resulting EPF
- **THEN** the exported tree SHALL match the native files byte for byte except for ConfigDumpInfo.xml
- **AND** the bootstrap report SHALL identify the 8.5.1.1150 target profile

#### Scenario: A report needs a descriptor decoder

- **GIVEN** a self-contained XML 2.21 report with a supported v5 descriptor and attribute types
- **WHEN** the caller builds the report and exports it again
- **THEN** the exported tree SHALL match the native files byte for byte except for ConfigDumpInfo.xml

#### Scenario: A caller requests an unsupported target or content

- **WHEN** a requested target conflicts with the seed or the compiler cannot round-trip the content
- **THEN** bootstrap SHALL return a diagnostic and SHALL leave no published package
