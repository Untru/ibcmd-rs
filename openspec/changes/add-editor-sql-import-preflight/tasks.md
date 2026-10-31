# Tasks

- [x] 1.1 Inspect current #382, merged #481 and verified current master.
- [x] 1.2 Record bounded SQL acceptance, immutable-plan and transaction contract.
- [x] 2.1 Add clean-room fail-before-write regression cases (#382).
- [x] 2.2 Implement bounded SQL preparation using existing compiler/verifier.
- [x] 2.3 Implement same-session identity/readset/ConfigSave CAS publication,
  retaining all unselected staging; keep capability closed until verified.
- [x] 2.4 Coordinate server token routing without changing FileDb behavior.
- [x] 3.1 Run allocated focused tests and independent review.
- [x] 3.2 Through the authorized operator, verify disposable SQL module/form
  stage -> native apply -> export and unchanged neighbour/staging.
- [x] 3.3 Run required current-head quality/CI gates; publish honest evidence.

## Foundation checkpoints and remaining broader work

F.4 and F.7 remain open for the broader profile/concurrency and owner cohorts.
Their accepted singleton CommonModule/Document FormModule portions are covered
by F.13–F.20 and criteria 2.3/3.2 above; these open broader items do not block
review of this PR. Pending statements in earlier completed checkpoints below
are historical and are superseded by F.19/F.20 within the published scope.

- [x] F.1 Mechanically extract neutral compiler planner with FileDb API preserved;
  donor 59a4a8f4, independently reviewed; original 2+14+6 narrow tests passed.
- [x] F.2 Bounded original-source admission and physical readset arithmetic;
  checkpoint 52ea775c, 14 synthetic integration tests passed.
- [x] F.3 Single-request read-only snapshot adapter and recording contract;
  23 combined integration tests and library check passed after reviewed fixes.
- [ ] F.4 Measure exact SQL/schema/header profile and prove actual acquisition
  locking, empty-resultset numbering, interruption and concurrent-writer controls.
- [x] F.5 Owned structural body-coverage invariant and omission/expansion/budget/
  cancellation controls: 27 combined tests and library check passed; not wired
  to shared effective-view policy or SQL routing and not an ownership proof.
- [x] F.6 Opt-in raw old/new export evidence, bounded collision/selection binding
  checks and real diagnostic consumer; fresh static review, owned-warning-free
  library check, 15 planner +14 FileDb +29 SQL-contract tests passed. Tightened
  refusal oracle passed one exact test; all fixtures and prior failures retained.
- [ ] F.7 Independent complete owner/kind/full-parent/selected-role witnesses,
  origin-aware versions authority and full readset-preserving SQL transaction.

- [x] F.8 Direct snapshot-only existing top-level CommonModule compiler, exact
  registered body-key selection and SQL-origin body-only versions guard; 10
  focused tests and production lib-check passed. No observer dependency or SQL
  route/publisher. See evidence/selected-module-snapshot.md; this is not F.7 or
  completion of SQL preparation/publication.

- [x] F.9 Document-only root-cohort Form module compiler from immutable snapshots;
  exact complete declared owner/child identity checks and unchanged non-module
  body state. 17 selected-module/Form tests and integrated lib-check passed.
  Form.xml and unexamined owners remain unsupported; see
  evidence/form-module-and-cas-contract.md.
- [x] F.10 Pure exact full physical-readset comparison plus 3 mutation/cancellation
  tests; concrete dedicated-session/rollback/COMMIT-unknown contract reviewed.
  This does not implement transactional SQL CAS/publication or complete F.7.

- [x] F.11 Minimal dedicated-session backend seam, unsupported default, owned MSSQL
  connection, poison/terminal rollback guards; 4 pure state/default-refusal tests
  passed and source review cleared at bbb1929; real-session proof remains open.
  The later library transaction primitive is tracked separately below.

- [x] F.12 Library held-lock CAS/DML and explicit transaction outcomes with 11
  recording tests, 29 existing SQL-contract regressions and lib-check passed;
  source review CLEAR. No real SQL session/write or user route was executed.
  See evidence/held-lock-cas-recording.md. Real lock/native writer admission and
  end-to-end acceptance remain unfinished 2.3/3.2, not a completed SQL capability.


- [x] F.13 Actual exact-a666 Windows held-capture/rollback/closure, experimental
  selected CommonModule staging, and native 8.3.27.2342 apply/full-export passed.
  Active CONFIG independently matched staged body/versions; all unselected
  CONFIG rows and four XML files were exact, CDI only selected version changed.
  This is the one-module public-fixture slice, not completion of 2.3/3.2 or
  general writer admission. Nonempty neighbour staging, native FormModule,
  live negative controls and guarded editor SQL routing remain open.

- [x] F.14 Exact-9d04 disposable SQL nonempty-neighbour staging, cancellation
  rollback and stale-plan pre-DML refusal passed one Windows integration test;
  CONFIG and four unselected physical staging rows/headers were preserved.
  The final six-part pending staging was not applied. Native FormModule and
  guarded editor publication remain open; see the same held-lock evidence.

- [x] F.15 Held filesystem CommonModule and Document FormModule preparation uses
  original admitted bytes, existing loader policy and custody revalidation.
  Native-file no-ops preserve raw storage; 27 focused tests, 33 transaction/
  compiler regressions (three live ignored), and production check passed.
  Native FormModule and admitted editor SQL routing remain unfinished.


- [x] F.16 Narrow existing-CommonModule operation admission and private one-shot
  editor plan candidate: canonical extended schema/current-context observation,
  Config/ConfigSave/Params held guards, single-part selected output and finite
  RPC outcomes; 73+6+30+14+14 local tests and library check passed. This is not
  current-head Windows/RPC/native acceptance or general writer admission.


- [x] F.17 Native2342 public Document/Form FILE→CF→SQL-load/apply/export baseline
  verified active root graph, both Documents/MainForm and exact native Module;
  no Form staging/user-route acceptance inferred. See Form/CAS evidence.
- [x] F.18 Original Form layout/raw module-token witnesses and grammar-only held
  preparation integrated with focused source/local checks. Exact disposable Form
  experiment constants are bound; Windows stage/native-cycle execution pending.


- [x] F.19 Exact5be2 public Document FormModule native-seven-field staging then
  native2342 apply/full-export passed. Module187B exact, five unselected XML and
  full rows preserved, CDI selected-only, Save0/FULL RELEASE. Common083a full
  RPC/native cycle also passed; receipts and scope limits are recorded separately.
- [x] F.20 Exact87617 guarded Document FormModule editor RPC and its separate
  native Module3 apply/full-export passed. Constructor/wiring review,105server
  unit tests and14editor_server tests passed; hostedCI7/7 andproduct2/2 passed.
  The final scope is explicit singleton existing module bodies only; other modes
  remain refused. The subsequently approved empty import_evidence.rs catalog
  registration and local CF-verifier/oracle follow-up are recorded in the final evidence.


The checked implementation/runtime items above refer to the two admitted
module-only cohorts, not generic metadata/object/changed imports or an inferred
native-build writer profile. Historical pending statements in foundation receipts
remain dated checkpoints; F.19/F.20 and the final evidence supersede their runtime
status. Broader profile/concurrency/parity measurements retain their original
scope and are not silently certified by these two public fixtures.


Final quality head `07385992b96adadbb2fed50b44f5fb20d3dd43d5` passed
local `cargo xtask ci`19/19, hosted CI40301563495 seven jobs and
VSCode/product40301490504 both jobs. The final checklist/evidence publication
changes documentation only; runtime receipts keep their exact87617 attribution.
No merge or release was performed. Broader foundation/profile measurements
above remain explicitly outside the two accepted singleton-module cohorts.


- [x] F.21 Integrate upstream4ad2c8a3 in the existing PR640 branch, preserve
  source-custody/policy and module-only contracts, register every test target,
  and pass the updated full local CI27/27. Focused SQL/codec/custody/editor tests
  and the retained public-body offline check passed; no live SQL/native cycle
  was repeated. New-head hosted checks are tracked in PR640, not inherited from
  the old07385992 runs. The integration is ready for review after its hosted
  checks passed; no merge/release or broader writer admission is claimed.
