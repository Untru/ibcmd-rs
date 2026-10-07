# Case4 loaded retained LIVE and explicit reconnect

One fresh owned private83 lifetime measured loaded phase1 retention, refusal
with connected users, recovery after closing the owned cohort, and real BSP
posting/reports by three fresh clients. It does not establish warm readiness,
same-session refresh, zero-error switching, five loaded LIVE switches, throughput
or a native activation comparator. Production readiness was not relaxed.

Raw root: F:/ibcmd/lab/05/wave3/load. Reproduce read-only reconciliation with
`scripts/apply-lab/live/workload/analyze_private4.py <raw root> --output <fresh F output>`.
The analyzer rejects malformed report outcomes, duplicates, wrong physical UUIDs
and unposted rows; different existing output is refused. Six copied actual
negative fixtures and immutable-output checks passed with original hashes unchanged.

## Provenance and ownership

Actual CLI source: 426c61731c73f242142d4d13a7b948ac83d97d66; SHA256
D3EC1869F3CC3BFD5250EC246153685F8632F2E15C69B0398263E0B716F8AED5.
This was no-default-features DEBUG: functional wall times, not release performance.
The reviewed V8 controller used scripts8babc8b9ab49d88b722a9768e6903b49f8505e56
and startup213. All20 frozen file hashes and binary hash were checked before
execution. Historical preparation manifest
C08CE467855194AFD87096F927C9934C898AE3005673BED5E86E8B79AF5A4373 remains
immutable, including its preparation-time runtime_executed=false. Actual execution
is recorded separately in runtime-proof.json and executor-proof.json. Later
metadata receipt mapping is not retroactive case4 runtime provenance.

New database ibcmd_rs_05_load_w3_private4_20261001 was restored from owned
f5-owned-full.bak, the previously explicitly normalized settled BSP fixture.
No additional normalization occurred. Private cluster
7c467d0e-d30f-4a79-9744-fbda88556b1b registered IB
aa7d34b8-76c4-4401-b2cb-fdf32383ae8f on localhost:5541/RAS5545.
Exact8.3.27.2214/SQL17.0.1135.8 binding was checked. Startup passed first attempt.
Whole-lifetime worker, per-command native and heavy leases were held; TEMP/TMP
were fresh F directories. Unresolved child receipts were absent. Shared service83
case2 remained untouched. The reviewed real BSP invoice/full-report EPF SHA was
6E4A29C045346CEBA9BA364418D9CCCAD02ECEC1811DAD29D9FDE530AD31C3C7.

## Actual work

| Cohort | Returned posted UUID/report pairs | Client/server marker | Unreturned attempt |
|---|---:|---|---|
| old-a | 44 | LIVE-live1 / LIVE-live1 | 45, absent at final SQL readback |
| old-b | 39 | LIVE-live1 / LIVE-live1 | 40, absent at final SQL readback |
| old-c | 34 | LIVE-live1 / LIVE-live1 | 35, absent at final SQL readback |
| fresh-a | 15 | LIVE-f5-4 / LIVE-f5-4 | none |
| fresh-b | 12 | LIVE-f5-4 / LIVE-f5-4 | none |
| fresh-c | 9 | LIVE-f5-4 / LIVE-f5-4 | none |

All153 returned UUIDs matched153 physical `_Document41` rows with `_Posted=01`
and exact comments; no unexplained physical commit remained. SQL UUID byte mapping
was independently established against1434 earlier W2 receipts on this BSP schema.
Three old unreturned RPCs stalled and remain unconfirmed as responses/reports;
document absence is a final point-in-time observation. No old completed operation
occurred after phase1 returned. This is explicit reconnect recovery, not loaded
warm admission. Pending attempts and full original journals remain retained.

SIMPLE57231 and FULL-with-only-COPY_ONLY57235 refused before publication;
full Config/ConfigSave/Params header/data fingerprints were unchanged. After an
ordinary owned FULL backup, phase1 committed and returned continuation_required.
Warm continuation refused with cycle1/artifact and one owned backup set retained.
Its tail before/after SHA was
E821EEF0F522E9916892A02218F0DBEE67CDDBA22208D5C935D12074FDC08C71.
Exact owned closure allowed cycle2; HEADERONLY recorded two token-bound ordered
sets with adjacent LSNs. Repeat returned already_complete without a third cycle;
before/after and current tail SHA was
38C9658620190598EC6F15AF63DF3F6B014CF5A249476F41221A924500D3610A.
The analyzer independently recomputes full Config/ConfigSave/Params and Files
header/hash equality for warm refusal and repeat. Files equality is measured
for those pairs, not assumed for operational activity.

Standalone recovery JSON format2 and its pack were read back against all10
captured full-header rows, whole-pack/range hashes and legacy LIVE format1 token.
This is not the later compact LIVE format2 implementation from455. Generic undo
was not executed. Native partial import supplied the stage; no native loaded
activation comparator was added. Actual child command/timestamps are retained
separately from FIFO acquisition bookends.

## Cleanup and remaining work

Exact owned clients were closed, registration removed, and standard guarded
private stop passed without Purge. srvinfo/logs and archived identities remain
retained. Native/heavy/worker leases were released. cleanup.json and external
OWNED.json record the database awaiting standard60-minute cleanup. Shared case2
is outside this private cleanup. Connected working-cohort admission and broader
F5/load acceptance remain open; this adds one bounded recovery case.
