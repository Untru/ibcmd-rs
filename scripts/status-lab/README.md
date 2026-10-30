# SQL status contract lab (#380)

This directory prepares a **public, bounded, sole-operator** SQL status smoke.
It never performs stage/apply, opens SQL connections, runs native tools or
changes consent settings. `verify_capture.py` only checks a collected sanitized
transcript. Its success always says `liveAcceptance: false`: a reviewer must also
check actual platform output, request captures and storage/source provenance.

Portable verifier checks:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/status-lab -p 'test_*.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 scripts/status-lab/verify_capture.py /external/owned-run/sanitized-transcript.json
```

`client_contract.cjs` executes the current pure `object-status.ts` adapter in
memory. Point `STATUS_LAB_TYPESCRIPT` at an already installed absolute TypeScript
module (for example the extension's `node_modules/typescript/lib/typescript.js`),
then run `NODE_DISABLE_COMPILE_CACHE=1 node --test scripts/status-lab/client_contract.cjs`.
It installs nothing and writes only new synthetic byte fixtures under TMPDIR,
always retained. It tests all six labels, including equal-current-bytes `both`,
unchanged historical reference and conservative unknowns. It does not instantiate
ObjectView, a real RPC transport or an extension host.

The synthetic `transcript()` factory in `test_verify_capture.py` is the schema
example. Its hashes, operator, lease and timestamps are invented public controls,
not evidence of an executed operation. Capture metadata (`generation`, phase,
lease, hashes and disposition) surrounds the actual status request/response; it
is not a proposed RPC wire-field extension. `requestBindings` maps the original
infobase/source/reference to stable sanitized logical labels; the actual request's
three parameter values are replaced consistently by those labels. Each capture
must preserve that parameter map and its main-view tree request. Raw local
captures must independently prove that the labels/digests map to the exact inputs.
This verifier covers native opt-in main status; active byte-only captures remain
separate attachments for operator review. Use real recorded values for a smoke.

A transcript has one public fixture digest, exact product commit plus embedded
commit/binary digest, a one-hour maximum exclusive disposable-DB lease, opaque
hashed target/source identities, and exactly two external consent receipts (stage
and apply). Each receipt binds the owner's exact preview, source, target, product
and lease. It does not itself perform or authorize a writer. The status operator
must obtain actual human/owner authorization separately, never generate a consent
receipt from a status result. A claimed boolean is evidence metadata, not an
access-control mechanism or a substitute for the supported writer's own checks.

There must be one published `before`, `staged` and `applied` capture plus actual
cancel and retired-generation controls. Maximum 24 captures, 64 objects each,
300 seconds per read interval and 1 MiB per transcript. Within each read interval,
source and storage fingerprints stay unchanged. Across intentional stage/apply
writes, storage may change; source identity stays fixed. Reference identity,
object/cache token and database snapshot must bind every native evidence row.
Unknown/full-required/error/cancellation cannot be submitted as conclusive clean.
The verifier does not derive labels or compare an invented native oracle.

Future operator sequence and explicit unproved boundaries:
[SQL acceptance](../../openspec/changes/add-vscode-object-status/sql-acceptance.md).
The isolated Rust suite is `tests/editor_sql_status_contract.rs`; run it only
with a planner grant and an external target/TMPDIR. Locally always set
`IBCMD_TEST_KEEP_FIXTURES=1`; owned synthetic fixtures remain for Trash-only
cleanup. CI may clean only its freshly owned disposable fixtures by default.
No script removes artifacts, drops databases, or alters originals.

No live SQL, VM, TDS, authentication, native platform or editor-host acceptance
is established by these portable controls. Existing merged #484 evidence remains
separate. No issue closure or full CI is implied.


## Local-only first operator package

`readonly_probe.py` prints a **synthetic**, non-executed JSON plan for the pinned
published candidate `2aff32c79d0b9fbe45d513693b27e52fc47457de`. It neither spawns
`serve` nor connects to a database. `executed=false`, `syntheticExample=true` and
`liveAcceptance=false` must remain visible when sharing its output. Its sample
hashes are intentionally fake; they must never be used as an actual build receipt.

First operator action is a metadata/lease review, before starting any child:
confirm one already owned disposable public-tiny SQL target, no concurrent
operator, exact source/reference digests, CONFIG/CONFIGSAVE fingerprints and
pending-stage markers, private settings identity, and an immutable clean build
receipt for this exact binary. Current CLI `--version` reports package version;
it does not attest git SHA. The separate receipt must bind source/tree/lockfile,
Rust 1.95.0, reviewed build command and binary hash. No build or operator execution
is granted by this plan. Missing actual receipt/metadata means STOP.

After that separate operator grant, substitute lease-bound private values into
exactly seven read-only JSON-RPC requests: initialize 1.4, active/main tree,
active status without a baseline, main status with the reference, a cancelled
native status, and a retired-generation native status. Resolve the exact target
from initialize against the private settings/lease; never pick the first entry,
expand a wildcard, or discover unrelated databases. Never put passwords in
frames, argv or public receipts. Require matching 1.4 handshake and capabilities.

The proposed collector budget is 180 seconds wall, 20 seconds/request, seven
requests, 1 MiB stdout, 16 KiB stderr, 8 KiB/frame header and 64 notifications.
On error/cap/timeout: no success publication, close input and reap only the owned
child with bounded terminate/kill handling; no broad process kill, automatic
retry or artifact purge. Pure decoder/error controls are tested, but **this plan module has
no actual process runner**: the separate fake-only runner below supplies synthetic process-control evidence, not actual-product evidence.
The public error projection drops raw message/data/stderr; retain raw local
receipts privately and never publish them through an error path.

The added-object transition helper verifies only independently recorded known
inventories: active remains old during stage, main gains exactly the expected
object, apply preserves main and brings active to it. Incomplete recording falls
back to active. It is not an execution of T-SQL, a staged-added-object adapter
fixture, or physical SQL stage/apply evidence. Existing Rust refusal controls
cover missing versions, commit/new markers and rename/delete fullRequired;
unknown-role coverage remains the conservative client boundary.

The first read-only probe needs no stage/apply consent because it performs no
write; future mutation is owned by SQL primary with a separate reviewed preview,
lease and explicit consent. The full `verify_capture.py` stage/apply schema still
expects its earlier embeddedCommit field. Do not fabricate that value from
`--version`: a separately reviewed build-receipt schema integration is required
before using it for an actual current-binary full transcript. This package and
its synthetic output do not silently upgrade that schema or prove live SQL.


## Bounded fake RPC process runner (local continuation)

`rpc_probe.py` now executes only the reviewed `fake_rpc_peer.py` snapshot under
the current sealed interpreter (`-I -S -B`), with a finite case label and exactly
seven read-only requests. It refuses actual ibcmd adapters, arbitrary commands,
unowned fixture paths, stale/missing seals, unknown protocol/schema/commit,
input/receipt/code identity drift, unexpected trailing stdout and process errors.
No shell or inherited auth/environment is admitted. Input and script snapshots
are retained, never deleted.

Before any execution, review the exact source hashes and
[rpc-runner-contract.md](rpc-runner-contract.md). Allocate only through the
minted fixture capability under this selected external lab's
`tmp/issue-380-rpc-runner`, and set STATUS_LAB_TEST_ROOT to that exact base.
Create the base as an owned external directory; never adopt an existing user
fixture. The gate command is then:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/status-lab -p 'test_*.py' -v
```

The current reviewed suite passed 69 tests on macOS/POSIX, including 21 runner
controls. Targeted cleanup controls passed first. Natural exit is reaped without
unnecessary empty-group signals; a live admitted group receives TERM/KILL while
its inherited pipes remain open. The descendant publishes readiness only after
installing SIGTERM-ignore, so escalation is exercised. No EPERM is suppressed as
success. All public reports discard raw frames/stderr/diagnostics and never
publish a UI status; cancelled/stale replies are not successful clean results.

[Runtime evidence](rpc-runner-evidence.md) lists exact hashes and retained logs,
including the initial red cleanup gate and safe diagnostic. This proves a
synthetic process/transport contract only. Windows Job Object code has static
review but no Windows execution evidence; actual ibcmd/TDS/SQL/VM/native/editor
host remain unexecuted. It does not duplicate the SQL primary's database snapshot
or profile capture and does not authorize stage/apply or the future operator run.


## Windows fake-process CI gate

[windows-rpc-controls.md](windows-rpc-controls.md) specifies the seven-test
Windows gate, four portable models and three real-kernel controls. The fixed
`windows_rpc_gate.py` requires Windows, exactly that inventory and zero skips.
The workflow `.github/workflows/public-fake-rpc-windows.yml` schedules an
ordinary hosted Windows job on scoped push/PR events. Windows results are pending. It retains only
new public fake fixtures, requires no product binary/SQL/native/VM/secrets, and
cannot turn POSIX results into Windows evidence. It checks the committed source manifest before execution and permits artifact
upload only after a <=4 MiB/128-file/256-entry inventory check. Logs cap at
64 KiB; artifact retention is seven days. No manual dispatch or runtime/package
download is used. Existing CI gates are unchanged.
