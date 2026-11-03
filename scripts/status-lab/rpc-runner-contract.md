# Bounded read-only RPC probe: local execution gate

Candidate/API: SQL contract checkpoint `2aff32c7` / `6ca6ade0`, protocol 1.4.
This fake-only transport harness is separate from live infobase acceptance.

The runner admits only the reviewed public Python fake peer. Its fixed invocation
uses the current interpreter with `-I -S -B`, a sealed retained peer snapshot,
`--case <finite-reviewed-case> serve --stdio`. No shell, arbitrary executable,
extra arguments, caller environment, SQL authentication or actual ibcmd adapter
is admitted. A future real-binary adapter needs a separate scope/review/operator
grant; package version does not attest git SHA.

Before spawn, require a regular bounded JSON seal binding candidate/API, actual
interpreter hash, runner/peer/contract source hashes and bounded immutable
source/reference/settings inventories. Refuse symlinks, changed identities,
unknown schema/commit/adapter/case, missing or stale seal. Recheck all original
inputs, seal, interpreter and code after the operation; the peer consumes an
owned exact script snapshot. The report carries executed fake/source digests,
never private paths, raw frames, stderr, SQL diagnostics or credentials.

Exactly seven requests: initialize, active/main tree, active/native status,
native cancellation and a retired-generation native status. Only fixed read
methods and one $/cancelRequest notification are emitted. A successful report
means the synthetic transport controls passed; liveAcceptance=false,
productExecuted=false and publishedStatus=false remain mandatory.

Limits: <=180s wall, <=20s/request, <=1MiB stdout, <=16KiB stderr, <=8KiB header,
<=64 notifications, seven requests. Test limits may be smaller. Refuse malformed,
nonfinite/duplicate-key JSON, wrong ids, errors, missing cancellation and unknown
native evidence; no success after early/nonzero exit, timeout, cap or failed
cleanup. Stale reply is discarded and no final UI status is published.

POSIX starts one new session and signals only its owned group before reaping the
leader. Windows puts the isolated interpreter in a kill-on-close Job Object
before releasing a reviewed bootstrap gate; the fake performs no work/spawns
before that gate. The admitted peer's descendants inherit containment. This does
not claim containment of arbitrary breakaway processes or transitive runtime
attestation; actual ibcmd execution is refused. Unsupported containment refuses
before bootstrap. A cleanup that cannot be confirmed fails closed.

All fixture/run directories are newly owned under external TMPDIR and retained.
No permanent deletion or broad process kill. Synthetic tests may mutate only
those fixtures to exercise source/seal drift. Fresh static review MUST clear
these exact files before any synthetic subprocess execution. No Cargo, VM, SSH,
SQL/native process or Code host is part of this gate. This is editor RPC
transport preparation; no DB snapshot/profile-capture package is duplicated.


## Reviewed ownership/preparation details

Fixture allocation is an explicit capability minted only by create_fixture_root
under this selected external lab's `tmp/issue-380-rpc-runner`, with matching
STATUS_LAB_TEST_ROOT. Existing arbitrary directories, forged capabilities and
reused roots are refused; no matching JSON seal can authorize them. Each admitted
root is newly reserved atomically and retained. Input hardlinks/symlinks refuse.

The contract document, code and settings bind content plus file identities;
pinned API originals, interpreter and receipt identities are rechecked too.
Directory enumeration enforces its entry cap incrementally. Separate <=10s
preparation deadlines are checked between bounded reads and again after cleanup.
These are cooperative filesystem deadlines, not hard realtime cancellation of
an OS kernel I/O stall; the admitted inputs are new local regular fixtures.

After the child exits, every stdout byte must be exhausted as strict frames.
Only bounded well-formed progress/log notifications may trail the seventh reply;
trailing results/errors/malformed bytes/floods fail. No last-response shortcut
can produce success. The protocol wall budget is separate from bounded cleanup
and pre/post verification. Windows Job Object code requires platform execution
evidence before any Windows execution/containment claim.


For the admitted peer only, every descendant retains both stdout/stderr until
exit. Cleanup observes EOF on either still-drained pipe, so a cap-stopped reader
cannot hide process exit. A natural exit is reaped without signalling an empty
group; a live group receives TERM, then KILL only while inherited pipes remain
open. EPERM is not suppressed or reinterpreted as successful cleanup. Real or
breakaway executables are outside this adapter and still refused. Each new
cleanup change requires fresh static review before runtime execution.
