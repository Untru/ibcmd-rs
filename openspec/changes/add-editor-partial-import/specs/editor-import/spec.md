## ADDED Requirements

### Requirement: Explicit file database connections
The editor MUST accept an exact file database through TOML `db-path` or RPC `dbPath`, resolve TOML-relative paths against their settings file, and reject conflicting source selectors or masks. Existing SQL entries MUST remain compatible and rows-dir MUST remain read-only.

#### Scenario: Conflicting selectors fail before opening
- **WHEN** a connection specifies both a server and a file database
- **THEN** connection validation refuses before opening or writing a target

### Requirement: Verified complete effective staging
Selected import MUST read locked CONFIG and CONFIGSAVE, construct the complete effective configuration, overlay only approved existing files, compile and verify the full expected tree, and preserve unselected objects and earlier staging. It MUST refuse unsupported additions/deletions, incomplete stages and unresolved references before publication.

#### Scenario: Existing module and form selection
- **WHEN** the user selects edits to an existing module and document form
- **THEN** the prepared stage contains those edits and retains unrelated effective objects and previous staged changes

#### Scenario: Unsupported or stale input
- **WHEN** selection introduces unsupported new metadata or the source/target/connection changes after preparation
- **THEN** publication is refused and the database remains unchanged

### Requirement: Confined guarded publication and recovery
The writer MUST retain its actual locked descriptor, revalidate identity/content and source state, check policy/cancellation at the final publication boundary, verify its work copy, and preserve recovery generations. Supported filesystem mutations MUST be relative to a retained parent-directory descriptor; unsupported platforms MUST refuse writes. These checks MUST NOT be described as unconditional atomic filesystem CAS against a malicious same-UID leaf-entry writer.

#### Scenario: Parent replacement after final guard
- **WHEN** an ancestor is renamed and replaced after the final check
- **THEN** publication and cleanup remain confined to the original held directory and do not mutate the replacement directory

#### Scenario: Backup update fails after publication
- **WHEN** the database rename succeeds and the subsequent backup rename fails
- **THEN** the result explicitly reports published data and retained recovery generations, without being overwritten by late cancellation

### Requirement: Confirmed editor import workflow
The client MUST expose selected and changed import, validate references, suggest dependencies, require separate consent before expanding scope and before publication, and refuse stale or dirty selections. Changed import MUST require a synchronized inventory baseline. The production client MUST invoke only Rust RPC, with native apply outside this workflow.

#### Scenario: Dependency confirmation is declined
- **WHEN** required source dependencies are suggested and the user declines
- **THEN** no expanded import is published

#### Scenario: Synthetic native acceptance
- **WHEN** selected module and form edits are staged on a new synthetic supported file database and applied/exported by the native platform in test orchestration
- **THEN** the expected selected source files and unchanged neighbour are reproduced, with inventory versions audited separately
