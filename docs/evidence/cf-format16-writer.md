# CF-008 evidence: semantic preamble and Format16 writer

CF-008 completes the standalone write path for a full Format16 artifact. The
implementation is split at the same boundary as the reader:

- `ibcmd-v8::writer` plans and writes the primary 64-bit container;
- `ibcmd-cf::preamble` models and writes the semantic Format15 envelope that
  precedes it at the fixed `0x1359` base.

No platform executable, EDT component, JVM, downloaded artifact, or opaque
preamble template is used.

## Primary container

The primary writer performs structural preflight before reading a payload or
writing the file header. Its plan contains one compact chain descriptor per
TOC/header/data object rather than one allocation per physical page. TOC
records, block headers, and next-page addresses are produced incrementally
with 64-bit fields relative to the start of the primary container.

Payload input is a declared-length `Read` source. Data moves through a fixed
8 KiB buffer directly into deterministic padded pages. A virtual 64 MiB source
and a counting output prove that the writer never asks for more than 8 KiB and
does not retain the payload. A separate plan-only regression crosses the
32-bit offset boundary without allocating or reading that logical payload.

Boundary payloads `0/1/511/512/513/4097`, absent data, explicit empty data,
raw logical headers, and multi-page chains round-trip through the native
Format16 parser. Rebuilding the same document produces identical bytes.

## Semantic preamble

`SemanticPreamble` contains the Format15 storage words and ordered named
entries with their absent-or-present payload state. Generation first measures
the deterministic Format15 envelope, rejects a model larger than `0x1359`, and
adds the exact difference to its final physical page. The result is then
parsed again as a Format15 container before it can be written.

`PreambleMode::Preserve` accepts bytes only after exact-length and structural
validation, and retains them byte-for-byte. `PreambleMode::Generate` builds the
same logical envelope from the semantic model. The checked-in clean-room
Format16 fixture verifies both paths: preserve mode reproduces the complete
artifact exactly, and generated mode parses to the same ordered primary names
and payloads.

## Offline regression gates

- `ibcmd-v8` writer tests cover deterministic Format15/Format16 output,
  boundary pages, large streaming input, and 64-bit planning.
- `ibcmd-cf` preamble tests cover exact-size generation, preserve validation,
  complete archive parsing, and clean-room preserve/regenerate behavior.
- The portable workspace, all-target compile/check, fixture corpus, and
  portable clippy gates run without 1C or EDT.
