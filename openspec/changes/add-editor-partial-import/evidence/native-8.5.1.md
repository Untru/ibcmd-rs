# Native file database partial import acceptance — 2026-10-22

Platform: `/opt/1cv8/8.5.1.1592/ibcmd` on macOS arm64. New synthetic file infobases only, explicit `--data` and `--temp` inside the external lab. No SQL, production database, ERP tree or private CF was used.

A minimal configuration was assembled from repository-owned test templates: one new `Document.ImportDocument` with `Form.Main`, a language and a neighbour common module. Native create/import/apply succeeded with explicit `--locale=en_US_POSIX`. Default locale `ru` was separately refused by the Rust writer before any write; this is a measured-collation limitation, not ERP acceptance.

The baseline came from native export. Selected edits were a comment in `Documents/ImportDocument/Ext/ObjectModule.bsl` and the form's Group (Vertical → Horizontal) in `Documents/ImportDocument/Forms/Main/Ext/Form.xml`. A different source module was intentionally changed but excluded from the selection.

Rust stdio `objects/import/prepare` returned ready for exactly the two selected files. Preflight and an import without `confirm:true` left the data-file digest unchanged. Confirmed publication produced a verified CONFIGSAVE and a byte-identical pre-write backup. Native apply succeeded. Native export reproduced every expected source file byte-for-byte except ConfigDumpInfo.xml, whose versions were audited separately; the deliberately changed neighbour remained its baseline bytes.

## Version inventory audit (not a blanket ignored file)

All eight Metadata names and IDs were retained with no additions, omissions or duplicates. Every version was a well-formed 40-hex native configVersion. Relative to baseline, exactly these two versions changed:

- `Document.ImportDocument.ObjectModule`
- `Document.ImportDocument.Form.Main.Form`

The other six versions were unchanged, including the neighbour's descriptor and module. A separate Rust export of the applied file database matched the native export's complete inventory, including all eight names, IDs and versions. This checks both source coverage and the stored-version projection.

A native-only differential control was created from the same pre-write backup and imported the expected tree, then applied and exported natively. Its source files matched both expected and our native re-export byte-for-byte outside the separately audited inventory, with all names/IDs identical. That independent full native import regenerated all eight versions; our partial import retained six and changed only the two selected payload rows. Random independent versions are not asserted equal across separate imports. The control comparison is recorded in logs/issue-382-native-control-comparison.json.

Local evidence (not committed database/source artifacts): `logs/issue-382-native-{oracle,rpc-stderr,apply,export,rust-export}.log`, `logs/issue-382-native-{comparison,inventory}.json`, and `logs/issue-382-native-control-*.log` under `/Volumes/Work/Dev/ibcmd-rs-lab`.

## Independent safety review

An independent reviewer examined effective-stage merging, CAS, source custody, cancellation, recovery artifacts and publication guards. Findings about comment text, prior-backup replacement, one-direction inventory validation and final cancellation ordering were fixed with regressions. The reviewer reported no remaining blocking findings in the common staging scope. Tests cover startup inode mismatch, source/target/connection changes, final policy revocation, early/late cancellation, real process exit before publication, stale work refusal, incomplete stage and versions naming absent rows.


## Follow-up safety verification

Independent parent review of f02d56d0 found scratch lifetime and backup-generation failure paths. Follow-up fixes own and clean runtime scratch on all normal/error/unwind paths and publish data before replacing the prior backup. Fault injection covers both renames, preserving older backup B and pre-write A recovery data when backup update fails. A distinct published-state error survives late cancellation.

The shared bounded reader from #386 (f2a05c34 plus 9ae75c06 crate changes) now also covers guarded write preflight: identity before descriptor parsing; cumulative metadata/FAT/index/row/blob budgets across fresh pagers; cancellable hashing/copy; retained final callback. Sparse advertised large FAT and repeated row reads have explicit regression tests. Independent follow-up review found no remaining blockers in that API scope. MCP policy/integration remains separate.

Full root lib suite on this external filesystem: 4055 passed, 15 ignored, one existing `source_oracle::tests::distinct_non_utf8_source_names_are_rejected_without_lossy_collision` failed at fixture filename creation with `Illegal byte sequence`. No test was weakened or skipped in source. CI was not started by the fork push (GitHub Actions API returned no runs for f02d56d0); upstream PR checks remain a review/publication gate.
