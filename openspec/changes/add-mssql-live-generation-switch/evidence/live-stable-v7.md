# 8.3.27 same-session live activation evidence

Date: 2026-08-30

- Platform: 1C:Enterprise 8.3.27.2214.
- Cluster endpoint: `localhost:2541` (RAS `localhost:2545`).
- Disposable MSSQL database: `ibcmd_rs_activation_main_20260829`.
- Command: `mssql-apply-source-change --mode live` for
  `CommonForms/_ДемоПримечание/Ext/Form/Module.bsl`.
- Before generation: `da2bb388-dcaf-4e7b-a9c9-406676c8d8c3`.
- After generation: `b649f5db-13ff-452e-94d6-1f174a500059`.
- Client process remained PID `46304`.
- 1C session remained UUID `5c37289c-0641-4194-b82c-afdf3ab5d9f1`.
- Same-session marker after activation:
  `live-stable-client-v7|live-stable-server-v7`.
- `dynamic=Нет`: the active ordinary generation changed; this was not a
  dynamic-overlay result.
- Tool timings: active export 224 ms, classification 34 ms, staging 894 ms,
  activation 12,742 ms, total 13,907 ms.
- External wall clock including process startup: 14,982 ms.
- Reference full-database restore experiment on the same small demo database:
  approximately 9.6 s end to end (actual SQL restore approximately 0.621 s).

The live protocol deliberately retains both non-copy transaction-log backup
sets in one operator-owned `.trn` artifact and restores `MULTI_USER` after each
recovery cycle. A one-second inter-cycle window was rejected because the OLE DB
connection-recovery attempt overlapped the second `RESTORING` interval. Five
seconds was verified without a client error.

## Rust verification

- Focused `mssql_main_activation` tests: 12 passed.
- Live CLI parsing test: passed.
- Release build: passed.
- Full `cargo test`: 2,551 passed and 16 pre-existing/unrelated
  `mssql_dump` form/predefined-data tests failed. No failed test exercises a
  file changed by this live-activation change.
