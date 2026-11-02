## ADDED Requirements

### Requirement: Filesystem directory provenance
Held-source revalidation SHALL compare explicitly filesystem-captured directory membership, type and detached identity. File-only inventories SHALL NOT authorize or stand for an empty filesystem directory census.

#### Scenario: Empty directory membership changes
- **GIVEN** an anchored source with unchanged keep.bin and an empty directory below admitted limits
- **WHEN** an empty directory is actually added, parked outside the source, or renamed after capture
- **THEN** require_unchanged SHALL refuse while regular-file bytes remain unchanged.

#### Scenario: Unknown versus proven empty
- **GIVEN** a legacy from_files inventory or digest-only projection
- **WHEN** it is considered as filesystem custody evidence
- **THEN** it SHALL remain Unbound and SHALL NOT be treated as Filesystem(empty); semantic file comparisons SHALL retain their existing meaning.

### Requirement: Anchored canonical directory census
Each captured directory SHALL be validated by the separately reviewed anchored directory-witness helper. Files and directories SHALL share one canonical namespace, and final recapture SHALL compare exact spellings and detached directory identities without directory-mtime equality.

#### Scenario: Case, alias or type changes
- **GIVEN** a filesystem-captured directory census
- **WHEN** exact spelling, directory identity or type changes, or a file/directory case alias is encountered
- **THEN** capture or revalidation SHALL refuse without a pathname-only custody fallback.

### Requirement: Bounded detached proof
Filesystem census retention SHALL have explicit cumulative entry and metadata-byte bounds, preserving caller traversal/depth/cancellation rules and O(depth) transient handles. Legacy admission changes SHALL be documented and reviewed before implementation.

#### Scenario: Retained proof exceeds its bound
- **GIVEN** a finite admitted census budget
- **WHEN** another namespace entry or charged spelling/identity record would exceed it
- **THEN** capture SHALL refuse before retaining that record; final recapture SHALL enforce the same bounds.

#### Scenario: Unchanged source
- **GIVEN** a filesystem-captured source within its bounds
- **WHEN** directory membership/identities and all regular-file witnesses remain unchanged
- **THEN** held-source revalidation SHALL succeed without retaining a descriptor for every directory.
