# Test suites

The repository has explicit portable, research-corpus, and live MSSQL test
boundaries. CI selects them through Cargo packages and features; it does not
maintain a list of tests to skip.

## Merge gate

Nothing in the gate needs SQL Server, a 1C installation, a platform
executable or a local research corpus: the tests read `tests/fixtures` and
build what else they need in the system temporary directory. The offline
matrix of `.github/workflows/offline-e2e.yml` runs on Linux x86-64 and ARM64,
Windows and macOS ARM64; its Linux and Windows lanes are required. Test
builds keep line tables only (`CARGO_PROFILE_DEV_DEBUG=line-tables-only`).
The test commands of each lane:

```text
cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings   # Windows; Quality on Linux
cargo test --locked --workspace --exclude ibcmd-rs
cargo test --locked -p ibcmd-rs --test '*' --no-default-features
cargo test --locked -p ibcmd-rs --lib --no-default-features
```

`cargo xtask ci` runs the same commands locally.

A pull request runs only the lanes its changed files can affect
(`scripts/ci_scope.py`): Markdown that no source, test, profile or tool
names, and `editors/`, skip every Rust step; changes confined to `src/` and
the root's integration suites skip the crate tests; changes confined to the
integration suites (`tests/*.rs`, `tests/support/`) and `tests/scripts/` skip
the root library. Pushes to master and manual runs run every lane. Root
examples are compiled by the `platform-oracle` lane below.

The research probes and the live MSSQL tests below are behind features and
not part of the gate. `.github/workflows/ci.yml` also compiles the root
package with `platform-oracle` on Windows:

```text
cargo check --locked -p ibcmd-rs --all-targets --features platform-oracle
cargo test --locked -p ibcmd-rs --no-run --features platform-oracle
```

## Root integration suites

`tests/integration.rs` builds the root package's integration suites as
modules of one test binary, so the large root library is linked once rather
than once per suite. A test there is named `<suite>::<test>`:

```text
cargo test --locked -p ibcmd-rs --test '*' --no-default-features            # every root suite
cargo test --locked -p ibcmd-rs --test integration cf_export:: --no-default-features
cargo test --locked -p ibcmd-rs --test integration cf_export::clean_room_cf_exports_known_families_with_an_empty_path -- --exact
```

The suites of that binary run in parallel in one process, so none of them may
change process-wide state, and scratch directories need names of their own
(`common::temp_dir` refuses a tag that two suites use). The filter
`<suite>::` matches test names by substring, so it also runs a suite whose
name ends with `<suite>` (`extension_export::` runs `file_extension_export`
too). A suite that runs process-global engine operations in the test
process (cancellation scopes, offline rows, the offline compile,
stored-object views) or needs its own `main` keeps a `[[test]]` target of
its own in `Cargo.toml` and runs with `--test <suite>`. The manifest sets `autotests = false`; the test
`every_suite_file_is_built` fails when a `tests/<suite>.rs` file is built by
no target or by two.

## Temporary directories

A test removes every directory and file it creates under the system temporary
directory, on success and on failure: it holds the path in a guard whose
`Drop` removes it. A `remove_dir_all` at the end of a test does not count,
since a failing assertion skips it. The guards are `common::Scratch`
(`common::temp_dir`, `common::temp_file`) in `tests/`,
`test_scratch::TestScratch` in the root library, and `tempfile::TempDir` or a
guard of the crate's own in `crates/`. Names are unique per run (a UUID, or the
process id with a counter or a timestamp), so the runs of two worktrees on one
machine share no directory but the two kept corpora below.

`IBCMD_TEST_KEEP_FIXTURES=1` keeps the fixtures for debugging; no other value
does, and no gate sets it for the Rust tests.

A run leaves three names on purpose. Two are the editor-server corpora that
the VS Code tests read by name: `ibcmd-external-editor-server-protocol`
(`IBCMD_TEST_ROWS`, `editors/vscode/README.md`) and
`ibcmd-external-editor-server-export`. The third is the empty `ibcmd-rs`,
the parent that the engine's own scratch folders share.

The offline matrix runs its tests with `TEMP`, `TMP` and `TMPDIR` set to an
empty directory and fails when anything else is left there. `cargo xtask ci`
does the same: its cargo test steps get a directory of their own,
`ibcmd-ci-<hash of the worktree path>` in the system temporary directory
(a Unix socket path the tests bind must stay short), which it checks
afterwards and then removes.

## Research corpus suite

Tests behind `research-corpus-tests` are opt-in manual probes. Most depend on
artifacts outside the portable repository test data. The current external
inputs include:

- source-tree scans that read `lab/ssl_*`, `lab/sfc`, or `D:\УХА\sfc`;
- module-blob probes that read the same lab/SFC trees or `E:\ibcmd_lab`;
- MOXEL comparisons and diagnostics that read `lab/ut_ibcmd_*`,
  `E:\ibcmd_lab`, or `.tmp_selected_dp_outdir.txt` (a local, untracked file
  holding the path of a selected export; `.tmp*` is ignored by Git);
- the external Avansovy round-trip diagnostic in `source_audit`.

Three pre-existing, ignored `module_blob` spreadsheet-packing diagnostics use
inline data rather than an external corpus. They live in the same opt-in manual
runner so the `--ignored` selector below selects one coherent lane and nothing
from the default legacy suite.

The similarly named `scans_real_register_family_layouts`,
`scans_real_chart_of_accounts_layouts`, and
`scans_real_chart_of_calculation_family_layouts` tests build synthetic trees in
the system temporary directory. They are portable and remain in the ordinary
root test harness.

Five MOXEL baseline tests use raw inputs committed under `tests/fixtures`, but
their expected `Template.xml` baselines are loaded from an external
`lab/ut_ibcmd_*` tree. Those tests therefore remain research-corpus tests; a
committed raw input alone does not make the comparison self-contained.

All research-corpus tests are both feature-gated and ignored. On a workstation
where every referenced corpus and marker file is provisioned, run only that
class with:

```text
cargo test --locked -p ibcmd-rs --features research-corpus-tests --lib -- --ignored --nocapture
cargo test --locked -p ibcmd-dump --features research-corpus-tests --lib -- --ignored --nocapture
```

The exporter's own probes live in `ibcmd-dump`; the root's feature forwards
to the crate's, and the root keeps the ones that read root code.

The `--ignored` selector is intentional: enabling the feature alone must not
also run the 179 known legacy failures. Some diagnostics still return early
when an optional corpus root is absent, so the research runner is responsible
for validating its prerequisites. Windows-looking paths used only as parser
input or expected values do not access the filesystem and remain ordinary unit
test data.

## MSSQL live suite

`mssql-live-tests` contains tests that invoke `sqlcmd` against local SQL Server
databases and compare their blobs with a provisioned research source tree. They
remain ignored even when compiled so that enabling a feature cannot contact a
database accidentally.

Prerequisites currently include integrated access to `localhost`, databases
`ut_ibcmd` and `ut_ibcmd_sweep_01`, `sqlcmd` on `PATH`, and matching
`E:\ibcmd_lab` sources. Run only the live comparisons explicitly:

```text
cargo test --locked -p ibcmd-rs --features mssql-live-tests live_compares_native_and_packed -- --ignored --nocapture
```

Neither opt-in feature is enabled by default or in ordinary CI.

The built-in SQL client's nine live tests live in `ibcmd-sql`. They retain
their `sql::mssql::live_tests::` names and are run explicitly against the
disposable `tempdb` tables described in that module:

```text
cargo test --locked -p ibcmd-sql --features mssql-live-tests sql::mssql::live_tests:: -- --nocapture
```

The root's `mssql-live-tests` feature forwards to the crate and to
`ibcmd-dump`, whose ignored live test of the storage generation overlay reads
a lab database named by `IBCMD_RS_DYNGEN_DB`; the ignored root blob
comparisons above remain in `ibcmd-rs`.

## Public Coverage corpus

Coverage native inputs, combined configurations and fixture records are pinned
as `tests/fixtures/1c-fixtures` from https://github.com/Untru/1c-fixtures.
Clone with `git clone --recurse-submodules` or run
`git submodule update --init --recursive` after checkout/branch changes.
CI initializes the exact gitlink; it never follows a moving remote branch.

`python tools/coverage/fixture_paths.py` rejects an absent/dirty/wrong checkout,
changed/missing/extra fixture bytes and orphaned local baseline references.
`cargo xtask ci` runs it plus native generator tests from the pinned corpus.
Converter equality baselines remain under `tests/fixtures/coverage`; source-probe
diagnostics remain in `tests/fixtures/coverage-records/source-probe-expectations.json`.
Update the public corpus first, then its gitlink and converter code in a PR.
Changing the corpus does not authorize removing an existing passing expectation.
