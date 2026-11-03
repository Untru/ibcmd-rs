## ADDED Requirements

### Requirement: Actual Windows partial-plan publication
The Windows integration target SHALL invoke actual FileDbPlan preparation and guarded publication on fresh public synthetic fixtures with admitted local fixed NTFS source/target profiles. A helper-only, zero-test, compile-only or unexpected-profile refusal SHALL NOT establish success.

#### Scenario: Two-stage existing-module import
- **GIVEN** an owned public tiny8.5.1 fixture and an actual stage1 descriptor Comment publication
- **WHEN** a fresh stage2 process prepares/publishes only an existing module despite stale unselected source edits
- **THEN** CONFIG full tuples SHALL remain unchanged, the exact pre-stage2 DB SHALL become backup, pending SHALL be true, complete CONFIGSAVE SHALL match the public effective CF view, and selected/staged/unselected file expectations SHALL hold.

### Requirement: Compiler and fixture isolation
Every compiler-stage case SHALL use a fresh exact child process with owned-root sentinel and finite read/source bounds, without resetting metadata globals or modifying production guards.

#### Scenario: Shared DB with fresh compiler per stage
- **GIVEN** one fresh owned target and one hashed Windows test executable
- **WHEN** descriptor and module stages run sequentially in distinct children
- **THEN** each child SHALL execute1 selected non-ignored test and all logs/receipts SHALL bind to the same fixture and donor stack, without claiming persistent-server reuse.

### Requirement: Refuse stale or unauthorized publication
Source census/contents, target snapshot/current connection, final policy, cancellation, budget and recovery checks SHALL run through Plan and SHALL preserve target and existing backup before publication.

#### Scenario: Source or target changes after prepare
- **GIVEN** a ready plan and an actually performed source or external-target mutation, or a different current target
- **WHEN** the plan is published
- **THEN** publication SHALL refuse and exact post-mutation target/backup bytes SHALL stay unchanged; an incidental sharing refusal to perform the mutation SHALL fail the test.

#### Scenario: Final policy or late cancellation
- **GIVEN** a ready plan and the actual final publication callback
- **WHEN** policy refuses or sets cancellation while returning Ok
- **THEN** nothing SHALL publish and cancellation SHALL remain typed after the final long checks.

#### Scenario: Recovery artifact or committed recovery error
- **GIVEN** a preexisting owned work/recovery artifact or a real backup-publication denial
- **WHEN** Plan publication runs
- **THEN** prepublication artifacts SHALL survive unchanged, or an already committed database SHALL remain committed with Error::Published and exact recoverable original data, never a no-op/automatic retry claim.
