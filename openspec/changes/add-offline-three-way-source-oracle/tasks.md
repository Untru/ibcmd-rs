# Offline three-way source oracle — tasks

## Task 1: Implement offline evidence command

**Status:** `[x]`

- [x] Read only supplied native, EDT, and ibcmd-rs trees.
- [x] Require explicit source/tool provenance and bound resource use.
- [x] Write new deterministic JSON and Markdown artifacts only outside inputs.
- [x] Publish the artifact pair transactionally with synced same-parent temps,
  no-overwrite final creation, parent revalidation, and rollback.

## Task 2: Classify agreement without overclaiming cause

**Status:** `[x]`

- [x] Cover the five deterministic equality branches at path level.
- [x] Preserve raw file hashes/sizes and deterministic tree hashes.
- [x] Label non-equal branch interpretations as candidates, not facts.

## Task 3: Make the workflow safely reusable offline

**Status:** `[x]`

- [x] Add a wrapper that never launches EDT/JVM by default.
- [x] Add synthetic hash-only fixture/tests; do not commit application content or secrets.
- [x] Document use, limits, and evidence boundaries in the parity protocol.
- [x] Reject mutable/replaced input files and hostile output aliases/Markdown.
- [x] Pin Windows inputs without delete sharing and compare volume/file identity.
- [x] Reject non-UTF-8 Unix path components without lossy key collisions.

## Task 4: Validate

**Status:** `[x]`

- [x] Run formatting, focused tests, and focused clippy.
- [x] Run `openspec validate add-offline-three-way-source-oracle --strict`.
