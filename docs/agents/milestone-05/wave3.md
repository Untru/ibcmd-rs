# Milestone 0.5 — third wave tasks

Design: wave3-design.md. Base: 6a7cbbf60e70d35c03767e0ad06c1afa4915e6e7.
Execution: adapted write-plan/subagent-dev implementer + independent reviewer,
already explicitly authorized by the user. Plans/lab STATUS do not replace
https://github.com/users/Untru/projects/9 Status and Agent fields.

## M — broader metadata, synonyms, #345

Worktree F:/ibcmd/src/ibcmd-rs-05-metadata-wave3; feat/0.5-metadata-coverage.
Lab F:/ibcmd/lab/05/wave3/metadata; Track meta; DB ibcmd_rs_05_meta_w3_*.

- [ ] M1 Inventory existing source/staging classification and native row names
  for ObjectModule, ManagerModule, managed forms and templates across registered
  object kinds. Read existing compiler/model/export maps before creating fixtures.
  Files: src/mssql_config_apply/dynamic.rs, dynamic_overlay.rs, gate.rs,
  src/mssql_source_change.rs, src/mssql_stage_*, src/metadata_model/.
- [ ] M2 Create the smallest owned native/own twins and actual session markers
  for distinct storage/classification families. Preserve native stages, all
  relevant inventories/raw blobs and exact source export comparisons. New,
  removed or descriptor/schema-changing shapes must still refuse.
- [ ] M3 Implement measured owner/body/template classification and pending
  history/deletion support with exact semantic/physical CAS. Ask coordinator
  before editing shared profile, main activation, generic recovery or LIVE files.
- [ ] M4 Address the 128-row limit with a measured >128-row delta/control and
  explicit bounded byte/row budgets; never remove limits by assumption. Add
  meaningful unknown-owner/suffix/layout and drift regressions.
- [ ] M5 Quick gates, matching actual write/final CLI provenance, immutable
  commit, independent code/spec/evidence review and guarded owned cleanup.
  State exactly which families remain unproved; continue the matrix.

## P — platform 8.5, release_review, #346

Worktree F:/ibcmd/src/ibcmd-rs-05-platform85-wave3;
feat/0.5-platform85-activation. Lab wave3/platform85; Track p85;
DB ibcmd_rs_05_p85_w3_*.

- [ ] P1 Read support-platform-8-5-live-activation design/tasks/evidence and the
  current params-ui correction. Inventory exact platform/storage/RAS assumptions
  in src/mssql_platform_profile.rs, mssql_main_activation.rs, mssql_apply.rs,
  mssql_worker_switch.rs, mssql_live*.rs and cluster kit. Ignore stale .ui theory.
- [ ] P2 Restore new BSP85 twins; capture native staging/activation/raw storage
  and old/new client/server marker observations. Use owned registrations on
  service 3541 initially; source infobase bsp and other users remain untouched.
- [ ] P3 Establish an owned isolated exact-build 8.5 worker boundary; propose
  helper/port/file ownership to coordinator before shared-kit changes. The
  existing 5540 worker is 8.3 and may not be silently replaced. Capture loaded
  infobases even without active connections before any process signal.
- [ ] P4 Implement only proven exact-build profile/staging/activation/worker
  differences; preserve 8.3 and unknown-build refusals. Validate modules, form
  and extensions; extension code changes require a new maintained Version.
- [ ] P5 Meaningful profile/identity/ambiguous-worker/refusal regressions,
  old/new and same-session views, native/own source parity, timing evidence,
  quick gates, immutable independent review and guard-preserving cleanup.

## W — real workload and readiness, s1_mix, #349/#409

Worktree F:/ibcmd/src/ibcmd-rs-05-load-wave3; feat/0.5-load-readiness.
Lab wave3/load; Track load; DB ibcmd_rs_05_load_w3_*.

- [x] W1 Reuse scripts/apply-lab/live and observer/readiness evidence. Select
  reproducible writable BSP document and report/read operations; use applicable
  1C skills/help/syntax validation for BSL instead of guessed APIs. Create an
  owned registration and bounded N-client harness with explicit cleanup.
  Accepted bounded checkpoint: load-wave3-w1w2 evidence, three clients and1434
  confirmed posted UUID/report pairs. W2 broader native/own matrix remains open.
- [ ] W2 Native control and own repeated dynamic generations under the same
  workload: committed operations, latency/error counters, old/new client/server
  marker observations, captured SQL/RAS ownership and storage/source evidence.
- [ ] W3 Reproduce remaining F5 with current opt-in checkpoint/continuation.
  Propose/implement adaptive readiness only from real reconnect/cohort/storage
  evidence. Active transactions, chain/identity/generation/master lock guards
  remain strict. Explicitly distinguish retained cycle1 from refused cycle2.
- [ ] W4 Prove continuation/idempotence, refusal on active work/unknown binding/
  stale artifact/deadline, five reproducible loaded operations and both old/new
  markers. Do not replace runtime parser errors with claimed functional proof.
- [ ] W5 Meaningful tests, quick gates, matching binaries/raw proof, immutable
  independent review. Mark #409 In progress when readiness implementation begins
  (after workload checkpoint moves to review); default-guard cleanup afterwards.

## C — coordinator and subsequent remaining work

Accepted additional checkpoints: full physical header capture/admission and
online replacement/retained preimages; compact standalone format2 recovery
(LIVE format1 unchanged); Unicode root containment and periodic watch content
verification; direct activation certificate-policy propagation. Exact85 initial
CommonModule native/own acceptance is integrated; forms/extensions/repeated85
and general worker/LIVE remain open. Each checkpoint has scoped immutable review
and quick gates; historical full wave2 gates do not cover subsequent Rust code.

- [ ] C1 Maintain visible owners/three current issue tracks/read-back, resolve
  shared-file ownership and independently review immutable source/raw proof.
- [ ] C2 Implement derived Files help/search rebuilding (#347) after a measured
  codec/index contract; current raw/inflated three-row differences are explicit.
  Read source collectors and native dependency inputs before proposing a codec.
- [ ] C3 Complete Params descriptor native-twin matrix (#418), generic F8 row
  header/recovery replay and remaining F6-F17 review; start each visibly when
  an active issue slot becomes available. Keep measured vs inherited gaps distinct.
- [ ] C4 Integrate reviewed commits without losing wave2, then one appropriate
  combined full-gate run and actual current Windows/Linux CI/E2E; rewrite PR #420
  and retain frozen head/binary/SBOM/proof/bundle on F.
- [ ] C5 Audit every milestone criterion (#344 included), complete remaining
  dependencies and actual operational/recovery documentation. No blanket closure,
  move, tag or release merely because a partial wave passes. Continue the work.
