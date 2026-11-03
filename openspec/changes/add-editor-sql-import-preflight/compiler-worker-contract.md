# Fresh compiler process contract (proposal, not implemented)

## Status and scope

This proposal is based on local diagnostic checkpoint
82ea6fd53330c9b9a2d57b50ac30335b4125c666. It specifies the next review boundary;
it adds no worker command, transport, process launcher, SQL access or publication.
The current provided-view diagnostic remains non-publishable. Neither its caller
labels nor a successful worker can establish SQL identity or mutation authority.

## Existing API evidence

| Source | Observed contract | Missing production proof |
| --- | --- | --- |
| `src/mssql/offline_compile.rs::compile_source_rows_offline` | Installs `PREFETCHED_BASE_ROWS` before source scanning; later attempts refuse. Detached `SqlExec` refuses database access. | Reusable compilation after success or later-phase failure. No reset is proposed. |
| `src/import_plan.rs::prepare_with_export_evidence` | Compiles and mandatorily verifies against an effective packed view; retains bounded raw old/new evidence. | SQL provenance, complete owner/role/parent and versions authority. |
| `src/load.rs::load_onto_base` / `export_base` | Accept filesystem artifacts/trees; export returns raw storage evidence. | An immutable in-memory worker input API. Materializing private input requires a separately reviewed IO design. |
| `src/test_process.rs::isolated_engine_test` and import planner test isolation | Fresh test executable, exact test name and successful one-test completion oracle. | Production deadline, bounded transport, process-domain completion and authenticated executable binding. `Command::output` is not a production supervisor. |
| `src/rac_process.rs::completed_output` | Owns original child and both readers under one deadline; retains unproved attempt and refuses reuse. | Completion of descendants that close inherited pipes. It sends no termination signal. Control output bounds are not compiler payload limits. |
| `src/profile_process.rs::bounded_output` | Applies a 20-second/64-KiB probe policy over the original collector. | Compiler-specific resource policy or proof of semantic success. |
| `src/mssql_managed_worker/child.rs::OriginalChild` | Retains original process/readers and explicit identity/completion observations. | A portable compiler supervisor. Managed native creator/shutdown authority must not be borrowed for a compiler. |
| VS Code test process watchdog | Isolated test supervision. | Direct exit alone does not prove descendant completion or consumed executable identity. |

A mutex serializes access but cannot restore the once-consumed compiler base.
A failed first compilation therefore consumes its worker too. Module-only paths
may avoid this compiler; that does not establish reusable metadata compilation.

## Parent-owned immutable attempt

The parent retains the original source custody and a coherent SQL snapshot.
Before dispatch it captures, bounds and hashes all needed source files, including
compiler metadata rather than only selected module bytes. Capture uses the
existing anchored file census, alias/symlink checks, entry/depth/byte limits and typed
cancellation. It revalidates the original source after capture. A path supplied to
the child is never permission to reopen a mutable operator source tree.

The current HeldSourceRoot witness is a file census with bounded traversal and
file/ancestor custody. It does not yet certify all directory membership/types:
an added empty directory may remain unnoticed. The directory-census correction
is a separate shared dependency. Until reviewed integration and negative controls,
no current source digest or final validation proves complete all-entry stability.
A future worker input adapter requiring that property must refuse; diagnostic
results remain non-publishable. Snapshot materialization cannot repair missing
original-tree membership evidence by hashing only the files it copied.

The worker receives only an immutable source snapshot, packed input rows,
selection and fixed platform/export profile, plus versioned binding data. It
receives no SQL client, connection string, credentials, settings authority,
publication token, live target path or inherited parent source capability.
Handle inheritance must be explicitly restricted by the reviewed launcher.
A child-local source tree is a serialization of captured bytes, not a replacement
for the parent's final source witness. No native platform or shell compiler is a
fallback. Relative-path validation is repeated before child-local materialization.

One request consumes one fresh process. A nonce is single-use, and any failure
requires a new attempt and fresh guards; it never resets globals or automatically
replays a request. The parent keeps source, target epoch and consent/token lifetime
independent of the worker. Preparation does not hold a SQL publication transaction
while waiting for compilation or user confirmation.

## Binding and proposed private interface

This is a type-level sketch only, not an IPC format or executable API:

```text
CapturedCompileInput {
  envelope_version, nonce,
  snapshot_digest, schema_digest, target_epoch,
  source_census_digest, captured_source_digest, selection_digest,
  platform_profile, export_profile, report_schema,
  packed_input_digest, source_lengths, row_lengths, limits,
  expected_worker_build
}
WorkerBuild {
  exact_artifact_digest, source_revision, dirty_source_digest,
  features, toolchain, envelope_schema
}
SupervisedCompile::run(CapturedCompileInput, ImmutablePayload, Guard)
  -> CompletedDiagnostic | Refusal | CompletionUnknown
```

Construction stays private. Caller-supplied hashes are labels until the parent
recomputes them over the exact consumed artifacts. The launcher must bind the
original spawned process to the inspected executable image and expected build;
a pathname check followed by ordinary spawn is not an executable CAS. A child
printing its own hash, PID lookup, or an embedded revision alone is insufficient.
The precise executable capability, platform support and build receipt mechanism
require review before implementation. Unsupported platforms refuse; filesystem
permissions and process limits are not changed to manufacture a guarantee.
No unconditional protection against a malicious same-UID actor is claimed.

The response binds nonce, input/binary/profile digests and exact bounded result
artifacts. Lengths and cumulative allocation budgets are checked before reads or
allocation; arithmetic is checked. A versioned framing design must require one
terminal response and exact EOF, reject trailing/truncated/duplicate frames, and
keep bounded diagnostics separate from artifact bytes. Raw paths, names and source
contents do not enter user-facing error chains. Transport is intentionally not
chosen here: neither arbitrary serde envelopes nor stdout blobs are authorized.

## Deadline, cancellation and custody

A monotonic attempt deadline covers input materialization, launch, compilation,
verification, output draining, result admission and process completion. Separate
source, packed-row, artifact, report/descriptor, allocation, output and traversal
budgets remain cumulative; defaults cannot be raised by a model or request.
Actual CPU/memory enforcement and platform capability are unresolved launcher
gates, not properties established by checking output lengths afterwards.

The supervisor retains the original child, original readers and an owned process
domain. A Unix process group alone cannot prove that descendants did not escape;
a Windows job alone needs reviewed assignment/escape semantics. Completion needs
both original reader EOFs, successful direct exit and proof that the owned domain
is empty under an explicit no-escape model. If that model cannot be established,
completion is unknown and the result is unusable. No guessed PID/name scan or
signal to an unrelated process is permitted. Cancellation/timeout may stop only
proved owned processes under separately reviewed termination authority, then must
reap/drain them. Drop and direct-child kill are not tree-completion proofs.

If startup, reader creation, termination or completion is unproved, retain the
owned attempt/recovery journal and prohibit another attempt from bypassing it.
Record only bounded redacted diagnostics and artifact identities. Test fixtures
stay retained under the lab policy; valuable evidence and unrelated files are
never deleted. A future production scratch lifecycle is a separate scoped design.

Outcomes are `CompletedDiagnostic`, `Refused`, `Cancelled`, `DeadlineExceeded`,
`Crashed`, `ProtocolInvalid` and `CompletionUnknown`. None means SQL publication.
Even `CompletedDiagnostic` requires verified bindings, exact successful completion
and final original source revalidation before any subsequent authority analysis.
Late cancellation discards this non-published diagnostic; it must not later be
reused to describe an acknowledged SQL COMMIT as not written.

## Separate engine evidence and publication gates

The current raw report is diagnostic evidence only. A future engine-owned passive
observer must retain duplicate occurrences before map reduction. It must include:

- Original packed-row hash/profile and metadata normalization path/outcome,
  including warnings/misses; normalized metadata alone cannot certify adoption.
- Root membership and exact ordered owner collection occurrences with typed
  schema provenance. Full UUID parent chains reject missing/multiple parents and
  cycles. Unknown families remain unsupported.
- Exact `SourceAssetRoute` family, role, suffix, relative path and codec plus
  supported emission/completeness. A `.0` suffix alone is ambiguous between a
  common module and a form module. Supported report disposition is not coverage.
- Origin-specific versions declared count, generation, every pair ordinal,
  counted/service classification and required service bindings before reductions.
- Physical table/name to logical projection candidates and decisions before
  generation ranking, hiding or dropping. Original CONFIG and staging observations
  remain separate from incoming versions pruning.

These hooks belong inside the existing engine, not copied into the worker or
planner. Source anchors and extraction ownership require a separately assigned
engine owner. Current owner-completeness refusal remains unconditional.

After a future worker completes, a dedicated non-pooled SQL session must reacquire
locks and prove identity/schema/full CONFIG+CONFIGSAVE physical readset CAS,
selected-key authority, preserved unselected raw headers/parts, versions policy,
source/epoch/TTL/consent/cancellation guards and mandatory merged verification.
Separate prepare and execute calls are not atomic. COMMIT acknowledgement and
unknown transport outcomes remain the transaction adapter's responsibility.

## Required controls before production implementation

| Control | Required result |
| --- | --- |
| Two sequential metadata requests; first fails after base installation | Separate fresh processes; no reset or stale base reuse. |
| Source file edit/add/rename, same-byte replacement, ancestor/root rebind | Parent refuses via original witnesses before result admission. |
| Empty-directory addition/removal or directory/type replacement after capture | Strict membership/type witness must refuse; current file-only census does not establish this gate. |
| Executable replacement or wrong build/features/profile | Refuse before trusting response; self-report cannot repair binding. |
| Stale/replayed nonce, changed target epoch, unbound selection | Refuse; no automatic replay. |
| Length overflow, cumulative budget exhaustion, extra/truncated response | Bounded refusal before result admission; no SQL writes. |
| Direct child exits while descendant holds pipe or closes all pipes | No success without full owned-domain completion proof. |
| Reader startup failure, crash, escaped/unproved descendant | Completion unknown, retain custody; no second attempt bypass. |
| Cancel/deadline during capture, compile, drain, verify, final source census | Typed terminal refusal, prove owned completion or retain unknown. |
| Valid diagnostic with partial/ambiguous owner or versions evidence | Explicit refusal; compiled full rows are not a write set. |
| Child attempts SQL/native/shell fallback or receives target credentials | Contract failure; no such capabilities in request or inheritance. |

These controls are proposed acceptance tests, not executed measurements. No Cargo,
worker, SQL, native or VM run was performed for this contract. Existing 82ea test
receipts keep their original attribution and do not validate a production worker.
