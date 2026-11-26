# Write phases

12861 writes/DDL statements in 59 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:08.049 .. +00:08.069 | 134 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:08.239 .. +00:08.251 | 134 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:08.259 .. +00:08.918 | 134 | Config | 77 | 38 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x14; INSERT <guid>.new <- ConfigSave x14; DELETE <guid>.<n>.new <- ConfigSave x21; INSERT <guid>.<n>.new <- ConfigSave x21; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:08.937 .. +00:09.052 | 134 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:09.135 .. +00:09.227 | 134 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +00:11.715 .. +00:11.717 | 75 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 7 | +00:11.743 .. +00:11.758 | 75 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +00:11.758 .. +00:11.761 | 75 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +00:11.816 .. +00:11.819 | 75 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 10 | +00:11.821 .. +00:11.822 | 75 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 11 | +00:11.823 .. +00:11.829 | 75 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 12 | +00:11.833 .. +00:11.835 | 75 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 13 | +00:11.840 .. +00:11.842 | 75 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 14 | +00:11.847 .. +00:11.855 | 75 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 15 | +00:11.863 .. +00:11.866 | 75 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 16 | +00:12.035 .. +00:12.076 | 75 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 17 | +00:12.085 .. +00:12.112 | 75 | _ConfigChngR_ExtPropsNG | 1 | 9996 |  | INSERT BULK x1 |  |
| 18 | +00:12.248 .. +00:12.282 | 75 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 19 | +00:12.283 .. +00:12.340 | 75 | _ConfigChngR_ExtPropsNG | 2 | 10658 |  | INSERT BULK x2 |  |
| 20 | +00:12.507 .. +00:12.522 | 75 | _ConfigChngRNG | 1 | 685 |  | INSERT BULK x1 |  |
| 21 | +00:12.525 .. +00:12.536 | 75 | _ConfigChngR_ExtPropsNG | 1 | 703 |  | INSERT BULK x1 |  |
| 22 | +00:12.538 .. +00:12.585 | 75 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 23 | +00:12.586 .. +00:12.616 | 75 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 24 | +00:12.616 .. +00:12.644 | 75 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 25 | +00:12.648 .. +00:12.658 | 75 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 26 | +00:13.523 .. +00:13.538 | 75 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 27 | +00:13.817 .. +00:13.877 | 75 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 28 | +00:14.568 .. +00:14.571 | 75 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 29 | +00:14.616 .. +00:14.655 | 75 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 30 | +00:14.715 .. +00:14.725 | 75 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 31 | +00:14.894 .. +01:05.299 | 75 | ConfigCAS | 12161 | 0 |  | DELETE <sha1> x12160; DELETE dbStruFinal<guid> x1 | `0002268f0fd24ea1bbde19a2e10fdc82bdcd9203` .. `ffe8c03da86fce9b38d6757817de0f982248d70c` |
| 32 | +01:05.304 .. +01:06.063 | 75 | Files | 311 | 2 | begin 1, commit 1 | DELETE userDocs_ru_<sha1>.bin x103; DELETE userPostings_ru_<sha1>.bin x103; DELETE userVocabulary_ru_<sha1>.bin x103; DELETE CAS_GC_Info x1; UPDATE CAS_GC_Info x1 | `userDocs_ru_0013adb20fcd7ec5b74a122401acf41c7a6...` .. `CAS_GC_Info` |
| 33 | +01:06.071 .. +01:10.412 | 75 | Params | 60 | 40 | begin 20, commit 20 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +4 more | `DBNames.New` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 34 | +01:10.533 .. +01:10.544 | 75 | Config | 6 | 4 | begin 2, commit 2 | INSERT commit x1; DELETE commit x1; UPDATE commit x1; INSERT dynamicCommit x1; DELETE dynamicCommit x1; UPDATE dynamicCommit x1 | `commit` .. `dynamicCommit` |
| 35 | +01:10.549 .. +01:10.555 | 75 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 36 | +01:10.562 .. +01:10.575 | 75 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 37 | +01:10.578 .. +01:10.582 | 75 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 38 | +01:10.582 .. +01:10.585 | 75 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 39 | +01:10.590 .. +01:10.958 | 75 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 40 | +01:11.076 .. +01:11.079 | 75 | PK___Referen__AC8ED0C4077A8F3B | 1 | 0 |  | ALTER INDEX x1 |  |
| 41 | +01:11.079 .. +01:11.081 | 75 | PK___Referen__AC8ED0C4DBBB6986 | 1 | 0 |  | ALTER INDEX x1 |  |
| 42 | +01:11.080 .. +01:11.083 | 75 | PK___Referen__AC8ED0C4720747AF | 1 | 0 |  | ALTER INDEX x1 |  |
| 43 | +01:11.083 .. +01:11.085 | 75 | PK___Enum108__AC8ED0C45E875D60 | 1 | 0 |  | ALTER INDEX x1 |  |
| 44 | +01:11.085 .. +01:11.088 | 75 | PK___Enum110__AC8ED0C4A5E2EE79 | 1 | 0 |  | ALTER INDEX x1 |  |
| 45 | +01:11.088 .. +01:11.091 | 75 | PK___Documen__AC8ED0C48EB57390 | 1 | 0 |  | ALTER INDEX x1 |  |
| 46 | +01:11.090 .. +01:11.096 | 75 | PK___Referen__AC8ED0C43F77AD82 | 1 | 0 |  | ALTER INDEX x1 |  |
| 47 | +01:11.096 .. +01:11.099 | 75 | PK___Referen__AC8ED0C45FCE1C7A | 1 | 0 |  | ALTER INDEX x1 |  |
| 48 | +01:11.099 .. +01:11.100 | 75 | PK___ConfigC__AC8ED0C4F2A802E9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 49 | +01:11.120 .. +01:11.140 | 75 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 50 | +01:11.167 .. +01:11.190 | 75 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 51 | +01:11.243 .. +01:11.284 | 75 | Params | 40 | 18 | begin 2, commit 2 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>_dynupdate_<guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>_dynupdate_<guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1; ... +1 more | `DBNames.New` .. `DynamicallyUpdated` |
| 52 | +01:11.285 .. +01:11.289 | 75 | Config | 3 | 2 | begin 2, commit 1 | INSERT dbStruFinal x1; DELETE dbStruFinal x1; UPDATE dbStruFinal x1 | `dbStruFinal` |
| 53 | +01:11.366 .. +01:11.380 | 75 | _ConfigChngR | 1 | 49 |  | UPDATE x1 |  |
| 54 | +01:11.408 .. +01:11.476 | 75 | Config | 78 | 41 | begin 1, commit 1 | DELETE <guid>_dynupdate_<guid>.<n> <- Config x21; UPDATE <guid>.<n>.new -> <guid>_dynupdate_<guid>.<n> x21; DELETE <guid>_dynupdate_<guid> <- Config x14; UPDATE <guid>.new -> <guid>_dynupdate_<guid> x14; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +4 more | `04332cbc-afdb-4c23-b188-b32fff0a1eb0_dynupdate_...` .. `DynamicallyUpdated` |
| 55 | +01:11.478 .. +01:11.501 | 75 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 56 | +01:11.501 .. +01:11.512 | 75 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 57 | +01:11.513 .. +01:11.521 | 75 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 58 | +01:11.545 .. +01:11.550 | 75 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 59 | +01:11.849 .. +01:12.023 | 75 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
