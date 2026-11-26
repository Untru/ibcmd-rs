# Bootstrap special entries for platform 8.3.27.1989

Evidence was collected on 2026-07-22 from a local test infobase by read-only
queries against its `_Config` storage. No 1C executable, Designer operation or
database mutation participated in collection.

The inspected configuration UUID was
`61ee2494-c14a-4992-8c93-8e78b20bea27`. Native row payloads use raw DEFLATE.

| Key | Compressed bytes | Compressed SHA-256 | Exact decompressed contract |
| --- | ---: | --- | --- |
| `root` | 46 | `d4d28b40f327cc7ec9ab0cb939470a7f76e189d251024651764afd553df49cf7` | UTF-8 BOM + `{2,61ee2494-c14a-4992-8c93-8e78b20bea27,}` |
| `version` | 28 | `919d95528de71dd5ecd5aaaf7f894c60013c37475c620139497f5530dc06f1eb` | UTF-8 BOM + `{\r\n{216,0,\r\n{80327,0}\r\n}\r\n}` |
| `versions` | 1,395,137 | `c12d362ad582cf680a4d5fac19566d360ad5326cef485e6585fef08e30f6127a` | UTF-8 BOM + layout 1, complete lexically sorted key map |

The decompressed `versions` payload was 3,124,814 bytes with SHA-256
`576e54580e8dd93bc9f7f5d5e11928659a14a695d0d07742cb2ed2a8798af6a8`.
Its map included object, suffixed object, `root`, `version`, and `versions`
entries. The generation UUID values are treated as opaque identities; the
standalone writer derives equivalent stable UUIDv8 values from exact compiled
payload digests instead of copying or randomly generating them.

These observations support only the named special-entry layouts. The
Configuration metadata body and metadata-family bodies require their own
profile evidence and must fail closed until their codecs are implemented.
