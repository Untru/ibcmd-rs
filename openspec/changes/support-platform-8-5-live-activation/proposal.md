# Change: support platform 8.5 live source activation

## Why

The direct MSSQL source pipeline and worker handoff are evidenced only on
1C:Enterprise 8.3.27. Platform 8.5 uses XML dialect 2.21 and may change native
configuration storage, generation markers, cluster administration output, and
cache invalidation. Treating 8.5 as identical without native evidence risks
corrupting configuration storage or interrupting unrelated sessions.

## What Changes

- Add an explicit, evidenced MSSQL storage/activation profile for 8.5.1.1150.
- Capture native `ibcmd` and SQL snapshots on a disposable clone of the BSP
  demo infobase from the 8.5 cluster.
- Compare module, managed-form, and extension cohorts against the existing
  8.3.27 profile and isolate every changed invariant.
- Make platform/profile selection explicit and fail closed for unknown builds.
- Prove guarded `worker --watch` activation in an already-open 8.5 session on
  a dedicated laboratory worker process.
- Keep the original `bsp` infobase and its active sessions read-only.

## Impact

The MSSQL profile detector, source compiler/stager, activation planner, worker
handoff diagnostics, compatibility evidence, and integration tests may change.
No native 1C executable is introduced into the production implementation.
