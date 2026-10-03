# Milestone 0.5: first-wave task board

Design: [design.md](design.md). Base: released v0.4.0 (`origin/master`).
Execute with `subagent-dev`; independent tracks may run in parallel.

## Dynamic track: #347

Worktree `F:\ibcmd\src\ibcmd-rs-05-dynamic`, branch `feat/0.5-dynamic-recovery`.
Source work: preserved `feat/0.5-dynamic` and `F:\ibcmd\lab\05\ui\STATUS.md`.
Files: `src/dropin/{apply,help,mod,parse}.rs`,
`src/mssql_config_apply/{dynamic,mod,sqlgen,errors,recovery}.rs`, related
profile/CLI seams and `docs/apply/dropin-dynamic*.md`.

- [x] D1 Recover the old dynamic commits without reverting v0.4 fixes.
- [x] D2 Verify explicit force, profile/size/body gates, no-op, reporting,
  and overlay-deletion refusal with meaningful focused tests.
- [x] D3 Run quick gates and document remaining native/session evidence.
- [x] D4 Independent review; repair findings before integration.

Checkpoint `d807ec00`: root independent review PASS; 3624 root library tests,
focused dynamic/drop-in/activation/profile suites and two CLI integration
targets pass. See [current evidence](../../apply/evidence/dropin-dynamic/recovery-20261001.md).
Fresh native/session acceptance and the pending-overlay import loop remain open.

## Live track: #409 F-5

Worktree `F:\ibcmd\src\ibcmd-rs-05-live`, branch `feat/0.5-live-resume`.
Source work: `feat/0.5-live-worker`, `F:\ibcmd\lab\05\live\STATUS.md`.
Files: `src/mssql_main_activation.rs`, dedicated live readiness/continuation
module(s), `src/mssql.rs`, `src/mssql_apply.rs` only necessary CLI seams,
`scripts/apply-lab/live/`, track-owned evidence/docs.

- [x] L1 Recover the preserved lab kit and model the first/second cycle states.
- [ ] L2 Implement bounded readiness that does not wait for idle SQL handles.
  First checkpoint: automatic completion only when verified RAS has no user
  sessions; otherwise report continuation required. Adaptive warm-session
  readiness remains a separate, uncompleted acceptance item.
- [x] L3 Implement guarded continuation with database/artifact identity,
  completion detection and refusal of ambiguous/repeated recovery states.
- [x] L4 Test idle/active/timeout/already-complete paths and maintain F-9/F-10 gates.
- [ ] L5 Run native/session smoke on new owned clones; record measured limits.
- [x] L6 Independent review; repair findings before integration.

Checkpoint `5818be39`: independent review PASS. Actual SQL-only header,
append, repeated no-op, pending/lock and foreign-backup guards are measured
on exact SQL Server 17.0.1135.8. Final targeted tests report 49 passed/0 failed/
1 ignored; all-targets/fmt/policy checks pass. Preceding full library run
reports 3611 passed/0 failed/10 ignored; the final two narrow fixes have
focused regressions. The scope uses an explicit already-staged option,
preserves default LIVE SQL, refuses source checkpoint apply/watch and
does not claim real RAS/session readiness. L2/L5 remain incomplete; SQL 16
and 1C 8.5 are unmeasured and refused. See [evidence](../../apply/evidence/live-gate/checkpoint-2026-10-01.md).

## Extension track: #348 interceptor remainder

Worktree `F:\ibcmd\src\ibcmd-rs-05-ext`, branch `feat/0.5-extension-interceptors`.
Source work: `F:\ibcmd\lab\05\ext\STATUS.md`, `tl\ic2_orc` evidence.
Files: `src/mssql_dump/extension/form.rs`, the form compiler event-handler
adapter identified by tracing, `docs/extensions/parity.md`, focused fixtures.

- [x] E1 Confirm native After/Override storage and exported XML shapes.
- [x] E2 Decode interceptor type from primary and additional-handler codes.
- [x] E3 Encode the evidenced no-BaseForm shape; keep unsupported forms refused.
- [x] E4 Test Before/After/Override and native-twin export-back equality;
  record fixture Version changes when applicable.
- [x] E5 Run quick gates and independent review; repair findings.

Checkpoint `c2cccffc`: independent reviewer PASS; final sequential quick
gates report 3607 passed/0 failed/9 ignored. Native Before/After/Override
reader twins and our After/Override staging followed by native activation
preserve all source files; ConfigDumpInfo differs only in configVersion
values. Fixture Version 1.7.0.0 → 1.7.0.1 was seeded natively. No CFE was
built; direct loader Version edits, further cohorts, drop-in and 8.5 remain
open. See [measured scope](../../extensions/evidence/form-interceptors-20261001.md).

## Coordinator

Worktree `F:\ibcmd\src\ibcmd-rs-05-integration`, branch `feat/0.5-integration`.

- [x] C1 Inventory preserved branches and define isolated ownership/acceptance.
- [x] C2 Resolve shared-file conflicts after independent review.
- [x] C3 Run appropriate integrated gates and record exact verified scope.
- [x] C4 Publish a reviewable first-wave PR and attach it to this chat.

Published draft [PR #420](https://github.com/Untru/ibcmd-rs/pull/420), attached
to the coordinator chat. Required GitHub checks govern merge readiness.

Integrated source `e8e88586` passes fmt, physical-adapter policy, all-targets
check, 2358 SQL-domain unit tests (2 ignored), and 13 drop-in/standalone-boundary
CLI tests. The three required strict OpenSpec validations also pass. Full
Windows/Linux CI is tracked at the PR; these local results do not replace
the required merge checks. See [validation](validation.md).

Follow-on ownership: dynamic track owns #345/#346 after #347; live track owns
#349 and remaining #409 readiness/load evidence; extension track owns the
remaining #348 drop-in/8.5 routes. The coordinator owns #344 final review and
#418 native marker-rule research after the apply seams have stabilized.

The open milestone is not closed by completing this board. Completion claims
must cite actual tests/evidence; unmeasured paths remain explicitly open.
