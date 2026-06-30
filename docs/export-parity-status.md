# Export Parity Status

Generated from:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --database ut_ibcmd `
  -o E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\ibcmd_rs_dump `
  --overwrite `
  --inflate `
  --extract-module-text `
  --extract-metadata-xml `
  --source-version 2.20 `
  --no-binary-rows

robocopy `
  E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\ibcmd_rs_dump `
  E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\ibcmd_rs_source_only `
  /E /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw `
  /XF manifest.json *.json

target\release\ibcmd-rs.exe source-diff `
  -o E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json `
  D:\ibcmd-rs\lab\ut_ibcmd_20260629_164647\ibcmd `
  E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\ibcmd_rs_source_only
```

The full JSON report is retained at
`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`.
Round 36 also produced a bounded signature-mining report at
`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\source-diff-signatures-round36.json`
to rank repeated XML path differences for the next agent batches.

Reference: native `ibcmd` export from `ut_ibcmd`.
Candidate: `ibcmd-rs` full export snapshot generated after the round 32
incremental verified fixes and before round 33 agent branches were merged.

Raw source-only summary:

| left_only | right_only | different | unchanged |
|---:|---:|---:|---:|
| 1 | 0 | 16864 | 32758 |

The only `left_only` file is `ConfigDumpInfo.xml`; it remains a deliberate
scope exclusion and is not counted as parity debt.

Overall full-snapshot readiness excluding `ConfigDumpInfo.xml`: **66.0%**
(`32758 / 49622` byte-identical files; 66.02% exact), with **16864** files still different.

Verification history:

| Object | Verification | Result |
|---|---|---|
| Round 36 diff-mining slice | reusable XML-path signature mining | added `source-diff-signatures`; a bounded run sampled 2,101 XML pairs from the last full diff and confirmed the largest remaining clusters are MXL `document/format/*` and `formatIndex` signatures |
| Round 36 Form.xml slice | unit-level extractor/packer verification | default `Page/ScrollOnCompress=false` is omitted from source XML while explicit `true` remains supported |
| Round 36 Flowchart.xml slice | unit-level source asset verification | BusinessProcess/GraphicalSchema connection-line `Font` now decodes serialized style font tuples instead of always emitting the default GUI font |
| Round 36 MXL template slice | unit-level extractor/packer verification | spreadsheet `textOrientation` format bit 13 exports to `<textOrientation>` and packs back into MOXCEL bodies |
| Round 36 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute property-tail `FillChecking` is parsed from the native tail and emitted as `ShowError` / `DontCheck` after `FillValue` |
| Round 36 source staging readiness slice | unit-level readiness/row-generation verification | `CommonCommand/Ext/CommandInterface.xml` raw command-interface source rows can be staged base-free |
| Round 35 diff-mining slice | sampled XML-path diff mining | added `docs/diff-mining-2026-07-01-round35.md` to prioritize repeated signatures such as MXL format indexes, Catalog child tails and Form default over-emission |
| Round 35 Form.xml slice | unit-level extractor/packer verification | default `ShowCommandBar=true` is omitted from source XML while explicit `false` and explicit packer `true` remain supported |
| Round 35 Catalog metadata slice | unit-level metadata XML verification | Catalog child attributes now pass `object_refs` into the shared property-tail parser, resolving `ChoiceParameters` design-time refs |
| Round 35 Flowchart.xml slice | unit-level source asset verification | BusinessProcess/GraphicalSchema item `ZOrder` is derived from serialized item order instead of hardcoded zero |
| Round 35 MXL template slice | unit-level template body verification | unknown MOXCEL format bits are consumed without dropping known neighboring format properties or format indexes |
| Round 35 source staging readiness slice | unit-level readiness/row-generation verification | root configuration application modules under `Ext/*.bsl` are explicitly covered as base-free staging rows |
| Round 35 Role Rights.xml slice | unit-level extractor/packer verification | Role rights object refs now include common and owned form refs, and source packing resolves `Form` child refs |
| Round 34 source staging readiness slice | unit-level row-generation verification | `Ext/ParentConfigurations.bin` source bytes are treated as inflated raw-deflated payload and re-deflated for the `ConfigSave` row without fetching an active base blob |
| Round 34 Catalog metadata slice | unit-level metadata XML verification | `Catalog` owner refs are parsed from native metadata and emitted as ordered `xr:MDObjectRef` items in `<Owners>` |
| Round 34 DCS template slice | unit-level template body verification | DCS `calculatedField` `TypeId` values normalize to `d4p1` current-config refs instead of falling back to `d5p1` |
| Round 34 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowFilter xsi:nil="true"` extracts and packs through property-bag key `10` with the native `{"U"}` marker |
| Round 34 ExchangePlan Content.xml slice | unit-level source asset verification | ExchangePlan content items are ordered by configuration metadata tree order, preserving blob order as fallback for unresolved items |
| Round 34 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute native property-tail `FillValue` emits string, nil, decimal and boolean XML values in native property order |
| CommandGroups | selected export of all 34 `CommandGroups` UUIDs from `ut_ibcmd` | `0 different / 34 unchanged` |
| Languages | selected export of `Languages/Русский.xml` from `ut_ibcmd` | `0 different / 1 unchanged` |
| FunctionalOptionsParameters | selected export of all 9 `FunctionalOptionsParameters` UUIDs from `ut_ibcmd` after selected write-set fix | `0 different / 9 unchanged` |
| SessionParameters | selected export of all 98 `SessionParameters` UUIDs from `ut_ibcmd` via `--file-name-list` | `0 different / 98 unchanged` |
| DocumentNumerators | selected export of all 3 `DocumentNumerators` UUIDs from `ut_ibcmd` via `--file-name-list` | `0 different / 3 unchanged` |
| WSReferences | selected export of `UpdateFilesApiImplService` metadata and `.0` body from `ut_ibcmd` via `--file-name-list` | `0 different / 2 unchanged` |
| IntegrationServices | selected export of `ОбменСообщениями` metadata and `.0` module from `ut_ibcmd` via `--file-name-list` | `0 different / 2 unchanged` |
| Ext/HomePageWorkArea.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.8` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext/ClientApplicationInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.b` from `ut_ibcmd` | byte-identical to native |
| Ext/CommandInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.a` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext/MainSectionCommandInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.9` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext | selected export of configuration-level `.8/.9/.a/.b` from `ut_ibcmd` with metadata indexes | `0 different / 4 unchanged`; all 15 `Ext` files covered by snapshot + selected verification |
| Round 9 Form.xml slice | unit-level packer verification | `TextDocumentField/ReadOnly` now patches existing form layout read-only slot |
| Round 9 DataProcessor metadata slice | unit-level metadata XML verification | owned `DataProcessor` template child refs are emitted in `<ChildObjects>` |
| Round 9 source staging readiness slice | unit-level staging verification | `CommonPicture` and configuration picture rows are prepared without reading active `Config` blobs |
| Round 9 selected command-interface performance slice | unit-level selected export verification | selected `.9` command refs can use targeted owner metadata rows and keep broad fallback |
| Round 10 Form.xml slice | unit-level packer verification | `TextDocumentField/Width` now patches existing and newly created form layout items |
| Round 10 MXL template slice | unit-level formatter verification | Moxel system style `-13` is emitted as `style:FieldTextColor` |
| Round 10 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits nested `CommonAttribute` child object headers |
| Round 10 source staging readiness slice | unit-level staging verification | object/form `Help.xml` rows use deterministic body ids without querying active `Config` |
| Round 11 Form.xml slice | unit-level packer verification | new `CheckBoxField` form child items can be compiled from XML |
| Round 11 Role Rights.xml slice | unit-level rights packer verification | omitted false/no-restriction rights are treated as false while preserving base table shape |
| Round 11 InformationRegister metadata slice | unit-level metadata XML verification | `InformationRegister` emits generated type `InternalInfo` entries |
| Round 11 source staging readiness slice | unit-level staging verification | common/object/nested command module bodies are prepared without active module blobs |
| Round 12 AccumulationRegister metadata slice | unit-level metadata XML verification | `AccumulationRegister` emits generated type `InternalInfo` entries |
| Round 12 Form.xml slice | unit-level packer verification | `Pages` / `Page` layout items support additional creation and patch properties |
| Round 12 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits nested `CommonModule` child object headers |
| Round 12 source staging readiness slice | unit-level staging verification | `ExchangePlan/Ext/Content.xml` rows are prepared without active Config blobs |
| Round 13 Form.xml slice | unit-level packer verification | new `PictureDecoration` child items can be compiled from XML |
| Round 13 Catalog metadata slice | unit-level metadata XML verification | `Catalog` metadata emits owned `Form` and `Template` child object headers |
| Round 13 MXL template slice | unit-level packer/formatter verification | SpreadsheetDocument `style:ButtonTextColor` is supported in pack and extract paths |
| Round 13 source staging readiness slice | unit-level staging verification | SpreadsheetDocument `Ext/Template.xml` rows are prepared without active Config blobs |
| Round 14 Form.xml slice | unit-level packer verification | new `InputField` items preserve explicit `ReadOnly` values |
| Round 14 BusinessProcess metadata slice | unit-level metadata XML verification | `BusinessProcess` emits generated type `InternalInfo` entries |
| Round 14 Task metadata slice | unit-level metadata XML verification | `Task` metadata emits owned `Form` and `Template` child object headers |
| Round 14 source staging readiness slice | unit-level staging verification | `Style/Ext/Style.xml` rows are prepared without active Config blobs |
| Round 15 Form.xml slice | unit-level packer verification | `InputField` `DataPath` is packed for existing and new layout entries |
| Round 15 partial metadata slice | unit-level metadata XML verification | `ExchangePlan` and related partial families emit owned `Form` and `Template` child object refs |
| Round 15 simple metadata slice | unit-level metadata XML verification | `DocumentJournal` emits owned `Form` and `Template` child object refs |
| Round 15 source staging readiness slice | unit-level staging verification | HTMLDocument template rows are covered as base-free help-style blobs |
| Round 16 Form.xml slice | unit-level packer verification | new `LabelField` items preserve explicit `ShowInHeader` values |
| Round 16 partial metadata slice | unit-level metadata XML verification | `AccountingRegister` and `CalculationRegister` emit generated type `InternalInfo` entries |
| Round 16 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits `Constant` child object headers |
| Round 16 source staging readiness slice | unit-level staging verification | raw-id `CommandInterface.xml` rows can be prepared without active Config blobs |
| Round 17 Form.xml slice | unit-level packer verification | new `InputField` items preserve explicit `SkipOnInput` values |
| Round 17 partial metadata slice | unit-level metadata XML verification | `Task` emits generated type `InternalInfo` entries |
| Round 17 MXL template slice | unit-level packer verification | SpreadsheetDocument `style:FieldSelectionBackColor` is supported in pack path |
| Round 17 source staging readiness slice | unit-level staging verification | root configuration raw-id `CommandInterface.xml` rows are covered as base-free |
| Round 18 Form.xml slice | unit-level packer verification | new top-level `Button` items preserve explicit `DefaultButton` values |
| Round 18 partial metadata slice | unit-level metadata XML verification | `ExchangePlan` emits `UseStandardCommands` from the metadata blob |
| Round 18 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits `Catalog` child object headers |
| Round 18 source staging readiness slice | unit-level staging verification | root configuration raw-id `MainSectionCommandInterface.xml` rows are covered as base-free |
| Round 19 Form.xml slice | unit-level packer verification | new `InputField` items preserve extended options (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`) |
| Round 19 partial metadata slice | unit-level metadata XML verification | `Subsystem` emits `UseStandardCommands` from the metadata blob |
| Round 19 source staging readiness slice | unit-level staging verification | `WSReference` definition rows are covered as base-free raw-deflated bodies |
| Round 20 Form.xml slice | unit-level packer verification | new `UsualGroup` items preserve extended `Behavior`, `Representation`, `ShowTitle=false`, and nested children |
| Round 20 partial metadata slice | unit-level metadata XML verification | register-family metadata emits `UseStandardCommands` from the metadata blob |
| Round 20 source staging readiness slice | unit-level staging verification | configuration `Ext/StandaloneConfigurationContent.bin` rows are covered as base-free raw-deflated bodies |
| Round 21 Form.xml slice | unit-level packer verification | nested `AutoCommandBar` items preserve `HorizontalAlign` and `Autofill` settings |
| Round 21 object metadata slice | unit-level metadata XML verification | `DataProcessor` emits `UseStandardCommands` from the metadata blob |
| Round 21 source staging readiness slice | unit-level staging verification | configuration `Ext/MobileClientSignature.bin` rows are covered as base-free raw-deflated bodies |
| Round 22 Form.xml slice | unit-level extractor/packer verification | form command `ModifiesSavedData=true` is extracted from slot 10 and packed back into existing/new command rows |
| Round 22 root/common attribute metadata slice | unit-level metadata XML verification | root `Configuration.xml` emits additional child families; `CommonAttribute` emits native-shaped `Content` and `AutoUse` enum values |
| Round 22 source staging readiness slice | unit-level readiness audit verification | form body staging remains base-dependent, but readiness reports precise blockers for layout, trailing sections, modules and item assets |
| Round 22 V8 container slice | unit-level module blob/container verification | shared `v8_container` parser/builder preserves module blob behavior and tests multi-element, round-trip and multi-page rejection behavior |
| Round 23 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract `AutoRefresh` / `AutoRefreshPeriod` from property bag keys `5`/`6` and pack them back into existing rows |
| Round 23 object metadata slice | unit-level metadata XML verification | `Document` metadata emits owned `Form` and `Template` child refs from generic form/template indexes |
| Round 23 partial metadata slice | unit-level metadata XML verification | `InformationRegister` reads `UseStandardCommands` from the extended native owner tuple while keeping simplified-shape fallback |
| Round 23 MXL template slice | unit-level extractor/packer verification | SpreadsheetDocument MOXCEL merge region kind `2` is preserved as `verticalUnmerge` and packs back into the same region list |
| Round 23 source staging readiness slice | unit-level readiness audit verification | `Role/Ext/Rights.xml` remains base-dependent, but readiness now reports exact blockers around role object order, right UUID slots, restrictions and template/trailing layout |
| Round 23 native dump performance slice | unit-level fetch/timing verification | streamed `mssql-dump-config` routes blob-bearing metadata/direct prepare reads through the existing native `bcp` parser and reports `sqlcmd`/`bcp` timing split |
| Round 24 Role Rights.xml slice | unit-level extractor/packer verification | role rights objects are sorted by object UUID for export, and packer maps XML objects back to base rights slots using the same UUID order |
| Round 24 Form.xml slice | unit-level extractor/packer verification | table child items extract and pack `SkipOnInput`, including wrapper `55` layouts |
| Round 24 object metadata slice | unit-level metadata XML verification | `Document` metadata emits `StandardAttributes`; the `Number` standard attribute fill value follows native number type |
| Round 24 CommonAttribute metadata slice | unit-level metadata XML verification | native `Content` pairs of metadata UUID and `{2,use_flag,...}` settings are parsed, including `use_flag=2` as `DontUse`, while simplified shape remains supported |
| Round 24 DCS template slice | unit-level template body verification | DataCompositionSchema template bodies rewrite known `v8:TypeId` values through the metadata type index to source-style current-config `v8:Type` references |
| Round 24 ExchangePlan Content.xml slice | unit-level source asset verification | `AutoRecord` values now map as `0=Deny`, `1=Allow`, `2=Auto` |
| Round 24 source staging readiness slice | unit-level readiness audit verification | `BusinessProcess/Ext/Flowchart.xml` remains base-dependent, but readiness now reports precise blockers for item table order, type-code slots, events, explanations and nested payload shape |
| Round 25 Role Rights.xml slice | unit-level extractor/packer verification | explicit disabled `false` rights without restrictions are preserved instead of being dropped |
| Round 25 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack `UseAlternationRowColor` through property bag key `9` |
| Round 25 object metadata slice | unit-level metadata XML verification | `Document` metadata emits numbering settings, standard command flag and default object/list/choice form refs |
| Round 25 InformationRegister metadata slice | unit-level metadata XML verification | `DefaultRecordForm` and `DefaultListForm` refs are resolved through owner metadata form indexes |
| Round 25 MXL template slice | unit-level template body verification | SpreadsheetDocument text-node quotes remain literal while XML structural characters stay escaped |
| Round 25 Configuration.xml slice | unit-level metadata XML verification | root scalar application/compatibility properties are emitted in native-shaped metadata XML |
| Round 25 source staging readiness slice | unit-level readiness audit verification | `Catalog/Ext/Predefined.xml` and `ChartOfCharacteristicTypes/Ext/Predefined.xml` remain base-dependent, but readiness reports precise blockers for row order, parent/type slots and trailing fields |
| Round 26 native dump performance slice | unit-level timing/batching verification | native `bcp` row batch cap is reduced to 256 MiB and timing JSON reports batch count/max rows/max binary bytes |
| Round 26 Role Rights.xml slice | unit-level extractor/packer verification | role object refs use UUID plus serialized tail fields, preserving repeated owner refs and standard-attribute refs |
| Round 26 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack observed `UpdateOnDataChange=Auto` through property bag key `14` |
| Round 26 object metadata slice | unit-level metadata XML verification | Catalog standard attribute labels and DataProcessor object/presentation scalars are parsed from metadata blobs |
| Round 26 register/exchange metadata slice | unit-level metadata XML verification | AccumulationRegister and ExchangePlan generated type `InternalInfo` coverage is expanded |
| Round 26 Configuration.xml slice | unit-level metadata XML verification | root `DefaultStyle` and `DefaultLanguage` refs are emitted from UUID-backed metadata fields |
| Round 26 DCS template slice | unit-level template body verification | observed DCS body `AnyIBRef`/`TypeSet` and settings `xsi:type` values are canonicalized |
| Round 26 source staging readiness slice | unit-level readiness audit verification | readable `CommandInterface.xml` refs now report precise base-free blockers while raw `kind:uuid` refs remain base-free |
| Round 27 Role Rights.xml slice | unit-level extractor verification | nested subsystem and integration service channel refs resolve to fuller native-style child object names |
| Round 27 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack existing `AllowGettingCurrentRowURL` boolean slots |
| Round 27 object metadata slice | unit-level metadata XML verification | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers are emitted |
| Round 27 workflow/register metadata slice | unit-level metadata XML verification | BusinessProcess/Task generated types and AccumulationRegister `IncludeHelpInContents=false` are emitted |
| Round 27 Configuration.xml slice | unit-level metadata XML verification | root `<DefaultRoles>` is emitted from resolved role refs in metadata field `39` |
| Round 27 DCS template slice | unit-level template body verification | TypeId normalization can use `xr:GeneratedType` entries from XML-shaped metadata text |
| Round 27 source staging readiness slice | unit-level readiness audit verification | configuration `Ext/HomePageWorkArea.xml` has explicit base-free readiness classification |
| Round 27 native dump performance slice | real selected-run timing plus unit-level summary verification | `mssql-dump-timing-summary` added; selected 663,776,134-byte blob run used 3 BCP batches with `fetch_rows_ms=3532` |
| Round 28 standalone-content source asset slice | unit-level source-only extraction verification plus real selected repro | `Ext/StandaloneConfigurationContent.bin` now builds targeted metadata refs when metadata XML extraction is disabled; the previous `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` blocker resolves as a role ref |
| Round 28 Role Rights.xml slice | unit-level extractor/packer verification | string-backed restriction fields export/import through `<restrictionByCondition><field>` while preserving base layout |
| Round 28 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `UserSettingsGroup` extracts and packs existing numeric property-bag slots |
| Round 28 object metadata slice | unit-level metadata XML verification | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior improved |
| Round 28 register/exchange slice | unit-level metadata/asset verification | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` BOM/final-newline framing are covered |
| Round 28 CommonAttribute slice | unit-level metadata XML verification | content item `xr:ConditionalSeparation` refs are resolved from native settings records |
| Round 28 DCS template slice | unit-level selected-index verification | selected template owners build `type_index` so DCS TypeIds can normalize in selected/source-only flows |
| Round 28 source staging readiness slice | unit-level readiness audit verification | metadata XML rows now report precise base-dependent blockers for incomplete native metadata compilation |
| Round 29 ExchangePlan content/performance slice | unit-level source-only extraction verification plus real selected repro | source asset extraction builds source-layout refs without metadata XML; ExchangePlan content failures now name the exact unsupported metadata id, currently a Constant ref not present in the selected/full source-layout index |
| Round 29 Role Rights.xml slice | unit-level extractor/packer verification | Role object rights preserve serialized object order instead of UUID/tail sorting |
| Round 29 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `DefaultItem=true` extracts and packs through property-bag key `11` |
| Round 29 object metadata slice | unit-level metadata XML verification | child `Attribute` property tails emit format/edit/fill/history details from native metadata blobs |
| Round 29 register metadata slice | unit-level metadata XML verification | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties are emitted |
| Round 29 CommonAttribute slice | unit-level metadata XML verification | native property detail defaults are emitted before `Content` for source XML 2.20/2.21 |
| Round 29 MXL template slice | unit-level template body verification | default-only spreadsheet `printSettings` blocks are suppressed to match native source output |
| Round 29 source staging readiness slice | unit-level readiness audit verification | `versions` rows now report precise base-free blockers for generation UUID maps, entry order, unchanged row sets, and platform-owned standard entries |
| Round 30 ExchangePlan content/performance slice | unit-level source-only extraction verification plus real selected/full repro | selected ExchangePlan content now requests `object_refs`; selected and full source-layout exports completed without `ConfigDumpInfo.xml` |
| Round 30 Role Rights.xml slice | unit-level packer verification | Role rights import packing maps objects by source-resolved refs instead of XML order, including commands, child dimensions/resources/attributes, standard attributes, and direct metadata refs |
| Round 30 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowPictureDataPath` extracts and packs through string property-bag key `19` |
| Round 30 object metadata slice | unit-level metadata XML verification | child `Attribute` tail properties now include choice/use/indexing/full-text/data-history scalar details, and Report/DataProcessor child commands emit command property tails |
| Round 30 register metadata slice | unit-level metadata XML verification | InformationRegister and AccumulationRegister standard attributes are emitted; AccumulationRegister type value `1` maps to `Turnovers` |
| Round 30 CommonAttribute slice | unit-level metadata XML verification | native separation tail properties and UUID-backed separation refs are emitted |
| Round 30 DCS template slice | unit-level template body verification | current-config `v8:Type` prefixes normalize by DCS context (`d4p1`, `d5p1`, `d6p1`) |
| Round 30 source staging readiness slice | unit-level readiness/row-generation verification | FilterCriterion manager modules can stage base-free from `FilterCriteria/<Name>/Ext/ManagerModule.bsl` |
| Round 31 native dump performance slice | real full source-layout timing plus unit-level extractor verification | `InputField` option bags are parsed once per form item; after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507 |
| Round 31 Role Rights.xml slice | unit-level extractor/packer verification | Task child refs map as `Task.<name>.AddressingAttribute.<child>` and source-based rights packing resolves `AddressingAttribute` children |
| Round 31 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `ChoiceFoldersAndItems` extracts and packs through the existing typed table slot |
| Round 31 object metadata slice | unit-level metadata XML verification | tabular-section property tails are emitted, and generic child property tails include empty `<LinkByType/>` before `ChoiceHistoryOnInput` |
| Round 31 register metadata slice | unit-level metadata XML verification | register dimensions/resources/attributes reuse generic child object parsing and emit decoded types plus common property tails |
| Round 31 Configuration.xml slice | unit-level metadata XML verification | root settings-storage refs (`CommonSettingsStorage`, report settings/variants storage, form data settings storage) are emitted from UUID-backed metadata fields |
| Round 31 DCS template slice | unit-level template body verification | generated type id lookup is case-insensitive for DataCompositionSchema template bodies |
| Round 31 source staging readiness slice | unit-level readiness/row-generation verification | WebService `Ext/Module.bsl` is covered as a base-free source staging body |
| Round 32 native dump performance slice | real full source-layout timing plus unit-level form verification | form child-item support indexes are built in one traversal; after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 68,886 to 40,598 and `source_asset_form_xml_cpu_ms` from 118,673 to 102,296 |
| Round 32 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RestoreCurrentRow` extracts and packs through property-bag key `12` |
| Round 32 object metadata slice | unit-level metadata XML verification | Document owner `<IncludeHelpInContents>` is emitted after default/auxiliary form properties |
| Round 32 ExchangePlan metadata slice | unit-level metadata XML verification | ExchangePlan code-4/code-27 child attributes are emitted with value types and property tails |
| Round 32 Configuration.xml slice | unit-level metadata XML verification | localized root information fields are emitted: `BriefInformation`, `DetailedInformation`, `Copyright`, `VendorInformationAddress`, and `ConfigurationInformationAddress` |
| Round 32 source staging readiness slice | unit-level readiness audit verification | unsupported `AdditionalIndexes.xml` families now report a precise `requires_base_blob` blocker instead of being silently omitted |
| Round 33 Role Rights.xml slice | unit-level extractor/packer verification | HTTPService URL template method refs resolve as `HTTPService.<service>.URLTemplate.<template>.Method.<method>` and pack back from source XML |
| Round 33 DCS template slice | unit-level template body verification | unqualified DCS core `xsi:type` values normalize to `dcscor:*`; data-core `StandardPeriod` / `StandardPeriodVariant` normalize to `v8:*` |
| Round 33 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `Period` extracts and packs through property-bag key `7` as `v8:StandardPeriodVariant=Custom` |
| Round 33 object metadata slice | unit-level metadata XML verification | DataProcessor child metadata emits non-empty `ChoiceParameters`, resolving design-time UUIDs to stable refs and fixed arrays |
| Round 33 ExchangePlan metadata slice | unit-level metadata XML verification | optional `xr:ThisNode` is emitted in `InternalInfo`, and `UseStandardCommands` is read relative to the detected header slot |
| Round 33 source staging readiness slice | unit-level row-generation verification | module `.bin` source assets containing inflated V8 containers can stage base-free for common/object/nested module bodies |
| Round 34 source staging readiness slice | unit-level row-generation verification | `Ext/ParentConfigurations.bin` source bytes are re-deflated into the staged `ConfigSave` body instead of being written directly |
| Round 34 Catalog metadata slice | unit-level metadata XML verification | Catalog `<Owners>` now emits resolved owner refs before `<SubordinationUse>` |
| Round 34 DCS template slice | unit-level template body verification | DCS `calculatedField` current-config type refs use the native `d4p1` namespace context |
| Round 34 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowFilter xsi:nil="true"` maps to property-bag key `10` / `{"U"}` |
| Round 34 ExchangePlan Content.xml slice | unit-level source asset verification | `ExchangePlan/Ext/Content.xml` ordering follows configuration metadata child order when that index is available |
| Round 34 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute property-tail `FillValue` supports string, nil, decimal and boolean encodings |
| Round 35 diff-mining slice | sampled XML-path diff mining | planning now targets repeated signatures instead of broad object families; full all-file pass still needs a faster reusable diagnostic |
| Round 35 Form.xml slice | unit-level extractor/packer verification | default `ShowCommandBar=true` is suppressed in export, and explicit XML still packs into the form layout |
| Round 35 Catalog metadata slice | unit-level metadata XML verification | Catalog child attributes reuse generic native-order property tails and resolve choice-parameter refs through `object_refs` |
| Round 35 Flowchart.xml slice | unit-level source asset verification | Flowchart item `ZOrder` is emitted from serialized item position rather than fixed zero |
| Round 35 MXL template slice | unit-level template body verification | unknown MOXCEL format bits no longer invalidate the entire format table |
| Round 35 source staging readiness slice | unit-level readiness/row-generation verification | configuration application modules have explicit base-free readiness and row-generation coverage |
| Round 35 Role Rights.xml slice | unit-level extractor/packer verification | role rights extraction and packing support form refs including owned forms |
| Round 36 diff-mining slice | reusable XML-path signature mining | `source-diff-signatures` turns a `source-diff` JSON into ranked repeated XML path signatures with per-kind sampling |
| Round 36 Form.xml slice | unit-level extractor/packer verification | `Page/ScrollOnCompress=false` is treated as a native omitted default; explicit `true` still round-trips |
| Round 36 Flowchart.xml slice | unit-level source asset verification | flowchart connection-line fonts decode from serialized style tuples, including standard style-item refs |
| Round 36 MXL template slice | unit-level extractor/packer verification | MXL `textOrientation` format bit 13 is preserved in export and import packing |
| Round 36 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute `FillChecking` is emitted from native property details in native XML order |
| Round 36 source staging readiness slice | unit-level readiness/row-generation verification | raw `CommonCommand/Ext/CommandInterface.xml` staging no longer needs an active base blob |

Performance note for selected extraction:

- `FunctionalOptionsParameters` selected export now writes only 9 rows / 9 metadata XML files / 0 source assets; output size is about 0.02 MB.
- The previous accidental broad run processed 40,576 rows and 12,647 source assets. The remaining selected-export cost is now `prepare_indexes_ms` for the broad metadata reference index, not row fetching or asset writing.
- `--file-name-list` is available for larger selected sets; long generated SQL is sent to `sqlcmd` through a temporary `.sql` file to avoid Windows command-line length limits.
- `DocumentNumerators` selected export writes 3 rows / 3 metadata XML files / 0 source assets; XML generation is about 3 ms. After skipping the broad metadata reference index for self-contained selected types, `prepare_indexes_ms` dropped from about 20 s to 77 ms on this set.
- `WSReferences` selected export writes 2 rows / 1 metadata XML file / 1 source asset. After extracting only the embedded WSDL XML and skipping broad indexes for `WSReference.0`, `prepare_indexes_ms` dropped from about 19.7 s to 81 ms on this set.
- `IntegrationServices` selected export writes 2 rows / 1 metadata XML file / 1 module text file. After parsing child channels and skipping broad indexes for `IntegrationService.0` module text, `prepare_indexes_ms` dropped from about 19 s to 89 ms on this set.
- Configuration-level `Ext` selected export maps `.8/.9/.a/.b` rows to source paths without requiring the `.0/.5/.6/.7` module group, so quick diagnostics can write these files from a `--file-name-list`.
- `Ext` `.8/.9/.a/.b` selected source-mode run is byte-identical to native and no longer performs a broad metadata fetch. Latest timing: `prepare_indexes_ms=1552`, split into `prepare_metadata_fetch_ms=211`, `prepare_metadata_texts_ms=768`, and `prepare_reference_indexes_ms=573`. Detailed reference-index timings were `prepare_command_refs_ms=392`, `prepare_form_refs_ms=163`, `prepare_metadata_refs_ms=14`; all other reference-index builders were skipped.
- Broader selected command-interface diagnostics can still need a targeted/cache reference index when command UUIDs only exist inside owner metadata blobs rather than as direct `FileName` rows.
- Issue #19 bottleneck note: `docs/issue-19-selected-export-bottleneck.md` pinpoints selected command-interface `command_refs` as the next safe optimization target.
- Round 23 #19 follow-up: streamed `mssql-dump-config` no longer uses `sqlcmd` for blob-bearing metadata/direct prepare reads; those paths now use the existing native `bcp` parser. `sqlcmd` still remains for lightweight row headers/control queries. The latest full-run evidence before this change had `fetch_rows_ms=11176` against `process_rows_wall_ms=85887` and `prepare_indexes_ms=33210`, so the primary remaining bottleneck is CPU/source generation and reference/index preparation, not SQL blob transfer.
- Round 26 #19 memory/timing follow-up: native `bcp` row fetch batches are capped at 256 MiB instead of 1 GiB to reduce peak temp-file plus parsed-row memory pressure. The dump report now includes `timings.fetch_row_batches`, `timings.fetch_row_batch_max_rows`, and `timings.fetch_row_batch_max_binary_bytes` to isolate whether the next full-run bottleneck is batch memory pressure or row-processing wall time.
- Round 27 #19 timing follow-up: added `mssql-dump-timing-summary` for saved dump JSON reports and ran a real selected blob fetch on `ut_ibcmd` under `E:\ibcmd_lab\perf`. The selected run fetched 663,776,134 bytes in 3 BCP batches, with `fetch_row_batch_max_binary_bytes=268302217` and `fetch_rows_ms=3532`.
- Round 28 #19 standalone-content follow-up: source-asset-only dumps now resolve `Ext/StandaloneConfigurationContent.bin` references without requiring `--extract-metadata-xml`. A real selected repro on `ut_ibcmd` wrote 6 rows / 40,421 bytes / 2 source assets under `E:\ibcmd_lab\perf\issue-19-standalone-content-v6-standalone-repro-targeted`, resolving `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` as `Role.РазделОтчетыИМониторингЦелевыеПоказатели`. A full source-layout timing retry passed standalone content and then stopped at `ExchangePlans\Полный\Ext\Content.xml`; no `ConfigDumpInfo.xml` was generated.
- Round 29 #19 ExchangePlan content follow-up: source-layout extraction now builds metadata object/type indexes when source assets are extracted without metadata XML, and `ExchangePlanContent` parsing reports precise unsupported ids. A selected repro for `ExchangePlans\Полный\Ext\Content.xml` now stops at `ExchangePlanContent item 0 references unsupported metadata id ff76e85a-6d29-41d3-a83e-f4a34139c6b2`; the id is a direct Constant row named `ВыгружатьВнутренниеШтрихкодыШтучныхТоваров`. Artifacts are under `E:\ibcmd_lab\perf\issue-19-exchange-content-v1-*`. No `ConfigDumpInfo.xml` was generated.
- Round 30 #19 ExchangePlan content follow-up: selected ExchangePlan owner metadata now requests `object_refs`, so the Constant referenced from `ExchangePlans\Полный\Ext\Content.xml` resolves. The selected repro passed, and a full release source-layout run completed under `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source`, writing 13,428 files from 40,576 rows / 10,277 source assets without `ConfigDumpInfo.xml`. Full timing: `prepare_indexes_ms=17461`, `prepare_object_refs_ms=10621`, `fetch_rows_ms=5043`, `process_rows_wall_ms=15598`, `source_asset_cpu_ms=191976`, `source_asset_form_cpu_ms=124212`.
- Round 31 #19 form CPU follow-up: `InputField` extended option bags are cached per form child item instead of reparsed for each option probe. The full source-layout after-run completed under `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-full-source-after` without `ConfigDumpInfo.xml`; `source_asset_form_child_items_cpu_ms` changed from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507.
- Round 32 #19 form CPU follow-up: form child-item support indexes for table names, table columns, child item names, and `UserSettingsGroup` resolution are now built in one traversal. The full source-layout after-run completed under `E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after` without `ConfigDumpInfo.xml`; `source_asset_form_child_items_cpu_ms` changed from 68,886 to 40,598, `source_asset_form_xml_cpu_ms` changed from 118,673 to 102,296, and `source_asset_form_cpu_ms` changed from 123,507 to 106,971.
- Scope decision: `ConfigDumpInfo.xml` is intentionally not generated. The native file is derived from the `versions` row, but it is not needed for our export/import target and should not be treated as remaining work.

Diff by file kind:

| kind | different |
|---|---:|
| form | 10690 |
| metadata_xml | 3902 |
| template | 1101 |
| other | 1170 |
| configuration_root | 1 |

## Top-Level Objects

| Object | State | Total | Unchanged | Different | Ready % |
|---|---|---:|---:|---:|---:|
| CommandGroups | done | 34 | 34 | 0 | 100.0 |
| CommonCommands | done | 612 | 612 | 0 | 100.0 |
| CommonModules | done | 5594 | 5594 | 0 | 100.0 |
| CommonPictures | done | 5505 | 5505 | 0 | 100.0 |
| Constants | done | 1235 | 1235 | 0 | 100.0 |
| DefinedTypes | done | 456 | 456 | 0 | 100.0 |
| DocumentNumerators | done | 3 | 3 | 0 | 100.0 |
| EventSubscriptions | done | 312 | 312 | 0 | 100.0 |
| Ext | done | 15 | 15 | 0 | 100.0 |
| FunctionalOptions | done | 567 | 567 | 0 | 100.0 |
| FunctionalOptionsParameters | done | 9 | 9 | 0 | 100.0 |
| IntegrationServices | done | 2 | 2 | 0 | 100.0 |
| Languages | done | 1 | 1 | 0 | 100.0 |
| ScheduledJobs | done | 402 | 402 | 0 | 100.0 |
| SessionParameters | done | 98 | 98 | 0 | 100.0 |
| StyleItems | done | 400 | 400 | 0 | 100.0 |
| WSReferences | done | 2 | 2 | 0 | 100.0 |
| XDTOPackages | done | 814 | 814 | 0 | 100.0 |
| Enums | partial | 1195 | 1136 | 59 | 95.1 |
| CommonTemplates | partial | 495 | 409 | 86 | 82.6 |
| Reports | partial | 2362 | 1597 | 765 | 67.6 |
| AccumulationRegisters | partial | 449 | 267 | 182 | 59.5 |
| Roles | partial | 2220 | 1207 | 1013 | 54.4 |
| ExchangePlans | partial | 366 | 190 | 176 | 51.9 |
| HTTPServices | partial | 10 | 5 | 5 | 50.0 |
| WebServices | partial | 36 | 18 | 18 | 50.0 |
| DocumentJournals | partial | 121 | 60 | 61 | 49.6 |
| DataProcessors | partial | 7058 | 3456 | 3602 | 49.0 |
| Tasks | partial | 49 | 23 | 26 | 46.9 |
| Catalogs | partial | 6705 | 3055 | 3650 | 45.6 |
| Documents | partial | 6219 | 2799 | 3420 | 45.0 |
| InformationRegisters | partial | 3978 | 1751 | 2227 | 44.0 |
| ChartsOfCharacteristicTypes | partial | 167 | 70 | 97 | 41.9 |
| BusinessProcesses | partial | 152 | 63 | 89 | 41.4 |
| SettingsStorages | partial | 82 | 32 | 50 | 39.0 |
| CommonForms | partial | 1116 | 411 | 705 | 36.8 |
| Subsystems | partial | 766 | 147 | 619 | 19.2 |
| FilterCriteria | partial | 7 | 1 | 6 | 14.3 |
| CommonAttributes | partial | 7 | 0 | 7 | 0.0 |
| Configuration.xml | partial | 1 | 0 | 1 | 0.0 |
| **Overall full snapshot** | **partial** | **49622** | **32758** | **16864** | **66.0** |

## Scope Exclusions

| Artifact | Decision | Reason |
|---|---|---|
| ConfigDumpInfo.xml | do not generate | Not required for the replacement export/import workflow; do not count it as parity debt. |

Note: `Configuration.xml` currently covers the root metadata header
(uuid/name/synonym/comment), source XML version selection, and selected root
child object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`),
plus selected root refs such as default roles/style/language/settings storages.
Localized root information fields are also covered.
Deeper root properties are still tracked as Issue #22 follow-up work.

## Delegation History

| Issue | Scope | Status |
|---|---|---|
| #15 | Catalogs/Documents/DataProcessors/Reports metadata XML | merged to `master` |
| #18 | register/subsystem/exchange-plan metadata and auxiliary assets | merged to `master` |
| #16 | Form.xml `TextDocumentField/ReadOnly` layout packing | merged to `master` in round 9 |
| #15 | DataProcessor owned template child refs | merged to `master` in round 9 |
| #19 | selected command-interface targeted owner refs | merged to `master` in round 9; issue stays open pending real lab timing |
| #21 | CommonPicture/configuration picture staging without base blob fetch | merged to `master` in round 9 |
| #16 | Form.xml `TextDocumentField/Width` layout packing | merged to `master` in round 10 |
| #17 | MXL `style:FieldTextColor` extraction | merged to `master` in round 10 |
| #22 | Configuration.xml nested CommonAttribute child headers | merged to `master` in round 10 |
| #21 | Help.xml staging without active Config query | merged to `master` in round 10 |
| #16 | Form.xml new `CheckBoxField` compilation | merged to `master` in round 11 |
| #13 | omitted false Role Rights.xml entries | merged to `master` in round 11 |
| #18 | InformationRegister generated type InternalInfo | merged to `master` in round 11 |
| #21 | module body staging without active module blobs | merged to `master` in round 11 |
| #18 | AccumulationRegister generated type InternalInfo | merged to `master` in round 12 |
| #16 | Form.xml `Pages` / `Page` layout properties | merged to `master` in round 12 |
| #22 | Configuration.xml nested CommonModule child headers | merged to `master` in round 12 |
| #21 | ExchangePlan Content.xml staging without active Config query | merged to `master` in round 12 |
| #16 | Form.xml new `PictureDecoration` compilation | merged to `master` in round 13 |
| #15 | Catalog owned Form/Template child refs | merged to `master` in round 13 |
| #17 | SpreadsheetDocument `style:ButtonTextColor` pack/extract | merged to `master` in round 13 |
| #21 | SpreadsheetDocument Template.xml staging without active Config query | merged to `master` in round 13 |
| #16 | Form.xml new `InputField/ReadOnly` generation | merged to `master` in round 14 |
| #18 | BusinessProcess generated type InternalInfo | merged to `master` in round 14 |
| #14 | Task owned Form/Template child refs | merged to `master` in round 14 |
| #21 | Style.xml staging without active Config query | merged to `master` in round 14 |
| #16 | Form.xml `InputField/DataPath` packing | merged to `master` in round 15 |
| #18 | partial metadata owned Form/Template child refs | merged to `master` in round 15 |
| #14 | DocumentJournal owned Form/Template child refs | merged to `master` in round 15 |
| #21 | HTMLDocument template staging coverage without base blob fetch | merged to `master` in round 15 |
| #16 | Form.xml new `LabelField/ShowInHeader` generation | merged to `master` in round 16 |
| #18 | AccountingRegister/CalculationRegister generated type InternalInfo | merged to `master` in round 16 |
| #22 | Configuration.xml root Constant child headers | merged to `master` in round 16 |
| #21 | raw-id CommandInterface.xml staging without active Config query | merged to `master` in round 16 |
| #16 | Form.xml new `InputField/SkipOnInput` generation | merged to `master` in round 17 |
| #18 | Task generated type InternalInfo | merged to `master` in round 17 |
| #17 | SpreadsheetDocument `style:FieldSelectionBackColor` pack | merged to `master` in round 17 |
| #21 | configuration CommandInterface.xml raw-id base-free coverage | merged to `master` in round 17 |
| #16 | Form.xml new top-level `Button/DefaultButton` generation | merged to `master` in round 18 |
| #18 | ExchangePlan `UseStandardCommands` metadata XML | merged to `master` in round 18 |
| #22 | Configuration.xml root Catalog child headers | merged to `master` in round 18 |
| #21 | configuration MainSectionCommandInterface.xml raw-id base-free coverage | merged to `master` in round 18 |
| #16 | Form.xml new `InputField` extended options generation | merged to `master` in round 19 |
| #18 | Subsystem `UseStandardCommands` metadata XML | merged to `master` in round 19 |
| #21 | WSReference definition raw-deflated base-free coverage | merged to `master` in round 19 |
| #16 | Form.xml new `UsualGroup` extended properties generation | merged to `master` in round 20 |
| #18 | register-family `UseStandardCommands` metadata XML | merged to `master` in round 20 |
| #21 | configuration StandaloneConfigurationContent raw-deflated base-free coverage | merged to `master` in round 20 |
| #16 | Form.xml nested `AutoCommandBar` settings | merged to `master` in round 21 |
| #15 | DataProcessor `UseStandardCommands` metadata XML | merged to `master` in round 21 |
| #21 | configuration MobileClientSignature raw-deflated base-free coverage | merged to `master` in round 21 |
| #21 | precise form body base-blob blocker audit | merged to `master` in round 22 |
| #22 | root child families and CommonAttribute content/AutoUse metadata XML | merged to `master` in round 22 |
| #16 | Form command `ModifiesSavedData` extract/pack support | merged to `master` in round 22 |
| #23 | shared V8 container parser/builder extraction | merged to `master` in round 22 |
| #15 | Document owned Form/Template child refs | merged to `master` in round 23 |
| #16 | Form.xml wrapper `55` table `AutoRefresh` / `AutoRefreshPeriod` extraction and packing | merged to `master` in round 23 |
| #17 | SpreadsheetDocument `verticalUnmerge` merge-region extraction and packing | merged to `master` in round 23 |
| #18 | InformationRegister extended-tuple `UseStandardCommands` metadata XML | merged to `master` in round 23 |
| #19 | `mssql-dump-config` blob-bearing prepare fetches routed through native `bcp` parser with timing split | merged to `master` in round 23 |
| #21 | precise Role `Rights.xml` base-free staging blocker audit | merged to `master` in round 23 |
| #13 | Role Rights.xml object UUID ordering for export and pack mapping | merged to `master` in round 24 |
| #15 | Document `StandardAttributes` metadata XML | merged to `master` in round 24 |
| #16 | Form.xml table `SkipOnInput` extraction and packing | merged to `master` in round 24 |
| #17 | DataCompositionSchema template `TypeId` to `Type` reference normalization | merged to `master` in round 24 |
| #18 | ExchangePlan `Content.xml` `AutoRecord` mapping | merged to `master` in round 24 |
| #21 | precise BusinessProcess `Flowchart.xml` base-free staging blocker audit | merged to `master` in round 24 |
| #22 | CommonAttribute native content pair parsing | merged to `master` in round 24 |
| #13 | explicit disabled Role Rights.xml entries | merged to `master` in round 25 |
| #15 | Document numbering settings, standard command flag and default form refs | merged to `master` in round 25 |
| #16 | Form.xml wrapper `55` table `UseAlternationRowColor` extraction and packing | merged to `master` in round 25 |
| #17 | SpreadsheetDocument text-node quote escaping | merged to `master` in round 25 |
| #18 | InformationRegister default record/list form refs | merged to `master` in round 25 |
| #21 | precise Predefined.xml base-free staging blocker audit | merged to `master` in round 25 |
| #22 | root Configuration.xml scalar application/compatibility properties | merged to `master` in round 25 |
| #13 | Role Rights.xml tail-aware object refs and standard attribute refs | merged to `master` in round 26 |
| #15 | Catalog standard attribute labels and DataProcessor owner presentation metadata | merged to `master` in round 26 |
| #16 | Form.xml wrapper `55` table `UpdateOnDataChange=Auto` extraction and packing | merged to `master` in round 26 |
| #17 | DCS template body canonicalization for observed TypeSet/xsi:type values | merged to `master` in round 26 |
| #18 | AccumulationRegister and ExchangePlan generated type metadata XML | merged to `master` in round 26 |
| #19 | native dump `bcp` batch memory cap and batch diagnostics | merged to `master` in round 26 |
| #21 | precise readable `CommandInterface.xml` base-free blocker audit | merged to `master` in round 26 |
| #22 | root Configuration.xml default style/language refs | merged to `master` in round 26 |
| #13 | Role Rights.xml nested subsystem/integration-service child refs | merged to `master` in round 27 |
| #15 | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers | merged to `master` in round 27 |
| #16 | Form.xml wrapper `55` table `AllowGettingCurrentRowURL` extraction and packing | merged to `master` in round 27 |
| #17 | DCS TypeId readiness from XML-shaped `xr:GeneratedType` metadata | merged to `master` in round 27 |
| #18 | BusinessProcess/Task generated types and AccumulationRegister include-help scalar | merged to `master` in round 27 |
| #19 | `mssql-dump-timing-summary` and selected blob timing evidence | merged to `master` in round 27 |
| #21 | explicit HomePageWorkArea base-free readiness classification | merged to `master` in round 27 |
| #22 | root Configuration.xml default role refs | merged to `master` in round 27 |
| #13 | Role Rights.xml string-backed restriction field export/import | merged to `master` in round 28 |
| #15 | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior | merged to `master` in round 28 |
| #16 | Form.xml wrapper `55` table `UserSettingsGroup` extraction and packing | merged to `master` in round 28 |
| #17 | selected template-owner `type_index` readiness for DCS TypeId normalization | merged to `master` in round 28 |
| #18 | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` file framing | merged to `master` in round 28 |
| #19 | standalone-content reference resolution without metadata XML and new ExchangePlan content blocker evidence | merged to `master` in round 28 |
| #21 | precise metadata XML row base-dependent readiness audit | merged to `master` in round 28 |
| #22 | CommonAttribute conditional-separation refs | merged to `master` in round 28 |
| #13 | Role Rights.xml serialized object order preservation | merged to `master` in round 29 |
| #15 | child `Attribute` property tails for object metadata XML | merged to `master` in round 29 |
| #16 | Form.xml wrapper `55` table `DefaultItem` extraction and packing | merged to `master` in round 29 |
| #17 | default-only spreadsheet `printSettings` suppression | merged to `master` in round 29 |
| #18 | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties | merged to `master` in round 29 |
| #19 | ExchangePlan `Content.xml` no-metadata-XML source refs and precise unsupported-id diagnostics | merged to `master` in round 29 |
| #21 | precise `versions` base-free staging blocker audit | merged to `master` in round 29 |
| #22 | CommonAttribute native property detail defaults | merged to `master` in round 29 |
| #13 | Role Rights.xml source-ref-based object mapping for import packing | merged to `master` in round 30 |
| #15 | child `Attribute` scalar tails and Report/DataProcessor child command properties | merged to `master` in round 30 |
| #16 | Form.xml wrapper `55` table `RowPictureDataPath` extraction and packing | merged to `master` in round 30 |
| #17 | DCS current-config type prefix normalization by context | merged to `master` in round 30 |
| #18 | register standard attributes and AccumulationRegister `Turnovers` enum mapping | merged to `master` in round 30 |
| #19 | ExchangePlan content direct object refs and successful full source-layout timing run | merged to `master` in round 30 |
| #21 | base-free FilterCriterion manager module staging | merged to `master` in round 30 |
| #22 | CommonAttribute separation tail properties and refs | merged to `master` in round 30 |
| #13 | Task `AddressingAttribute` Role Rights.xml refs and source packing | merged to `master` in round 31 |
| #15 | tabular-section property tails and child `<LinkByType/>` metadata XML | merged to `master` in round 31 |
| #16 | Form.xml wrapper `55` table `ChoiceFoldersAndItems` extraction and packing | merged to `master` in round 31 |
| #17 | case-insensitive DCS generated type id lookup | merged to `master` in round 31 |
| #18 | register child object decoded types and property tails | merged to `master` in round 31 |
| #19 | form source-asset CPU option-bag caching and after-run timing | merged to `master` in round 31 |
| #21 | WebService module base-free source staging coverage | merged to `master` in round 31 |
| #22 | Configuration.xml root settings-storage refs | merged to `master` in round 31 |
| #23 | shared V8 container parser/builder extraction | closed in round 22; kept as the required dependency for future Config/ConfigSave container work |
| #15 | Document owner `IncludeHelpInContents` metadata XML | merged to `master` in round 32 |
| #16 | Form.xml wrapper `55` table `RestoreCurrentRow` extraction and packing | merged to `master` in round 32 |
| #18 | ExchangePlan child attributes with value types and property tails | merged to `master` in round 32 |
| #19 | form child-item support-index performance optimization and after-run timing | merged to `master` in round 32 |
| #21 | precise unmapped `AdditionalIndexes.xml` base-free staging blocker audit | merged to `master` in round 32 |
| #22 | Configuration.xml localized root information fields | merged to `master` in round 32 |
| #13 | HTTPService URL template method Role Rights.xml refs and source packing | merged to `master` in round 33 |
| #15 | child `ChoiceParameters` metadata XML with design-time refs | merged to `master` in round 33 |
| #16 | Form.xml wrapper `55` table `Period` extraction and packing | merged to `master` in round 33 |
| #17 | DCS core/data-core `xsi:type` normalization | merged to `master` in round 33 |
| #18 | ExchangePlan `xr:ThisNode` and header-relative `UseStandardCommands` | merged to `master` in round 33 |
| #21 | base-free module `.bin` V8 container body staging | merged to `master` in round 33 |
| #15 | Catalog `<Owners>` metadata XML refs | merged to `master` in round 34 |
| #16 | Form.xml wrapper `55` table `RowFilter xsi:nil` extraction and packing | merged to `master` in round 34 |
| #17 | DCS calculated-field current-config namespace normalization | merged to `master` in round 34 |
| #18 | ExchangePlan `Content.xml` metadata-tree ordering | merged to `master` in round 34 |
| #21 | base-free `Ext/ParentConfigurations.bin` raw-deflated staging | merged to `master` in round 34 |
| #22 | CommonAttribute property-tail `FillValue` metadata XML | merged to `master` in round 34 |
| #13 | Role Rights.xml common/owned form refs and source packing | merged to `master` in round 35 |
| #15 | Catalog child attribute property tails and choice-parameter refs | merged to `master` in round 35 |
| #16 | Form.xml `ShowCommandBar=true` default suppression | merged to `master` in round 35 |
| #17 | MOXCEL unknown format-bit tolerance for MXL templates | merged to `master` in round 35 |
| #18 | BusinessProcess Flowchart `ZOrder` export | merged to `master` in round 35 |
| #19 | sampled XML-path diff-mining report for round35 planning | merged to `master` in round 35 |
| #21 | configuration application module base-free staging audit | merged to `master` in round 35 |

Worker result on #18: one selected subsystem `Ext/CommandInterface.xml` is byte-identical now, but the `Subsystems` group is still partial.
