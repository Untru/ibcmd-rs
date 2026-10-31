## ADDED Requirements

### Requirement: SQL preparation is bounded and read-only
The server SHALL prepare existing selected SQL main-configuration changes from
bounded original source bytes and an immutable target snapshot without target
DML. Unsupported changes SHALL fail before any target write.

#### Scenario: Unsupported or cancelled preparation
- **GIVEN** a source outside the admitted existing module/form cohort, an exceeded
  resource limit, or a cancelled request
- **WHEN** SQL preparation is requested
- **THEN** no target mutation is dispatched and no publishable token is returned.

### Requirement: Consent and plan binding precede publication
The server SHALL bind each one-use plan to actual SQL identity, connection
generation, source census and exact target readset. It SHALL require final
confirmation and a new preparation after dependency expansion.

#### Scenario: Unresolved references or declined confirmation
- **GIVEN** missing references, a declined dependency suggestion, or no final consent
- **WHEN** import is requested
- **THEN** CONFIG and CONFIGSAVE remain unchanged.

#### Scenario: Source or connection changes
- **GIVEN** a prepared plan whose source or connection binding changed
- **WHEN** publication is attempted
- **THEN** it is refused before target DML and requires fresh preparation.

### Requirement: Publication is same-session and preserves staging
The SQL adapter SHALL validate identity and the complete compiler readset and
CONFIGSAVE CAS under retained locks in the same session/transaction as target
DML. It SHALL preserve every unselected staged row.

#### Scenario: Concurrent target or staging change
- **GIVEN** target rows, database identity or pre-existing staging changed since prepare
- **WHEN** the publication transaction validates the plan
- **THEN** it refuses and leaves CONFIG and CONFIGSAVE exactly as found.

#### Scenario: Existing unselected stage
- **GIVEN** supported pre-existing staging for an unselected object
- **WHEN** a confirmed verified selected change is published
- **THEN** that staging remains byte-identical and only the selected staged keys change.

#### Scenario: Interrupted publication
- **GIVEN** failure before commit, or a lost response after possible commit
- **WHEN** the adapter reports the outcome
- **THEN** a pre-commit error rolls back, and an uncertain or published outcome is
  not falsely reported as cancellation with no writes.

### Requirement: SQL readiness requires actual acceptance
The SQL capability SHALL remain unadvertised until the guarded adapter and
actual disposable SQL acceptance are verified.

#### Scenario: Synthetic tests without SQL acceptance
- **GIVEN** only a recording backend and portable fixture tests passed
- **WHEN** capabilities or completion are reported
- **THEN** real SQL publication and native apply/export are still pending.

### Requirement: Snapshot acquisition is one bounded session request
The SQL snapshot adapter SHALL acquire identity, a measured storage schema and
all CONFIG/CONFIGSAVE physical parts in one request while retaining read locks
on both tables. It SHALL require a matching operator-measured SQL version/schema
profile, enforce cumulative limits before blob streaming, and accept no partial
snapshot after callback cancellation, transport failure or a missing terminal seal.

#### Scenario: Unsupported profile or interrupted stream
- **GIVEN** an unmeasured version/schema, an identity mismatch, an exceeded
  part/payload budget, cancellation or an incomplete result stream
- **WHEN** snapshot acquisition is requested
- **THEN** no immutable snapshot is returned, no target DML is dispatched and
  no fallback to separate-session reads or legacy staging is attempted.

#### Scenario: Synthetic transport contract
- **GIVEN** only a recording SQL client
- **WHEN** the single-request decoder and refusal tests pass
- **THEN** actual SQL lock/coherence/transport behavior remains an operator
  acceptance gate and SQL publication remains unavailable.

### Requirement: Partial staging cannot prune unselected coverage
The SQL planner SHALL retain an independently justified pre-stage projected body
coverage witness and original staging coverage before consulting incoming stage
versions. It SHALL refuse omitted or unexplained effective/compiled keys before
publication. Historical generation projection SHALL NOT authorize a new partial
stage to hide an unselected object.

#### Scenario: Partial inventory hides a configuration-only neighbor
- **GIVEN** pre-stage projected CONFIG contains A and B and staging names only A
- **WHEN** the candidate effective or compiled coverage contains only A
- **THEN** preparation refuses before target writes instead of validating the
  already-pruned set.

#### Scenario: Original staged neighbor is omitted
- **GIVEN** a previously admitted staged neighbor C independent of incoming versions
- **WHEN** effective or compiled coverage omits C
- **THEN** preparation refuses, and C's original physical headers and parts remain
  outside any replacement authority.

### Requirement: Compiler output evidence preserves raw ambiguity
The opt-in SQL evidence path SHALL retain raw old/new storage export entries,
notes, dispositions and descriptor candidates, bound to the immutable packed
views, source census, selection and platform/profile. OLD evidence SHALL be
captured before selected tree edits and NEW evidence SHALL come from export of
the exact independently verified compiled artifact. The existing FileDb planner
path SHALL retain its prior behavior.

#### Scenario: Duplicate or unsafe output claim
- **GIVEN** duplicate key/path claims, canonical slash/case collisions, an escaped
  path, unsupported profile or an exceeded witness budget
- **WHEN** bounded raw output evidence is indexed for an SQL candidate
- **THEN** the candidate refuses without overwriting a prior claim or publishing.

#### Scenario: Supported but incomplete or ambiguous ownership
- **GIVEN** a Supported entry missing an expected output or no independent proof
  of a complete canonical UUID/kind/role/parent chain
- **WHEN** selected mapping is requested
- **THEN** the candidate reports insufficient evidence and cannot gain mutation
  authority from the surviving outputs or a reduced canonical lookup map.

### Requirement: Compiler diagnostics cannot grant SQL authority
A library diagnostic consumer SHALL perform bounded source admission and opt-in
raw compiler evidence capture without DB I/O, RPC dispatch or publication. Its
caller-provided packed view/context SHALL NOT be classified as a verified SQL
snapshot. Results SHALL contain bounded counts/digests and explicit refusal,
never raw descriptor contents, object names or compiler note chains.

#### Scenario: Diagnostic-only stable output mapping
- **GIVEN** an admitted source and a caller-provided packed view with stable raw
  selected output pairs
- **WHEN** compiler diagnostics are requested
- **THEN** owner/completeness remains refused, no write token or accepted SQL
  capability is returned, and the source/cancellation guards are rechecked.

### Requirement: Engine observations remain bounded passive diagnostics
The opt-in diagnostic adapter SHALL bind old and compiled packed observations to
their separate canonical-map and ordered-view digests, enforce one combined
old/new observation budget, preserve typed cancellation and expose only redacted
counts/digests and unsupported dimensions. It SHALL retain the unconditional
owner-completeness refusal and final original-source validation.

#### Scenario: Two observations exhaust one request budget
- **GIVEN** old and compiled views under one diagnostic request
- **WHEN** a view exceeds its conservative non-transferable budget partition
- **THEN** the entire diagnostic refuses without partial evidence or SQL writes.

#### Scenario: Recognized header candidates without authority
- **GIVEN** engine candidates with key-anchored UUIDs or recognized graph families
- **WHEN** passive diagnostic observations succeed
- **THEN** owner, parent, body-role, versions and projection authority remain
  unavailable and publication remains false.

### Requirement: Snapshot module compilation derives exact existing keys
A CommonModule body candidate SHALL derive its exact stored key from the
captured Configuration membership, typed existing descriptor and registered
source role. It SHALL compile only immutable snapshot/source bytes without SQL
fetches or granting publication authority.

#### Scenario: Existing selected module and complete unrelated staging
- **GIVEN** a bounded coherent snapshot, complete original/staged inventories,
  unique existing owner identity and the exact registered Module source path
- **WHEN** its source text changes
- **THEN** only that body's packed bytes and its validated versions delta are
  returned, while the original physical rows, neighbour headers and parts remain
  unchanged.

#### Scenario: Ambiguous owner or destructive module re-encoding
- **GIVEN** duplicate membership, a root/descriptor identity mismatch, colliding
  source names, a wrong role or unsupported module-container members
- **WHEN** a module candidate is compiled
- **THEN** compilation refuses without producing a replacement or contacting SQL.

#### Scenario: Stale-count versions contain an unselected trailing entry
- **GIVEN** a valid SQL inventory whose trailing entries exceed its declared count
- **WHEN** a selected module generation changes
- **THEN** every unselected entry including trailing generations, all service
  generations, pair order and declared count remain exact; omission, addition or
  mutation of any such entry refuses.

### Requirement: Existing Form module edits retain complete declared ownership
A Document-owned Form module candidate SHALL resolve original Configuration,
Document and Form occurrences through admitted typed codecs before lookup-map
reduction. Unexamined root families SHALL refuse. Compilation SHALL retain every
unselected Form field and metadata descriptor.

#### Scenario: Existing managed Form module changes
- **GIVEN** one unique root-to-Document-to-Form chain and a supported body/profile
- **WHEN** its exact registered FormModule source is compiled from captured bytes
- **THEN** only that body and the validated versions delta are returned, with
  identical revision, layout and trailing sections and no SQL contact.

#### Scenario: Competing owner or identity
- **GIVEN** duplicate membership, multiple parents, a generated/child identity
  collision, mismatched descriptor UUID or case-colliding owner/Form paths
- **WHEN** the Form module is prepared
- **THEN** it refuses before creating a replacement or contacting SQL.

### Requirement: Transaction admission compares complete physical readsets
The SQL publisher SHALL compare identity, schema, complete CONFIG/CONFIGSAVE
coverage, physical keys, sizes, headers and raw bytes while locks remain held on
its one dedicated transaction session. A pure readset comparison or a capture
whose batch rolled back SHALL NOT authorize later writes on another session.

#### Scenario: Unselected physical state changes
- **GIVEN** a confirmed plan and a later changed neighbour/header/part/schema or
  target identity, including additions and deletions outside the selected keys
- **WHEN** transaction admission compares the current full readset
- **THEN** it refuses before application DML rather than trusting a selected-only
  comparison or a digest.

#### Scenario: COMMIT acknowledgement is lost
- **GIVEN** COMMIT was dispatched but transport ended before its acknowledgement
- **WHEN** the operation reports its outcome
- **THEN** it reports unknown, preserves reconciliation evidence and never retries
  or reports an unproven no-write cancellation.


### Requirement: Dedicated session work cannot escape into pool reuse or replay
The SQL session seam SHALL invoke work once on an owned existing-target
connection, default to Unsupported for other clients and refuse further
application work after failed requests. A poisoned session SHALL NOT return
success because its callback swallowed the failure.

#### Scenario: Failed or unwound request
- **GIVEN** a failed driver or row callback request, including a panic caught by
  the outer callback
- **WHEN** more application work or callback success is attempted
- **THEN** it refuses without replay or pool reuse, and closes the owned connection.

#### Scenario: Explicit rollback-only cleanup
- **GIVEN** a usable dedicated connection with an active transaction
- **WHEN** rollback-only cleanup succeeds
- **THEN** the rollback was acknowledged and the session is terminal. An absent
  transaction, unusable protocol state or missing acknowledgement SHALL NOT be
  reported as a successful rollback.


### Requirement: Editor SQL operation is one existing CommonModule or Document FormModule
The SQL editor operation SHALL accept exactly one canonical existing CommonModule
`Ext/Module.bsl` or Document-owned FormModule `Ext/Form/Module.bsl` within the
admitted Document-only root cohort, with mandatory verification. Its private server-owned
admission SHALL validate closed measured SQL storage semantics and current
execution identity using the same canonical fragment as snapshot and held CAS.
It SHALL retain Config, ConfigSave and Params locks in that order, refuse dynamic
markers and unsupported DML-affecting catalog features, and require single-part
selected body/versions output. It SHALL NOT infer a native platform build or
activate the staged configuration.

#### Scenario: Unsupported request cannot contact SQL
- **GIVEN** objects/changed/multiple files, another file role or disabled verification
- **WHEN** editor SQL preparation is requested
- **THEN** the request is refused before source or SQL I/O.

#### Scenario: Prepared editor operation is single-use and outcome preserving
- **GIVEN** an admitted source/snapshot-bound CommonModule or Document FormModule plan and explicit confirmation
- **WHEN** its target/epoch/TTL-bound token is consumed for publication
- **THEN** only retained bytes may be staged, full held CAS remains mandatory,
  and acknowledged Committed or Unknown outcomes SHALL NOT become cancellation
  or an automatic retry. Native activation remains separate.
