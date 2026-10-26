## ADDED Requirements

### Requirement: Complete empty format projection
The spreadsheet writer SHALL accept an empty identity format pool and SHALL reject malformed empty or nonempty projections.

#### Scenario: No format pool
- **GIVEN** a document whose records only refer to default slot zero
- **WHEN** its packed spreadsheet body is extracted
- **THEN** extraction succeeds without inventing a format entry

### Requirement: Complete audit source references
The offline spreadsheet audit SHALL retain custom style-font references and SHALL continue refusing unresolved references and invalid font indices.

#### Scenario: Custom style font
- **GIVEN** a clean source tree with a declared StyleItem font and a spreadsheet using it
- **WHEN** the spreadsheet is packed, extracted and repacked
- **THEN** the style reference and table indexing survive the full round trip

### Requirement: Evidenced native colour and print properties
The spreadsheet codec SHALL round-trip Windows ScrollBar as native palette kind 1/code 0, key 0 string pageSize distinct from numeric paper, and key 18/value 0 as printAccuracy Auto.

#### Scenario: Named system colour
- **GIVEN** a format whose Windows colour is ScrollBar
- **WHEN** it is written and read
- **THEN** the semantic colour survives with its evidenced native code

#### Scenario: String page size and automatic accuracy
- **GIVEN** pageSize including an empty string, page geometry and printAccuracy Auto
- **WHEN** the document is written and read
- **THEN** the string member, geometry and accuracy remain present and repack identically

#### Scenario: Ambiguous or unknown print property
- **GIVEN** both numeric paper and string pageSize, an unknown accuracy, or malformed property types
- **WHEN** the codec processes those settings
- **THEN** it refuses instead of dropping or guessing values
