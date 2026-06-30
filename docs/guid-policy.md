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
- Suspicious production DB-specific items: none found in this pass.
