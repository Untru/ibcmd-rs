# DefinedType native row — 8.3.27.1989

Status: experimental base-free layout backed by read-only corpus evidence.
Earlier base-patching experiments for this family were accepted by a running
1C platform, but the new fully generated row remains subject to a separate
platform-acceptance gate.

This slice was implemented without starting 1C or writing a database. Existing
XCF and `_Config` artifacts showed that every `DefinedType` owns an independent
generated `TypeId` and `ValueId`, followed by the common metadata header and a
type pattern. One retained compressed row had these characteristics:

- logical key: metadata object UUID, without a suffix;
- storage encoding: raw DEFLATE over UTF-8 text, without a UTF-8 BOM;
- compressed bytes: 238;
- plaintext bytes: 351;
- compressed SHA-256:
  `3d81c2ea4020ca16e51f5c822a9f9bbb5cc883883252dea9dd2a2e50f0dc80c9`;
- complete redacted grammar:

```text
{1,
  {0,<defined-type-TypeId>,<defined-type-ValueId>,
    <metadata-header>,
    {"Pattern",<pattern-item>...}
  },
0}
```

`metadata-header` and `pattern-item` use the exact layouts documented by the
Language and SessionParameter fixtures. A second retained plaintext row for
`DefinedType.БезопасныйРежим` confirms the ordered pattern
`{"B"},{"S",120,1}`. Reference items are `{"#",<TypeId>}` and resolve only
through canonical generated types from the complete validated configuration.

The XCF `InternalInfo/xr:GeneratedType` entry is not reducible to one UUID:
both `xr:TypeId` and `xr:ValueId` occur in the native row and are independently
assigned. The canonical core therefore represents `ValueId` as an optional
typed field on `GeneratedType`; legacy canonical JSON without it remains
readable, while the semantic digest domain is explicitly advanced to v2 so a
new ValueId changes semantic identity.

The committed `defined-type-v1-crlf-no-bom` profile rejects missing, nil, or
equal TypeId/ValueId values, inconsistent generated names/categories,
unresolved or ambiguous `cfg:*` types, duplicate pattern entries, unknown
native tags and extra fields. Fixed strings and Date-only/Time-only qualifiers
remain fail-closed until their native encodings are evidenced. Fixtures cover
XCF 2.20 and 2.21 through XML -> canonical IR -> deterministic native blob ->
native IR -> XML -> canonical IR.
