## ADDED Requirements

### Requirement: Explicit chart representation
Configuration filesystem export SHALL accept `--chart-series-names stored|russian|english`, with default `stored`, as immutable per-operation policy. It SHALL NOT infer publication language from host, source-language, ScriptVariant or object names.

#### Scenario: Stored compatibility
- **GIVEN** an export without a representation override
- **WHEN** a chart is rendered
- **THEN** stored localized bytes are preserved

#### Scenario: Explicit Russian and English labels
- **GIVEN** an unchanged generated mandatory extra-series name, including zero real series or a named palette
- **WHEN** Russian or English is explicitly selected
- **THEN** its content is respectively `Сводная` or `Pivot`
- **AND** localized item identifiers and unrelated chart data remain unchanged

#### Scenario: User label preservation
- **GIVEN** a name with `strIsChanged=true`
- **WHEN** any supported policy is selected
- **THEN** the complete stored localized text remains unchanged

### Requirement: Operation isolation and source compatibility
The policy SHALL be passed explicitly through nested chart rendering. Memory/editor reads and compiler source validation SHALL retain Stored behavior.

#### Scenario: Independent exports
- **GIVEN** exports using different policies
- **WHEN** rendering is interleaved or a request fails
- **THEN** no policy affects another operation or default decoder call

### Requirement: Refusal before publication
Unknown values and unsupported non-Stored incremental/base or extension exports SHALL fail before publishing files or inventory.

#### Scenario: Unknown value
- **GIVEN** an unsupported policy value
- **WHEN** CLI arguments are parsed
- **THEN** export is refused without output publication

#### Scenario: Incremental policy cannot be established
- **GIVEN** non-Stored policy with base/sync or an unsupported extension override
- **WHEN** export is requested
- **THEN** it fails before output publication
