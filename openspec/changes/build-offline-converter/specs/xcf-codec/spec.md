# XCF Codec

## ADDED Requirements

### Requirement: XML syntax and semantics are separate

The system MUST parse XML into an ordered lossless representation before mapping known nodes into the canonical model.

#### Scenario: Unknown namespaced node is encountered

- **WHEN** a known metadata object is parsed
- **THEN** the node, namespace, order and anchor are retained as opaque data

### Requirement: Dialects use baseline plus deltas

Each supported XML dialect MUST declare syntax, feature availability, defaults and ordering independently from platform build.

#### Scenario: Convert 2.20 to 2.21

- **WHEN** a structural difference exists
- **THEN** an explicit migration/dialect rule transforms it; replacing only the root version attribute is forbidden

### Requirement: Source-tree output is atomic and safe

The writer MUST reject traversal/duplicate paths, run complete preflight and publish through a temporary sibling directory.

#### Scenario: One object cannot be encoded

- **THEN** no partial output tree replaces the destination

