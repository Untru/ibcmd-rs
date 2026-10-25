# Corpus-derived clean-room probes

The application configurations are not fixtures. These probes use public
Coverage templates, platform property names and independently written data.
The generated sets live in `../coverage/*/corpus-*/`; their manifests pin the
native executable, inputs, saved CF and both native export rounds.

`native-features.json` checks that the requested property actually survived
native normalization. Successful import alone is insufficient: inapplicable
properties can disappear without an error.

`source-probes.json` keeps five minimized XML probes outside the CF oracle:

| Probe | Native result | Why it remains separate |
| --- | --- | --- |
| Embedded pivot table | Accepted, property normalized away | The saved CF no longer contains the failing XML subtree. |
| Inconsistent chart series count | Accepted, count normalized | Import must still classify the original legacy XML precisely. |
| Generated selected-rows panel reference | Accepted, reference normalized away | The source names a generated panel absent from ordinary XML items. |
| `CreateByParameter` | Refused on a fresh native infobase | Legacy XML fragment; not a native-built command fixture. |
| `EditList` | Refused on a fresh native infobase | Legacy XML fragment; not a native-built command fixture. |

The same manifest also audits the two existing public Interface sets from
PR #535, without mutating their XML.

Each source probe pins its native baseline manifest and changes exactly one
subtree or property in that context. Its test requires the observed diagnostic
on the exact owning metadata file. When a codec gains support, replace that
known-refusal assertion with an equivalence check. A compiling stage is not
evidence of SQL import, apply or round-trip parity.

`empty-html.txt` is the ten-byte, data-free legacy `{5,0,0}` HTML record, with
its UTF-8 BOM. It is injected into the public CommonTemplate's row cache.
The selected-rows group probe similarly instantiates a minimized legacy
physical layout in a public form; it does not claim a native-built modified CF.

`extension-sql-8327.hex` is the `_ExtensionsInfo` value obtained by loading
the public `CovCombined` extension into an isolated SQL infobase on
8.3.27.2214 and applying it with native ibcmd. It is a data-free envelope
probe, not a database snapshot. Its native create/load/apply/export control
passed; the 8.5 SQL control was not established. No extension source or
released version is changed by these tests.

`coverage_storage_records` extracts metadata cache records from public CFs
to exercise a second reader route. CF service records are not an SQL Config
snapshot; this test does not assert SQL root/version completeness. Only
byte-equal paths from successful exports enter `storage-baseline.json`.

Run the portable checks without a 1C installation:

```text
python -m unittest discover -s tools/coverage -p test_*.py
cargo test --locked --no-default-features --test coverage_corpus
cargo test --locked --no-default-features --test coverage_fixture_features
cargo test --locked --no-default-features --test coverage_storage_records
cargo test --locked --no-default-features --test coverage_extension_envelope
```

The Python generator checks are included in `cargo xtask ci` and the existing
offline GitHub checks. No new dependency is required.
