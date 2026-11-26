# Write phases

391 writes/DDL statements in 53 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:45.431 .. +00:45.450 | 153 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:47.423 .. +00:47.430 | 153 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:47.433 .. +00:47.760 | 153 | Config | 83 | 41 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x15; INSERT <guid>.new <- ConfigSave x15; DELETE <guid>.<n>.new <- ConfigSave x23; INSERT <guid>.<n>.new <- ConfigSave x23; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:47.778 .. +00:47.794 | 153 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:47.871 .. +00:48.050 | 153 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-d0d081f9-a401-11f1-8f51-00e04c68009...` |
| 6 | +00:51.197 .. +00:51.202 | 135 | _DbCopiesInfoBaseUse | 1 | 1 | begin 1 | UPDATE x1 |  |
| 7 | +00:51.204 .. +00:51.223 | 135 | _DbCopiesInitialLast | 1 | 0 |  | DELETE x1 |  |
| 8 | +00:51.224 .. +00:51.241 | 135 | _DbCopiesUpdates | 1 | 0 |  | DELETE x1 |  |
| 9 | +00:51.278 .. +00:51.300 | 135 | _DbCopiesTrChObj | 2 | 0 |  | TRUNCATE x2 |  |
| 10 | +00:51.301 .. +00:51.312 | 135 | _DbCopiesTrChanges | 2 | 0 | commit 1 | TRUNCATE x2 |  |
| 11 | +00:51.317 .. +00:51.363 | 135 | _ExtensionsRestructNGS | 1 | 3 |  | DELETE x1 |  |
| 12 | +00:51.404 .. +00:51.424 | 135 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 13 | +00:51.425 .. +00:51.431 | 135 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 14 | +00:51.514 .. +00:51.516 | 135 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 15 | +00:51.517 .. +00:51.518 | 135 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 16 | +00:51.518 .. +00:51.524 | 135 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 17 | +00:51.529 .. +00:51.532 | 135 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 18 | +00:51.537 .. +00:51.539 | 135 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 19 | +00:51.540 .. +00:51.541 | 135 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 20 | +00:51.543 .. +00:51.545 | 135 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 21 | +00:57.441 .. +00:57.490 | 135 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 22 | +00:57.495 .. +00:57.540 | 135 | _ConfigChngR_ExtPropsNG | 1 | 9929 |  | INSERT BULK x1 |  |
| 23 | +00:57.783 .. +00:57.822 | 135 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 24 | +00:57.823 .. +00:57.859 | 135 | _ConfigChngR_ExtPropsNG | 2 | 10504 |  | INSERT BULK x2 |  |
| 25 | +00:57.946 .. +00:57.963 | 135 | _ConfigChngRNG | 1 | 1219 |  | INSERT BULK x1 |  |
| 26 | +00:57.965 .. +00:57.989 | 135 | _ConfigChngR_ExtPropsNG | 1 | 1430 |  | INSERT BULK x1 |  |
| 27 | +00:57.996 .. +00:58.195 | 135 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 28 | +00:58.197 .. +00:58.221 | 135 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 29 | +00:58.222 .. +00:58.252 | 135 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 30 | +00:58.263 .. +00:58.276 | 135 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 31 | +01:02.567 .. +01:02.590 | 135 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 32 | +01:03.481 .. +01:03.785 | 135 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 33 | +01:08.388 .. +01:08.411 | 135 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 34 | +01:08.498 .. +01:08.580 | 135 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 35 | +01:08.765 .. +01:08.780 | 135 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 36 | +01:08.786 .. +01:08.890 | 135 | Params | 12 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +1 more | `DBNames.New` .. `DBNames-Ext-d0d081f9-a401-11f1-8f51-00e04c68009...` |
| 37 | +01:16.731 .. +01:19.139 | 135 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 38 | +01:19.267 .. +01:19.275 | 135 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 39 | +01:19.280 .. +01:19.282 | 135 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 40 | +01:19.285 .. +01:19.293 | 135 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 41 | +01:19.294 .. +01:19.297 | 135 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 42 | +01:19.298 .. +01:19.298 | 135 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 43 | +01:19.302 .. +01:19.803 | 135 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 44 | +01:19.945 .. +01:19.946 | 135 | PK___ConfigC__AC8ED0C4FB57324E | 1 | 0 |  | ALTER INDEX x1 |  |
| 45 | +01:19.969 .. +01:19.995 | 135 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 46 | +01:20.026 .. +01:20.047 | 135 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 47 | +01:20.108 .. +01:20.490 | 135 | Params | 39 | 33 | begin 1, commit 1 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `DBNames.New` .. `DynamicallyUpdated` |
| 48 | +01:20.516 .. +01:20.655 | 135 | Config | 83 | 82 |  | DELETE <guid>.<n> <- Config x23; UPDATE <guid>.<n>.new -> <guid>.<n> x23; DELETE <guid> <- Config x15; UPDATE <guid>.new -> <guid> x15; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +3 more | `06427cd3-a661-4ec9-a4e4-204b9c6b9d6c.0` .. `DynamicallyUpdated` |
| 49 | +01:20.656 .. +01:20.663 | 135 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 50 | +01:20.666 .. +01:20.673 | 135 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 51 | +01:20.674 .. +01:20.681 | 135 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 52 | +01:20.710 .. +01:20.713 | 135 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 53 | +01:21.252 .. +01:21.482 | 135 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
