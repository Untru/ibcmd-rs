# CF-009 evidence: native archive to ordered StorageImage

CF-009 connects the bounded Format15/Format16 reader to the neutral
`ibcmd-core::storage::StorageImage` without introducing SQL, platform, EDT, or
filesystem types into that model.

## Mapping contract

The decoder preserves source TOC order. For every native entry it retains:

- the exact element name as both logical display name and logical key;
- dotted native suffixes as part of that key;
- the raw logical element header and raw 12- or 24-byte address record;
- exact packed bytes and strictly decoded unpacked bytes;
- explicit `stored` or `raw-deflate` compression;
- caller-supplied source storage profile and bounded provenance.

Compression comes from an explicit profile resolver (or an explicitly chosen
uniform mode); it is never guessed from payload bytes or a file-name suffix.
This keeps profile/version decisions outside the binary container layer.

Multipart identity is independent of the native dotted suffix. One CF TOC
record maps to one `MultipartIdentity::single()` entry. Repeated exact names
are rejected with both TOC indexes before any payload is decoded, rather than
being silently overwritten or guessed into multipart order.

The neutral payload model requires byte buffers even for an absent CF data
address. A versioned CF attribute therefore records `Absent` versus `Present`
alongside the raw address record. An absent entry carries empty stored buffers,
but remains distinguishable from a present zero-length block for lossless
repack.

## Archive metadata

`CfArchive` retains the ordered image plus the revision, base offset, stream
length, page size, storage version, reserved word, exact file header, and exact
Format16 preamble. This is sufficient input for the later deterministic repack
task without deriving container coordinates from logical payloads.

## Offline regressions

- Both checked-in clean-room revisions decode to the same five ordered logical
  keys and expose the object record plus `root`, `version`, and `versions`.
- Repeated decode produces the same order-sensitive `StorageImage` digest.
- Format15 keeps 12-byte and Format16 keeps 24-byte raw address records.
- A mixed synthetic archive proves explicit stored/raw-DEFLATE classification,
  dotted-suffix preservation, unpacked bytes, and absent-data state.
- A duplicate-name archive fails with the exact first and duplicate indexes.
- The entire path runs against in-memory fixtures without 1C, EDT, or a JVM.

The clean-room layout is cross-checked against the independently implemented
[e8tools/v8unpack reader](https://github.com/e8tools/v8unpack/blob/d34bb1e3565572e0de30a4aa4d66d6cd3e3e08e2/src/V8File.cpp)
and [onec_dtools container reader](https://github.com/Infactum/onec_dtools/blob/99c0b394f51fbd4225735ab37068c2dae00fdc00/onec_dtools/container_reader.py).
