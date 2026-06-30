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
Recommended parallel-agent ownership by object family is tracked in
[docs/metadata-agent-slicing.md](docs/metadata-agent-slicing.md).

EDT XML exporter/importer plugin findings are summarized in
[docs/edt-xml-layer-analysis.md](docs/edt-xml-layer-analysis.md). The short
version: use the EDT XML layer as a reference for ordering, file layout and
import hierarchy, not as a production runtime dependency.

| Object / group | Status | Ready | Remaining |
|---|---|---:|---:|
| CommandGroups, CommonCommands, CommonModules, CommonPictures, Constants, DefinedTypes, DocumentNumerators, EventSubscriptions, Ext, FunctionalOptions, FunctionalOptionsParameters, IntegrationServices, Languages, ScheduledJobs, SessionParameters, StyleItems, WSReferences, XDTOPackages | done / byte-identical | 100.0% | 0 |
| Enums | partial | 93.9% | 73 |
| CommonTemplates | partial | 82.0% | 89 |
| Roles | partial | 78.9% | 468 |
| AccumulationRegisters | partial | 59.5% | 182 |
| Reports | partial | 55.0% | 1063 |
| HTTPServices | partial | 50.0% | 5 |
| WebServices | partial | 50.0% | 18 |
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
| Subsystems | partial | 19.2% | 619 |
| FilterCriteria | partial | 14.3% | 6 |
| CommonAttributes | partial | 0.0% | 7 |
| Configuration.xml | partial | 0.0% | 1 |
| **Overall full snapshot** | **partial** | **65.8%** | **16984** |

Latest verified slices:

| Round | Area | Verified progress |
|---|---|---|
| 29 | Native dump performance / ExchangePlan Content.xml | source asset extraction now builds source-layout refs without metadata XML; ExchangePlan content failures report the exact unsupported metadata id, with the next blocker isolated to a missing Constant ref index |
| 29 | Role Rights.xml | object rights preserve serialized object order in export and pack paths |
| 29 | Form.xml | wrapper `55` table `DefaultItem=true` extracts and packs through property-bag key `11` |
| 29 | Object metadata XML | child `Attribute` property tails emit format/edit/fill/history details from native metadata blobs |
| 29 | Register metadata XML | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties are emitted |
| 29 | CommonAttributes | native property detail defaults are emitted before `Content` for source XML 2.20/2.21 |
| 29 | MXL templates | default-only spreadsheet `printSettings` blocks are suppressed to match native source output |
| 29 | Source staging readiness | `versions` rows now report precise base-free blockers instead of a generic bootstrap note |
| 28 | Native dump performance | standalone-content refs resolve without `--extract-metadata-xml`; full timing now advances to an ExchangePlan `Content.xml` blocker |
| 28 | Role Rights.xml | string-backed restriction fields export/import through `<restrictionByCondition><field>` while preserving base layout |
| 28 | Form.xml | wrapper `55` table `UserSettingsGroup` extracts and packs existing numeric property-bag slots |
| 28 | Object metadata XML | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior improved |
| 28 | Register/ExchangePlan metadata/assets | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` BOM/final-newline framing covered |
| 28 | CommonAttributes | content item `xr:ConditionalSeparation` refs are resolved from native settings records |
| 28 | DCS templates | selected template owners build `type_index` so DCS TypeIds can normalize in selected/source-only flows |
| 28 | Source staging readiness | metadata XML rows now report precise base-dependent blockers for incomplete native metadata compilation |
| 27 | Role Rights.xml | nested subsystem and integration service channel refs resolve to fuller native-style child object names |
| 27 | Form.xml | wrapper `55` table items extract and pack existing `AllowGettingCurrentRowURL` boolean slots |
| 27 | Object metadata XML | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers are emitted |
| 27 | Workflow/Register metadata XML | BusinessProcess/Task generated types and AccumulationRegister `IncludeHelpInContents=false` are emitted |
| 27 | Configuration.xml | root `<DefaultRoles>` is emitted from resolved role refs in metadata field `39` |
| 27 | DCS templates | TypeId normalization can use `xr:GeneratedType` entries from XML-shaped metadata text |
| 27 | Source staging readiness | configuration `Ext/HomePageWorkArea.xml` has explicit base-free readiness classification |
| 27 | Native dump performance | `mssql-dump-timing-summary` added; selected 664 MB blob timing run confirmed 3 BCP batches and `fetch_rows_ms=3532` |
| 26 | Native dump performance | native `bcp` row batch cap reduced to 256 MiB and timing JSON reports batch count/max size diagnostics |
| 26 | Role Rights.xml | role object refs use UUID plus serialized tail fields, preserving repeated owner refs and standard-attribute refs |
| 26 | Form.xml | wrapper `55` table items extract and pack observed `UpdateOnDataChange=Auto` through property bag key `14` |
| 26 | Object metadata XML | Catalog standard attribute labels and DataProcessor object/presentation scalars are parsed from metadata blobs |
| 26 | Register/ExchangePlan metadata XML | AccumulationRegister and ExchangePlan generated type `InternalInfo` coverage expanded |
| 26 | Configuration.xml | root `DefaultStyle` and `DefaultLanguage` refs are emitted from UUID-backed metadata fields |
| 26 | DCS templates | observed DCS body `AnyIBRef`/`TypeSet` and settings `xsi:type` values are canonicalized |
| 26 | Source staging readiness | readable `CommandInterface.xml` refs now report precise base-free blockers while raw refs remain base-free |
| 25 | Role Rights.xml | explicit disabled `false` rights without restrictions are preserved in export and pack paths |
| 25 | Form.xml | wrapper `55` table items extract and pack `UseAlternationRowColor` through the existing property bag |
| 25 | Object metadata XML | `Document` metadata emits numbering settings, standard command flag and default form refs |
| 25 | InformationRegister metadata XML | `DefaultRecordForm` and `DefaultListForm` refs are resolved from owner metadata form indexes |
| 25 | Configuration.xml | root scalar application/compatibility properties are emitted in native-shaped metadata XML |
| 25 | MXL templates | SpreadsheetDocument text nodes keep literal quotes while still escaping XML structural characters |
| 25 | Source staging readiness | `Predefined.xml` readiness reports precise base-blob blockers for row order, parent/type slots and trailing fields |
| 24 | Role Rights.xml | role rights objects are exported and packed against base slots in object UUID order |
| 24 | Form.xml | table child items extract and pack `SkipOnInput`, including wrapper `55` layouts |
| 24 | Object metadata XML | `Document` metadata emits `StandardAttributes` with number-type-aware `Number` fill value |
| 24 | CommonAttributes metadata XML | native `Content` metadata/use pairs are parsed, including `use_flag=2` as `DontUse` |
| 24 | DCS templates | DataCompositionSchema template bodies rewrite known `v8:TypeId` values to source-style `v8:Type` references through the metadata type index |
| 24 | ExchangePlan Content.xml | `AutoRecord` mapping is corrected to `0=Deny`, `1=Allow`, `2=Auto` |
| 24 | Source staging readiness | `BusinessProcess/Ext/Flowchart.xml` readiness reports precise base-blob blockers for item order, type slots, events and nested payload shape |
| 23 | Form.xml | wrapper `55` table items extract and pack `AutoRefresh` / `AutoRefreshPeriod` through existing property bag keys |
| 23 | Object metadata XML | `Document` metadata emits owned `Form` and `Template` child refs from generic indexes |
| 23 | Partial metadata XML | `InformationRegister` reads `UseStandardCommands` from the extended native owner tuple |
| 23 | MXL templates | SpreadsheetDocument MOXCEL merge regions preserve `verticalUnmerge` instead of converting it to ordinary `merge` |
| 23 | Source staging readiness | `Role/Ext/Rights.xml` readiness reports precise base-blob blockers instead of a generic reason |
| 23 | Native dump performance | `mssql-dump-config` routes blob-bearing metadata/direct prepare reads through the existing native `bcp` parser and reports `sqlcmd`/`bcp` timing split |
| 22 | Form.xml | `Command/ModifiesSavedData=true` is extracted and packed for form commands |
| 22 | Root/CommonAttributes metadata XML | root `Configuration.xml` child families expanded; `CommonAttribute` emits native-shaped `Content` items and `AutoUse` enum values |
| 22 | Source staging readiness | form body readiness audit now reports precise base-blob blockers for layout, trailing sections, modules and item assets |
| 22 | V8 container layer | module blob V8 container parser/builder moved into a shared tested internal module |
| 21 | Form.xml | nested `AutoCommandBar` items preserve `HorizontalAlign` and `Autofill` settings |
| 21 | Object metadata XML | `DataProcessor` emits `UseStandardCommands` from the metadata blob |
| 21 | Source staging readiness | configuration `Ext/MobileClientSignature.bin` rows are covered as base-free raw-deflated bodies |
| 20 | Form.xml | new `UsualGroup` items preserve extended `Behavior`, `Representation`, `ShowTitle=false`, and nested children |
| 20 | Partial metadata XML | register-family metadata emits `UseStandardCommands` from the metadata blob |
| 20 | Source staging readiness | configuration `Ext/StandaloneConfigurationContent.bin` rows are covered as base-free raw-deflated bodies |
| 19 | Form.xml | new `InputField` items preserve extended options (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`) |
| 19 | Partial metadata XML | `Subsystem` emits `UseStandardCommands` from the metadata blob |
| 19 | Source staging readiness | `WSReference` definition rows are covered as base-free raw-deflated bodies |
| 18 | Form.xml | new top-level `Button` items preserve explicit `DefaultButton` values |
| 18 | Partial metadata XML | `ExchangePlan` emits `UseStandardCommands` from the metadata blob |
| 18 | Configuration.xml | root `Catalog` child object headers are emitted |
| 18 | Source staging readiness | root configuration raw-id `MainSectionCommandInterface.xml` rows are covered as base-free |
| 17 | Form.xml | new `InputField` items preserve explicit `SkipOnInput` values |
| 17 | Partial metadata XML | `Task` generated type `InternalInfo` entries are emitted |
| 17 | MXL templates | SpreadsheetDocument `style:FieldSelectionBackColor` is supported in pack path |
| 17 | Source staging readiness | root configuration raw-id `CommandInterface.xml` rows are covered as base-free |
| 16 | Form.xml | new `LabelField` items preserve explicit `ShowInHeader` values |
| 16 | Partial metadata XML | `AccountingRegister` and `CalculationRegister` generated type `InternalInfo` entries are emitted |
| 16 | Configuration.xml | root `Constant` child object headers are emitted |
| 16 | Source staging readiness | raw-id `CommandInterface.xml` rows can be prepared without active `Config` blobs |
| 15 | Form.xml | `InputField` `DataPath` is packed for existing and new layout entries |
| 15 | Partial metadata XML | `ExchangePlan` and related partial families emit owned `Form`/`Template` child refs |
| 15 | Simple metadata XML | `DocumentJournal` emits owned `Form`/`Template` child refs |
| 15 | Source staging readiness | HTMLDocument template rows are covered as base-free help-style blobs |
| 14 | Form.xml | new `InputField` items preserve explicit `ReadOnly` values |
| 14 | BusinessProcess metadata XML | generated type `InternalInfo` entries are emitted |
| 14 | Task metadata XML | owned `Form` and `Template` child object headers are emitted |
| 14 | Source staging readiness | `Style/Ext/Style.xml` rows are prepared without active `Config` blobs |
| 13 | Form.xml | new `PictureDecoration` child items can be compiled from XML |
| 13 | Catalog metadata XML | owned `Form` and `Template` child object headers are emitted |
| 13 | MXL templates | SpreadsheetDocument `style:ButtonTextColor` is supported in pack and extract paths |
| 13 | Source staging readiness | SpreadsheetDocument `Ext/Template.xml` rows are prepared without active `Config` blobs |

Scope exclusion: `ConfigDumpInfo.xml` is intentionally not generated and is not
counted as parity debt.

`Configuration.xml` support currently covers the root metadata header
(name/synonym/comment/uuid), source XML version selection, and selected root
child object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`);
deeper root properties remain tracked under Issue #22.

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

The `mssql-storage-*` and `mssql-delta-*` commands shell out to `bcp`. Native
`mssql-dump-config` also uses the existing `bcp` native parser for blob-bearing
row fetches in streamed export paths, while lightweight headers/control queries
still use `sqlcmd`. By default these paths invoke `bcp` with native format,
which works across `bcp` versions including `bcp 13` (Microsoft ODBC Driver 13
for SQL Server).

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
