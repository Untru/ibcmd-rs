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
- Confirmed config-specific production behavior that must be removed or
  replaced with structural parsing:
  - `normalize_form_dynamic_list_settings` special-cases `СписокЗаказов` and
    `Document.ЗаказКлиента`;
  - form picture and numeric-label decisions depend on concrete Russian item
    names and data paths;
  - role-right reconstruction contains mappings between concrete Russian
    characteristic-plan attribute and tabular-section names.
- Resolved in `6ee75f0`: the concrete command-group UUID to
  `CommandGroup.Органайзер` mapping was removed. Custom command groups now use
  current-metadata `object_refs`. Separate parent/current exports were identical
  across all 12,197 relative paths, lengths, and SHA-256 content hashes.
- `FORM_GLOBAL_COMMAND_SOURCE_TYPE_UUID` (`2ef6d6fa-...`) is not in that
  category. It is accepted only in exact typed command-source record shapes,
  maps to the platform token `FormCommandPanelGlobalCommands`, and was also
  observed in an independent infobase sample. A second full native-tree parity
  run remains the stronger portability gate.
