# ibcmd-rs

Research-first Rust tool for building a replacement path for slow 1C configuration
loads from XML sources.

The first milestone started read-only for the target infobase:

- locate 1C command-line tools;
- scan source trees into deterministic manifests;
- compare manifests and produce a load plan;
- profile external `ibcmd` or `1cv8` runs;
- generate SQL Server and 1C technical log trace templates.
- group exported SQL Server Extended Events XML by normalized SQL text.

Guarded direct database writers are now available for researched storage rows.
They are still intended only for throwaway test databases.

## Export Compatibility Status

Current parity tracking is maintained in
[docs/export-parity-status.md](docs/export-parity-status.md). The table below is
the compact top-level view from the last full `ut_ibcmd` export comparison
against native `ibcmd`; incremental verified fixes after that snapshot are
listed in the status document until the next full diff is regenerated.

EDT XML exporter/importer plugin findings are summarized in
[docs/edt-xml-layer-analysis.md](docs/edt-xml-layer-analysis.md). The short
version: use the EDT XML layer as a reference for ordering, file layout and
import hierarchy, not as a production runtime dependency.

| Object / group | Status | Ready | Remaining |
|---|---|---:|---:|
| CommonCommands, CommonModules, CommonPictures, Constants, DefinedTypes, EventSubscriptions, FunctionalOptions, ScheduledJobs, StyleItems, CommandGroups, FunctionalOptionsParameters, Languages, SessionParameters, DocumentNumerators, WSReferences, IntegrationServices, Ext | done / byte-identical | 100.0% | 0 |
| Enums | partial | 93.9% | 73 |
| CommonTemplates | partial | 82.0% | 89 |
| Roles | partial | 78.6% | 476 |
| AccumulationRegisters | partial | 59.5% | 182 |
| Reports | partial | 55.0% | 1063 |
| XDTOPackages | partial | 50.0% | 407 |
| WebServices | partial | 50.0% | 18 |
| HTTPServices | partial | 50.0% | 5 |
| ExchangePlans | partial | 49.7% | 184 |
| DocumentJournals | partial | 49.6% | 61 |
| Tasks | partial | 46.9% | 26 |
| DataProcessors | partial | 45.5% | 3844 |
| Catalogs | partial | 45.2% | 3672 |
| InformationRegisters | partial | 44.0% | 2227 |
| Documents | partial | 43.8% | 3498 |
| ChartsOfCharacteristicTypes | partial | 41.9% | 97 |
| BusinessProcesses | partial | 41.4% | 89 |
| SettingsStorages | partial | 39.0% | 50 |
| CommonForms | partial | 36.8% | 705 |
| FilterCriteria | partial | 14.3% | 6 |
| Subsystems | partial | 9.9% | 690 |
| CommonAttributes | partial | 14.3% | 6 |
| Configuration.xml | partial | 100.0% | 0 |

Scope exclusion: `ConfigDumpInfo.xml` is intentionally not generated and is not
counted as parity debt.

`Configuration.xml` support currently covers the root metadata header
(name/synonym/comment/uuid) and source XML version selection; deeper root
properties remain tracked under Issue #22.

## Commands

```powershell
cargo run -- probe --deep
cargo run -- scan C:\path\to\xml-sources -o current-manifest.json
cargo run -- plan current-manifest.json -b baseline-manifest.json -o load-plan.json
cargo run -- source-diff C:\ibcmd-export C:\ibcmd-rs-dump --path-prefix Catalogs\Валюты -o source-diff.json
cargo run -- profile-run --capture-output -- ibcmd infobase config load ...
cargo run -- dump-sources --settings C:\repo\autumn-properties.json --extension EmergingTravelGroup -o C:\repo\src\cfe\EmergingTravelGroup --overwrite
cargo run -- mssql-dump-config --database MyInfobase -o C:\repo\db-dump --include-config-save --inflate --extract-module-text
cargo run -- trace-template .\trace
cargo run -- trace-analyze .\trace\events.xml -o trace-analysis.json
cargo run -- storage-map .\trace\events.xml -o storage-map.json
cargo run -- compatibility
cargo run -- mssql-compare --left ref_db --right target_db -o compare.json
cargo run -- mssql-clone --source target_db --target ours_db --overwrite --allow-non-lab
cargo run -- mssql-storage-export --database import_only_db -o storage-bundle --overwrite
cargo run -- mssql-storage-import --database empty_target_db -i storage-bundle --replace --allow-non-lab
cargo run -- mssql-delta-export --database staged_db -o delta-bundle --overwrite
cargo run -- mssql-delta-import --database target_db -i delta-bundle --allow-non-lab
cargo run -- module-blob-pack --text CommonModules\...\Ext\Module.bsl --base-blob module.bin -o module.blob
cargo run -- versions-blob-patch -i versions.bin -o versions-new.bin --change <metadata-id> --change <metadata-id>.0
cargo run -- mssql-stage-common-module --database target_db --module-id <metadata-id> --text CommonModules\...\Ext\Module.bsl --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-modules --database target_db --module <metadata-id>=CommonModules\...\Ext\Module.bsl --module <metadata-id>=CommonModules\...\Ext\Module.bsl --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-metadata --database target_db --module-id <metadata-id> --xml CommonModules\...\Module.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-object --database target_db --xml CommonModules\Module.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-common-module-objects --database target_db --xml CommonModules\Module1.xml --xml CommonModules\Module2.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-metadata-objects --database target_db --source-root C:\full\xml-sources --xml Constants\SomeConstant.xml --xml SessionParameters\SomeParameter.xml --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-metadata-objects --database target_db --source-root C:\full\xml-sources --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-common-module-objects --database target_db --source-root C:\full\xml-sources --replace-config-save --allow-non-lab
cargo run -- mssql-stage-source-objects --database target_db --source-root C:\full\xml-sources --source-version 2.21 --replace-config-save --allow-non-lab
```

### bcp client compatibility

The `mssql-storage-*` and `mssql-delta-*` commands shell out to `bcp`. By
default they invoke it with `-T -n` (trusted connection, native format), which
works across `bcp` versions including `bcp 13` (Microsoft ODBC Driver 13 for SQL
Server).

`bcp 18+` (ODBC Driver 18) defaults to encrypted connections and may need the
server certificate to be trusted. Pass `--bcp-trust-cert` to add `bcp -u` (trust
server certificate) in that case. Do **not** set it with `bcp 13` or earlier:
those builds do not recognize `-u` and will fail with an "unknown argument"
usage error.

```powershell
# bcp 18+ over an encrypted connection to a self-signed server:
cargo run -- mssql-storage-export --database import_only_db -o storage-bundle --overwrite --bcp-trust-cert
```

## First ERP Experiment

1. Prepare a disposable ERP infobase on SQL Server.
2. Export or prepare ERP XML sources.
3. Build a baseline manifest:

   ```powershell
   cargo run -- scan C:\erp-src -o baseline-manifest.json
   ```

4. Generate trace templates:

   ```powershell
   cargo run -- trace-template .\trace
   ```

5. Start SQL Server Extended Events from `trace\sqlserver-xevents.sql`.
6. Run the slow load through `profile-run`:

   ```powershell
   cargo run -- profile-run --capture-output -- ibcmd ...
   ```

7. Stop the SQL trace and keep the `.xel`, 1C technical log, manifest and
   profile JSON together.
8. Export event XML from SQL Server and group it:

   ```powershell
   cargo run -- trace-analyze C:\temp\events.xml -o trace-analysis.json
   ```
9. Map the grouped SQL into storage-mutation families:

   ```powershell
   cargo run -- storage-map C:\temp\events.xml -o storage-map.json
   ```

## Roadmap

1. Source model: parse object identity, UUIDs, module/form/template ownership.
2. Storage bundle bridge: export/import `ConfigSave` and `Params` using native
   SQL Server BCP so a prepared import state can be applied in an empty infobase.
3. Delta bundle bridge: export/import staged `ConfigSave` rows for a prepared
   partial change in an existing infobase.
4. Common module body compiler: build a valid module `.0` blob from BSL source
   as `deflate(V8File(info,text))`, using a base blob for element headers.
5. Versions patcher: build a staged `versions` blob by replacing generation,
   optional `root`/`version`/`versions` entries when present, and changed file UUIDs;
   source/metadata staging can append missing changed entries for newly staged body rows.
6. SQL common-module stager: read active `Config`, generate the changed `.0`
   and `versions` blobs, and write the five-row `ConfigSave` staging set.
7. Multi-module stager: stage several common module body changes in one
   `ConfigSave` set with a single patched `versions` blob.
8. Common module metadata stager: stage XML changes for `Name`, `Synonym`,
   `Comment`, execution-context flags, `Privileged`, and `ReturnValuesReuse`
   with a four-row `ConfigSave` set.
9. Common module object stager: stage a complete common module from XML and
   sibling `Ext\Module.bsl` in one five-row `ConfigSave` set.
10. Batch common module object stager: stage several complete common modules
   from XML plus sibling `Ext\Module.bsl` files with one shared `versions` blob.
11. Generic simple metadata stager: stage metadata-only XML changes for
   `Name`, `Synonym`, and `Comment` while preserving the rest of each metadata
   blob; verified on `Constant` and `SessionParameter`. For `Constant`, it also
   stages supported `Type` patterns (`boolean`, `string`, `decimal`,
   `dateTime`) and `UseStandardCommands`. For `DefinedType`, it stages builtin
   `Type` patterns with one or more `boolean`, `string`, `decimal`, and
   `dateTime` entries, plus `cfg:*` reference types resolved from
   generated `TypeId` values under `--source-root`. For `CommonCommand`, it stages
   `Representation`, `ToolTip`, `IncludeHelpInContents`, `ParameterUseMode`,
   `ModifiesData`, `Picture` for empty or `CommonPicture.<name>` refs,
   `CommandParameterType` for empty or a single `cfg:DefinedType.<name>`, and
   the currently observed `OnMainServerUnavalableBehavior` value `Auto`.
   Reference resolution requires `--source-root`; `StdPicture.User` is mapped
   to the platform-owned user picture UUID, while other `StdPicture.*` values
   and arbitrary multi-type command parameter sets are still rejected until
   mapped.
12. SQL verifier: compare table shape, row counts and later row checksums.
13. Trace analyzer: expand `.xel` export support and add more robust SQL
   normalization.
14. Storage mapper: map 1C metadata operations to observed SQL mutations per
   platform version.
15. Writer hardening: keep destructive staging behind explicit confirmation
   flags and add more safety gates before non-lab use.
16. Compatibility matrix: platform build, DBMS, compatibility mode, configuration
   type and supported operation set.
