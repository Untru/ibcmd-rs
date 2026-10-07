# Current 0.5 verification checkpoint

The immutable source `7aebba15` completed the full local run with **14 PASS and
two packaging failures**. Both packaging scripts stopped at Python's inherited
invalid stdin handle (`WinError 6`); no Rust test or compilation failed. The
original failed outcome remains unchanged, and the exact owned heavy lease was
released with known exit zero and no unresolved child.

A separately frozen V5 child controller establishes the started child identity
and output tracking before closing redirected stdin to EOF. The live-child
close-failure regression and independent peer review passed. The unchanged SBOM
and release-audit scripts then both returned zero on the same clean source and
unchanged release executable. This is **14 original PASS + two targeted PASS**,
not a rewritten claim that the original single run passed all sixteen gates.
The release binary SHA256 is
`44513F752CB946644C856B69D740189A045452682989C6E7FC955A59035D0E38`.
Library tests: **3729 passed / 0 failed / 10 ignored**. The producer and exact
verification receipts are preserved in [the raw checkpoint](evidence/current-checks-2026-10-02/checkpoint.json).

The D2 desired source already exists on F: root and peer independently read
all 12198 baseline and desired files, checked SHA256, lengths and ordinary path
ancestry, and found exactly the three measured template donor differences.
All module B bytes and ConfigDumpInfo remain preserved. A fresh read-only
certificate corrects the earlier incomplete *knowledge state*; historical
receipts remain immutable. No redundant copy, acquire or SQL/native write was
performed. This proves source readiness only; template activation remains open.

The producer above is historical. Integration now includes the later master
change for typical configuration export/import and partial files. Its initial
local clippy run refused the merged source; the exact owned heavy lease was
released with known exit zero. Missing compile seams, five lint warnings and
two portable-test fixtures were corrected. The current merged source still
requires its own complete CI/local gates and a newly bound producer; the older
release executable does not certify these later Rust changes.

The actual native D2 template run restored its owned database, then refused
private-cluster startup before registration or configuration writes. Exact
stop and worker release returned zero. The existing database and full preimage
are preserved. Independent review of the additive resume preparation found
two errors before execution: snapshot CLI paths and a state-absence check that
would reject its own successfully started cluster. A separate successor is
being tested against actual emitted paths and phase-specific ownership.

Compact8 refused before restore or native writes at the archive-node budget.
The distinct compact9 V2 preparation uses a streaming bounded archive census,
explicit startup expiry diagnostics and a caller deadline within the existing
maximum. Root and independent peer verified its 147-file closure. The historical
producer is retained explicitly for this scoped laboratory experiment; no LIVE
continuation, second-cycle or load acceptance is claimed before actual results.

Native Params empty-stage apply returned zero but left all six storage tables
and ten auxiliary tables exactly unchanged; the expected settled-shape guard
then refused. The subsequent B/C/D resume stopped before SQL capture or native
commands because uppercase snapshot labels violated the existing whitelist.
A separate successor uses lowercase evidence labels while preserving fixture
paths, the same owned database and all prior receipts. Its full marker matrix,
OWN parity and production rule remain open.

Nested forms, larger stages, platform 8.5, repeated generation changes, full
LIVE load, general WORKER ownership and guarded undo remain open. All eight
milestone issues stay open; no issue closure or release tag follows from this
checkpoint.
