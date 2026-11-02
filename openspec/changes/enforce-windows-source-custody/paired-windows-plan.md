# Paired ordinary Windows execution plan (not executed)

## Immutable inputs and placement

- Baseline/tests-only: `751751f8f5670d84cf2d96aab86895b6c5013d83`.
- Fixed production candidate: `63637dbe9914709e57ff64af56b1d8a7fba27a22`.
- Fixed tree: `6a73a7710683d0b9532b981e882e8d433dc1c983`.
- Shared base: `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4`.

Before execution, verify both exact commit/tree receipts and byte-identical windows_custody_tests.rs blobs. No moving branch HEAD, old binary or compile-only result substitutes for either input. Coordinator chooses the separately authorized ordinary push/PR Windows job placement; this document creates no workflow, dispatch, publication, settings change or execution grant. Do not use VM/SSH, user data, native1C, ACL changes or a registration/selector bypass.

Preferred execution is a sequential paired ordinary Windows job with immutable checkouts and captured expected-red outcomes. If using the existing ordinary PR jobs instead, finish and retain the baseline Windows runtime evidence before advancing the branch to the fixed descendant, so workflow concurrency cannot cancel away the baseline proof. Do not broaden/redesign CI without its own scoped authorization. Do not manually dispatch or rerun merely to obtain green.

## Environment and attribution

Use Rust1.95.0, x86_64-pc-windows-msvc, jobs1 and test-threads1. Give baseline and fixed distinct physical TMP/TEMP/TMPDIR and CARGO_TARGET_DIR directories under the job-owned workspace/temp; KEEP_FIXTURES=1. All fixtures are new owned directories/public short strings. Record runner image/OS, toolchain, actual commit/tree, compile exit, exact test names, raw stdout/stderr/exit codes and test-harness SHA256. Retain scoped fixture/log/receipt artifacts, not credentials, unrelated build caches or user files. This is a CI runner plan; all local preparation artifacts stay in the external lab.

First compile each exact Windows unit harness, without running any tests:

```text
cargo test --locked -p ibcmd-rs --lib --no-default-features --no-run --message-format=json
```

Require exit0. Extract the root lib compiler-artifact executable from Cargo JSON and hash that exact executable. Any compile/link/dependency/API-signature failure stops the phase and is NOT an observed custody red. Direct harness execution avoids accidentally accepting a zero-test filter or mixing a cached binary from the other target.

## Baseline: four exact isolated tests

Execute each compiled harness test separately with `--exact --nocapture --test-threads=1`:

```text
mssql_source_change::files::windows_custody_tests::windows_source_directories_deny_delete_without_any_leaf_handle
mssql_source_change::files::windows_custody_tests::windows_preexisting_ancestor_delete_access_refuses_source_custody
mssql_source_change::files::windows_custody_tests::windows_zero_access_directory_probe_is_not_rename_custody
mssql_source_change::files::windows_custody_tests::windows_directory_anchor_preserves_idle_child_edits_without_leaf_custody
```

| Baseline control | Required observed outcome before calling it RED |
| --- | --- |
| No-leaf directory custody | Nonzero test exit and exact expected-sharing-denial assertion because a DELETE probe unexpectedly succeeds; setup/rename round-trip had succeeded |
| Pre-existing DELETE | Nonzero test exit and exact existing-DELETE-not-excluded assertion after successful normal admission and DELETE-handle creation |
| Explicit zero-access metadata probe | One selected test PASS, exit0; metadata-only probe remains the negative control |
| Idle child edit | One selected test PASS, exit0; ordinary creation/edit/nested rename succeeds |

Record actual exits (typically101 for failed Rust tests); the exit number alone is not proof. A different panic, unsupported profile/setup/ACL failure, missing/ignored/zero-selected test or cancelled job invalidates the red receipt and stops the phase. Do not ignore/relabel existing tests or hide unexpected failures under broad continue-on-error. A deliberate expected-red verification wrapper must check the exact test/assertion outcome; it must never waive fixed/required merge checks.

The pinned sharing helper requires raw_os_error()==Some(32) for a denied probe. On fixed PASS, executing that assertion proves ERROR_SHARING_VIOLATION32. On a baseline unexpectedly successful open, there is no applicable failed-call GetLastError; do not invent32 or read a stale last-error value. Preserve raw assertion output and label inferred versus printed OS-result evidence accurately.

## Fixed: identical custody controls plus three profile controls

Run the identical four tests against the separately compiled fixed harness. Require all four PASS, one selected test per invocation, exit0. Then run these exact new tests:

```text
mssql_source_change::files::windows_profile::tests::source_profile_refuses_remote_nonfixed_unknown_and_non_ntfs_facts
mssql_source_change::files::windows_profile::tests::source_member_volume_is_bound_to_source_root_not_a_target
mssql_source_change::files::windows_profile::tests::canonical_root_namespace_is_bounded_without_claiming_raw_operator_policy
```

Require all three PASS/exit0. The profile decision table proves its refusal predicates, not actual ReFS/SMB runtime. Positive capture exercises actual source root/directory/leaf NTFS handles independently of any database target. Internal canonical-path tests do not claim a new raw-operator spelling policy; canonical_directory's existing per-ancestor reparse checks remain authoritative.

## Existing compatibility controls

Run the unchanged Windows selected/detached tests exactly:

```text
mssql_source_change::original_source_tests::windows_required_original_and_ancestors_exclude_write_delete_share
mssql_source_change::original_source_tests::windows_detached_compiler_cache_retains_bytes_and_rejects_later_mutation
mssql_source_change::traversal_budget_tests::consumed_cache_revalidates_file_and_ancestor_witnesses
mssql_source_change::traversal_budget_tests::consumed_cache_rejects_ancestor_rebinding_with_unchanged_leaf
mssql_source_change::traversal_budget_tests::empty_directory_growth_is_bounded_during_initial_and_final_capture
mssql_source_change::traversal_budget_tests::depth_limit_refuses_instead_of_truncating_the_census
```

Require actual PASS with no missing/ignored substitutions. Preserve the distinction: selected leaf handles exclude writes/deletes; detached nested-cache edits remain possible but final witness/digest/census must refuse changes. Existing typed cancellation must not become an IO/profile fallback.

A separately authorized ordinary Unix job must retain the existing low-FD regression:

```text
cargo test --locked -p ibcmd-rs --lib --no-default-features mssql_source_change::traversal_budget_tests::source_xml_custody_is_bounded_under_low_fd_limit -- --exact --nocapture --test-threads=1
```

It uses the existing child-only soft FD limit128 for1024 XMLs; do not alter host/global limits. Retain ordinary Unix source/witness/traversal/cancellation checks as appropriate to the eventual merge gate. No new local test run is authorized by this plan.

## Completion and limits

The evidence table must distinguish baseline predicted/observed red, fixed Windows compile/runtime, synthetic profile refusal, existing regression results and unavailable profiles. Record every pass/fail/ignored/missing outcome with exact run/job/head/harness hash. Do not claim actual remote/ReFS acceptance, end-to-end Windows editor import or MCP mutation, universal ABA protection or atomic source/target CAS. Source review, local macOS compilation and any #530 target-layer results are separate evidence only. Publication/CI/Ready/merge remain outside this preparation phase.


## Reviewed-runner preparation

The dedicated `source-custody.yml` ordinary PR workflow and `tools/run-source-custody-ci.py` implement this plan for the single Jimmo910 source-custody branch. They add no manual trigger or permissions beyond contents:read. Existing CI workflows and merge requirements are unchanged. The Windows matrix entry executes both immutable inputs sequentially; a separate Ubuntu entry executes the unchanged exact low-FD control. Neither job accepts a moving production candidate: HEAD must descend from the fixed input and differ only in this change's OpenSpec documents, runner and dedicated workflow.

The runner requires pinned Rust1.95 and the declared host, refuses reused target/evidence roots, compiles separate lib harnesses, verifies a single root executable per phase, records its SHA256 and runs each test exactly once. Receipts/raw stdout/stderr and new owned fixtures are uploaded even after failure; build caches and unrelated temporary compiler files are excluded. The classifier requires one selected non-ignored test with the exact name/result; expected baseline failures additionally require exit101 and the pinned assertion text. Local stdlib-only classifier self-checks passed two valid outcomes and five refusal cases (zero selected, ignored, unrelated assertion, wrong process exit, unexpected success). This is parser evidence only; no Rust compilation or Windows runtime ran during runner preparation.

The source/contract approvals predate this runner delta; obtain fresh independent review of this workflow/runner and exact prospective head before ordinary fork push/Draft. Do not reuse this branch-specific pinned workflow as a general future CI claim after production changes.


The HEAD guard explicitly requests `git diff --no-renames --name-only -z` and parses NUL-delimited byte paths. This exposes protected deletions/additions even when an unchanged production file is renamed into the allowed OpenSpec directory. Its local retained Git fixture forces `diff.renames=true`, confirms the old destination-only listing would be admitted, then requires the production guard to reject the same rename. The fixture uses public synthetic bytes, tree objects without commits/identity, an empty template and no remote or global configuration. CI retains that owned self-test fixture (including its synthetic Git objects) alongside the scoped receipts; no unrelated cache is added. This guard regression is separate from unrun Windows custody evidence.


The first published candidate843 did not pass actual Windows compilation: E0432 rejected importing DRIVE_FIXED from the windows-sys FileSystem namespace. Candidate2070 declares the official documented GetDriveTypeW DRIVE_FIXED=3 return code locally, without a new dependency/feature or predicate change. The old immutable843/tree9c75 and run receipts remain historical evidence; compilation failure is not an observed baseline RED or fixed acceptance. Baseline751 and all four identical custody controls are preserved. The new functional pin and this runner-pin delta require source re-review and a new ordinary push event only after the first paired run's available receipts are retained; no manual rerun or moving-head substitution.


Candidate2070 then passed Windows compilation but failed four real rename-positive controls in root CI; its dedicated paired run40079496139 stopped at idle child rename after three fixed passes. That terminal archive12555720101 was retained and its SHA2564914299c83ff5027675601a0cf0bedfeb4818943f0917d60bd2f044b4a33240d/raw assertions verified before changing pins. Baseline751 again had2 intended RED+2 PASS. The updated directory-only contract96c4 permits READ|WRITE sharing without DELETE sharing for MS-FSA destination-parent add-child opens; leaves retain exact strict READ sharing. Functional63637dbe/tree6a73a771 implements only that conditional directory mask. All four custody tests remain byte-identical to751, and every existing original-source/traversal regression is unchanged. Review this exact implementation and runner-pin descendant before the next ordinary synchronize; source approval alone is not a runtime verdict. Require both deny controls and all positives/regressions to pass, without releasing retained handles or weakening an assertion.
