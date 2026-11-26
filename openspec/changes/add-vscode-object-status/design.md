# Native versions and independent local-file evidence

`objects/status` keeps its original byte-only request/response. Optional
`versionBaselineDir` adds native DB evidence through #362's shared
`native_config_dump_info` / `ConfigDumpInfoSnapshot` helper. `fullRequired`, typed
`nativeChanges` and `databaseSnapshot` are preserved. Per-row
`databaseVersionStatus` contains basis, exact tree object, objectVersion,
baselineIdentity, databaseSnapshot, conclusive and optional changed.

The engine reads the native inventory before/after tree/byte comparison and checks
reference bytes before/after. The client makes both source and reference calls in
native mode, compares the complete native snapshot, per-object cache tokens and
reference SHA, then combines the native changed flag with independent local/reference
bytes. Tokens/hashes are freshness identities, never native configVersion values.
The byte-difference union remains solely a way to discover local/reference paths.
Native mode is default; byte preview is explicit and never an automatic fallback.

The adapter consumes #362's canonical `owners()` mapping (a5fdd226), retaining
immediate object, native object and root in `nativeOwners`. Supported module/form
rows roll up explicitly to their immediate UI object and root object (whose source
files contain those nested files). Storage row UUIDs are not metadata UUIDs.
Unknown roles and incomplete coverage stay inconclusive; there is no guessed
nearest ancestor. Native fullRequired makes common-object evidence inconclusive.
DB-only/file-only existence remains separate from native version change reporting.

Filedb uses the shared effective CONFIG+completed CONFIGSAVE backend. Native SQL
status and `tree/children(mainConfiguration=true)` use a separate main-view cache
and the existing read-only CONFIGSAVE overlay. Legacy tree/byte requests retain the
active view. SQL native exports read the whole main view before object partitioning:
selected raw rows cannot reconstruct completed staging. Recording SqlClient tests
verify CONFIG/CONFIGSAVE SELECTs, separate cache/arguments and reject all writes;
no live SQL login or server acceptance is claimed. Missing native evidence cannot
fall back to byte equality. This favors correctness over the excluded #379 work.

Generation invalidation and AbortController reject cancelled/stale requests after
refresh, selection, settings, crash or workspace removal. Folder references are
read-only. Git refs are pinned and materialized in an explicit external cache.
No native executable or shell is invoked by production code.

See version-contract-proposal.md for historical rationale and its implementation
update, plus evidence/native-integration-20261022.md for real six-state acceptance.
Parent review/PR gate remains; no complete ERP browser/performance or GUI claims.

The typed ConfigDumpInfo codec is integrated narrowly from #362 30b0461: XML
grammar, row/part types, escaping and native bytes belong to ibcmd-xml; storage
extraction and canonical routing remain in the adapter. The b46 snapshot/a5 owner
APIs and separate SQL main view remain unchanged. Six public native XML captures
are retained for codec byte-parity tests. Later CLI commands/native_versions mode
and its empty-CONFIG entry point are outside this integration. Inventory capture
now belongs to the existing config_dump_info adapter. Policy baseline is unchanged.

## Review follow-up: path identity, deadlines and cache ownership

Windows file identities are normalized through the editor URI API and folded for
identity comparisons only; filesystem read paths and confinement checks are not
rewritten. Dirty-editor and descriptor/folder decoration checks use directory and
drive boundaries. POSIX identity remains case-sensitive.

Each objects/status call has its own configurable deadline (statusTimeoutSeconds,
600 by default, integer 30–3600). Invalid runtime values use 600. Explicit abort,
stale-generation and reference/snapshot guards remain; other RPC deadlines retain
their existing defaults. Virtual-time tests execute the real framed RpcClient and
compiled ObjectView through two 40-second responses, deadline expiry and abort.

A session lazily owns one main StoredConfiguration cache, separately from active
inventory/exports. Every reuse retains versions-digest invalidation. Native status
still brackets work with two inventory snapshots: removing the second read would
weaken freshness. Owner rollup looks up immediate/root and verified dot-boundary
canonical ancestors, without scanning every unrelated target or changing unknown
coverage policy. A public tiny fixture tests main-cache reuse/invalidation and a
synthetic 2000-entry/1000-target fixture checks unrelated-root verdicts.

Git snapshot reuse/batched extraction remains #379 work. A safe reusable cache
requires repository/tree/prefix identity, completed-only publication and an
integrity/cancellation policy. Retained snapshots and existing path/link guards
remain unchanged; no ERP-scale Git performance claim is made.
