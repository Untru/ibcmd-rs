# Rights, Predefined and support data on 8.3.27

Status: bounded evidence for the standalone compiler profile
`platform-8.3.27.1989`.

The native layouts were recovered by comparing XCF sources with inflated
`Config` rows retained under
`E:\ibcmd_lab\batch133_register_roots_full_20260718`. The implementation is
pure Rust and does not invoke 1C, EDT or a JVM at runtime.

## Role Rights

The selected layout is `rights-v1-raw-deflate-utf8-bom`. Its root marker is
`10`; it contains the counted object table, counted restriction-template
table, three role flags and the fixed trailing marker. Object references keep
their owner UUID plus explicit kind/slot fields, so ordinary objects and
standard attributes do not depend on a base row. Right UUIDs, enabled/disabled
values and condition restrictions are emitted directly from typed input.

The bounded decoder validates root arity, counts, UUIDs, right values,
restriction wrappers, templates and trailing fields. It retains formatting in
the shared native AST, including a CRLF immediately after an opening brace, so
decode/serialize preserves the evidenced plaintext. The transitional MSSQL
export bridge now delegates body decoding and structural validation to this
codec; its historical no-BOM test cohort is accepted only by an explicitly
named compatibility reader, not by profile-gated compilation.

Representative inflated plaintext evidence:

| Source role | Native row | SHA-256 |
| --- | --- | --- |
| empty object table | `737404de-5b9d-4445-81c5-2c5b50b36846.0` | `73da4ddc5ed16b7ca8a30299dd680cc47c3e87f61f080efc2c274149d6a7b60f` |
| one object / `View` | `8bf1beb4-e19b-48ac-b70a-3a1b3016da33.0` | `27a0497abfdba9338ac73e8ad9b87e2504c529e6be819247979f4c6d4e4633e3` |
| RLS conditions and templates | `6260799c-c3b3-49e6-9d68-b8604d05371a.0` | `d188ec6c34e6f9d0f9bceeacd00b83ee706c3ee3e6a2ae9290bb06e0d3c101ba` |

Unit fixtures verify exact plaintext for the empty and one-object layouts and
base-free condition-restriction round-trip. Unknown roots, invalid counts,
UUIDs, values or restriction shapes fail closed.

## Predefined data

The profile selects `predefined-v1-raw-deflate-utf8-bom`. The registry owns
all four source/native routes:

| Family | Native suffix | Root marker | Current typed capability |
| --- | ---: | ---: | --- |
| Catalog | `.1c` | `0` | base-free compile and strict typed round-trip |
| ChartOfCharacteristicTypes | `.7` | `1` | exact same-profile opaque passthrough |
| ChartOfAccounts | `.9` | `2` | exact same-profile opaque passthrough |
| ChartOfCalculationTypes | `.2` | `9` | exact same-profile opaque passthrough |

The typed Catalog codec builds the seven-column schema, profile-known
predefined-item Type UUID, synthetic root, deterministic preorder row indexes,
nested child tables and the explicit CodeLength/DescriptionLength string
patterns. Every item carries its UUID, folder bit, name, code and description;
non-folder children, excessive lengths, invalid UUIDs and unknown tails are
rejected before bytes are produced.

The representative Catalog row
`f31f3dab-5796-47ac-a54d-3081f8cad817.1c` has inflated SHA-256
`2ca013a2d69a1daf37a8d9af6cf8f69a208fd4519db0478f5b676a8b2c76b865`.
The fixture covers its `CodeLength=0`, `DescriptionLength=150` cohort and
round-trips nested item models without reading a base artifact.

Chart layouts contain family-specific accounting flags, characteristic value
types, calculation links and other facets that are not interchangeable with
the Catalog schema. They are therefore not guessed. A decoded chart body keeps
its original compressed bytes and source profile; only byte-identical
same-profile output is allowed. Cross-profile reuse returns a typed blocker
until an independently evidenced typed layout/migration is added.

## Support and signatures

`ParentConfigurations.bin`, `MobileClientSignature.bin` and
`StandaloneConfigurationContent.bin` are classified as `OpaqueSupport` rather
than generic raw-binary assets. An opaque value can only be captured with:

- an exact source profile and source path;
- observed bytes;
- an independently supplied lowercase SHA-256 that matches those bytes.

Same-profile transfer returns the original bytes and digest unchanged.
Cross-profile transfer is blocked by default. The only drop path requires the
explicit `Drop` policy and returns a structured loss report containing kind,
source/target profiles, path, digest and reason. Empty signatures, digest
mismatches and attempts to move a signature to another profile fail closed;
there is deliberately no signature-generation API.

## Compatibility boundary

The body codecs are enabled only when all independent profile coordinates
match the evidenced platform/storage cohort and the corresponding profile
constant is present:

- `bootstrap.body.rights.layout`;
- `bootstrap.body.predefined.layout`;
- `bootstrap.body.support.layout`;
- platform build `8.3.27.1989`;
- storage profile `storage:mssql-config-configsave`.

The bundled 8.3.24 and 8.5.1 profiles do not inherit these declarations and
fail selection. A new platform version therefore adds explicit evidence and a
new profile declaration; opaque bytes never become an implicit migration.

## Verification

The standalone checks used for this slice are:

```text
cargo test compiler::bodies --lib
cargo test compiler::families::assets::tests::registry_paths_and_family_suffixes_are_unique_and_safe --lib
cargo check --workspace --all-targets
cargo fmt --all -- --check
```

Strict workspace Clippy still reports the repository's pre-existing lint
backlog; it reports no diagnostic in the new `compiler/bodies` modules.
