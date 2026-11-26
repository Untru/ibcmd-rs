# Native extension publication, 8.5.1.1150

Synthetic/public Coverage control, measured on 2026-10-25 for #578. Base public Coverage plus its own catalog, an adopted public catalog, an object module, a managed form and a text template. No private application content.

Version 1.0.0.1 -> 1.0.0.2; BSL string, form color and text template changed independently. Native CFE build/load/apply/re-export all exited 0. The ten staged packed rows are the complete native namespace; root layout 76 has 77 properties, payload marker 26 has 29 fields. Native publication changes the registry root, registry previous rowversion suffix and Version; no dbStruFinal marker is created. Main configuration, schema and unrelated derived tables are unchanged. Two Params *.ui payloads are licensing/service data and are classified separately, as in docs/apply/params-ui.md.

Source inputs: public 1c-fixtures afcdcb010841f89f07c04a99aa07120a76b7116f, coverage/8.5.1.1150/extensions/combined, corpus-managed-forms and commontemplates. Raw snapshots and SQL database identifiers remain local. Fixtures omit physical row timestamps and retain only the synthetic bytes required for deterministic tests.
