# Write phases

12862 writes/DDL statements in 57 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:09.422 .. +00:09.428 | 183 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:09.556 .. +00:09.557 | 183 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:09.561 .. +00:09.702 | 183 | Config | 77 | 38 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x14; INSERT <guid>.new <- ConfigSave x14; DELETE <guid>.<n>.new <- ConfigSave x21; INSERT <guid>.<n>.new <- ConfigSave x21; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:09.706 .. +00:09.717 | 183 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:09.756 .. +00:09.939 | 183 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-b3fa0ef0-9968-11f1-8f4f-00e04c68009...` |
| 6 | +00:11.446 .. +00:11.494 | 129 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 7 | +00:11.510 .. +00:11.517 | 129 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 8 | +00:11.518 .. +00:11.520 | 129 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +00:11.619 .. +00:11.689 | 129 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 10 | +00:11.690 .. +00:11.691 | 129 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 11 | +00:11.692 .. +00:11.694 | 129 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 12 | +00:11.696 .. +00:11.697 | 129 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 13 | +00:11.700 .. +00:11.700 | 129 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 14 | +00:11.701 .. +00:11.702 | 129 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 15 | +00:11.704 .. +00:11.705 | 129 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 16 | +00:15.319 .. +00:15.457 | 129 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 17 | +00:15.468 .. +00:15.564 | 129 | _ConfigChngR_ExtPropsNG | 1 | 9996 |  | INSERT BULK x1 |  |
| 18 | +00:16.637 .. +00:16.794 | 129 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 19 | +00:16.803 .. +00:16.928 | 129 | _ConfigChngR_ExtPropsNG | 2 | 10658 |  | INSERT BULK x2 |  |
| 20 | +00:17.311 .. +00:17.354 | 129 | _ConfigChngRNG | 1 | 685 |  | INSERT BULK x1 |  |
| 21 | +00:17.371 .. +00:17.414 | 129 | _ConfigChngR_ExtPropsNG | 1 | 703 |  | INSERT BULK x1 |  |
| 22 | +00:17.451 .. +00:17.585 | 129 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 23 | +00:17.601 .. +00:17.774 | 129 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 24 | +00:17.779 .. +00:17.915 | 129 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 25 | +00:17.939 .. +00:18.005 | 129 | _ExtensionsRestruct | 4 | 4 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 26 | +00:19.361 .. +00:19.368 | 129 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 27 | +00:19.698 .. +00:19.877 | 129 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 28 | +00:21.958 .. +00:21.993 | 129 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 29 | +00:22.106 .. +00:22.263 | 129 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 30 | +00:22.538 .. +00:22.572 | 129 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 31 | +00:22.960 .. +01:36.802 | 129 | ConfigCAS | 12161 | 0 |  | DELETE <sha1> x12160; DELETE dbStruFinal<guid> x1 | `0002268f0fd24ea1bbde19a2e10fdc82bdcd9203` .. `ffe8c03da86fce9b38d6757817de0f982248d70c` |
| 32 | +01:36.816 .. +01:38.214 | 129 | Files | 311 | 2 | begin 1, commit 1 | DELETE userDocs_ru_<sha1>.bin x103; DELETE userPostings_ru_<sha1>.bin x103; DELETE userVocabulary_ru_<sha1>.bin x103; DELETE CAS_GC_Info x1; UPDATE CAS_GC_Info x1 | `userDocs_ru_0013adb20fcd7ec5b74a122401acf41c7a6...` .. `CAS_GC_Info` |
| 33 | +01:38.225 .. +01:43.509 | 129 | Params | 60 | 40 | begin 20, commit 20 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +4 more | `DBNames.New` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 34 | +01:43.662 .. +01:43.691 | 129 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 35 | +01:43.749 .. +01:43.776 | 129 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 36 | +01:43.792 .. +01:43.910 | 129 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 37 | +01:43.925 .. +01:43.929 | 129 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 38 | +01:43.935 .. +01:43.937 | 129 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 39 | +01:43.961 .. +01:45.006 | 129 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 40 | +01:45.178 .. +01:45.187 | 129 | PK___Referen__AC8ED0C4077A8F3B | 1 | 0 |  | ALTER INDEX x1 |  |
| 41 | +01:45.192 .. +01:45.216 | 129 | PK___Referen__AC8ED0C4DBBB6986 | 1 | 0 |  | ALTER INDEX x1 |  |
| 42 | +01:45.221 .. +01:45.226 | 129 | PK___Referen__AC8ED0C4720747AF | 1 | 0 |  | ALTER INDEX x1 |  |
| 43 | +01:45.228 .. +01:45.232 | 129 | PK___Enum108__AC8ED0C45E875D60 | 1 | 0 |  | ALTER INDEX x1 |  |
| 44 | +01:45.232 .. +01:45.242 | 129 | PK___Enum110__AC8ED0C4A5E2EE79 | 1 | 0 |  | ALTER INDEX x1 |  |
| 45 | +01:45.244 .. +01:45.259 | 129 | PK___Documen__AC8ED0C48EB57390 | 1 | 0 |  | ALTER INDEX x1 |  |
| 46 | +01:45.260 .. +01:45.287 | 129 | PK___Referen__AC8ED0C43F77AD82 | 1 | 0 |  | ALTER INDEX x1 |  |
| 47 | +01:45.288 .. +01:45.292 | 129 | PK___Referen__AC8ED0C45FCE1C7A | 1 | 0 |  | ALTER INDEX x1 |  |
| 48 | +01:45.293 .. +01:45.295 | 129 | PK___ConfigC__AC8ED0C4E0D77DBA | 1 | 0 |  | ALTER INDEX x1 |  |
| 49 | +01:45.335 .. +01:45.364 | 129 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 50 | +01:45.398 .. +01:45.423 | 129 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 51 | +01:45.635 .. +01:46.156 | 129 | Params | 39 | 33 | begin 1, commit 1 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `DBNames.New` .. `DynamicallyUpdated` |
| 52 | +01:46.204 .. +01:46.497 | 129 | Config | 87 | 84 |  | DELETE <guid> <- Config x16; UPDATE <guid>_dynupdate_<guid> -> <guid> x2; DELETE <guid>.<n> <- Config x23; UPDATE <guid>_dynupdate_<guid>.<n> -> <guid>.<n> x2; DELETE versions_dynupdate_<guid> x1; DELETE deleted_dynupdate_<guid> x1; UPDATE <guid>.<n>.new -> <guid>.<n> x21; UPDATE <guid>.new -> <guid> x14; ... +7 more | `a627e390-8fad-4a95-afe6-674f54813188` .. `DynamicallyUpdated` |
| 53 | +01:46.498 .. +01:46.520 | 129 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 54 | +01:46.527 .. +01:46.552 | 129 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 55 | +01:46.568 .. +01:46.574 | 129 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 56 | +01:46.618 .. +01:46.622 | 129 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 57 | +01:47.071 .. +01:47.446 | 129 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
