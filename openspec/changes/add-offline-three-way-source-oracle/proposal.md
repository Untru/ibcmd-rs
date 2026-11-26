# Proposal: add an offline three-way source oracle

## Why

Native-versus-ibcmd-rs parity cannot distinguish an implementation mismatch from
an EDT import/export divergence. A bounded third, independently produced tree
adds research evidence without making EDT/JVM a runtime dependency or exposing
application fixtures.

## What changes

- Add an offline CLI and PowerShell wrapper that only read three pre-existing
  trees: native `ibcmd`, EDT import/export, and `ibcmd-rs`.
- Require explicit source and tool-version provenance; emit stable JSON and
  Markdown with per-path raw hashes and deterministic tree hashes.
- Classify exactly five equality states and label every diagnosis as a candidate,
  never a fact inferred from hashes alone.
- Add bounded resource limits and synthetic hash-only tests/fixtures.

## Impact

No DCS/MXL extraction or writer code changes. Default builds and runs neither
EDT nor Java/JVM.
