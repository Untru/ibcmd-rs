| table | op | row name | БСП-long-steady(1x') | БСП-short(2n) | БСП-8.5-steady | УХ |
|---|---|---|---|---|---|---|
| Params | DELETE | `<guid>.ui` | 3 st / 0 rows | 3 st / 0 rows | 3 st / 0 rows | 2 st / 0 rows |
| Params | UPDATE | `<guid>.ui` | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.8 KB | 3 st / 3 rows / 23.9 KB | 2 st / 2 rows / 16.0 KB |
| Params | DELETE | `ibparams.inf` |  |  |  | 1 st / 0 rows |
| Params | UPDATE | `ibparams.inf` |  |  |  | 1 st / 1 rows / 324 B |
| Params | DELETE | `locale.inf` |  |  |  | 1 st / 0 rows |
| Params | UPDATE | `locale.inf` |  |  |  | 1 st / 1 rows / 112 B |
| ConfigSave | DELETE | `LIKE %.new` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `LIKE %.new` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `root.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `root.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `version.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `version.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `versions.new <- ConfigSave` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `versions.new <- ConfigSave` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | INSERT | `MobileVersions.datNEW` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | DELETE | `MobileVersions.datNEW` | 3 st / 0 rows | 1 st / 0 rows | 3 st / 0 rows | 1 st / 0 rows |
| Files | UPDATE | `MobileVersions.datNEW` | 3 st / 3 rows / 108.4 KB | 1 st / 1 rows / 36.1 KB | 3 st / 3 rows / 108.4 KB | 1 st / 1 rows / 80 B |
| Params | INSERT | `DBNames.New` | 2 st / 2 rows |  | 2 st / 2 rows | 2 st / 2 rows |
| Params | DELETE | `DBNames.New` | 4 st / 0 rows |  | 4 st / 0 rows | 4 st / 0 rows |
| Params | UPDATE | `DBNames.New` | 2 st / 2 rows / 280.5 KB |  | 2 st / 2 rows / 269.1 KB | 2 st / 2 rows / 31.8 KB |
| Params | INSERT | `DBNames-Ext-1.New` | 2 st / 2 rows |  | 2 st / 2 rows |  |
| Params | DELETE | `DBNames-Ext-1.New` | 4 st / 0 rows |  | 4 st / 0 rows |  |
| Params | UPDATE | `DBNames-Ext-1.New` | 2 st / 2 rows / 40 B |  | 2 st / 2 rows / 40 B |  |
| Params | INSERT | `DBNames-Ext-<guid>.New` | 4 st / 4 rows |  | 4 st / 4 rows |  |
| Params | DELETE | `DBNames-Ext-<guid>.New` | 8 st / 0 rows |  | 8 st / 0 rows |  |
| Params | UPDATE | `DBNames-Ext-<guid>.New` | 4 st / 4 rows / 36.7 KB |  | 4 st / 4 rows / 49.1 KB |  |
| _DbCopiesInfoBaseUse | UPDATE |  |  |  | 1 st / 1 rows | 1 st / 2 rows |
| _DbCopiesInitialLast | DELETE |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| _DbCopiesUpdates | DELETE |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| _DbCopiesTrChObj | TRUNCATE |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| _DbCopiesTrChanges | TRUNCATE |  |  |  | 1 st / 0 rows | 1 st / 0 rows |
| _ExtensionsRestructNGS | DELETE |  | 5 st / 4 rows | 1 st / 0 rows | 5 st / 4 rows | 1 st / 0 rows |
| SchemaStorage | UPDATE |  | 4 st / 4 rows |  | 4 st / 4 rows |  |
| _ConfigChngRNG | INSERT BULK |  | 3 st / 20685 rows |  | 3 st / 21219 rows |  |
| _ConfigChngR_ExtPropsNG | INSERT BULK |  | 4 st / 21357 rows |  | 4 st / 21863 rows |  |
| _ExtensionsRestruct | DELETE |  | 2 st / 2 rows |  | 2 st / 2 rows |  |
| _ExtensionsRestruct | INSERT |  | 2 st / 2 rows |  | 2 st / 2 rows |  |
| _ExtensionsRestructNGS | INSERT |  | 4 st / 4 rows |  | 4 st / 4 rows |  |
| Files | DELETE | `extd_props_cached/gc.mrk` | 2 st / 0 rows |  | 2 st / 0 rows |  |
| Files | INSERT | `extd_props_cached/gc.mrk` | 1 st / 1 rows |  | 1 st / 1 rows |  |
| Files | UPDATE | `extd_props_cached/gc.mrk` | 1 st / 1 rows / 9 B |  | 1 st / 1 rows / 9 B |  |
| Params | INSERT | `<guid>.sinew` | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows |
| Params | DELETE | `<guid>.sinew` | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows | 16 st / 0 rows |
| Params | UPDATE | `<guid>.sinew` | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 1.5 MB | 16 st / 16 rows / 238.2 KB |
| Config | INSERT | `commit` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `commit` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |
| Config | UPDATE | `commit` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| DBSchema | UPDATE |  | 1 st / 1 rows |  | 1 st / 1 rows |  |
| Params | DELETE | `<guid>.si <- Params` | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows |
| Params | UPDATE | `<guid>.sinew -> <guid>.si` | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows | 16 st / 16 rows |
| Params | DELETE | `siVersions` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Params | UPDATE | `siVersions` | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB | 1 st / 1 rows / 1.2 KB |
| Params | DELETE | `DynamicallyUpdated` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | INSERT | `dbStruFinal` |  | 1 st / 1 rows |  |  |
| Config | DELETE | `<guid>.<n> <- Config` | 21 st / 21 rows | 6 st / 6 rows | 23 st / 23 rows | 10 st / 10 rows |
| Config | UPDATE | `<guid>_dynupdate_<guid>.<n> -> <guid>.<n>` |  | 2 st / 2 rows |  |  |
| Config | DELETE | `versions_dynupdate_<guid>` |  | 1 st / 0 rows |  |  |
| Config | DELETE | `deleted_dynupdate_<guid>` |  | 1 st / 0 rows |  |  |
| Config | UPDATE | `<guid>.<n>.new -> <guid>.<n>` | 21 st / 21 rows | 4 st / 4 rows | 23 st / 23 rows | 10 st / 10 rows |
| Config | DELETE | `<guid> <- Config` | 14 st / 14 rows | 4 st / 4 rows | 15 st / 15 rows | 10 st / 10 rows |
| Config | UPDATE | `<guid>_dynupdate_<guid> -> <guid>` |  | 2 st / 2 rows |  |  |
| Config | UPDATE | `<guid>.new -> <guid>` | 14 st / 14 rows | 2 st / 2 rows | 15 st / 15 rows | 10 st / 10 rows |
| Config | DELETE | `root <- Config` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | UPDATE | `root.new -> root` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `version <- Config` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | UPDATE | `version.new -> version` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `versions <- Config` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | UPDATE | `versions.new -> versions` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `DynamicallyUpdated` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| ConfigSave | DELETE | `LIKE %` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Files | DELETE | `MobileVersions.dat <- Files` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Files | UPDATE | `MobileVersions.datNEW -> MobileVersions.dat` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |
| Config | DELETE | `dynamicCommit` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Config | DELETE | `dbStruFinal` | 1 st / 0 rows | 2 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Params | INSERT | `<guid>.ui` |  |  |  | 2 st / 2 rows |
| Config | UPDATE | `dbStruFinal` |  | 1 st / 1 rows |  |  |
| _ConfigChngR | UPDATE |  |  | 1 st / 7 rows |  |  |
| Files | DELETE | `userDocs_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userDocs_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userDocs_ru.new` | 4 st / 4 rows / 46.2 KB | 4 st / 4 rows / 46.4 KB | 4 st / 4 rows / 47.2 KB |  |
| Files | DELETE | `userVocabulary_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userVocabulary_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userVocabulary_ru.new` | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 103.1 KB | 4 st / 4 rows / 104.3 KB |  |
| Files | DELETE | `userPostings_ru.new` | 2 st / 0 rows | 2 st / 0 rows | 2 st / 0 rows |  |
| Files | INSERT | `userPostings_ru.new` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows |  |
| Files | UPDATE | `userPostings_ru.new` | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB | 4 st / 4 rows / 511.9 KB |  |
| Files | DELETE | `userDocs_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userPostings_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userVocabulary_ru.bin` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |  |
| Files | DELETE | `userDocs_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Files | UPDATE | `userDocs_ru.new -> userDocs_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Files | DELETE | `userVocabulary_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Files | UPDATE | `userVocabulary_ru.new -> userVocabulary_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Files | DELETE | `userPostings_ru.bin <- Files` | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows | 1 st / 0 rows |
| Files | UPDATE | `userPostings_ru.new -> userPostings_ru.bin` | 1 st / 1 rows | 1 st / 1 rows | 1 st / 1 rows | 1 st / 0 rows |
| Files | DELETE | `userDocs_en.bin <- Files` |  |  |  | 1 st / 0 rows |
| Files | UPDATE | `userDocs_en.new -> userDocs_en.bin` |  |  |  | 1 st / 0 rows |
| Files | DELETE | `userVocabulary_en.bin <- Files` |  |  |  | 1 st / 0 rows |
| Files | UPDATE | `userVocabulary_en.new -> userVocabulary_en.bin` |  |  |  | 1 st / 0 rows |
| Files | DELETE | `userPostings_en.bin <- Files` |  |  |  | 1 st / 0 rows |
| Files | UPDATE | `userPostings_en.new -> userPostings_en.bin` |  |  |  | 1 st / 0 rows |

DDL statements by verb:

| verb | БСП-long-steady(1x') | БСП-short(2n) | БСП-8.5-steady | УХ |
|---|---|---|---|---|
| ALTER INDEX | 1 |  | 1 |  |
| CREATE TABLE | 2 |  | 2 |  |
| CREATE UNIQUE CLUSTERED INDEX | 2 |  | 2 |  |
| CREATE UNIQUE INDEX | 4 |  | 4 |  |
| DROP INDEX | 3 |  | 3 |  |
| DROP TABLE | 2 |  | 2 |  |
| EXEC SP_RENAME | 5 |  | 5 |  |
| TRUNCATE TABLE |  |  | 2 | 2 |
