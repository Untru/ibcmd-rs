# XCF 2.20 to 2.21 migration evidence

MIG-003 implements one explicit, verified, upgrade-only graph edge from
`xml-2.20` to `xml-2.21`. It has no runtime dependency on 1C, EDT, Java, or a
JVM. The edge is intentionally bounded to the strict `Constant` and `Catalog`
codecs plus evidenced `Catalog` descendants (`Attribute`, `TabularSection`,
and `Command`). Any other metadata family, broken owner chain, or source/opaque
provenance other than exact `xml-2.20` fails closed.

## Confirmed deltas

The XML-006 corpus and bundled profiles prove only these deltas used by the
edge:

- the XCF root version changes from `2.20` to `2.21`;
- a 2.21 `Catalog` root gains the exact
  `xmlns:pal="http://v8.1c.ru/8.1/data/ui/colors/palette"` binding;
- the palette namespace and `UseInInterfaceCompatibilityMode` capabilities
  change from explicitly unsupported in `xml-2.20` to supported in
  `xml-2.21`.

Canonical semantics do not change for the supported cohort. The migration step
therefore returns an independently owned, validated model with an empty loss
list; the target XML adapter owns the lexical root-version and palette binding
changes. This keeps XML details out of `ibcmd-core` while ensuring that a 2.21
adapter can read every produced fixture.

The committed fixture manifest covers:

- `unchanged-constant.xml`: only target-dialect serialization changes;
- `changed-catalog.xml`: the evidenced palette namespace is added;
- `newly-supported.json`: both capability transitions are asserted directly
  against the resolved bundled profiles.

Every fixture is SHA-256 pinned in
`tests/fixtures/migrations/2.20-to-2.21/manifest.json`. The integration test
plans and executes the graph edge under the default `error` loss policy,
requires a complete one-step report with no loss evidence, decodes the output
with the exact 2.21 family codec, and compares semantic digests. Core unit tests
also require the stable `migration.xcf-2-20-to-2-21.unknown-delta` diagnostic
for an unevidenced family.

## Reproduction

```text
cargo test -p ibcmd-core migration::v2_20_to_v2_21
cargo test -p ibcmd-xml metadata::business_objects::tests::catalog_cross_profile_encoding_applies_evidenced_palette_delta -- --exact
cargo test --test migration_2_20_to_2_21
```
