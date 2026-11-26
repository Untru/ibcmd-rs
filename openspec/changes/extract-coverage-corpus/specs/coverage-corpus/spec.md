## ADDED Requirements

### Requirement: Reproducible external corpus
The converter SHALL consume the public Coverage corpus at the exact published submodule commit and retain all converter equality and refusal expectations in its own repository.

#### Scenario: Valid pinned checkout
- **GIVEN** an initialized checkout of the indexed corpus commit
- **WHEN** the mandatory corpus validation runs
- **THEN** all inventoried fixture bytes and baseline references are checked before converter tests.

#### Scenario: Missing or changed corpus
- **GIVEN** a missing submodule, a different commit, modified fixture bytes or an extra fixture
- **WHEN** validation runs
- **THEN** it SHALL fail with a concrete diagnostic rather than skip coverage or update baselines.

#### Scenario: Orphaned expectation
- **GIVEN** a local baseline naming a set, variant or stored path absent from the pinned corpus
- **WHEN** validation runs
- **THEN** it SHALL fail without changing the baseline.

### Requirement: Lossless extraction
Extraction SHALL account for every source corpus file and preserve native oracle bytes and all existing baseline bytes.

#### Scenario: Migration verification
- **GIVEN** the source snapshot and the extracted checkout
- **WHEN** the migration verifier runs
- **THEN** every source path SHALL be accounted for by an identical relocated file, an identical retained baseline or a documented lossless split.

### Requirement: Mandatory checks survive extraction
Existing mandatory CI jobs, triggers and platform matrices SHALL remain enabled and SHALL initialize the pinned corpus for fixture-dependent checks.

#### Scenario: Clean clone
- **GIVEN** a fresh recursive clone without private corpus credentials
- **WHEN** the validation and converter test suites run
- **THEN** they SHALL exercise the same native fixtures and baseline promises as before extraction.
