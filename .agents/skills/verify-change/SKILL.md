---
name: verify-change
description: Prove an ibcmd-rs change works before calling it done. Picks the tests the diff needs, runs the check loop, separates your failures from failures that already exist on the merge base, and writes the evidence. Use after implementing or fixing anything in ibcmd-rs, when a check or test fails, or when asked whether a change works.
---

# Verify a change

Done means evidence a reviewer can re-run, not "it compiles". Work through
the steps in order and keep the commands you ran for the report.

## 1. Scope

```text
cargo xtask affected
```

It prints the test commands for the crates and root suites your diff touches,
plus notes for what it cannot map (root `src/`, shared fixtures, manifests,
gates). For a root `src/` change, find the covering suites yourself:
`rg -l '<type or function>' tests/`, and the unit tests next to the module
(`cargo test --locked -p ibcmd-rs --lib <module::path> --no-default-features`).

## 2. A test that fails without the change

A new behaviour or a bug fix needs a test that fails before the change. Run
it before the fix (or with the fix temporarily reverted) and keep the failing
line. A test that never failed proves nothing.

Expected outputs under `tests/fixtures/` change only when the new output is
proven right (native evidence, an EDT writer rule). Never regenerate an
expectation from your own output to make a test pass.

## 3. The inner loop

```text
cargo xtask check
```

Only the tail of a failing step is printed; the full log is
`target/xtask/<step>.log` and its first line is the exact command. Fix the
first error first, re-run that one command, then the whole loop.

## 4. Failures that are not yours

A failure is pre-existing only when it fails the same way on the merge base:

```text
git worktree add ../ibcmd-base "$(git merge-base HEAD origin/master)"
cd ../ibcmd-base && cargo test --locked <the same command>
```

List it in the report with both commands. Never `#[ignore]`, relabel,
feature-gate or delete a test to get green.

## 5. The merge gate

Before the PR leaves draft:

```text
cargo xtask ci
```

It takes tens of minutes on a small machine; run it once at the end, not in
the loop, and after `git add`: the corpus guard reads tracked files only. A `SKIP` line means a tool is missing here (pwsh, npx, python). CI
still runs that step: install the tool, or name the step in the PR as not run
locally. The release build, native micro-corpus parity, SBOM and release
audit run only in CI (`docs/release-criteria.md`); touch release scripts and
you reproduce them by hand from that document.

## 6. Report

The PR's Evidence section, from real runs only:

- each command with its result (`12 passed, 0 failed`), or the measurement;
- the failing-first line of the new test;
- pre-existing failures with their merge-base reproduction;
- what you did not run, and why.
