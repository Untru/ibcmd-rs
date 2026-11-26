# Constant native layout evidence for 8.3.27.1989

Status: implemented as the `constant-v1-crlf-utf8-bom` experimental platform
layout. No installed 1C platform, EDT, JVM, or database access is required by
the codec.

## Retained sources

- XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\Constants\АдресаСерверовМетокВремени.xml`.
- Inflated primary row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\8cf925d9-6811-4bce-b116-09d6a26ff3cb__part0.txt`.
- Inflated row length: 692 bytes; SHA-256:
  `41b05e2f74627b5f01db05b6d276b44a6ec3ef1d3f333f434990797a263eb87e`.
- Independent retained rows with the same structural shell cover Boolean
  (`9893e2d6-f3f8-4d73-bb06-19bf26d216ab`), Number
  (`87fec241-b812-44b6-af6d-47f5dcea9834`), and DateTime
  (`a4ec7d3d-2d01-436c-98ec-ec42733ee5c6`) patterns.
- `docs/ssl-lab-2026-06-25.md` records an earlier platform-accepted Constant
  mutation for this object: variable string length 80, synonym change, and
  `UseStandardCommands=false`; the generated compressed row was 392 bytes with
  SHA-256
  `f8bd300c5bc51e0e828b96e09538a6ebc1ecbedafb2a4dcb2725a9fa587e5aa`.

The current implementation only reads these retained artifacts. It does not
start 1C or write Config/ConfigSave.

## Exact native grammar

The primary row is a three-field root:

```text
{1, ConstantObject, 0}
```

`ConstantObject` has exactly 17 fields. Its first nested owner has exactly 23
fields and contains the common metadata header plus exactly one type pattern.
The six generated identities occur in this order:

1. Manager TypeId and ValueId;
2. ValueManager TypeId and ValueId;
3. ValueKey TypeId and ValueId.

All six IDs must be non-nil and pairwise distinct. The generated cohort flag is
exactly `1`; `UseStandardCommands` is the following `0` or `1`. Every remaining
owner/object slot is matched against the retained constant shell. Unknown,
missing, reordered, or extra fields fail closed.

Supported pattern items are the retained mappings:

- Boolean: `{"B"}`;
- unlimited variable String: `{"S"}`;
- bounded variable String: `{"S", Length, 0}`;
- Number: `{"N", Digits, FractionDigits, NonnegativeFlag}`;
- DateTime: `{"D"}`;
- generated or evidenced built-in type: `{"#", TypeId}`.

Fixed strings and Date-only/Time-only qualifiers remain unsupported because no
independently retained native row proves those conversions. Native references
do not encode the XML `Type` versus `TypeSet` spelling; IR-to-XCF restores
`TypeSet` only for an exact `cfg:DefinedType.<name>` mapping and uses `Type` for
other exact generated/built-in names.

## Version boundary

XCF 2.20 and 2.21 share this native layout only through explicit XML codecs.
The platform and storage coordinates remain independent profile axes. A future
platform must opt into its own Constant layout constant and evidence instead of
implicitly inheriting this byte grammar.
