# Shared backend API (first dependency commit)

`settings::DatabaseBinding` adds `db_path: Option<PathBuf>` and `agent_write: bool` (default false). TOML keys `db-path` and `agent-write`; file path resolves against the settings file. File entries reject server/rows-dir/masks; agent-write rejects masks. `[mcp]` is accepted as opaque TOML by the common parser; MCP owns strict policy validation.

`server::infobases::EntryParams` adds `db_path: Option<PathBuf>` (JSON `dbPath`). `Entry` adds the same response field and uses `source = "file-db"`. `Session::open` revalidates public Entry invariants and canonicalizes/opens the actual data file without SQL or inherited credentials.

`stored_objects::ConfigSource::FileDb(PathBuf)` is a real file database. `StoredConfiguration::{children,resolve,export,read_file,status}` use the existing effective CONFIG+CONFIGSAVE export path. No native platform process is spawned.

This first dependency commit provides READ support only. `objects/import` and `config/pending` refuse file-db explicitly; the partial write API follows separately. Consumers must not fall back to SQL on file-db. `agent_write` is schema, not permission enforcement. MCP must resolve an exact settings entry and canonical target; the general settings database matcher permits server wildcards and cannot alone authorize writes. Editor initialize entries cannot grant agent permission.

Settings/server consumer file ownership remains with #382 until this commit is integrated. RowsSnapshot additions from #386 are additive and need merge reconciliation at ConfigSource matches.

## Evidence

Contract test was run before implementation: failed on unknown `dbPath`. Five integration tests then passed: real 1CD tree/read/status with byte-identical target and no backup, legacy SQL/rows-dir routing, conflicting selectors, opt-in default/mask refusal, and public Session invariant revalidation. Independent reviewer found the latter validation gap; fixed with regression test. Build uses explicit official Rust 1.95, two jobs, lab-only target/temp/cache.

# Shared partial staging API

`file_import::{Selection, prepare, prepare_guarded, Plan}` is the shared Rust engine; it does not launch the platform. Selection accepts exactly one of relative `files`, full metadata `objects`, or `changed`.

`prepare_guarded(target, source, selection, platform, &mut guard)` calls `guard(&std::fs::Metadata)` on the consumed locked descriptor before reading CONFIG/CONFIGSAVE and after compilation. The opaque plan exposes a serializable `summary()` with selected changed files, readiness, unresolved static metadata references and source dependency suggestions. Missing references produce an unpublishable plan; callers must explicitly confirm added scope and prepare again. No new objects/deletions are accepted.

`Plan::publish_guarded(current_target, &mut guard)` reopens the current target under a write lock, checks inode+whole-file digest CAS, rechecks the retained source census, and invokes the guard on actual descriptor metadata. The final publication callback runs source validation, operator policy and cancellation in that order; target binding is checked again after the callback. MCP owns its policy and pinned startup identity; a path-only check is insufficient. Unguarded wrappers are available to the local editor/CLI library consumer.

`ibcmd_filedb::WriteSession::{snapshot, metadata, replace_checked, replace_guarded}` retain the existing lock, verified-copy and atomic rename protocol. Backup preparation uses a separate `.bak.new`; rejection preserves the previous backup. Interrupted `.tmp`/`.bak.new` artifacts are refused, never silently overwritten. On Unix the CAS token includes dev/ino; on other OSes this new partial API refuses until descriptor identity is measured. Existing full importer and SQL APIs remain available; whole-tree file importer now rejects unused path-prefix rather than accidentally importing everything.

Limitations: measured file collation remains en_US_POSIX/ru_RU only; Mac default `ru` is refused before writes. Existing staged deletions, incomplete/unrecognised stages, new/missing files and unsupported compiler changes are refused before publication. Reference preflight handles static metadata object/type references; runtime BSL expressions remain platform-language behavior. Full expected-tree verification always runs, regardless of diagnostic environment switches.

## Changed import synchronization guard

Changed import requires the source export's `ConfigDumpInfo.xml`. The engine reuses `incremental::{DumpInfo, Ownership}` to associate each byte-different file with its stored row, and requires the synchronized and effective inventories to agree on row ID, metadata name and version before considering it a local change. A stale source can therefore never roll back an earlier stage automatically. Missing/stale baseline is refused before publication; refresh the export baseline or select files explicitly. Import does not silently rewrite the source inventory after publication.
