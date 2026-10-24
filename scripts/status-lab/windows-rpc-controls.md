# Windows public fake RPC containment: CI activation

Base: local `005a4ffa6f2c0d985c5529d82f61d061c9e12c7b`. The earlier public SQL checkpoint is `2aff32c7`. This continuation adds a
scoped hosted Windows workflow. Windows runtime results remain pending; no
product execution or SQL acceptance is claimed.

## Exact gate

`windows_rpc_gate.py` admits Windows only, no CLI arguments, exactly seven named
tests and zero skips. It emits an exclusive-created receipt with exact source
hashes before/after and semantic candidate/API bindings. This receipt attests a
fake process contract, not a product binary build. The base must be freshly
reserved by the operator/CI, empty, and match STATUS_LAB_TEST_ROOT; test fixtures
use the existing single-use ownership capabilities. Every fixture is retained.

The activated workflow is `.github/workflows/public-fake-rpc-windows.yml`.
It runs only for scoped push/PR events, with read-only contents permission and
pinned checkout/upload action revisions. There are no dispatch inputs, secrets,
product builds, package installs or runtime downloads. Python must already be
installed in the hosted Windows tool cache. No arbitrary command or shell text
is supplied through event fields. The eight-minute job limit is an outer
fail-safe, not a passing cleanup result. Existing CI gates are unchanged.

Before Python executes the fixed entry, PowerShell checks the literal manifest
SHA-256 and every executed module against `windows-source-sha256.json`. Python
rechecks those sources plus manifest and interpreter identity before/after the
suite. Per-probe seals additionally bind settings, source/reference fixture
inventories, actual interpreter and exact API originals. The candidate/API
labels remain semantic contract identities, not an actual product binary claim.

Only newly owned public fake fixtures are used. Test log bytes cap at 64 KiB;
artifact admission refuses >4 MiB total, >1 MiB/file, >128 files, >256 entries,
hardlinks, symlinks and junctions. Upload is conditional on a successful bounded
artifact admission marker even when tests fail. Artifact retention is seven
days. No local lab fixture is deleted or expires through this workflow.

## Discriminating controls

- Portable model: EOF must wait for normal peer exit before Job Object close.
  The `005a4ffa` order closes the job first, allowing kill-on-close to race a
  normally exiting interpreter. The new wait is <=grace; expiry stays failure.
- Assignment refusal reaps the peer and closes the job before bootstrap. Stale
  code seal refuses before spawning. These models do not prove Windows behavior.
- Real Windows: assert IsProcessInJob and current active membership=1 before the
  bootstrap write. Inspect exact isolated Python argv, finite case and minimal
  environment. The peer cannot spawn descendants before bootstrap admission.
- Real Windows descendant: open a SYNCHRONIZE handle while the ready child is
  alive, verify timeout before teardown and signal on the SAME retained handle
  after job close. Missing PID or skipped tests cannot stand in for termination.
  Explicit stdout/stderr arguments make the reviewed descendant retain BOTH
  pipes on Windows too (default Windows Popen inheritance is insufficient).
  Only the owned job is closed; no broad process search, PID kill or taskkill.
- Hang, stdout/stderr flood, source drift and seal drift must fail, with reader
  and writer joins confirmed and no live/product claim or raw path in report.
  The existing finite fake peer supplies all cases; no arbitrary executable or
  injection surface is added.

The actual Windows gate has not run at activation review. macOS model results or the earlier 69-test
POSIX gate cannot establish Job Object layout, assignment, Python DLL loading,
job nesting on hosted runners or Windows termination behavior. A failing
Windows run must retain its evidence and stop; do not retry or relax containment
until the specific failure has been reviewed. This proposal does not authorize
any actual ibcmd/SQL operation.


## Local evidence (POSIX, not Windows)

Fresh static review first found the missing explicit Windows descendant pipes;
that fixture was corrected and re-reviewed BEFORE any new execution. Final
source review: APPROVE. No Windows kernel test has run.

The order model run against the old `005a4ffa` ProcessScope failed exactly its
one expected assertion. On the revised source, **10 targeted tests passed**
(1.837 s recorded by unittest). Then the same-hash
complete suite passed: **76 run, 73 passed, 3 explicit Windows-only skips**
(2.371 s recorded by unittest). No additional skips, failures or errors; no ResourceWarning
in the retained logs. All fake fixtures remain retained. No compiler, host,
SQL/product process or heavy slot was used.

| Reviewed runtime source | SHA-256 |
| --- | --- |
| process_scope.py | e315c9c93dbb77efecc2da940d0aaadf8acceddd2828ce708628bac65e4c8354 |
| fake_rpc_peer.py | 6c3f68a78d2cde57cc58188be54cb829fe0258f3b0c4833ace21a77388f298ed |
| test_windows_rpc_probe.py | 6448739f7366a19529446307f0823b0f569889b2560a0ef117e6d50893786d74 |
| windows_rpc_gate.py | 07dd09069aae3ff351fd0b074a2afc0becdc2bfae991880be04be9198a2f6b73 |

External retained logs:

- `logs/issue-380-windows-model-e315c9c9-targeted.log`
- `logs/issue-380-windows-model-e315c9c9-full.log`

The seven-test Windows-only entry point, actual Job Object queries, same-handle
descendant wait and hosted Windows workflow remain unexecuted. The source-reviewed workflow activation is a subsequent checkpoint; these
local results do not establish any Windows execution result.


## Activation checkpoint local evidence

The final workflow/source-manifest/artifact-admission diff received a fresh
independent APPROVE before execution. Its first review found that an upload
marker could precede final admission. The fix writes a pending marker, validates
its bytes/entry within the same cap, and only then atomically renames it to the
upload marker. Both success and final-refusal regressions passed.

On unchanged reviewed sources: **12 targeted pure/model tests passed (0.016 s)**,
then the complete suite: **84 run, 81 passed, 3 explicit Windows kernel skips
(2.412 s)**. These are POSIX results only. Windows fixed-seven/zero-skip results
remain pending the automatic workflow. No manual dispatch is used.

Manifest SHA-256:
`b301f67a27a1784a82749b66d44bdba2e2196cc75c8e6b9ce43bce107476ae2d`.
External retained logs:

- `logs/issue-380-windows-activation-b301f67a-targeted.log`
- `logs/issue-380-windows-activation-b301f67a-full.log`


## First activation failures and checkout correction

On `6fa969bf`, neither automatic Windows job reached the seven tests. Fork
job `120850121678` failed because Get-Command returned multiple application
matches (hosted Python plus a WindowsApps alias) and Source was passed as an
array. The correction filters the bounded PATH-priority candidate list to the
approved hosted tool cache and executes one scalar path. This does not claim
that exactly one Python installation exists.

Upstream job `120850130242` stopped earlier at manifest binding. Its merge
commit and the fork head contain the identical manifest Git blob
`f7e1ee95b08a8b9d5613389bad09af09477ab567`; the old failure log does not print
the mismatching checkout hash, so the precise byte difference is not attested.
The correction declares LF for only the status-lab sources and three pinned
Rust API files at checkout, avoiding a post-checkout conversion assumption. The
strict expected hash is unchanged; new workflow output records the measured
public manifest SHA-256 and byte length before comparison.

Retained local Git 2.52 diagnostics show core.autocrlf=true creates CRLF without
an explicit LF attribute. The old restore step DID repair it on this Mac, so
these diagnostics do not reproduce the exact Git-for-Windows failure. New
positive/negative checkout tests discriminate the declarative LF rule itself;
they are public synthetic Git controls, not Windows kernel execution evidence.
The old failures remain recorded; no manual retry/dispatch is used.


Checkout correction local evidence: **two targeted Git controls passed (0.048 s)**,
then unchanged reviewed source hashes passed the full suite: **86 run, 83 passed,
three explicit Windows kernel skips (2.499 s)**. External retained logs:
`logs/issue-380-checkout-469f2805-targeted.log` and
`logs/issue-380-checkout-469f2805-full.log`. The manifest remains `b301f67a`.
These results validate the LF contract under this local Git, not the unexecuted
corrected Windows workflow. No product/SQL/native/VM/host/Rust process ran.


## Real Windows artifact metadata and review follow-up

On `6e2a834e`, fork run `40087101511` / job `120852166426` printed the correct
manifest SHA-256 (`b301f67a`, 887 bytes), invoked the test suite, then failed
artifact admission. The seven outcomes were not uploaded, so no 7/7 result is
claimed. [Python 3.12 documents](https://docs.python.org/3.12/library/os.html#os.DirEntry.stat)
that Windows DirEntry.stat returns zero link/device/inode fields; os.stat must
be used for those values. Admission now queries no-follow os.stat and retains
st_nlink==1, regular-file, reparse-point, count and byte limits. A simulated
zero-link DirEntry and a real retained hardlink are separate counterexamples.

Fixed public test labels/outcomes are emitted before artifact admission, without
assertion text, exception messages, frames, stderr or paths. Failed/error/skipped
subtests mark their owning control and cannot be printed as pass. The gate now
requires all seven exact named outcomes to be pass as well as the existing
aggregate count, zero-skip, source/seal and process checks.

CodeRabbit review `5637960036` was COMMENTED, not APPROVED. Its three confirmed
shape/path findings are addressed together: malformed peer error objects use
the defined ValueError refusal; malformed root/nested/deep transcript JSON uses
the bounded CLI refusal path; fixture spelling is resolved before comparison,
while absolute/confined/empty/new ownership and symlink/junction guards remain.
The Rust recording refusal assertion now checks the injected SQL failure chain;
no local Cargo run is included in this source-only follow-up. Existing decoder
malformation tests are retained; the new shape matrix is not duplicated there.

Current follow-up manifest SHA-256:
`9a1439ae693487583eddf2835b39d77ff3c8130a8be907ab4fe2b4d28dccbbb9`.
Actual corrected Windows outcomes remain pending; prior failed logs are retained.


Follow-up local validation: three old malformed-shape AttributeErrors reproduced
against published `6e2a834e` functions in memory. The revised **20 targeted pure
controls passed (0.017 s)**, then the same reviewed sources passed the complete
suite: **98 run, 95 passed, three explicit Windows kernel skips (2.505 s)**.
External logs: `logs/issue-380-review-9a1439ae-targeted.log` and
`logs/issue-380-review-9a1439ae-full.log`. All fixtures retained. No local Cargo
or corrected Windows execution is included in these counts; the stronger Rust
assertion awaits the ordinary CI run.


## Path/descriptor native witness follow-up

Exact head `5cf8f5f9` reached artifact upload successfully. Fork Windows run
`40087849695` / job `120854314127` ran seven tests without skips: three model
controls passed, four named controls errored while preparing seals. The retained
artifact log places every error at the path-stat/descriptor-fstat identity
comparison, before fake process/Job Object execution. No kernel pass is claimed.

The artifact ZIP was retained externally and verified against GitHub's archive
SHA-256 `97ac73a699827f56e7dc87c166b58702c491f4e8b07795c5215c964e13f3598e`.
Receipt interpreter SHA-256:
`4d6f5f81a4bca11191c4c7c6b43632694d0a4ce74e068619d8fdc161d469859a`.
The log does not record which stat field differed, so no particular field is
asserted to be the cause.

The correction obtains both path and descriptor witnesses with the same native
read-only metadata API family: [FILE_ID_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info),
[FILE_BASIC_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_basic_info)
and [FILE_STANDARD_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_standard_info).
It preserves volume and full 128-bit file ID, byte size, write/change/creation
times and link count. It ignores access time, which ordinary reads can change.
Reparse/directory/deletion-pending/unsupported metadata refuse; owned metadata
handles close confirmed, and the live Python descriptor handle remains borrowed.
No mismatching field is dropped and no byte/hash/size/unique-link guard is relaxed.
POSIX retains its previous stat/fstat witness. This is fake-harness input sealing,
not a production filedb/SQL adapter or actual product status implementation.


Native witness follow-up local evidence: **eight pure ABI/custody/refusal models
passed (0.001 s)**; then the unchanged reviewed runtime hashes passed the full
suite: **106 run, 103 passed, three explicit Windows kernel skips (2.351 s)**.
No Windows API was called locally: model tests inject a fake kernel, and POSIX
keeps its previous stat/fstat path. This is not corrected Windows proof.
Manifest SHA-256:
`1e2350906d046e69f14ce6f73c2070900077a7930f6b23842d6159656511f80a`.
Retained external logs: `logs/issue-380-native-witness-1e235090-targeted.log`
and `logs/issue-380-native-witness-1e235090-full.log`.


A bounded diagnostic in the corrected gate compares legacy stat/fstat on the
one fixed, manifest-verified gate source. It emits only differing names from a
seven-field allowlist; no IDs, timestamps, values or paths. A pure control checks
that diagnostic projection. Legacy values do not decide admission: the genuine
native path/descriptor witness remains authoritative. The exact old differing
field stays unclaimed until this actual Windows diagnostic runs.


Final source including the requested label-only diagnostic: **nine pure targeted
controls passed (0.002 s)**, then the same reviewed hashes passed the full suite:
**107 run, 104 passed, three explicit Windows kernel skips (2.380 s)**.
Final manifest SHA-256:
`380397ccf73b9503d4903352acee50ceb9ebf117e1fba58ec3793ae18044047e`.
Retained external logs: `logs/issue-380-native-witness-380397cc-targeted.log`
and `logs/issue-380-native-witness-380397cc-full.log`. These supersede the local
8/106 checkpoint for the additional diagnostic source; neither establishes an
actual corrected Windows outcome or live SQL acceptance.
