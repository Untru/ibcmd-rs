# Canonical and Native Models

## ADDED Requirements

### Requirement: Native storage is represented independently of MSSQL and CF

The system MUST expose an ordered `StorageImage` containing logical entries, multipart identity, raw headers, packed/unpacked payloads, compression kind and provenance.

#### Scenario: Read equivalent storage from CF and MSSQL fixture

- **WHEN** both adapters decode their input
- **THEN** they can produce comparable `StorageImage` values without SQL or CF types leaking into the model

### Requirement: Semantic metadata uses a canonical graph

The system MUST model object UUIDs, logical identity, ownership, ordered properties, references, generated types and assets independently of source format.

#### Scenario: Validate an object graph

- **GIVEN** duplicate UUIDs, a dangling reference or an ownership cycle
- **WHEN** model validation runs
- **THEN** it produces stable path-addressed diagnostics and rejects encoding

### Requirement: Unknown data is preserved explicitly

Unknown data MUST be stored as anchored opaque facets with source profile, media type and digest.

#### Scenario: Same-profile no-op conversion

- **WHEN** an unchanged opaque payload is decoded and encoded for the same profile
- **THEN** passthrough mode preserves its original bytes

#### Scenario: Cross-profile opaque conversion

- **WHEN** no migration rule exists for an opaque facet
- **THEN** conversion fails by default instead of copying or dropping it silently

