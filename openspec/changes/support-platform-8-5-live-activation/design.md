# Platform 8.5 live activation design

## Safety boundary

The registered BSP infobase `bsp` on cluster `localhost:3541` is a source-only
oracle. It has multiple desktop and web sessions and MUST NOT receive test
writes, session termination, worker turn-off, scheduled-job changes, or SQL
recovery operations. All mutation tests run on a uniquely named disposable SQL
clone registered in an isolated 8.5 laboratory cluster or assignment boundary.

## Evidence-first profile

Platform build, XML dialect, and MSSQL storage profile remain independent axes.
The existing XML 2.21 codec is not evidence that 8.5 native storage matches
8.3.27. The implementation may reuse an invariant only after byte/row evidence
shows it is unchanged. Unknown or partially evidenced 8.5 builds fail closed.

The evidence matrix covers:

1. native full source export and bounded module/form/extension export;
2. `Config`, `ConfigSave`, `Params`, extension tables, and relevant generation
   markers before staging, after native staging, and after activation;
3. same-session client/server marker observation;
4. `rac` connection/process output and dedicated-worker replacement;
5. recovery and cleanup back to the original clone snapshot.

## Compatibility architecture

Profile detection produces an explicit platform/storage identity before a
write. Planning and rendering receive that identity instead of relying on a
global 8.3.27 assumption. Shared invariants remain common code; divergent row
layouts, marker rules, or generation transitions live behind a small profile
strategy. Reports include the detected build/profile and evidence status.

Worker activation retains the existing three fail-closed checks: dedicated
process before staging, assignment recheck before SQL promotion, and exact old
process recheck before handoff. The 8.5 test cluster must provide a dedicated
worker; the crowded source cluster is never signalled.

## Acceptance

Support is complete only when native and custom exports agree for the selected
cohorts, module/form/extension round-trips pass, an unchanged open 8.5 session
observes new client and server code, 8.3.27 regression tests remain green, and
the disposable laboratory state is removed or restored.
