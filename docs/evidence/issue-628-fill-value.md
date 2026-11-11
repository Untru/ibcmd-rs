# Declared-type normalization of FillValue (#628)

An empty stored string is not always a string-valued filling default. The
native exporter writes `xsi:nil="true"` when the declared type's leaves are
known to exclude strings. String-containing, missing, empty, unresolved and
cyclic declarations retain the stored empty string. The existing CF reader
already applies this rule; this change applies it to model descriptor export.
Other typed-value properties and nonempty filling values keep their behavior.

## Native evidence

The public fixture submodule is pinned at
`8d138b896f462c3053d16076cbabf0ffe5030c1c`.

| Platform | Profile | Decoded CF SHA-256 |
| --- | --- | --- |
| 8.3.27.2214 | corpus-defined-type-resource | `d7395e53b90077390976b8a6b639ac45e646d96e62c41a836c642f3778b425d8` |
| 8.5.1.1150 | corpus-defined-type-resource | `cac16912c6f6aa17b3e6a2a0121909157373dafecaa7ef88b6e1e3e38084e062` |
| 8.3.27.2214 | corpus-form-deep-bindings | `2a23fe4a1c1e93c8bc5431b1e6e947fe850f4d772d74e4a2f1960e3b43ba54b8` |
| 8.5.1.1150 | corpus-form-deep-bindings | `ead48609831bc1bf352e55c5223f04fc82eed86e453ad4e581e8aaf0b49794a1` |

The affected native descriptors are identical between the two platforms:

| Descriptor | Inflated row SHA-256 |
| --- | --- |
| InformationRegisters/CovDefinedResourceTemplate.xml | `d6a3897fdcd9775cb1c8d57c459618593f5c0ecf9a26560979b45961c0010f00` |
| Catalogs/CovBindingProducts.xml | `26917b250d082a389f31559df2c0d69ba2061ffc8254cc3ccce448a98278b5be` |

The native DefinedType descriptor is also identical in both profiles
(SHA-256 `12fa9a6d10cf486f44eaea2aa5043db1b41976b866a17bfe42ba9d32d5cc1a89`).
Independent CF/brace extraction confirms its five-member revision-0 payload:
type id at 1, value id at 2, revision-3 metadata header at 3, type pattern at 4.
`DefinedTypeLayout` now gives decoding and indexing those shared coordinates.

For a focused measurement, `cf extract` extracted the exact native object
records from each hash-checked CF. A small Rust driver called the public
`export_descriptor` API on those inflated records, with `NameIndex` built
from the profile's native source tree. This was repeated against the old
model library and the changed library; no records were rewritten.

Across the four profiles, all **8/8 FillValue elements** now match native
attributes and text, compared with **4/8** before. All three catalog
descriptors in each deep-bindings profile now match native XML byte for byte
(**6/6**, previously 4/6). The two register descriptors retain exactly one
XML difference: the omitted `BinaryDataStorageLocationUse` property (#629).
Neither native fixtures nor corpus baselines were changed.

This focused API measurement is not a database import/export run. Raw-row
name-index declaration extraction and parallel index merging are checked
separately by the snapshot/row regression below. The existing CLI corpus
measurement can be rerun with:

```text
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records::clean_room_corpus_stored_row_route_holds_its_baseline --no-default-features -- --nocapture
```

## Regression evidence

`cargo test --locked -p ibcmd-metadata-model`: **72 passed, 0 failed**.
Strict Clippy for this crate and all its targets also passes.

The new direct regression feeds an actual stored empty-string value through
both attribute-body readers. Before the fix, a numeric declaration fails
with `left: None`, `right: Some("true")` for the nil attribute. It also checks
Boolean, date, reference, string-containing and empty declarations.

Named declarations are read from both an immutable XML snapshot and the
native DefinedType/Characteristic record layouts. The snapshot paths do not
exist on disk. Additional synthetic graph cases cover nested aliases,
missing definitions, unknown type ids, empty declarations and cycles.

Independent review found whitespace around XML type names needed trimming.
The regression now includes synthetic whitespace around string, numeric,
and nested TypeSet declarations. Removing that trim reproduces an incorrect
nil for the string alias; restoring it passes the full crate suite.
Independent source review then approved the correction without findings.

The physical-adapter guard reports 17 new `name-special-case` occurrences
for this reviewed semantic policy: `export.rs` 2, `index.rs` 6,
`objects_export.rs` 2, `simple_export.rs` 1, `types_export.rs` 6.
They select metadata kinds, declaration categories and primitive/type-set
leaves; none selects a named business object. An independent targeted scan
confirmed all 17 allowances. The policy baseline appends exactly those
entries and preserves every existing entry, including stale allowances.

## Scope

Full-project checks remain separate merge gates. Completion of the whole
register's byte-parity acceptance also requires #629; this patch uses
`Refs #628` and does not claim that dependent criterion is complete.
