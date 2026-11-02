# SQL status acceptance boundary (#380)

This follow-up adds tests and an offline evidence verifier, not a new status or
write route. The existing `objects/status` contract and six-state classifier
remain unchanged. `SqlExec::with_client` can inject a recording client into
`StoredConfiguration`; the public server session API cannot inject it into
`serve --stdio`. Direct API assertions therefore do not prove TDS, RPC/session
selection, editor generation handling, authentication or live SQL parity.

## Portable preparation

`tests/editor_sql_status_contract.rs` feeds committed public tiny CONFIG rows
through the real stored-configuration reader, exporter and native-version
comparison. Its SQL client accepts only known row/header SELECT shapes, records
every request and refuses scripts, statement execution, JSON and bulk writes.
It models plain one-part names and an unchanged-membership completed CONFIGSAVE
overlay; it is not a T-SQL interpreter and does not establish alias, multipart,
collation, isolation or physical stage/apply semantics. Unknown query shapes fail.

Scenarios cover active/main routing, completed versus distinguishably different unfinished stage inventory,
independent byte/native evidence, missing/source-only objects, source immutability,
read errors, cooperative cancellation with versionBaselineDir, reference drift and database-version drift, and rename/delete fullRequired refusal.
The stage/apply transition is an in-memory fixture transition only. All
process-wide engine/cancellation scenarios run serially in one exact child test;
the parent must observe exactly one passing test, not merely exit zero.

`scripts/status-lab/verify_capture.py` validates a bounded, already-collected
public smoke transcript. It invokes no binary, SQL, network or writer. It rejects
stale request generations, request/response/object/baseline/snapshot mismatches,
mutation of the source tree, cancelled replies presented as clean, and a staged
or applied phase without the corresponding external operator consent receipt.
Passing synthetic transcripts is contract preparation, never live acceptance.

## Future sole-operator SQL smoke (not executed)

1. Obtain an exclusive bounded operator lease for ONE newly owned disposable SQL
   database. Record lease id, start/expiry, operator, immutable public fixture
   identity, exact source HEAD and binary SHA-256 bound by an immutable clean build receipt, platform build,
   DB identity digest and chosen active/main mode. Credentials and host/database
   names remain in the private lab; publish only sanitized identities and counts.
2. Use a fresh external run directory. Hash the public source/reference trees and
   preserve them. Capture baseline `tree/children` and `objects/status` with and
   without `versionBaselineDir`, exact request ids/generation, full responses,
   request deadline (30..300 seconds), native export-status output and before/after
   storage fingerprints. A timeout/error/cancellation is not a clean result.
3. Separately preview ONE supported selected stage/import. Stop if unsupported,
   stale, ambiguous, or a refusal occurs. Its owner validates the preview against
   source/DB identity and obtains explicit write consent before that owner's
   supported stage operation. Status itself has no write consent or mutation path.
4. Before/after the completed stage, capture active/main tree and status again.
   Compare native versions independently of file bytes; check the untouched
   control object and snapshot freshness. Preserve the same source tree hashes.
5. Only with a new exact apply preview/consent and a still-valid lease, let the
   apply owner run its supported apply. Capture status/native output/fingerprints
   again. Assert main semantic state survives apply while pending state clears;
   do not infer that CONFIG and CONFIGSAVE physical bytes stay identical across
   intentional writes. Require their fingerprints to remain stable within EACH
   read-only status interval, not across stage/apply.
6. Cancel one bounded status request; deliberately retire one client generation
   and retain its late response as rejected evidence. Request ids and generation
   must bind the final UI capture. No unknown/error may be relabeled same.
7. Validate the sanitized transcript offline, then independently review raw local
   provenance before any live-SQL claim. End the lease and retain owned artifacts
   for Trash-only cleanup. Never drop/purge a database or fixture as teardown.

## Current gate state

- [x] Current #380 comments and merged #484 coverage limits inspected.
- [x] Isolated worktree pinned to master `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4`.
- [x] Initial Rust contract suite at `42f703dd`: 2 passed under bounded jobs=2 grant; log `logs/issue-380-sql-contract-42f703dd.log`.
- [x] Revised discriminating stage/refusal controls at `2aff32c7`: 2 passed, 0 failed, 0 ignored; log `logs/issue-380-sql-contract-2aff32c7.log`; full slot released.
- [x] Offline transcript verifier: 29 synthetic controls passed (Python unittest).
- [x] Initial independent review: verifier token/parameter findings corrected; 11 rejection controls fail the prior verifier and pass now.
- [x] Actual pure client adapter: 6 tests pass, including equal-current-bytes both.
- [x] Independent review of the revised stage/client delta: APPROVE; static review plus prior lightweight controls.
- [x] Strict OpenSpec, pinned standalone Rustfmt and diff-check pass.
- [ ] Actual sole-operator SQL smoke after separate VM/operator grant.
- [ ] Live RPC/editor/native parity and supported stage/apply owner evidence.

No VM, Cargo, native process, SSH, access change or full CI is part of the initial
preparation. This document does not close #380 or upgrade prior mocked coverage
to live acceptance.

## Public testbrief coverage still separated

The SQL model has no dynamic aliases, multipart rows or added-object membership
changes. It therefore does not prove a staged-added-object tree, actual SQL
isolation or every storage topology. Unsupported native owner roles remain
covered here only at the client inconclusive-evidence boundary; existing server
owner tests are not claimed rerun. The direct cancellation case calls
`status_with_versions`, not just export. Actual RPC `-32800`, ObjectView clear/late
ignore and native Windows smoke require separate transport/platform evidence.

Synthetic Python results: 29 passed. Client adapter source SHA-256:
`63c42026d2e0465825d1b7f471b614d228f4d46b75d6a06f0b8997ddcb36ef6b`.
All local fixtures were retained; no old fixture or receipt was purged.

## First read-only operator package (local follow-up)

`readonly_probe.py` pins candidate `2aff32c7` / API base `6ca6ade0`, protocol
1.4, exact read-only request sequence, required private DB metadata, output/frame
caps and owned-child termination requirements. It prints a clearly synthetic,
non-executed plan; no VM/SSH/SQL/native/host/Cargo process is started. The code
checks framing/privacy and claimed inventory transitions only. It is not a
collector or proof of process timeout/reaping, live TDS, RPC or physical staging.

Current CLI source has package-version output, not embedded git identity.
Before any actual probe, an operator must independently verify an immutable
build receipt, binary hash, clean source/lockfile hashes and pinned toolchain.
The legacy full transcript verifier's embeddedCommit schema is not silently
reinterpreted; adapting that schema to real build receipts needs separate review.

Read-only preparation adds no mutation consent: stage/apply stays with SQL
primary. No target creation/drop, stage/apply, preview, authentication or access
change is executed by this package. See scripts/status-lab/README.md for the
first operator action and exact exclusions. Synthetic output is retained at
`logs/issue-380-readonly-probe-synthetic-plan.json`, never as live evidence.
