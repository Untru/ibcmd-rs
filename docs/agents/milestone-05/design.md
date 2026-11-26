# Milestone 0.5: first implementation wave

This wave resumes the preserved September 30 work on top of released v0.4.0.
It does not declare the milestone complete. Existing OpenSpec designs remain
authoritative: `direct-mssql-online-activation`, `add-mssql-live-generation-switch`,
`direct-mssql-extensions`, and `support-platform-8-5-live-activation`.

## Scope and acceptance

1. Recover the unmerged drop-in dynamic implementation (#347), reconcile it
   with the v0.4 import/apply changes, and verify its bounded, explicit-force
   contract. Preserve the refusal for a nonempty overlay deletion list until
   the native service-info protocol has been implemented and measured.
   `auto`, `disable`, and `prompt` must retain exclusive behavior.
2. Add an explicit `--live-checkpoint` alternative to the live SQL-connection-
   count readiness assumption (#409 F-5), with bounded session-aware checks
   and a recoverable continuation for already-staged activation. Preserve
   the default renderer; source apply/watch refuse the checkpoint option
   before staging. Separate committed promotion/cycle 1 from cycle 2. A repeated continuation
   must never blindly run a third cycle. Recovery evidence and unfinished
   session work must remain visible. Existing F-9/F-10 safety gates remain.
   The first checkpoint completes automatically only on a verified infobase
   without user sessions; active sessions leave continuation explicitly
   required. Shared guards protect source-stage writes and default live
   activation from an unfinished checkpoint. Adaptive warm-session readiness,
   high-level source orchestration and fresh session/load proof remain open.
3. Correct extension form interceptor read/write handling (#348): decode
   both primary and additional handler codes when an adopted form has no
   BaseForm. The measured After event uses an additional handler; Override
   uses the primary handler with code 2. Require exact export-back equality
   and native twins; preserve refusal of unsupported shapes.

Each direction has its own branch and worktree. Implementers commit only
their direction; the coordinator integrates after an independent combined
specification/correctness review. Shared files are reconciled by the
coordinator. Review findings return to the implementer, at most two iterations.

## Lab and preservation

All new work and evidence are on F:. Original worktrees, handover bundles,
reference trees, and existing research databases remain preserved.
`F:\ibcmd\lab\04\README.md` supplies lab safety rules; its old integration
branch pointer is historical. Write only newly restored, owned disposable
databases using the shared restore/registration tools. Never read `.env`.
Native writes and worker-cluster runs use their respective shared FIFO locks;
heavy operations use the heavy lock. Use four build/test threads and quick
gates in track worktrees. Full regression gates run once on integration.

Any 1C extension changed for delivery in a PR must increment its Version
using its existing format before a CFE build. Verify the built version and
report both versions. Lab fixture edits must record their version explicitly;
do not add data metadata solely to version a code change.

## Follow-on work

After this wave: #345 wider module/form/template coverage, #346 platform 8.5
generation switching, #349 measured concurrent-user load, remaining #348
drop-in extension import/apply and 8.5 writes, #418 Params marker parity,
and #344 final evidence/review. No capability is enabled merely by inference.
