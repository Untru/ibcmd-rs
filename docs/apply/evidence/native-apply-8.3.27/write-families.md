| table | op | row name | E01 | modules-excl | modules-dyn | modules-excl-2 | modules-dyn-2 | no-change | attribute | new-catalog | new-infobase |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Params | DELETE | `<guid>.ui` | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows |  |
| Params | UPDATE | `<guid>.ui` | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB |  |
| ConfigSave | DELETE | `LIKE %.new` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `LIKE %.new` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `<guid>.1c.new <- ConfigSave` | 14 st / 0 rows |  |  |  |  |  |  | 14 st / 0 rows | 14 st / 0 rows |
| Config | INSERT | `<guid>.1c.new <- ConfigSave` | 14 st / 14 rows |  |  |  |  |  |  | 14 st / 14 rows | 14 st / 14 rows |
| Config | DELETE | `deleted.new <- ConfigSave` |  |  |  |  |  |  |  | 1 st / 0 rows |  |
| Config | INSERT | `deleted.new <- ConfigSave` |  |  |  |  |  |  |  | 1 st / 1 rows |  |
| Config | DELETE | `<guid>.a.new <- ConfigSave` | 1 st / 0 rows |  |  |  |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `<guid>.a.new <- ConfigSave` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `<guid>.b.new <- ConfigSave` | 1 st / 0 rows |  |  |  |  |  |  |  | 1 st / 0 rows |
| Config | INSERT | `<guid>.b.new <- ConfigSave` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 1 rows |
| Config | DELETE | `<guid>.c.new <- ConfigSave` | 1 st / 0 rows |  |  |  |  |  |  |  | 1 st / 0 rows |
| Config | INSERT | `<guid>.c.new <- ConfigSave` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 1 rows |
| Config | DELETE | `<guid>.f.new <- ConfigSave` | 1 st / 0 rows |  |  |  |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `<guid>.f.new <- ConfigSave` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `root.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `root.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `version.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `version.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `versions.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `versions.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | INSERT | `MobileVersions.datNEW` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | DELETE | `MobileVersions.datNEW` | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 1 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 1 st / 0 rows |
| Files | UPDATE | `MobileVersions.datNEW` | 3 st / 3 rows / 108.4 KB | 3 st / 3 rows / 108.4 KB | 3 st / 3 rows / 108.4 KB | 3 st / 3 rows / 108.4 KB | 3 st / 3 rows / 108.4 KB | 1 st / 1 rows / 36.1 KB | 3 st / 3 rows / 108.4 KB | 3 st / 3 rows / 108.4 KB | 1 st / 1 rows / 43 B |
| Params | INSERT | `DBNames.New` | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows |  | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows |
| Params | DELETE | `DBNames.New` | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows |  | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows |
| Params | UPDATE | `DBNames.New` | 2 st / 2 rows / 280.5 KB | 2 st / 2 rows / 280.5 KB | 2 st / 2 rows / 280.5 KB | 2 st / 2 rows / 280.5 KB | 2 st / 2 rows / 280.5 KB |  | 2 st / 2 rows / 280.5 KB | 2 st / 2 rows / 280.6 KB | 2 st / 2 rows / 134.5 KB |
| Params | INSERT | `DBNames-Ext-1.New` | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows |  | 2 st / 2 rows | 2 st / 2 rows |  |
| Params | DELETE | `DBNames-Ext-1.New` | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows | 4 st / 0 rows |  | 4 st / 0 rows | 4 st / 0 rows |  |
| Params | UPDATE | `DBNames-Ext-1.New` | 2 st / 2 rows / 40 B | 2 st / 2 rows / 40 B | 2 st / 2 rows / 40 B | 2 st / 2 rows / 40 B | 2 st / 2 rows / 40 B |  | 2 st / 2 rows / 40 B | 2 st / 2 rows / 40 B |  |
| Params | INSERT | `DBNames-Ext-<guid>.New` | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows |  | 4 st / 4 rows | 4 st / 4 rows |  |
| Params | DELETE | `DBNames-Ext-<guid>.New` | 8 st / 0 rows | 8 st / 0 rows | 8 st / 0 rows | 8 st / 0 rows | 8 st / 0 rows |  | 8 st / 0 rows | 8 st / 0 rows |  |
| Params | UPDATE | `DBNames-Ext-<guid>.New` | 4 st / 4 rows / 36.7 KB | 4 st / 4 rows / 36.7 KB | 4 st / 4 rows / 36.7 KB | 4 st / 4 rows / 36.7 KB | 4 st / 4 rows / 36.7 KB |  | 4 st / 4 rows / 36.7 KB | 4 st / 4 rows / 36.7 KB |  |
| _ExtensionsRestructNGS | DELETE |  | 5 st / 1 rows | 5 st / 1 rows | 5 st / 1 rows | 5 st / 4 rows | 5 st / 4 rows | 1 st / 0 rows | 5 st / 1 rows | 5 st / 4 rows | 1 st / 0 rows |
| SchemaStorage | UPDATE |  | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows |  | 7 st / 7 rows | 6 st / 6 rows | 121 st / 121 rows |
| _DataHistoryMetadataNG | UPDATE |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _SystemSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _CommonSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _RepSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _RepVarSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _FrmDtSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _DynListSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _ErrorProcessingSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _URLExternalDataNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _InternalSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _DefaultSystemSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _DefaultInternalSettingsNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _ExtensionsRestructNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _ExtensionsRestructNGSNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _ExtensionsInfoNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _ExtensionsInfoNGSNG | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 0 rows |
| _DbCopiesUpdatesNG | INSERT |  |  |  |  |  |  |  | 1 st / 0 rows |  |  |
| _DbCopiesNG | INSERT |  |  |  |  |  |  |  | 1 st / 0 rows |  |  |
| _ConfigChngRNG | INSERT BULK |  | 3 st / 20685 rows | 3 st / 20685 rows | 3 st / 20685 rows | 3 st / 20685 rows | 3 st / 20685 rows |  | 3 st / 20685 rows | 3 st / 20688 rows |  |
| _ConfigChngR_ExtPropsNG | INSERT BULK |  | 4 st / 21357 rows | 4 st / 21357 rows | 4 st / 21357 rows | 4 st / 21357 rows | 4 st / 21357 rows |  | 4 st / 21357 rows | 4 st / 21357 rows |  |
| _ExtensionsRestruct | DELETE |  | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows |  | 2 st / 2 rows | 2 st / 2 rows |  |
| _ExtensionsRestruct | INSERT |  | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows | 2 st / 2 rows |  | 2 st / 2 rows | 2 st / 2 rows |  |
| _ExtensionsRestructNGS | INSERT |  | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows | 4 st / 4 rows |  | 4 st / 4 rows | 4 st / 4 rows |  |
| Files | DELETE | `extd_props_cached/gc.mrk` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  | 2 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows |
| Files | INSERT | `extd_props_cached/gc.mrk` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | UPDATE | `extd_props_cached/gc.mrk` | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B |  | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B | 1 st / 1 rows / 9 B |
| Files | INSERT | `CAS_GC_Info` |  |  |  |  |  |  |  |  | 1 st / 1 rows |
| ConfigCAS | DELETE | `<sha1>` | 12160 st / 0 rows | 12160 st / 0 rows | 12160 st / 0 rows |  |  |  | 12160 st / 0 rows |  |  |
| ConfigCAS | DELETE | `dbStruFinal<guid>` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |  |  | 1 st / 0 rows |  |  |
| Files | DELETE | `userDocs_ru_<sha1>.bin` | 103 st / 0 rows | 103 st / 0 rows | 103 st / 0 rows |  |  |  | 103 st / 0 rows |  |  |
| Files | DELETE | `userPostings_ru_<sha1>.bin` | 103 st / 0 rows | 103 st / 0 rows | 103 st / 0 rows |  |  |  | 103 st / 0 rows |  |  |
| Files | DELETE | `userVocabulary_ru_<sha1>.bin` | 103 st / 0 rows | 103 st / 0 rows | 103 st / 0 rows |  |  |  | 103 st / 0 rows |  |  |
| Files | DELETE | `CAS_GC_Info` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |  |  | 1 st / 1 rows |  | 1 st / 0 rows |
| Files | UPDATE | `CAS_GC_Info` | 1 st / 1 rows / 21 B | 1 st / 1 rows / 21 B | 1 st / 1 rows / 21 B |  |  |  | 1 st / 1 rows / 21 B |  | 1 st / 1 rows / 21 B |
| Params | INSERT | `<guid>.sinew` | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows |
| Params | DELETE | `<guid>.sinew` | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows |
| Params | UPDATE | `<guid>.sinew` | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB |
| Config | INSERT | `commit` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `commit` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |
| Config | UPDATE | `commit` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | INSERT | `dynamicCommit` |  |  | 1 st / 1 rows |  | 1 st / 1 rows |  |  |  |  |
| DBSchema | UPDATE |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Params | DELETE | `DBNames <- Params` |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Params | UPDATE | `DBNames.New -> DBNames` |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Params | DELETE | `DBNamesVersion-DBNames` |  |  |  |  |  |  | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Params | UPDATE | `DBNamesVersion-DBNames` |  |  |  |  |  |  | 1 st / 1 rows / 43 B | 1 st / 1 rows / 43 B | 1 st / 1 rows / 43 B |
| Params | INSERT | `siVersions` |  |  |  |  |  |  |  |  | 1 st / 1 rows |
| Params | DELETE | `<guid>_dynupdate_<guid>.si <- Params` |  |  | 16 st / 0 rows |  | 16 st / 0 rows |  |  |  |  |
| Params | UPDATE | `<guid>.sinew -> <guid>_dynupdate_<guid>.si` |  |  | 16 st / 16 rows |  | 16 st / 16 rows |  |  |  |  |
| Params | DELETE | `<guid>.si <- Params` | 16 st / 16 rows | 16 st / 16 rows |  | 16 st / 16 rows |  | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 0 rows |
| Params | UPDATE | `<guid>.sinew -> <guid>.si` | 16 st / 16 rows | 16 st / 16 rows |  | 16 st / 16 rows |  | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows |
| Params | DELETE | `siVersions` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 2 st / 0 rows |
| Params | UPDATE | `siVersions` | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 2 st / 2 rows / 1.3 KB |
| Params | DELETE | `DynamicallyUpdated` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `deleted.new` |  |  |  |  |  |  |  | 1 st / 0 rows |  |
| Params | UPDATE | `DynamicallyUpdated` |  |  | 1 st / 1 rows / 119 B |  | 1 st / 1 rows / 156 B |  |  |  |  |
| Config | INSERT | `dbStruFinal` |  |  | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows |  |  |  |
| Config | DELETE | `<guid> <- Config` | 4705 st / 4711 rows | 16 st / 16 rows |  | 14 st / 14 rows |  | 4 st / 4 rows | 2 st / 2 rows | 4930 st / 4929 rows | 4929 st / 6 rows |
| Config | UPDATE | `<guid>_dynupdate_<guid> -> <guid>` | 2 st / 2 rows | 2 st / 2 rows |  |  |  | 2 st / 2 rows |  |  |  |
| Config | DELETE | `<guid>.<n> <- Config` | 4795 st / 4800 rows | 23 st / 23 rows |  | 21 st / 21 rows |  | 6 st / 6 rows | 4 st / 4 rows | 4665 st / 4670 rows | 4888 st / 2 rows |
| Config | UPDATE | `<guid>_dynupdate_<guid>.<n> -> <guid>.<n>` | 2 st / 2 rows | 2 st / 2 rows |  |  |  | 2 st / 2 rows |  |  |  |
| Config | DELETE | `versions_dynupdate_<guid>` | 1 st / 0 rows | 1 st / 0 rows |  |  |  | 1 st / 0 rows |  |  |  |
| Config | DELETE | `deleted_dynupdate_<guid>` | 1 st / 0 rows | 1 st / 0 rows |  |  |  | 1 st / 0 rows |  |  |  |
| Config | UPDATE | `<guid>.<n>.new -> <guid>.<n>` | 4793 st / 4799 rows | 21 st / 21 rows |  | 21 st / 21 rows |  | 4 st / 4 rows | 4 st / 4 rows | 4665 st / 4678 rows | 4888 st / 4888 rows |
| Config | UPDATE | `<guid>.new -> <guid>` | 4703 st / 4707 rows | 14 st / 14 rows |  | 14 st / 14 rows |  | 2 st / 2 rows | 2 st / 2 rows | 4930 st / 4934 rows | 4929 st / 4929 rows |
| Config | DELETE | `<guid>.1c <- Config` | 14 st / 14 rows |  |  |  |  |  |  | 14 st / 14 rows | 14 st / 0 rows |
| Config | UPDATE | `<guid>.1c.new -> <guid>.1c` | 14 st / 14 rows |  |  |  |  |  |  | 14 st / 14 rows | 14 st / 14 rows |
| Config | DELETE | `<guid>.a <- Config` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 0 rows |
| Config | UPDATE | `<guid>.a.new -> <guid>.a` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `<guid>.b <- Config` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 0 rows |
| Config | UPDATE | `<guid>.b.new -> <guid>.b` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 1 rows |
| Config | DELETE | `<guid>.c <- Config` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 0 rows |
| Config | UPDATE | `<guid>.c.new -> <guid>.c` | 1 st / 1 rows |  |  |  |  |  |  |  | 1 st / 1 rows |
| Config | DELETE | `<guid>.f <- Config` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 0 rows |
| Config | UPDATE | `<guid>.f.new -> <guid>.f` | 1 st / 1 rows |  |  |  |  |  |  | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `root <- Config` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Config | UPDATE | `root.new -> root` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `version <- Config` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Config | UPDATE | `version.new -> version` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `versions_dynupdate_<guid> <- Config` |  |  | 1 st / 0 rows |  | 1 st / 0 rows |  |  |  |  |
| Config | UPDATE | `versions.new -> versions_dynupdate_<guid>` |  |  | 1 st / 1 rows |  | 1 st / 1 rows |  |  |  |  |
| Config | DELETE | `versions <- Config` | 1 st / 1 rows | 1 st / 1 rows |  | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Config | UPDATE | `versions.new -> versions` | 1 st / 1 rows | 1 st / 1 rows |  | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `DynamicallyUpdated` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | UPDATE | `DynamicallyUpdated` |  |  | 1 st / 1 rows / 82 B |  | 1 st / 1 rows / 119 B |  |  |  |  |
| ConfigSave | DELETE | `LIKE %` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Files | DELETE | `MobileVersions.dat <- Files` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Files | UPDATE | `MobileVersions.datNEW -> MobileVersions.dat` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `dynamicCommit` | 1 st / 0 rows | 1 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | UPDATE | `dynamicCommit` |  |  | 1 st / 1 rows / 1.4 KB |  | 1 st / 1 rows / 1.4 KB |  |  |  |  |
| Config | DELETE | `dbStruFinal` | 1 st / 0 rows | 1 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | UPDATE | `dbStruFinal` |  |  | 1 st / 1 rows |  | 1 st / 1 rows | 1 st / 1 rows |  |  |  |
| _ConfigChngR | UPDATE |  |  |  | 1 st / 49 rows |  | 1 st / 49 rows | 1 st / 7 rows |  |  |  |
| Config | DELETE | `<guid>_dynupdate_<guid>.<n> <- Config` |  |  | 21 st / 0 rows |  | 21 st / 0 rows |  |  |  |  |
| Config | UPDATE | `<guid>.<n>.new -> <guid>_dynupdate_<guid>.<n>` |  |  | 21 st / 21 rows |  | 21 st / 21 rows |  |  |  |  |
| Config | DELETE | `<guid>_dynupdate_<guid> <- Config` |  |  | 14 st / 0 rows |  | 14 st / 0 rows |  |  |  |  |
| Config | UPDATE | `<guid>.new -> <guid>_dynupdate_<guid>` |  |  | 14 st / 14 rows |  | 14 st / 14 rows |  |  |  |  |
| Files | DELETE | `userDocs_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userDocs_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userDocs_ru.new` | 4 st / 4 rows / 2.2 KB | 4 st / 4 rows / 46.2 KB | 4 st / 4 rows / 46.2 KB | 4 st / 4 rows / 46.2 KB | 4 st / 4 rows / 46.2 KB | 4 st / 4 rows / 46.4 KB | 4 st / 4 rows / 46.4 KB | 4 st / 4 rows / 2 B |  |
| Files | DELETE | `userVocabulary_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userVocabulary_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userVocabulary_ru.new` | 4 st / 4 rows / 8.6 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 2 B |  |
| Files | DELETE | `userPostings_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userPostings_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userPostings_ru.new` | 4 st / 4 rows / 4.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 2 B |  |
| Files | DELETE | `userDocs_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userPostings_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userVocabulary_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userDocs_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | UPDATE | `userDocs_ru.new -> userDocs_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | DELETE | `userVocabulary_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | UPDATE | `userVocabulary_ru.new -> userVocabulary_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | DELETE | `userPostings_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | UPDATE | `userPostings_ru.new -> userPostings_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| _DbCopiesInfoBaseUse | INSERT |  |  |  |  |  |  |  |  |  | 1 st / 1 rows |

DDL statements by verb:

| verb | E01 | modules-excl | modules-dyn | modules-excl-2 | modules-dyn-2 | no-change | attribute | new-catalog | new-infobase |
|---|---|---|---|---|---|---|---|---|---|
| ALTER DATABASE |  |  |  |  |  |  |  |  | 1 |
| ALTER INDEX | 9 | 9 | 9 | 1 | 1 |  | 9 | 1 | 165 |
| ALTER TABLE |  |  |  |  |  |  |  |  | 10 |
| CREATE CLUSTERED INDEX |  |  |  |  |  |  |  |  | 48 |
| CREATE INDEX |  |  |  |  |  |  | 2 | 3 | 312 |
| CREATE TABLE | 2 | 2 | 2 | 2 | 2 |  | 5 | 8 | 1815 |
| CREATE UNIQUE CLUSTERED INDEX | 2 | 2 | 2 | 2 | 2 |  | 6 | 5 | 1613 |
| CREATE UNIQUE INDEX | 4 | 4 | 4 | 4 | 4 |  | 8 | 10 | 1984 |
| DROP INDEX | 3 | 3 | 3 | 3 | 3 |  | 7 | 7 | 211 |
| DROP TABLE | 2 | 2 | 2 | 2 | 2 |  | 5 | 3 | 62 |
| EXEC SP_RENAME | 5 | 5 | 5 | 5 | 5 |  | 14 | 19 | 5507 |
