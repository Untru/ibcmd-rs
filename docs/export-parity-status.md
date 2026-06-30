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
| CommonAttributes | not_started | 7 | 0 | 7 | 0.0 |
| ConfigDumpInfo.xml | excluded | 1 | 0 | 0 | n/a |
| Configuration.xml | not_started | 1 | 0 | 1 | 0.0 |

## Scope Exclusions

| Artifact | Decision | Reason |
|---|---|---|
| ConfigDumpInfo.xml | do not generate | Not required for the replacement export/import workflow; do not count it as parity debt. |

## Active Delegation

| Issue | Scope | Status |
|---|---|---|
| #15 | Catalogs/Documents/DataProcessors/Reports metadata XML | in-progress |
| #18 | register/subsystem/exchange-plan metadata and auxiliary assets | in-progress |

Worker result on #18: one selected subsystem `Ext/CommandInterface.xml` is byte-identical now, but the `Subsystems` group is still partial.
