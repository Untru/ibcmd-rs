## ADDED Requirements

### Requirement: Handle-bound Windows snapshot fingerprint
WriteSession::snapshot SHALL retain its Result<String> API and provide an opaque identity/content equality token only for the admitted local fixed-NTFS Windows profile. It SHALL read the actual exclusively held file descriptor, retain ancestor custody, check named binding before and after the hash, and preserve cancellation and cumulative read-budget behavior. Unix token construction SHALL remain compatible.

#### Scenario: Stable locked descriptor
- **GIVEN** a supported clean-room database held by one Windows write session
- **WHEN** snapshot is called twice without changes
- **THEN** the same token SHALL be returned and ordinary external writer/delete/ancestor-renaming operations SHALL remain denied while the session is held.

#### Scenario: Ancestor already opened for delete access
- **GIVEN** an ordinary existing DELETE-access handle on a clean-room database ancestor
- **WHEN** the writer attempts to acquire share-participating ancestor custody without FILE_SHARE_DELETE
- **THEN** it SHALL refuse before database or recovery mutation; it SHALL NOT fall back to zero-access custody or change permissions.

#### Scenario: Same bytes on another identity
- **GIVEN** a completed preflight whose original leaf is subsequently replaced by a distinct live file containing identical bytes
- **WHEN** a new locked session takes a snapshot
- **THEN** the token SHALL differ, without claiming universal identity-reuse or ABA exclusion.

#### Scenario: Content changes between sessions
- **GIVEN** the same live file identity with changed content after the old session ended
- **WHEN** a new session takes a snapshot
- **THEN** the token SHALL differ.

### Requirement: Unsupported or cancelled snapshot refuses without publication
Unknown/non-NTFS/remote profiles, reparse or non-file identities, failed identity/volume queries, changed binding and cancellation SHALL refuse before partial-import publication. No pathname identity fallback SHALL authorize a snapshot.

#### Scenario: Unsupported profile
- **GIVEN** a remote/UNC, non-fixed, non-NTFS or unavailable identity profile
- **WHEN** snapshot admission is requested
- **THEN** it SHALL refuse with no database, work or recovery output mutation.

#### Scenario: Cancelled hashing
- **GIVEN** a bounded write session whose retained cancellation callback rejects during hashing
- **WHEN** snapshot is requested
- **THEN** it SHALL return that refusal, leave the database bytes unchanged and create no work or backup artifacts.

### Requirement: Verified Windows publication chain
Snapshot support SHALL preserve the existing actual-descriptor guard, deny-sharing custody, measured collation/index validation, verified-copy and Windows security checks, final revalidation and ordered data/backup publication. It SHALL NOT be described as unconditional atomic filesystem CAS or universal ABA protection. Acceptance SHALL include real Windows CI coverage of the whole chain.

#### Scenario: Snapshot to guarded publication
- **GIVEN** an admitted clean-room Windows database and an unchanged freshly validated snapshot
- **WHEN** guarded CONFIGSAVE replacement succeeds
- **THEN** the new stage SHALL read back exactly, unchanged CONFIG SHALL remain intact, and the backup SHALL contain the prior complete database.

#### Scenario: Final guard refuses
- **GIVEN** an existing backup and a verified work copy
- **WHEN** the final publication guard refuses
- **THEN** database and prior backup bytes SHALL remain unchanged and no new stage SHALL be published.

#### Scenario: Late backup publication failure
- **GIVEN** a successful data rename followed by a failed backup rename
- **WHEN** publication reports its result
- **THEN** it SHALL report the published-state error and retain the old database recovery generation and previous backup.
