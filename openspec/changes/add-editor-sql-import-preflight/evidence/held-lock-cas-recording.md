# Held-lock CAS recording checkpoint (#382)

## Local implementation and evidence

The library-only transaction adapter uses one dedicated session, acquires the
existing master-scoped LIVE guard, then holds CONFIG followed by CONFIGSAVE table
locks through the full identity/schema/header/part/payload comparison, selected
staging updates, post-write comparison and COMMIT. Unselected physical staging is
retained; selected metadata comes from the captured effective part-zero row.
The existing readonly capture still rolls back and uses its original seal.

Cancellation is checked after the last synchronous guard immediately before
COMMIT dispatch. Acknowledged COMMIT remains committed despite later cancellation;
a lost COMMIT acknowledgement is Unknown with no automatic retry. A rollback
acknowledgement and an uncertain/poisoned connection are separate outcomes.

Four population parameter values are summed with SqlParam::wire_bytes and checked
addition. This budget excludes SQL text and protocol framing; it is not a complete
TDS request-size bound. No native multipart/header writer profile is asserted.

Rust 1.95.0, one compiler job, external target/temp and retained fixtures:

- transaction::tests: 11 passed, zero failed/ignored; 1m29s compile, 0.01s tests.
  Includes exact parameter-budget minus-one/exact/plus-one, cancellation raised
  inside the final guard, XACT_STATE=-1, SPID=0, stale readsets, preserved
  neighbouring staging, rollback failure, commit-response loss and late cancel.
- editor_sql_import: 29 passed, zero failed, one real-SQL test ignored.
- production library check: exit zero, 16.65s; existing 97 warning headings.
- Independent read-only source review: CLEAR. No test or source review opened SQL.
- Legacy LIVE rendered format template was expanded and compared against the
  parent checkpoint: byte-identical. Canonical snapshot.sql remains unchanged.

Logs and hash receipt: logs/issue-382-held-cas-{mock,snapshot-regression,check}.log
and logs/issue-382-held-cas-receipt.json in the external lab workspace. Existing
failed-run artifacts and foreign package sources are retained.

## Next disposable-SQL gate

Only the existing sole operator executes VM/SQL work. First revalidate the exact
public disposable target GUID, server/family/fork/schema and certificate-valid
product TDS identity. Use the existing ordinary invocation path, not the retired
custom launcher. The next integration must prove a dedicated session keeps the
same positive SPID and one committable transaction while acquiring CONFIG then
CONFIGSAVE, comparing the complete captured readset and rolling back without
application DML. Confirm closed connection/locks and unchanged database afterward.

Before an application write, separately review an explicit disposable-fixture
integration invoking the snapshot-only CommonModule compiler and this adapter.
Prove stale full readsets produce zero application DML; successful staging keeps
all unselected CONFIGSAVE headers/parts byte-identical; selected multipart/header
encoding is accepted by native apply; native re-export matches the intended edit.
A transport-disconnected COMMIT is Unknown, never retried blindly. A busy or
uncertain earlier operation stops the next attempt. No working ERP/UT target is
eligible. This checkpoint does not dispatch that integration or invent a writer
profile from recording tests.

There is no CLI/RPC/MCP write-route caller. The public library stage function is
not technically mock-restricted; the executions reported here use recording
clients only. Form.xml remains unsupported; Form module support is the existing
Document-only cohort. Full #382 acceptance, confirmation routing, real SQL
atomicity and native stage/apply acceptance remain incomplete. The diagnostic
observer branch policy-registration failure remains open; no baseline or guard
was changed and this branch is not publication-ready.

## Operator-only held-capture test prepared

`transaction_tests.rs` now contains one ignored test:
`server::sql_import::transaction::tests::public_sql_held_capture_rolls_back_and_closes_dedicated_session`.
No production API changed. A Windows-only gate and explicit existing disposable
GUID/endpoint opt-in precede product TDS/TLS connection creation. The profile and
bounded initial snapshot must match the existing public 8-part/3313-byte fixture.
The existing pooled reader remains open while one owned dedicated connection
acquires the normal LIVE guard, begins a transaction and performs two held
captures. Both seals require TX depth 1, XACT_STATE 1 and the same positive SPID.
Both complete readsets equal the initial capture. One terminal rollback is
attempted even if a comparison fails; PASS requires acknowledgement.

After the dedicated callback returns, the pooled reader requires an idle session,
a distinct SPID, existing VIEW SERVER PERFORMANCE STATE permission and zero DMV
rows for the old dedicated SPID. It does not grant that permission. A fresh
bounded capture must still equal the entire initial physical readset. No temp
population, application DML, COMMIT or native call is in this ignored test.

Local Rust 1.95.0 test compilation passed in 40.64s: 11 recording tests passed and
this one live test remained ignored. Independent source re-review CLEAR after
adding the DMV visibility requirement. Existing 40 lib-test warnings unchanged;
no source-owned warning. Log: logs/issue-382-held-capture-compile.log, fixtures
retained. This is Mac compilation, not Windows execution or real SQL acceptance.

Once an actual Windows lib-test executable with checked source commit and SHA256
is available, the sole operator uses the existing ordinary SSH/PowerShell path:

```powershell
$env:IBCMD_SQL_IMPORT_PUBLIC_FIXTURE = '9000716B-B12F-45BE-BB5B-27FB88534F95'
# Endpoint and existing certificate configuration come from the current lab
# binding; do not substitute a guessed endpoint or certificate bypass.
& .\ibcmd_rs-lib-test.exe --ignored --exact server::sql_import::transaction::tests::public_sql_held_capture_rolls_back_and_closes_dedicated_session --nocapture --test-threads=1
```

Before dispatch, verify the exact hash/source binding, current identity/schema,
TLS, authorized integrated principal, idle window and absence of an uncertain
previous attempt. Require exactly one test passed; zero tests is not acceptance.
Raw failure details remain VM-local. No automatic retry or write-route enabling.

Windows artifact preparation is blocked: automatic approval review rejected the
proposed change that would upload the test executable and build metadata through
the existing Windows CI artifact step. The workflow edit did not execute; the
workflow is unchanged, with no push/dispatch or alternative artifact transport.
The explicit reason was missing trusted end-user authorization for that exact
external artifact egress/workflow side effect. A new specific authorization is
required before retrying that action. Windows Rust/MSVC availability on the lab VM
is not established; no toolchain was installed or new launcher created.

The artifact-upload blocker above is historical: a subsequent explicit approval
covers the exact existing fork workflow edit, push, Windows build and executable/
SHA256/source-commit artifact download for this disposable-lab test. The minimal
existing Portable (Windows) job addition packages only one Cargo-reported root
lib-test executable and its build receipt in a fresh RUNNER_TEMP subdirectory.
It does not execute ignored SQL tests, change permissions/secrets or upload
application XML/database data. PowerShell AST check and independent source review
passed before dispatch. This authorization does not establish runtime PASS or
make the diagnostic branch policy gate pass.

## First Windows CI result and checkout fix

CI run 40186940076 at 53ca54e17cff6cd280dad39098911ebefb4d03ca completed:
Linux and Legacy Windows passed, but Portable Windows root tests had 4263 passed,
6 failed and 17 ignored. The six new recording failures all stopped at
`canonical snapshot control anchor changed`; the upload steps did not run.
The original failed log is retained as issue-382-windows-ci-first-failure.log.

Git's `core.autocrlf=true cat-file --filters HEAD:src/server/sql_import/snapshot.sql`
reproduced the cause locally: 7101 bytes, 110 CRLF pairs, SHA256
50b80805d1942e51f9946a7e180bd2cdf864fd4133c1de93d513be684249c928,
and zero matches for the three strict newline-bearing control anchors.
A single scoped `.gitattributes` rule pins this embedded SQL file to `text eol=lf`.
The same checkout-filter control then returns exactly the canonical 6991 bytes,
zero CR bytes and source SHA256 e6d1ecbfe06cd2df605cf53066a425dd98058056a701b661786567c057056da6.
All original anchor counts are restored (1/1/1 and two table-lock anchors).
No parser, SQL, guard or test assertion changed. Independent source review CLEAR;
the same local CAS suite passes 11/11 with the one live test still ignored.
Actual corrected Windows CI/runtime remain separate pending observations.

## Measured server-permission invocation correction

Corrected checkout CI 40189346243 at 0746b58aff692381c97cca0d1f6a5ffebb3d29b6
passed all seven jobs. Windows root tests: 4269 passed, zero failed, 17 ignored.
Artifact 12598286457 contained only the executable and build receipt; source/tree,
Rust 1.95.0, x64 PE and executable SHA256 were checked. The retained first EXE is
0d7e39ec9340572cf196ddebb475d2600424383147b4314bc83db9d21e82e7fe.

The sole operator stopped at SOURCE_PREFLIGHT before launching that EXE. One
preflight connection closed; no held transaction, appDML or native call ran.
A bounded own-permission SELECT then measured NULL for
HAS_PERMS_BY_NAME(NULL,'SERVER',...), but 1 for the documented server-level
HAS_PERMS_BY_NAME(NULL,NULL,...). The existing caller was
WIN-LHDQGRLB1L1\Administrator, with sysadmin=1 and CONTROL SERVER=1; the permission
was recognized. This was an invalid function invocation, not a missing right.
No GRANT or other security change is needed or performed. Diagnostic FULL RELEASE
SHA256: bf9679f2ceb998b80778d27d0f280bf19288c11960d1983209ec13873a72a074.

The runtime closure now shares a canonical NULL,NULL query with the regression.
Visibility still must equal 1; invalid NULL and denied 0 fail distinctly, and all
idle/distinct-SPID/closed-session checks remain strict. Tests also refuse an
existing session, the reader's own SPID, and a nonpositive held SPID. No production
API changed. Local focused tests: 12 passed, zero failed, one live test ignored;
38.42s compilation, 0.02s execution. Independent source review CLEAR. The operator
owns the matching preflight correction. The old executable must not be used;
a fresh changed-head Windows artifact and actual held-capture runtime remain
required. Raw SQL and failure artifacts remain retained in the lab.

## First actual held runtime and bounded closure observation

The corrected permission-call EXE (head caeccbd4, artifact 12600246403, SHA256
ed8d63763de26fc6b052f3a597352f51a2eba93a02403c946077a6f8b040b09e)
actually ran on the public fixture: natural exit 101 after 4.443 seconds, zero
passed / one failed. It reached the final combined closure predicate. By the
pinned code's order, both held captures, TX1/XACT1/same positive SPID, full readset
comparison, acknowledged rollback and visibility=1 passed before that failure.
Individual closure scalars were not recorded, so neither a leak nor asynchronous
cleanup nor SPID reuse was established. External fingerprints remained equal;
no appDML occurred. FULL RELEASE SHA256:
7db210fede4b2ac79560c00aea71b5a782f8f65799e837b79c4451cc506df81b.
No blind retry or staging followed.

The next change is confined to the existing test module. It records the existing
pooled observer SPID and captures exactly one self row for the held session with
an exact eight-byte login_time representation. Closure observes that captured
SPID+birth incarnation; a new session reusing the number is reported separately.
This timestamp is not globally unique: a birth collision conservatively remains
Pending/failure. Session-incarnation disappearance is not an independent TCP
shutdown acknowledgement.

One fully drained observer query holds one pool lease. It returns only bounded
scalar observations, preserving positive/distinct/unchanged observer identity,
idle transaction state and visibility exactly 1 on every row. Invalid states fail
immediately; only captured-session presence permits another observation. The
server and client cap samples at 20, with at most 19 waits of 50ms. No WAITFOR
follows the final sample. Blocking SQL, network and destructor time are not
hard-bounded by that logical limit; the existing outer operator bound remains
necessary. No capture, transaction or DML is replayed. Exhaustion fails.

Safe flags and phase timings are retained even on observation failure; raw SPIDs,
birth bytes, query text and application payloads are not printed. PASS still
requires successful batch drainage, confirmed captured-session disappearance,
rollback acknowledgement and final complete snapshot equality. No production
connection drop order, public API, SQL write route or permission changed.

Independent source review CLEAR. Local Rust 1.95.0 focused tests: 15 passed, zero
failed, one live ignored; 1m10s compilation, 0.02s execution. The three added tests
cover pending-to-closed, persistent presence/limits, invalid states after Pending,
observer changes/frame gaps, exact birth width, reused SPID and conservative
collision refusal. Logs: issue-382-session-closure-observation.log, retained.
A fresh Windows artifact and actual runtime remain required; this is not PASS
of the earlier failed run and does not establish its unrecorded cause.

Follow-up source review corrected the test observer ordering: count DMV session
rows first, then sample observer identity, transaction state and visibility
immediately before emitting the row. This prevents pre-query state from being
reported as post-query state; it does not explain the original unrecorded failure
and changes no session setting or acceptance criterion. A source-order regression
asserts count < sample < emission < wait. Independent source review CLEAR.
Focused Rust 1.95.0 local run: 16 passed, zero failed, one live ignored;
32.21s compilation and 0.02s execution. Retained log:
issue-382-closure-post-dmv-state.log. Actual Windows runtime remains pending.

## Measured SQL16 observer expression context and integrated candidate

Exact Windows candidate 6e11d42a passed CI 40200808727 (all seven jobs;
4274 root tests passed, 17 ignored) but its real public held test FAILED:
0 passed / 1 failed, natural exit 101 in 4.342s. Rollback was acknowledged,
the captured SPID was absent, observer identity was positive/distinct/unchanged,
and depth was zero; state-zero was false. The rejected row was not counted.
Fingerprints matched and FULL RELEASE was confirmed. Release SHA-256:
28c6f89e125d3f577079c80c341cf1328f1de389fb3df2edd77b2a5b0f8d5321.
No staging, application DML or native command ran.

A separate read-only, two-phase SQL16 expression probe measured actual integer
values through sys.sp_executesql. Both before and after an own-session DMV read,
combined SELECT and plain SELECT assignments gave depth=0/state=1, whereas
standalone SET assignments before/after gave depth=0/state=0. The implicit bit
was zero, permission was one and the observer remained the same. Therefore the
permission lookup is not necessary for the SELECT-expression effect. This
establishes a defective idle-state observation for this SQL/context; it does not
prove a product connection leak or retrospectively identify the first run's
unrecorded scalar. Exact types were 17 Int32 plus one Int64 per resultset; both
resultsets drained. Probe source SHA-256:
71e645e7b752dec4f8c091f56ffa11f0bd717605f0f80b8c49cf11e0d3d17c66.
Actual probe FULL RELEASE SHA-256:
f35782c9057fc61241e5a63867f0d8d668e2932a6ecb70ec5e7c02cd810cf3b6.
No transaction commands, application data reads, DML or settings reset ran.

The test now uses separate SET statements for permission, reader, depth and
state, then emits saved typed variables. Strict depth/state-zero, visibility,
identity, disappearance, successful drainage and fresh readset equality remain.
Both state=1 and state=-1 are explicitly refused. The query-form regression
requires separate SET sampling after the DMV and before emission. Independent
source review CLEAR; production TDS cleanup/settings remain unchanged.

Parallel reviewed donors were integrated with original provenance: c7d1a904
(one-shot registry), 42370984 and 7ea46d43 (real CommonModule/FormModule compiler
bridges). FileDB confirmations now have a fixed server-owned five-minute TTL,
remove-before-validation, epoch/target binding and the previous capacity/replacement
policy. SQL prepare/publication routes remain unwired; the existing legacy SQL
fallback is still an explicit blocker before a guarded route can be enabled.
Test-only factories retain the private typed Form fixture and give the recorder
ownership of framed physical headers. Form.xml remains refused.

Actual local Rust 1.95.0, jobs=1, external target/TMP, KEEP_FIXTURES=1:
registry 9 passed; transaction/real-compiler bridges 33 passed, one live ignored;
FileDB partial-import 14 passed; production lib-check passed. The orchestration
initially expected 13 FileDB tests and STOPPED despite exit zero: the imported
owned_scratch test was the fourteenth. That counter-only STOP/log is retained,
source-confirmed correction is separate, no successful tests were repeated,
and only the remaining lib-check was then run. Test/production warnings match
the existing 40/97 baselines. Retained directory:
logs/issue-382-proven-oracle-integrated-c8114da2b5ad44ad9664e5111a341053.

One further ignored public staging experiment is compiled into the same test
artifact to avoid another Windows build after held acceptance. It requires a
separate stage-confirm GUID before connection opening. It uses immutable literal
CommonModule text and the explicitly declared platform-8.3.27.1989 seed codec,
not a claimed 8.3.27.2342 writer profile. Outcome is printed before assertions;
Unknown/noncommitted outcomes stop without retry/native action. Confirmed commit
must match the exact expected physical readset, unchanged CONFIG and decoded
selected text. Its marker excludes target-platform admission, native apply and
nonempty prior-staging preservation. Source review CLEAR. The final affected
local filter passed 33 tests with TWO live tests ignored; compilation passed.
Log: logs/issue-382-final-set-and-stage-compile.log. Neither live SQL test was
executed locally or in CI. Actual staging still requires corrected held PASS
and a separate sole-operator target/experiment review; there is no user write route.

The unrelated diagnostic observer policy-registration blocker is unchanged.
No baseline growth, whitelist, relocation workaround, merge or release is claimed.


## Actual one-module held, staged and native acceptance (a666b1bb)

Exact source a666b1bbb57f06fc02ba885f22786bbc00afa4c5 passed Windows CI
40206960611, all seven jobs; root tests: 4300 passed, 18 ignored. Verified
Windows lib-test executable SHA-256:
f94ca6f7c1076a9a136b1eb4ee1ebdd5b679fa315e69bbba92ef36ffe423858c.
The two ignored tests were then executed separately by the sole authorized
operator against the same disposable public SQL fixture, SQL16/native
8.3.27.2342. Neither test ran against a working infobase.

Held capture: one passed, zero failed, natural exit zero, stderr empty, 4.241s.
Two captures had one positive SPID, TX1/XACT1 and identical full readsets;
rollback was acknowledged. Closure took one sample (2ms), the session was
absent, and a fresh capture was unchanged. Full release SHA-256:
fe54f2b0f6dcd7775e9784d0e939032c43330f14945da31ee501d7ab9f2f73b2.

Staging: one passed, zero failed, natural exit zero, stderr empty, 4.241s.
The report was Committed with confirmed cleanup. CONFIG stayed exactly eight
parts/3313 bytes, including all physical header and payload seals. Initially
empty CONFIGSAVE became exactly two parts/452 bytes: the selected existing
module body and versions. Full expected-readset equality and decoded module
text passed. Result SHA-256:
e393909391c439212aa30ba1c58ffb219c100eb6bbc515f85fe7e24346d0ea1d.
Full release SHA-256:
449c5d8d6108cc76dc16dea0112e47c3ceecf565928c2e3d09f5c80428a93a0e.

One native apply then one full native export completed with natural exit zero
in 1.732s and 1.624s, both with empty stderr. Independent SQL readback proved
active CONFIG body/versions matched the actual staged payloads; CONFIG became
eight parts/3327 bytes and CONFIGSAVE became empty. All unselected CONFIG rows,
including their headers, were identical. The native tree contained six files,
25148 bytes, no missing/extra paths. The selected native module was exactly
56 bytes, SHA-256:
fea4fcfe9697b678655b03d9a670313961e20ce8e11e32956c97cd35fb6daebc.
All four unselected XML files were byte-identical. The CDI retained the five
ordered metadata tuples; only the selected version attribute changed, matching
the actual staged version. No XML or raw SQL data left the VM. Native result
SHA-256: 968c243ce47461ac71060aedcfa89fe04b7e79f61b4014c80291ee173f8bee20.
Full release SHA-256:
a03015af6624de7b73c057b06af5e15ae13ef3af2d67e2e96f284601c7f6908c.
Both result/release pairs were independently rehashed. Workers were zero and
source/native pins unchanged; neither staging nor native apply was repeated.

This proves the complete selected CommonModule cycle on this exact public
fixture. It does not admit a general target writer profile, prove preservation
of initially nonempty neighbour staging, prove live conflict/cancellation
controls, or cover native FormModule acceptance. The editor SQL write route
remains closed. Recording and real-compiler bridge tests retain their separate
attribution; no policy baseline was enlarged.


## Next nonempty-stage/cancel/stale experiment (compiled, not executed)

A separate ignored operator test requires the same confirmed active public
fixture (CONFIG eight parts/3327 bytes), a distinct neighbour-confirm GUID opt-in,
and a native-prepared unselected CONFIGSAVE payload that actually differs from
CONFIG or adds an object. An unchanged overlay is not accepted. Existing fixed
identity/schema/TLS and stage opt-ins remain; no production API/route is added.

Two real selected CommonModule edits are prepared against the same immutable
snapshot. The first attempt refuses at the fourth, pre-COMMIT guard with typed
cancellation, requires acknowledged rollback and exact full original readset.
The next attempt commits the first edit and verifies full expected physical rows,
unchanged CONFIG and exact unselected staging headers/parts. The second original
plan must then fail at the held full-readset CAS, matching the exact comparison
failure derived from the confirmed postcommit state. Final rows must still equal
the first committed expected readset. Outcome markers precede assertions; an
uncertain/committed-without-PASS outcome never grants replay or native action.

Independent source review APPROVE after tightening the stale-error oracle.
Actual Rust1.95/jobs1/KEEP_FIXTURES=1 external-target transaction filter:
33 passed, zero failed, THREE SQL tests ignored (0.06s runtime). The new ignored
test is compiled but has not run on SQL or Windows. Native neighbour preparation,
Windows artifact verification and actual execution remain prerequisites.
Retained log: logs/issue-382-neighbour-acceptance-compile.log.


## Directory-census integration, separate from the SQL runtime candidate

The exact twelve-commit source-custody donor chain was integrated with original
cherry-pick provenance at 91ddc3cab175f0e6e0e2bd63db8066322da9e5d8, over the
separately frozen 9d04a5f1 SQL runtime candidate. The six resulting source/test
blobs match the independently reviewed donor receipt exactly; fourteen additional
paths are its OpenSpec documentation. No SQL, workflow, Cargo, policy baseline or
FileDB publication path was changed. Existing public HeldSourceRoot/inventory
signatures are preserved. SourceAdmission inherits the bounded anchored directory
census and rejects empty-directory membership changes during revalidation; it
does not claim an atomic filesystem freeze or defeat all ABA races.

Fresh integration/applicability review APPROVE. Actual Rust1.95/jobs1/external
retained target at 91dd: directory census 14 passed; Unix directory witnesses nine
passed; editor_partial_import 14 passed; editor_sql_import 29 passed, one live test
ignored; production library check passed. Root test/library warnings remained
40/97. Logs retained as issue-382-integrated-census.log,
issue-382-integrated-directory-witness.log, issue-382-census-filedb-compat.log,
issue-382-census-sql-admission.log and issue-382-census-libcheck.log under the
external logs directory. All processes exited. A following documentation-only
correction removes the obsolete SourceAdmission empty-directory disclaimer.

The 9d04a5f1 Windows/runtime evidence and earlier a666 one-module native success
retain their exact source attribution. This donor integration does not inherit
that SQL runtime acceptance, admit a writer profile, or enable an editor write
route. The unrelated diagnostic observer policy-registration blocker is unchanged;
no baseline growth, whitelist or relocation exception is introduced.


The unchanged normal physical-adapter policy guard was run after integration and
still refuses the existing unregistered diagnostic observer, file-path hash
5574a62490168322556d12d020d387ae765a939f0f12587067ea69e930825999.
This is retained as a full-branch readiness blocker, not a passing guard or a
new directory-census failure. Log SHA-256:
3a9d27cd753c39d45b59801fd3fbf8e6de6b4e6097c939c6df87e49bb1954ba1.
No baseline/scanner/whitelist was changed or rerun with an exception. Formatting,
diff checks and all four affected OpenSpec changes validate strictly.


## Actual nonempty-staging cancellation and stale-plan gate

Exact source `9d04a5f1a19924062b797dc852b24e7d13ab43e8` passed Windows
CI 40217548424. The sole operator ran only
`public_sql_nonempty_stage_cancel_and_stale_preserve_neighbors` from executable
SHA256 `7ed3569d51a3c19f15bfc066791db829723a568f255ec590c221247d6cc288c9`:
one passed, zero failed, natural exit zero, empty stderr, 8.581 seconds.
The artifact receipt declares the older held-test filter; the actual invocation
and runtime markers bind this newer exact test explicitly.

Native neighbour preparation had already succeeded once and produced five
CONFIGSAVE parts (1352 bytes): template metadata and body, root, version and
versions. The native-produced root/version payloads matched CONFIG with changed
physical headers. These are measured preparation effects, not additional SQL
mutation authority. The test-only continuation did not repeat native import or
apply. Cancellation rolled back to that entire baseline; selected CommonModule A
then committed its expected body/versions; a second plan from the stale baseline
rolled back at the held readset comparison before application DML. Independent
poststate checks confirmed unchanged CONFIG, all four unselected CONFIGSAVE
rows/headers exact, and all non-A version entries exact.

Result SHA256:
`5c51de8e9c4f9ee898b5434cfbc351908e2d471607843829d88e72d398d9741a`.
FULL RELEASE SHA256:
`7145bbbd5cd049c0035404c331bdf88ac4755600486fd236996b1b49b085ce4b`.
Both retained sanitized receipts were independently rehashed. Raw rows/logs stay
inside the VM. Final CONFIG remains eight parts/3327 bytes; CONFIGSAVE is now
six parts/1525 bytes, pending and not activated. This old target must not be
reused as a clean baseline or accidentally applied. This closes the demonstrated
CommonModule nonempty-neighbour/cancel/stale slice, not native FormModule,
general writer admission, lost-ack runtime injection or the guarded editor route.


## Narrow existing-CommonModule editor candidate

The candidate uses one existing `CommonModules/<name>/Ext/Module.bsl` selection;
objects, changed mode, other file roles, multiple files and disabled verification
refuse before source or SQL I/O. Retained source bytes and directory witnesses,
full CONFIG/CONFIGSAVE physical snapshot and a private observed storage token are
owned by the consumed target/epoch/TTL-bound plan. The compiler uses its existing
CommonModule grammar without manufacturing a native build coordinate. Selected
body and versions must remain single physical parts; multipart neighbours stay
untouched. An unchanged native-format module returns zero planned rows and no
transaction, after digest and cancellation checks.

One canonical read-only SQL prefix is shared by observation, initial capture and
held recapture. Its seal now includes the closed SQL16 storage semantics, current
effective sysadmin membership and hashed execution SID, plus the absence of a
Params dynamic marker under a third retained table lock. Config -> ConfigSave ->
Params is the deterministic lock order. Incoming/outgoing foreign keys, RLS,
triggers/checks (including disabled/opaque ones), special table/column features,
legacy bound defaults/rules and indexed schema-bound dependencies refuse. This
restricts supported existing principals; no rights are granted or changed. The
operation is text+versions staging only, not platform-build or native activation
authority. The old two-table schema receipts do not validate this extended seal.

The existing RPC stores either a FileDB plan or this private SQL plan. Confirmation
can supply only its retained token/source binding; it cannot replace selection or
disable verification. SQL Report Committed/Unknown takes precedence over late
cancellation and recovery errors; no fallback, retry or native apply occurs.
Main-view caches already recheck effective versions before use.

Actual local gates: 73 SQL-import library tests passed, three live tests ignored;
six RPC outcome tests passed; 30 source-admission tests passed, one live test
ignored; all 14 editor_server and 14 editor_partial_import tests passed. The
black-box SQL refusal test covers ten publication and thirteen prepare refusals,
with zero accepted TCP connections and no source creation. The no-op preview
regression first failed with `SQL replacement stage is empty`, then passed with
wrong-snapshot and typed-cancellation controls. Production library check passed;
root test/library warning totals remain 40/97. Fresh source reviews found no
remaining scoped adapter, observer or physical-plan blocker. Logs are retained
under external `logs/issue-382-operation-plan-*` and
`logs/issue-382-noop-preview-red.log`.

This is local candidate evidence. Current-head Windows/runtime RPC preparation,
publication and native apply/export are still required. Earlier a666/9d04 runtime
receipts retain their exact attribution. FormModule remains outside this editor
route and its native fixture acceptance is tracked separately. The unrelated
full-branch diagnostic-file registration policy refusal remains unresolved; no
baseline growth or whitelist was introduced.


## Native RCSI database default

The first production RPC attempt on the fresh public CommonModule baseline
completed initialization, connection and pending-state checks, then refused
preparation with the unsupported database-profile error. Publication was never
dispatched; full Config/ConfigSave seals were unchanged and the product exited
naturally. Retained metadata identified RCSI=true after native apply, while the
pure verifier unnecessarily required false. No database option was changed.

Both boolean defaults are now admitted. This does not remove a locking guard:
initial observation/capture acquires Config, ConfigSave and Params in that order
with TABLOCK,HOLDLOCK; held publication uses TABLOCKX,HOLDLOCK on all three for
the full same-session compare/write/recheck/commit boundary. SQL Server defines
HOLDLOCK as SERIALIZABLE and retains the table locks until transaction completion
([table hints](https://learn.microsoft.com/en-us/sql/t-sql/queries/hints-transact-sql-table)).
The required typed RCSI field remains in the exact canonical UTF-16 JSON hash;
changing it between observation and capture still invalidates the profile seal.
This supports database READ COMMITTED defaults, not arbitrary explicit SNAPSHOT
transactions or a change to the dedicated-session protocol.

The new regression first failed on the true default. It covers both booleans,
missing/null/nonboolean refusal and an altered RCSI bit with the old byte hash.
Source controls retain all three shared/exclusive lock hints and the held schema
comparison. Corrected-head Windows RPC and Form runtime remain separate gates;
older artifacts do not inherit this fix. Nine focused snapshot tests passed,
production library check passed with the existing 97 warnings, and fmt/diff
checks passed. The expected failing log is retained as
`logs/issue-382-rcsi-red.log`; corrected runs are `issue-382-rcsi-green.log`
and `issue-382-rcsi-libcheck.log` in the external logs directory.

## Mixed-collation measured-version guard

Exact97cf was built on the lab Windows host from the verified tracked archive,
using official Rust1.95.0/MSVC/SDK, serial locked/offline jobs1 and unchanged
features/TLS/ASM policy. Both product and libtest builds exited naturally with
zero; all4913 source files and Cargo.lock matched before/after, then workers0
and leaseNone were confirmed. This is VM-local build evidence, not a GitHub
artifact or embedded-Git attestation. Final build release SHA256:
`a5f6649ed0e46ce656b01464b2956e9911bfc22f73dc729295a48c61169b6156`.

The guarded CommonModule RPC then reached preparation and received SQL Server
error468/state9/class16 at snapshot.sql line124: ProductVersion and its parameter
carried SQL_Latin1_General_CP1_CI_AS/Cyrillic_General_CI_AS in the not-equal
operation. No publication was dispatched. Natural product exit0/stderr0 and
full Config8/3314/Save0 seals were unchanged. Result SHA256:
`3dc528fe392ce8fbc12570f47a3c57c2aa9a86cb33efad88653680529cf94e5f`;
FULL RELEASE `348da68a5a613c0910db0b63973780c0a7be0debaeea36ca60a2ce1fb1cb895a`.
A retained-file-only diagnostic bound the exact server cause without a new SQL
query: SHA256 `344a85a5f1866b7d0b89c456a449e6e11b4868b3ef2f9f4ec6a76b04feed53e9`.

The shared canonical/held query now converts both Unicode version operands to
varbinary before comparison, matching adjacent exact target-identity guards.
The measured version, schema hash, locks, current-context checks and parameter
bindings are retained; no collation, SQL option or security setting changes.
Collation precedence applies to character types, not binary operands
([SQL Server collation rules](https://learn.microsoft.com/en-us/sql/t-sql/statements/collation-precedence-transact-sql)).

A focused canonical/held source-contract regression first failed, then passed;
the observer still ends before the measured-version predicate. Covering SQL
unit tests passed75/0fail with4 live tests ignored. Incremental
`cargo xtask check --base HEAD` passed format, workspace-crate clippy and root
all-target check. The initial zero-test filter run is retained separately and
is not counted as RED or PASS; authoritative RED is
`logs/issue-382-collation-red-exact.log`, GREEN is `issue-382-collation-green.log`,
covering tests `issue-382-collation-sql-regressions.log`, and check
`issue-382-collation-xtask-check.log`. Root test warnings remained40 before/after.
Fresh source review found no equivalent unguarded parameter comparison in these
snapshot paths.

One separately bounded read-only ADO scalar probe on the same public target
confirmed correct-version mismatch=false, wrong-version mismatch=true,
trailing-space mismatch=true and cross-collation mismatch=false. One reader
returned four System.Boolean values, drained fully and closed; no application
rows, DDL/DML, transaction, product/native/Form invocation or setting change.
Probe result SHA256 `c2ecb6fcddc2721e8a11c5e04d34b3242345eb7a4a9b315429d03d001100f1fe`;
FULL RELEASE `1b8f5142f5f4232be64dc9d292dd760699ec49df5cda60e7c658c70ba18c98c5`.
This proves the predicate through ADO, not the product/TDS path. Corrected-head
Windows product/libtest and actual Common RPC/Form/native gates remain pending;
97cf artifacts do not inherit the SQL correction. The unrelated full-branch
policy refusal remains unchanged; no whitelist/baseline growth is included.


## Actual guarded editor RPC and native CommonModule acceptance (083a)

The VM-local exact-source build of `083a4360c59885dc44ef960491da006613aed972`
completed the production JSON-RPC prepare/confirmed publish flow on the fresh
public CommonModule SQL target. Preparation, acknowledged `Committed`, pending
state, MAIN tree/status, single-use token replay refusal and full post-readset
checks passed. CONFIG remained eight parts / 3314 bytes; CONFIGSAVE became only
selected body plus versions, two parts / 452 bytes. The original result
`3dcc24cdd740590c36e0b06b4398bfbd1f6d676d5d949846351692e5649a31e1`
retains `COMMITTED_NEEDS_RECONCILIATION`: its stderr classifier expected numbers
where the existing exporter emits three Rust boolean skip flags. Four retained
lines were classified as known exporter summaries, with no severity or unknown
line. The separate assessment
`5f1e7faa4db70e69ab032e37a49e3513184a758577e90616d2a9a5530ade420c`
closes that diagnostic-only reconciliation without changing either original
receipt or repeating publication. Its independent review used source emitters
and hash-bound sanitized classification, not an independent reparse of raw VM
stderr.

The subsequent sole-operator native 8.3.27.2342 apply and full export both exited
naturally with code zero and zero stderr, in 3.248 and 1.529 seconds. Active
body and versions exactly matched staging; CONFIGSAVE became empty. The exported
module was exactly 56 bytes with SHA256
`fea4fcfe9697b678655b03d9a670313961e20ce8e11e32956c97cd35fb6daebc`.
Four unselected XML files and every unselected CONFIG row were byte-exact. CDI
retained the ordered five-entry inventory and changed only the selected version,
matching the actual staged version. Export left the full SQL readset unchanged.
Result SHA256:
`41e160daf0473bbaa644502c697be9e8b2046e0e126f7f8ee0d1472d236c8e78`;
FULL RELEASE SHA256:
`2a2eea16d3e2a748413667c50fcabf73278bda9b111be9142be90819a2378a97`.
This proves the existing CommonModule user route and native cycle on this exact
public target and VM-local build; it is not CI attestation or general writer
admission. Previous nonempty-neighbour/cancellation/stale evidence remains a
separate measured target and revision.

## Native Form envelope refusal before staging (083a)

The subsequent ignored public Form experiment exited 101 at the original-body
decoder, before module preparation and before its sole transaction stage call.
The measured native Form has seven trailing fields; the base-free compiler
codec admits four. The retained reason was exactly `native Form body is invalid:
expected 4 trailing sections, got 7`. Independent full seals remained identical:
CONFIG eight parts / 6073 bytes and CONFIGSAVE empty. Result
`3e17596b8e7b4bb94db7d665c4b89296f343663f4c76a8e620529511a007559d`
and FULL RELEASE
`c454df71fbce00d521e65aa0bf773f93b64e2b9fdcc8d4855dbabd87aba46721`
retain their original indeterminate classification. Separate source/receipt
assessment `71667e12cdfd4c25570c84bcb2ea241bce7006b960282945a852a5e7617c45a6`
identifies a prestage decoder refusal with no stage dispatch and no state change;
it does not convert the experiment to PASS. No Form native apply or automatic
retry followed. A separately validated native module-only envelope and corrected
build are required before another experiment; the four-tail base-free compiler
contract and Form editor route remain unchanged.


### Measured native envelope and separate module-only correction

One bounded read-only extraction of the known synthetic public Form body verified
part0 / DataSize1153 and packed SHA256
`7f989004e1b22bd2896b74cdf8c4b2df9bc0562eae5f5069be806ff8b73043ff`.
Before/after full CONFIG and CONFIGSAVE seals were unchanged; no neighbouring
payload, SQL write, test or native command was included. Extraction result
`a2837483bb296448ccc3a30426afcc43922b116fbe293bc833f592b56aa4287a`
and FULL RELEASE
`4f761bb98c26144a362a7fde81aa0ed5e8d24054c52a9de0cd0abe1cfe87fc91`
bind the exact body to the public native CF baseline. Inflated bytes were3012,
SHA256 `55bec38f031bc49be0c2d7f45a6e66381b1e8657e79906dbba8acb7a898c9836`:
UTF8 BOM, container revision4, layout50, quoted module, five blocks, final0/0.
The packed payload remains outside the repository and is not published.

The correction adds a separate native module-only decoder and raw edit witness.
It uses the existing bounded complete lexical parser, requires that precise
outer grammar before parsing original spans, performs no legacy upgrade, and
never serializes the validation tree. Five trailing blocks remain opaque and
byte-preserved. Both original containers must be valid, and every plaintext byte
outside the quoted module token must match. Existing four-tail compiler/profile
APIs remain unchanged; no Form.xml or additional owner family is admitted.

A clean-room selected preparation regression first failed for the same
four-versus-seven reason. Corrected local gates passed: ten body tests (one
opt-in fixture test ignored by the normal filter), the explicit offline known
public-body test, and76 SQL tests with four live tests ignored. The offline
fixture check replaces only `Номер = 1;` with `Номер = 2;`, verifies exact raw
non-module preservation and the expected187-byte native projection SHA256
`a2d21b5183f1b1f907fcf4a88c67a0127a3d9d7585bd0f0c5efabc2a82ab5f01`.
Earlier attempts that stopped at a wrong compiler environment and a test-only
borrow mismatch are retained as setup failures, not claimed as the regression
RED. The ignored live experiment now uses the structural envelope rather than
claiming a1989 codec coordinate. Native Form staging/apply and editor routing
remain unaccepted until a new exact-source Windows build and actual gates pass.


## Native Form module staging and activation acceptance (5be2)

The exact VM-local `5be2ee5ee9bc83b55be115a193b17aef5740f2e5` release and
libtest builds both exited naturally zero; all4913 source files and Cargo.lock
matched before/after. Build result
`992026e5e8b9755d9fcb5337ffe947e0ebe88cd97ab12e0167bdc0722290150e`
and FULL RELEASE
`0f02ecb6c20728a5d43daad160d5609a53ee8ff6f2fa983ba4dd3b02e53e8614`
bind the libtest131877888B/SHA256
`2ab48f921187706209a9337f15f9fe7726ffa8bf882bcab1e312eb44bcc36cc6`.
This is VM-local source provenance, not CI or embedded-Git attestation.

The corrected sole ignored public Form test passed1/0, naturalexit0/stderr0,
in4.948s. It acknowledged clean `Committed`, retained full CONFIG8/6073 exact,
and staged only selected body plus versions: CONFIGSAVE2/1436 with full seal
`2f128fb460641fa8f6ab6feac9e1c7568896a2485aa6fdef7bae246ca558ad1e`.
Result `9a53090ca3db916b696f6a99ea4482f9d4d48cc19f6a0c0734c7bd49fbf67f57`
and FULL RELEASE
`800758d712a4aa111f0ca45e5d1f43b10acc27e7e29dd19182d3178874922ba1`
record unchanged source/runtime pins, inherited headers, no reconciliation and
zero workers. The earlier prestage refusal remains a separate preserved failure;
no uncertain write was retried.

One subsequent native8.3.27.2342 apply and one full export exited naturally zero
with zero stderr, in3.054s and1.618s. Active selected payloads and versions were
exactly staged; CONFIGSAVE became empty and CONFIG8/6074 had full seal
`759214381ad28490f10ce72f53f572a593822ae1bef805f34dad8f9a77357640`.
All unselected CONFIG rows were exact. The exported module was187B/SHA256
`a2d21b5183f1b1f907fcf4a88c67a0127a3d9d7585bd0f0c5efabc2a82ab5f01`.
All five other XML files, including Form.xml and both Document descriptors, were
byte-exact. CDI retained ordered five-entry names/IDs and changed only the
selected Form-body version to the actual staged version. Full SQL state was
unchanged across export. Result
`8087d9434fa65d9f9ae3c5219935d8b40d1ff957b80a5c36fa30c1a179ea1ded`
and FULL RELEASE
`c688e8c6168cd29bdd570355c783a0b3e89e3f471e09b27a9ff2939c0cd9e658`
confirm closure and unchanged pins. No publication/activation was repeated.
This establishes the native module-only operation on this exact Document-owned
public cohort; it does not certify Form.xml rewriting, other owners, arbitrary
multipart output, nonempty Form staging on this target or an exact native build
inferred from SQL schema.

## Guarded Document FormModule RPC candidate after native acceptance

The server now dispatches either explicit canonical CommonModule or
Document-owned FormModule selection through one retained-plan/publisher pipeline.
Only request syntax chooses the role, before I/O; source/compiler/SQL failures
never select a fallback. Immutable prepared backend identity is retained through
confirmation and the acknowledged reply. The Form backend is
`mssql-document-form-module-configsave`. Both roles retain original census bytes,
private storage admission, complete capture/held CAS, single-part output, strict
Document ownership where applicable, consumed TTL/epoch token, mandatory
verification, finite transaction outcomes and external native activation.
Form.xml, other owner families, object/changed selections and multiple files
remain refused before source/SQL. The CommonModule API/messages and FileDb route
remain compatible; publisher code is unchanged.

Source review cleared the isolated constructor and final RPC wiring. Local server
unit gates passed105/0 with four live SQL tests ignored; the full editor_server
suite passed14/0. Existing ten no-token publication and thirteen unsupported
preparation wire cases remain intact; two canonical role controls reach the
missing-owned-source refusal with zero SQL TCP connections. Backend replies and
late Committed/Unknown handling have focused coverage. These are source/local
checks: the final Form RPC product build, actual token publication/readback and
its native successor remain separate acceptance gates.


## Final guarded Form editor RPC and native Module3 acceptance (87617)

The exact87617f1b408676d0ae1085a367bb09f2c7d56ba7 VM-local release and libtest
builds both exited naturally zero. The previously lost observer session was
reconciled from retained Windows completion markers, not rerun. All4913 source
files matched before/after; package product63838720B/SHA256
`3c823a29a1b01752d0d2bf388e71413ae17ca8469a2f7ac9de967ec1399905f5`
is bound by build result
`6e4cdb6dbfca49aa28d2d6281f6a7b4aa2fa44bbbc10a3af7b7751dbed5c9430`
and FULL RELEASE
`0eb285781fe800680efe7ea168e5b02228eb6ea6108db49344193a65b99d03c5`.

The actual production Form JSON-RPC flow issued eleven requests: explicit target
initialization/list/Windows connect before preparation; empty pending state;
one mandatory verified singleton prepare; one confirmed token-only publish;
pending MAIN tree/status with fresh native CDI baseline; replay refusal only
after acknowledged Committed; shutdown/exit/natural process closure. The new
intentional edit was only `Номер = 2;` to `Номер = 3;` over the fresh native
Module2 export. This was not a repetition of the earlier Module2 staging.
Prepared and acknowledged backend both were
`mssql-document-form-module-configsave`, stagedRows2. Source freeze/census,
MAIN Form owner/parent mapping, filesystem equality plus changed native version,
expired-token refusal, exact SQL post-readset and lifecycle checks all passed.
The process exited0. Stderr743B contained four known model summaries with no
severity/unknown line; the boolean-summary parser matched the existing emitter.
CONFIG8/6074 remained exact; CONFIGSAVE2/1437 had seal
`563096707f3928efb7dac766b987e18383dd1881c6c84b3fec3d3b34c12c6436`.
RPC result `907862e4f8f4621676bbff34c2506648e3e6231c750868adc8c0e8de840a66c6`
and FULL RELEASE
`b74993c05b21d690caf2745a9c06d1f1c6080d721fea09a6ab40e020f4b94124`
confirm no reconciliation, zero workers and unchanged source/runtime pins.

One native2342 apply and one full export then exited naturally zero with zero
stderr, in3.036s and1.517s. Module3 was exactly187B/SHA256
`4dd8a461dd70c0a2e4633bd83175f6a4e889f55a0b7cebc418e4fe01bf729b2c`.
The five other XML files, including Form.xml, Form metadata, both Documents and
Configuration.xml, were byte-exact. CDI retained ordered names/IDs/cardinality
and changed only the selected Form-body version to the staged value. Active
body/versions matched staging; CONFIGSAVE was empty, CONFIG8/6075. All unselected
CONFIG full rows and the post-export full SQL readset were exact. Native result
`072e4961733484bb4023bd7bc853b56b2870f1ca9d500616aeafa402a3e31a09`
and FULL RELEASE
`7e725e3cfaa666f4f0c926ba94f3d85ad7b02e53f69b404af98a5b252a4c000d`
record clean closure, no retry and unchanged pins. No further publication or
native activation is needed for this acceptance scenario.

GitHub Actions was recovered by the owner accepting the fork banner. CI
[40285744346](https://github.com/Jimmo910/ibcmd-rs/actions/runs/40285744346)
passed7/7 and product
[40285495021](https://github.com/Jimmo910/ibcmd-rs/actions/runs/40285495021)
passed2/2 for87617. Prior5be2 CI40282592569 also passed. Each required dispatch
was deduplicated; no permissions, token policy, secrets or allowlists changed.
These hosted results remain distinct from the measured VM-local executable.

## Final scope and historical local governance gate

Accepted production operations are one explicit existing CommonModule module or
one Document-owned native FormModule file, with the closed storage/grammar,
current-context principal, ownership, full census/readset/CAS, single-part output
and finite outcomes described above. Native activation remains separate.
Objects/changed/multiple selection, Form.xml/metadata rewriting, other Form owner
families, dynamic/unsupported layouts and arbitrary repartitioning remain
refused. No generic native-build writer profile, production-database acceptance,
all-format parity or new owner support is claimed. Historical nonempty-neighbour
and cancellation/stale controls retain their own exact revision/target receipts.

The required full local `cargo xtask ci` passed format and both EDT governance
steps, then stopped before the remaining15 steps at physical-adapter policy.
The missing catalog record is `src/mssql_dump/import_evidence.rs` (logical-path
SHA2565574a62490168322556d12d020d387ae765a939f0f12587067ea69e930825999),
not the SQL compiler diagnostic. Independent source audit found zero guarded
occurrences; the existing baseline already has fourteen empty file records.
The minimal proposed registration adds one empty occurrence record and62bytes,
preserving all48 existing records and every previous byte after removing that
insertion; no literal allowance increases. The unchanged guard's self-tests pass.

Automatic approval review rejected that persistent catalog registration before
execution because it changes governance coverage and requires explicit owner
approval under the previous scope restriction. No workaround/retry followed.
The real baseline remains SHA256
`c4bf9eeb5fdeda66191e11a984033d0bb2290fd4f688e9e9cae90095481173cd`.
Thus hosted CI and runtime acceptance are green, but full local governance/CI is
not claimed complete and the change must not be marked release/merge ready.


## Approved catalog registration and final local quality follow-up

The owner explicitly approved the single empty catalog record for
`src/mssql_dump/import_evidence.rs` after the earlier pre-execution refusal.
The approved retry added exactly62bytes and one `occurrences: []` record;
all48 existing records and their ordering are unchanged, with no added literal
allowances. The resulting baseline SHA256 is
`4064964dbbaf7f3e5adca0d6e94339fd16433865fe32547c5745bf782d79af46`.
The normal physical-adapter guard and its unchanged self-test both passed.
The earlier refusal and original incomplete-CI outcome above remain historical
receipts; they are not rewritten as a successful attempt.

The first complete local CI attempt after this registration passed17 steps and
exposed five `cf_load_compiled` failures. Four tests reconstructed a fresh UUID
scratch output under `IBCMD_TEST_KEEP_FIXTURES=1` instead of retaining the actual
published output path. Returning that path preserves the existing full packed
body and configinfo digest assertions. The native exported BSL oracle now
explicitly expects BOM+CRLF, while raw packed comparisons still use raw input.
The next narrow run passed12 tests and exposed the compiled CF verifier's raw
source-versus-native-file comparison. Only the already reviewed verifier hunks
from donor `b6a1ec00` were reused: expected `.bsl` source may be projected through
`native_module_file_bytes`; actual exported bytes remain exact, XML policy is
unchanged, and altered content/malformed output/non-BSL cases remain refused.
The final narrow suite passed13 tests with one existing ignored case; the
expected-only verifier unit and its negative controls passed.

A subsequent full CI attempt reached `offline_conversion_matrix` and exposed
its equivalent raw-LF versus native-BOM+CRLF expectation. Its sole expected-byte
literal was replaced with the same independently reviewed donor oracle; file
inventory, pipeline, revision and content assertions remain intact. No producer,
fixture or cleanup policy was changed for that test. Independent source reviews
cleared the test paths, both native-file oracles and verifier-only port.
The complete root integration command then exited0 with no failing suites.
Its retained log SHA256 is
`4e5a82d8f5d20da4117c686179ac591685d78742ed7667cd84fe2723a2725b35`.
Initial CI and narrow failures remain retained separately.

These quality fixes change only the CF-load verifier and test harness/oracles;
SQL/RPC prepare, publication and codec/source-custody paths remain byte-identical
to the accepted87617 product. The existing Common and Form RPC/native receipts
retain that exact runtime attribution; no repeat database write or activation
was dispatched for these local quality fixes. Native build-wide writer admission,
all-format parity, merge and release remain outside this acceptance claim.


The final staged local `cargo xtask ci` completed naturally with **19 passed**:
format, both EDT governance checks, physical-adapter policy and self-test, four
registered strict OpenSpec checks, workspace clippy, four Python checks, schema
tests, all-target compile, all workspace crate tests, all root integration suites
and root configuration contracts. Workspace tests took185seconds and root
integration150seconds. Retained final log SHA256:
`3ad87428be9dfbc6d8cd32a743946995b65e9fc489b1ae09449eedfd8e61d83b`.
The additional strict validation of this change is recorded with the final
publication checks. The release build, native micro-corpus matrix, SBOM and
release audit are not claimed as local runs; existing scoped public runtime
receipts are separate from those release gates. Hosted validation of the final
published quality-fix head is tracked separately from87617 runtime acceptance.


## Final hosted verification and scoped closure

Published source/quality head
`07385992b96adadbb2fed50b44f5fb20d3dd43d5` completed both required hosted
runs successfully, independently read back after natural completion:

- [CI40301563495](https://github.com/Jimmo910/ibcmd-rs/actions/runs/40301563495):
  seven of seven jobs successful, including both portable platforms and the
  Windows legacy-root harness.
- [VSCode/product40301490504](https://github.com/Jimmo910/ibcmd-rs/actions/runs/40301490504):
  two of two jobs successful, including packaged Windows extension installation
  in a clean editor and offline-tree verification.

No rerun or duplicate product dispatch was created. The sanitized final result
snapshot SHA256 is
`9f8b638175c1f29a808ea3d33f861261d3cf68b0a896574fec598a10981fe56b`.
Together with the final19-step local CI and strict change validation, this closes
quality item3.3. The subsequent checklist/evidence commit changes these two
OpenSpec documents only; source, tests, Cargo inputs, tools and policy baseline
remain identical to the hosted-tested head. The scoped existing CommonModule
and Document-owned FormModule RPC→staging→native-apply/export acceptance is
complete. Generic metadata/object/changed imports, other owner cohorts, dynamic
storage, arbitrary repartitioning and build-wide writer admission remain refused
or unsupported as stated above. No successful runtime cycle was repeated and
no merge or release was performed.


## PR640: integration with current upstream master

The same PR branch integrated upstream
`4ad2c8a3c0ffa214c0a92dbd90d101a5e6c603f2` without rewriting its history.
The four textual conflicts were the CI workflow, `src/lib.rs`,
`src/mssql_dump/mod.rs` and the physical-adapter policy baseline. Resolution
retains the upstream form-schema crate alias, form-body/MOXCEL extractions and
required timing exports, alongside the private import planner, SQL versions
validators and native module-file projection export.

The CI workflow is byte-identical to that upstream revision. Its retired
portable job was not resurrected. The existing Windows library-test packaging
and upload steps moved after the root library tests in the upstream Offline E2E
workflow, gated by the same library scope and the Windows runner; packaging
Cargo is explicitly offline and performs no SQL operation. The upstream
`autotests=false` conversion required an explicit `editor_sql_import` Cargo test
target. Static enumeration and the resulting test run preserve every suite
exactly once, including the new SQL admission contracts.

The baseline is the exact upstream75-record catalog plus only the previously
approved62-byte empty `import_evidence.rs` record. Removing that insertion
reproduces upstream bytes exactly, including all27 guarded-crate records and
all occurrence arrays/counts/order. Its resulting SHA256 is
`03a9fa52b46bd33869767211f77a161d670b5172cfa79af0f1341d690513a48d`.
The source-custody integration retains the full directory-census, witness and
Windows custody production code; its incoming change is only a timestamp-tick
test helper. Independent source reviews cleared all these resolutions.

Before the full gate, pinned Rust1.95 all-target workspace checking passed.
Focused results were105 server/SQL tests passed with four live cases ignored;
10 Form-codec tests passed with one opt-in case ignored;71 source-custody tests
passed with three opt-in cases ignored;14 editor-server tests passed;30 SQL
admission/readset tests passed with one opt-in case ignored. The explicit
read-only offline test of the retained1153-byte public native Form body also
passed, with its existing exact hash check. The new pinned fixture submodule
was checked out at `9832bb8b5cce18fb36d65dd398880f4588053b0d`; fixture integrity
and the extended physical-adapter policy guard passed. Workflow YAML syntax
was validated independently.

The first integrated full `cargo xtask ci` passed26 steps, including all workspace
and root integration suites, then its full root library run passed4259 tests
and failed one cleanup oracle, with20 cases ignored. The existing
`ImportScratch::Drop` deliberately retains scratch for exact
`IBCMD_TEST_KEEP_FIXTURES=1`, whereas the old unit oracle unconditionally expected
removal. Only that unit's three existence assertions now match the existing
retention policy. Success, error and unwind cases remain, and the same exact
unit passed in separate processes both with retention enabled and with the
variable removed. Production cleanup/identity/symlink guards and process-global
environment were not changed. The original failing full-run log is retained.

SQL/RPC production files and Form-codec production behavior were not changed by
these resolutions; the parsed Form record/revision now use the upstream
identical-field crate reexports. Recording, codec and exact public-body gates
validate that integration. No live database write, repeated publication or native
activation was dispatched; the earlier87617 runtime receipts keep their exact
revision attribution. A new-head hosted result is required and is not inferred
from the prior07385992 green runs. At this integration checkpoint PR640 was
still Draft; the subsequently successful hosted checks satisfy its review
readiness gate. No PR merge, release, auto-closing issue reference or new general
writer admission is introduced.


The final integrated staged `cargo xtask ci` completed naturally with
**27 steps passed**, including the extended upstream governance/coverage gates,
all workspace crates, all root integrations and the entire root library.
The final library result was4260 passed,0 failed,20 ignored. Workspace tests
completed in194seconds, root integrations147seconds and root library19seconds.
The final log SHA256 is
`498c3f44b18fda9fd6cc4aab9620ae2df526cb5e62b386b3790a0d5f30e070d3`.
The additional all-target workspace check, explicit public-body offline gate,
focused suites and both cleanup modes above also passed on this integration.
New-head hosted checks and PR mergeability are tracked on PR640 after publishing
this local integration; prior07385992 hosted success is historical evidence only.


## Subsequent upstream Form reader extraction (06a582ff)

Upstream master06a582ffc43e1f17e9518a78adab0ff780a9240d introduces the
Form reader entrypoint extraction (#592) after the prior review-ready head499f1bc7.
The only textual merge conflict is the physical-adapter policy catalog. Its
resolution retains all76 incoming records byte-for-byte and only the previously
approved empty import_evidence.rs record (77 total,62 additional bytes); no
allowance is added. The policy guard, its self-tests and strict change validation
passed. Independent source review confirmed that the moved Form entrypoints
retain the same root codec and explicit host/extension context.

The strict original/native Form module envelope and SQL/server source paths
are unchanged. The original module-token range parser, explicit SQL integration
test target and Windows libtest packaging remain present. The incoming public
fixture submodule is pinned to8d138b896f462c3053d16076cbabf0ffe5030c1c.
This source review is not execution evidence: the new merge-head hosted and
focused results are reported separately in PR640. The earlier exact runtime
receipts remain historical; no SQL/native/VM operation was repeated.


## Review5654540045 and subsequent Form reader move

The review is COMMENTED, not an approval. Its ten actionable findings were
checked against head38f43e2b and the working integration of master47e53bf9
(#641). That move caused one policy-catalog conflict; all76 incoming records
and only the prior approved empty import-evidence record remain (77 total).

Confirmed corrections: operator-only capture parameter NULL refusal, production
scratch retention isolated to cfg(test), expired-plan payload release during
insert/take, shared Windows packaging checks in workflow and xtask, and the five
scope/status/evidence documentation findings. Shared packaging refuses dirty or
changed tracked source before accepting immutable HEAD/tree receipt pins. Three
test-fixture cleanup nitpicks now honor exact KEEP1 retention; existing public
lab artifacts were not removed.

The initial native Form-range controls covered space/tab/CR/LF and passed,
which led to an incomplete false-positive classification. A subsequent parser
boundary check found that the native lexical parser also admits form-feed,
whereas the shared field scanner does not trim it. Two new tests actually failed
at the intended wider range and accepted neighbouring-FF mutation. The native
witness now narrows that field range to the quoted token, matching the existing
original-layout witness. Non-module bytes remain protected; the unchanged
packer's loss of FF therefore refuses before any publication. No grammar,
opaque-section serialization, legacy codec or packer behavior was widened.
Independent source review cleared this minimal fail-closed correction.

Registry drop-counter regressions first produced9PASS/3FAIL at the intended
expiry/capacity assertions, then the corrected server suite passed108 tests.
Strict Form tests passed11, passive adapter tests6 and test-build scratch
lifecycle1. An initially empty output-writer filter is not counted. SQL NULL
source controls produced seven intended RED subcases then GREEN; this is not
a SQL-engine NULL execution measurement. Source review covered token burn/error
ordering, the passive adapter authority refusal and the shared packaging helper.

The ordinary editor-server regression uses the real non-test binary with KEEP1
and isolated child TMP/TEMP/TMPDIR, then checks that successful preflight leaves
no compiler scratch. It is compiled locally; execution is reserved for hosted
CI because local fixture deletion is restricted. No source-extraction mock,
manual SQL/native replay, dependency addition, permission change, bot invocation
or legacy writer-profile admission was introduced. Final revision-specific
checks and the Windows packaging receipt are reported separately in PR640.


The form-feed correction passed workspace all-target checking, all13 Form
codec tests,108 server/SQL tests and the explicitly invoked retained1153-byte
native-body check (packed SHA256
`7f989004e1b22bd2896b74cdf8c4b2df9bc0562eae5f5069be806ff8b73043ff`).
The two new FF tests were observed RED before the witness fix. This tightens
byte-preservation refusal; it is not a new SQL/native execution or support for
rewriting unknown FF-framed bodies. The incomplete initial four-whitespace
classification was corrected explicitly in both this record and PR640.
