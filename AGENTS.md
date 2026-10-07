# ibcmd-rs — notes for coding agents

ibcmd-rs reimplements 1C:Enterprise's native `ibcmd` (configuration export and
import between XML and a Microsoft SQL Server database). What it does and how
to run it: `README.md`. History and the full command reference:
`docs/HISTORY.md`. Merge and release gates: `docs/release-criteria.md`.

## Visible work tracking

The user's working board is https://github.com/users/Untru/projects/9.
Before taking an issue or dispatching an implementer, the coordinator updates
its project Status to In progress and records the owner/current scope in Agent.
Update the board at each state transition and verify the values by reading
them back. Local plans and GitHub status labels do not replace project fields.
Put reviewable PRs in In review; incomplete issue acceptance remains open.
Do not mark future work started merely because it has a proposed owner.

## Releases

**Mandatory before every release:** go through the release's GitHub milestone
(Untru/ibcmd-rs, e.g. "0.3 — Прямая замена ibcmd.exe") and check that every
issue in it is actually done. Close each done issue with a comment naming the
evidence (commit, measurement, test), finish or explicitly move (with the
reason) any issue that is not done, and bring every status up to date. Do not
create the `v*` tag while the milestone holds an open or stale issue. Details:
"Milestone check before tagging" in `docs/release-criteria.md`.

## Issue tracking

When taking milestone issues into implementation, update their GitHub Project
status to `In progress` at the start. Keep the board synchronized with actual
work: move ready work to `In review`, and use `Done` only after the issue's
acceptance criteria are satisfied. A draft PR does not complete its issues.
