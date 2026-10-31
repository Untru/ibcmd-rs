## ADDED Requirements

### Requirement: Detached directory provenance from the retained root
The source custody module SHALL construct an opaque detached directory witness only from a validated directory traversal rooted at the existing retained RootAnchor. Successful witnesses SHALL retain no descriptors or heap ancestry ledger.

#### Scenario: Stable directory and ordinary child edits
- **GIVEN** an admitted source root and one canonical relative directory
- **WHEN** the directory is witnessed repeatedly or an ordinary child is created/edited/renamed without replacing directory identities
- **THEN** root/member equality SHALL remain exact, the framed identity/spelling commitment SHALL remain equal, and no temporary handles SHALL prevent later detached child/directory operations.

#### Scenario: Directory or ancestor identity changes
- **GIVEN** an earlier witness and a distinct actual member or ancestor identity at the same relative spelling
- **WHEN** a fresh witness is captured
- **THEN** witness equality SHALL differ even if the retained leaf itself has the same identity under a replaced ancestor.

### Requirement: Anchored fail-closed traversal
Every component SHALL be checked for directory type, alias/reparse refusal, source profile and identity according to the host adapter. Unix traversal SHALL begin at the same retained root descriptor; Windows SHALL retain the reviewed parent-held chain without claiming HANDLE-relative semantics. Final edge/root checks SHALL precede return.

#### Scenario: Invalid spelling, regular file or linked component
- **GIVEN** a noncanonical/escaping relative spelling, a regular-file member, or a symlink/reparse component
- **WHEN** directory_witness is requested
- **THEN** it SHALL refuse without following the linked target or changing source data.

#### Scenario: Cancellation or changed root binding
- **GIVEN** a cancelled operation or a root pathname rebound to a distinct identity
- **WHEN** capture or name revalidation runs
- **THEN** it SHALL refuse, preserve typed cancellation where applicable and release all temporary descriptors.

### Requirement: Compact bounded identity commitment
The witness SHALL compare explicit root/member identities and a domain/version/type-separated SHA256 commitment to checked exact-relative framing and ordered component identities. It SHALL NOT use directory timestamps as membership proof or claim atomic census/universal ABA protection. The consumer SHALL charge path/key/witness/bookkeeping before retention.

#### Scenario: Repeated deep walks under a finite descriptor limit
- **GIVEN** a bounded synthetic directory tree and a finite process descriptor limit
- **WHEN** repeated successful/refused captures finish
- **THEN** descriptor use SHALL remain O(depth), witnesses SHALL retain no handles, and existing file-custody behavior SHALL remain unchanged.
