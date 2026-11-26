# Catalog and Document native layout evidence for 8.3.27.1989

Status: implemented as the experimental `catalog-v1-crlf-utf8-bom` and
`document-v1-crlf-utf8-bom` layouts. Both codecs are standalone: they read the
validated canonical graph and do not start 1C, EDT, a JVM, or read an existing
configuration row as a base artifact.

## Retained sources

- Minimal Catalog XCF:
  `E:\ibcmd_lab\ut\ut_ibcmd\Catalogs\Производители.xml`.
- Its inflated native primary row:
  `E:\ibcmd_lab\060726\full_compare_20260706_103131\ibcmd_rs_dump\Config_inflated\369232c9-b721-40af-a244-f6588346f044__part0.txt`.
- Child-rich Catalog XCF:
  `E:\ibcmd_lab\ut\ut_ibcmd\Catalogs\КлассификаторПолномочийФНСМЧД002.xml`.
- Minimal Document XCF:
  `E:\ibcmd_lab\ut\ut_ibcmd\Documents\УдалитьКассоваяСмена.xml`.
- Its inflated native primary row:
  `E:\ibcmd_lab\060726\full_compare_20260706_103131\ibcmd_rs_dump\Config_inflated\0f880343-b3fb-4fbf-897b-6dd5130aadc1__part0.txt`.
- Child-rich Document XCF:
  `E:\ibcmd_lab\ut\ut_ibcmd\Documents\РассылкаКлиентам.xml`.
- The legacy read-only extractors in `src/mssql_dump/mod.rs` provide an
  independent parser for the retained collection UUIDs and child wrappers.

The compiler fixtures cover a minimal and a child-rich object for each family.
The rich fixtures include a direct attribute, tabular section with a nested
attribute, command, form, template, a field reference, a generated reference
type, and a default-form reference.

## Exact native grammar

Both primary rows have an exact eight-field root. Root discriminator is `1`,
the object body is field 1, field 2 is the exact outer collection count `5`,
and fields 3 through 7 contain the five family-specific collections.

The Catalog body has exactly 61 fields and discriminator `57`. Its five root
generated TypeId/ValueId pairs occupy slots 1/2, 3/4, 5/6, 7/8, and 34/35 for
Object, Ref, Selection, List, and Manager. Its collection UUIDs are:

- templates: `3daea016-69b7-4ed4-9453-127911372fe6`;
- commands: `4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6`;
- tabular sections: `932159f9-95b2-4e76-a8dd-8849fe5c5ded`;
- attributes: `cf4abea7-37b2-11d4-940f-008048da11f9`;
- forms: `fdf816d2-1ead-11d5-b975-0050bae0a95d`.

The Document body has exactly 53 fields and discriminator `40`. Its Object,
Ref, Selection, List, and Manager pairs occupy slots 1/2, 3/4, 5/6, 7/8, and
26/27. Its collection UUIDs are:

- tabular sections: `21c53e09-8950-4b5e-a6a0-1054f1bbc274`;
- templates: `3daea016-69b7-4ed4-9453-127911372fe6`;
- attributes: `45e46cbc-3e24-4165-8b7b-cc98a6f80211`;
- commands: `b544fc6a-2ba3-4885-8fb2-cb289fb6d65e`;
- forms: `fb880e93-47d7-4127-9357-a20e69c17545`.

For both families, nested tabular attributes use collection UUID
`888744e1-b616-11d4-9436-004095e12fc7`. Direct and nested attribute wrappers,
tabular-section generated identities, command ValueId, common headers, counts,
and UUID inventories are checked exactly. Embedded attributes, tabular
sections, nested attributes, and commands remain inside the owner row. Forms
and templates are UUID references to separately routed rows; readable names
are resolved only through the validated canonical graph.

The selected layout emits a UTF-8 BOM, deterministic CRLF text, and raw
DEFLATE. Missing or additional root/body fields, unknown collection markers,
count mismatches, unresolved references, duplicate identities, malformed type
patterns, and future layout constants fail closed.

## Supported boundary and versioning

The current strict slice supports the evidenced scalar root properties,
generated types, attributes and their retained type qualifiers, tabular
sections, commands, and form/template references. Non-empty
`StandardAttributes` or `Characteristics`, and complex properties without a
retained native mapping, are rejected instead of guessed or discarded.

XCF 2.20 and 2.21 are independent XML-dialect inputs over these two explicitly
selected native layouts. Platform build and storage profile remain independent
axes. A future platform version must declare its own Catalog and Document
layout constants and pass family fixtures; it is never treated as compatible
merely because its version number is newer.
