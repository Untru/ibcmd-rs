# Export Parity Status

Generated from:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --database ut_ibcmd `
  -o E:\ibcmd_lab\full_diff_20260630_184120\ibcmd_rs_dump `
  --overwrite `
  --inflate `
  --extract-module-text `
  --extract-metadata-xml `
  --source-version 2.20 `
  --no-binary-rows

robocopy `
  E:\ibcmd_lab\full_diff_20260630_184120\ibcmd_rs_dump `
  E:\ibcmd_lab\full_diff_20260630_184120\ibcmd_rs_source_only `
  /E /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw `
  /XF manifest.json *.json

target\release\ibcmd-rs.exe source-diff `
  -o E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json `
  D:\ibcmd-rs\lab\ut_ibcmd_20260629_164647\ibcmd `
  E:\ibcmd_lab\full_diff_20260630_184120\ibcmd_rs_source_only
```

The full JSON report is retained at
`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`.

Reference: native `ibcmd` export from `ut_ibcmd`.
Candidate: `ibcmd-rs` full export snapshot generated before the round 22 through
round 25 incremental verified fixes listed below.

Raw source-only summary:

| left_only | right_only | different | unchanged |
|---:|---:|---:|---:|
| 1 | 0 | 16984 | 32638 |

The only `left_only` file is `ConfigDumpInfo.xml`; it remains a deliberate
scope exclusion and is not counted as parity debt.

Overall full-snapshot readiness excluding `ConfigDumpInfo.xml`: **65.8%**
(`32638 / 49622` byte-identical files), with **16984** files still different.

Verification history:

| Object | Verification | Result |
|---|---|---|
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
- Scope decision: `ConfigDumpInfo.xml` is intentionally not generated. The native file is derived from the `versions` row, but it is not needed for our export/import target and should not be treated as remaining work.

Diff by file kind:

| kind | different |
|---|---:|
| form | 10690 |
| metadata_xml | 3856 |
| template | 1267 |
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
| Enums | partial | 1195 | 1122 | 73 | 93.9 |
| CommonTemplates | partial | 495 | 406 | 89 | 82.0 |
| Roles | partial | 2220 | 1752 | 468 | 78.9 |
| AccumulationRegisters | partial | 449 | 267 | 182 | 59.5 |
| Reports | partial | 2362 | 1299 | 1063 | 55.0 |
| HTTPServices | partial | 10 | 5 | 5 | 50.0 |
| WebServices | partial | 36 | 18 | 18 | 50.0 |
| ExchangePlans | partial | 366 | 182 | 184 | 49.7 |
| DocumentJournals | partial | 121 | 60 | 61 | 49.6 |
| Tasks | partial | 49 | 23 | 26 | 46.9 |
| DataProcessors | partial | 7058 | 3214 | 3844 | 45.5 |
| Catalogs | partial | 6705 | 3033 | 3672 | 45.2 |
| InformationRegisters | partial | 3978 | 1751 | 2227 | 44.0 |
| Documents | partial | 6219 | 2721 | 3498 | 43.8 |
| ChartsOfCharacteristicTypes | partial | 167 | 70 | 97 | 41.9 |
| BusinessProcesses | partial | 152 | 63 | 89 | 41.4 |
| SettingsStorages | partial | 82 | 32 | 50 | 39.0 |
| CommonForms | partial | 1116 | 411 | 705 | 36.8 |
| Subsystems | partial | 766 | 147 | 619 | 19.2 |
| FilterCriteria | partial | 7 | 1 | 6 | 14.3 |
| CommonAttributes | partial | 7 | 0 | 7 | 0.0 |
| Configuration.xml | partial | 1 | 0 | 1 | 0.0 |
| **Overall full snapshot** | **partial** | **49622** | **32638** | **16984** | **65.8** |

## Scope Exclusions

| Artifact | Decision | Reason |
|---|---|---|
| ConfigDumpInfo.xml | do not generate | Not required for the replacement export/import workflow; do not count it as parity debt. |

Note: `Configuration.xml` currently covers the root metadata header
(uuid/name/synonym/comment), source XML version selection, and selected root
child object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`).
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

Worker result on #18: one selected subsystem `Ext/CommandInterface.xml` is byte-identical now, but the `Subsystems` group is still partial.
