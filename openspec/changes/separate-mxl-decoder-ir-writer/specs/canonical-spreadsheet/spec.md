## ADDED Requirements

### Requirement: MXL decoder emits an explicit canonical hand-off

The MOXCEL decoder SHALL preserve decoded palette-slot provenance and an
explicit canonical-to-XML format-reference map before spreadsheet XML
projection begins.

#### Scenario: A decoded payload uses an identity format map

- **WHEN** the decoder proves that canonical and XML format slots are one-based
  identity
- **THEN** the canonical hand-off records an explicit identity map
- **AND** the XML writer does not treat an absent map as permission to infer a
  different mapping

#### Scenario: A decoded payload uses a non-one-based format map

- **WHEN** the decoder proves a non-identity canonical/XML mapping
- **THEN** the hand-off retains both directions of the bijection
- **AND** an inconsistent mapping returns a decoder diagnostic

### Requirement: Spreadsheet XML projection consumes a decoded plan

The spreadsheet XML writer SHALL consume the format output order and index map
provided by canonical IR rather than recalculate palette or `formatIndex` from
raw MOXCEL fields.

#### Scenario: The decoded plan is incomplete

- **WHEN** a plan does not completely and bijectively project the known format
  palette
- **THEN** the writer returns `mxl.writer.format-plan-incomplete`
- **AND** it emits no XML from a guessed replacement plan

### Requirement: MXL diagnostics identify the responsible layer

The MXL pipeline SHALL expose stable diagnostic stage and code for failures at
the decoder/writer boundary.

#### Scenario: Native body decode fails

- **WHEN** a supported container cannot be decoded into bounded canonical IR
- **THEN** the failure has stage `decoder` and a stable `mxl.decoder.*` code
- **AND** it is not reported as an XML writer defect

#### Scenario: XML schema evidence is absent

- **WHEN** a proposed change would require a new QName, element order or
  default-value rule without evidence
- **THEN** this change does not add that writer behaviour
- **AND** the existing evidenced projection remains unchanged
