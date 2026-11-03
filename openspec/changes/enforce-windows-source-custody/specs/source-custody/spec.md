## ADDED Requirements

### Requirement: Share-participating Windows directory custody
Windows source root and ancestor custody SHALL use read-only directory access that participates in Windows delete-sharing checks and SHALL exclude DELETE sharing. Directory READ and WRITE sharing SHALL permit ordinary destination-parent add-child opens for unpinned child renames. Leaf read/share behavior SHALL remain unchanged with strict READ sharing and write/delete exclusion. Unavailable required access SHALL refuse without permission changes.

#### Scenario: Directory-only custody without a leaf
- **GIVEN** an owned empty local NTFS source root and a successful ancestor rename round-trip before capture
- **WHEN** only its RootAnchor directory capabilities are retained
- **THEN** root/ancestor DELETE opens and rename/delete operations SHALL fail with the intended sharing violation; they SHALL succeed again after release.

#### Scenario: Ancestor DELETE access already exists
- **GIVEN** an ordinary existing ancestor DELETE handle with permissive sharing
- **WHEN** source directory custody is acquired
- **THEN** acquisition SHALL refuse while that handle is alive and SHALL succeed after it closes, before any payload consumption or target mutation.

#### Scenario: Metadata-only probe is not custody
- **GIVEN** an explicit zero-access metadata directory handle
- **WHEN** the owned directory is renamed and restored
- **THEN** that probe SHALL not be counted as deny-delete custody or evidence for the stronger contract.

### Requirement: Independently admitted source identity profile
Windows source custody SHALL admit only the reviewed local fixed-NTFS profile using actual source handle filesystem/volume/file identity evidence. Target database admission SHALL NOT authorize the source profile. Remote/non-fixed/non-NTFS/unknown/inconsistent profiles SHALL refuse; no pathname identity fallback SHALL authorize capture.

#### Scenario: Unsupported source with an admitted target
- **GIVEN** an admitted NTFS target and a remote, ReFS or unknown source profile
- **WHEN** source capture is requested
- **THEN** source admission SHALL refuse independently before payload consumption or publication.

### Requirement: Preserve selected and detached source lifetimes
Selected pinned originals, detached immutable compiler caches, final witness/digest/census checks, typed cancellation, traversal/byte bounds and Unix behavior SHALL remain unchanged. The change SHALL NOT retain one descriptor chain per cached XML, freeze ordinary child edits, or claim universal ABA/atomic filesystem CAS.

#### Scenario: Idle child edits remain possible
- **GIVEN** a held source directory anchor without a pinned leaf
- **WHEN** ordinary child files are created or edited, or unpinned child entries are renamed (including entries created after capture)
- **THEN** those operations SHALL remain possible; directory custody SHALL not be presented as a frozen subtree.

#### Scenario: Detached cache changes before publication
- **GIVEN** cached immutable original bytes whose temporary file/parent handles have been released
- **WHEN** the nested source is edited or rebound
- **THEN** cached bytes SHALL remain original and final witness/digest/census validation SHALL refuse the changed source.

#### Scenario: Large XML census
- **GIVEN** a bounded census of many metadata files
- **WHEN** compiler bytes are consumed and revalidated
- **THEN** transient handle ownership SHALL remain bounded by traversal depth rather than retaining a chain for each cached XML.
