# Compact6 V2: bounded resource wait, runtime not reached

One root-authorized lifetime used clean source639bd0073a42037223535bc63650f6505030a24b
and frozen V2 manifest D26E884FB0C84580C69B03BCCAD1B867922DC5633E33A53E0EAA09BF2658C7B2.
All54 pins matched before entry, after worker grant and after exit. Source stayed
clean and unchanged. Matching fbf81CC binary was hash-checked; no activation
using it occurred during this attempt.

At 2026-10-01T22:01:37Z, the whole worker lease was acquired. The exact clean
Case5 archive completed at22:01:46Z into
F:/ibcmd/lab/05/wave3/load/evidence/compact6-case5-prior, preserving registry,
logs and stopped-state evidence without Purge. Before/after authority had the
same lease/stopped-state SHA and no saved/root process or private listeners.
The597 archived files are retained and hashed, totaling871096480 bytes.

The subsequent heavy acquire hit its approved20-minute deadline, returning1
at22:21:51Z. Its captured owner was the separate edt-uha85-affinity track.
Restore, startup, registration, workload, native staging and activation were
never dispatched. The controller did not retry. Its conservative potential-hold
cleanup attempted a heavy release, which returned1 because that owner was
foreign; the foreign lease stayed intact. Own worker release returned0.

Fresh bounded read-only SQL verified database
ibcmd_rs_05_load_w3_compact6_20261002 does not exist. Saved restore manifest,
binding and restore/start command evidence have no compact6 entry. No private
load state, root server processes, private listeners or uncertain child receipts
remain. Resource snapshot showed no worker owner and the foreign EDT track
holding heavy/native. No foreign process or resource was signalled or removed.
No DB was created, so database DROP/default60 cleanup was unnecessary.

The generic cleanup report conservatively has guarded_stop_clean=false and
resources_may_need_exact_owned_cleanup=true because no cluster was started.
Those fields do not establish an unclosed own runtime: the separate explicit
postwait SQL/CIM/receipt/manifest/resource absence proof records zero owned
resources. The executor returned1 with unchanged source and exact closure.

F:/ibcmd/lab/05/wave3/load/compact6-runtime-resource-proof.json, SHA256
72D05BACA921E350773C05CDD5F1616A18FE0961282FB81CC3950EAAB0B1E450,
binds613 retained files, including597 archive files. Repository summaries/raw
result copies and a pointer to that full immutable proof are in
compact6-resource-outcome/. Original compact6 V1 all52 pins, compact5 V4 all45
pins and all285 compact5 raw files were rehashed unchanged after this attempt.

This is a resource refusal with zero workload operations, not a measured SQL
or 1C apply failure. Phase1, warm refusal, cycle2, repeat, five malformed actual
copies and fresh N3 posting/report controls were not reached. F5/connected warm
readiness/idempotence/zero-error acceptance remains open. A future writer needs
separate scheduling and review of the already completed archive/evidence state;
this checkpoint grants no automatic retry or old-IB/artifact rebind.
