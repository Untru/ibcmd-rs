---
name: native-parity
description: Compare ibcmd-rs output with native 1C ibcmd (and EDT) and turn the result into committed, clean-room evidence. Use when a change alters export, import, apply or cf output, when a comparison with native output fails, or when an issue's acceptance asks for a measurement.
compatibility: Needs an installed 1C platform with ibcmd and disposable infobase copies; full SQL Server runs also need PowerShell. None of this exists in CI.
---

# Native parity and evidence

Native `ibcmd` output is the oracle. A difference is a hypothesis about a
rule until a general cause (schema, EDT writer rule, raw-profile cluster) is
shown; it is never fixed per object.

## 1. Build and record

```text
cargo build --locked --profile iter                             # product commands
cargo build --locked --profile iter --features platform-oracle  # research commands (dump-sources, source-diff)
git rev-parse HEAD; sha256sum target/iter/ibcmd-rs
```

Before a long run, check that the binary you start is the one you just built.

## 2. Inputs

- Disposable copies only; commands that write to an infobase never touch
  anything else.
- Native `ibcmd` shares a default work directory. Give each run its own
  `--data=<dir>`, or a parallel run fails with «Рабочий каталог заблокирован
  процессом».
- Native export localizes some default names by process locale. Run both
  sides with the same `LANG` (`LANG=ru_RU.UTF-8` for Russian infobases).
- A standalone server (`ibsrv`) ignores SIGTERM; stop it with SIGINT.

## 3. Run and compare

Every difference that survives classification becomes a coverage variant
before anyone fixes it (skill `coverage-case`).

Write both sides into new directories that are never reused, then compare
the trees (`diff -r`, or `source-diff` of the platform-oracle build). Full
SQL Server runs follow `docs/parity-protocol.md`: immutable run directories
and `parity-manifest.json`. Before a full run, collect every refusal with the
diagnostic mode, cluster them by raw profile and fix them in batches; one
full run per next blocker is too slow.

## 4. What goes into the repository

- Evidence: `openspec/changes/<change>/evidence/<topic>-<yyyymmdd>.md` with
  the commands, platform and ibcmd-rs versions (commit, binary hash), counts
  and tree hashes. Never the trees themselves.
- Fixtures: clean-room (made from an empty or synthetic configuration),
  minimal, under `tests/fixtures/<area>/`; malformed CF inputs as `.hex`
  under `tests/fixtures/malformed/cf/`, asserted by `tests/cf_corruption.rs`.
- Never: private `.cf`/`.1CD` files, full native trees, raw `.0` blobs,
  application XML/BSL or GUID material, client paths, credentials
  (`IBCMD_DB_PSW` stays in the environment).

The README "Проверено" column and progress percentages change only from a
completed immutable run.
