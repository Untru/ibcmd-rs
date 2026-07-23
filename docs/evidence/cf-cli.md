# CF-010 evidence: offline CF inspect and verify

CF-010 adds a standalone command surface over the bounded `ibcmd-v8` reader.
The commands open the archive directly and never probe `PATH`, start 1C/EDT,
or require a JVM.

## Stable report contract

Both commands emit JSON schema version `1`. A successful report contains:

- `layout`: Format15/Format16 revision, base/preamble offsets, stream length,
  page/storage words, element/page counts, and indexed encoded bytes;
- `profile`: the validated storage-profile identifier and the explicitly
  selected `raw-deflate` or `stored` payload representation;
- `selection`: requested names, selected/archive counts, and list-only state;
- `elements`: exact top-level names in source order, absent/present state,
  chain sizes, page counts, verification state, and packed/unpacked SHA-256.

Errors use the same envelope with `ok: false` and stable diagnostic `code`
values. The CLI writes this JSON to `stderr` and exits with code `2`, so callers
do not need to scrape Rust error prose.

## Selective and bounded verification

Repeated `--element NAME` options select exact top-level entries. Only selected
payloads are read and decoded; indexing remains structural and bounded.
`--list-only` validates layout and chains without loading payload bytes.
Repeated `--expect-sha256 NAME=HASH` options compare canonical lowercase
SHA-256 against unpacked bytes. Missing names, duplicate expectations, invalid
digests, decode failures, and mismatches fail closed.

Compression is an input contract (`--compression raw-deflate|stored`), not a
heuristic based on payload prefixes or dotted element names.

## Portable acceptance coverage

[`tests/cf_cli.rs`](../../tests/cf_cli.rs) builds a clean-room Format15 archive
in memory and launches the compiled binary with an empty `PATH`. It proves that
`cf inspect` succeeds without any installed platform. A separately corrupted
archive proves a nonzero status and parseable JSON `invalid_archive` error.

Focused verification:

```powershell
cargo test --locked commands::cf
cargo test --locked --test cf_cli
```
