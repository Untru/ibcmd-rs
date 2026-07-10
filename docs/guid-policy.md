# GUID Literal Policy

Production code must not depend on GUID literals copied from a concrete
infobase or configuration. DB/config metadata UUIDs from samples such as
`ut_ibcmd`, `uha`, or `sfc` are allowed only in tests, fixtures, or lab notes.

Production GUID literals are allowed only when they are documented
platform-level 1C constants. Typical examples are standard picture IDs,
built-in type IDs, form property or command IDs, metadata/form marker IDs, and
role right IDs.

When adding a production GUID literal:

- name it as a semantic constant or keep it inside a clearly named platform
  mapping;
- document what platform structure, property, or command it represents;
- add or keep tests proving behavior is driven by the current database metadata,
  not by one concrete database object's UUID;
- if the GUID was observed in only one database sample, parse or discover the
  value from the current database blob/index instead of hardcoding it.

Synthetic GUIDs such as `aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa` may be used in
unit tests. Real DB/config metadata UUIDs may be recorded in lab documents, but
must not drive generic production behavior.

## Current Audit

Commands used:

```powershell
rg -n "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}" src --glob '!target/**'
rg -n "const .*UUID|FORM_.*UUID|STD_.*UUID|UUID" src\mssql_dump.rs
```

Classification:

- Platform constants in production: standard pictures, form list markers, form
  property/command/event IDs, common command groups, DCS form setting section
  IDs, built-in type IDs, and role right UUID-to-name maps.
- Tests and synthetic fixtures: repeated `aaaaaaaa`/`bbbbbbbb` style UUIDs and
  focused sample blobs under `#[cfg(test)]` modules in `src`.
- Lab documentation: real SFC/SSL sample UUIDs appear in `docs/*lab*` and
  coverage notes as audit evidence only.
- Resolved in the accepted UsualGroup style gate: the inverse Form pack path no
  longer injects the BSP `StyleItem.ГиперссылкаЦвет` UUID. Custom title colors
  and fonts resolve through the current metadata index; built-in title-color
  codes use the field-specific platform mapping (`-3 -> FormTextColor`,
  `-21 -> FieldSelectionBackColor`). The final gate changed exactly six
  `Form.xml` files by adding 13 native lines and no non-native lines. A scan of
  232 production UUID literals against 10,080 exported metadata-object UUIDs
  found an empty intersection; the removed BSP UUID has no production hit.
- Resolved in `6ee75f0`: the concrete command-group UUID to
  `CommandGroup.Органайзер` mapping was removed. Custom command groups now use
  current-metadata `object_refs`. Separate parent/current exports were identical
  across all 12,197 relative paths, lengths, and SHA-256 content hashes.
- Resolved in `8485b02`: Input/Label field right alignment now comes from raw
  slot `23 + top_level_offset`, not `СоставЗаказа.*` data paths. The full gate
  restored 13 native `HorizontalAlign=Right` lines across ten forms with zero
  target additions.
- Resolved in `6b70292`: the `СписокЗаказов` plus
  `Document.ЗаказКлиента` dynamic-list fallback was removed. The raw
  `AutoSaveUserSettings` setting already reaches the generic normalizer.
  Separate before/after exports contained 12,197 files each and had a zero
  whole-tree content diff.
- Resolved in `1141be9`: Role Rights no longer renames characteristic-plan
  attributes through concrete Russian attribute/tabular-section names or
  serialized occurrence. Top-level and tabular-section attributes are resolved
  by their distinct current-metadata child UUIDs. A selected `ut_ibcmd` runtime
  gate was byte-identical across 2,208 files, including two native-exact target
  `Rights.xml` files; the full BSP before/after tree was also identical across
  all 12,198 files.
- Resolved in `fc67436`: Form item picture assets no longer use Russian item
  names, serialized occurrence, or nearest-name windows to choose `Picture`,
  `RowsPicture`, `HeaderPicture`, or `ValuesPicture`. Export and inverse pack
  share a wrapper/property-slot classifier. An over-strict first gate admitted
  only 55 of 61 assets and was rejected before integration; the accepted model
  restores all 61 native paths and SHA-256 hashes. Its isolated and serialized
  full BSP trees were byte-identical to the 12,198-file accepted snapshot.
- `FORM_GLOBAL_COMMAND_SOURCE_TYPE_UUID` (`2ef6d6fa-...`) is not in that
  category. It is accepted only in exact typed command-source record shapes,
  maps to the platform token `FormCommandPanelGlobalCommands`, and was also
  observed in an independent infobase sample. A second full native-tree parity
  run remains the stronger portability gate.
