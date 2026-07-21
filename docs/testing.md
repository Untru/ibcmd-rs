# Test suites

The repository has explicit portable, research-corpus, and live MSSQL test
boundaries. CI selects them through Cargo packages and features; it does not
maintain a list of tests to skip.

## Portable release gate

The standalone layer crates (`ibcmd-core`, `ibcmd-xml`, `ibcmd-v8`, and
`ibcmd-cf`) and the explicit root smoke test require neither SQL Server nor a
1C installation, platform executable, or local research corpus. CI runs these
commands on both Windows and Linux:

```text
cargo check --locked --workspace --exclude ibcmd-rs --all-targets
cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings
cargo test --locked --workspace --exclude ibcmd-rs
cargo test --locked -p ibcmd-rs --test portable_root_smoke
```

The root smoke test covers public, platform-free root APIs against a temporary
source tree. It is deliberately narrower than the legacy root unit-test
harness.

The `ibcmd-rs` package still contains the Windows-oriented CLI, legacy MSSQL
integration code, and reverse-engineering assertions. CI checks all of its
targets and compiles its test harness in a separate Windows lane:

```text
cargo check --locked -p ibcmd-rs --all-targets
cargo test --locked -p ibcmd-rs --no-run
```

This is an explicit temporary boundary, not a portability claim for the whole
root package. Plain `cargo test --locked --workspace` is therefore not the
portable release gate.

The bounded BASE-004 inventory run, before the three dormant debug probes were
moved into the opt-in research runner, reported 1,276 passed, 179 failed, and 3
ignored tests. Two failures are `module_blob` serialization assertions; the
other 177 are `mssql_dump` reverse-engineering assertions. The same 179
failures remain visible legacy debt and are not relabeled as portable, live, or
research merely to make CI green.

## Research corpus suite

Tests behind `research-corpus-tests` are opt-in manual probes. Most depend on
artifacts outside the portable repository test data. The current external
inputs include:

- source-tree scans that read `lab/ssl_*`, `lab/sfc`, or `D:\УХА\sfc`;
- module-blob probes that read the same lab/SFC trees or `E:\ibcmd_lab`;
- MOXEL comparisons and diagnostics that read `lab/ut_ibcmd_*`,
  `E:\ibcmd_lab`, or `.tmp_selected_dp_outdir.txt`;
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
```

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
