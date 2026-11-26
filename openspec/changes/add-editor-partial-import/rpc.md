# Editor import protocol 1.2

Existing 1.1 clients are accepted. SQL objects/import retains its previous contract. rows-dir stays read-only. New verified editor import currently supports Unix file-db connections with measured collation; Windows refuses the new CAS path explicitly.

Prepare (exactly one selection mode):

```json
{"method":"objects/import/prepare","params":{"infobase":"lab","sourceDir":"/workspace/src","files":["Documents/Example/Ext/ObjectModule.bsl"]}}
```

`objects:["Document.Example"]` selects existing objects, including their owned files. `changed:true` selects local differences only when their synchronized ConfigDumpInfo row identity/name/version still agrees with the effective target. Missing/stale baseline fails before publication. Import does not automatically rewrite the source inventory: refresh it after synchronization, or use explicit file selection when reviewing a conflict.

Response: `{plan, preflight:{target, files, ready, missingReferences}}`. Missing references include an optional suggestedObject. Adding dependencies is a separate user-confirmed scope change and requires a fresh preflight; new objects remain unsupported. A plan with unresolved references cannot publish.

Publish:

```json
{"method":"objects/import","params":{"infobase":"lab","sourceDir":"/workspace/src","plan":"opaque-preflight-token","confirm":true}}
```

Without confirmation there is no write and the plan remains usable. Tokens are one-use, bound to the connection generation, canonical target inode/content and source census. A reconnect invalidates all pending tokens. Source or target changes require a fresh preflight. There is one pending plan per connection, at most four per server.

VSCode commands: `ibcmd.importSelected` accepts file Uri selections or #380 Row values `{object,uri}`; `ibcmd.importChanged` validates Explorer URI context and otherwise uses the current connection. `ibcmd.sourceDirectory` (default `.`) is workspace-relative. File/object selection outside that source tree and dirty files are refused. Source/connection/settings are rechecked after each asynchronous confirmation. Dependency consent and final file-list consent are separate; cancellation is forwarded through $/cancelRequest. Early known validation messages are typed and displayed; unknown connection errors retain existing sanitization.

No production extension code launches native ibcmd. Applying the stage belongs to #384 and the platform. `config/pending` remains unavailable for file-db in this slice; the UI reports pending unknown rather than asserting no pending changes.

## MCP integration boundary

The common guard API supplies consumed descriptor metadata and a final publication policy gate. Untrusted consumers use `prepare_guarded_with_limits(target, source, selection, platform, guard, ReadControl { limits, check })`. `limits` is `ibcmd_filedb::ReadLimits { max_records, max_binary_bytes, max_object_bytes }`; `check` is `Arc<dyn Fn() -> ibcmd_filedb::Result<()> + Send + Sync>` retained by the opaque Plan. `publish_guarded` needs no separate budget argument.

`open_for_write_with_limits` acquires the lock and invokes the caller guard on actual descriptor metadata BEFORE parsing database descriptors. One cumulative budget covers metadata, FAT vectors, index expansion and repeated CONFIG/CONFIGSAVE reads, including aliases. Preparation and publication each get a fresh bounded session under the same limits. Snapshot hashing, copy loops, page/record/blob walks and final publication observe the callback. The retained callback runs again after long source/policy checks, immediately before the final global cancellation check and publication. The synchronous engine also supports the existing `cancel::scope` for cancellation while compiling/exporting the expected tree.

The adapter must still supply its own exact startup dev/ino, settings/policy bytes, source-root identity and opt-in checks through `guard` on EVERY call, including the final publication gate. This API does not authorize agent writes. MCP integration and its end-to-end validation belong to #386.

## Temporary data and publication failures

Each prepare owns a private 0700 scratch directory. A lifecycle guard removes only that runtime-owned directory on success, error, cancellation or unwind; a completed Plan contains verified rows, not persistent copies of the configuration. Deliberately retained native-oracle lab evidence lives elsewhere. An abrupt process termination can still leave owned scratch for operator inspection.

Publication renames the verified new database first, retaining the previous backup until that succeeds. If database publication fails, the original data and old backup remain unchanged. If the following backup publication fails, `ibcmd_filedb::Error::Published { recovery, error }` explicitly reports that the new database IS published; the complete pre-write database remains at `.ibcmd-rs.bak.new`, and the older `.ibcmd-rs.bak` remains unchanged. No destructive rollback is attempted. The caller must report this recovery state and inspect it before retrying. A late cancellation cannot overwrite this outcome. A leftover `.bak.new` blocks subsequent writes. Fault-injection tests cover both rename failures.

For untrusted consumers, `prepare_guarded_with_source_limits` takes an explicit
`SourceInventoryLimits` alongside retained `ReadControl`. The guarded path also
limits source directory entries (including empty directories) to `max_files`
and depth to 64, through initial custody and every final revalidation. The walker
keeps admitted ancestor iterators open to avoid eager directory caching outside
the entry budget. Source census checks cancellation per entry; original file
reads and hashing check it per 64 KiB chunk. Ordinary editor `prepare` retains
its existing source admission defaults. These controls do not replace consumer
policy checks or prove descriptor-relative publication confinement.


## Filesystem namespace boundary

On Unix, the writer retains a parent-directory descriptor and uses single-component `openat(O_NOFOLLOW)`, `linkat`, `renameat` and `unlinkat` names for target/work/backup operations. Data and work-copy identities are checked relative to that descriptor. An ancestor replacement after the final guard cannot redirect writes or cleanup to the replacement directory. Non-Unix file database writes fail closed until an equivalent mutation adapter is implemented; read support remains.

This is mutation confinement with advisory locking and preflight identity/content revalidation, not unconditional atomic filesystem CAS. A malicious same-UID process able to mutate leaf entries in the held directory can race the last leaf check against rename/unlink without respecting the lock. No existing user filesystem permissions are changed to claim otherwise. After an ancestor rename, original absolute paths in reports/recovery errors can be stale: recovery must locate the original directory and verify identities before using any named file. A path that has been rebound must not be treated as trustworthy recovery data.

Import errors never expose server exception chains in editor UI. Code `-32016`
means database publication succeeded but backup finalization failed; the editor
shows a fixed recovery-required warning and must not describe it as cancellation
or a no-op. Ordinary failures retain `-32014`; source cancellation retains
`-32800` through the source-census error chain.


The import client advertises protocol 1.2 during initialization; a 1.1-only
server rejects initialization before any import or connection request.
Dependency expansion uses every canonical root collection, including modules,
forms and descriptors inside their owning objects.

Consumed XML bytes are immutable and belong to the original source census.
Their cache retains compact file and ordered relative-ancestor stamps rather
than a descriptor chain per file. Final publication reopens each consumed name
relative to the retained root, compares those stamps and the original digest,
and repeats the complete census. Temporary file and ancestor handles remain
open throughout each read and name verification. Selected OriginalSource
capabilities retain their original handles separately. Resource use for cached
XML handles is bounded by path depth, not XML count; byte and traversal budgets
are unchanged. On Windows this detached cache detects ordinary replacements on
revalidation instead of preventing moves between phases. File IDs/timestamps
can be recycled or restored: this is not unconditional ABA exclusion against a
non-cooperating same-user filesystem writer. Advisory-lock limitations remain.
