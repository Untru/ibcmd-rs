# Extract MCP platform admission contract

## Why

Issue #386 has a working Unix descriptor adapter, while platform-independent
operator policy is mixed with host-specific snapshots. A future Windows file-db
snapshot does not by itself prove the MCP settings/source/target boundary.

## What changes

- Extract typed capability/admission, argument/path, binding and budget controls.
- Describe the minimum snapshot adapter contract and test refusals with synthetic data.
- Keep current Windows/unknown-host and SQL admission refused. No new I/O backend,
  SQL writes, Windows mutations, native invocation or dependencies are introduced.
