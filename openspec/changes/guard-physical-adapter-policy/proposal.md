# Proposal: guard physical-adapter policy surface

## Why

The physical MSSQL/module-blob adapters still contain compatibility decisions that
are necessarily more literal than the canonical schema layer.  A new object-
specific workaround or a new XML QName/order/default decision can otherwise be
introduced without an explicit inventory review.

## What changes

- Add a deterministic, offline PowerShell validator and a committed normalized
  baseline for the non-MXL physical adapter slice.
- Reject a newly scoped source file, or any new sensitive occurrence fingerprint
  relative to the baseline; removals remain valid.
- Run the validator and its synthetic self-tests on Linux and Windows in offline
  CI.
- Document scope, exclusions, baseline update review, and limitations.

## Impact

The guard covers `module_blob.rs` and selected `mssql_dump` decoder, fetch, and
raw-row modules.  It intentionally excludes source-oracle work and MXL/MOXL
production modules.  It is an inventory gate, not a replacement for moving
existing policy into schema-owned writers.
