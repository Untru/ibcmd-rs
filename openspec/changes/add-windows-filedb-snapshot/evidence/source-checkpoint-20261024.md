# Source checkpoint, 2026-10-24

Base: `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4` (current master inspected, merged #481 included).
API/OpenSpec contract: `238ea99f`.
Initial functional source/tests: `cccc90a235931a20130f1fff67afea6223cb252e`; source-review P1 correction: `ebe6079a030d98fd09d382a99a8b608fef1cfb4c`.

## Checked

- Read latest #382 comments, merged #481 scope, current AGENTS/CONTRIBUTING and OpenSpec/review instructions.
- Strict cached OpenSpec1.6 validation returned `Change 'add-windows-filedb-snapshot' is valid` before production edits.
- Standalone pinned Rust1.95 rustfmt parsed/formatted all six edited Rust files; no Cargo command was used.
- `git diff --check` passed. Scope audit: only ibcmd-filedb source/tests and this OpenSpec change; no dependency/lockfile, read_limits, Windows security, server/SQL/file_import/planner/MCP/source-custody or #529 edits.
- Source audit traced the complete Windows path: actual-descriptor guard, original share0, ancestor denyDELETE, copy/security/row-index-unowned-page verification, hardlink/fallback backup, final identity/security checks, original-handle release, data-first and backup-second renames. The original-handle release is an explicit race boundary, not an atomic-CAS guarantee.
- API/threat model sent directly to the #386 owner and resource coordinator. #386 keeps Windows MCP mutation unsupported until its complete chain is independently proven.
- One scoped issue update: https://github.com/Untru/ibcmd-rs/issues/382#issuecomment-6380254254 . Project9 mutation was not retried.

## Authored, NOT executed

Seven Windows integration controls: pre-existing ancestor DELETE-access refusal; held writer/delete/ancestor exclusion; same-byte distinct live identity; content change on the same file object; cancellation during hashing without publication; snapshot through full guarded CONFIGSAVE publication with exact prior database backup and unchanged CONFIG; final-guard refusal preserving the original database and previous backup.

Four Windows unit controls: remote/unknown/non-NTFS/unusable identity profile refusal; UNC/device/drive-relative path refusal; both injected rename failures preserving the appropriate data/recovery generations with ancestor custody held; injected fallback-copy cancellation and pre-existing recovery refusal.

The existing Windows cancellation case now expects the actual retained cancellation callback error. Existing deny-write/delete, junction and security/ACL assertions remain unchanged. Eleven new tests are source contracts, not eleven passes.

## Pending gates

A separately granted, jobs=1 crate-only macOS phase subsequently ran successfully: `cargo check --locked -p ibcmd-filedb --all-targets`; `cargo clippy --locked -p ibcmd-filedb --all-targets -- -D warnings`; `cargo test --locked -p ibcmd-filedb --test snapshot_unix -- --test-threads=1` (**2 passed, 0 failed/ignored**). Both newly authored Unix controls use explicit lab TMPDIR or a worktree-local fallback, fresh directories, no precleanup, and KEEP-aware Drop. The test checks exact legacy Unix identity/content token compatibility and cancellation with unchanged database/no recovery artifacts. All local fixtures were retained. The grant was fully released immediately after completion. These macOS commands do not compile or execute Windows-gated code. No root link, all-workspace suite, full CI, native-platform command, VM or Windows runtime control was run. The fresh independent source-only reviewer found a P1 in the existing ancestor custody: access_mode(0) does not participate in deny-delete sharing under MS-FSA. Commit ebe6079a fixes retained ancestors to FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES without FILE_SHARE_DELETE and adds a direct pre-existing DELETE-handle refusal regression. Named/leaf metadata queries stay zero-access for compatibility with the exclusive original. No ACL/access setting is changed. The same reviewer returned **APPROVE — source only** for ebe6079a with no remaining blocking source findings; it cannot replace compilation or actual Windows CI acceptance. The existing legacy native_files helper still has unconditional test teardown, so any later local focused run must audit retention first; a blanket KEEP flag is not sufficient. New fixture helpers use explicit TMPDIR or a worktree-local fallback and honor KEEP=1.

Before capability acceptance: independent review; separately granted safe focused checks; ordinary Windows CI exercising new and existing whole-publication/security/deny-sharing controls; required full CI. Keep any PR Draft and leave #382/#386 acceptance open. No source/target permissions, credentials, VM or access settings changed.

The same independent reviewer completed the final source recheck with **APPROVE — source only**, exact `96bce7a3801bf0a6345595771aaed23c38d3afe8`. It confirmed unchanged production code, the retained Unix tests, the check/Clippy/2-test logs and the outstanding Windows gates; it performed no additional build/test/native/VM reruns. Actual Windows controls and full required CI remain separate pending gates. The local source approval/macOS checks do not authorize enabling the #386 MCP Windows mutation selector.
