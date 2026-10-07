# BSP load laboratory kit

Build `../build_workload.ps1 -LabRoot F:/ibcmd/lab/05/wave3/load`, then launch the
unchanged `../obs.ps1` against an explicitly owned new load clone with unique
labels, `-Mode poll -Srvr localhost:2541 -LabRoot <own lab>`.
The workload EPF accepts the same startup protocol. It waits for label.go; each
one-second iteration confirms one real BSP invoice COMMIT/posting and runs the
actual global invoice report. label.pause retains a warm marker observer,
label.stop or 600 seconds ends its timer. Stop/terminate only exact journal +
infobase/session/PID-owned clients; never treat a hibernating session as empty.
Build/native helpers require an unlinked F 0.5 lab with Track load OWNED.json;
native labels must have no pre-existing command, lock or result artifact.

The snapshot/case/ownership/reconciliation scripts preserve the actual wave3
fixture values deliberately. They refuse other databases or require fresh
artifact labels; adapt the manifest and exact bindings explicitly for a fresh
clone. No reference/corpus/standing database is writable by this kit.

`analyze.py <lab root>` checks the full journal and actual persisted UUID readback,
all five own outcomes/repeated full configuration row-header equality, preserves
errors, and reports phase latencies. During intervals use completed-call journal
timestamps within outer command bookends. Native bookends include FIFO waiting;
no child-execution-only overlap count is inferred without separate timestamps.

Raw failures, source/EPF/CLI hashes and actual native flags remain on F. See
`docs/apply/evidence/live-gate/load-wave3-2026-10-01.md`. This kit proves a bounded
BSP document/report workload, not a throughput ceiling or LIVE readiness.

`analyze_isolated.py <lab root>` reconciles the private83 journals against final
physical posted rows, retains unreturned attempts including commits without a
response, and counts completions against actual native-child bookends. Its
point-in-time SQL UUID mapping depends on the earlier W2 COM reconciliation.
See `docs/apply/evidence/live-gate/load-wave3-private83-2026-10-01.md` for the
separate current3ff DEBUG provenance, setup/harness failures and readiness limits.
Its summary publication creates a new file or accepts byte-identical content;
it refuses a different existing summary. Use `--output <fresh F 0.5 path>` for
a separate review result. Receipt fields are exact semicolon-delimited values;
duplicate fields and malformed committed/report status values refuse analysis.
