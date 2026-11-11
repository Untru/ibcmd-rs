# Information-register binary data storage location (#629)

The public fixture submodule is pinned at
`8d138b896f462c3053d16076cbabf0ffe5030c1c`. No fixture or policy baseline
was regenerated for this change.

## Native evidence

Both `coverage/<platform>/corpus-defined-type-resource` profiles contain
`InformationRegisters/CovDefinedResourceTemplate.xml`, whose `StoredValue`
resource is `v8:ValueStorage` and exports
`<BinaryDataStorageLocationUse>DontUse</BinaryDataStorageLocationUse>`.

| Platform | Decoded `configuration.cf.b64` SHA-256 |
| --- | --- |
| 8.3.27.2214 | `d7395e53b90077390976b8a6b639ac45e646d96e62c41a836c642f3778b425d8` |
| 8.5.1.1150 | `cac16912c6f6aa17b3e6a2a0121909157373dafecaa7ef88b6e1e3e38084e062` |

An independent CF extraction reads a revision-8 resource wrapper with seven
fields, field 5 equal to `1`, and the final storage-reference pair
`{1,00000000-0000-0000-0000-000000000000}`. The register descriptor is identical
in both CFs (SHA-256
`d6a3897fdcd9775cb1c8d57c459618593f5c0ecf9a26560979b45961c0010f00`).
The existing CF reader independently defines this field as `0` = omitted,
`1` = `DontUse`, for both resource revision 8 and attribute revision 5.

## Regression evidence

The tests compile the native resource XML in both dialects and assert the
measured wrapper field. With the old writer they fail with `left: Some(0)`,
`right: Some(1)`. A reader test starts with the native field `1`, verifies
`DontUse` in its native property position, verifies omission for `0`, and
rejects an unknown code.

The attribute test is explicitly synthetic: it reuses the native ValueStorage
resource's properties as an attribute and checks the existing revision-5
layout in both dialects. It also fails with `0` versus `1` before the fix.
It is not a fresh native measurement of a ValueStorage attribute.

`cargo test --locked -p ibcmd-metadata-model`: **71 passed, 0 failed**.
The independent critic verified both CF descriptors and the policy guard,
and approved the final source and synthetic-test scope without findings.

## Scope

The model preserves the explicit property in both directions and keeps
omitted/zero values omitted. The separate `FillValue` normalization defect
(#628) remains outside this patch; this evidence does not claim byte parity
for the whole register or completion of all #629 acceptance criteria.

The complementary FillValue normalization is submitted separately in
[PR #712](https://github.com/Untru/ibcmd-rs/pull/712). Its focused measurement
of these same native register records leaves only the storage-location
property difference covered here; neither branch claims a combined merge
or a live database round trip.
