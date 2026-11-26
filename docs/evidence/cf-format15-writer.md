# CF-007 evidence: deterministic Format15 writer

## Model and allocation

`ibcmd-v8::writer::Format15Document` carries the page size, preserved storage
and reserved header words, ordered elements, raw logical element headers, and
an optional data payload. `None` maps to the absent-address sentinel;
`Some(Vec::new())` allocates a real zero-length block.

The writer constructs a complete allocation plan before writing:

1. the file header occupies the first 16 bytes;
2. the ordered 12-byte TOC records use a compact page chain;
3. each raw element header uses a compact page chain;
4. each present data payload uses fixed configured pages, including
   deterministic zero padding on the final page.

Every address, page extent, logical size, and next-page pointer is checked
against the Format15 `0x7fffffff` address sentinel before output starts. The
first page declares total logical size; continuation pages declare zero while
retaining canonical lowercase eight-digit fields. Writing accepts any
`std::io::Write`, so the production API does not require a full output buffer;
`write_format15_to_vec` is only a convenience wrapper.

The root module-container compatibility API now delegates to this writer;
there is no second production allocator.

## Verification

- Payload sizes `0`, `1`, `511`, `512`, `513`, and `4097` write and parse back
  byte-for-byte; the latter two use two and nine pages respectively.
- Absent data and an explicit empty block remain distinct after parsing.
- Non-zero service bytes in a preserved raw element header survive exactly.
- Two writes from the same document are identical; parsing canonical output,
  rebuilding a document, and writing again is also byte-identical.
- An empty container has one valid zero-length compact TOC page.
- Zero page size and malformed element headers fail before output.
- `cargo test -p ibcmd-v8 --locked`: 29/29.
- Legacy `v8_container` regressions: 6/6.
- `cargo clippy -p ibcmd-v8 --locked --all-targets -- -D warnings`: pass.

No installed 1C platform, EDT, JVM, subprocess, or proprietary writer is used.
