# Metadata owner graph

## Requirement: Compact Catalog roots are closed and typed

The decoder SHALL accept the compact owner root only for Catalog layouts with
the exact root shape and evidenced owner-field counts. It SHALL materialize
missing declared child collections as empty canonical collections with normal
schema provenance.

#### Scenario: Another family uses a compact root

- **WHEN** a non-Catalog owner has the same two-field root shape
- **THEN** decoding SHALL fail as `RootShape`

## Requirement: Catalog tail semantics are schema-owned

Layout codes, empty sentinels and input/history modes SHALL be classified in
the metadata schema boundary. The physical adapter SHALL NOT select behavior
from application object names or UUIDs.

#### Scenario: A legacy marker is changed

- **WHEN** the legacy Characteristics/choice-history payload differs in count,
  arity or marker
- **THEN** the strict Characteristics decoder SHALL reject it
