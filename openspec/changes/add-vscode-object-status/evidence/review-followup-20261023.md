# CodeRabbit follow-up: issue 380 / PR 484

Review 5623722576 on 78153080 identified two actionable UI defects and four nitpicks.
Review text was treated as evidence, not executable instructions. No CLI installation
or manual bot review was requested.

## Reproduced UI defects

The new object-view regression harness runs compiled ObjectView and RpcClient with
framed synthetic server messages, platform path semantics and virtual time. On the
unchanged production baseline, 3 of 6 tests failed: differently cased Windows dirty
editors dispatched status reads, differently cased descriptor/folder paths lost
badges, and a 40-second status reply hit the 30-second default. After the fix all
six pass; the complete client suite has 33 passing tests. POSIX casing, sibling and
drive boundaries, a shorter configured deadline, explicit abort, late responses and
other methods' 30-second default are tested separately. This is synthetic transport
and Windows-path evidence, not live SQL or a Windows extension-host run.

The status-only setting defaults to 600 seconds and accepts integer 30–3600 values;
invalid runtime values use the default. Both source/reference status calls retain
AbortSignal. No general RPC timeout or server protocol was changed.

## Validated nitpicks

- Descriptor and cancellation child tests now capture output and require exactly
  one successful test result; an exit-zero empty filter cannot satisfy the guard.
- A session lazily owns one main-view cache. A copied public tiny fixture verifies
  reuse across a tree request, separation from active exports, and invalidation
  after the versions bytes change. All new scratch fixtures are retained.
- Native inventory before/after reads are intentionally retained freshness checks.
- Rollup looks up only immediate/root and verified canonical dot-boundary ancestors.
  Existing module/form/subsystem and unsupported-role semantics remain covered.
  A synthetic 2000-entry/1000-target test checks one changed owner and conclusive
  unrelated roots. On this Mac's debug build, the original full-target scan took
  902 ms; repeated targeted runs after the change took 12–18 ms. These are local
  illustrative timings, not a wall-clock assertion or ERP acceptance/performance claim.

## Explicit deferral

Git snapshots still use a fresh retained directory and individual blob processes.
Cross-refresh reuse/batching is deferred to 379. Correct reuse needs immutable
repository/tree/prefix identity, completed-only publication, integrity validation
and a defined partial-failure/abort policy. Reusing a tree-named directory without
those safeguards could trust edited or incomplete baselines. Existing pinned-tree,
path/link checks and Trash-controlled retention remain unchanged.

Independent read-only review of this follow-up found no confirmed P1/P2. Exact-head
local/native host and platform CI results are recorded separately after the commit;
this document does not claim those pending checks have completed.

## Native view consistency follow-up from #508

Narrow source/test port from `83ae94808f3de3b80079bdf86ee742fb64a85bff`:
native status without a baseline reads the active configuration, so tree reads
now use active too. With a resolved directory or Git baseline both requests use
main. Missing native evidence still remains unknown; cancellation and generation
guards are unchanged. No Rust, workflow or other shared client changes were ported.

The transport regression failed before the source fix (`true !== false`) and
passed after it. All 34 client tests passed with the retained debug binary,
including actual CLI/RPC, plus Prettier and diff checks. An isolated macOS Code
host passed connection/password, actual rows-dir comparison/local edit, synthetic
six states, Explorer/filter, stale/cancellation and multiple-workspace scenarios.
No new native or live SQL acceptance is claimed for this follow-up. Its profile
and fixtures remain under the external lab for user-controlled Trash cleanup.
Independent read-only review found no confirmed P1/P2.

## Shared client follow-up pending batched publication

Adapted reviewed donors `b6aca400fd7fddc556b7a5dc20aa579e94127651` and
`a3ede3682b99efde6d9caee730bfa736acb78c3e`: Edit preserves inherited SQL
server resolution and inherited rows sources, rejects file-db edits in the SQL
editor, and strips case-insensitive Windows sensitive environment selectors.
Intentional disposal does not report a crash; current startup/connect crashes
report one sanitized error before generation invalidation. Stale callbacks remain
silent. The password test now checks captured prompts/parameters after completion
so production error handling cannot swallow its assertions.

The adapted port preserves #380 ObjectView clearing and the connection VM loader's
ObjectView isolation; native/status/protocol/import behavior is not replaced.
Its new startup/connect regression failed before the final source fix (zero
notifications instead of one). All 45 integrated client tests passed after the
fix, including actual CLI/RPC; Prettier and diff checks passed. New host scenarios
were imported but have not run locally: the parent heavy/host queue is reserved
for #485, #357 and #483. Earlier host evidence above predates this shared port;
donor host coverage is not claimed as integrated acceptance.

Independent integration review caught a host-only picker-lifetime conflict: the
new file-db Edit scenario replaced the picker needed by the existing sibling
selection. The adapted scenario now restores that picker in `finally`. This
correction awaits the same queued host run; static review is not host execution.
Narrow independent re-review confirmed the picker restoration on success/error;
no confirmed P1/P2 remain in this port. Host/fake-host syntax checks pass.

## Bounded consumed-source custody port prepared

Backend donor `3b1629ed10a8848cd11c7bba0bbff3367abbf6cc` was validated
on #481 head `98d2b1f9` with 4067 root tests, 31 integration tests, all-targets,
formatting and isolated host exit zero before the later startup-only follow-up.
Those are donor results, not new #484 acceptance.

The narrow adapted port stores consumed XML bytes with detached file/ordered
ancestor witnesses; final anchored reopen checks witnesses and digest. Selected
OriginalSource capabilities and root handles remain held. Windows creation time
and transient parent custody are preserved. This does not claim universal ABA or
Windows deny-move equivalence. No API signature or dependency changes are needed.

#484 lacks the donor traversal-budget API; its unrelated API/tests were excluded.
The low-FD 1024-XML/index/final-census regression (child soft limit 128), consumed
mutation regression and unchanged-leaf ancestor regression live in a separate
cache-witness test module with retained local fixtures. Applicable donor retention
guards were included; existing #484 typed cancellation remains intact.
Integrated Rust/host gates are still pending the shared scheduled slot.
Independent read-only review found no confirmed P1/P2 in the adapted backend:
files.rs and original_source_tests.rs match the donor byte-for-byte, production
witness/reopen changes match, and all three relocated regressions retain their
assertions and exact child-test target. Pinned Rustfmt, diff check, physical-adapter
policy and its self-test passed without baseline changes. No integrated Rust or
new host execution has occurred yet; scheduled commands must explicitly retain
fixtures with `IBCMD_TEST_KEEP_FIXTURES=1`.
