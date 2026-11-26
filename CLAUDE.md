# ibcmd-rs — notes for Claude Code

The project notes for every coding agent (Codex, Claude Code and others) live
in `AGENTS.md`; keep them there so there is one copy.

@AGENTS.md

## Claude Code specifics

- `.claude/settings.json` runs `cargo xtask hook-rustfmt` after every edit of
  a `.rs` file and `cargo xtask hook-stop` when you finish a turn with Rust
  changes against `origin/master`. A Stop failure comes back to you: fix it,
  or reproduce it on the merge base and say so.
- Skills `verify-change`, `review-change`, `native-parity`, `coverage-case`
  and `openspec-change` live in `.agents/skills/`; the entries in
  `.claude/skills/` only point there (the corpus guard refuses symlinks).
- The `critic` subagent (`.claude/agents/critic.md`) does the fresh-context
  review of `review-change`.
- With the `rust-analyzer-lsp` plugin installed, prefer its LSP tool for
  definitions and references in the large files over reading them.
