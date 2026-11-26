# First-wave integrated validation, 2026-10-01

Reviewed implementation source: `e8e885861a1f59a53a83a448bb78cd53eba0aabc`.
Subsequent coordinator commits only record results and task state. Independent
resolution review PASS; all three accepted feature deltas and v0.4 fixes remain.

| Local gate | Result |
|---|---|
| Format | PASS |
| Physical adapter policy | PASS |
| All root targets, no default features | PASS |
| SQL-domain root unit tests | 2358 passed, 0 failed, 2 ignored |
| Drop-in CLI integration | 10 passed, 0 failed |
| Standalone/platform-oracle boundary | 3 passed, 0 failed |
| Strict OpenSpec: build-offline-converter | PASS |
| Strict OpenSpec: add-fast-parity-evidence | PASS |
| Strict OpenSpec: add-selected-cf-extract | PASS |

Raw coordinator logs: `F:\ibcmd\lab\05\wave1\coordinator\integration` and
the sibling `openspec-*.log` files. Builds used four jobs and the sequentially
released, new extension-track target cache; no original worktree/cache was
modified. This local check is not a release build or a full native/RAS/load
matrix. The full portable/offline regressions and release-shaped binaries
are checked by GitHub CI on the published branch/PR. Required merge checks
remain those in [release-criteria.md](../../release-criteria.md).

Published as draft [PR #420](https://github.com/Untru/ibcmd-rs/pull/420).
Use its current checks for live Windows/Linux CI status; local results above
refer to the reviewed implementation source, not a claim that CI completed.

The initial Linux Offline E2E run passed compilation, integration tests,
release-shaped build and native micro-corpus parity, then found a stale audit
expectation: `--dynamic=force` was still classified as unsupported. The audit
now checks the still-unsupported `--extension=E` route, as the Rust boundary
test already does. Exit/message, empty-PATH, oracle-command, binary-marker and
SBOM guards are retained. Local clean-PATH infobase audit passes on the saved
LIVE checkpoint CLI binary; current combined release binaries are checked by
the updated Windows/Linux CI runs.

The branch stays a first-wave milestone checkpoint. No milestone issue is
closed wholesale; no v0.5 tag or release is created. The remaining acceptance
is listed in [roadmap.md](roadmap.md), especially warm/native RAS sessions,
writer workload, wider object coverage, 8.5 and the Params marker rule.
