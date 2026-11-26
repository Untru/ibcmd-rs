---
name: openspec-change
description: Write or update an OpenSpec change under openspec/changes/ for ibcmd-rs and validate it strictly. Use when adding a capability (command, format, conversion route, gate), when an issue's scope or acceptance changes, or when the OpenSpec (strict) check fails.
compatibility: Needs Node.js for npx @fission-ai/openspec@1.6.0.
---

# OpenSpec change

## When

A new capability or a change to a behaviour contract gets a change. A bug fix
that restores specified behaviour, a refactor or a test-only change does not.

## Layout

Model: `openspec/changes/add-file-infobase-backend/`. Name the directory
verb-noun in kebab case (`add-…`, `extract-…`, `enforce-…`).

- `proposal.md`: `# <title>`, `## Why` (the problem and the issues),
  `## What changes`.
- `design.md`: the decisions, the alternatives rejected and why, the risks.
- `tasks.md`: numbered checkboxes with issue references. Tick one only when
  it is done with evidence; an open acceptance item keeps its issue open.
- `specs/<capability>/spec.md`: `## ADDED Requirements` (or `MODIFIED`,
  `REMOVED`), each `### Requirement:` written with SHALL and followed by
  `#### Scenario:` blocks of **GIVEN** / **WHEN** / **THEN** bullets. Cover
  the refusals too: what fails, and that nothing is written.
- `evidence/`: measurements (skill `native-parity`).

## Validate

```text
npx --yes @fission-ai/openspec@1.6.0 validate <change> --strict --no-interactive
```

Set `OPENSPEC_TELEMETRY=0`. The merge check "OpenSpec (strict)" validates the
changes listed in `.github/workflows/offline-e2e.yml`, and `cargo xtask ci`
reads the same list. Adding a change to that list makes it a merge gate; do
it when the maintainer accepts the change as a contract.
