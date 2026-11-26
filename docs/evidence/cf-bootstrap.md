# Standalone XML-to-CF bootstrap evidence

BOOT-013 adds the first public base-free path from a complete hierarchical XML
source tree to a newly published CF. It does not invoke or discover `ibcmd`,
`1cv8`, Designer, EDT, a JVM, SQL Server, or another process as part of the
compiler or writer.

## Boundaries

The implementation keeps the existing architecture intact:

- `ibcmd-xml` reads the bounded source tree and metadata envelopes;
- the root compiler projects supported XCF into canonical IR, validates the
  complete object graph, resolves every asset through the explicit source
  registry, and produces an all-compiled `StoragePatch`;
- `ibcmd-cf` converts that patch into a complete ordered `StorageImage`, writes
  Format15 or Format16, reopens it through the normal decoder, and compares
  every logical key plus packed and unpacked payload;
- the CLI only selects independent XML dialect, platform profile, storage
  words and container revision and emits a stable JSON report.

No core-IR change was required. Platform builds remain data-driven effective
profiles, so adding a new build means supplying and testing new profile
constants rather than branching the container or canonical model. XML dialect
and CF revision remain independent axes.

## Readiness and publication

The final source-tree coordinator is stricter than the earlier coarse
`SourceKind` readiness inventory. It requires exactly one `Configuration.xml`,
matches its `ChildObjects` set to the decoded top-level metadata set, selects
only registered family codecs, consumes every remaining file exactly once by
owner-relative route, validates the complete root/version/versions inventory,
and runs `StoragePatch::preflight`. Unknown properties, families, layouts,
assets, unreferenced files, ambiguous owners, invalid DEFLATE, multipart
targets, or missing special entries all fail before publication.

Publication uses a same-directory create-new temporary file, flushes and syncs
it, reopens and validates it, and publishes through a no-clobber hard link.
Existing destinations are retained and failed work removes its temporary file.

## Verified corpus

[`tests/fixtures/bootstrap/manifest.json`](../../tests/fixtures/bootstrap/manifest.json)
declares a hand-authored clean-room XCF 2.20 tree for
`platform-8.3.27.1989`. It contains one Configuration, one CommonModule and its
module body. The expected six storage entries are the Configuration row,
CommonModule row, module row, `root`, `version`, and `versions`.

`tests/cf_bootstrap.rs` runs the installed binary with an empty `PATH` and
proves:

1. source tree -> Format16 CF succeeds without `--base`;
2. `cf inspect` returns exactly the six manifest entries;
3. `cf export` returns exactly the original three relative source paths, with
   no missing or extra file;
4. adding an unregistered source returns a stable
   `bootstrap_compile_failed` diagnostic and creates no CF.

The same in-memory compiler corpus is assembled, written and reopened as both
Format15 and Format16. Unit tests additionally cover deterministic patches,
graph reachability, invalid/missing sources, unknown Configuration properties,
invalid compressed payloads, missing special entries, zero page size,
destination no-clobber and temporary-file cleanup.

## Compatibility claim

`cf-bootstrap` is reported as `partial`, not universally implemented. The
command is available for the exact metadata and asset routes accepted by the
selected profile. A configuration using a known but not yet integrated body
route, an unknown future field, or an unevidenced platform layout is rejected
without output. This keeps the standalone capability useful without silently
claiming arbitrary CF compatibility.

## Reproducible checks

```text
cargo test --locked --test cf_bootstrap
cargo test --locked --lib bootstrap
cargo test --locked -p ibcmd-cf bootstrap::
cargo check --locked --workspace --all-targets
cargo fmt --all -- --check
```
