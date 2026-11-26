# Public issue self-service

## Why

External contributors cannot update Project 9 without individual write invitations. Issue #536 needs claim/release/review through comments while retaining owner-only completion and protected repository access.

## What changes

- Trusted issue-comment workflow and a Python standard-library API controller for `/claim`, `/unclaim`, `/review #PR`.
- Serial per-issue queue, durable request receipts, strict caller/issue/PR binding and readback of Assignees, Status and Agent.
- Offline API contract tests and read-only PR checks; setup/runbook for a separate personal-project credential.
- AGENTS/CONTRIBUTING use confirmed self-service instead of requiring collaborators to edit the board.
