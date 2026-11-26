# Managed Form and CommandInterface on 8.3.27

Status: bounded evidence for the standalone compiler profile
`platform-8.3.27.1989`.

The native layouts were recovered from the independently retained inflated
`Config` corpus under
`E:\ibcmd_lab\batch133_register_roots_full_20260718`. The production codecs
are pure Rust and do not invoke 1C, EDT or a JVM.

## Managed Form body

The selected layout is
`managed-form-marker50-v1-raw-deflate-utf8-bom`. A body is one raw-DEFLATE
stream whose plaintext starts with a UTF-8 BOM and a type-`4` container. Its
layout root uses marker `50`, followed by the module string and exactly four
typed sections: attributes, parameters, form commands and form command
interface. The marker-50 root child span is counted and followed by the 24
platform trailer fields observed in the 8.3.27 corpus.

A sample of 100 retained 8.3.27 CommonForm bodies used marker `50`; historical
marker-`59` unit fixtures remain readable only through the explicitly named
legacy compatibility bridge. They cannot be selected by the standalone
profile encoder or strict decoder.

The base-free formatter starts from a profile-owned empty marker-50 shape, not
from an installed platform or an active `Config` row. It then materializes the
typed source model:

- root scalars, localized text, supported root events and AutoCommandBar;
- deterministic command UUIDs and form-item UUIDs derived from kind, numeric
  id and source name;
- nested container, page, button, field, table and search-addition trees;
- typed attributes, parameters, form commands, local command references and
  module text;
- the four counted trailing sections in stable source order.

The representative test matrix covers CommandBar, Button, UsualGroup, Pages,
Page, Table, InputField, LabelField, CheckBoxField, TextDocumentField,
PictureDecoration and all three search/status additions. It compiles twice to
identical compressed bytes, is decoded by the bounded strict codec and is
exported back through the MSSQL compatibility adapter. Assertions compare the
source-visible element kinds, ids, names, data paths, commands, attributes,
parameters and module text. This round-trip also caught and corrected a legacy
creation bug where a compact LabelField was emitted with the InputField
discriminator.

Unsupported element-creation shapes, ambiguous event identifiers, duplicated
ids/names, unresolved external references, report/property-bag variants,
mobile command-bar content and embedded item assets are explicit blockers.
Unknown native markers, missing BOMs, malformed containers and any trailing
section count other than four fail before bootstrap output is accepted.

Representative retained plaintext evidence:

| Source role | Native row | Inflated SHA-256 |
| --- | --- | --- |
| CommonForm marker-50 body | `3e4361e9-2baf-407b-9320-b2a265b44e46.0` | `3027f39783cc3c1d722c2d008657b5839eae9d7309a62b5ed22e144607c7c812` |

## CommandInterface body

The selected layout is
`command-interface-v7-sections-v1-raw-deflate-utf8-bom`. Its typed model owns
all five ordered sections:

1. command visibility and common flag;
2. command placement, group and auto/manual mode;
3. command order inside groups;
4. subsystem order;
5. command-group order.

Command references retain their numeric kind and UUID rather than being
reconstructed from display names. Empty command sentinels are accepted only in
the native positions where they are evidenced. Counts, UUIDs, duplicate
entries, placement modes and the fixed trailing marker are validated under the
shared depth/node/payload limits. New raw-reference XML compilation now routes
through this typed codec and emits the required BOM; readable XCF references
that do not contain their native kind remain blocked until a canonical graph
can prove the complete tuple.

The legacy export adapter delegates inflation and structural validation to the
same codec. Its compatibility entry point accepts old no-BOM fixtures, while
profile-selected compilation and decoding remain strict.

Representative retained plaintext evidence:

| Source role | Native row | Inflated SHA-256 |
| --- | --- | --- |
| Subsystem five-section CommandInterface | `65c12682-631e-4654-abf6-66bacc828229.1` | `018a88d6f330dc23d1d3fa3d32ac12c8f18bcdff585266968671d81ef645f455` |

## Routing and compatibility boundary

The source-asset registry marks aggregate Form rows as `ManagedForm` and
`Ext/CommandInterface.xml` rows as `CommandInterface`. Generic asset emission
continues to defer these aggregate bodies to their typed body compilers, so a
Form module is never written as an independent storage row.

Both codecs are enabled only when all independent coordinates select the
evidenced cohort:

- platform build `8.3.27.1989`;
- storage profile `storage:mssql-config-configsave`;
- `bootstrap.body.form.layout` or
  `bootstrap.body.command_interface.layout` with the exact value above.

The bundled 8.3.24 and 8.5.1 profiles do not inherit these constants and fail
selection. Adding another platform version therefore requires an explicit
profile declaration and evidence; it cannot silently reuse marker-50 bytes.

## Verification

```text
cargo test compiler::bodies --lib
cargo test module_blob::tests::audits_ --lib
cargo test compiler::families::assets::tests::registry_paths_and_family_suffixes_are_unique_and_safe --lib
cargo test marker_50_form --lib
cargo check --locked --workspace --all-targets
cargo fmt --all -- --check
```
