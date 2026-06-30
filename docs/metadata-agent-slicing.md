# Metadata Agent Slicing

This table is the current practical split for parallel export/import parity
work. It favors independent ownership over launching one agent for every visible
README row at once: many rows share the same compiler/decompiler files and would
otherwise conflict heavily.

Use `E:\ibcmd_lab` for any lab exports/imports. The import path remains staging
over an existing compatible infobase, not bootstrap into an empty database. Do
not generate or require `ConfigDumpInfo.xml`.

| Slice | Object rows | Primary issue | Primary files | Recommended parallelism |
|---|---|---:|---|---|
| Form compiler/decompiler | CommonForms plus all object `Ext/Form.xml` bodies | #16 | `src/module_blob.rs`, `src/mssql_dump.rs` for decompile-only slices | one agent per round |
| MXL/SKD/template bodies | CommonTemplates and object Templates | #17 | `src/module_blob.rs`, `src/mssql_dump.rs`, `src/mssql.rs` for staging coverage | one agent per round, not with form packer edits in the same file |
| Role rights | Roles | #13 | `src/module_blob.rs` | one focused agent |
| Object metadata, high-volume owners | Catalogs, Documents, DataProcessors, Reports | #15 | `src/mssql_dump.rs`, `src/module_blob.rs` for pack/stage follow-up | split by owner family when tests are isolated |
| Register-like partial metadata | InformationRegisters, AccumulationRegisters, AccountingRegisters, CalculationRegisters | #18 | `src/mssql_dump.rs` | one family or one property layer per agent |
| Workflow partial metadata | BusinessProcesses, Tasks | #18 / #14 | `src/mssql_dump.rs`, `src/mssql.rs` for body rows | one family per agent |
| Subsystems / ExchangePlans | Subsystems, ExchangePlans | #18 | `src/mssql_dump.rs`, `src/mssql.rs` for `Ext/*` rows | one object type per agent |
| Simple metadata tail | Enums, DocumentJournals, SettingsStorages, FilterCriteria, HTTPServices, WebServices, XDTOPackages | #14 | `src/mssql_dump.rs`, `src/module_blob.rs`, `src/mssql.rs` | group by storage layout, not by README row |
| Configuration root | Configuration.xml, CommonAttributes | #22 | `src/mssql_dump.rs`, `src/module_blob.rs` | one focused root/child-object layer per agent |
| Base-free staging | body rows that can be generated without active `Config` blobs | #21 | `src/mssql.rs` | one agent per round, independent from export XML agents |
| Performance | selected export timing and memory regressions | #19 | `src/mssql_dump.rs`, profiling docs under `docs/` | run only when a measurable lab command is available |

Best next batching:

| Batch | Agents | Why |
|---|---|---|
| A | #16 Form, #18 partial metadata, #21 base-free staging | Low file overlap and directly targets the top three blockers. |
| B | #17 templates, #15 object metadata, #22 root metadata | Good follow-up batch after form/partial/base-free merges are stable. |
| C | #13 roles, #14 simple metadata, #19 performance | Focused cleanup and validation batch; performance should use real lab timings. |

Avoid running more than five code-writing agents at once until the remaining
work is split into disjoint file ownership. The limiting factor is not the
number of open README rows; it is the concentration of changes in
`src/module_blob.rs` and `src/mssql_dump.rs`.
