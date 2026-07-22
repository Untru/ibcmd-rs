# CF-002 evidence: lossless Format15 structural reader

## Scope

`ibcmd-v8::format15` is the portable 32-bit V8 container reader. It has no dependency on SQL, CLI, XML, process execution, 1C, EDT, or a JVM. The existing root `v8_container` module now delegates parsing to this crate and retains only its compatibility writer until CF-007.

The reader preserves:

- all 16 raw file-header bytes and their four decoded 32-bit words;
- all 31 raw bytes for the TOC and element block headers;
- every raw 12-byte TOC address record;
- original TOC order;
- raw element header and stored payload bytes;
- an absent data-address sentinel as `None`, distinct from an empty payload.

## Ambiguous header word

The third file-header word is exposed as `storage_version` for compatibility with the existing code and public reverse-engineering terminology. Observed module containers use `0`, `1`, or `2`; the clean-room configuration envelope uses `5`; independently published configuration files can carry larger values that correlate with element count. The structural layer therefore preserves the word without normalizing it or rejecting future values. Artifact-specific interpretation belongs above `ibcmd-v8`.

## Fail-closed boundary

CF-002 reads single-page blocks only. A non-sentinel next-page address returns the typed `ChainedPagesUnsupported` error with exact block and next-page coordinates. This is deliberate: cycle, overlap, repeated-address, range, and size validation are implemented together in CF-003 instead of partially following chains here.

Malformed headers, hex fields, TOC widths, markers, ranges, UTF-16LE names, and absent header addresses return typed errors without panic. The parser checks the declared page extent before copying the shorter logical payload.

## Evidence

- `cargo test -p ibcmd-v8 --locked`: 6/6, including clean-room corpus parsing, raw-header/order preservation, observed header words `0/1/2/5`, absent data, invalid marker, and chained-page boundary.
- `cargo test --locked v8_container`: 6/6 legacy parser/builder regressions.
- Targeted module regressions: `packs_module_inner_with_plain_info_and_text`, `module_outer_blob_is_raw_deflate`, and `unpacks_module_blob_text_element` pass.
- `cargo clippy -p ibcmd-v8 --locked --all-targets -- -D warnings`: pass.
- Fixture provenance and hashes: [`tests/fixtures/cf/README.md`](../../tests/fixtures/cf/README.md).

No platform component or external executable is used by these tests.
