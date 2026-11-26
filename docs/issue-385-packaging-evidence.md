# Issue 385 engineering evidence

Refs #385. Dependency: pinned PR #484 (`8ddfa886b8318d979dc322e5a3b98195e3dd5a57`),
which carries the connection/tree/status foundation from PR #477. No donor branch
was rewritten. Review the issue-385 delta after that pin; merge the dependency first.
Package tooling/lockfile/workflows are owned here; #381/#384 provide feature command
and menu additions independently. The original packaging delta did not change Rust production code; maintainer commit `c1c74850` later preserved the shared backend custody fix in this dependency tree.

## Local acceptance — 2026-10-23

- Locked npm install, esbuild bundle, 33 client unit/real CLI tests, 2 executable
  resolution tests and 2 archive tests passed. Format and diff whitespace checks passed.
- Full VS Code extension-host suite passed: connection lifecycle, real committed-CF
  rows inventory/export and local edits, synthetic lifecycle/six-state failures,
  Explorer/filter, cancellation/stale responses and multi-root removal. Synthetic
  controls are not evidence of a live SQL login.
- Two complete Mac VSIX packages were byte-identical at SOURCE_DATE_EPOCH 1792729235:
  SHA-256 `856816fba05784a78b27d079cff260ae0669bcc1f0b0cbc589b64e8db7831c29`.
  Subsequent metadata-only npm script portability edits do not alter the runtime
  bundle; exact-head Windows CI repeats full packaging comparison.
- Actual installed VSIX acceptance passed in VS Code 1.137.0 and Cursor 1.105.1,
  using separate clean profiles. Tests assert installed extension location, invoke
  the bundled Mac executable, match the four fixture object identities and visible
  tree, then repeat after a machine `ibcmd-rs.path` override. Both hosts exited 0.
- Fixture objects: Configuration, Language.Русский, CommonForm.ФормаСправа,
  CommonForm.ФормаСлева. Unknown status without baseline is expected. No SQL/native
  version equivalence is claimed by this installation test.
- Independent review identified Windows `.cmd` launch, stale bundle and SVG package
  defects; all addressed. Recheck prompted portable Python selection and explicit
  CI Python provisioning, also addressed.

Local retained evidence (all under `/Volumes/Work/Dev/ibcmd-rs-lab`):

- `logs/issue-385-full-host.log`
- `logs/issue-385-final-vscode.log`
- `logs/issue-385-final-cursor.log`
- `tmp/issue-385-final-vscode/installed-u72Sm0/acceptance.json`
- `tmp/issue-385-final-cursor/installed-vUI86Q/acceptance.json`
- `build/issue-385/final-a.vsix` and `final-b.vsix`

The Mac package uses retained `build/issue-380/debug/ibcmd-rs`, built from the pinned
Rust dependency. This is installation evidence, not a production release binary.
CI builds its own Windows release binary with Rust 1.95.0; the release workflow
packages the matching audited Windows release binary. Local new fixture/staging
outputs are retained; no permanent deletion or original ERP access occurred.

## Dependency audit follow-up

The initial vsce 3.6.2 pin reported six high-severity transitive advisories. It was
replaced by exact `@vscode/vsce` 4.0.0 (Node >=22, matching CI). `npm audit` now reports
zero advisories at every severity. No scanner or baseline was disabled. Bundle,
format, executable/archive tests and actual packaging pass with the patched tool.
The retained patched packaging audit is `logs/issue-385-npm-audit-patched.json`.
Two patched complete VSIX files are byte-identical with SHA-256
`6863b55a6048b34eafd46b8ce9cdc0de6e9bbef75bb74333943dfabdd67c6c49`;
see `build/issue-385/patched-vsce-{a,b}.vsix`. This supersedes the earlier package
hash above. Installed checks also passed again with this patched tool in
`tmp/issue-385-vsce4-vscode/installed-9RWXtY/acceptance.json` and
`tmp/issue-385-vsce4-cursor/installed-xknz3f/acceptance.json`; logs are
`logs/issue-385-vsce4-{vscode,cursor}.log`. The normalized epoch was the timestamp of the preceding engineering
commit (0bf5b256); changing SOURCE_DATE_EPOCH intentionally changes ZIP bytes.

## Remaining owner/external acceptance

No release was dispatched or created, and Marketplace/Open VSX publication and
publisher credentials were not configured. The owner must perform publication and
release gates. No VSIX was installed from an already published release; the test
installs the locally built candidate. Windows acceptance is provided by the new
CI job, not by this Mac run. Screenshot capture was unavailable because Computer
Use permissions were not granted; README uses an explicitly labeled illustration.
GitHub Project 9 updates are unavailable to this account, per coordinator evidence;
no denied board write was retried. Keep the full issue open and this partial-scope
PR Draft until the owner's remaining acceptance is satisfied.

## Windows checkout formatting regression

Current-head Windows job [120334143270](https://github.com/Jimmo910/ibcmd-rs/actions/runs/39929553539/job/120334143270)
failed `npm run format:check` at a3488081: `scripts/build.cjs` and
`scripts/python.cjs` were converted to CRLF because the editor attributes covered
only .ts/.js/.json. The same failure was reproduced locally with
`git -c core.autocrlf=true checkout-index` in a retained external snapshot; both
scripts contained CRLF and Prettier exited 1.

Adding `.cjs text eol=lf` makes the identical forced checkout pass Prettier. A new
package regression performs that checkout and asserts LF without CRLF in both
scripts. All three Python archive/checkout tests, two Node executable tests,
`format:check` and whitespace checks pass. Independent read-only review found no
blocker. Formatter settings and checks are unchanged. Before/after snapshots are
retained under `tmp/issue-385-crlf-{before,after}-*`; the failed Windows job log is
`logs/issue-385-windows-a3488081.log`. Windows exact-head CI remains the final proof.

## Actual Codex findings follow-up

Read the actual inline findings from the review of a3488081, rather than treating
an automation summary or later sampling error as a completed clean review:

- [4358230173](https://github.com/Untru/ibcmd-rs/pull/508#discussion_r4358230173):
  file-db Edit could preserve `db-path` while adding `server`, yielding invalid
  settings. Reused reviewed source and unit-test portions of #477 donor
  `b6aca400fd7fddc556b7a5dc20aa579e94127651`; adapted the connection harness to retain
  this branch's object-view and executable resolver mocks. File-db/unknown sources
  are excluded, the writer guards `db-path`, and supported inherited server/rows
  semantics remain intact. Shared Windows env and intentional-dispose corrections
  are retained from the same reviewed donor. Donor branches were not modified.
- [4358230185](https://github.com/Untru/ibcmd-rs/pull/508#discussion_r4358230185):
  without native baseline, status used active but tree used main. Tree now selects
  main only when the native baseline exists, matching existing server dispatch.
  The #484 owner confirmed this contract. No Rust or protocol change was needed.
- [4358230193](https://github.com/Untru/ibcmd-rs/pull/508#discussion_r4358230193):
  both editor workflow filters now cover `src/**`, `crates/**`, Cargo manifests/lock,
  editor-server tests and their common/fixture dependencies. Parsed both trigger
  lists and verified nine concrete backend/client/dependency path cases, including
  stored_objects, native_export_status, mssql_dump and ibcmd-xml. Retained parsed
  inputs: `tmp/issue-385-editor-workflow-triggers.json`.

Before-source tests reproduced both the missing file-db guard and active/main
mismatch in `logs/issue-385-review-regressions-before.log`. Final integrated checks:
42 client unit/real CLI tests, 2 executable tests, 3 packaging/Windows-checkout
checks, formatting and whitespace all pass. Independent integrated review found
no blocker; original assertions and the 7825043c LF correction are preserved.

The planner-granted isolated VS Code host run exited 0 on 2026-10-23 10:11:53 UTC.
It verifies a mixed real-server MSSQL/file-db list: the picker excludes file-db,
TOML stays byte-identical, and real `settings show` still accepts it. The full
existing connection/tree/cancellation suite also passed. The host slot was released;
no Cargo/native/SQL or ERP run started. Retained evidence:

- `logs/issue-385-review-client-green.log`
- `logs/issue-385-review-host.log`
- `tmp/issue-385-review-host/host-IgcFAL/workspace/file-db-edit-evidence.toml`

This is local client/server-contract evidence, not a new native CONFIGSAVE oracle
or live SQL login. Exact-head fork CI must still validate Windows packaging and
installation. Existing publication, screenshot and release acceptance limits remain.
No additional bot review request or manual workflow rerun was sent.

## Shared current-crash follow-up

Reused the three-file donor `a3ede3682b99efde6d9caee730bfa736acb78c3e` from #477,
over the previously integrated b6aca400. A current startup/connect crash could
advance the selection generation before its catch handler reported the error,
leaving a disconnected state with no visible explanation. The existing current
selection callback now reports one sanitized error before invalidation; this
branch's object clearing, executable resolver and protocol wiring remain intact.
A stale old connection still cannot notify or replace the new selection.

The donor's password test records callback observations and asserts afterward,
preventing the command handler from swallowing test assertion failures. Reused
current/stale crash tests and the actual missing-executable transport regression.
Before applying the runtime fix, the current startup case reproduced zero visible
errors rather than one. After integration: all 45 client tests, five packaging/
checkout checks, esbuild bundle, formatting and whitespace pass. Independent
integrated read-only review found no blocker. Retained logs:

- `logs/issue-385-crash-followup-before.log`
- `logs/issue-385-crash-followup-green.log`

No new local host, Cargo, native, SQL or ERP process was started. Fresh host coverage
for this follow-up is explicitly pending the current #485 → #357 → #483 queue;
earlier host runs do not validate this new path. Do not attribute donor host
coverage to this branch. Ordinary exact-head CI and owner acceptance remain pending.

### Interrupted Windows installed-host investigation (2026-10-23)

Fork run `39939946627`, exact head `b273223e7b169e2fd19f476c7bd44b25abf15663`, was deliberately cancelled after its assigned Windows installed-host step remained active from 12:06:36 to 13:54:06 UTC. This is an interrupted diagnostic run, not an acceptance pass or an assertion failure. Linux passed; Windows package tests, formatting, 45 client tests, release binary build and duplicate VSIX hash comparison completed before the stall.

Terminal job logs confirm successful VSIX installation at 12:07:12 UTC, GUI extension-host startup at 12:07:14 and fixture inventory/export at 12:07:16. The unresolved wait is later in the suite, not the runner queue or installer. The old harness did not record finer phases, so a specific configuration/picker/activation wait is not yet proven. Retained local log: `/Volumes/Work/Dev/ibcmd-rs-lab/logs/issue-385-interrupted-windows-job.log`.

The follow-up keeps every installed-tree and override assertion. An independent supervisor enforces an eight-minute whole-run deadline, including synchronous child hangs, and terminates only its spawned process tree. Installer and fixture CLI calls have 120/60-second deadlines; the workflow step has a 15-minute backstop. Phase JSONL and isolated editor logs are uploaded on failure, along with already-built VSIX artifacts. Local subprocess regressions cover success, nonzero exit, spawn failure, synchronous infinite-loop termination and descendant termination. Independent review found no blocking correctness issue; its phase-granularity suggestion was addressed with separate markers for every settings update. Windows process-tree termination and full installed acceptance still require the new-head CI run; no unchanged-code rerun was requested.

### Installed picker API identity regression (2026-10-23)

Exact-head Windows job [120465160770](https://github.com/Jimmo910/ibcmd-rs/actions/runs/39968660045/job/120465160770) failed its eight-minute watchdog on `c1c74850`. The retained 17-file diagnostic artifact ends at `select-connection`, after all four settings writes and extension activation. Package installation and reproducibility passed; the Windows VSIX SHA-256 was `b45a649fa22d5b83fd2d024cf61688f337fd0fe4d7c55f37e1f2f98a96c89582`.

A local installed-host probe on the same pinned VS Code 1.95.3 reproduced selection waiting for input: the installed extension and external driver have distinct `vscode` API/window objects, and the driver-side mocked picker received zero calls before the 15-second diagnostic deadline failed. The suite had patched the external driver's API instead of the installed extension's API. This was a test harness UI wait, not evidence of a backend or installer failure. Red log: `logs/issue-385-picker-195-red.log`; downloaded Windows diagnostics: `logs/issue-385-c1c74850-windows-installed`.

The fix obtains the installed extension's API with Node `createRequire` at its actual bundled entrypoint and patches/restores that window namespace. Assertions now require exactly the expected `lab/offline` picker item and exactly one invocation per selection, including after executable override. All original inventory, visible tree, object identity and override assertions remain. The eight-minute watchdog and CLI/workflow deadlines are unchanged. Independent read-only review found no blocker.

The patched installed-host test on VS Code 1.95.3 passed, including two observed picker invocations and all four fixture objects. Green log: `logs/issue-385-picker-195-green.log`; retained acceptance: `tmp/issue-385-picker-195-green/installed-8emTfh/acceptance.json`. The VSIX uses the current unchanged runtime bundle and retained pinned-dependency Mac binary `build/issue-380/debug/ibcmd-rs`; it does not claim compilation of the newer shared Rust custody donor. This root-cause comparison used isolated external profiles/extensions/workspaces and retained every fixture. Exact new-head Windows CI remains required for that platform's final acceptance.

The same patched installed suite also passed in actual Cursor 1.105.1 with a separate external profile, two observed picker invocations, exact fixture tree and executable override. Evidence: `logs/issue-385-picker-cursor-green.log` and `tmp/issue-385-picker-cursor-green/installed-7PQAgG/acceptance.json`. Both hosts exited 0, and no task editor processes remained after completion. Local host slot is released; no Rust build was needed. Six Node checks, three Python checks, TypeScript compilation, formatter and diff checks also passed after the fix.
