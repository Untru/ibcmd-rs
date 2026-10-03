# ibcmd-rs — notes for coding agents

ibcmd-rs reimplements 1C:Enterprise's native `ibcmd` (configuration export and
import between XML and a Microsoft SQL Server database). What it does and how
to run it: `README.md`. History and the full command reference:
`docs/HISTORY.md`. Merge and release gates: `docs/release-criteria.md`.

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
