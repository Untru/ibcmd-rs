# CF-004 evidence: Format16 detection and structural reader

## Scope

`ibcmd-v8::format` is the unified revision detector and parse entry point.
Every complete supported artifact first carries the Format15 sentinel. A
Format16 artifact additionally carries the 64-bit
`0xffffffffffffffff` sentinel at the standard base offset `0x1359`; this
second signature takes precedence during detection. Unknown or short inputs
return typed errors instead of falling back to byte guessing.

`ibcmd-v8::format16` parses the complete artifact in two explicit parts:

1. the exact `0x1359` preamble bytes, also validated as a semantic Format15
   container;
2. the primary 64-bit container whose addresses are relative to that primary
   slice.

The returned model preserves the preamble bytes and parsed graph, the raw
20-byte file header, raw 55-byte header for every page, raw 24-byte address
records, original entry order, stored header/data bytes, and absent-data
sentinels. The structural layer retains the header's storage-version word
without inferring a platform or XML dialect from it.

## Safety boundaries

The same transactional claim rules used by Format15 apply to 64-bit page
chains. Address addition is checked before conversion to `usize`; header and
declared page extents must fit the primary input; cycle, reuse, overlap, early
end, extra continuation, invalid hex, and invalid address-table marker errors
remain distinct. A valid two-page Format16 data chain and a direct three-page
wide chain are covered in memory.

The detection coordinates and layouts were checked against the pinned
[e8tools/v8unpack `V8File.h`](https://github.com/e8tools/v8unpack/blob/d34bb1e3565572e0de30a4aa4d66d6cd3e3e08e2/src/V8File.h)
and
[`V8File.cpp`](https://github.com/e8tools/v8unpack/blob/d34bb1e3565572e0de30a4aa4d66d6cd3e3e08e2/src/V8File.cpp)
source revision. Those sources are evidence only; no code or runtime component
is linked or executed.

## Verification

- `cargo test -p ibcmd-v8 --locked`: 22/22.
- Golden Format15/Format16 detection returns bases `0` and `0x1359`.
- The clean-room Format16 fixture yields five ordered entries including
  `root`, `version`, and `versions`, and its preamble yields its five expected
  entries.
- Primary-only parsing yields the same element model as complete-artifact
  parsing.
- Exact regressions cover truncated primary header, absent data, a `u64`
  address overflow, valid two-/three-page chains, and unknown signatures.
- `cargo clippy -p ibcmd-v8 --locked --all-targets -- -D warnings`: pass.

All verification is portable and uses no installed 1C platform, EDT, or JVM.
