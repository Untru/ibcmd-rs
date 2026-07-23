# Platform-oracle compile-time isolation

APP-003 removes installed-platform discovery and execution paths from the
default and release-shaped product build. The non-default Cargo feature
`platform-oracle` is the only way to compile the legacy research integrations.

## Compile-time boundary

The following CLI commands and their implementation modules are gated by
`#[cfg(feature = "platform-oracle")]`:

- `probe` and `src/probe.rs`, which search for installed 1C executables;
- `profile-run` and `src/profile.rs`, which start an arbitrary profiled process;
- `dump-sources` and `src/dump_sources.rs`, which execute `ibcmd`;
- `infobase` and `src/infobase.rs`, whose roundtrip/sweep workflows use the
  installed platform as an oracle.

The default feature set remains empty. Pure profile models, XCF/CF codecs,
conversion, compatibility reporting, and the process-free
`adapters::mssql_legacy` capability descriptor stay available. Direct MSSQL
research codecs remain separate from the installed-1C oracle boundary and do
not serve as a fallback for standalone conversion.

The gate exists at three layers: CLI variants, crate module exports, and the
oracle source files themselves. A future accidental module export therefore
does not silently restore the code to a default build.

## Release-shaped audit

`tests/platform_oracle_boundary.rs` runs the default binary with an empty
`PATH`. It requires `convert`, `cf`, and `compatibility`, rejects every oracle
subcommand in top-level help, and scans the built executable for known 1C,
EDT/JAR, JNI, and OSGi payload/path markers.

The portable Windows/Linux CI lane now compiles all root targets with
`--no-default-features` and runs that boundary test. The Windows legacy lane
does the inverse check with `--features platform-oracle`, proving that research
commands were isolated rather than deleted.

## Reproduction

```powershell
cargo check --locked -p ibcmd-rs --all-targets --no-default-features
cargo test --locked -p ibcmd-rs --test platform_oracle_boundary --no-default-features
cargo check --locked -p ibcmd-rs --all-targets --features platform-oracle
cargo build --locked --release --no-default-features
```
