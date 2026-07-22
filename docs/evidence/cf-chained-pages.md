# CF-003 evidence: safe chained block pages

## Scope

`ibcmd-v8::block::BlockReader` assembles 32-bit Format15 block chains without
1C, EDT, a JVM, or any external executable. A single reader owns the claimed
page map for one container parse, so TOC, element-header, and element-data
chains cannot silently alias one another.

For each page the reader retains the address, all 31 raw header bytes, decoded
logical/page sizes, next-page address, and the number of logical bytes consumed
from the page. The first header defines the total logical size. The last page
may contain padding; padding is range-checked and claimed but is not returned as
payload.

Claims are committed only after the complete chain validates. An invalid chain
therefore cannot poison the reader state used by later diagnostics or recovery
logic.

## Fail-closed cases

The public `BlockError` distinguishes:

- a cycle within one chain, including a self-reference;
- exact reuse of a page already claimed by another chain;
- a partial overlap with a prior page or reserved file-header extent;
- an address whose 31-byte header is outside the input;
- a declared page extent outside the input;
- malformed delimiters or hexadecimal header fields;
- a chain that ends before its declared logical size;
- a chain that continues after the declared logical size is complete;
- a zero-progress page while bytes remain.

All address arithmetic is widened before range comparison. Data is copied only
after the complete declared page extent is known to be in bounds.

## Evidence

- `cargo test -p ibcmd-v8 --locked`: 13/13.
- A direct three-page test verifies payload assembly, final-page padding, page
  order, and preservation of every raw header.
- Corrupt synthetic tests assert exact typed errors for cycle, overlap, reused
  address, header/page range, early end, and unexpected continuation.
- A Format15 integration test rewrites a clean-room entry into a valid
  two-page data chain and verifies byte-identical logical payload plus both page
  records.
- `cargo test --locked v8_container`: 6/6; the legacy wrapper now accepts the
  same validated multi-page payload.
- `cargo clippy -p ibcmd-v8 --locked --all-targets -- -D warnings`: pass.

The tests construct their binary pages in memory and do not invoke or inspect
an installed platform.
