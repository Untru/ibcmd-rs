# Protocol and authorization design

Protocol research verified 2026-10-22:

- [Current specification](https://modelcontextprotocol.io/specification/2026-07-28)
- [stdio binding](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)
- [Versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
- [Official Rust SDK rmcp 3.6.0](https://docs.rs/crate/rmcp/3.6.0)

Pin rmcp exactly to 3.6.0 (MSRV 1.88, tested with Rust1.95). Use its supported
AsyncRwTransport over stdio, newline-delimited MCP JSON-RPC. The SDK owns modern
server/discover/per-request metadata and legacy initialize/initialized, version
and capability processing, notifications/cancelled, errors and disconnects.
Only tools capability is advertised. No listener or native platform subprocess.

The operator launches:

```
ibcmd-rs mcp --settings /operator/ibcmd-rs.toml --policy /operator/mcp.toml
```

The explicit common settings file has one fixed offline database, e.g.:

```toml
[[database]]
name = "demo"
platform = "8.3.27"
rows-dir = "/approved/canonical/rows"
```

The policy may reside in `[mcp]` of the same `--settings` file (default),
or an explicit separate `--policy` file:

```toml
[mcp]
database = "demo"
source-root = "/approved/canonical/source"
```

Common-file schema from dependency 644684f2: same `[mcp]` fields in ibcmd-rs.toml,
plus `[[database]] agent-write = true` for one exact fixed target, false by
default; true forbidden for masks. Adapter must not infer write permission from
client annotations, tool parameters or a model's confirmation. Import requires
`db-path`; rows fixtures remain immutable. Unknown policy fields fail.

Every call checks pinned policy bytes, common settings bytes, source root identity
and offline rows root identity. The selected entry must match the startup bytes,
not just a second read by the shared parser. For filedb, pin the data file inode
and pass a fresh NOFOLLOW descriptor with the same device/inode directly to
FileDb::from_file; the existing shared effective CONFIG+CONFIGSAVE reader returns
a snapshot. Do not reopen the target through Session after authorization. No model-supplied server, id, auth,
password, settings, source root or output destination. No automatic reconnect to
a replaced target. SQL is disabled until immutable live identity is available. Local filedb reads
use shared dependencies 644684f2 and 571e6a13 (integrated as ea8097f5/7c911837).
The operator must protect these files with OS permissions independently of agent
source edits; the adapter does not authenticate a model as the database operator.

Read offline rows through directory-relative NOFOLLOW descriptors into a bounded
immutable snapshot. Feed that snapshot into existing OfflineRows::from_memory,
ConfigTable and export sinks, preserving a single backend. Source comparisons
also use directory-relative NOFOLLOW reads, reject traversal, absolute paths,
backslashes, empty components and special files. Offline snapshot rows are limited before accumulation to
128 MiB and 100000 parts; filedb input files are limited to 128 MiB. Filedb reads use FileDb::from_file_with_limits: cumulative limits are charged
before object buffers and copied blob/metadata values are allocated, including
aliases and repeated table reads. Budget: 100000 descriptors/live records,
128 MiB copied row binary, 256 MiB object/metadata buffers; each metadata chain
is capped at 1 MiB in the bounded API. Cancellation is checked in initialization,
page reads, metadata parsing, record loops and blob-block walks. Existing
unbounded APIs retain their previous limits. These counters bound major input
buffers/copies; they do not claim an exact process heap ceiling. Tool inputs are
limited to 32 objects/32 KiB, transport lines 64 KiB, results 1 MiB. Resource bounds are enforced before publication; native engine resource
limits still govern decoding. One admitted worker avoids an unbounded blocking
queue. Cancelled futures set the shared engine flag and suppress publication.
Backend error details are redacted from tool content; research environment
filesystem overrides are refused at startup.

`tree` lists children; `read` returns one exported file; `export` returns selected
files in memory as UTF-8/base64. `status` and `compare` use the shared object
comparator over a confined source snapshot and return `ObjectStatus` entries
with changed/baseOnly/localOnly paths and `comparisonScope=selected-objects`.
Explicitly selected objects absent from the database can report local-only
files. The snapshot caps traversal at 64 levels, 100000 entries and 128 MiB;
reads check cancellation and file stability in 64 KiB chunks. A descriptor-based
file/directory census is revalidated after comparison so concurrent additions,
renames and replacements cannot silently produce equality. Symlink entries and
same-inode root aliases are refused. Existing CLI/editor status keeps its lazy
filesystem reads through the same comparator. This descriptor boundary supports
Unix; other platforms refuse MCP startup.
`import` accepts only selected object names from the fixed source root, with
explicit `agent-write=true` on the exact operator-selected `[[database]]`.
Missing/false opt-in, rows fixtures and SQL refuse writes. The shared
`prepare_guarded_with_source_limits` owns compiled rows and source custody;
MCP checks exact settings/policy bytes, root and consumed database dev/inode
before parsing and inside final publication. Database budgets are retained
across write-reader pagers/hash/copy. Actual source custody enforces 128 MiB,
100000 entries and depth 64, including empty directories; its walker avoids
eager ancestor caching and checks cancellation per entry/64 KiB source chunk.

Consumed XML caches retain immutable bytes and detached leaf/ordered-ancestor
stamp witnesses rather than a file descriptor and ancestor handles for every
XML. Final publication reopens each name beneath the held root, compares its
witness and digest, and releases transient handles before the next member.
Selected OriginalSource capabilities retain their original pinned handles.
Windows witnesses include creation time, file identity, modification time,
ancestor stamps and content hash to detect ordinary mutations. Detached
revalidation does not provide unconditional ABA protection or the same
move-denial semantics as continuously held Windows handles. MCP startup remains
Unix-only; this shared engine behavior does not extend MCP platform support.

Import stages CONFIGSAVE only. It returns `status=staged`, `stagePublished=true`,
`applied=false`, `restartRequired=true`; the old session then refuses every call
rather than reconnecting implicitly to an atomically replaced inode. A shared
`Error::Published` maps to `published_recovery_required` with the same restart
requirement, without disclosing recovery paths or misreporting cancellation.
Cancellation after the commit point cannot undo publication and is not reported
as a no-op. Native apply remains an external operator/test-orchestrator action.


## Validation evidence (2026-10-22)

Rust1.95, explicit PATH/RUSTC, jobs1, external build/issue-386 and lab-env caches.
`cargo test --test mcp_adapter --test editor_server --test editor_file_db` passed:
3 real MCP subprocess/client tests, 6 existing editor protocol tests, 6 shared
filedb contract tests. MCP fixture exports match existing cf export bytes. Both
2025-11-25 initialize and2026-07-28 discovery are exercised, including a genuine
SDK cancellation notification. Policy changes, target replacements, override
parameters, traversal and symlinks are refused. Real filedb MCP read/export and
compare match tiny-xml, with identical database bytes and no backup created.

External native READ oracle:
`IBCMD_MCP_NATIVE=/opt/1cv8/8.5.1.1592/ibcmd cargo test --test mcp_adapter real_mcp_client_reads_disposable_filedb_without_mutation -- --exact`
passed. The test creates its own disposable DB and native --data/--temp/output
under external TMPDIR, and compares native-exported module bytes with the actual
MCP result/source. Native execution exists only in this test orchestrator.
Logs: lab logs/issue-386-filedb-mcp.log and logs/issue-386-native-read.log.
This is read evidence; it does not satisfy the write/apply roundtrip gate.

Independent security reviews found and fixed parser separation, rows snapshot
confinement, worker admission, server identity, FIFO reads, part-count limits and
strict policy fields. Descriptor filedb review confirms consumed identity and
snapshot routing; malformed-file materialization was subsequently closed with constructor/row
budget tests and cancellation checks, including malformed native-fixture copies
with repeated blob and descriptor aliases.

Further regression evidence:4056 root library tests passed (15 ignored),19
filedb unit tests passed, all targets checked, and filedb all-target Clippy passed
with warnings denied. Real MCP tests cover changed/base-only/local-only and
not-in-base objects; deterministic census tests cover concurrent additions and
renames. Independent review found no remaining findings after the census and
same-inode alias fixes. Logs: lab logs/issue-386-lib-all-final.log,
logs/issue-386-full-status-final.log, logs/issue-386-filedb-clippy.log,
logs/issue-386-all-targets-final.log.

## Guarded write integration checkpoint

Shared chain2e651993/a647ca82/01af72a1/7542bc87 is integrated, including prior
backup preservation, fallback cleanup and bounded database reads. Source control
fix0c689f35 adds actual custody traversal limits. Test-onlyc35280e0 isolates
process-global cancellation scopes; MCP boundary tests use a separate test process
so global offline rows cannot interfere with SQL adapter tests.

Current functional evidence:6 genuine MCP tests pass with a real native
module-edit/staging/apply/export roundtrip and explicit inventory names/IDs/versions
audit; missing/false opt-in, override parameters, revoked policy and SDK import
cancellation refuse without writes.5 adapter unit tests cover published-recovery error semantics, consumed descriptor
mismatch and policy/target/source/root changes between prepare and publication.
43 source-custody tests pass (3 ignored);4065 root library tests pass (15 ignored);
25 filedb unit and10 filedb integration tests pass, including dirfd/fault tests;
25 staging/editor/filedb tests pass with default invocation. All-targets,
filedb Clippy warnings-denied, physical-adapter-policy guard+selftest and
OpenSpec strict validation pass; no baseline expansion was made.

Logs under the external lab:logs/issue-386-candidate-native-mcp.log,
logs/issue-386-candidate-root-lib.log,logs/issue-386-dirfd-regression.log,
logs/issue-386-candidate-filedb.log,logs/issue-386-candidate-all-targets.log,
logs/issue-386-candidate-clippy.log,logs/issue-386-physical-policy-current.log,
logs/issue-386-physical-policy-current-selftest.log,
logs/issue-386-openspec-current.log. Native fixture evidence is retained in
`tmp/ibcmd-mcp-write-ae4797b4-8b79-4016-a4c4-a87589eb7767`; no private ERP was used.
The [native evidence](evidence/native-8.5.1.md) records the full roundtrip and gate.

The ancestor-rebinding finding is closed by reviewed shared253dff7f
(integrated asb97a0b9a), followed by behavior-equivalent RAII fix83e0ce8f
(as946b95eb). Publication/cleanup use one held parent descriptor and relative
leaf operations; unsupported platforms refuse writes. Independent combined MCP
review found no new blocker, and parent cleared both engine commits.

This provides held-directory confinement and advisory-lock guarantees, not
atomic filesystem CAS against a noncooperating same-UID process changing leaf
entries inside that directory. The operator must protect the database directory
with OS permissions outside model-controlled source edits. Recovery path strings
can become stale after an ancestor rename; verify held file identities when
recovering. A panic may leave `.ibcmd-rs.tmp` or `.ibcmd-rs.bak.new` artifacts
requiring inspection before retry; RAII releases descriptors/locks and is not a
universal artifact cleanup mechanism. Exact-head MCP opt-in/target/confirmation
review by the parent remains a publication gate.

Unknown tool names use protocol error `-32602` rather than method-not-found,
matching the [official tools error example](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling).
Both supported lifecycle versions exercise this with an actual SDK client.

Source-consuming tools (`status`, `compare`, `import`) reject dot-prefixed
files, directories (including `.git`) and links anywhere under source-root.
Only iterator pseudo-entries `.` and `..` are ignored; real dot entries count
towards the entry budget before refusal. Use a dedicated XML source root without
repository metadata. Database-only `tree`/`read`/`export` do not enumerate source.
This MCP admission rule leaves ordinary editor/CLI source inventory unchanged.

Import retains the initial descriptor census (without retaining another copy of
source bytes) through the final publication callback. File and directory stamps
are revalidated there, including detection of empty hidden-directory additions.
Operator policy is checked again after that potentially long validation.
