# ibcmd-rs — notes for coding agents

ibcmd-rs reimplements 1C:Enterprise's native `ibcmd`: configuration export,
import and apply between XML and SQL Server or file infobases, and `.cf`/XML
conversion without the platform. Users read `README.md` (Russian). The
workflow, the AI policy and the review process are in `CONTRIBUTING.md`
(Russian); test
boundaries in `docs/testing.md`; merge and release gates in
`docs/release-criteria.md`; parity measurement in `docs/parity-protocol.md`.

## Commands

```text
cargo xtask check      # inner loop: fmt, clippy (crates/), root compile check, tests of changed crates/suites
cargo xtask affected   # the test commands your diff needs, and what else to run
cargo xtask ci         # local mirror of the required merge checks; before a PR leaves draft
cargo fmt --all
cargo test --locked -p ibcmd-filedb <name>                               # one crate, one test
cargo test --locked -p ibcmd-rs --test cf_export --no-default-features   # one root suite
cargo test --locked -p ibcmd-rs --lib <module> --no-default-features     # root unit tests, filtered
cargo build --locked --profile iter                                      # binary for corpus runs
```

- xtask prints one line per step and only the tail of a failing step; full
  logs are in `target/xtask/<step>.log`. Read the log before guessing.
- `clippy -D warnings` covers `crates/*`. The root package (`src/`, `tests/`)
  has clippy debt and is compile-checked only; add no new warnings there.
- Run `cargo xtask ci` after `git add`: the corpus guard checks tracked files
  only, so a new untracked file escapes it locally and fails in CI. It also
  refuses symlinks anywhere in the repository.
- Plain `cargo test --workspace` is not a gate: the legacy root unit harness
  has known failures (docs/testing.md). Run the suites that cover your change.
- `rust-toolchain.toml` pins Rust 1.95.0, the toolchain of CI and releases;
  rustup installs it on first use. Do not override it except for fuzzing
  (`cargo +nightly fuzz`).
- Debug builds for tests, `--profile iter` for corpus runs, `--release` only
  for release checks. One cargo command at a time per target directory;
  parallel agents work in separate worktrees.
- Clean up after your tests and builds. A debug `target/` grows by tens of
  GB per worktree (a shared one reached 140 GB), and a full disk breaks every
  build on the machine with linker crashes. When the work in a worktree is
  done, run `cargo clean` there or remove the worktree; delete the infobases,
  native exports and corpus outputs your runs created. A target directory
  another cargo process uses (`pgrep -a cargo`; PowerShell:
  `Get-Process cargo, rustc`) is not yours to clean. Check free space
  (`df -h`; PowerShell: `Get-PSDrive -PSProvider FileSystem`) before a long
  build or corpus run.

## The loop

1. Take one Backlog or Ready issue with `/claim` and wait for the bot confirmation (see
   Tracking). Work on `feature/issue-<N>-<slug>` in your own worktree.
2. New capability (command, format, route, gate): write the OpenSpec change
   first (skill `openspec-change`). Bug: write the failing test first and see
   it fail for the right reason. A mismatch found on a typical configuration
   (ERP, Бухгалтерия, БСП…) is first reproduced as a coverage variant and seen
   red in `tests/coverage_corpus.rs` (skill `coverage-case`); only then fixed.
   The variant also enters the platform's combined configuration: rebuild it
   with `tools/coverage/combine.py` (the coverage test fails until you do).
3. Change in small steps; after each, run the narrowest test that covers it.
4. `cargo xtask check` until green. A failure is pre-existing only after you
   reproduce it on the merge base and say so, with the command.
5. Before the PR: `cargo xtask ci`, then a fresh-context review (skill
   `review-change`). Comparisons with native `ibcmd`: skill `native-parity`.
6. The PR states What / Why / Evidence. It stays draft until `ci` passes.
   Done means the issue's acceptance criteria hold with evidence, not that
   the code compiles.

## Traps

- The monoliths are 17–80k lines (`src/mssql_dump/{mod,form_body,tests}.rs`,
  `src/module_blob.rs`, `src/mssql.rs`). Never read one whole: use `rg -n`,
  the LSP and ranged reads.
- Slot semantics come from schema-owned names and bounds (`ibcmd-schema`). No
  new numeric slot indices in production parsers and no special cases for
  named objects (a form called `ФормаСписка`): find the rule shared by all.
  `tools/validate-physical-adapter-policy.ps1` fails new literals in
  `src/module_blob.rs` and `src/mssql_dump/`. Refresh its baseline only for a
  reviewed policy, never to hide literals (`tools/physical-adapter-policy-guard.md`).
- A new heuristic needs an EDT writer rule or a full corpus of
  counterexamples, plus a measured win on the full gate.
- Fail closed: an unknown profile, an unexpected opaque record or an
  unclassified loss is an error before anything is published, never a guess.
- Put new self-contained code in a crate under `crates/` (portable, linted)
  rather than growing `src/`.
- `crates/ibcmd-schema/data/*.json` comes from EDT through
  `tools/import-edt-*.ps1`; re-run the importer, do not hand-edit.
  `crates/ibcmd-edt/vendor/morph1c` is a pinned snapshot. Do not edit it
  without explicit maintainer approval of the exact adaptation/path/commit
  scope. Approved changes must preserve upstream provenance, add positive and
  rejection tests, and update only corresponding `LOCAL-FILES.json` entries
  and adaptation notes after semantic review. Existing adaptations or passing
  tests are not approval. Any new path or behavior outside the approved scope
  requires fresh owner approval; no blanket refresh or inherited mismatch cleanup.
  Approved scopes are recorded in `docs/morph1c-local-adaptations.md`.
- Never commit private `.cf`/`.1CD` files, native export trees, raw blobs,
  application XML/BSL, credentials, or reports with application paths.
  Fixtures are clean-room or minimized sanitized fragments. A typical
  configuration's object never becomes a fixture: its mismatch becomes a
  generated coverage variant; only a legacy layout the current platform cannot
  write gets a minimized raw fragment, marked as such.
- Commands that write to an infobase run only against disposable copies.
- No full parity run until the exact raw fixture, extraction on a saved BCP,
  the policy gate and the targeted tests pass locally. Check the binary's
  embedded commit before a long run. README progress figures change only
  from a completed immutable manifest.
- Another agent's "done" is a claim: re-run its commands and read its diff.
- New dependencies affect the SBOM, licences and the release audit: ask
  first. Update one crate with `cargo update -p <crate> --precise <version>`.
- Code comments, commits and PRs describe the code and the evidence, never
  this conversation ("as discussed", "the user asked").

## Tracking

The working board is https://github.com/users/Untru/projects/9. Anyone who
can comment on a public issue may claim it without repository Collaboration
or Project write access:

- Comment `/claim` on an unassigned Backlog or Ready issue. Wait for the bot's final
  success reply and verify Assignees / In progress before starting work or
  dispatching an implementer. A pending receipt is not approval to start.
- The assigned contributor comments `/review #PR` for their open non-Draft
  PR targeting master and explicitly referencing the issue. The bot records
  the PR in Agent and moves the card to In review; this does not run Codex
  Review. Request that separately with `@codex review` on the PR.
- Only the assigned contributor can `/unclaim`; it removes their assignment
  and returns the card to Ready. Never take over another contributor's issue.
- Work through a fork and pull request when repository write access is absent.
  Keep a concrete human operator responsible for agent work.

While command processing is enabled, route tracking changes through the bot.
Before direct owner maintenance of Assignees, Status or Agent, disable this
workflow and verify every running/queued command is terminal; then edit,
read back and re-enable. The API has no conditional field write: a manual
edit inside the read/write interval can be overwritten without detection.
Owner acknowledgement (`ISSUE_BOT_SINGLE_WRITER_CONFIRMED=true`) is required
before commands can mutate tracking. See the maintenance runbook below.

The bot updates and reads back Assignees, Status and its own `Claim bot:`
Agent line; under this protocol it preserves other owners' Agent lines. A failed or unfinished
command may have partially applied: inspect the current assignment and send
one new command to reconcile your own work. Never rerun an old command to
restore state. A sole existing owner may retry `/claim` in Backlog, Ready or
In progress. If the bot is unavailable, ask the maintainer for help. An
unassigned issue outside Backlog/Ready needs maintainer triage; do not grant
yourself access or start silently.
Setup, refusals and recovery: [issue self-service](docs/issue-self-service.md).

Contributors may claim Backlog directly; maintainers may triage into Ready, reconcile unexpected interference,
and mark Done only after acceptance evidence. A draft PR does not complete
its issues. Local plans and labels do not replace Project fields. Read back
manual board transitions and do not mark future work started because it has
a proposed owner. Direct Project editing requires Project write access; it
is not required for the public comment commands.

## Releases

Before every `v*` tag, go through the release's GitHub milestone and check
that every issue is actually done. Close each with a comment naming the
evidence (commit, measurement, test); finish or move (with the reason) any
that is not. No tag while the milestone holds an open or stale issue. Details:
"Milestone check before tagging" in `docs/release-criteria.md`.

## Commits and PRs

- Subject `type(scope): summary` (`feat(filedb): …`, `fix(xml): …`), with
  `#N` when it belongs to an issue. The body says why.
- Commits written with an agent keep a trailer naming it: Claude Code's
  `Co-Authored-By: Claude …`, or `Assisted-by: <tool>:<model>`. The human
  who opens the PR answers for it.
- Never force-push a branch someone else works on. Never merge your own PR
  unless the maintainer told you to for that PR.
- If a rule here was missing or wrong, propose the fix under "Suggested
  AGENTS.md changes" in the PR description; do not edit this file in passing.
