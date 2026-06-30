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
| Enums | partial | 95.1% | 59 |
| CommonTemplates | partial | 82.6% | 86 |
| Reports | partial | 67.6% | 765 |
| AccumulationRegisters | partial | 59.5% | 182 |
| Roles | partial | 54.4% | 1013 |
| ExchangePlans | partial | 51.9% | 176 |
| HTTPServices | partial | 50.0% | 5 |
| WebServices | partial | 50.0% | 18 |
| DocumentJournals | partial | 49.6% | 61 |
| DataProcessors | partial | 49.0% | 3602 |
| Tasks | partial | 46.9% | 26 |
| Catalogs | partial | 45.6% | 3650 |
| Documents | partial | 45.0% | 3420 |
| InformationRegisters | partial | 44.0% | 2227 |
| ChartsOfCharacteristicTypes | partial | 41.9% | 97 |
| BusinessProcesses | partial | 41.4% | 89 |
| SettingsStorages | partial | 39.0% | 50 |
| CommonForms | partial | 36.8% | 705 |
| Subsystems | partial | 19.2% | 619 |
| FilterCriteria | partial | 14.3% | 6 |
| CommonAttributes | partial | 0.0% | 7 |
| Configuration.xml | partial | 0.0% | 1 |
| **Overall full snapshot** | **partial** | **66.0%** | **16864** |

Latest verified slices:

| Round | Area | Verified progress |
|---|---|---|
| 39 | Form.xml | default `Table` child item `SkipOnInput=false` is suppressed while explicit `true` remains exported |
| 39 | MXL templates | MOXCEL system style refs `-25`, `-26`, `-27`, `-34`, `-35`, `-36`, `-37`, and `-38` now map to native report/table/button style color refs |
| 39 | InformationRegister metadata XML | extended owner tuple field emits `DataLockControlMode` for `InformationRegister` as well as `AccumulationRegister` |
| 39 | Role Rights.xml | role rights object ordering can follow metadata tree order while preserving serialized order within one owner |
| 39 | Source staging readiness | Form body base-dependency audit now reports precise counts for root/layout scalars, child items, trailing sections and `Ext/Form/Items/**` assets |
| 38 | Form.xml | default `WindowOpeningMode=DontBlock` is suppressed in export while explicit XML still packs into the form layout |
| 38 | MXL templates | native empty format slots `{0}` are preserved, so later width-bearing formats are no longer dropped |
| 38 | MXL templates | mixed `NamedItemCells` / `NamedItemDrawing` lists now preserve valid named areas without packing bogus drawing areas |
| 38 | Object metadata XML | Document and Report child attributes now reuse shared property-tail extraction, including `DataHistory` after `ChoiceForm` |
| 38 | Subsystems | metadata scalar tail now emits `IncludeHelpInContents`, `IncludeInCommandInterface`, and native `UseOneCommand` |
| 38 | Source staging readiness | `XDTOPackages/*/Ext/Package.bin` is covered as base-free raw-deflated staging and source-load coverage |
| 37 | Form.xml | dynamic-list `Settings/Field` entries now export `dataPath`/`field` and pack back into the serialized settings bag |
| 37 | MXL templates | `textPlacement=Cut` now extracts from MOXCEL code `1` and packs back to the same format bit |
| 37 | MXL templates | non-1-based column `formatIndex` values are preserved separately from the normalized internal indexes used for row/cell decoding |
| 37 | Configuration.xml | root `CommonPicture` child objects are classified by native field shape and emitted in `Configuration.xml` for source XML 2.20/2.21 |
| 37 | Object metadata XML | child attribute `ChoiceForm` tails now preserve empty values and resolved non-empty design-time refs |
| 37 | Source staging readiness | root `Ext/MainSectionPicture.xml` is covered as base-free staging and source-load coverage |
| 36 | Diff mining | reusable `source-diff-signatures` CLI added; bounded scan of the last full diff sampled 2,101 XML pairs and shows the largest remaining clusters are still MXL `document/format/*` and `formatIndex` signatures |
| 36 | Form.xml | default `Page/ScrollOnCompress=false` is suppressed while explicit `true` remains exported and packable |
| 36 | Flowchart.xml | BusinessProcess/GraphicalSchema `Font` now decodes serialized style font tuples for connection lines |
| 36 | MXL templates | spreadsheet `textOrientation` format bit 13 extracts to XML and packs back into MOXCEL bodies |
| 36 | CommonAttributes | native property-tail `FillChecking` now emits `ShowError` / `DontCheck` after `FillValue` in source XML order |
| 36 | Source staging readiness | `CommonCommand/Ext/CommandInterface.xml` raw command-interface rows are covered as base-free staging |
| 35 | Diff mining | sampled XML-path mining report added to drive agents by repeated signatures instead of object families |
| 35 | Form.xml | default `ShowCommandBar=true` is suppressed while explicit `false` and explicit packer values remain supported |
| 35 | Object metadata XML | Catalog child attributes now reuse generic property-tail parsing with resolved `ChoiceParameters` refs |
| 35 | Flowchart.xml | BusinessProcess/GraphicalSchema item `ZOrder` exports from serialized item order instead of hardcoded zero |
| 35 | MXL templates | unknown MOXCEL format bits no longer drop the whole format table; known neighboring format properties and indexes are preserved |
| 35 | Source staging readiness | root configuration application modules under `Ext/*.bsl` have explicit base-free readiness and row-generation coverage |
| 35 | Role Rights.xml | form refs resolve and pack in role rights, including owned refs such as `Catalog.<name>.Form.<form>` |
| 34 | Source staging readiness | `Ext/ParentConfigurations.bin` now stages base-free by re-deflating inflated raw-deflated source bytes into the `ConfigSave` row |
| 34 | Object metadata XML | Catalog owner refs are emitted as ordered `xr:MDObjectRef` items in `<Owners>` |
| 34 | DCS templates | calculated-field `TypeId` normalization uses the native `d4p1` current-config namespace context |
| 34 | Form.xml | wrapper `55` table `RowFilter xsi:nil="true"` extracts and packs through property-bag key `10` |
| 34 | ExchangePlan Content.xml | content items are ordered by configuration metadata tree order with blob order as fallback |
| 34 | CommonAttributes | property-tail `FillValue` emits string, nil, decimal and boolean XML values between fill-from and fill-checking fields |
| 33 | Role Rights.xml | HTTPService URL template method refs resolve as `HTTPService.<service>.URLTemplate.<template>.Method.<method>` and pack back from source XML |
| 33 | DCS templates | unqualified DCS core `xsi:type` values normalize to `dcscor:*`; data-core `StandardPeriod` / `StandardPeriodVariant` normalize to `v8:*` |
| 33 | Form.xml | wrapper `55` table `Period` extracts and packs through property-bag key `7` as `v8:StandardPeriodVariant=Custom` |
| 33 | Object metadata XML | DataProcessor child metadata emits non-empty `ChoiceParameters`, resolving design-time UUIDs to stable refs and fixed arrays |
| 33 | ExchangePlan metadata XML | optional `xr:ThisNode` is emitted in `InternalInfo`, and `UseStandardCommands` is read relative to the detected header slot |
| 33 | Source staging readiness | module `.bin` source assets containing inflated V8 containers can stage base-free for common/object/nested module bodies |
| 32 | Native dump performance / Form.xml CPU | consolidated form child-item support index traversal; full source-layout after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 68,886 to 40,598 and `source_asset_form_xml_cpu_ms` from 118,673 to 102,296 |
| 32 | Form.xml | wrapper `55` table `RestoreCurrentRow` extracts and packs through property-bag key `12` |
| 32 | Object metadata XML | Document owner `<IncludeHelpInContents>` is emitted after default/auxiliary form properties |
| 32 | ExchangePlan metadata XML | code-4/code-27 child attributes are emitted with value types and property tails |
| 32 | Configuration.xml | localized root information fields are emitted (`BriefInformation`, `DetailedInformation`, copyright and information addresses) |
| 32 | Source staging readiness | unsupported `AdditionalIndexes.xml` families now report a precise base-blob blocker instead of being silently omitted |
| 31 | Native dump performance / Form.xml CPU | full source-layout after-run completed without `ConfigDumpInfo.xml`; `InputField` option-bag parsing is cached per form item, reducing `source_asset_form_child_items_cpu_ms` from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507 |
| 31 | Role Rights.xml | Task child refs now map and pack as `Task.<name>.AddressingAttribute.<child>` instead of ordinary attributes |
| 31 | Form.xml | wrapper `55` table `ChoiceFoldersAndItems` extracts and packs through existing typed table slot |
| 31 | Object metadata XML | tabular-section property tails are emitted, and child property tails now include empty `<LinkByType/>` between `ChoiceForm` and `ChoiceHistoryOnInput` |
| 31 | Register metadata XML | register dimensions/resources/attributes reuse generic child object parsing and emit decoded types plus common property tails |
| 31 | Configuration.xml | root settings-storage refs are emitted from UUID-backed metadata fields |
| 31 | DCS templates | generated type id lookup is case-insensitive for DataCompositionSchema template bodies |
| 31 | Source staging readiness | WebService `Ext/Module.bsl` is covered as a base-free source staging body |
| 30 | Native dump performance / ExchangePlan Content.xml | selected ExchangePlan content now requests `object_refs`; selected and full source-layout exports completed without `ConfigDumpInfo.xml`, with full timing captured under `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source-report.json` |
| 30 | Role Rights.xml | import packing maps rights objects by source-resolved refs instead of XML order, including commands, child dimensions/resources/attributes, standard attributes, and direct metadata refs |
| 30 | Form.xml | wrapper `55` table `RowPictureDataPath` extracts and packs through string property-bag key `19` |
| 30 | Object metadata XML | child `Attribute` tail properties now include choice/use/indexing/full-text/data-history scalar details |
| 30 | Object metadata XML | Report/DataProcessor child commands emit command property tails from owner metadata blobs |
| 30 | Register metadata XML | InformationRegister and AccumulationRegister standard attributes are emitted; AccumulationRegister type value `1` now maps to `Turnovers` |
| 30 | CommonAttributes | native separation tail properties and UUID-backed separation refs are emitted |
| 30 | DCS templates | current-config `v8:Type` prefixes normalize by DCS context (`d4p1`, `d5p1`, `d6p1`) |
| 30 | Source staging readiness | FilterCriterion manager modules can stage base-free from `FilterCriteria/<Name>/Ext/ManagerModule.bsl` |
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
(name/synonym/comment/uuid), source XML version selection, selected root child
object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`) and
selected root refs such as default roles/style/language/settings storages;
localized root information fields are also covered;
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
