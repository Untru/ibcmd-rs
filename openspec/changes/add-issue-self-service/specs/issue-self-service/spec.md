## ADDED Requirements

### Requirement: Public self-service ownership
The bot SHALL accept a new `/claim` only for an open unlocked unassigned Backlog or Ready issue in the configured project, assigning only the verified command author, setting In progress and recording its own Agent line while preserving other owners' text under the confirmed single writer protocol. A sole existing owner MAY use a new command to reconcile their own partial claim; In review SHALL NOT be demoted by `/claim`.

#### Scenario: External contributor takes a task
- **GIVEN** a human commenter without collaborator access and a free Backlog or Ready issue
- **WHEN** the commenter submits `/claim`
- **THEN** assignment and both project fields are read back before a successful receipt is published.

#### Scenario: Partial Backlog assignment
- **GIVEN** a Backlog claim assigned its author but the status write failed
- **WHEN** the same sole owner submits a fresh `/claim`
- **THEN** In progress and the bot Agent line are verified without a manual Ready transition.

#### Scenario: Task is not available
- **GIVEN** another assignee, an unavailable issue state or an unknown/missing project profile
- **WHEN** `/claim` is submitted
- **THEN** it is refused without an Assignees/Status/Agent mutation; an In review refusal names review only for that actual state, while other unavailable statuses explain the Backlog/Ready requirement.

### Requirement: Owner-only release and review
The bot SHALL permit only the verified sole owner to release a task or request review of their own open non-Draft PR in the same repository targeting master and explicitly referencing the issue. It SHALL NOT close issues, merge PRs or grant access.

#### Scenario: Release and review
- **GIVEN** the requester is the sole owner
- **WHEN** `/unclaim` or a valid `/review #PR` is submitted
- **THEN** the corresponding Ready/unassigned or In review/assigned state is verified and acknowledged.

#### Scenario: Foreign ownership or PR
- **GIVEN** a foreign owner/author/repository, a draft/closed/unrelated PR or malformed command
- **WHEN** a command is submitted
- **THEN** it is refused without changing tracking state.

### Requirement: Serialized durable command processing
The bot SHALL serialize commands per issue and create a digest-bound durable receipt before tracking mutations. Duplicate or stale commands SHALL NOT replay. Every tracking write SHALL use fresh guarded state and verify its result. An observed uncertain or partial operation SHALL NOT produce a success receipt. These checks SHALL NOT be described as conditional writes: direct Project edits between read and write are not detectable reliably and are outside the supported protocol.

#### Scenario: Concurrent and repeated claims
- **GIVEN** competing command deliveries or a repeated delivery
- **WHEN** requests are processed
- **THEN** a confirmed owner is never displaced and a recorded request is not applied again.

#### Scenario: Drift or API failure
- **GIVEN** an API write fails or a changed tracking snapshot is observed
- **WHEN** the operation is checked
- **THEN** further tracking writes stop, the receipt records refusal/error where possible, and no successful confirmation is emitted for an unverified state.

### Requirement: Trusted execution and separate project authentication
The workflow SHALL execute only trusted default-branch code for commands, treat comments as data, and isolate repository and project credentials. PR checks SHALL have no project secret or write permissions. Missing credentials SHALL refuse tracking mutations.

#### Scenario: Missing project credential
- **GIVEN** PROJECT_TOKEN is absent or cannot access the configured Project
- **WHEN** a command is received
- **THEN** no Assignees/Status/Agent write occurs and the contributor receives a configuration error where repository comment access permits.

### Requirement: Confirmed single writer protocol
Commands SHALL refuse all tracking mutations unless the repository owner has explicitly confirmed the single writer protocol through ISSUE_BOT_SINGLE_WRITER_CONFIRMED=true. Direct owner maintenance and other Status/Agent automations SHALL occur only after command ingestion is disabled and every running and queued command is verified terminal. The owner SHALL read back maintenance before enabling ingestion. Acknowledgement SHALL NOT be represented as API-level exclusion of direct editors.

#### Scenario: Owner has not confirmed the protocol
- **GIVEN** the acknowledgement variable is absent or not exactly true
- **WHEN** a public command is processed
- **THEN** no Assignees/Status/Agent write occurs and a configuration error is recorded where comment access permits.

#### Scenario: Owner maintenance
- **GIVEN** direct tracking edits are needed
- **WHEN** the owner disables ingestion and verifies all command jobs terminal
- **THEN** maintenance is performed and read back before re-enabling; old commands are not replayed.
