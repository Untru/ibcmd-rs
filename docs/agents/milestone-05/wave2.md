# Milestone 0.5 — second wave

Base: reviewed first-wave PR #420, `13d9c6dd` (2026-10-01).
User-visible board: https://github.com/users/Untru/projects/9.
This plan is a source checkpoint snapshot. Final validation/publication state
is recorded in PR #420 checks/body and F:/ibcmd/lab/05/wave2/coordinator handover;
the board Status and Agent fields remain the user-facing live tracking.
#347/#348/#409 and PR #420 are In review at the reviewed source freeze. The Agent
field identifies implementation, review and remaining evidence scopes.
Update the board at pickup/state transitions;
local files alone do not communicate task state to the user.

## D: repeated dynamic generation — release_review, #347

Worktree F:/ibcmd/src/ibcmd-rs-05-dynamic-wave2, feat/0.5-dynamic-overlay.
Lab F:/ibcmd/lab/05/wave2/dynamic; clone owner Track ui.

- [x] D1 Inventory native repeated import/force storage with a fresh BSP 8.3
  twin: versions, alias deletion, registration file lists and Params service
  rows. Reuse original reference trees read-only; no .ui licensing hypothesis.
- [x] D2 Implement only the measured pending-generation import/force path in
  source-import/effective-row/dynamic seams; retain strict refusal for unknown
  structures, profiles, alias layouts and drift. Request shared-file ownership
  before editing common extension or LIVE seams.
- [x] D3 Prove two consecutive native/own generations preserve the earlier
  change, export the requested source, and reject tampered/deleted/stale stage;
  retain atomic/CAS/unknown-commit guarantees. Focused regression tests and
  appropriate quick gates; record exact measured limits and cleanup ownership.
- [x] D4 Root independent code/evidence review before integration.

## L: real native/RAS checkpoint — s1_mix, #409 F-5

Worktree F:/ibcmd/src/ibcmd-rs-05-live-wave2, feat/0.5-live-native.
Lab F:/ibcmd/lab/05/wave2/live; clone owner Track live.

- [x] L1 Establish a fresh registered BSP 8.3 clone and real idle RAS/native
  staged activation plus continuation on measured SQL 17.0.1135.8. Prefer the
  existing service cluster; change no global cluster settings or other DBs.
  Diagnose the prior empty-registry worker failure without touching others.
- [x] L2 Exercise a real owned observer session, warm idle versus active work,
  deadline/refusal and repeated continuation. Record client/server generation,
  connection errors and ownership; five repetitions if the path is functional.
- [x] L3 Repair actual bounded readiness/recovery defects supported by those
  measurements. Do not infer warm readiness from idle SQL handles or expand
  engine/profile support without evidence. Default LIVE and F-9/F-10 preserved.
- [x] L4 Meaningful tests, raw logs and compact evidence; independent review
  by an available peer. Keep load/zero-error acceptance open if unproved.

## E: guarded extension Version edits — synonyms, #348

Worktree F:/ibcmd/src/ibcmd-rs-05-ext-wave2, feat/0.5-extension-version.
Lab F:/ibcmd/lab/05/wave2/extensions; clone owner Track ext.

- [x] E1 Trace `src/mssql_extension_tree_load.rs` and
  `src/mssql_extension_load.rs` root-property refusal; measure a native
  Version-only twin and an updated interceptor at the new version.
- [x] E2 Compile a Version-only root-property change using the existing format
  without inventing metadata or widening unrelated structural acceptance.
  Native validation/export must match source version and retain adopted roots.
- [x] E3 Prove own load/native activation/source export and four-extension
  baseline parity; targeted negative/root-drift tests and appropriate gates.
  For any delivered CFE, build it only after Version increment and independently
  verify built Version. No CFE-delivery claim without an actual verified CFE.
- [x] E4 Independent peer code/evidence review before integration.

## Coordinator

- [x] C0 Repair the #409 F-8 artifact publication race in
  `src/mssql.rs::write_new_or_identical`: publish fully written adjacent temp
  files without overwriting a concurrent artifact; bounded identical-file
  comparisons, concurrency/crash-boundary tests and independent review.
  Row-header completeness and online undo semantics remain separate F-8 items.
- [x] C1 Review each source delta and real evidence, then integrate accepted
  commits into PR #420; review any shared-file resolution independently.
- [ ] C2 Appropriate combined validation and current Windows/Linux PR CI.
- [ ] C3 Preserve Git/raw evidence/binaries on F, release owned registrations,
  processes/locks and clean new disposable DBs through the shared idle guard.
- [ ] C4 Update board, PR and handover with measured results/remaining criteria.

All tracks read F:/ibcmd/lab/04/README.md and use shared clone/register/FIFO
helpers. Native writes hold one native ticket per command; owned worker runs
hold the worker ticket; heavy operations serialize. Four build/test jobs,
worktree-local iter cache, no full/release track build, no .env reads, no old
DB/worktree/cache changes. Preserve wave1 fixes and fail-closed gates. Agents
commit locally; coordinator alone integrates/pushes and maintains board state.

Coordinator C0 accepted: independent review PASS, standalone four concurrency/
interruption tests PASS, no-default all-targets check PASS, required debug quick
gates PASS (3652 passed, 0 failed, 10 ignored). Publication is complete and
no-clobber; whole F-8 row headers/online undo and power-loss durability are not
claimed. Raw evidence: F:/ibcmd/lab/05/wave2/coordinator.
Extension checkpoint accepted after root code/evidence review and integrated
as `30a4b361` (source `b51df0fc`): measured Version-only and Version+After, all 464
source files each, native activation exact proposed CAS, four-extension reader
baseline preserved. Native CDI normalized only configVersion. No CFE/8.5 write
or full #348 closure claim. Both owned EXT clones were removed through the
shared 60-minute idle cleanup guard. #348 is In review with coordinator owner.

LIVE source checkpoint `05a96538` independently reviewed PASS by root and peer,
integrated as `cfee9cd5`. Renderer recovery-token comparison now accepts either
hex case without changing backup names. Bounded successful RAS session output
admits only ASCII whitespace; OEM names and hibernating sessions still refuse.
Native idle activation/continuation, byte-identical no-op and real active/warm
refusal are recorded on an explicitly normalized owned fixture. Its original
Config fingerprint omits Creation/Modified/Attributes; treat it as partial
header proof. Five bounded warm operations and a separate full-header control
were completed and independently reviewed. Historical warm2–5 tail equality
is a retained harness assertion, without separately saved historical SHA pairs.
The later stale-artifact regression retains both SHA values and full snapshots.
Followup source `9cdd1ecf` fixes CATCH lock-release context and passes independent
review, quick gates (3653/0/10), focused tests (18/0/1) and all-targets check.
Both owned infobases are unregistered and processes/locks released. Both LIVE
databases were removed through the shared default 60-minute idle cleanup guard;
the final exploratory cleanup log is retained in the coordinator laboratory.
Same-session refresh, connected-user readiness, load/zero-error acceptance and
whole #409 remain open.

Dynamic source `9e89f731` and repair `cce4ef91` passed root and two independent
final code/spec/evidence reviews, integrated as `0af45551` and `44ab9c13`.
Recovery now retains immutable siVersions bytes plus full planned RowMeta,
validates exact rewrite coverage before artifact creation, and does not reread
that row. Two meaningful regressions cover seven transient storage/header
states and invalid metadata/coverage. Matching quick gates PASS (3665/0/10),
iter build PASS; historical actual write CLI and final hardened CLI provenance
are distinct. The two-generation proof independently confirms 12,198 files per
export, CDI configVersion-only differences, both edits retained, exact 6/12
registration additions and preservation of prior aliases/stale registrations.
Whole-storage equality is false: three derived Files help/search cache payloads
differ from native. Native exclusive settlement of collected SI aliases remains
required; arbitrary inventories, active/load, 8.5 and recovery replay stay open.
At this source freeze C2-C4 validation/publication and the final UI idle cleanup
are pending; final results are maintained in PR #420 and coordinator handover.
