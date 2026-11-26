# Issue 381 acceptance evidence — 2026-10-23

Dependency base: PR #484 head `8ddfa886` (includes connection UI #477), merged
with native export #485 head `218324cb` at `8f55a62e`. Donor branches/PRs were
not modified. The file database read backend inherited through #484 originates
in #481. No foreign #379 / PR #495 implementation is copied.

## Verified

- Rust 1.95.0, offline/locked, no default features, CARGO_BUILD_JOBS=2:
  `editor_export_plan`, `editor_file_db`, `editor_native_status`,
  `native_export_commands`, `native_export_status`: 39 passed; one optional
  private corpus test ignored. No failing tests. Fixtures retained.
- JavaScript/TypeScript client: 47 passed, zero skipped, with the actual built
  binary. Includes dirty object/inventory buffers, warning cancellation,
  explicit overwrite, untracked/ignored/staged paths, nested Git diff.relative,
  disk edits during confirmation, non-Git files, symlinks/hard links,
  case-insensitive dirty/Git paths, alias collision refusal, original backups
  and preservation of unrelated files.
- Actual installed VS Code host, unique external profile/workspace:
  dirty TextDocument refused; real stdio group export matched the native tiny
  module and ConfigDumpInfo bytes; database unchanged; commands registered.
  Final run exited 0 (`ISSUE381_HOST_PASS`).
- Fresh native `/opt/1cv8/8.5.1.1592/ibcmd` via Rosetta, explicit disposable
  `--db-path`, external `--data` and `--temp`: tiny selection and add/change/
  delete/rename cases matched native full-export byte trees and inventory.
  All five database files unchanged; unrelated source README preserved.
  Native full exports took 5.438–8.627 s; RPC plans 0.049–0.080 s on these tiny
  fixtures. These timings do not establish ERP performance.
- Rust formatting, Prettier, strict OpenSpec and unchanged physical-adapter
  policy baseline validated. Independent read-only review findings addressed:
  deleted descriptor removal; no whole-tree source scan; inventory hash/parser
  identity; staged diff.relative paths; concurrent save rechecks; non-Git
  warning; filesystem case aliases; hard links; hidden unrelated notes.

External retained logs: `logs/issue381-rust-focused.log`,
`logs/issue381-js-tests.log`, `logs/issue381-export-host-final.log`,
`logs/issue381-native.log`, `logs/issue381-policy.log` under the lab root.
Native aggregate manifest:
`fixtures/issue381-native-1792745781842521000/summary.json`.
Only repository-owned tiny fixtures were used. No private ERP XML/CF/data was
published or read by this acceptance run; original ERP is never a write target.

## Remaining acceptance

Exact-head remote CI is recorded in the Draft PR checks. ERP UH document latency
on an approved representative disposable fixture and live SQL selection/export
remain unverified. Case-only filesystem write/remove aliases are refused before
mutation and require resolution before retry. Interrupted publication is not
transactional; retained original bytes support recovery, and inventory is last.
The PR remains Draft and uses `Refs #381`; this evidence does not close the issue.
Project 9 writes were denied to the parent account; fields may remain stale and
were not retried through another route.

## Representative ERP selection and read-cost regression

Supplemental read-only evidence uses the authorized external disposable ERP
2.5.27.70 copy, not ERP UH. A document selected by median descriptor XML size
(79,293 bytes among 747 documents) owns 14 exported files / 289,495 bytes.
The fresh native 8.5.1.1592 recursive selected export took 31.720 s including
process startup. The initial debug RPC reached the unchanged 180 s bound
before staging files. Its failed artifacts and native reference are retained.

The clean release baseline at `babdfeb7` took 43.985 s for the same RPC
selection, excluding separately measured initialization (0.437 s) and connection
(3.703 s). Its 14 assets matched the fresh native selection byte for byte, and
its inventory matched the retained native-query-bound inventory. The native
query was retained, not freshly rerun. The database, selected source files and
source inventory were unchanged. Release binary SHA-256:
`0cba0c203d50cdfee3297540c55ecfa437ef4fad43391606ba3ba6f315edaa89`.
These differently bounded timings are not a native speedup claim.

A discriminating regression counted six full raw CONFIG/CONFIGSAVE loads during
one selected request; the converter already emitted only the selected module
files. The operation-local raw-row scope reduces that count to two: one shared
immutable read for planning/conversion and one independent final inventory read.
It does not cache SQL reads or change active-generation/CONFIGSAVE publication.
The guard is thread-bound, refuses nesting, checks typed cancellation and uses
canonical data-file identity; other databases cannot replace its rows.
Six controls cover the work count and actual converter file set, aliases and
cross-database isolation, error/cancellation cleanup, completed/incomplete stages,
and a mid-operation database mutation rejected before inventory publication.
The existing dynamic-generation fixture additionally compares scoped/fresh rows.
The red six-versus-two result is retained alongside the passing controls.

Private evidence stays under external lab fixtures/logs. No document names,
configuration XML/CF, or business data are included in this repository.
The patched release measurement is recorded in the Draft PR after its quiet run;
ERP UH latency and live SQL acceptance remain open.

## Shared native-export fixes integrated after the measured candidate

The stack merges donor #485 head `53c35614` (functional commit `1b3bc824`)
by ancestry, retaining its commits and evidence. Its actual incremental-selection
refusal, strict object output guards and query failure reports supplement the
older dependency. Editor changed-object fallback still explicitly requests a
full conversion for rename/removal. The operation-local FileDb rows and drop
before independent final inventory remain unchanged. The patched ERP timing
belongs to `aeba3d16` and its preserved binary; the baseline remains `babdfeb7`,
and the native timing was captured separately. None was rerun or relabeled as
measurement of this later merge. Post-integration local gates and exact-head
CI must be reported separately before claiming this merge validated.

Post-integration local gates at merge `a1f39a9f`: 10 selected unit checks passed
(scope, dynamic generation, native actual-selection refusal and empty output),
42 integration tests passed across the five export targets, one optional private
corpus test ignored. All-target root check, Rust formatting and the unchanged
physical-adapter policy guard passed. TypeScript/JS: 47 passed, zero skips.
Actual installed Code host on a fresh external profile (`tmp/h381m`) exited 0:
dirty buffer refusal, real RPC group export, native module/inventory bytes and
DB immutability. Fixtures and logs are retained; no fresh native or ERP timing
was run. Logs: `logs/issue381-donor-integration-{units,focused,check,policy,js,host}.log`.
The final evidence-only follow-up does not change the tested Rust or JS sources.
