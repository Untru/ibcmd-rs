# Source and local compile evidence, 2026-10-24

Base/current master inspected: `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4`.
Test-first baseline: `751751f8f5670d84cf2d96aab86895b6c5013d83`.
Functional candidate: `8433693b85f6130dccfff11c219207530b02f432`.
Exact reviewed implementation tree: `9c75fc6e226437422a7246ceaac63fb4a3e31c1e` (matches functional commit tree).

## What is established

- Strict OpenSpec1.6 validation passed for enforce-windows-source-custody before production edits. Standalone pinned Rust1.95 rustfmt parsed/formatted source/tests; diff checks passed.
- Independent peer source review approved the exact751 contract/test boundary, then the exact9c75 implementation tree. No actionable source finding remained. The peer read affected/dependent lifetime/census/cancellation code and official Windows contracts without editing or running a compiler/runtime.
- Four isolated Windows controls were authored before production changes: no-leaf directory custody, pre-existing DELETE conflict, explicit zero-access metadata negative control, and ordinary idle child edits. Their code is unchanged between baseline and fixed candidate.
- Three additional profile decision-table controls cover supported/rejected drive/filesystem/identity facts, source-root/member volume binding and the canonical-root namespace. These are synthetic fact/path controls, not actual remote/ReFS-volume execution.
- A single separately bounded local command ran successfully on macOS with pinned Rust1.95, jobs1, an isolated external target and lab TMPDIR/KEEP1: `cargo check --locked -p ibcmd-rs --lib --no-default-features`. It finished in 2m11s. The root reported98 warnings; no warning block named the owned files.rs/private-profile paths. Other warnings are not classified as proven pre-existing without a baseline run.
- No local test, root binary link, all-targets/full-workspace/full-CI, native platform, VM or SSH command ran. The compiler reservation was fully released immediately after completion. No ACL, privilege, credential, network/access setting or global Git setting changed.

## What is NOT established

All seven new Windows controls are authored/unrun. The two predicted baseline failures are derived from source and MS-FSA, not an observed RED. The macOS library check does not compile Windows-gated code and cannot prove Windows ABI or runtime behavior. No branch/PR/CI publication or route enablement is established by this checkpoint.

The paired baseline/fixed Windows plan is in paired-windows-plan.md. A compilation failure or unrelated path/ACL error is not valid red-control evidence. All existing selected leaf, detached cache, witness/census, low-FD/traversal and typed cancellation behavior stays in scope for regression checking, without claiming universal ABA or atomic source/target filesystem CAS.

#530 and its runtime receipts are independent, unchanged evidence for the filedb adapter. Its target NTFS admission does not authorize this source profile. Windows editor_partial_import remains Unix-gated in the inspected baseline; isolated source controls do not establish end-to-end Windows partial/MCP acceptance.
