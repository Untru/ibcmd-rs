# Guarded XCF 2.21 to 2.20 downgrade evidence

MIG-004 adds a verified downgrade edge from `xml-2.21` to `xml-2.20`. It is a
standalone Rust path with no runtime dependency on 1C, EDT, Java, or a JVM. The
same graph can now plan the exact profile pair in both directions; neither
direction is inferred from version strings.

## Portable and lossy cases

The strict `Constant` and `Catalog` cohort, including evidenced `Catalog`
descendants, is semantically portable. The target XML adapter changes the root
version and removes the known 2.21 `pal` namespace from `Catalog`. The 2.20
adapter reads those outputs with the same semantic digest and an empty loss
report.

`UseInInterfaceCompatibilityMode=Any` on 2.21 managed-form metadata is treated
differently. The bounded canonical fixture allows only a typed `Name`, that
exact enum value, no owner, and no references, assets, generated types, or
opaque facets. This narrow projection prevents an unrelated or future form
property from passing through the downgrade silently.

The stable loss code is
`migration.xcf-2-21-to-2-20.use-in-interface-compatibility-mode`. Every
occurrence is addressed by its exact canonical object and property paths. Its
policy behavior is:

- `error` (default): returns no candidate model and an error diagnostic;
- `warn`: records `continue_with_warning` and retains the property in canonical
  IR, so a later target preflight still cannot mistake it for encoded 2.20;
- `drop`: removes only this exact `Any` property and records
  `dropped_explicitly` under a codec-owned `DropAllowed` declaration.

Any other interface-mode value, family, property, relationship, opaque form
facet, or non-2.21 provenance blocks during analysis. The source model is never
mutated under any policy.

## Fixtures and verification

`tests/fixtures/migrations/2.21-to-2.20/manifest.json` SHA-256 pins:

- an unchanged Constant;
- a Catalog with the changed palette namespace;
- the path-addressed lossy managed-form projection.

The integration suite checks target-adapter readability and semantic equality
for the portable XML fixtures, all three loss policies, stable JSON report
evidence, source immutability, and exact bidirectional graph planning.

```text
cargo test -p ibcmd-core migration::v2_21_to_v2_20
cargo test --test migration_2_21_to_2_20
```
