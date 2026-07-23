# Clean-room malformed CF corpus

These files are hand-authored hexadecimal byte streams. They contain no 1C
application code or vendor configuration data. Blank lines and text after `#`
are ignored by `tests/cf_corruption.rs`; every other token is exactly one byte.

| Seed | Boundary |
|---|---|
| `file-header-truncated.hex` | truncated primary header |
| `file-header-unknown-signature.hex` | unknown container signature |
| `block-invalid-hex.hex` | non-hex ASCII in a block header field |
| `toc-offset-out-of-range.hex` | TOC points beyond the input |
| `chain-self-cycle.hex` | a page chain links to itself |
| `element-name-odd-utf16.hex` | odd UTF-16LE name byte count |
| `element-name-invalid-utf16.hex` | unpaired UTF-16 surrogate |
| `payload-invalid-deflate.hex` | structurally valid entry with invalid raw DEFLATE |
| `nested-truncated-child.hex` | stored nested-container payload is truncated |

The corpus is intentionally small and reviewable. Deterministic generated
property cases cover valid Format15/Format16 documents, page boundaries,
absent/empty payloads and nested trees; the fuzz target extends these seeds to
arbitrary input under the same resource limits.
