---
name: review-change
description: Fresh-context review and PR hand-off for an ibcmd-rs change. Runs an independent critic over the diff, resolves its findings, then opens or updates the PR with What, Why and Evidence and moves the board card. Use when a change is ready for a PR, before a draft is marked ready, or when asked to review a branch.
---

# Review a change and hand it off

## 1. Preconditions

`cargo xtask ci` passed, or its failures are proven pre-existing (skill
`verify-change`). The issue and its acceptance criteria are at hand.

## 2. Read your diff as a stranger

```text
git diff "$(git merge-base HEAD origin/master)"...HEAD
```

Remove debug output, commented-out code, drive-by edits unrelated to the
issue, and comments that narrate the change or mention this session. Comments
explain why the code is the way it is, nothing else.

## 3. Independent review

Hand the reviewer the issue, the base revision and your evidence, nothing
more: no suspicions, no hints about what is right.

- Claude Code: the `critic` subagent.
- Other agents: a new session given the instructions in
  `.claude/agents/critic.md` (the text after its front matter).

On `REQUEST_CHANGES`, fix, re-verify and send the result back to the same
reviewer so it keeps its context. Answer a finding you disagree with by
evidence, never by silence. Repeat until `APPROVE`.

## 4. The PR

Branch `feature/issue-<N>-<slug>`; title `type(scope): summary`. Body:

```markdown
## What
<the change in a few sentences>

## Why
<the problem, with the issue>

## Evidence
<commands and results, measurements, pre-existing failures with their
merge-base reproduction, steps not run locally>

AI: <tool and model>, <what it did>.

Closes #N
```

Use `Refs #N` instead of `Closes` when acceptance stays open. Create it as a
draft (`gh pr create --draft --base master --body-file <file>`) and mark it
ready only after `ci` passes and the critic approves.

## 5. The board

As the assigned contributor, comment `/review #PR` on the issue after the PR
leaves Draft. Wait for the bot's success reply and read back In review and
the PR reference in Agent (AGENTS.md, Tracking). Project write access and
repository Collaboration are not required. If the bot is unavailable, ask
the maintainer to perform and verify this transition; do not request broad
access or silently leave stale tracking.

## 6. Rules you missed

If you hit a trap AGENTS.md does not mention, propose the rule under
"Suggested AGENTS.md changes" in the PR body. A rule earns its place when it
is non-obvious, keeps recurring and is specific enough to act on.

## 7. Clean up

Scratch infobases, native exports and corpus outputs go as soon as their
run is reported. When the PR is merged or you leave the worktree, run
`cargo clean` in it, or `git worktree remove` it. Leave a target directory
alone while another cargo process uses it. Say in your final report what you
removed and what you kept, and why.
