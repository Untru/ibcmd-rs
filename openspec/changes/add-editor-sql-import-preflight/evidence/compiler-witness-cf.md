# Focused compiler witness verification

Base: 914e9e8c95c35d074a807cfc08f82bb77f0e445c. This record covers only
opt-in raw compiler evidence and the library diagnostic consumer. No accepted SQL
profile, DB I/O, RPC dispatch, publisher or native/VM execution is delivered.

## Verified scope

- Raw old export report and descriptor candidates captured before selected edits.
- New raw report from the exact mandatory-verified compiled CF, complete raw key
  coverage before the unchanged adapter's service filtering, and artifact hashes.
- Exact/canonical/case duplicate refusal, unsafe path refusal, missing/opaque/
  failed/profile/stale binding controls, actual internally resolved selection.
- Private raw evidence and redacted counts/digests/refusals. Supported entries
  and reduced canonical maps never establish owner or completeness authority.
- Existing FileDb prepare uses the no-capture branch. No extra export/refusal is
  introduced there; its unchanged partial-import regression suite passed.

## Commands and attribution

Rust 1.95.0, external target/cache/TMPDIR, KEEP1. Build jobs were 2 for focused
suites and 1 for the final exact smoke. Commands ran only in allocated slots.

| Command scope | Result |
| --- | --- |
| Root library check with real diagnostic consumer | exit0; zero owned-file warnings; 98 warnings elsewhere |
| `cargo test --locked -p ibcmd-rs --lib import_plan:: --no-default-features` | 15 passed;0 failed |
| `cargo test --locked -p ibcmd-rs --test editor_partial_import --no-default-features` | 14 passed;0 failed |
| `cargo test --locked -p ibcmd-rs --test editor_sql_import --no-default-features` | 29 passed;0 failed |
| One exact provided-view diagnostic smoke after oracle tightening | 1 passed;0 failed |
| OpenSpec strict and scoped formatting/diff checks | passed |

The external receipts issue-382-diagnostic-check-end.json,
issue-382-raw-witness-runtime-isolated-end.json and
issue-382-diagnostic-exact-end.json bind the precise source hashes and commands.
The exact smoke changed only a test assertion; reversing its block restores the
broader suite's source hash. No broad rerun or full CI result is inferred.
Earlier compile/test failures remain retained externally. No failure was waived
as pre-existing, no warning suppressed and no compiler process guard reset.

The CF input is the public tiny.1CD fixture through the existing reader/exporter,
configured for platform 8.5.1. The synthetic diagnostic observed one changed
file, one output pair, three old/new descriptor candidates and the explicit
owner-completeness refusal. SQL/native flags and publication are false. No raw
application XML/BSL/CF or private names are included here.

## Remaining gates

The metadata compiler remains once-per-process. Fresh test children do not prove
reusable production compilation. Exact engine owner/parent/role completeness,
pre-pruning effective-view provenance, service/versions mutation authority,
same-session SQL CAS/commit outcome handling, actual synthetic 8.3 SQL/native
acceptance and full repository/CI gates remain pending. This does not complete
issue382 and cannot authorize publication of any configuration changes.
