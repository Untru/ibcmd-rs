# Bounded CF payload evidence

CF-005 establishes the byte-level safety boundary used by later Format15,
Format16, nested-tree, and bootstrap asset work. It does not require the 1C
platform and does not infer a container format from payload bytes.

## Accepted encodings

- `Stored` returns the exact input bytes after aggregate entry and byte-budget
  checks.
- `RawDeflate` accepts exactly one RFC 1951 stream. The low-level inflater must
  report `StreamEnd`, and its consumed-byte count must equal the complete input.
- A truncated stream, invalid stream, or any byte after `StreamEnd` is a typed
  error. There is no permissive fallback from compressed to stored data.

## Resource contract

`ibcmd-core::limits` owns a reusable limit set and atomic traversal budget for
maximum nesting depth, entry count, encoded bytes, decoded bytes, and per-entry
decoded-to-encoded ratio. The CF decoder emits into a fixed 16 KiB scratch
buffer, checks totals before extending retained output, and only commits entry
accounting after the payload has passed every codec and limit check.

## Regressions

The ordinary test suite covers stored and compressed round trips, missing
`StreamEnd`, trailing bytes, a highly compressible 256 KiB payload under small
decoded-byte/ratio limits, aggregate entry count, and container depth. All
tests are synthetic and deterministic, so they run on hosts without 1C, EDT,
Java, or project-external fixtures.
