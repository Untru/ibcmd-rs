# Evidence-derived compatibility matrix

APP-002 replaces the previous hand-written feature summary with one strict,
versioned source of truth at `compatibility/matrix.json`. The library embeds
that document, validates it against the bundled profile registry, and the
`compatibility` CLI serializes the same model. No runtime probe, installed 1C
platform, EDT component, Java process, or repository checkout is required.

## Exact route coordinates

Every record is keyed by all of these independent coordinates:

- operation;
- source artifact and target artifact;
- exact source profile and target profile;
- metadata family;
- referenced evidence.

Support and verification are separate. `supported` says that the exact route
can execute; `verified` says that the route also references at least one green
Cargo integration test. A supported route without executable evidence remains
`experimental`. An unsupported route cannot claim preservation. A missing
record is not a wildcard: exact lookup returns `unsupported`.

The Rust validator rejects duplicate or unsorted IDs, duplicate route
coordinates, unknown bundled profiles, unsafe evidence paths, missing evidence
references, unused evidence, and status claims that do not match the evidence
derivation. The companion `compatibility/matrix.schema.json` publishes the
same structural contract for other tools.

## Repository evidence gate

`tests/compatibility_matrix.rs` additionally runs in a source checkout and
proves that every evidence path exists. For a green Cargo test link, it also
requires the named `#[test]` case to remain in the referenced integration-test
file. The normal workspace test lane executes those cases, so a route cannot
retain green evidence while its test is deleted or failing.

The current verified surface is deliberately narrow:

- XCF 2.20 to 2.21 and 2.21 to 2.20 for the evidenced `Constant` and `Catalog`
  cohort;
- clean-room XCF 2.20 to platform 8.3.27.1989 CF bootstrap and reverse export
  for `ConfigurationRoot` and `CommonModule`;
- same-profile 8.3.27.1989 CF repack.

Known but unevidenced routes are explicit `experimental`/`unsupported`
records. A syntactically valid but unknown profile returns
`UnknownSourceProfile` or `UnknownTargetProfile`; it is never inferred from a
nearby platform or XCF version.

## Reproduction

```powershell
cargo run --locked -- compatibility
cargo test --locked --test compatibility_matrix
cargo test --locked --test conversion_cli `
  --test migration_2_20_to_2_21 `
  --test migration_2_21_to_2_20 `
  --test cf_roundtrip
```
