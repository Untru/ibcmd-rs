## ADDED Requirements

### Requirement: Native micro-corpus has exact provenance

Every committed native-evidence fixture SHALL identify the exact 1C build,
source XML version, executable digest, seed lineage, native round-trip and file
digests required to reproduce its proven claims.

#### Scenario: A mapping is promoted to platform-proven

- **WHEN** a raw-to-XML rule is implemented from the native micro-corpus
- **THEN** its fixture manifest identifies two stable native source rounds and the exact raw and XML evidence

### Requirement: Fast parity fails closed

The offline parity check SHALL reject digest drift, unknown physical enum tokens
and any byte difference in selected native outputs.

#### Scenario: Task scalar mapping regresses

- **WHEN** the Task decoder emits an XML value different from the committed native output
- **THEN** the focused offline test fails without invoking the 1C platform

### Requirement: Unica is a seed, not an oracle

Unica-derived inputs SHALL remain hypothesis-level until reproduced by the
pinned native platform.

#### Scenario: Unica output has no native round-trip

- **WHEN** a seed or heuristic is available only from Unica
- **THEN** it cannot be labelled platform-proven in the fixture manifest
