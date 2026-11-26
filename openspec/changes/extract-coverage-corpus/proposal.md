# Extract the public Coverage corpus

## Why
Coverage configuration sources and native fixtures should be maintained independently of the converter. Issue #566 requests Untru/1c-fixtures, a pinned submodule, unchanged converter baselines and no reduction of mandatory checks.

## What changes
- Move public Coverage sources, native fixture records and native generation tools to Untru/1c-fixtures.
- Pin its published commit at tests/fixtures/1c-fixtures.
- Keep baseline.json, storage-baseline.json and converter refusal expectations in ibcmd-rs.
- Validate checkout identity, complete fixture inventory and baseline referential integrity before CI tests; initialize the submodule in existing workflows.
- Record byte-level migration evidence and verify a clean recursive clone.
