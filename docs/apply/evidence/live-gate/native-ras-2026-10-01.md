# Native staged/RAS checkpoint, 2026-10-01

Scoped research on SQL Server **17.0.1135.8**, service cluster **8.3.27.2214**,
with newly restored, Track live BSP clones only. No cluster settings changed.
Raw evidence is retained at `F:/ibcmd/lab/05/wave2/live`; ownership and current
remaining work are in its `STATUS.md`. This is not full F-5 or zero-error acceptance.

## Fixture boundary

The corpus contains native dynamic history. Native partial import of
`CommonModules/ОбсужденияСлужебныйКлиентСервер/Ext/Module.bsl`, followed by
`config apply --force --dynamic=disable`, removes Config aliases/marker but
retains an 82-byte `Params.DynamicallyUpdated`. The supported checkpoint still
refuses that history. The proof fixture explicitly removes this captured residual
marker after checking no RAS/SQL user sessions; it does not claim unchanged native BSP.

`proof-import-baseline.command.json` and `proof-force-baseline.command.json`
preserve exact executable/flags. `proof-normalize.sql`/`.log` retain the marker
bytes, conditional exactly-one deletion, and equal fingerprints of all 9841
ordinary Config rows. That fingerprint includes filename, part, DataSize,
binary length and SHA, **but not Creation/Modified/Attributes**. It is a partial
header fingerprint; do not retroactively interpret it as full-header parity.
The separate exploratory clone's first normalization lacked raw-marker and
whole-Config fingerprint preservation and is excluded from this audited proof.

A **separate later control on that existing exploratory clone** establishes the
full-header deletion boundary; it does not upgrade the earlier proof fixture.
All own observers were closed, their exact journal/IB-bound hibernating UUIDs
terminated, and the infobase unregistered. Native partial import plus
`--force --dynamic=force`, then immediate force/disable, still left four genuine
Config history rows. No marker was deleted in that state. One additional partial
tag plus force/disable settled Config. Every native command held its own FIFO
ticket; exact arguments are in `explore-*.command.json`.

`explore-fullheaders-normalize.sql`/`.log` then require empty SQL users, no
Config/Params triggers, ConfigSave empty and no Config markers/aliases. Under a
serializable transaction, the captured 82-byte Params marker is conditionally
deleted using all its headers and bytes; exactly one row must match. The full
ordered Config row lists include FileName, PartNo, Creation, Modified, Attributes,
DataSize, BinaryData length and SHA. All 9841 rows are identical before/after,
fingerprint `7D6E9C98BF059366862DA5ACC60E6F062E4F31092D431A6042014A1B68D2DE85`.
The other 37 Params rows have unchanged full fingerprints too. Raw marker dates
are 4026 (platform storage epoch), attributes zero; bytes and binary date headers
are retained. This is explicit fixture normalization, not permission to ignore
genuine native history in production.

## Measured checkpoint and refusal

The real native sparse stage contains five complete rows including `versions`,
with no `commit`/`.new` rows (`proof-stage-idle2.log`). A fresh iter build from the
wave2 source is required; binary SHA/source provenance is saved separately.

| Case | Observed result / raw files |
|---|---|
| Production renderer token | BASE dry-run refused before DB writes because uppercase report SHA differed from lowercase validation (`proof-idle2-dry.stderr`). Digest comparison now accepts either hex spelling, retains backup names, and refuses wrong/nonhex/short tokens. |
| Empty RAS / two idle 1C SQL handles | Real promotion/cycle 1 and automatic cycle 2 complete (`proof-idle2-execute.json`); new COM session reads `LIVE-idle2`. |
| Repeated continuation | Two `already_complete`, `cycle_2_executed=false` reports; tail SHA unchanged; actual HEADERONLY contains the two owned sets (`proof-idle2-noop*.json`, `proof-idle2-header.log`, `proof-idle2-assertions.log`). |
| Own observer with real open SQL transaction | F-10 `57238` before script/recovery/tail publication. Config and ConfigSave snapshots unchanged (`explore-active-{before,after}.log`, `.stderr`, `-assertions.log`). |
| Warm polling user | Promotion/cycle 1 commit, one owned log set, ConfigSave empty. Cycle 2 is retained. Initial native RAS info encounters an OLE DB reconnect error; later strict UTF-8 inventory fails on OEM username bytes (`proof-warm1-execute.json`, `-refusal-retry.stderr`, `-ras-raw.stdout.bin`). |
| Fixed inventory reader | Nonempty OEM inventory emits the intended warm-session refusal, with byte-identical tail (`proof-warm1-fixed-refusal*`). |
| Client process closed | Exact recorded session remains hibernating; inventory still refuses. Only the journal/infobase-bound owned UUID is terminated (`proof-warm1-orphan-ras.txt`, `-terminate.command.json`, `-terminated-ras.txt`). |
| Resume after ending that session | `complete`, then byte-identical `already_complete`. First fresh COM attempt reports a terminated-admin session; the next succeeds with `LIVE-warm1` (`proof-warm1-closed-continue.json`, `-noop.json`, `-new-marker*.log`). Both outcomes are evidence; success does not erase the first error. |

The warm1 journal continues reading the old client/server marker while cycle 2 is
pending. Shorter later cases retain their pre-switch client/server markers; they
do not establish client-code refresh in the same session.
The production RAS deadline remains five seconds. Windows subprocess protocol
tests allow 20 seconds for pwsh startup; the deadline/kill regression remains bounded.

## Five bounded warm operations

Warm1 uses the token-fixed build for promotion and the reader-fixed build for its
measured refusal/resume. Warm2–5 use the freshly built immutable source
`05a96538e155aa5c474224c9be5a59a669805b23`, binary SHA256
`196061DC64295D462E07D9872EF1BED75C0FBA397CB87D1F386C9230CC831122`.
These are sequential functional operations on the same normalized owned fixture,
not five independent corpus restores or multiuser/load acceptance.

| Operation | Boundary and subsequent result |
|---|---|
| warm1 | Raw OEM warm refusal; hibernating session still refused; owned cleanup then complete/noop. First fresh COM error retained, second new marker. |
| warm2 | Automatic finish reports the intended warm refusal. Standalone harness used an invalid CLI flag: retained parser error, **not a runtime refusal**. Exact owned cleanup then complete/noop, new marker on first attempt. |
| warm3 | Automatic and explicit warm refusal; tail unchanged; exact owned cleanup then complete/noop, new marker on first attempt. |
| warm4 | Same bounded refusal/cleanup/resume/noop checks; new marker on first attempt. |
| warm5 | Same bounded refusal/cleanup/resume/noop checks; new marker on first attempt. |

Every completed operation has exactly two owned native HEADERONLY sets. Immediate
already_complete retries before staging the next tag passed the harness's byte
equality assertions. Warm2–5 did **not** retain the historical before/after SHA
values or separate tail copies, so that historical equality cannot be independently
reconstructed from the saved artifacts. Final
read-only headers and SHA comparisons are `proof-warmN-final-header.log` and
`-final-verification.json`; per-case commands, reports, assertions and exact
session/PID/journal binding are retained alongside `warm_repeat.ps1`. That harness
asserts success rows, exact two backup names and marker text, not exit status alone.
Older artifacts are generation-fenced once a later operation changes Config;
they cannot be treated as still-applicable already_complete retries.

## Stale-generation refusal and master lock context

An old warm1 artifact against the later warm5 generation fails before cycle 2.
The first run exposed a diagnostic defect: CATCH still used the target database
after the generation guard, so releasing the master-owned app lock masked 57266
with SQL1223 (`proof-warm1-final-noop.stderr`). CATCH now returns to master before
release. The bounded actual regression (`stale_artifact.ps1`, `stale-artifact-*`)
reports **57266 published generation/rows changed**, no 1223, identical complete
Config/ConfigSave header/data snapshots and tail SHA, ONLINE/MULTI_USER, and
immediate successful reacquisition/release of the same master lock. No stale
cycle 2 was attempted. This new regression retains both tail SHA values and separate
full before/after SQL snapshots in `stale-artifact-assertions.json` and `-before/-after.log`.
Focused renderer tests cover validation and execution
branches; this native check supplies the SQL database-context behavior.

## Validation and remaining acceptance

Quick gates on the unchanged production version: fmt, policy guard, workspace
layer clippy and root library **3652 passed, 0 failed, 10 ignored**; all-targets
check passed. The final tests-only pwsh startup allowance is followed by focused
continuation tests and fmt. No full/release track gates were run.

After the measured CATCH-context correction, final quick gates pass again:
fmt, policy guard, workspace-layer clippy, root library **3653 passed, 0 failed,
10 ignored**, and all-targets check. Focused continuation tests are **18 passed,
0 failed, 1 ignored**. Logs are `quick-catch/summary.txt`, `catch-focused.log` and
`check-catch-all-targets.log`. The freshly built correction binary and source hash
are retained in `binary-catch.json`; no broad native matrix was repeated for this
refusal-only change.

L1 is measured on this explicitly normalized native fixture. L2 has real active
work/warm refusal and safe operator continuation evidence, five bounded functional
operations, and a separate full-header normalization control. Warm readiness that
permits connected users, load acceptance, same-session client refresh and zero
connection errors remain **OPEN**. Five repetitions alone will not establish those
properties. Source-to-promotion checkpoint support and broader SQL/platform builds
remain unsupported.
