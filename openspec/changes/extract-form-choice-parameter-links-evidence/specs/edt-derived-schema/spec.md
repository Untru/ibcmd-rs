## ADDED Requirements

### Requirement: ChoiceParameterLinks evidence is exact-release derived

The research extractor SHALL accept only a top-level inventory array for EDT
`2025.2.3+30`, exact expected bundle coordinates, and exact JVM descriptors
for every inspected method before deriving ChoiceParameterLinks evidence.

#### Scenario: Installed release differs

- **WHEN** inventory or installed EDT does not identify exact release and
  bundle versions
- **THEN** extraction fails
- **AND** no evidence JSON is emitted

#### Scenario: Method name matches but descriptor differs

- **WHEN** javap contains the expected method name with a different JVM
  descriptor
- **THEN** extraction fails before its instructions contribute evidence

### Requirement: ChoiceParameterLinks evidence covers the verified writer slice

The evidence SHALL cover owner wrapper QName/prefix/item QName/empty/null/
version/order, verified item field order, name QName/default, datapath
QName/delegate/xsi-type, changeMode QName/default/lexical map and extension
behaviour.

QName evidence SHALL additionally require the exact runtime binding, subclass
hierarchy, absence of relevant subclass overrides, complete feature-map
envelope and exact base-provider fallback calls.

#### Scenario: A mandatory method or relationship is ambiguous

- **WHEN** a method, control-flow edge, QName provider, model default, DataPath
  delegate or regular-extension relationship is missing or ambiguous
- **THEN** extraction fails closed
- **AND** the property is not inferred from neighbouring bytecode

#### Scenario: A model-default opcode changes

- **WHEN** either name or changeMode default is no longer the accepted
  `aconst_null` slice
- **THEN** the same default-fact helper used by real extraction rejects it

### Requirement: Unverified properties remain research gaps

Properties not proved by the accepted bytecode SHALL be recorded as explicit
`not-proven` entries and SHALL NOT become production emission rules.

#### Scenario: Generic feature-writer semantics are not in scope

- **WHEN** a boolean feature-writer argument is observed but its semantic
  meaning is not independently proved
- **THEN** the raw argument may be recorded as evidence
- **AND** its emission meaning remains `not-proven`

### Requirement: Research evidence is autonomous and deterministic

The repository artifact SHALL contain only sanitized deterministic JSON and
synthetic javap selftest data, without EDT payload or machine-local paths.

#### Scenario: Extractor is repeated against the same EDT

- **WHEN** two independent extraction processes run with identical accepted
  inputs
- **THEN** output bytes are identical
- **AND** neither run modifies production form writer, coverage or baseline
