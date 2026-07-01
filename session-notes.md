# Session Notes — 2026-07-01 13:31:27 +03:00

## Current Task
Replace `ibcmd` configuration import/export with direct MSSQL paths for the reference database `Srvr="localhost";Ref="ut_ibcmd"`, while keeping the GitHub issue queue aligned with the fastest path to zero-diff scoped roundtrip.

## Completed
- Fixed duplicate form-event alias patching in `module_blob` so repeated event UUID/name entries no longer collapse distinct handlers during Form body staging.
- Added exact native Form body reuse when the current DB blob already exports to the same `Form.xml` and `Module.bsl` with no source item assets.
- Revalidated and closed the representative `DataProcessors/ИнтеграцияС1СДокументооборот` blocker.
- Refreshed the representative sweep queue and confirmed the first representative default-family pass is green.
- Added deeper sweep discovery controls: `--candidate-offset`, `--candidates-per-family`, `--stop-on-first-non-ok`.
- Fixed scoped post-apply selected export for `Ext/Predefined.xml`, which made `Catalogs/Организации` falsely appear as `left_only=1`.
- Added `--drop-target-db-after-run` so sweep runs can clean up temporary cloned MSSQL databases after each prefix.
- Re-ran the second representative default-family sweep with cleanup enabled and confirmed all selected prefixes are green.
- Updated umbrella issue `#32` with fresh evidence and opened issue `#47` for the new sweep-throughput bottleneck.

## Pending
- Speed up sweep discovery by reusing one target DB or one restore path across prefixes instead of doing a full backup/restore clone for every prefix.
- Re-run deeper-than-second representative discovery after the sweep-throughput change.
- Open the next parity issue only from fresh non-green evidence after the faster discovery loop is in place.
- Review whether local `.github/workflows/ci.yml` should be committed as part of the current debt queue.

## Next Action
Implement issue `#47`: change `infobase config sweep` so one restored target DB is reused or repeatedly restored from one prepared backup across prefixes, then rerun the second-or-deeper representative sweep with `--candidate-offset 2 --stop-on-first-non-ok --drop-target-db-after-run` to isolate the first current non-green prefix.

## Key Decisions
- Use representative scoped roundtrip on `ut_ibcmd` as the primary completion signal, not broad stale parity buckets.
- Prefer exact native blob reuse for unchanged complex bodies instead of chasing serializer micro-diffs when that preserves native export parity.
- Treat sweep discovery throughput as a first-class bottleneck once first and second representative passes are green.
- Keep GitHub issues tightly scoped from fresh measured evidence rather than continuing work from stale umbrella descriptions.

## Modified Files
- `.github/workflows/ci.yml`
- `src/cli.rs`
- `src/infobase.rs`
- `src/module_blob.rs`
- `src/mssql.rs`
- `src/mssql_dump/fetch.rs`
- `src/mssql_dump/mod.rs`
- `src/source_audit.rs`
- `session-notes.md`
