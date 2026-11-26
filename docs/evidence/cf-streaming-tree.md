# CF-006 evidence: bounded streaming and nested traversal

## Lazy V8 index

`ibcmd-v8::reader::StreamingReader` accepts any `Read + Seek` source and
supports both detected Format15 and Format16 layouts. Opening it reads:

- the file header and TOC payload;
- every element-name header payload;
- only the block headers for element data chains.

Each indexed page retains its relative address, absolute payload offset,
declared page size, logical byte count, next address, and raw header. Full page
extents are checked and claimed, so cycle, reuse, overlap, range, size, marker,
and arithmetic failures remain typed even though data bytes are lazy.

The index enforces the shared entry and encoded-byte limits plus bounded page
and structural-index counts. An explicit `read_entry_data`,
`read_named_data`, or `read_entry_header` fetches only logical bytes from the
recorded page slices; final-page padding is never read into memory.

## Nested CF traversal

`ibcmd-cf::tree::traverse` is visitor-based and does not build a retained tree.
The caller classifies each path as skipped, a leaf, or a nested container and
selects `Stored` or strict `RawDeflate` decoding. One `PayloadDecoder` is shared
across the complete walk, so nested depth, selected entry count, encoded bytes,
decoded bytes, and compression ratio are aggregate rather than reset per child.

Only the selected encoded payload and its decoded result are live while a child
is visited. Container depth is entered before a child reader is opened and is
balanced on success or error.

## Verification

- A custom seekable sparse source advertises a valid Format15 page of 1 GiB
  while storing only small mapped segments. Indexing and selecting its four
  logical bytes perform less than 512 bytes of reads, with no request above 128
  bytes and no allocation proportional to the declared page size.
- The checked-in Format16 fixture is indexed through its `0x1359` base and one
  named `root` payload matches the structural parser byte-for-byte.
- A two-level stored nested container visits deterministic `nested/leaf` paths
  and finishes with aggregate depth zero.
- A three-level recursive container under maximum depth two returns
  `DepthExceeded { maximum: 2, actual: 3 }` at `nested/nested`.
- A raw-DEFLATE leaf is decoded through the same aggregate budget.
- `cargo test -p ibcmd-v8 --locked`: 24/24.
- `cargo test -p ibcmd-cf --locked`: 8/8.
- Clippy for both crates with `-D warnings`: pass.

No platform executable, EDT component, JVM, database, network service, or
temporary full-file copy is used.
