# Private83 loaded control and retained LIVE continuation

This supplements the shared-service W3 failure measurement. It establishes a
bounded private-cluster native control, strict setup refusals and recovery after
closing the owned stalled cohort. It does not establish warm readiness, five
loaded LIVE switches, zero-error switching or a throughput ceiling. Production
readiness code was not changed.

Raw root: F:/ibcmd/lab/05/wave3/load. Compact reconciliation and raw file hashes
are in `load-wave3-private83/summary.json`; reproduce them with
`scripts/apply-lab/live/workload/analyze_isolated.py <raw root>`.

## Exact fixture and executables

The new owned `ibcmd_rs_05_load_w3_isolated_20261001` was restored from the
owned f5-owned-full.bak, taken after the explicitly normalized settled fixture
and before shared-service case1. It was never simultaneously registered with
the service cluster. Fresh private83 cluster 14f52c9b-1ee2-4316-a889-a5210ffe23de
registered IB3481c01b-099c-43a1-ad59-af4c6df7cb25 on localhost:5541/RAS5545.
Platform 8.3.27.2214 and SQL 17.0.1135.8 were checked. The whole lifetime held
the load worker FIFO; each native import/apply held a separate native ticket.
Other cluster settings, processes and infobases were untouched.

The actual CLI was root's combined source
3ff022dd112215a660769bf0a839afd4198c3769, SHA256
B99D34D7EF884C90F1C6681D1562E4C2E5ADF7B0CED42A70C4C5A75321AC59F1.
It was a no-default-features DEBUG build: these are functional wall times, not
release performance. Its full Config-header/CAS changes differ from historical
6a controls; the binary/source attribution is kept separate. The workload EPF
SHA256 was 6E4A29C045346CEBA9BA364418D9CCCAD02ECEC1811DAD29D9FDE530AD31C3C7,
with operation-start before RPC and unknown-commit transport recording. The
actual BSP invoice posting and real report are the earlier reviewed W1/W2
workload, not polling substituted for work.

Initial 30s startup timed out before RAS/registration. Owned process identities
and available registry copies were saved, then guarded stop/purge passed.
The single authorized fresh 60s retry started in about55s. Locked registry-copy
failure and JSON-depth warnings remain in the first lifetime's evidence; no
complete copy of that locked file is claimed. The successful lifetime used
owned F TEMP/TMP. Its exact state/identity/registry archives are retained.

## Actual native and own work

| Cohort/control | Confirmed posted document/report pairs | Result |
|---|---:|---|
| Native old N3 | 95 | Client/server LIVE-live1; 11 returned during the actual native child, 31 after its return |
| Native fresh N3 | 37 | Client/server LIVE-native1 |
| Own SIMPLE setup refusal N3 | 80 | 57231 before artifact/tail/publication; 9846 full Config/ConfigSave header rows unchanged |
| Own FULL + COPY_ONLY setup refusal N3 | 82 | 57235 before artifact/tail/publication; 9846 full header rows unchanged |
| Own ordinary-FULL-chain old N3 | 66 | Phase1 committed; warm RAS refused cycle2; zero old-cohort operations returned after phase1 return |
| Own fresh N3 after closed-cohort continuation | 40 | Client/server LIVE-f5-3 |

Native child bookends are recorded after FIFO acquisition:
2026-10-01T16:23:57.7954776Z–16:24:01.7755415Z. The native31 post-return
operations still ran the old client **and server** marker. This directly shows
why activity or counter advance alone does not prove the new generation.
Native partial existing-module import followed by force --dynamic=disable left
zero Config aliases/markers, zero Params dynamic marker and empty ConfigSave.
No additional Params normalization/deletion was needed in this lifetime.

The restore helper resets SIMPLE. SET FULL plus COPY_ONLY was insufficient to
establish its fresh log chain. Both preparation errors and all their native,
SQL, RAS and client receipts are retained; the strict F9 guards were not relaxed.
An ordinary full backup then established the chain before the actual own switch.

Own phase1 returned at2026-10-01T16:34:49.9240986Z with continuation_required,
one owned tail set and token
86A9872078714CABEC21B39B55AF044F1891E6DEC95C56721FD3115A19094246.
Three post-return RAS inventories remained responsive. All three old clients
stopped returning RPC results; their markers remained LIVE-native1. Exact PID,
IB/session/start/command bindings and owned-window observations were saved.
No counter or hibernate state was used to admit the cohort.

The first external harness exited0 after its pending snapshots because its
function was named PowerShell's reserved `Continue` keyword. That exit is a
harness failure, not cycle2 acceptance. Its exact executed SHA/source is retained.
The separately hashed resume-only harness used InvokeContinuation and the saved
exact owned cohort; it did not repeat phase1. A fresh standalone warm refusal
left the tail SHA byte-identical:
0B44667ED1CF8ABBE5400294550F5E17BFF77DECCD5A48710E80E39CEB51DB5A.
After closing only those proven own sessions, continuation completed cycle2 at
16:38:33Z. Actual HEADERONLY contained exactly two ordered owned-token sets.
The repeated already_complete result executed no cycle2 and preserved tail SHA
F947A12B653599B4DE3538D0FFD11689B9D00AE94729C49C791E7B62D7712A11;
that SHA was independently recomputed from the retained current tail.

## Receipts, lost responses and limits

The final physical readback found401 posted documents. All400 journal-confirmed
UUIDs matched their exact attempt/comment and posted flag; all400 have report
receipts. One extra posted UUID8c2586d4-bdb4-11f1-8f5e-ac361be9dbd1 belongs to
native-old-b attempt32, whose RPC response never returned. It is a committed
document without a confirmed report/response and is excluded from completed
pairs. Own-old attempts28/23/18 and fresh-own-new-b attempt14 also never returned;
none had a persisted row at the final readback. Absence at that point does not
establish a general transport-error rollback rule.

The SQL UUID-byte mapping was independently established against all1434 earlier
W2 COM receipts on the same backup/schema. This isolated run uses physical SQL
readback, not an additional COM readback. Actual report success remains a client
journal receipt. No emitted journal error occurred in these bounded cohorts,
but five unreturned calls, stalled clients, setup refusals and the harness failure
prevent any zero-error claim. Counts include the setup-refusal cohorts and are
not transferred into five successful LIVE cases.

The shipped read-only analyzer reran successfully. Copied negative fixtures with
a wrong UUID, unposted physical row or duplicate receipt all refused. Scripts/docs
fmt and physical-policy checks passed; no new Rust build/test result is claimed.
Review found an unconditional summary overwrite and substring report-status
classification in the first analyzer checkpoint. The repair publishes only new
or byte-identical output, with a fresh `--output` option, and parses exact status
fields. False report values and duplicate known receipt fields refuse; negative publication
and identical-retry checks preserve the original summary bytes. The actual raw
400 report receipts contain exact report_ok=1 values; historical validator
defects do not count as successful malformed-input handling.
Historical text-mode output used CRLF; the new byte publication uses LF. A
default rerun therefore refuses that different historical summary and preserves
it. Use a fresh `--output` to reproduce its JSON content; identical retries of
that new byte output succeed. Startup and transport-error descriptions remain
free-form evidence rather than structured report receipts.

## Raw evidence and ownership

- logs/isolated-{start,start-retry,first-stop,register,unregister,final-stop}.*
  and isolated-binding.json: private lifetime and cleanup.
- logs/isolated-native-*; obs/isolated-native-*; exact executed
  isolated_native_control.ps1 SHA in its command JSON: native actual child,
  post-return work, source/stage/after snapshots and final markers.
- logs/isolated-3-* and isolated-full-3-*; the refused-simple/full comparison
  JSON and corresponding full storage snapshots: setup negative controls.
- logs/isolated-chain-3-*; snapshots/isolated-chain-3-*;
  isolated_checkpoint_chain.ps1 and isolated_resume_chain.ps1 with executed
  hashes: pending phase1, all failures, warm refusal, exact owned close,
  completion/header/noop, before/after tail SHA and fresh cohort.
- snapshots/isolated-physical-documents.txt, isolated-proof-summary.json,
  logs/isolated-reconciliation-source.txt and isolated-reconciliation-negatives.json:
  full per-attempt reconciliation, unknown commit and read-only negative copies.

All private clients were closed with their exact current owned binding. The IB
was unregistered; the private cluster stopped/purged through guarded kit paths;
its raw registry/log/state evidence was preserved outside the root. Worker/native
locks were released. The SQL clone awaits the standard60min cleanup guard.
The separate shared-service case2 DB/registration/pending artifact and unresolved
UUIDs remain as recorded in W3; no unproved termination or shared restart occurred.

Further loaded controls need fresh owned private lifetimes. A warm readiness
predicate remains unproved: native old-generation service continuity and own
stalled cohorts are different observations. Strict F10, chain/token identity,
empty-RAS admission and unknown-profile refusal remain in force. #409 F5 is open.
