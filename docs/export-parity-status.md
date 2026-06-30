# Export Parity Status

Generated from:

```powershell
target\release\ibcmd-rs.exe source-diff `
  -o lab\ut_ibcmd_20260629_164647\diff_full_after_constants5_065800.json `
  lab\ut_ibcmd_20260629_164647\ibcmd `
  lab\ut_ibcmd_20260629_164647\infobase_export_constants4_063257
```

The full JSON report was temporary and may be regenerated with the command above. It was removed after summarizing because the lab disk is nearly full.

Reference: native `ibcmd` export from `ut_ibcmd`.
Candidate: latest `ibcmd-rs` export after closing `Constants`.

Overall summary:

| left_only | right_only | different | unchanged |
|---:|---:|---:|---:|
| 0 | 0 | 17623 | 32000 |

Incremental selected verification after this snapshot:

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
- Scope decision: `ConfigDumpInfo.xml` is intentionally not generated. The native file is derived from the `versions` row, but it is not needed for our export/import target and should not be treated as remaining work.

Diff by file kind:

| kind | different |
|---|---:|
| form | 10690 |
| metadata_xml | 4494 |
| template | 1267 |
| other | 1170 |
| configuration_root | 1 |
| other_xml | 1 |

## Top-Level Objects

| Object | State | Total | Unchanged | Different | Ready % |
|---|---|---:|---:|---:|---:|
| CommonCommands | done | 612 | 612 | 0 | 100.0 |
| CommonModules | done | 5594 | 5594 | 0 | 100.0 |
| CommonPictures | done | 5505 | 5505 | 0 | 100.0 |
| Constants | done | 1235 | 1235 | 0 | 100.0 |
| DefinedTypes | done | 456 | 456 | 0 | 100.0 |
| EventSubscriptions | done | 312 | 312 | 0 | 100.0 |
| FunctionalOptions | done | 567 | 567 | 0 | 100.0 |
| ScheduledJobs | done | 402 | 402 | 0 | 100.0 |
| StyleItems | done | 400 | 400 | 0 | 100.0 |
| CommandGroups | done | 34 | 34 | 0 | 100.0 |
| FunctionalOptionsParameters | done | 9 | 9 | 0 | 100.0 |
| Languages | done | 1 | 1 | 0 | 100.0 |
| SessionParameters | done | 98 | 98 | 0 | 100.0 |
| DocumentNumerators | done | 3 | 3 | 0 | 100.0 |
| WSReferences | done | 2 | 2 | 0 | 100.0 |
| IntegrationServices | done | 2 | 2 | 0 | 100.0 |
| DataProcessors | partial | 7058 | 3214 | 3844 | 45.5 |
| Catalogs | partial | 6705 | 3033 | 3672 | 45.2 |
| Documents | partial | 6219 | 2721 | 3498 | 43.8 |
| InformationRegisters | partial | 3978 | 1751 | 2227 | 44.0 |
| Reports | partial | 2362 | 1299 | 1063 | 55.0 |
| CommonForms | partial | 1116 | 411 | 705 | 36.8 |
| Subsystems | partial | 766 | 76 | 690 | 9.9 |
| Roles | partial | 2220 | 1744 | 476 | 78.6 |
| XDTOPackages | partial | 814 | 407 | 407 | 50.0 |
| ExchangePlans | partial | 366 | 182 | 184 | 49.7 |
| AccumulationRegisters | partial | 449 | 267 | 182 | 59.5 |
| ChartsOfCharacteristicTypes | partial | 167 | 70 | 97 | 41.9 |
| BusinessProcesses | partial | 152 | 63 | 89 | 41.4 |
| CommonTemplates | partial | 495 | 406 | 89 | 82.0 |
| Enums | partial | 1195 | 1122 | 73 | 93.9 |
| DocumentJournals | partial | 121 | 60 | 61 | 49.6 |
| SettingsStorages | partial | 82 | 32 | 50 | 39.0 |
| Tasks | partial | 49 | 23 | 26 | 46.9 |
| WebServices | partial | 36 | 18 | 18 | 50.0 |
| FilterCriteria | partial | 7 | 1 | 6 | 14.3 |
| HTTPServices | partial | 10 | 5 | 5 | 50.0 |
| Ext | done | 15 | 15 | 0 | 100.0 |
| CommonAttributes | partial | 7 | 1 | 6 | 14.3 |
| ConfigDumpInfo.xml | excluded | 1 | 0 | 0 | n/a |
| Configuration.xml | partial | 1 | 1 | 0 | 100.0 |

## Scope Exclusions

| Artifact | Decision | Reason |
|---|---|---|
| ConfigDumpInfo.xml | do not generate | Not required for the replacement export/import workflow; do not count it as parity debt. |

Note: `Configuration.xml` currently covers the root metadata header
(uuid/name/synonym/comment) and source XML version selection. Deeper root
properties are still tracked as Issue #22 follow-up work.

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

Worker result on #18: one selected subsystem `Ext/CommandInterface.xml` is byte-identical now, but the `Subsystems` group is still partial.
