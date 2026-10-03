# Actual compact7: readonly archive guard refusal

The approved ONE runtimeV3 attempt ran frozen controller sourcead9f1e2d and
manifest781CFD57/100 with the historical matching fbf/81CC DEBUG binary.
It ended at the read-only prior-archive authority check before heavy acquisition,
restore, cluster startup, registration, clients, native import or phase1.
There is no SQL/1C activation, loaded operation or recovery-cycle result here.

Original controllerPID101168, parent/executor95692, was launched at
2026-10-02T06:39:41Z; exact PID/birth/executable/command/arguments are retained.
The original handle/tool session34695 proved direct exit1 at06:50:57Z.
Worker acquired at09:50:33 local and released rc0 using the same exact raw lease.
Per-operation resource proof sequence1→2 records successful acquire then release.
No foreign owner was released and no process was signalled. There was no retry.

The first refusal was `case5 saved PID/reused PID/root process present; no archive`
at the frozen readonly guard line69. A later bounded read-only census found
saved Case5 RAS PID99324 had been reused by a foreign pwsh.exe process:
old birth2026-10-01T21:15:13.035463Z, current birth2026-10-02T06:38:27.230062Z,
current parent40344. The current command had no load-cluster root marker and
there were no private listeners. Only sanitized identity fields and command
SHA are captured. The exact historical admission census was not saved, so this
later observation is not an independent reconstruction of that same census.
The numeric-PID conservative condition explains this current false-positive
boundary; no authority to signal the reused foreign PID is inferred.

Post-refusal bounded read-only SQL reports the fresh target database
ibcmd_rs_05_load_w3_compact7_20261002 ABSENT. Target ownership-manifest records0,
registration binding/private active state absent, native command artifacts0,
unknown child receipts0, latest resource operation resolved and worker owner
absent. Thus no DB unregister/drop/default60 cleanup is needed for this attempt.
The generic cleanup status has clean=false/default60 pending because startup
never occurred; the independent absence inventory supplies the actual scope.

The frozen100 hashes remained unchanged after terminal exit. The exact17-file
raw proof is F:/ibcmd/lab/05/wave3/load/compact7-runtime-guard-refusal-proof.json,
SHA256 A839CD20E1FA3F694F25CD48CBCBA63429133CA5F0A2B85D48A70C1CE7FB7B68.
Committed raw copies are byte-exact. Empty own compact7 TEMP/child receipts and
the two resource proofs remain preserved; retained compact6 TEMP is untouched.

A future separately authorized scope can fix the read-only archive predicate
to bind saved PID plus full birth/executable/command identity rather than
numeric reuse alone, retaining refusal for missing identity or root processes.
It must preserve this consumed attempt and receive fresh root/peer approval
and scheduling. No change, retry, warm admission or loaded acceptance is
established by this report.
