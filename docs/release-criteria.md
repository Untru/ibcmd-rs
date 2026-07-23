# Standalone release criteria

This document defines the fail-closed merge and release gates for the portable
XML/CF converter. A release is the default-feature-disabled Rust binary and its
audited archive. Installed 1C platform tools, EDT, Java, and the research-only
platform oracle are outside that boundary.

## Required merge checks

The `Offline E2E` workflow is the source of merge evidence. Repository branch
rules must require all four named checks:

- `OpenSpec (strict)`;
- `Quality`;
- `Offline matrix (Linux)`;
- `Offline matrix (Windows)`.

The matrix fetches the locked dependency set once and then sets
`CARGO_NET_OFFLINE=true` for compilation, tests, release build, and SBOM
generation. It runs:

1. `cargo check --locked --workspace --all-targets --no-default-features`;
2. all tests in the standalone workspace crates;
3. all root integration targets (`--test '*'`) with default features disabled;
4. clean-environment XML to CF, CF to XML, same-profile, and cross-profile
   conversion for the registered Format15/Format16 and XCF 2.20/2.21 routes;
5. malformed/truncated input and unknown-profile cases, which must fail before
   publication;
6. the evidence-derived compatibility matrix and bootstrap manifest checks;
7. the default `LossPolicy::Error` downgrade case, which must report the exact
   loss and block publication;
8. a release-binary boundary audit with an empty child-process `PATH` and all
   known 1C/Java discovery variables removed.

The legacy root unit harness contains database-export research tests and is not
part of the standalone product contract. Its targets are compile-checked; the
portable root behavior is covered by integration suites. Live database and
external-corpus features remain opt-in and are never enabled by these gates.

GitHub Actions can report these checks but cannot make them mandatory by
itself. After the workflow exists on the default branch, the branch protection
rule or repository ruleset must require the four check names above and reject
force-push bypasses for ordinary contributors.

## Release gate

`Standalone Release` runs for `v*` tags and by explicit manual dispatch. A
manual run produces artifacts but does not publish a GitHub release. A tag run
publishes only after the gate and both target builds succeed.

The gate rejects a tag or input version that differs from the version in
`Cargo.toml` after removal of an optional leading `v`. It then repeats strict
OpenSpec validation, formatting, standalone linting, standalone workspace
tests, and all root integration suites.

Each target build uses:

- Rust `1.95.0`;
- the checked-in `Cargo.lock`;
- `--no-default-features`;
- a target-specific dependency fetch followed by `CARGO_NET_OFFLINE=true`;
- the source commit time as `SOURCE_DATE_EPOCH`.

The supported release targets are currently
`x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`. A new target is not a
supported release target until it is added to both the offline and release
matrices and its green evidence is checked in.

## Artifact contract

Every target produces three externally published files:

- `ibcmd-rs-<version>-<target>.zip`;
- the matching `.zip.sha256` sidecar;
- `ibcmd-rs-<version>-<target>.sbom.cdx.json`.

The archive contains exactly one versioned root and this allowlist:

- `ibcmd-rs` or `ibcmd-rs.exe`;
- `README.md`;
- `compatibility/matrix.json`;
- `compatibility/matrix.schema.json`;
- `sbom.cdx.json`.

No DLL/shared library, Java archive/class, EDT/OSGi payload, vendor executable,
or hidden runtime can be added without making `scripts/audit_release.py` fail.
The audit also rejects platform-oracle markers and commands in the default
binary and verifies that the archived binary and SBOM exactly equal the audited
files.

The CycloneDX 1.5 SBOM is generated from Cargo's locked, normal-dependency
graph. Components and dependency edges are sorted, registry checksums are read
from `Cargo.lock`, and the target/default-feature coordinates are recorded.

The ZIP writer normalizes text line endings, sorts member names, stores fixed
Unix modes, uses no variable compression metadata, and takes its timestamp from
`SOURCE_DATE_EPOCH` (or the current Git commit). It serializes the archive twice
in memory and refuses output if the bytes differ. The SHA-256 sidecar is then
verified against the final archive.

## Local reproduction

From a checkout with the locked crates already fetched:

```powershell
$env:CARGO_NET_OFFLINE = "true"
cargo fmt --all --check
cargo check --locked --workspace --all-targets --no-default-features
cargo test --locked --workspace --exclude ibcmd-rs
cargo test --locked -p ibcmd-rs --test '*' --no-default-features
cargo build --locked --release --no-default-features

python scripts/generate_sbom.py `
  --manifest-path Cargo.toml `
  --output target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.sbom.cdx.json `
  --target x86_64-pc-windows-msvc

python scripts/package_release.py `
  --binary target/release/ibcmd-rs.exe `
  --sbom target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.sbom.cdx.json `
  --output target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.zip `
  --version 0.1.1 --target x86_64-pc-windows-msvc --repository-root .

python scripts/audit_release.py `
  --binary target/release/ibcmd-rs.exe `
  --sbom target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.sbom.cdx.json `
  --archive target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.zip `
  --checksum target/release-artifacts/ibcmd-rs-0.1.1-x86_64-pc-windows-msvc.zip.sha256
```

On Linux, use the Linux target coordinate and binary path. Strict specification
validation is reproducible with:

```text
npx --yes @fission-ai/openspec@1.6.0 validate build-offline-converter --strict --no-interactive
```

## Adding a platform or XCF version

A new version starts unsupported. Add its exact profile and capabilities, codec
or guarded migration edge, clean-room fixtures, green repository evidence, and
compatibility-matrix route. Extend the offline matrix before changing a route
to `verified`. Missing evidence, an unknown profile, an unexpected opaque
record, or an unclassified loss must continue to fail closed.
