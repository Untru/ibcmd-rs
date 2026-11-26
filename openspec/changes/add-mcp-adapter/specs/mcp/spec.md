## ADDED Requirements

### Requirement: Genuine local MCP protocol

The adapter SHALL use supported MCP stdio framing, version/capability handling,
tool schemas, cancellation and lifecycle from the maintained official Rust SDK.
stdout SHALL contain only protocol messages; backend diagnostics SHALL NOT leak
credentials through tool content. Legacy editor JSON-RPC SHALL remain separate.

#### Scenario: Real client initialization and discovery

- **WHEN** a supported MCP client connects through stdio
- **THEN** it can negotiate the supported protocol, enumerate six tools with schemas/help, and call them through the SDK
- **AND** both supported legacy initialization and modern discovery are exercised by real client tests

### Requirement: Fixed operator authorization

An operator SHALL select the exact target and source root outside tool arguments.
Every call SHALL check unchanged settings/policy and pinned target/source identity.
The adapter SHALL reject masks, policy changes, replaced roots, traversal,
symlinks, model-selected settings/auth/target overrides and resource violations.
Import SHALL require explicit `agent-write=true` on the exact fixed file database;
missing or false opt-in SHALL deny writes. SQL and rows-fixture mutation SHALL
remain unavailable in this adapter.

#### Scenario: Agent cannot opt itself into writes

- **GIVEN** a configured database without explicit write opt-in
- **WHEN** a client calls import or supplies a settings, target, source or opt-in override
- **THEN** the adapter refuses the call without changing the database or backup

#### Scenario: Policy revoked after preparation

- **GIVEN** an operator-authorized prepared import
- **WHEN** settings, policy, source custody or consumed database identity changes before publication
- **THEN** the adapter refuses publication without changing the database or previous backup

### Requirement: Shared configuration operations

Tree/read/export/status/compare/import SHALL use the common configuration engine.
The adapter SHALL NOT implement another backend or invoke native processes.
Status/compare SHALL include changed, base-only and local-only paths for selected
objects, including objects absent from the database. Confined source snapshots
SHALL be revalidated before returning comparison results.

#### Scenario: Local additions invalidate equality

- **WHEN** a selected object has a local-only file, or a source file is added or renamed during comparison
- **THEN** the result reports the local-only difference or refuses the unstable snapshot rather than silently reporting equality

### Requirement: Bounded and cancellable custody

The actual database and source readers SHALL enforce cumulative materialization
and traversal limits, including repeated blob references, empty directories and
depth. Cancellation SHALL be checked during census, metadata/blob processing,
source reads/hashing and immediately before publication. A prior unbounded
capture followed by rejection SHALL NOT count as enforcing a resource budget.

#### Scenario: Empty-directory enlargement

- **WHEN** a source gains excessive empty directories or depth after an earlier census
- **THEN** shared custody and final revalidation reject within the configured traversal budget

#### Scenario: Cancellation before publication

- **WHEN** a genuine client cancels an import before its atomic publication boundary
- **THEN** the database and previous backup remain unchanged

### Requirement: Confined atomic staging and stale-session safety

Import SHALL stage into CONFIGSAVE through the shared guarded API and SHALL NOT
apply the configuration. The API SHALL check the actual consumed descriptor
before parsing and inside the final publication callback. Publication and cleanup
SHALL remain confined to a held parent directory despite pathname ancestor
replacement. Unsupported platforms SHALL fail closed. Atomic publication SHALL
invalidate the current MCP session; a new session SHALL explicitly bind the new
identity. A published recovery error SHALL NOT be reported as a safe cancellation.
These are held-directory/advisory-lock guarantees; the adapter SHALL NOT claim
atomic filesystem CAS against noncooperating same-UID leaf-entry mutations.
Operators SHALL protect the database directory outside model-controlled edits.
Panic leftovers SHALL require operator inspection before retry rather than an
automatic reconnect or blind deletion.

#### Scenario: Ancestor replaced after final check

- **WHEN** the database parent pathname is rebound after the final identity/policy check
- **THEN** mutations remain confined to the held directory and cannot affect a replacement outside the admitted directory

#### Scenario: Client continues after publication

- **WHEN** an import publishes its stage
- **THEN** the response distinguishes staging from apply and requires restart
- **AND** subsequent calls in the old session refuse instead of silently reconnecting

### Requirement: Native acceptance evidence

Issue #386 SHALL remain incomplete until a genuine MCP client stages a changed
module in a disposable native filedb, an external orchestrator applies normally,
and native re-export matches source. Mock write-policy tests SHALL NOT substitute
for this roundtrip or for negative permission/race tests.

#### Scenario: Disposable module roundtrip

- **WHEN** an authorized MCP client imports a changed source module into a new lab database and the external test orchestrator runs native apply/export
- **THEN** exported module/source bytes match, unselected source files remain equal, and inventory IDs/versions are audited

### Requirement: Consistent source admission

Status, compare and import SHALL reject dot-prefixed source files, directories
and links before comparison or staging. Directory iterator pseudo-entries SHALL
be ignored; actual dot entries SHALL count toward the source entry budget.
Database-only reads SHALL not enumerate the source tree.

#### Scenario: Hidden source member

- **WHEN** source-root contains a hidden file, `.git` directory or hidden symlink
- **THEN** status, compare and import all refuse without changing the database or creating staging/backup files

#### Scenario: Hidden directory added after preparation

- **WHEN** an empty hidden directory is added after import preparation but before publication
- **THEN** the retained source census refuses publication and leaves the database and backup unchanged
- **AND** operator policy is rechecked after final census validation and before publication
