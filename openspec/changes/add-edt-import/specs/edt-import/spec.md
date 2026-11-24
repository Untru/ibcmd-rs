## ADDED Requirements

### Requirement: EDT import source

Whole-configuration import SHALL accept an EDT project selected by `--source-format edt` and its explicit `--source-profile`. The supported initial profiles SHALL be edt-2025.2.3-xml-2.20 and edt-2025.2.3-xml-2.21.

#### Scenario: Import and apply

- **GIVEN** a losslessly convertible project and a file, SQL Server or PostgreSQL infobase
- **WHEN** the user imports a configuration or selects an extension with `--extension`
- **THEN** staging SHALL consume the converted content without writing an intermediate Designer XML tree
- **AND** after native apply, native export SHALL match the two-step conversion and XML import

#### Scenario: Refuse before staging

- **GIVEN** an unknown profile, unsupported project file or setting, or an unaccounted conversion loss
- **WHEN** import inspects the project
- **THEN** it SHALL identify the offending path and refuse before changing ConfigSave
