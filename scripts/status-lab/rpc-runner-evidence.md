# Bounded fake RPC runner evidence

Historical scope at checkpoint `005a4ffa`: local continuation after operator
plan `5e8a420d`, with public PR #532 then on `2aff32c7`. No runner delta had
been pushed at that checkpoint. The later Windows activation is described in
`windows-rpc-controls.md`; the historical hashes/results below are preserved. Candidate/API labels
are `2aff32c7` / `6ca6ade0`, protocol 1.4; these are semantic contract bindings,
not a claim that the Python interpreter is the product binary.

Fresh static review first rejected ownership, identity/contract sealing,
trailing-output and preparation bounds. Those findings were fixed before the
first subprocess execution. Review source hashes were explicitly checked before
every permitted runtime gate.

The first 20-test runner gate failed 11 cleanup controls. The report stayed
success=false; an allowlisted exception/errno-only diagnostic confirmed
PermissionError errno 1. No raw diagnostic text was published. The corrected
order reaps an exited peer without signalling an empty group; active admitted
groups retain bounded TERM/KILL escalation. EPERM is not ignored. Exact owned
PIDs from the initial warning log were checked and none remained; no broad kill,
manual cleanup, original mutation or deletion was performed.

After fresh static approval of the fix:

- Six targeted cleanup/normal/hang/cap/descendant controls passed (1.789 s).
- Complete 69-test Python suite passed (2.390 s).
- Final provenance-only report fields received static approval; unchanged
  contract/scope/peer/tests plus final runner hash were verified before the
  final complete suite: **69 passed, zero failed (2.401 s)**.
- No ResourceWarnings in the two successful complete gates. All owned peer
  handles were waited; bounded reader/writer threads joined. The descendants
  retain inherited pipes and readiness confirms their SIGTERM-ignore control.
- Every fixture/output is retained under the external lab; no permanent delete.

Final reviewed runtime SHA-256:

| File | SHA-256 |
| --- | --- |
| rpc-runner-contract.md | 4be279154cdb718fa48964d87a75e3e2c7f8a23372f298d285c039749459cb32 |
| process_scope.py | c1c2916168a7444b7c0ab3ab133bbec8ca3ff001a0ed5eacdc1dfc027104eadc |
| fake_rpc_peer.py | 0872a2f0b499f333c001ba859ab496ee2ad33682d3d0583aec9cd8e152a247a8 |
| rpc_probe.py | 5faf32fc83841d18703fe0d5bf92c55b369c48d5e1b3c983ba63d718d6f6f39e |
| test_rpc_probe.py | 58ef609e52d77c47cd470b5b4066f689ac0c33e8fc601e2768fdf1f1ba90ba5c |

Retained external logs (not repository fixtures):

- logs/issue-380-rpc-probe-9921b254-first.log
- logs/issue-380-rpc-probe-2feb9afe-diagnostic.log
- logs/issue-380-rpc-probe-d2f97169-targeted.log
- logs/issue-380-rpc-probe-d2f97169-full.log
- logs/issue-380-rpc-probe-5faf32fc-full.log

Reports carry finite case, exact seal/interpreter/peer/input digests, cancelled
request 6, discarded generation/request 7 and successful post-binding verification.
They always retain fixtureOnly=true, productExecuted=false,
liveAcceptance=false and publishedStatus=false. Success is transport-control
success, never a clean SQL configuration or live readiness assertion.

Remaining boundaries: only current macOS/POSIX runtime executed; Windows
containment is statically reviewed only. No actual ibcmd, SQL/TDS, VM, SSH,
native platform, Code host or Cargo was run in this gate. Actual process/kernel
I/O hard realtime and arbitrary breakaway executables are not claimed. Full
legacy transcript build-receipt schema integration and a real SQL operator lease
still require separate reviewed authorization/evidence.
