# CF-011 evidence: deterministic lossless CF repack

CF-011 connects the neutral ordered storage image to both native container
writers and adds validated atomic publication. The implementation is in
[`crates/ibcmd-cf/src/writer.rs`](../../crates/ibcmd-cf/src/writer.rs).

## Lossless preflight

No output byte is emitted until every entry passes these checks:

- logical name and key are identical and the physical identity is single-part;
- the exact retained raw logical header decodes to that same name;
- opaque CF attributes decode to an explicit absent or present data state;
- compression is exactly `stored` or `raw-deflate`, never inferred;
- absent entries contain no payload and use `stored`;
- each present packed payload strictly decodes to its retained unpacked bytes
  under the shared aggregate resource budget.

Format15 writes the retained page/storage/reserved words, ordered raw headers,
and exact packed bytes. Format16 does the same with 64-bit addressing and also
validates and copies the exact semantic preamble retained by the reader.

## Reopen validation and atomic publication

`publish_repacked_new` creates a collision-resistant temporary file in the
destination directory with create-new semantics. It writes, flushes, calls
`sync_all`, closes, reopens, and validates:

- revision, base offset, raw file header, storage version, and preamble;
- entry count, source order, names, raw logical headers, and data state;
- exact packed bytes and decoded unpacked bytes for every present entry;
- reported and actual output lengths.

Only after validation is the complete temporary inode linked to the final name.
The no-clobber link operation fails if another file already owns that name, so
an existing destination is not overwritten in the check/publish race. A guard
removes the temporary name on every earlier failure.

## Portable round-trip coverage

[`tests/cf_roundtrip.rs`](../../tests/cf_roundtrip.rs) decodes both checked-in
clean-room revisions, writes them twice, validates each artifact through the
streaming reader, and decodes again. The test compares layout semantics,
ordered names/keys, raw headers, absent/present state, packed/unpacked bytes,
compression, profile, and provenance; the two outputs are byte-identical.

A separate test publishes Format16 twice to fresh paths, compares the files,
checks reopen-validation counts, proves an existing file remains unchanged,
and verifies that no temporary files remain.

```powershell
cargo test --locked --test cf_roundtrip
cargo clippy --locked -p ibcmd-cf --all-targets -- -D warnings
```
